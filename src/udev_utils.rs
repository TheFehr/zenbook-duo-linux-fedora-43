use udev::Event;
use std::ffi::OsString;
use std::path::Path;
use std::fs;
use std::process::Command;
use crate::config::DeviceConfig;

pub fn is_device_duo_keyboard(device: &udev::Device, config: &DeviceConfig) -> bool {
    let mut vendor_match = false;
    let mut product_match = false;

    for prop in device.properties() {
        if let (Some(name), Some(val)) = (prop.name().to_str(), prop.value().to_str()) {
            check_property(name, val, &mut vendor_match, &mut product_match, config);
        }
    }
    vendor_match && product_match
}

pub fn is_it_duo_keyboard(event: &Event, known_devpath: &Option<OsString>, config: &DeviceConfig) -> Option<OsString> {
    // If we know the devpath (from a previous ADD), and this is a REMOVE event matching that path, return it.
    if let Some(known) = known_devpath {
        if event.event_type() == udev::EventType::Remove {
            let known_path = Path::new(known);
            let event_path = Path::new(event.devpath());

            // Check if paths match, or if one is a sub-path of the other (e.g. interface vs device)
            if known_path == event_path || known_path.starts_with(event_path) || event_path.starts_with(known_path) {
                return Some(known.clone());
            }
        }
    }

    let mut vendor_match = false;
    let mut product_match = false;

    for prop in event.properties() {
        if let (Some(name), Some(val)) = (prop.name().to_str(), prop.value().to_str()) {
            check_property(name, val, &mut vendor_match, &mut product_match, config);
        }
    }

    if vendor_match && product_match {
        Some(event.devpath().to_os_string())
    } else {
        None
    }
}

pub fn find_keyboard_event_path(config: &DeviceConfig) -> Option<std::path::PathBuf> {
    let mut enumerator = udev::Enumerator::new().ok()?;
    enumerator.match_subsystem("input").ok()?;

    for device in enumerator.scan_devices().ok()? {
        // We look for a parent device that matches our keyboard VID/PID
        let mut current_dev = Some(device.clone());
        while let Some(dev) = current_dev {
            if is_device_duo_keyboard(&dev, config) {
                // If we found the keyboard parent, the original 'device'
                // is likely the /dev/input/eventX node
                if let Some(devnode) = device.devnode() {
                    if devnode.to_string_lossy().contains("event") {
                        return Some(devnode.to_path_buf());
                    }
                }
            }
            current_dev = dev.parent();
        }
    }
    None
}

fn check_property(name: &str, val: &str, vendor_match: &mut bool, product_match: &mut bool, config: &DeviceConfig) {
    match name {
        "ID_VENDOR_ID" | "ID_VENDOR" => {
            if val.trim().eq_ignore_ascii_case(&config.vendor_id) {
                *vendor_match = true;
            }
        }
        "ID_MODEL_ID" | "ID_MODEL" => {
            if val.trim().eq_ignore_ascii_case(&config.product_id) {
                *product_match = true;
            }
        }
        "PRODUCT" => {
            let parts: Vec<&str> = val.trim().split('/').collect();
            if parts.len() >= 2 {
                if parts[0].eq_ignore_ascii_case(&config.vendor_id) {
                    *vendor_match = true;
                }
                if parts[1].eq_ignore_ascii_case(&config.product_id) {
                    *product_match = true;
                }
            }
        }
        _ => {}
    }
}

