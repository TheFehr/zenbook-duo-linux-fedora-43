use std::{thread};
use std::process::{Command, Stdio};
use std::time::Duration;
use crate::config::Config;
use crate::usb::DeviceState;

pub fn handle_if_changed(current: &Option<DeviceState>, before: &Option<DeviceState>, config: &Config) {
    // --------------------------------------------------------------
    // 1️⃣ Gather the shared arguments once
    // --------------------------------------------------------------
    let scale = config.scale.to_string();

    // These are the arguments that never change:
    let base_args = vec![
        "set",
        "--logical-monitor",
        "--primary",
        "--scale",
        &scale,
        "--monitor",
        "eDP-1",
    ];

    // --------------------------------------------------------------
    // 2️⃣ Choose which full argument list to run
    // --------------------------------------------------------------
    match (current, before) {
        // ── Keyboard added → single‑monitor layout ─────────────────────
        (Some(DeviceState::Added), None) |
        (Some(DeviceState::Added), Some(DeviceState::Removed)) => {
            println!("Zenbook Duo Keyboard detected!");

            thread::sleep(Duration::from_millis(500));

            // Debug: Log the full command string
            println!("DEBUG: Executing 'gdctl {}'", base_args.join(" "));

            // No extra args – just use the base list
            let status = Command::new("gdctl")
                .args(&base_args)
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .status();

            match status {
                Ok(s) => {
                    if !s.success() {
                        eprintln!("gdctl exited with status: {}", s);
                    }
                }
                Err(e) => eprintln!("Failed to execute gdctl: {}", e),
            }
        }

        // ── Keyboard removed → dual‑monitor layout ───────────────────────
        (Some(DeviceState::Removed), None) |
        (Some(DeviceState::Removed), Some(DeviceState::Added)) => {
            println!("Zenbook Duo Keyboard removed!");

            // Extend a **copy** of the base args with the second monitor bits
            let mut dual_args = base_args.clone(); // cheap Vec clone
            dual_args.extend_from_slice(&[
                "--logical-monitor",
                "--scale",
                &scale,
                "--monitor",
                "eDP-2",
                "--below",
                "eDP-1",
            ]);

            thread::sleep(Duration::from_millis(500));

            // Debug: Log the full command string
            println!("DEBUG: Executing 'gdctl {}'", dual_args.join(" "));

            let status = Command::new("gdctl")
                .args(&dual_args)
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .status();

            match status {
                Ok(s) => {
                    if !s.success() {
                        eprintln!("gdctl exited with status: {}", s);
                    }
                }
                Err(e) => eprintln!("Failed to execute gdctl: {}", e),
            }
        }

        // Anything else – nothing to do
        _ => {}
    }
}