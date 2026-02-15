mod config;
mod install;
mod monitor_handling;
mod udev_utils;
mod usb;

use crate::config::load_config;
use std::env;
use futures::{StreamExt, stream::FuturesUnordered};
use tokio::task::LocalSet;

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

    let config = load_config();
    usb::backlight::set_backlight_level(config.brightness as u8, &config).expect("Failed to set initial backlight level");

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

            println!("Monitoring started (USB events & Special keys)...");

            watchers.for_each(|_| async {}).await;
        })
        .await;
}
