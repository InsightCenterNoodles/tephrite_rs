use crate::multiprocess::child_process_id;
use bevy::{
    math::{DVec3, IVec2},
    reflect::Reflect,
};
use std::{str::FromStr, sync::OnceLock};

/// Physical location of the display, as measured in room coordinates
#[derive(Debug, Default, Reflect, Clone)]
pub struct ScreenPhys {
    pub lower_left: DVec3,
    pub lower_right: DVec3,
    pub upper_right: DVec3,
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
pub struct Configuration {
    /// The rank of the process
    pub process_rank: u32,

    /// The display to use
    pub display_name: Option<String>,

    /// The graphics card to use
    pub card_index: Option<u32>,

    /// The physical disposition of the display
    pub display_physical: ScreenPhys,

    /// The pixel resolution of the display (w, h)
    pub resolution: IVec2,

    /// VRPN configuration information
    pub vrpn_config: VRPNConfig,
}

fn get_hacky_config() -> [ScreenPhys; 6] {
    [
        ScreenPhys {
            lower_left: (-2.499, 0.915, -1.768).into(),
            lower_right: (0.432, 0.915, -1.768).into(),
            upper_right: (0.432, 2.468, -1.768).into(),
        },
        ScreenPhys {
            lower_left: (-2.497, 0.001, -1.768).into(),
            lower_right: (0.432, -0.001, -1.768).into(),
            upper_right: (0.431, 1.554, -1.768).into(),
        },
        ScreenPhys {
            lower_left: (-0.469, 0.913, -1.768).into(),
            lower_right: (2.466, 0.911, -1.768).into(),
            upper_right: (2.466, 2.466, -1.768).into(),
        },
        ScreenPhys {
            lower_left: (-0.467, 0.001, -1.768).into(),
            lower_right: (2.466, 0.000, -1.768).into(),
            upper_right: (2.466, 1.553, -1.768).into(),
        },
        ScreenPhys {
            lower_left: (-2.494, 0.0, -0.1719).into(),
            lower_right: (0.436, 0.0, -0.175).into(),
            upper_right: (0.436, 0.0, -1.768).into(),
        },
        ScreenPhys {
            lower_left: (-0.467, 0.0, -0.175).into(),
            lower_right: (2.472, 0.0, -0.175).into(),
            upper_right: (2.468, 0.0, -1.768).into(),
        },
    ]
}

static CONFIG: OnceLock<Configuration> = OnceLock::new();

/// Build a dummy config till we can figure out how to work with files
fn build_child_config() -> Configuration {
    let process_rank = child_process_id();

    let physical_id = match process_rank {
        0 | 1 => 0,
        2 | 3 => 1,
        4 | 5 => 2,
        6 | 7 => 3,
        8 | 9 => 4,
        10 | 11 => 5,
        _ => panic!("Does not yet support this screen configuration"),
    };

    let card_index = match process_rank {
        0 | 1 => 4,
        2 | 3 => 5,
        4 | 5 => 2,
        6 | 7 => 3,
        8 | 9 => 1,
        10 | 11 => 0,
        _ => panic!("Unknown display index"),
    };

    Configuration {
        process_rank,
        card_index: Some(card_index),
        display_name: Some(format!(":0.{process_rank}")),
        display_physical: get_hacky_config()[physical_id as usize].clone(),
        resolution: (1920, 1200).into(),
        vrpn_config: VRPNConfig {
            head: "Head0@10.79.144.3:3883".parse().unwrap(),
        },
    }
}

pub fn get_child_configuration() -> &'static Configuration {
    CONFIG.get_or_init(build_child_config)
}
