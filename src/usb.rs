use std::env;
use tokio::io::unix::AsyncFd;
use udev::{EventType, MonitorBuilder};
use crate::config::{load_config, Config};
use crate::monitor_handling::handle_if_changed;
use crate::udev_utils::{is_device_duo_keyboard, is_it_duo_keyboard};
use std::time::Duration;
use rusb::{Context, UsbContext};
use crate::config;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DeviceState {
    Added,
    Removed,
}

pub fn set_backlight_level(level: u8, config: &Config) -> Result<(), rusb::Error> {
    if level > 3 {
        return Err(rusb::Error::InvalidParam);
    }

    let vendor_id = u16::from_str_radix(&config.device.vendor_id, 16).map_err(|_| {
        eprintln!("Error: Invalid Vendor ID in config: '{}'", config.device.vendor_id);
        rusb::Error::InvalidParam
    })?;

    let product_id = u16::from_str_radix(&config.device.product_id, 16).map_err(|_| {
        eprintln!("Error: Invalid Product ID in config: '{}'", config.device.product_id);
        rusb::Error::InvalidParam
    })?;

    let context = Context::new()?;
    let handle = context.open_device_with_vid_pid(vendor_id, product_id)
        .ok_or(rusb::Error::NoDevice)?;

    let report_id: u8 = 0x5A;
    let w_value: u16 = 0x035A;
    let w_index: u16 = 4;

    let mut data = [0u8; 16];
    data[0] = report_id;
    data[1] = 0xBA;
    data[2] = 0xC5;
    data[3] = 0xC4;
    data[4] = level;

    let has_kernel_driver = handle.kernel_driver_active(w_index as u8).unwrap_or(false);
    if has_kernel_driver {
        handle.detach_kernel_driver(w_index as u8)?;
    }

    handle.claim_interface(w_index as u8)?;

    handle.write_control(
        0x21, // bmRequestType
        0x09, // bRequest (SET_REPORT)
        w_value,
        w_index,
        &data,
        Duration::from_secs(1),
    )?;

    handle.release_interface(w_index as u8)?;

    if has_kernel_driver {
        let _ = handle.attach_kernel_driver(w_index as u8);
    }

    Ok(())
}

pub fn run_backlight_command(args: &[String]) {
    if args.len() < 2 {
        eprintln!("Usage: zenbook-duo --backlight <0-3>");
        eprintln!("Set the keyboard backlight level to the specified value or default from config.");
        std::process::exit(1);
    }

    // 1. Handle Elevation
    if env::var("USER").unwrap_or_default() != "root" {
        println!("Backlight control requires root privileges. Re-running with sudo...");
        let current_exe = env::current_exe().expect("Failed to get current executable path");
        let status = std::process::Command::new("sudo")
            .arg(current_exe)
            .args(&args[1..])
            .status();

        match status {
            Ok(s) if s.success() => return,
            _ => {
                std::process::exit(1);
            }
        }
    }

    // 2. Parse and Execute
    let config = config::load_config_interactive();

    // Check if a level was provided, otherwise use the one from config
    let level = if let Some(level_str) = args.get(2) {
        match level_str.parse::<u8>() {
            Ok(l) if l <= 3 => l,
            _ => {
                eprintln!("Invalid level '{}'. Please provide an integer between 0 and 3.", level_str);
                std::process::exit(1);
            }
        }
    } else {
        println!("No level provided, using default from config: {}", config.brightness);
        config.brightness as u8
    };

    if let Err(e) = set_backlight_level(level, &config) {
        print_backlight_error(e, &config);
        std::process::exit(1);
    }
    println!("Backlight successfully set to level {}", level);
}

fn print_backlight_error(err: rusb::Error, config: &Config) {
    match err {
        rusb::Error::NoDevice => {
            eprintln!("Error: Keyboard not found (VID: {}, PID: {}).",
                      config.device.vendor_id, config.device.product_id);
        },
        rusb::Error::Access => {
            eprintln!("Error: Permission denied. Even with sudo, the device might be busy.");
        },
        _ => eprintln!("USB Error: {:?}", err),
    }
}

