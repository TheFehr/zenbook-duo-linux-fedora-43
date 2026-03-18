use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path};
use std::process::Command;
use directories::BaseDirs;
use crate::config;

/// Install the Zenbook Duo CLI, create a per-user systemd service for it, and enable/start that service.
///
/// This performs interactive preflight checks (distribution and desktop environment), optionally prompts
/// the user if running as root, loads or creates configuration interactively, installs the current
/// executable to /usr/local/bin/zenbook-duo (using a temporary file and sudo for copying/replacing),
/// writes a systemd user unit under ~/.config/systemd/user/zenbook-duo.service configured with the
/// detected desktop environment, reloads the user systemd daemon, and enables/starts the service.
///
/// # Examples
///
/// ```no_run
/// // Run this as the intended user (not root) to install and enable the service.
/// install();
/// ```
pub fn install() {
    // 0. Pre-flight checks: Distro and DE
    let desktop_env = check_requirements();

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

    // 1 & 2. Handle Config
    config::load_config_interactive();

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

    crate::udev_utils::ensure_touch_rule();

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
Environment=XDG_CURRENT_DESKTOP={}

[Install]
WantedBy=default.target
"#,
        install_path.display(), desktop_env
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

/// Detects the desktop environment and performs a Fedora release compatibility check.
///
/// Reads /etc/os-release and warns (with a prompt) if the host is not Fedora Linux 43.
/// Determines the desktop environment from the `XDG_CURRENT_DESKTOP` environment variable
/// (case-insensitive). If that variable is not decisive, falls back to checking for the
/// presence of `/usr/bin/gdctl` or `/usr/bin/kscreen-doctor`. Exits the process
/// with status 1 when the user declines to continue after the distro warning or when no
/// supported desktop environment can be detected.
///
/// # Returns
///
/// `"GNOME"` if a GNOME desktop environment is detected, `"KDE"` if a KDE Plasma environment is detected.
///
/// # Examples
///
/// ```
/// // The return value will be either "GNOME" or "KDE" depending on the environment.
/// let env = check_requirements();
/// assert!(env == "GNOME" || env == "KDE");
/// ```
fn check_requirements() -> &'static str {
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
    let desktop = env::var("XDG_CURRENT_DESKTOP").unwrap_or_default().to_uppercase();
    
    if desktop.contains("GNOME") {
        return "GNOME";
    } else if desktop.contains("KDE") {
        return "KDE";
    }

    // Fallbacks
    if Path::new("/usr/bin/gdctl").exists() {
        return "GNOME";
    } else if Path::new("/usr/bin/kscreen-doctor").exists() {
        return "KDE";
    } else {
        println!("Error: This tool relies on GNOME (gdctl) or KDE Plasma (kscreen-doctor).");
        println!("It seems you are running an unsupported environment.");
        std::process::exit(1);
    }
}

pub fn uninstall() {
    println!("Uninstalling Zenbook Duo Linux Tools...");

    // 1. Delete the udev rule
    crate::udev_utils::remove_touch_rule();

    // 2. Stop and disable the systemd user service
    println!("Stopping and disabling systemd user service...");
    let _ = Command::new("systemctl")
        .args(&["--user", "stop", "zenbook-duo.service"])
        .status();
    let _ = Command::new("systemctl")
        .args(&["--user", "disable", "zenbook-duo.service"])
        .status();

    let base_dirs = BaseDirs::new().expect("Could not determine base directories");
    let service_path = base_dirs.config_dir().join("systemd").join("user").join("zenbook-duo.service");
    
    if service_path.exists() {
        if let Err(e) = fs::remove_file(&service_path) {
            eprintln!("Failed to remove service file at {:?}: {}", service_path, e);
        } else {
            println!("Removed systemd service file.");
        }
    }

    let _ = Command::new("systemctl")
        .args(&["--user", "daemon-reload"])
        .status();

    // 3. Delete configuration files/directories
    if let Some(config_path) = crate::config::get_config_path() {
        if let Some(config_dir) = config_path.parent() {
            if config_dir.exists() {
                if let Err(e) = fs::remove_dir_all(config_dir) {
                    eprintln!("Failed to remove configuration directory {:?}: {}", config_dir, e);
                } else {
                    println!("Removed configuration directory {:?}", config_dir);
                }
            }
        }
    }

    // 4. Remove the binary from the installation path
    let install_path = Path::new("/usr/local/bin/zenbook-duo");
    if install_path.exists() {
        println!("Removing binary from {:?}...", install_path);
        let rm_status = Command::new("sudo")
            .arg("rm")
            .arg("-f")
            .arg(install_path)
            .status();

        match rm_status {
            Ok(s) if s.success() => println!("Binary removed successfully."),
            Ok(s) => eprintln!("Failed to remove binary (exit code {}).", s),
            Err(e) => eprintln!("Failed to execute sudo rm: {}", e),
        }
    }

    println!("Uninstallation complete!");
}