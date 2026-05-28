use serde::Deserialize;

use super::{VRPNAddress, VRPNCoordinateTransform, deserialize_legacy_vrpn_list};

#[derive(Debug, Default, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub use_offaxis: bool,
    #[serde(default)]
    pub debug_renderer: bool,
    #[allow(dead_code)]
    pub render: Option<Render>,
    #[serde(default)]
    pub vrpn: Vrpn,
    #[serde(default)]
    pub environment: Environment,
    #[serde(default)]
    pub displays: Vec<Display>,
    #[serde(default)]
    pub screens: Vec<Screen>,
}

#[derive(Debug, Default, Clone, Deserialize)]
pub struct Environment {
    #[serde(default)]
    pub alerts: Vec<AlertZone>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AlertZone {
    pub plane_point: [f32; 3],
    pub plane_normal: [f32; 3],
    pub distance: f32,
    pub location: [f32; 3],
    pub direction: [f32; 3],
    pub scale: f32,
    #[serde(default)]
    pub image: AlertImage,
}

#[derive(Debug, Default, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlertImage {
    #[default]
    Forward,
    Left,
    Rear,
    Right,
}

#[derive(Debug, Default, Clone, Deserialize)]
#[allow(dead_code)]
pub struct Render {
    pub api: String,
}

#[derive(Debug, Default, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InteractorType {
    #[default]
    Controller,
    Flystick,
}

#[derive(Debug, Default, Clone, Deserialize)]
pub struct InteractorConfig {
    #[serde(default)]
    pub addresses: Vec<VRPNAddress>,
    #[serde(rename = "type", default)]
    pub ty: InteractorType,
}

#[derive(Debug, Default, Clone, Deserialize)]
pub struct Vrpn {
    pub head: Option<VRPNAddress>,

    #[serde(default)]
    pub late_latch_head: bool,

    // LEGACY OPTION. Remove later.
    #[serde(
        default,
        rename = "joystick",
        deserialize_with = "deserialize_legacy_vrpn_list"
    )]
    pub joystick_legacy: Option<Vec<VRPNAddress>>,

    pub interactor: Option<InteractorConfig>,

    #[serde(default)]
    pub coordinate_transform: VRPNCoordinateTransform,
}

#[derive(Debug, Default, Clone, Deserialize)]
pub struct Display {
    // 3D points
    pub lower_left: [f64; 3],
    pub lower_right: [f64; 3],
    pub upper_right: [f64; 3],
    /// width x height
    pub resolution: [u32; 2],
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Placement {
    /// X x Y
    pub location: [u32; 2],
    /// width x height
    pub resolution: [u32; 2],
}

#[derive(Debug, Default, Clone, Deserialize)]
pub struct Screen {
    // index into `displays`
    pub display: u32,
    pub card_index: Option<u32>,
    pub x_display: Option<String>,

    #[serde(default)]
    pub fullscreen: bool,

    pub placement: Option<Placement>,

    #[serde(default)]
    pub is_right: bool,
}
