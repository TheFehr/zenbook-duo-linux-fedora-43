mod config;
mod install;
mod monitor_handling;
mod udev_utils;
mod usb;

use clap::{Parser, Subcommand};
use crate::config::load_config;
use futures::{StreamExt, stream::FuturesUnordered};
use tokio::task::LocalSet;
use log::{info, LevelFilter};

#[derive(Parser)]
#[command(name = "zenbook-duo", about = "Zenbook Duo Linux Tools")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Install the daemon and configure udev rules
    Install,
    /// Uninstall the daemon, service, and rules
    Uninstall,
    /// Control the keyboard backlight
    Backlight {
        /// Set the keyboard backlight level (0-3)
        level: Option<u8>,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let mut config = load_config();

    // Check for verbose flag in args
    if cli.verbose {
        config.verbose = true;
    }

    if let Some(command) = &cli.command {
        match command {
            Commands::Install => {
                install::install();
                return;
            }
            Commands::Uninstall => {
                install::uninstall();
                return;
            }
            Commands::Backlight { level } => {
                usb::backlight::run_backlight_command(*level);
                return;
            }
        }
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
