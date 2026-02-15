mod config;
mod install;
mod monitor_handling;
mod udev_utils;
mod usb;

use crate::config::load_config;
use std::env;
use futures::{StreamExt, stream::FuturesUnordered};
use tokio::task::LocalSet;
use log::{info, error, LevelFilter};

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() > 1 && args[1] == "--install" {
        install::install();
        return;
    }

    if args.len() > 1 && args[1] == "--backlight" {
        usb::backlight::run_backlight_command(&args);
        return;
    }

    let mut config = load_config();
    
    // Check for verbose flag in args
    if args.iter().any(|arg| arg == "--verbose" || arg == "-v") {
        config.verbose = true;
    }

    // Initialize logger
    let log_level = if config.verbose {
        LevelFilter::Debug
    } else {
        LevelFilter::Info
    };

    env_logger::Builder::from_default_env()
        .filter_level(log_level)
        .init();

    let (current_state, _keyboard_devpath) = usb::check_initial_state(&config);
    if current_state == Some(usb::DeviceState::Added) {
        if let Err(e) = usb::backlight::set_backlight_level(config.brightness as u8, &config) {
            error!("Failed to set initial backlight level: {:?}. (Are you running as root or have udev rules set up?)", e);
        }
    } else {
        info!("Keyboard not detected at startup, skipping initial backlight setup.");
    }

    // LocalSet allows us to spawn !Send futures (like the udev monitor) on the current thread
    let local = LocalSet::new();

    local
        .run_until(async move {
            let watchers = FuturesUnordered::new();

            // 1. USB Connection Watcher (Display handling)
            watchers.push(tokio::task::spawn_local(async move {
                usb::monitor_usb_events().await;
            }));

            // 2. Keyboard Key Watcher (Backlight F4 handling)
            let config_keys = config.clone();
            watchers.push(tokio::task::spawn_local(async move {
                usb::monitor_special_keys(config_keys).await;
            }));

            info!("Monitoring started (USB events & Special keys)...");

            watchers.for_each(|_| async {}).await;
        })
        .await;
}
