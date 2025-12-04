use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use directories::ProjectDirs;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
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
            scale: 1.5,
            device: DeviceConfig {
                vendor_id: "b05".to_string(),
                product_id: "1bf2".to_string(),
            },
        }
    }
}

pub fn load_config() -> Config {
    let config_path = get_config_path();

    if let Some(path) = config_path {
        if path.exists() {
            match fs::read_to_string(&path) {
                Ok(contents) => {
                    match toml::from_str(&contents) {
                        Ok(config) => {
                            println!("Loaded config from {:?}", path);
                            return config;
                        },
                        Err(e) => eprintln!("Failed to parse config file: {}", e),
                    }
                },
                Err(e) => eprintln!("Failed to read config file: {}", e),
            }
        }
    }

    println!("Using default configuration");
    Config::default()
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
        assert_eq!(cfg.scale, 1.5);
        assert_eq!(cfg.device.vendor_id, "b05");
        assert_eq!(cfg.device.product_id, "1bf2");
    }

    #[test]
    fn get_config_path_for_custom_home() {
        let fake_home = std::path::PathBuf::from("/home/testuser");
        let path = get_config_path_for_home(Some(fake_home)).unwrap();
        // Should be $HOME/.config/zenbook-duo/config.toml
        assert!(path.ends_with(".config/zenbook-duo/config.toml"));
    }

    #[test]
    fn toml_round_trip_works() {
        let cfg = Config::default();
        let s = toml::to_string(&cfg).expect("serialize");
        let de: Config = toml::from_str(&s).expect("deserialize");
        assert_eq!(de.scale, cfg.scale);
        assert_eq!(de.device.vendor_id, cfg.device.vendor_id);
        assert_eq!(de.device.product_id, cfg.device.product_id);
    }
}
