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
    /// Configure eDP-1 as the primary monitor using the provided scale.
    ///
    /// This method applies the given scale to the eDP-1 display and attempts to set it as the primary monitor.
    /// Failures to apply the configuration are logged.
    ///
    /// # Parameters
    ///
    /// - `scale`: The scale factor to apply to eDP-1 (for example, `"1.00"` or `"1.25"`).
    ///
    /// # Examples
    ///
    /// ```
    /// // Assuming `mgr` implements this method (e.g., GnomeManager or KdeManager).
    /// // let mgr = GnomeManager::new();
    /// // mgr.set_single_monitor("1.00");
    /// ```
    fn set_single_monitor(&self, scale: &str) {
        let base_args = vec![
            "set", "--logical-monitor", "--primary", "--scale", scale, "--monitor", "eDP-1"
        ];
        debug!("Executing 'gdctl {}'", base_args.join(" "));
        
        match Command::new("gdctl")
            .args(&base_args)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
        {
            Ok(status) if !status.success() => {
                error!("gdctl exited with status: {}", status);
            }
            Err(e) => {
                error!("Failed to execute gdctl: {}", e);
            }
            _ => {}
        }
    }

    /// Configure a dual-monitor layout by applying `scale` to both displays and placing `eDP-2` below `eDP-1`.
    ///
    /// This will invoke the `gdctl` utility to make `eDP-1` the primary monitor and position `eDP-2` beneath it, using the provided scale value for both monitors. Errors encountered while launching `gdctl` are logged.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// let manager = GnomeManager;
    /// manager.set_dual_monitor("1.25");
    /// ```
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
    /// Configure KDE to use a single-monitor layout by scaling `eDP-1` and disabling `eDP-2`.
    ///
    /// The function invokes `kscreen-doctor` with arguments to set the scale for `eDP-1` and
    /// disable `eDP-2`. Errors from launching the command are logged but not returned.
    ///
    /// # Examples
    ///
    /// ```
    /// // Create a KdeManager and set a scale of 1.25 for the internal display.
    /// let mgr = KdeManager { /* fields if any */ };
    /// mgr.set_single_monitor("1.25");
    /// ```
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

    /// Configure KDE to use a dual-monitor layout and apply the given scale to both displays.
    ///
    /// The `scale` string is passed directly to `kscreen-doctor` (for example `"1.0"` or `"1.25"`).
    ///
    /// # Examples
    ///
    /// ```no_run
    /// let mgr = KdeManager {};
    /// mgr.set_dual_monitor("1.0");
    /// ```
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
/// A `Box<dyn DisplayManager>` containing a `KdeManager` if the `XDG_CURRENT_DESKTOP` environment
/// variable contains "KDE" (case-insensitive); otherwise a `GnomeManager`.
fn get_display_manager() -> Box<dyn DisplayManager> {
    if std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default().to_uppercase().contains("KDE") {
        Box::new(KdeManager)
    } else {
        Box::new(GnomeManager)
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
