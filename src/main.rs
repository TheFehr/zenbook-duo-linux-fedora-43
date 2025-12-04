mod usb;
mod udev_utils;
mod monitor_handling;
mod config;
mod install;

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

    // LocalSet allows us to spawn !Send futures (like the udev monitor) on the current thread
    let local = LocalSet::new();

    local.run_until(async move {
        let watchers = FuturesUnordered::new();

        // USB Watcher (Udev)
        // We use spawn_local because udev::Monitor is not Send
        watchers.push(tokio::task::spawn_local(async move {
            usb::monitor_usb_events().await;
        }));

        println!("joining tasks :)");

        watchers.for_each(|_| async {}).await;

        println!("joined all tasks :)");
    }).await;
}