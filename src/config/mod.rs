use crate::multiprocess::child_process_id;
use bevy::{
    log::{error, warn},
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
        pub(crate) use_offaxis: Option<bool>,
        pub(crate) debug_renderer: Option<bool>,
        pub(crate) render: Option<Render>,
        pub(crate) vrpn: Vrpn,
        pub(crate) displays: Vec<Display>,
        pub(crate) screens: Vec<Screen>,
    }

    #[derive(Debug, Default, Deserialize)]
    pub(crate) struct Render {
        pub(crate) api: String,
    }

    #[derive(Debug, Default, Deserialize)]
    pub(crate) struct Vrpn {
        pub(crate) head: Option<String>,
        pub(crate) joystick: Option<String>,
        pub(crate) coordinate_transform: Option<super::VRPNCoordinateTransform>,
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

    #[derive(Debug, Clone, Default, Deserialize)]
    pub(crate) struct Placement {
        /// X x Y
        pub(crate) location: [u32; 2],
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

        pub(crate) placement: Option<Placement>,

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
    pub sensor: Option<u16>,
}

#[derive(Debug, thiserror::Error)]
pub enum VRPNAddressParseError {
    #[error("Missing address part {0}")]
    MissingPart(String),
    #[error("Bad port {0}")]
    BadPort(#[from] std::num::ParseIntError),
    #[error("Invalid sensor {0}")]
    BadSensor(String),
}

impl FromStr for VRPNAddress {
    type Err = VRPNAddressParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Should be in the form of sender/sensor@host:port
        let (sender, endpoint) = s
            .split_once('@')
            .ok_or_else(|| VRPNAddressParseError::MissingPart("host".into()))?;
        let (host, port) = endpoint
            .split_once(':')
            .ok_or_else(|| VRPNAddressParseError::MissingPart("port".into()))?;

        if sender.is_empty() {
            return Err(VRPNAddressParseError::MissingPart("sender".into()));
        }

        if host.is_empty() {
            return Err(VRPNAddressParseError::MissingPart("host".into()));
        }

        if port.is_empty() || port.contains(':') || port.contains('@') {
            return Err(VRPNAddressParseError::MissingPart("port".into()));
        }

        if sender.contains('@') || host.contains('@') || host.contains(':') {
            return Err(VRPNAddressParseError::MissingPart("address".into()));
        }

        let port: u16 = port.parse()?;

        let mut sensor: Option<u16> = None;

        let sender = if let Some((sndr, snsr)) = sender.split_once('/') {
            if sndr.is_empty() {
                return Err(VRPNAddressParseError::MissingPart("sender".into()));
            }

            if snsr.is_empty() || snsr.contains('/') {
                return Err(VRPNAddressParseError::BadSensor(snsr.into()));
            }

            sensor = Some(snsr.parse().map_err(|err: std::num::ParseIntError| {
                VRPNAddressParseError::BadSensor(err.to_string())
            })?);
            sndr
        } else {
            sender
        };

        Ok(Self {
            sender: sender.into(),
            host: host.into(),
            port,
            sensor,
        })
    }
}

/// Configure VRPN connectivity
#[derive(Debug, Default, Clone)]
pub struct VRPNConfig {
    pub head: Option<VRPNAddress>,
    pub joystick: Option<Vec<VRPNAddress>>,
    pub coordinate_transform: VRPNCoordinateTransform,
}

/// Named coordinate transforms for VRPN tracker poses.
#[derive(Debug, Default, Clone, Copy, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VRPNCoordinateTransform {
    /// Preserve the historical Tephrite VRPN mapping: position `[-x, z, y]`
    /// and rotation `[-x, z, y, w]`.
    #[default]
    VrpnBevy,
    /// Use VRPN position and quaternion components as-is.
    Identity,
}

#[derive(Debug, Default, Clone)]
pub struct RenderConfiguration {
    pub use_offaxis: bool,

    pub debug_renderer: bool,

    /// The rank of the process
    pub process_rank: u32,

    /// The render API to force
    pub render_api: Option<String>,

    /// The display to use
    pub display_name: Option<String>,

    /// The graphics card to use
    pub card_index: Option<u32>,

    /// The physical disposition of the display
    pub display_physical: DisplayPhysical,

    /// The pixel resolution of the display (w, h)
    pub resolution: UVec2,

    pub placement: UVec2,

    pub fullscreen: bool,

    pub is_right: bool,
}

#[derive(Debug, Default, Clone)]
pub struct LogicConfiguration {
    pub debug_renderer: bool,

    /// VRPN configuration information
    pub vrpn_config: VRPNConfig,

    /// Child information
    pub child_count: u32,
}

static HOST_CONFIG: OnceLock<LogicConfiguration> = OnceLock::new();
static CHILD_CONFIG: OnceLock<RenderConfiguration> = OnceLock::new();