pub fn ensure_touch_rule() {
    let mut found = false;
    if let Ok(mut enumerator) = udev::Enumerator::new() {
        if let Ok(_) = enumerator.match_subsystem("input") {
            if let Ok(devices) = enumerator.scan_devices() {
                for device in devices {
                    let mut vendor_match = false;
                    let mut product_match = false;

                    for prop in device.properties() {
                        if let (Some(name), Some(val)) = (prop.name().to_str(), prop.value().to_str()) {
                            if (name == "ID_VENDOR_ID" || name == "ID_VENDOR") && val == "04f3" {
                                vendor_match = true;
                            }
                            if (name == "ID_MODEL_ID" || name == "ID_MODEL") && val == "4448" {
                                product_match = true;
                            }
                        }
                    }

                    if vendor_match && product_match {
                        found = true;
                        break;
                    }
                }
            }
        }
    }

    if !found {
        println!("Lower touchscreen (Vendor 04f3, Model 4448) not detected. Skipping udev touch rule installation.");
        return;
    }

    println!("Lower touchscreen detected. Installing udev touch rule...");
    let tmp_path = Path::new("/tmp/99-zenbook-touch.rules");
    let target_path = "/etc/udev/rules.d/99-zenbook-touch.rules";
    let rule_content = "ENV{ID_INPUT_TOUCHSCREEN}==\"1\", ENV{ID_VENDOR_ID}==\"04f3\", ENV{ID_MODEL_ID}==\"4448\", ENV{LIBINPUT_CALIBRATION_MATRIX}=\"1 0 0 0 0.5 0.5 0 0 1\"\n";

    if let Err(e) = fs::write(tmp_path, rule_content) {
        eprintln!("Failed to write temporary udev rule: {}", e);
        return;
    }

    let mv_status = Command::new("sudo")
        .arg("mv")
        .arg("-f")
        .arg(tmp_path)
        .arg(target_path)
        .status();

    match mv_status {
        Ok(s) if s.success() => {},
        Ok(s) => {
            eprintln!("Failed to move udev rule into place (exit code {}).", s);
            return;
        }
        Err(e) => {
            eprintln!("Failed to execute sudo mv for udev rule: {}", e);
            return;
        }
    }

    let reload_status = Command::new("sudo")
        .args(&["udevadm", "control", "--reload-rules"])
        .status();

    match reload_status {
        Ok(s) if !s.success() => eprintln!("Failed to reload udev rules (exit code {}).", s),
        Err(e) => eprintln!("Failed to execute udevadm control: {}", e),
        _ => {}
    }

    let trigger_status = Command::new("sudo")
        .args(&["udevadm", "trigger"])
        .status();

    match trigger_status {
        Ok(s) if s.success() => println!("Udev touch rule installed successfully."),
        Ok(s) => eprintln!("Failed to trigger udev rules (exit code {}).", s),
        Err(e) => eprintln!("Failed to execute udevadm trigger: {}", e),
    }
}

pub fn remove_touch_rule() {
    println!("Removing udev touch rule...");
    let target_path = "/etc/udev/rules.d/99-zenbook-touch.rules";

    if Path::new(target_path).exists() {
        let rm_status = Command::new("sudo")
            .arg("rm")
            .arg("-f")
            .arg(target_path)
            .status();

        match rm_status {
            Ok(s) if s.success() => println!("Removed {}.", target_path),
            Ok(s) => eprintln!("Failed to remove udev rule (exit code {}).", s),
            Err(e) => eprintln!("Failed to execute sudo rm for udev rule: {}", e),
        }

        let reload_status = Command::new("sudo")
            .args(&["udevadm", "control", "--reload-rules"])
            .status();

        match reload_status {
            Ok(s) if !s.success() => eprintln!("Failed to reload udev rules (exit code {}).", s),
            Err(e) => eprintln!("Failed to execute udevadm control: {}", e),
            _ => {}
        }

        let trigger_status = Command::new("sudo")
            .args(&["udevadm", "trigger"])
            .status();

        match trigger_status {
            Ok(s) if s.success() => println!("Udev touch rule removed successfully."),
            Ok(s) => eprintln!("Failed to trigger udev rules (exit code {}).", s),
            Err(e) => eprintln!("Failed to execute udevadm trigger: {}", e),
        }
    } else {
        println!("Udev touch rule not found. Skipping removal.");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> DeviceConfig {
        DeviceConfig { vendor_id: "b05".into(), product_id: "1bf2".into() }
    }

    #[test]
    fn vendor_id_matches_on_id_vendor_id() {
        let mut v = false;
        let mut p = false;
        check_property("ID_VENDOR_ID", "b05", &mut v, &mut p, &cfg());
        assert!(v);
        assert!(!p);
    }

    #[test]
    fn product_id_matches_on_id_model_id() {
        let mut v = false;
        let mut p = false;
        check_property("ID_MODEL_ID", "1bf2", &mut v, &mut p, &cfg());
        assert!(!v);
        assert!(p);
    }

    #[test]
    fn product_field_splits_and_matches() {
        let mut v = false;
        let mut p = false;
        // Typical PRODUCT format is "vendor/product/version" but we only need first two
        check_property("PRODUCT", "b05/1bf2/1234", &mut v, &mut p, &cfg());
        assert!(v);
        assert!(p);
    }

    #[test]
    fn non_matching_values_do_not_set_flags() {
        let mut v = false;
        let mut p = false;
        check_property("ID_VENDOR_ID", "abcd", &mut v, &mut p, &cfg());
        check_property("ID_MODEL_ID", "ef12", &mut v, &mut p, &cfg());
        assert!(!v);
        assert!(!p);
    }
}