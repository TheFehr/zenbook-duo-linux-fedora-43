use std::env;
use std::time::Duration;
use rusb::{Context, UsbContext};
use crate::config;
use crate::config::Config;

pub fn set_backlight_level(level: u8, config: &Config) -> Result<(), rusb::Error> {
    if level > 3 {
        return Err(rusb::Error::InvalidParam);
    }

    let vendor_id = u16::from_str_radix(&config.device.vendor_id, 16).map_err(|_| {
        eprintln!("Error: Invalid Vendor ID in config: '{}'", config.device.vendor_id);
        rusb::Error::InvalidParam
    })?;

    let product_id = u16::from_str_radix(&config.device.product_id, 16).map_err(|_| {
        eprintln!("Error: Invalid Product ID in config: '{}'", config.device.product_id);
        rusb::Error::InvalidParam
    })?;

    let context = Context::new()?;
    let handle = context.open_device_with_vid_pid(vendor_id, product_id)
        .ok_or(rusb::Error::NoDevice)?;

    let report_id: u8 = 0x5A;
    let w_value: u16 = 0x035A;
    let w_index: u16 = 4;

    let mut data = [0u8; 16];
    data[0] = report_id;
    data[1] = 0xBA;
    data[2] = 0xC5;
    data[3] = 0xC4;
    data[4] = level;

    let has_kernel_driver = handle.kernel_driver_active(w_index as u8).unwrap_or(false);
    if has_kernel_driver {
        handle.detach_kernel_driver(w_index as u8)?;
    }

    handle.claim_interface(w_index as u8)?;

    handle.write_control(
        0x21, // bmRequestType
        0x09, // bRequest (SET_REPORT)
        w_value,
        w_index,
        &data,
        Duration::from_secs(1),
    )?;

    handle.release_interface(w_index as u8)?;

    if has_kernel_driver {
        let _ = handle.attach_kernel_driver(w_index as u8);
    }

    Ok(())
}

pub fn run_backlight_command(args: &[String]) {
    if args.len() < 2 {
        eprintln!("Usage: zenbook-duo --backlight <0-3>");
        eprintln!("Set the keyboard backlight level to the specified value or default from config.");
        std::process::exit(1);
    }

    // 1. Handle Elevation
    if env::var("USER").unwrap_or_default() != "root" {
        println!("Backlight control requires root privileges. Re-running with sudo...");
        let current_exe = env::current_exe().expect("Failed to get current executable path");
        let status = std::process::Command::new("sudo")
            .arg(current_exe)
            .args(&args[1..])
            .status();

        match status {
            Ok(s) if s.success() => return,
            _ => {
                std::process::exit(1);
            }
        }
    }

    // 2. Parse and Execute
    let config = config::load_config_interactive();

    // Check if a level was provided, otherwise use the one from config
    let level = if let Some(level_str) = args.get(2) {
        match level_str.parse::<u8>() {
            Ok(l) if l <= 3 => l,
            _ => {
                eprintln!("Invalid level '{}'. Please provide an integer between 0 and 3.", level_str);
                std::process::exit(1);
            }
        }
    } else {
        println!("No level provided, using default from config: {}", config.brightness);
        config.brightness as u8
    };

    if let Err(e) = set_backlight_level(level, &config) {
        print_backlight_error(e, &config);
        std::process::exit(1);
    }
    println!("Backlight successfully set to level {}", level);
}

fn print_backlight_error(err: rusb::Error, config: &Config) {
    match err {
        rusb::Error::NoDevice => {
            eprintln!("Error: Keyboard not found (VID: {}, PID: {}).",
                      config.device.vendor_id, config.device.product_id);
        },
        rusb::Error::Access => {
            eprintln!("Error: Permission denied. Even with sudo, the device might be busy.");
        },
        _ => eprintln!("USB Error: {:?}", err),
    }
}