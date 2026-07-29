pub(crate) mod common;
pub(crate) mod config;
pub mod environment;
pub mod input;
pub mod material;
pub mod multiprocess;
pub mod remote_control;
mod render;
pub mod replication;
#[doc(hidden)]
pub mod serialize;
pub(crate) mod simulator;
pub mod ui;
pub(crate) mod vrpn;

use std::num::NonZero;

pub use bevy;

use bevy::{
    DefaultPlugins,
    app::{App, Plugin},
    ecs::error::BevyError,
};

pub mod prelude {
    pub use super::TephriteApp;
    pub use super::run;
    pub use crate::common::DeferredRendering;
    pub use crate::common::EnvironmentLighting;
    pub use crate::common::Head;
    pub use crate::common::OffAxisProjectionSettings;
    pub use crate::common::OrderIndependantTransparency;
    pub use crate::common::ScreenSpaceAmbientOcclusionSettings;
    pub use crate::common::ScreenSpaceReflectionsSettings;

    pub use crate::input::*;
    pub use crate::material::*;
}

struct NonTephDefaultPlugin;

impl Plugin for NonTephDefaultPlugin {
    fn build(&self, _app: &mut App) {}
}

pub trait TephriteApp: Plugin {
    fn non_teprite_plugin() -> impl Plugin {
        NonTephDefaultPlugin
    }

    /// Command line processing. This will only be done on the logic process. Return an error of any kind to
    /// halt the application. Command line processing will happen BEFORE your plugin is added to the scene.
    #[allow(unused)]
    fn process_command_line(app: &mut App) -> Result<(), BevyError> {
        Ok(())
    }
}

/// Primary entry point for your application
///
/// As this is a multiprocess application, we need to steal control from normal execution paths.
/// This function takes care of this for you; pass in a plugin that defines your application.
/// See examples for demonstrations of this approach.
///
pub fn run<T: TephriteApp>(user_plugin: T) -> bevy::app::AppExit {
    if std::env::var("TEPH_DISABLE").is_ok() {
        let mut app = App::new();

        app.add_plugins(DefaultPlugins);
        app.add_plugins(user_plugin);
        app.add_plugins(T::non_teprite_plugin());

        return app.run();
    }

    if multiprocess::is_child_process() {
        multiprocess::render_process::run()
    } else {
        let mut app = multiprocess::logic_process::setup();

        if let Err(err) = T::process_command_line(&mut app) {
            bevy::log::error!("Terminating application: {err}");
            return bevy::app::AppExit::Error(NonZero::new(1u8).unwrap());
        }

        app.add_plugins(user_plugin);

        let result = app.run();

        multiprocess::logic_process::cleanup(app);

        result
    }
}
