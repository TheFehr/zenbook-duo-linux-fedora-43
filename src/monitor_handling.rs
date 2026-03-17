use std::thread;
use std::process::{Command, Stdio};
use std::time::Duration;
use crate::config::Config;
use crate::usb::backlight::set_backlight_level;
use crate::usb::DeviceState;
use log::{info, debug, error};

trait DisplayManager {
    fn set_single_monitor(&self, scale: &str);
    fn set_dual_monitor(&self, scale: &str);
}

struct GnomeManager;

impl DisplayManager for GnomeManager {
    fn set_single_monitor(&self, scale: &str) {
        let base_args = vec![
            "set", "--logical-monitor", "--primary", "--scale", scale, "--monitor", "eDP-1"
        ];
        debug!("Executing 'gdctl {}'", base_args.join(" "));
        
        if let Err(e) = Command::new("gdctl")
            .args(&base_args)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status() 
        {
            error!("Failed to execute gdctl: {}", e);
        }
    }

    fn set_dual_monitor(&self, scale: &str) {
        let dual_args = vec![
            "set", "--logical-monitor", "--primary", "--scale", scale, "--monitor", "eDP-1",
            "--logical-monitor", "--scale", scale, "--monitor", "eDP-2", "--below", "eDP-1"
        ];
        debug!("Executing 'gdctl {}'", dual_args.join(" "));
        
        if let Err(e) = Command::new("gdctl")
            .args(&dual_args)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status() 
        {
            error!("Failed to execute gdctl: {}", e);
        }
    }
}

struct KdeManager;

impl DisplayManager for KdeManager {
    fn set_single_monitor(&self, scale: &str) {
        let args = vec![
            format!("output.eDP-1.scale.{}", scale),
            "output.eDP-2.disable".to_string(),
        ];
        debug!("Executing 'kscreen-doctor {}'", args.join(" "));
        
        if let Err(e) = Command::new("kscreen-doctor")
            .args(&args)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status() 
        {
            error!("Failed to execute kscreen-doctor: {}", e);
        }
    }

    fn set_dual_monitor(&self, scale: &str) {
        let args = vec![
            format!("output.eDP-1.scale.{}", scale),
            "output.eDP-2.enable".to_string(),
            format!("output.eDP-2.scale.{}", scale),
        ];
        debug!("Executing 'kscreen-doctor {}'", args.join(" "));
        
        if let Err(e) = Command::new("kscreen-doctor")
            .args(&args)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status() 
        {
            error!("Failed to execute kscreen-doctor: {}", e);
        }
    }
}

fn get_display_manager() -> Box<dyn DisplayManager> {
    if std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default().to_uppercase().contains("KDE") {
        Box::new(KdeManager)
    } else {
        Box::new(GnomeManager)
    }
}

pub fn handle_if_changed(current: &Option<DeviceState>, before: &Option<DeviceState>, config: &Config) {
    let scale = config.scale.to_string();
    let manager = get_display_manager();

    match (current, before) {
        // ── Keyboard added → single‑monitor layout ─────────────────────
        (Some(DeviceState::Added), None) |
        (Some(DeviceState::Added), Some(DeviceState::Removed)) => {
            info!("Zenbook Duo Keyboard detected!");
            thread::sleep(Duration::from_millis(500));

            manager.set_single_monitor(&scale);

            info!("Setting backlight level to {}", config.brightness);
            if let Err(e) = set_backlight_level(config.brightness as u8, config) {
                error!("Failed to set backlight level: {}", e);
            }
        }

        // ── Keyboard removed → dual‑monitor layout ───────────────────────
        (Some(DeviceState::Removed), None) |
        (Some(DeviceState::Removed), Some(DeviceState::Added)) => {
            info!("Zenbook Duo Keyboard removed!");
            thread::sleep(Duration::from_millis(500));

            manager.set_dual_monitor(&scale);
        }
        _ => {}
    }
}