pub async fn monitor_usb_events() {
    let config = load_config();
    let (mut current_state, mut keyboard_devpath) = check_initial_state(&config);
    let mut last_processed_state: Option<DeviceState> = None;

    // If we found it on startup, apply the state immediately
    handle_if_changed(&current_state, &last_processed_state, &config);
    last_processed_state = current_state.clone();

    let builder = MonitorBuilder::new().expect("Failed to create udev monitor builder");

    // Filter only for the USB subsystem, similar to "udevadm monitor --subsystem-match=usb"
    let builder = builder
        .match_subsystem("usb")
        .expect("Failed to match usb subsystem");

    let monitor = builder.listen().expect("Failed to listen to udev monitor");

    // AsyncFd is required to poll the file descriptor in an async context
    let mut async_monitor = AsyncFd::new(monitor).expect("Failed to create AsyncFd");
    // keyboard_devpath is already initialized above

    println!("Started monitoring USB events...");

    loop {
        // Wait for the monitor socket to be readable
        let mut guard = async_monitor
            .readable_mut()
            .await
            .expect("Failed to wait for readability");

        // We need to access the inner monitor to iterate over events
        let monitor = guard.get_inner_mut();

        for event in monitor.iter() {
            if let Some(devpath) = is_it_duo_keyboard(&event, &keyboard_devpath, &config.device) {
                if event.event_type() == EventType::Add {
                    keyboard_devpath = Some(devpath);
                } else if event.event_type() == EventType::Remove {
                    keyboard_devpath = None;
                }
            } else {
                continue;
            }

            match event.event_type() {
                EventType::Add => update_if_not_yet(&mut current_state, DeviceState::Added),
                EventType::Remove => update_if_not_yet(&mut current_state, DeviceState::Removed),
                _ => continue, // Ignore other events like Change or Move if not needed
            }
        }

        if current_state != last_processed_state {
            handle_if_changed(&current_state, &last_processed_state, &config);
            last_processed_state = current_state.clone();
        }

        // Signal that we have processed the available events.
        // If we didn't read everything, the next poll would wake up immediately.
        guard.clear_ready();
    }
}

fn update_if_not_yet(last_state: &mut Option<DeviceState>, new_state: DeviceState) {
    match last_state {
        None => *last_state = Some(new_state),

        Some(last_state_value) => match (last_state_value, new_state.clone()) {
            (DeviceState::Added, DeviceState::Removed) => *last_state = Some(new_state),
            (DeviceState::Removed, DeviceState::Added) => *last_state = Some(new_state),
            _ => {}
        },
    }
}

fn check_initial_state(config: &Config) -> (Option<DeviceState>, Option<std::ffi::OsString>) {
    let mut enumerator = udev::Enumerator::new().expect("Failed to create enumerator");
    enumerator
        .match_subsystem("usb")
        .expect("Failed to match subsystem");

    for device in enumerator.scan_devices().expect("Failed to scan devices") {
        // We can reuse the logic inside is_it_duo_keyboard, but we need to construct a dummy event
        // or just check properties directly. Since is_it_duo_keyboard takes an Event,
        // let's just copy the check logic here for the udev::Device.

        if is_device_duo_keyboard(&device, &config.device) {
            println!("Initial check: Keyboard found at {:?}", device.devpath());
            return (
                Some(DeviceState::Added),
                Some(device.devpath().to_os_string()),
            );
        }
    }

    println!("Initial check: Keyboard not found");
    (Some(DeviceState::Removed), None)
}

#[cfg(test)]
mod tests {
    use super::{DeviceState, update_if_not_yet};

    #[test]
    fn update_sets_from_none() {
        let mut state: Option<DeviceState> = None;
        update_if_not_yet(&mut state, DeviceState::Added);
        assert_eq!(state, Some(DeviceState::Added));
    }

    #[test]
    fn update_ignores_same_state_added() {
        let mut state = Some(DeviceState::Added);
        update_if_not_yet(&mut state, DeviceState::Added);
        assert_eq!(state, Some(DeviceState::Added));
    }

    #[test]
    fn update_ignores_same_state_removed() {
        let mut state = Some(DeviceState::Removed);
        update_if_not_yet(&mut state, DeviceState::Removed);
        assert_eq!(state, Some(DeviceState::Removed));
    }

    #[test]
    fn update_switches_added_to_removed() {
        let mut state = Some(DeviceState::Added);
        update_if_not_yet(&mut state, DeviceState::Removed);
        assert_eq!(state, Some(DeviceState::Removed));
    }

    #[test]
    fn update_switches_removed_to_added() {
        let mut state = Some(DeviceState::Removed);
        update_if_not_yet(&mut state, DeviceState::Added);
        assert_eq!(state, Some(DeviceState::Added));
    }
}
