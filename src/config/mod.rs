use crate::multiprocess::child_process_id;
use bevy::{
    log::warn,
    math::{DVec3, UVec2, uvec2},
    reflect::Reflect,
};
use std::{str::FromStr, sync::OnceLock};

mod file {
    use std::path::{Path, PathBuf};

    use bevy::log::debug;
    use serde::Deserialize;

    #[derive(Debug, Default, Deserialize)]
    pub(crate) struct Config {
        pub(crate) vrpn: Vrpn,
        pub(crate) displays: Vec<Display>,
        pub(crate) screens: Vec<Screen>,
    }

    #[derive(Debug, Default, Deserialize)]
    pub(crate) struct Vrpn {
        pub(crate) head: String,
    }

    #[derive(Debug, Default, Deserialize)]
    pub(crate) struct Display {
        // 3D points
        pub(crate) lower_left: [f64; 3],
        pub(crate) lower_right: [f64; 3],
        pub(crate) upper_right: [f64; 3],
        /// width x height
        pub(crate) resolution: [u32; 2],
    }

    #[derive(Debug, Default, Deserialize)]
    pub(crate) struct Screen {
        // index into `displays`
        pub(crate) display: u32,
        pub(crate) card_index: Option<u32>,
        pub(crate) x_display: Option<String>,

        #[serde(default)]
        pub(crate) fullscreen: bool,

        #[serde(default)]
        pub(crate) is_right: bool,
    }

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

        // None found
        None
    }

    /// Load and parse the config file into your Config struct
    pub fn load_config() -> Result<Config, Box<dyn std::error::Error>> {
        let path = find_config_file().ok_or("Config file not found in common locations")?;
        let text = std::fs::read_to_string(&path)?;
        let config: Config = toml::from_str(&text)?;
        Ok(config)
    }
}

/// Physical location of the display, as measured in room coordinates
#[derive(Debug, Default, Reflect, Clone)]
pub struct DisplayPhysical {
    pub lower_left: DVec3,
    pub lower_right: DVec3,
    pub upper_right: DVec3,
}

impl DisplayPhysical {
    fn make_plain() -> Self {
        Self {
            lower_left: [-1.0, 0.0, 0.0].into(),
            lower_right: [1.0, 0.0, 0.0].into(),
            upper_right: [1.0, 1.0, 0.0].into(),
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct VRPNAddress {
    pub sender: String,
    pub host: String,
    pub port: u16,
}

#[derive(Debug, thiserror::Error)]
pub enum VRPNAddressParseError {
    #[error("Missing address part {0}")]
    MissingPart(String),
    #[error("Bad port {0}")]
    BadPort(#[from] std::num::ParseIntError),
}

impl FromStr for VRPNAddress {
    type Err = VRPNAddressParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Should be in the form of sender@host:port
        let mut iter = s.split(&['@', ':']);
        let sender = iter
            .next()
            .ok_or_else(|| VRPNAddressParseError::MissingPart("sender".into()))?;
        let host = iter
            .next()
            .ok_or_else(|| VRPNAddressParseError::MissingPart("host".into()))?;
        let port = iter
            .next()
            .ok_or_else(|| VRPNAddressParseError::MissingPart("port".into()))?;

        let port: u16 = port.parse()?;

        Ok(Self {
            sender: sender.into(),
            host: host.into(),
            port,
        })
    }
}

/// Configure VRPN connectivity
#[derive(Debug, Default, Clone)]
pub struct VRPNConfig {
    pub head: VRPNAddress,
}

#[derive(Debug, Default, Clone)]
pub struct RenderConfiguration {
    /// The rank of the process
    pub process_rank: u32,

    /// The display to use
    pub display_name: Option<String>,

    /// The graphics card to use
    pub card_index: Option<u32>,

    /// The physical disposition of the display
    pub display_physical: DisplayPhysical,

    /// The pixel resolution of the display (w, h)
    pub resolution: UVec2,

    pub fullscreen: bool,

    pub is_right: bool,
}

#[derive(Debug, Default, Clone)]
pub struct LogicConfiguration {
    /// VRPN configuration information
    pub vrpn_config: VRPNConfig,

    /// Child information
    pub child_count: u32,
}

static HOST_CONFIG: OnceLock<LogicConfiguration> = OnceLock::new();
static CHILD_CONFIG: OnceLock<RenderConfiguration> = OnceLock::new();

fn build_child_config() -> RenderConfiguration {
    let file::Config {
        vrpn: _,
        displays,
        screens,
    } = file::load_config()
        .inspect_err(|x| warn!("Unable to load configuration file: {x}"))
        .unwrap_or_default();

    let this_screen = screens.into_iter().nth(child_process_id() as usize);
    let this_display = this_screen
        .as_ref()
        .and_then(|x| displays.into_iter().nth(x.display as usize));

    let resolution: UVec2 = this_display
        .as_ref()
        .map(|x| x.resolution.into())
        .unwrap_or_else(|| uvec2(1920, 1200));

    RenderConfiguration {
        process_rank: child_process_id(),
        card_index: this_screen.as_ref().and_then(|x| x.card_index),
        fullscreen: this_screen
            .as_ref()
            .map(|x| x.fullscreen)
            .unwrap_or_default(),
        is_right: this_screen.as_ref().map(|x| x.is_right).unwrap_or_default(),
        display_name: this_screen.and_then(|x| x.x_display),
        display_physical: this_display
            .map(|x| DisplayPhysical {
                lower_left: x.lower_left.into(),
                lower_right: x.lower_right.into(),
                upper_right: x.upper_right.into(),
            })
            .unwrap_or_else(DisplayPhysical::make_plain),
        resolution,
    }
}

pub fn get_render_configuration() -> &'static RenderConfiguration {
    CHILD_CONFIG.get_or_init(build_child_config)
}

pub fn get_logic_configuration() -> &'static LogicConfiguration {
    fn build() -> Option<LogicConfiguration> {
        let file::Config {
            vrpn,
            displays: _,
            screens,
        } = file::load_config()
            .inspect_err(|x| warn!("Unable to load configuration file: {x}"))
            .ok()?;

        Some(LogicConfiguration {
            vrpn_config: VRPNConfig {
                head: vrpn.head.parse().ok()?,
            },
            child_count: screens.len().try_into().unwrap(),
        })
    }
    HOST_CONFIG.get_or_init(|| build().unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::math::UVec2;
    use std::path::Path;

    #[test]
    fn loads_example_asset_and_builds_configs() {
        // Point the config loader to the example asset
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let example_path = Path::new(manifest_dir)
            .join("assets")
            .join("config_example.toml");

        assert!(
            example_path.exists(),
            "Example config not found at {:?}",
            example_path
        );

        // Ensure the config file is discovered
        unsafe { std::env::set_var("TEPH_CONFIG_PATH", &example_path) };

        // Validate logic configuration derived from the example
        let logic = get_logic_configuration();
        assert_eq!(logic.child_count, 12);
        assert_eq!(logic.vrpn_config.head.sender, "Head0");
        assert_eq!(logic.vrpn_config.head.host, "10.79.144.3");
        assert_eq!(logic.vrpn_config.head.port, 3883);

        // Prepare render context for child 0 and validate render configuration
        unsafe { std::env::set_var("TEPHRITE_CHILD_PROCESS", "0") };
        let render = get_render_configuration();
        assert_eq!(render.process_rank, 0);
        assert_eq!(render.card_index, Some(4));
        assert_eq!(render.display_name.as_deref(), Some(":0.0"));
        assert_eq!(render.resolution, UVec2::new(1920, 1200));
    }
}
