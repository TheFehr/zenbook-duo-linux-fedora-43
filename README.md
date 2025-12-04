# Linux for the ASUS Zenbook Duo

A rust script to handle the Zenbook Duo keyboard.

## Functionality Status

| Feature | Working | Not Working |
|---------|:-------:|:-----------:|
| Toggle bottom screen on when keyboard removed | ✅ | |
| Toggle bottom screen off when keyboard placed on | ✅ | |

## Tested on

The following models and operating systems have been validated by users

- **Models**
    - 2025 Zenbook Duo (UX8406CA)

- **Distros**
    - Fedora 43

## Install

Download the latest release from the [releases page](https://github.com/TheFehr/linux-zenbook-duo/releases).
Then run it with `--install`.

**Notes:**

1. After installation is complete, log out and log back into your user session for changes to take effect. Alternatively, reboot your machine, ensuring the keyboard is connected during boot.

This will configure the necessary systemd scripts to manage the functionality described above.
