use std::thread;
use std::process::Command;
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
        info!("Switching to single monitor mode (scale: {})", scale);
        debug!("Executing 'gdctl {}'", base_args.join(" "));
        
        match Command::new("gdctl")
            .args(&base_args)
            .output()
        {
            Ok(output) => {
                if !output.status.success() {
                    error!("gdctl failed with status {}: {}", output.status, String::from_utf8_lossy(&output.stderr));
                } else {
                    debug!("gdctl succeeded");
                }
            }
            Err(e) => {
                error!("Failed to execute gdctl: {}", e);
            }
        }
    }

    fn set_dual_monitor(&self, scale: &str) {
        let dual_args = vec![
            "set", "--logical-monitor", "--primary", "--scale", scale, "--monitor", "eDP-1",
            "--logical-monitor", "--scale", scale, "--monitor", "eDP-2", "--below", "eDP-1"
        ];
        info!("Switching to dual monitor mode (scale: {})", scale);
        debug!("Executing 'gdctl {}'", dual_args.join(" "));
        
        match Command::new("gdctl")
            .args(&dual_args)
            .output()
        {
            Ok(output) => {
                if !output.status.success() {
                    error!("gdctl failed with status {}: {}", output.status, String::from_utf8_lossy(&output.stderr));
                } else {
                    debug!("gdctl succeeded");
                }
            }
            Err(e) => {
                error!("Failed to execute gdctl: {}", e);
            }
        }
    }
}

struct KdeManager;

impl DisplayManager for KdeManager {
    fn set_single_monitor(&self, scale: &str) {
        let args = vec![
            format!("output.eDP-1.position.0,0"),
            format!("output.eDP-1.scale.{}", scale),
            "output.eDP-2.disable".to_string(),
        ];
        info!("Switching to single monitor mode (scale: {})", scale);
        debug!("Executing 'kscreen-doctor {}'", args.join(" "));
        
        match Command::new("kscreen-doctor")
            .args(&args)
            .output()
        {
            Ok(output) => {
                if !output.status.success() {
                    error!("kscreen-doctor failed with status {}: {}", output.status, String::from_utf8_lossy(&output.stderr));
                } else {
                    debug!("kscreen-doctor succeeded");
                }
            }
            Err(e) => {
                error!("Failed to execute kscreen-doctor: {}", e);
            }
        }
    }

    fn set_dual_monitor(&self, scale: &str) {
        let physical_height = get_edp1_physical_height();
        let scale_f: f32 = scale.parse().unwrap_or(1.0);
        let logical_height = (physical_height as f32 / scale_f).round() as i32;

        let args = vec![
            format!("output.eDP-1.position.0,0"),
            format!("output.eDP-1.scale.{}", scale),
            "output.eDP-2.enable".to_string(),
            format!("output.eDP-2.scale.{}", scale),
            format!("output.eDP-2.position.0,{}", logical_height),
        ];
        info!("Switching to dual monitor mode (scale: {}, eDP-1 height: {})", scale, logical_height);
        debug!("Executing 'kscreen-doctor {}'", args.join(" "));
        
        match Command::new("kscreen-doctor")
            .args(&args)
            .output()
        {
            Ok(output) => {
                if !output.status.success() {
                    error!("kscreen-doctor failed with status {}: {}", output.status, String::from_utf8_lossy(&output.stderr));
                } else {
                    debug!("kscreen-doctor succeeded");
                }
            }
            Err(e) => {
                error!("Failed to execute kscreen-doctor: {}", e);
            }
        }
    }
}

/// Helper function to retrieve the physical height of eDP-1 via `kscreen-doctor -o`.
/// This is used to calculate the logical position of eDP-2.
fn get_edp1_physical_height() -> i32 {
    match Command::new("kscreen-doctor").arg("-o").output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for section in stdout.split("Output: ") {
                if section.contains("eDP-1") {
                    let mut current_height = 0;
                    let mut current_scale = 1.0;
                    for line in section.lines() {
                        if line.contains("Geometry:") {
                            let parts: Vec<&str> = line.split_whitespace().collect();
                            if parts.len() >= 3 {
                                if let Some(h_str) = parts[2].split('x').nth(1) {
                                    current_height = h_str.parse().unwrap_or(0);
                                }
                            }
                        }
                        if line.contains("Scale:") {
                            let parts: Vec<&str> = line.split_whitespace().collect();
                            if parts.len() >= 2 {
                                current_scale = parts[1].parse().unwrap_or(1.0);
                            }
                        }
                    }
                    if current_height > 0 {
                        return (current_height as f32 * current_scale).round() as i32;
                    }
                }
            }
            1800 // Fallback for Zenbook Duo
        }
        Err(_) => 1800,
    }
}

