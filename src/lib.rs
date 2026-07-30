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

use std::{fmt::Debug, num::NonZero};

pub use bevy;

use bevy::{
    DefaultPlugins,
    app::{App, Plugin, Plugins},
    ecs::error::BevyError,
    prelude::{Asset, Component, Resource},
};
use replication::ReplicationRegistryAppExt;
use serialize::{FastRead, FastWrite, RemappableAsset};

pub mod prelude {
    pub use super::ApplyMode;
    pub use super::TephriteApp;
    pub use super::TephriteAppConfig;
    pub use super::run;
    pub use crate::common::DeferredRendering;
    pub use crate::common::EnvironmentLighting;
    pub use crate::common::Head;
    pub use crate::common::OffAxisProjectionSettings;
    pub use crate::common::OrderIndependantTransparency;
    pub use crate::common::ScreenSpaceAmbientOcclusionSettings;
    pub use crate::common::ScreenSpaceReflectionsSettings;

    pub use crate::serialize::ByteSink;
    pub use crate::serialize::ByteSource;
    pub use crate::serialize::FastRead;
    pub use crate::serialize::FastWrite;

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

    /// Add Tephrite-specific configuration that must be applied to both the
    /// logic process and every render process.
    ///
    /// This hook runs after the user plugin has been added to the logic app and
    /// during render process setup before the replication reader is installed.
    /// It is the right place to register mirrored components, assets, resources,
    /// and renderer-side plugins that must exist on both sides of the process
    /// boundary.
    ///
    /// Table IDs for mirrored data are assigned by insertion order. Because this
    /// hook is executed by both sides from the same binary, registrations made
    /// here produce matching tables without sending table metadata over the
    /// transcript.
    #[allow(unused)]
    fn configure_tephrite(config: &mut TephriteAppConfig) {}
}

type AppConfigurator = Box<dyn FnOnce(&mut App) + Send + 'static>;

#[derive(Debug, Clone, Copy)]
pub enum ApplyMode {
    Both,
    RenderOnly,
}

/// Shared Tephrite configuration applied to both logic and render apps.
///
/// `TephriteAppConfig` stores an ordered list of app mutations. The list is
/// built once in the logic process and once in each render process by calling
/// [`TephriteApp::configure_tephrite`]. Built-in mirror registrations are added
/// first, then user-provided configuration is appended.
///
/// Use this instead of conditionally configuring only the logic or render path
/// when a type must be mirrored. Deterministic insertion order is what gives the
/// replication registry stable compact table IDs.
pub struct TephriteAppConfig {
    // Plugins can be split between both worlds or only render
    configurators: Vec<(AppConfigurator, ApplyMode)>,
}

impl TephriteAppConfig {
    /// Create a config preloaded with Tephrite's built-in mirror registrations.
    ///
    /// Tests and custom harnesses that bypass [`run`] can use this and then
    /// call [`Self::apply_to`] before installing the replication writer or
    /// reader plugin.
    pub fn new() -> Self {
        let mut config = Self {
            configurators: Vec::new(),
        };
        config.register_builtin_mirrors();
        config
    }

    /// Queue an arbitrary app mutation to run on both logic and render apps.
    ///
    /// This is the escape hatch for setup that is more specific than the mirror
    /// helpers below. The closure is stored and executed later, preserving the
    /// order in which calls were made.
    pub fn configure_app<F>(&mut self, configure: F, mode: ApplyMode) -> &mut Self
    where
        F: FnOnce(&mut App) + Send + 'static,
    {
        self.configurators.push((Box::new(configure), mode));
        self
    }

    /// Add Bevy plugins to both logic and render apps.
    ///
    /// Use this for renderer support plugins required by mirrored data that should be created on both logic and render processes. The
    /// plugin value is consumed by the queued closure, so construct it directly
    /// in the call.
    pub fn add_plugins<M, Marker>(&mut self, plugins: M) -> &mut Self
    where
        M: Plugins<Marker> + Send + 'static,
        Marker: 'static,
    {
        self.configure_app(
            |app| {
                app.add_plugins(plugins);
            },
            ApplyMode::Both,
        )
    }

