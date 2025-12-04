use tokio::io::unix::AsyncFd;
use udev::{EventType, MonitorBuilder};
use crate::config::{load_config, Config};
use crate::monitor_handling::handle_if_changed;
use crate::udev_utils::{is_device_duo_keyboard, is_it_duo_keyboard};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DeviceState {
    Added,
    Removed,
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
