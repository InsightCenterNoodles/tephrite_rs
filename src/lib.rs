pub(crate) mod common;
pub(crate) mod config;
pub mod environment;
pub mod input;
pub mod material;
pub mod multiprocess;
pub mod remote_control;
mod render;
pub mod replication;
pub(crate) mod serialize;
pub(crate) mod simulator;
pub mod ui;
pub(crate) mod vrpn;

use bevy::{
    DefaultPlugins,
    app::{App, Plugin},
};

pub mod prelude {
    pub use super::run;
    pub use crate::common::DeferredRendering;
    pub use crate::common::EnvironmentLighting;
    pub use crate::common::Head;
    pub use crate::common::OffAxisProjectionSettings;
    pub use crate::common::OrderIndependantTransparency;
    pub use crate::common::ScreenSpaceAmbientOcclusionSettings;
    pub use crate::common::ScreenSpaceReflectionsSettings;
    pub use crate::replication::components::PropagateReplication;
    pub use crate::replication::components::Replicated;

    pub use crate::input::*;
    pub use crate::material::*;
}

/// Primary entry point for your application
///
/// As this is a multiprocess application, we need to steal control from normal execution paths.
/// This function takes care of this for you; pass in a plugin that defines your application.
/// See examples for demonstrations of this approach.
///
pub fn run(user_plugin: impl Plugin) -> bevy::app::AppExit {
    if multiprocess::is_child_process() {
        multiprocess::render_process::run()
    } else {
        let mut app = multiprocess::logic_process::setup();

        app.add_plugins(user_plugin);

        let result = app.run();

        multiprocess::logic_process::cleanup(app);

        result
    }
}

pub enum RunOption {
    Normal,
    DisableTephrite,
}

pub struct TephriteOptions {
    run_options: RunOption,
}

pub fn run_with_options(
    user_plugin: impl Plugin,
    non_teprite_plugin: impl Plugin,
    options: TephriteOptions,
) -> bevy::app::AppExit {
    match options.run_options {
        RunOption::Normal => run(user_plugin),
        RunOption::DisableTephrite => {
            let mut app = App::new();

            app.add_plugins(DefaultPlugins);
            app.add_plugins(user_plugin);
            app.add_plugins(non_teprite_plugin);

            return app.run();
        }
    }
}