    /// Add Bevy plugins to only render apps.
    ///
    /// Use this for renderer support plugins required by mirrored data. The
    /// plugin value is consumed by the queued closure, so construct it directly
    /// in the call.
    pub fn add_plugins_render_only<M, Marker>(&mut self, plugins: M) -> &mut Self
    where
        M: Plugins<Marker> + Send + 'static,
        Marker: 'static,
    {
        self.configure_app(
            |app| {
                app.add_plugins(plugins);
            },
            ApplyMode::RenderOnly,
        )
    }

    /// Register a component type for automatic entity replication.
    ///
    /// Any entity that has at least one mirrored component is tracked, along
    /// with its parent chain. The component must implement Tephrite's fast
    /// serializer on both write and read sides.
    pub fn mirror_component<C>(&mut self) -> &mut Self
    where
        C: Component + FastWrite + FastRead<Ret = C> + 'static,
    {
        self.configure_app(
            |app| {
                app.replicate_component::<C>();
            },
            ApplyMode::Both,
        )
    }

    /// Register an asset type for mirroring through the transcript.
    ///
    /// Asset IDs are remapped on the render side, so mirrored asset types must
    /// implement [`RemappableAsset`] in addition to fast serialization.
    pub fn mirror_asset<A>(&mut self) -> &mut Self
    where
        A: Asset + FastWrite + FastRead<Ret = A> + RemappableAsset + Debug + 'static,
    {
        self.configure_app(
            |app| {
                app.replicate_asset::<A>();
            },
            ApplyMode::Both,
        )
    }

    /// Register a resource type for mirroring through the transcript.
    ///
    /// Changed resources are sent as complete resource values. Resource removal
    /// is also tracked and mirrored to the render app.
    pub fn mirror_resource<R>(&mut self) -> &mut Self
    where
        R: Resource + FastWrite + FastRead<Ret = R> + 'static,
    {
        self.configure_app(
            |app| {
                app.replicate_resource::<R>();
            },
            ApplyMode::Both,
        )
    }

    /// Apply all queued configuration to an app.
    ///
    /// Normal applications do not need to call this directly; [`run`] applies
    /// the config to both process roles. It is public for tests and custom app
    /// harnesses that construct writer/reader apps manually.
    pub fn apply_to(self, app: &mut App, is_render_process: bool) {
        for configurator in self.configurators {
            match configurator.1 {
                ApplyMode::Both => configurator.0(app),
                ApplyMode::RenderOnly => {
                    if is_render_process {
                        configurator.0(app);
                    }
                }
            }
        }
    }

    fn register_builtin_mirrors(&mut self) {
        self.configure_app(
            |app| {
                crate::replication::register_builtin_replication_types(app.world_mut());
            },
            ApplyMode::Both,
        );
    }
}

impl Default for TephriteAppConfig {
    fn default() -> Self {
        Self::new()
    }
}

fn apply_tephrite_config<T: TephriteApp>(app: &mut App, is_render_process: bool) {
    let mut config = TephriteAppConfig::new();
    T::configure_tephrite(&mut config);
    config.apply_to(app, is_render_process);
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
        apply_tephrite_config::<T>(&mut app, true);

        return app.run();
    }

    if multiprocess::is_child_process() {
        multiprocess::render_process::run::<T>()
    } else {
        let mut app = multiprocess::logic_process::setup();

        if let Err(err) = T::process_command_line(&mut app) {
            bevy::log::error!("Terminating application: {err}");
            return bevy::app::AppExit::Error(NonZero::new(1u8).unwrap());
        }

        app.add_plugins(user_plugin);
        apply_tephrite_config::<T>(&mut app, false);
        multiprocess::logic_process::finish_setup(&mut app);

        let result = app.run();

        multiprocess::logic_process::cleanup(app);

        result
    }
}
