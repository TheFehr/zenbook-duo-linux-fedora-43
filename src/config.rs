use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use directories::ProjectDirs;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    pub brightness: i8,
    pub scale: f64,
    pub device: DeviceConfig,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeviceConfig {
    pub vendor_id: String,
    pub product_id: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            brightness: 1,
            scale: 1.5,
            device: DeviceConfig {
                vendor_id: "b05".to_string(),
                product_id: "1bf2".to_string(),
            },
        }
    }
}

pub fn save_config(config: &Config) {
    if let Some(path) = get_config_path() {
        let config_dir = path.parent().expect("Config path has no parent");
        if !config_dir.exists() {
            fs::create_dir_all(config_dir).expect("Failed to create config dir");
        }

        let content = toml::to_string(config).expect("Failed to serialize config");
        if let Err(e) = fs::write(&path, content) {
            eprintln!("Failed to write config file to {:?}: {}", path, e);
        }
    }
}

pub fn load_config() -> Config {
    load_config_internal(false)
}

pub fn load_config_interactive() -> Config {
    load_config_internal(true)
}

fn load_config_internal(interactive: bool) -> Config {
    let config_path = get_config_path();

    if let Some(path) = config_path {
        if path.exists() {
            if let Ok(contents) = fs::read_to_string(&path) {
                // Parse as generic value to check for missing fields
                let table: toml::Value = contents.parse().unwrap_or(toml::Value::Table(Default::default()));

                let mut config = Config::default();
                let mut needs_save = false;

                // Scale
                if let Some(s) = table.get("scale").and_then(|v| v.as_float()) {
                    config.scale = s;
                } else if interactive {
                    config.scale = prompt_scale();
                    needs_save = true;
                }

                // Brightness
                if let Some(b) = table.get("brightness").and_then(|v| v.as_integer()) {
                    config.brightness = b as i8;
                } else if interactive {
                    config.brightness = prompt_brightness();
                    needs_save = true;
                }

                // Device
                if let Some(device) = table.get("device") {
                    if let Ok(d) = device.clone().try_into() {
                        config.device = d;
                    }
                }

                if needs_save && interactive {
                    save_config(&config);
                }

                return config;
            }
        }
    }

    let default_cfg = Config::default();
    if interactive {
        save_config(&default_cfg);
    }
    default_cfg
}

fn prompt_scale() -> f64 {
    let mut input = String::new();
    print!("Missing config: What monitor scale (1.5 = 150%) [Default: 1.5]? ");
    io::stdout().flush().unwrap();
    let _ = io::stdin().read_line(&mut input);
    let val = input.trim();
    if val.is_empty() { 1.5 } else { val.parse().unwrap_or(1.5) }
}

fn prompt_brightness() -> i8 {
    let mut input = String::new();
    print!("Missing config: What keyboard backlight level (0-3) [Default: 2]? ");
    io::stdout().flush().unwrap();
    let _ = io::stdin().read_line(&mut input);
    let val = input.trim();
    if val.is_empty() { 2 } else { val.parse().unwrap_or(2).min(3) }
}

pub fn get_config_path() -> Option<PathBuf> {
    get_config_path_for_home(None)
}

pub fn get_config_path_for_home(home_override: Option<PathBuf>) -> Option<PathBuf> {
    if let Some(home) = home_override {
        let mut path = home;
        path.push(".config");
        path.push("zenbook-duo");
        path.push("config.toml");
        return Some(path);
    }

    if let Some(proj_dirs) = ProjectDirs::from("", "", "zenbook-duo") {
        let mut path = proj_dirs.config_dir().to_path_buf();
        path.push("config.toml");
        return Some(path);
    }
    // Fallback to manual path if ProjectDirs fails for some reason
    if let Ok(home) = std::env::var("HOME") {
        let mut path = PathBuf::from(home);
        path.push(".config");
        path.push("zenbook-duo");
        path.push("config.toml");
        return Some(path);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_values_are_as_expected() {
        let cfg = Config::default();
        assert_eq!(cfg.brightness, 1);
        assert_eq!(cfg.scale, 1.5);
        assert_eq!(cfg.device.vendor_id, "b05");
        assert_eq!(cfg.device.product_id, "1bf2");
    }

    #[test]
    fn get_config_path_for_custom_home() {
        let fake_home = PathBuf::from("/home/testuser");
        let path = get_config_path_for_home(Some(fake_home)).unwrap();
        // Should be $HOME/.config/zenbook-duo/config.toml
        assert!(path.ends_with(".config/zenbook-duo/config.toml"));
    }

    #[test]
    fn toml_round_trip_works() {
        let cfg = Config::default();
        let s = toml::to_string(&cfg).expect("serialize");
        let de: Config = toml::from_str(&s).expect("deserialize");
        assert_eq!(de.brightness, cfg.brightness);
        assert_eq!(de.scale, cfg.scale);
        assert_eq!(de.device.vendor_id, cfg.device.vendor_id);
        assert_eq!(de.device.product_id, cfg.device.product_id);
    }
}
