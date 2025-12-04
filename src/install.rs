use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path};
use std::process::Command;
use directories::BaseDirs;
use crate::config::{self, Config, DeviceConfig};

pub fn install() {
    // 0. Pre-flight checks: Distro and DE
    check_requirements();

    // Check if we are root. If so, warn the user.
    // We prefer running as a normal user to set up config correctly.
    if env::var("USER").unwrap_or_default() == "root" || env::var("SUDO_USER").is_ok() {
        println!("Warning: It is recommended to run this installer as your normal user (without sudo).");
        println!("We will ask for sudo password only when necessary (copying the binary).");
        println!("If you continue as root, the configuration file will be created for root, which might not be what you want.");
        print!("Continue anyway? [y/N] ");
        io::stdout().flush().unwrap();
        let mut answer = String::new();
        io::stdin().read_line(&mut answer).unwrap();
        if !answer.trim().eq_ignore_ascii_case("y") {
            std::process::exit(1);
        }
    }

    println!("Installing Zenbook Duo Linux Tools...");

    // 1. Get user input for scale
    let mut scale = String::new();
    print!("What would you like to use for monitor scale (1 = 100%, 1.5 = 150%, 2=200%) [Default: 1.5]? ");
    io::stdout().flush().unwrap();
    io::stdin().read_line(&mut scale).unwrap();
    let scale_val = scale.trim();
    let scale_f64: f64 = if scale_val.is_empty() {
        1.5
    } else {
        scale_val.parse().unwrap_or(1.5)
    };

    // 2. Create Config
    // We are running as the user, so get_config_path works perfectly.
    let config_path = config::get_config_path().expect("Could not determine config path");
    let config_dir = config_path.parent().expect("Config path has no parent");

    if !config_dir.exists() {
        fs::create_dir_all(config_dir).expect("Failed to create config dir");
    }

    let config_struct = Config {
        scale: scale_f64,
        device: DeviceConfig {
            vendor_id: "0b05".to_string(),
            product_id: "1bf2".to_string(),
        },
    };

    let config_content = toml::to_string(&config_struct).expect("Failed to serialize config");

    match fs::write(&config_path, config_content) {
        Ok(_) => println!("Configuration created at {:?}", config_path),
        Err(e) => {
            eprintln!("Failed to write config file to {:?}: {}", config_path, e);
            return;
        }
    }

    // 3. Install Binary
    let current_exe = env::current_exe().expect("Failed to get current executable path");
    let install_path = Path::new("/usr/local/bin/zenbook-duo");

    println!("Installing binary to {:?}...", install_path);

    // Directly copying over a running binary can fail with "Text file busy" on some systems.
    // To avoid this, copy to a temporary path and then atomically replace the target via mv.
    let temp_install_path = Path::new("/usr/local/bin/zenbook-duo.new");

    // Step 1: copy to temporary location with sudo
    let copy_status = Command::new("sudo")
        .arg("cp")
        .arg(&current_exe)
        .arg(&temp_install_path)
        .status();

    match copy_status {
        Ok(s) if s.success() => {},
        Ok(s) => {
            eprintln!("Failed to copy binary to temporary location (exit code {}).", s);
            eprintln!("Do you have sudo privileges?");
            return;
        }
        Err(e) => {
            eprintln!("Failed to execute sudo cp: {}", e);
            return;
        }
    }

    // Ensure temp file is executable (cp usually preserves, but be safe)
    let _ = Command::new("sudo").arg("chmod").arg("+x").arg(&temp_install_path).status();

    // Step 2: atomically replace the destination
    let mv_status = Command::new("sudo")
        .arg("mv")
        .arg("-f")
        .arg(&temp_install_path)
        .arg(&install_path)
        .status();

    match mv_status {
        Ok(s) if s.success() => println!("Binary installed successfully."),
        Ok(s) => {
            eprintln!("Failed to move binary into place (exit code {}).", s);
            eprintln!("Temporary file left at {:?}", temp_install_path);
            return;
        }
        Err(e) => {
            eprintln!("Failed to execute sudo mv: {}", e);
            eprintln!("Temporary file left at {:?}", temp_install_path);
            return;
        }
    }


    // 4. Create Systemd User Service
    // We install it to ~/.config/systemd/user/ so no sudo needed
    // Use BaseDirs to get the standard config location
    let base_dirs = BaseDirs::new().expect("Could not determine base directories");
    let service_dir = base_dirs.config_dir().join("systemd").join("user");

    if !service_dir.exists() {
        fs::create_dir_all(&service_dir).expect("Failed to create systemd user dir");
    }

    let service_path = service_dir.join("zenbook-duo.service");

    let service_content = format!(
        r#"[Unit]
Description=Zenbook Duo User Handler
After=graphical-session.target

[Service]
ExecStart={}
Restart=always
RestartSec=5
Environment=XDG_CURRENT_DESKTOP=GNOME

[Install]
WantedBy=default.target
"#,
        install_path.display()
    );

    match fs::write(&service_path, service_content) {
        Ok(_) => println!("Service file created at {:?}", service_path),
        Err(e) => {
            eprintln!("Failed to write service file to {:?}: {}", service_path, e);
            return;
        }
    }

    // 5. Reload Daemon & Enable
    println!("Reloading systemd user daemon...");

    // No sudo needed for user services
    let _ = Command::new("systemctl")
        .args(&["--user", "daemon-reload"])
        .status();

    let status = Command::new("systemctl")
        .args(&["--user", "enable", "--now", "zenbook-duo.service"])
        .status();

    match status {
        Ok(s) if s.success() => {
            println!("Service enabled and started.");
        },
        Ok(s) => eprintln!("Failed to enable service. Exit status: {}", s),
        Err(e) => eprintln!("Failed to execute systemctl: {}", e),
    }

    println!("Installation complete!");
}

fn check_requirements() {
    // Check Distro
    if let Ok(os_release) = fs::read_to_string("/etc/os-release") {
        if !os_release.contains("Fedora Linux 43") {
            println!("Warning: This tool is designed for Fedora Linux 43.");
            println!("If you are on Fedora 42 or older, please use the original script:");
            println!("https://github.com/jaydio/zenbook-duo-linux-fedora");

            print!("Do you want to continue anyway? [y/N] ");
            io::stdout().flush().unwrap();
            let mut answer = String::new();
            io::stdin().read_line(&mut answer).unwrap();
            if !answer.trim().eq_ignore_ascii_case("y") {
                std::process::exit(1);
            }
        }
    }

    // Check Desktop Environment
    let desktop = env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    let is_gnome = if !desktop.is_empty() {
        desktop.to_uppercase().contains("GNOME")
    } else {
        // Only check fallback if we suspect we are not in a clear session
        Path::new("/usr/bin/gnome-shell").exists()
    };

    if !is_gnome {
        println!("Error: This tool relies on GNOME specific tools (gdctl).");
        println!("It seems you are not running GNOME.");
        std::process::exit(1);
    }
}