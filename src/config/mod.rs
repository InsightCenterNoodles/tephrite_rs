mod loader;
mod render;
mod schema;
mod vrpn;

use std::sync::OnceLock;

use bevy::log::warn;

#[allow(unused_imports)]
pub use loader::{find_config_file, load_config};
#[allow(unused_imports)]
pub use render::{DisplayPhysical, RenderConfiguration};
#[allow(unused_imports)]
pub use schema::{
    AlertCube, AlertImage, AlertZone, Config, Display, Environment, InteractorConfig,
    InteractorType, Placement, Render, Screen, Vrpn,
};
#[allow(unused_imports)]
pub use vrpn::{VRPNAddress, VRPNAddressParseError, VRPNCoordinateTransform};

pub(crate) use vrpn::deserialize_legacy_vrpn_list;

use crate::multiprocess::child_process_id;

static CONFIG: OnceLock<Config> = OnceLock::new();
static CHILD_CONFIG: OnceLock<RenderConfiguration> = OnceLock::new();

pub fn get_configuration() -> &'static Config {
    CONFIG.get_or_init(|| {
        load_config()
            .inspect_err(|x| warn!("Unable to load configuration file: {x}"))
            .unwrap_or_default()
    })
}

fn build_child_config() -> RenderConfiguration {
    get_configuration().render_configuration(child_process_id())
}

pub fn get_render_configuration() -> &'static RenderConfiguration {
    CHILD_CONFIG.get_or_init(build_child_config)
}
