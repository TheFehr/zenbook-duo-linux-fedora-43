# Linux for the ASUS Zenbook Duo

A Rust-based utility to handle the unique features of the ASUS Zenbook Duo (2025/UX8406CA) on Linux.

## Features

| Feature | Status |
|---------|:-------:|
| **Automatic Display Toggling** (Single/Dual monitor mode) | ✅ |
| **Keyboard Backlight Control** (USB HID) | ✅ |
| **Brightness Cycling** (via F4 key) | ✅ |
| **Touchscreen Calibration** (Lower screen mapping) | ✅ |
| **GNOME & KDE Support** (`gdctl` / `kscreen-doctor`) | ✅ |

## Tested on

The following models and operating systems have been validated by users:

- **Models**
    - 2025 Zenbook Duo (UX8406CA)
- **Distros**
    - Fedora 43 (Workstation)
- **Desktop Environments**
    - GNOME 48+
    - KDE Plasma 6

## Install

Download the latest release from the [releases page](https://github.com/TheFehr/zenbook-duo-linux-fedora-43/releases).
Then run the installer:

```bash
chmod +x zenbook-duo
./zenbook-duo install
```

**Notes:**

1. **User Session:** After installation, log out and log back in (or reboot) for the systemd user service and udev rules to take effect.
2. **Keyboard Connection:** Ensure the keyboard is connected during the first boot after installation to initialize the state.
3. **Privileges:** The installer will ask for `sudo` to copy the binary to `/usr/local/bin` and install udev rules.

## Usage

The `zenbook-duo` utility provides several subcommands. You can use the global `--verbose` (or `-v`) flag with any command to see detailed logs.

### Subcommands

**`install` / `uninstall`**
Setup or remove the background daemon, systemd service, and udev rules.
```bash
# Install the tool and configure the system
zenbook-duo install

# Uninstall and clean up all configuration
zenbook-duo uninstall

# Run install with verbose logging for troubleshooting
zenbook-duo --verbose install
```

**`backlight`**
Manually control the keyboard backlight level (0 to 3).
```bash
# Set backlight to level 2 (0-3)
zenbook-duo backlight 2
```