struct NullManager;

impl DisplayManager for NullManager {
    fn set_single_monitor(&self, _scale: &str) {
        debug!("No supported desktop environment detected. Skipping display configuration.");
    }
    fn set_dual_monitor(&self, _scale: &str) {
        debug!("No supported desktop environment detected. Skipping display configuration.");
    }
}

/// Selects a display manager implementation appropriate for the current desktop environment.
///
/// # Examples
///
/// ```
/// let _mgr = get_display_manager();
/// ```
///
/// # Returns
///
/// A `Box<dyn DisplayManager>` containing:
/// - `KdeManager` if `XDG_CURRENT_DESKTOP` contains "KDE".
/// - `GnomeManager` if `XDG_CURRENT_DESKTOP` contains "GNOME".
/// - `NullManager` otherwise, to prevent errors on unsupported environments.
fn get_display_manager() -> Box<dyn DisplayManager> {
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default().to_uppercase();
    if desktop.contains("KDE") {
        info!("Initializing KDE Display Manager (detected via env)");
        Box::new(KdeManager)
    } else if desktop.contains("GNOME") {
        info!("Initializing GNOME Display Manager (detected via env)");
        Box::new(GnomeManager)
    } else if std::path::Path::new("/usr/bin/kscreen-doctor").exists() {
        info!("Initializing KDE Display Manager (detected via kscreen-doctor)");
        Box::new(KdeManager)
    } else if std::path::Path::new("/usr/bin/gdctl").exists() {
        info!("Initializing GNOME Display Manager (detected via gdctl)");
        Box::new(GnomeManager)
    } else {
        info!("No supported Desktop Environment detected (XDG_CURRENT_DESKTOP='{}')", desktop);
        Box::new(NullManager)
    }
}

/// Adjusts monitor layout and backlight when the Zenbook Duo keyboard state changes.
///
/// This inspects the transition from `before` to `current` and:
/// - If the keyboard was added (current = `Some(Added)` and previous is `None` or `Some(Removed)`),
///   waits 500ms, switches to the single‑monitor layout using `config.scale`, and attempts to set
///   the backlight to `config.brightness` (errors are logged).
/// - If the keyboard was removed (current = `Some(Removed)` and previous is `None` or `Some(Added)`),
///   waits 500ms and switches to the dual‑monitor layout using `config.scale`.
/// - Otherwise does nothing.
///
/// Parameters:
/// - `current`: the new device state to evaluate.
/// - `before`: the previous device state to compare against.
/// - `config`: runtime configuration containing `scale` (used for monitor layout) and `brightness`
///   (used when the keyboard is added).
///
/// # Examples
///
/// ```
/// // Example usage (types and constructors depend on the surrounding crate):
/// // let cfg = Config { scale: "1.0".into(), brightness: 120, ... };
/// // handle_if_changed(&Some(DeviceState::Added), &None, &cfg);
/// ```
pub fn handle_if_changed(current: &Option<DeviceState>, before: &Option<DeviceState>, config: &Config) {
    let scale = config.scale.to_string();
    let manager = get_display_manager();

    match (current, before) {
        // ── Keyboard added → single‑monitor layout ─────────────────────
        (Some(DeviceState::Added), None) |
        (Some(DeviceState::Added), Some(DeviceState::Removed)) => {
            info!("Zenbook Duo Keyboard detected!");
            thread::sleep(Duration::from_millis(1000));

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
            thread::sleep(Duration::from_millis(1000));

            manager.set_dual_monitor(&scale);
        }
        _ => {}
    }
}
