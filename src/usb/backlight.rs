use std::env;
use std::time::Duration;
use rusb::{Context, UsbContext, DeviceHandle};
use lazy_static::lazy_static;
use std::sync::Mutex;
use crate::config;
use crate::config::Config;
use log::{error};

lazy_static! {
    static ref USB_MUTEX: Mutex<()> = Mutex::new(());
}

struct InterfaceGuard<'a, T: UsbContext> {
    handle: &'a DeviceHandle<T>,
    interface: u8,
    detached: bool,
}

impl<'a, T: UsbContext> InterfaceGuard<'a, T> {
    fn new(handle: &'a DeviceHandle<T>, interface: u8) -> Result<Self, rusb::Error> {
        let detached = handle.kernel_driver_active(interface).unwrap_or(false);
        if detached {
            handle.detach_kernel_driver(interface)?;
        }

        handle.claim_interface(interface)?;

        Ok(InterfaceGuard {
            handle,
            interface,
            detached,
        })
    }
}

impl<'a, T: UsbContext> Drop for InterfaceGuard<'a, T> {
    fn drop(&mut self) {
        let _ = self.handle.release_interface(self.interface);
        if self.detached {
            let _ = self.handle.attach_kernel_driver(self.interface);
        }
    }
}

pub fn set_backlight_level(level: u8, config: &Config) -> Result<(), rusb::Error> {
    let _lock = USB_MUTEX.lock().unwrap();

    if level > 3 {
        return Err(rusb::Error::InvalidParam);
    }

    let vendor_id = u16::from_str_radix(&config.device.vendor_id, 16).map_err(|_| {
        error!("Invalid Vendor ID in config: '{}'", config.device.vendor_id);
        rusb::Error::InvalidParam
    })?;

    let product_id = u16::from_str_radix(&config.device.product_id, 16).map_err(|_| {
        error!("Invalid Product ID in config: '{}'", config.device.product_id);
        rusb::Error::InvalidParam
    })?;

    let mut retries = 3;
    while retries > 0 {
        match set_backlight_internal(level, vendor_id, product_id) {
            Ok(_) => return Ok(()),
            Err(rusb::Error::Busy) if retries > 1 => {
                retries -= 1;
                std::thread::sleep(Duration::from_millis(100));
                continue;
            }
            Err(e) => return Err(e),
        }
    }
    Err(rusb::Error::Busy)
}

fn set_backlight_internal(level: u8, vendor_id: u16, product_id: u16) -> Result<(), rusb::Error> {
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

    {
        let _guard = InterfaceGuard::new(&handle, w_index as u8)?;

        handle.write_control(
            0x21, // bmRequestType
            0x09, // bRequest (SET_REPORT)
            w_value,
            w_index,
            &data,
            Duration::from_secs(1),
        )?;
    }

    Ok(())
}

pub fn run_backlight_command(level_arg: Option<u8>) {
    // 1. Handle Elevation
    if env::var("USER").unwrap_or_default() != "root" {
        println!("Backlight control requires root privileges. Re-running with sudo...");
        let current_exe = env::current_exe().expect("Failed to get current executable path");
        
        let mut cmd = std::process::Command::new("sudo");
        cmd.arg(current_exe).arg("backlight");
        if let Some(l) = level_arg {
            cmd.arg(l.to_string());
        }
        
        let status = cmd.status();

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
    let level = if let Some(l) = level_arg {
        if l <= 3 {
            l
        } else {
            eprintln!("Invalid level '{}'. Please provide an integer between 0 and 3.", l);
            std::process::exit(1);
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