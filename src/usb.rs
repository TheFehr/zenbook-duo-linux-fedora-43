pub(crate) mod backlight;

use evdev::{Device};
use std::time::{Duration, Instant};
use tokio::io::unix::AsyncFd;
use udev::{EventType, MonitorBuilder};
use crate::config::{load_config, Config};
use crate::monitor_handling::handle_if_changed;
use crate::udev_utils::{is_device_duo_keyboard, is_it_duo_keyboard};
use crate::{udev_utils};
use log::{info, error};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceState {
    Added,
    Removed,
}

pub async fn monitor_usb_events() {
    let config = load_config();
    let (mut current_state, mut keyboard_devpath) = check_initial_state(&config);
    let mut last_processed_state: Option<DeviceState> = None;

    // Give the desktop session a moment to settle so environment variables are more likely to be there
    tokio::time::sleep(Duration::from_millis(1000)).await;

    let builder = MonitorBuilder::new().expect("Failed to create udev monitor builder");

    // Filter only for the USB subsystem, similar to "udevadm monitor --subsystem-match=usb"
    let builder = builder
        .match_subsystem("usb")
        .expect("Failed to match usb subsystem");

    let monitor = builder.listen().expect("Failed to listen to udev monitor");

    // AsyncFd is required to poll the file descriptor in an async context
    let mut async_monitor = AsyncFd::new(monitor).expect("Failed to create AsyncFd");
    // keyboard_devpath is already initialized above

    info!("Started monitoring USB events...");

    loop {
        // If we have a pending state change that hasn't been successfully applied yet,
        // don't wait indefinitely for udev events.
        if current_state != last_processed_state {
            if handle_if_changed(&current_state, &last_processed_state, &config) {
                last_processed_state = current_state.clone();
            } else {
                // Wait a bit before retrying if it failed (e.g. DE not ready)
                tokio::time::sleep(Duration::from_millis(2000)).await;
                continue; 
            }
        }

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

pub fn check_initial_state(config: &Config) -> (Option<DeviceState>, Option<std::ffi::OsString>) {
    let mut enumerator = udev::Enumerator::new().expect("Failed to create enumerator");
    enumerator
        .match_subsystem("usb")
        .expect("Failed to match subsystem");

    for device in enumerator.scan_devices().expect("Failed to scan devices") {
        // We can reuse the logic inside is_it_duo_keyboard, but we need to construct a dummy event
        // or just check properties directly. Since is_it_duo_keyboard takes an Event,
        // let's just copy the check logic here for the udev::Device.

        if is_device_duo_keyboard(&device, &config.device) {
            info!("Initial check: Keyboard found at {:?}", device.devpath());
            return (
                Some(DeviceState::Added),
                Some(device.devpath().to_os_string()),
            );
        }
    }

    info!("Initial check: Keyboard not found");
    (Some(DeviceState::Removed), None)
}

pub async fn monitor_special_keys(config: Config) {
    // Keep the "active" level locally; config.brightness is just the default.
    let mut current_level: u8 = (config.brightness as u8).min(3);

    // Debounce: ignore additional matching events right after a toggle.
    let mut last_toggle_at: Option<Instant> = None;
    let debounce_window = Duration::from_millis(250);

    loop {
        if let Some(path) = udev_utils::find_keyboard_event_path(&config.device) {
            if let Ok(mut device) = Device::open(&path) {
                info!("Listening for special keys on {:?}", path);
                loop {
                    match device.fetch_events() {
                        Ok(iterator) => {
                            for event in iterator {
                                if event.event_type() == evdev::EventType::KEY
                                    && event.code() == 190 // F4/Backlight key
                                    && event.value() == 1 // Key down
                                {
                                    let now = Instant::now();
                                    if let Some(last) = last_toggle_at {
                                        if now.duration_since(last) < debounce_window {
                                            continue;
                                        }
                                    }

                                    current_level = (current_level + 1) % 4;
                                    info!("Backlight key pressed, toggling to level {}", current_level);
                                    if let Err(e) = backlight::set_backlight_level(current_level, &config) {
                                        error!("Failed to set backlight via special key: {}", e);
                                    }
                                    last_toggle_at = Some(now);
                                }
                            }
                        }
                        Err(e) => {
                            error!("Error fetching evdev events: {}", e);
                            break; // Re-open the device
                        }
                    }
                }
            }
        }
        tokio::time::sleep(Duration::from_millis(1000)).await;
    }
}
