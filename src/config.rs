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
