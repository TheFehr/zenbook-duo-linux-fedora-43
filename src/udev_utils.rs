use udev::Event;
use std::ffi::OsString;
use std::path::Path;
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