fn build_child_config() -> RenderConfiguration {
    let file::Config {
        use_offaxis,
        debug_renderer,
        render,
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

    let resolution: UVec2 = this_screen
        .as_ref()
        .and_then(|x| x.placement.clone())
        .map(|x| x.resolution.into())
        .or(this_display.as_ref().map(|x| x.resolution.into()))
        .unwrap_or_else(|| uvec2(1920, 1200));

    let mono_override = std::env::var("TEPH_MONO").ok().map(|_| false);

    let placement: UVec2 = this_screen
        .as_ref()
        .and_then(|x| x.placement.clone())
        .map(|x| x.location.into())
        .unwrap_or_else(|| uvec2(0, 0));

    RenderConfiguration {
        use_offaxis: use_offaxis.unwrap_or_default(),
        debug_renderer: debug_renderer.unwrap_or_default(),
        process_rank: child_process_id(),
        render_api: render.map(|x| x.api),
        card_index: this_screen.as_ref().and_then(|x| x.card_index),
        fullscreen: this_screen
            .as_ref()
            .map(|x| x.fullscreen)
            .unwrap_or_default(),
        is_right: mono_override
            .or(this_screen.as_ref().map(|x| x.is_right))
            .unwrap_or_default(),
        display_name: this_screen.and_then(|x| x.x_display),
        display_physical: this_display
            .map(|x| DisplayPhysical {
                lower_left: x.lower_left.into(),
                lower_right: x.lower_right.into(),
                upper_right: x.upper_right.into(),
            })
            .unwrap_or_else(DisplayPhysical::make_plain),
        resolution,
        placement,
    }
}

pub fn get_render_configuration() -> &'static RenderConfiguration {
    CHILD_CONFIG.get_or_init(build_child_config)
}

fn get_multiple_vrpn_addresses(string: &str) -> Vec<VRPNAddress> {
    string
        .split(',')
        .filter_map(|x| {
            VRPNAddress::from_str(x.trim())
                .inspect_err(|e| error!("Error parsing VRPN address: {e}"))
                .ok()
        })
        .collect()
}

pub fn get_logic_configuration() -> &'static LogicConfiguration {
    fn build() -> Option<LogicConfiguration> {
        let file::Config {
            debug_renderer,
            use_offaxis: _,
            render: _,
            vrpn,
            displays: _,
            screens,
        } = file::load_config()
            .inspect_err(|x| warn!("Unable to load configuration file: {x}"))
            .unwrap_or_default();

        Some(LogicConfiguration {
            debug_renderer: debug_renderer.unwrap_or_default(),
            vrpn_config: VRPNConfig {
                head: vrpn.head.and_then(|x| x.parse().ok()),
                joystick: vrpn.joystick.map(|x| get_multiple_vrpn_addresses(&x)),
                coordinate_transform: vrpn.coordinate_transform.unwrap_or_default(),
            },
            child_count: screens.len().try_into().unwrap(),
        })
    }
    HOST_CONFIG.get_or_init(|| build().unwrap_or_default())
}

pub(crate) const ENV_VAR_LOG_RENDERER: &str = "TEPH_LOG_RENDERER";

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
        let head = logic.vrpn_config.head.as_ref().unwrap();
        assert_eq!(logic.child_count, 12);
        assert_eq!(head.sender, "Head0");
        assert_eq!(head.host, "10.79.144.3");
        assert_eq!(head.port, 3883);
        assert!(matches!(
            logic.vrpn_config.coordinate_transform,
            VRPNCoordinateTransform::VrpnBevy
        ));

        // Prepare render context for child 0 and validate render configuration
        unsafe { std::env::set_var("TEPHRITE_CHILD_PROCESS", "0") };
        let render = get_render_configuration();
        assert_eq!(render.process_rank, 0);
        assert_eq!(render.card_index, Some(4));
        assert_eq!(render.display_name.as_deref(), Some(":0.0"));
        assert_eq!(render.resolution, UVec2::new(1920, 1200));
    }

    #[test]
    fn parses_vrpn_address_without_sensor() {
        let address: VRPNAddress = "Head0@127.0.0.1:3883".parse().unwrap();

        assert_eq!(address.sender, "Head0");
        assert_eq!(address.host, "127.0.0.1");
        assert_eq!(address.port, 3883);
        assert_eq!(address.sensor, None);
    }

    #[test]
    fn parses_vrpn_address_with_sensor() {
        let address: VRPNAddress = "Head0/3@127.0.0.1:3883".parse().unwrap();

        assert_eq!(address.sender, "Head0");
        assert_eq!(address.host, "127.0.0.1");
        assert_eq!(address.port, 3883);
        assert_eq!(address.sensor, Some(3));
    }

    #[test]
    fn rejects_malformed_vrpn_addresses() {
        for address in [
            "@127.0.0.1:3883",
            "/1@127.0.0.1:3883",
            "Head0@",
            "Head0@:3883",
            "Head0@127.0.0.1",
            "Head0@127.0.0.1:",
            "Head0@127.0.0.1:3883:extra",
            "Head0@127.0.0.1:3883@extra",
            "Head0/@127.0.0.1:3883",
            "Head0/1/2@127.0.0.1:3883",
            "Head0/not-a-number@127.0.0.1:3883",
        ] {
            assert!(
                VRPNAddress::from_str(address).is_err(),
                "{address} should not parse"
            );
        }
    }
}
