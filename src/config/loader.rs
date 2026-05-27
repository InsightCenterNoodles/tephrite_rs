use std::path::{Path, PathBuf};

use bevy::log::debug;

use super::Config;

/// Try to locate the configuration file for this app.
///
/// Search order:
/// 1. `$TEPH_CONFIG_PATH` environment variable
/// 2. `~/.teph/config.toml`
/// 3. `~/.config/teph.toml`
/// 4. `/opt/teph/config.toml`
/// 5. `/etc/teph/config.toml`
pub fn find_config_file() -> Option<PathBuf> {
    // 1. Environment variable override
    if let Ok(path) = std::env::var("TEPH_CONFIG_PATH") {
        let p = PathBuf::from(path);
        if p.exists() {
            debug!("Using config env var");
            return Some(p);
        }
    }

    // 2. User home (~/.teph/config.toml)
    if let Some(home_dir) = dirs::home_dir() {
        let candidate = home_dir.join(".teph").join("config.toml");
        if candidate.exists() {
            debug!("Using user-local config");
            return Some(candidate);
        }
    }

    // 3. User home (~/.config/teph.toml)
    if let Some(home_dir) = dirs::home_dir() {
        let candidate = home_dir.join(".config").join("teph.toml");
        if candidate.exists() {
            debug!("Using user-local config");
            return Some(candidate);
        }
    }

    // 4. /opt/teph/config.toml
    let opt_path = Path::new("/opt/teph/config.toml");
    if opt_path.exists() {
        debug!("Using /opt config");
        return Some(opt_path.to_path_buf());
    }

    // 5. /etc/teph/config.toml
    let etc_path = Path::new("/etc/teph/config.toml");
    if etc_path.exists() {
        debug!("Using /etc config");
        return Some(etc_path.to_path_buf());
    }

    None
}

/// Load and parse the config file into the app config struct.
pub fn load_config() -> Result<Config, Box<dyn std::error::Error>> {
    let path = find_config_file().ok_or("Config file not found in common locations")?;
    let text = std::fs::read_to_string(&path)?;
    let config: Config = toml::from_str(&text)?;
    Ok(config)
}
