use std::thread;
use std::process::Command;
use std::time::Duration;
use std::collections::HashMap;
use crate::config::Config;
use crate::usb::backlight::set_backlight_level;
use crate::usb::DeviceState;
use log::{info, debug, error};

trait DisplayManager {
    fn set_single_monitor(&self, scale: &str);
    fn set_dual_monitor(&self, scale: &str);
    fn as_any(&self) -> &dyn std::any::Any;
}

struct GnomeManager {
    env: HashMap<String, String>,
}

impl DisplayManager for GnomeManager {
    fn as_any(&self) -> &dyn std::any::Any { self }
    fn set_single_monitor(&self, scale: &str) {
        let base_args = vec![
            "set", "--logical-monitor", "--primary", "--scale", scale, "--monitor", "eDP-1"
        ];
        info!("Switching to single monitor mode (scale: {})", scale);
        debug!("Executing 'gdctl {}'", base_args.join(" "));
        
        match Command::new("gdctl")
            .envs(&self.env)
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
            .envs(&self.env)
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

struct KdeManager {
    env: HashMap<String, String>,
}

impl DisplayManager for KdeManager {
    fn as_any(&self) -> &dyn std::any::Any { self }
    fn set_single_monitor(&self, scale: &str) {
        let args = vec![
            format!("output.eDP-1.position.0,0"),
            format!("output.eDP-1.scale.{}", scale),
            "output.eDP-2.disable".to_string(),
        ];
        info!("Switching to single monitor mode (scale: {})", scale);
        debug!("Executing 'kscreen-doctor {}'", args.join(" "));
        
        match Command::new("kscreen-doctor")
            .envs(&self.env)
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
        let physical_height = get_edp1_physical_height(&self.env);
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
            .envs(&self.env)
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
fn get_edp1_physical_height(env: &HashMap<String, String>) -> i32 {
    match Command::new("kscreen-doctor").envs(env).arg("-o").output() {
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
    fn as_any(&self) -> &dyn std::any::Any { self }
    fn set_single_monitor(&self, _scale: &str) {
        debug!("No supported desktop environment detected. Skipping display configuration.");
    }
    fn set_dual_monitor(&self, _scale: &str) {
        debug!("No supported desktop environment detected. Skipping display configuration.");
    }
}

/// Selects a display manager implementation appropriate for the current desktop environment.
///
/// This function attempts to detect the desktop environment (GNOME or KDE) by:
/// 1. Checking `XDG_CURRENT_DESKTOP`.
/// 2. If empty, attempting to refresh environment variables from the systemd user session.
/// 3. Falling back to checking for existence of `gdctl` or `kscreen-doctor` if a display is active.
///
/// # Returns
///
/// A `Box<dyn DisplayManager>` containing:
/// - `KdeManager` if KDE is detected.
/// - `GnomeManager` if GNOME is detected.
/// - `NullManager` otherwise, to prevent errors on unsupported environments.
fn get_display_manager() -> Box<dyn DisplayManager> {
    let mut env: HashMap<String, String> = std::env::vars().collect();
    
    let mut desktop = env.get("XDG_CURRENT_DESKTOP").map(|s| s.to_uppercase()).unwrap_or_default();
    let mut wayland_display = env.get("WAYLAND_DISPLAY").cloned().unwrap_or_default();
    let mut display = env.get("DISPLAY").cloned().unwrap_or_default();

    debug!("DE Detection (initial): XDG_CURRENT_DESKTOP='{}', WAYLAND_DISPLAY='{}', DISPLAY='{}'", 
           desktop, wayland_display, display);

    // If environment variables are missing (common when systemd starts the service too early),
    // try to fetch them from the systemd user environment.
    if desktop.is_empty() && wayland_display.is_empty() && display.is_empty() {
        debug!("Environment variables missing. Attempting to refresh from systemd...");
        if let Ok(output) = Command::new("systemctl").args(&["--user", "show-environment"]).output() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                if let Some((key, value)) = line.split_once('=') {
                    env.insert(key.to_string(), value.to_string());
                }
            }
            // Re-check variables after refresh
            desktop = env.get("XDG_CURRENT_DESKTOP").map(|s| s.to_uppercase()).unwrap_or_default();
            wayland_display = env.get("WAYLAND_DISPLAY").cloned().unwrap_or_default();
            display = env.get("DISPLAY").cloned().unwrap_or_default();
            debug!("DE Detection (after refresh): XDG_CURRENT_DESKTOP='{}', WAYLAND_DISPLAY='{}', DISPLAY='{}'", 
                   desktop, wayland_display, display);
        }
    }

    if desktop.contains("KDE") {
        info!("Initializing KDE Display Manager (detected via XDG_CURRENT_DESKTOP)");
        Box::new(KdeManager { env })
    } else if desktop.contains("GNOME") {
        info!("Initializing GNOME Display Manager (detected via XDG_CURRENT_DESKTOP)");
        Box::new(GnomeManager { env })
    } else if !desktop.is_empty() {
        info!("Unsupported Desktop Environment: {}", desktop);
        Box::new(NullManager)
    } else if std::path::Path::new("/usr/bin/kscreen-doctor").exists() && (!wayland_display.is_empty() || !display.is_empty()) {
        info!("Initializing KDE Display Manager (detected via kscreen-doctor and active display)");
        Box::new(KdeManager { env })
    } else if std::path::Path::new("/usr/bin/gdctl").exists() && (!wayland_display.is_empty() || !display.is_empty()) {
        info!("Initializing GNOME Display Manager (detected via gdctl and active display)");
        Box::new(GnomeManager { env })
    } else {
        info!("No supported Desktop Environment detected or no active display found.");
        Box::new(NullManager)
    }
}

/// Adjusts monitor layout and backlight when the Zenbook Duo keyboard state changes.
/// Returns true if a supported manager was used, false if NullManager was used.
pub fn handle_if_changed(current: &Option<DeviceState>, before: &Option<DeviceState>, config: &Config) -> bool {
    let scale = config.scale.to_string();
    let manager = get_display_manager();
    
    // Check if we are using a real manager
    let is_null = std::any::TypeId::of::<NullManager>() == manager.as_any().type_id();

    match (current, before) {
        // ── Keyboard added → single‑monitor layout ─────────────────────
        (Some(DeviceState::Added), None) |
        (Some(DeviceState::Added), Some(DeviceState::Removed)) => {
            info!("Zenbook Duo Keyboard detected!");
            if is_null {
                info!("Desktop environment not ready yet. Will retry.");
                return false;
            }
            thread::sleep(Duration::from_millis(1000));

            manager.set_single_monitor(&scale);

            info!("Setting backlight level to {}", config.brightness);
            if let Err(e) = set_backlight_level(config.brightness as u8, config) {
                error!("Failed to set backlight level: {}", e);
                return false;
            }
            info!("Backlight level set successfully.");
        }

        // ── Keyboard removed → dual‑monitor layout ───────────────────────
        (Some(DeviceState::Removed), None) |
        (Some(DeviceState::Removed), Some(DeviceState::Added)) => {
            info!("Zenbook Duo Keyboard removed!");
            if is_null {
                info!("Desktop environment not ready yet. Will retry.");
                return false;
            }
            thread::sleep(Duration::from_millis(1000));

            manager.set_dual_monitor(&scale);
        }
        _ => {}
    }
    !is_null
}
