//! Minimal in-process remote control webserver for Bevy apps.
//!
//! 1. Add [`RemoteControlPlugin`]. This is usually done automatically by Tephrite.
//! 2. Initialize or fetch [`RemoteControlDefinitions`].
//! 3. Push one [`PropertyDefinition`] per controllable property.
//! 4. Observe [`RemoteControlEvent`] on those property entities (or use
//!    [`use_cases::RemoteControlTransform`] for common transform controls).
//! 5. In the observer, mutate your world state/components based on `event.value`.
//!
//! Property routing uses `(entity, aspect_id)` as a composite identifier. This
//! allows multiple controls to target one entity without allocating helper
//! entities just to disambiguate callbacks.
//!
//! # Example
//! ```ignore
//! use bevy::prelude::*;
//! use tephrite_rs::remote_control::prelude::*;
//!
//! fn setup(mut commands: Commands, mut defs: ResMut<RemoteControlDefinitions>) {
//!     let speed_property = commands.spawn_empty().id();
//!     defs.push(PropertyDefinition {
//!         id: speed_property,
//!         aspect_id: 0, // Multiple definitions can refer to the same entity; use this to discriminate between them.
//!         label: "Speed".into(),
//!         control: PropertyControl::Slider {
//!             min: 0.0,
//!             max: 20.0,
//!             step: 0.1,
//!             initial: 5.0,
//!         },
//!     });
//!
//!     commands
//!         .entity(speed_property)
//!         .observe(|trigger: On<RemoteControlEvent>, mut query: Query<&mut Transform>| {
//!             if let Ok(mut tf) = query.get_mut(trigger.entity) {
//!                 if let PropertyValue::Float(v) = trigger.event().value {
//!                     tf.translation.x = v;
//!                 }
//!             }
//!         });
//! }
//!
//! App::new()
//!     .add_plugins(DefaultPlugins)
//!     .init_resource::<RemoteControlDefinitions>()
//!     .add_plugins(RemoteControlPlugin::default())
//!     .add_systems(Startup, setup);
//! ```

pub mod common;
pub(crate) mod content;
pub mod events;
pub mod property;
mod scene_api;
mod server;
pub mod use_cases;

use bevy::prelude::*;
use bevy::remote::{
    RemotePlugin,
    http::{Headers, RemoteHttpPlugin},
};

use crate::common::TephExit;
use crate::remote_control::events::{RemoteControlEvent, RemoteControlEventInternal};
use crate::remote_control::property::PropertyDefinition;
use crate::remote_control::server::{check_updates, register_http_handlers};

/// Startup definitions consumed by [`RemoteControlPlugin`] to build the control page.
///
/// Typical setup:
/// - call `app.init_resource::<RemoteControlDefinitions>()`
/// - in startup systems, push [`PropertyDefinition`] entries into this resource
/// - add observers on each property entity for [`events::RemoteControlEvent`]
#[derive(Debug, Default, Resource)]
pub struct RemoteControlDefinitions(pub Vec<PropertyDefinition>);

impl RemoteControlDefinitions {
    /// Add one property definition to be exposed on the remote control page.
    pub fn push(&mut self, definition: PropertyDefinition) {
        self.0.push(definition);
    }

    /// Extend the exposed property list.
    pub fn extend(&mut self, definitions: impl IntoIterator<Item = PropertyDefinition>) {
        self.0.extend(definitions);
    }
}

#[derive(Debug, Default, Resource)]
pub struct RemoteControlOpts {
    bind_addr: String,
    brp_port: Option<u16>,
}

/// Bevy plugin that hosts the local remote-control HTTP endpoint.
///
/// The plugin snapshots [`RemoteControlDefinitions`] during `PostStartup`.
/// Definitions added after startup are not reflected until next app launch.
pub struct RemoteControlPlugin {
    /// HTTP bind address for the control page (for example `127.0.0.1:8081`).
    pub bind_addr: String,
    /// HTTP port for the Bevy Remote Protocol endpoint.
    ///
    /// Set to `None` to disable BRP hosting for custom harnesses or tests that
    /// only need the Tephrite control page.
    pub brp_port: Option<u16>,
}

impl Default for RemoteControlPlugin {
    fn default() -> Self {
        Self {
            bind_addr: "0.0.0.0:8081".into(),
            brp_port: Some(bevy::remote::http::DEFAULT_PORT),
        }
    }
}

impl Plugin for RemoteControlPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        if !app.is_plugin_added::<crate::http::HTTPPlugin>() {
            app.add_plugins(crate::http::HTTPPlugin);
        }

        register_http_handlers(&mut app.world_mut().resource_mut::<crate::http::HTTPResources>());

        app.insert_resource(RemoteControlOpts {
            bind_addr: self.bind_addr.clone(),
            brp_port: self.brp_port,
        });

        app.add_systems(Startup, launch_http_server);

        if let Some(brp_port) = self.brp_port {
            let headers = Headers::new()
                .insert("Access-Control-Allow-Origin", "*")
                .insert("Access-Control-Allow-Headers", "Content-Type");
            app.add_plugins(RemotePlugin::default());
            app.add_plugins(
                RemoteHttpPlugin::default()
                    .with_port(brp_port)
                    .with_headers(headers),
            );
        }

        app.world_mut()
            .get_resource_or_init::<RemoteControlDefinitions>();
        app.add_systems(Update, check_updates);
        app.add_observer(bounce);

        app.add_plugins(use_cases::UseCasesPlugin);
    }
}

fn launch_http_server(mut commands: Commands, res: Res<RemoteControlOpts>) {
    let server = match crate::http::HTTPServer::new(&res.bind_addr) {
        Ok(x) => x,
        Err(err) => {
            error!("unable to spawn remote control server: {err}");
            return;
        }
    };

    commands.spawn(server);
}

/// Translate internal remote-control events into public Bevy entity events.
fn bounce(trigger: On<RemoteControlEventInternal>, mut commands: Commands) {
    info!("Handling remote control event {:?}", trigger.event());
    match trigger.event() {
        RemoteControlEventInternal::PropertyChanged {
            property,
            aspect_id,
            value,
        } => commands.trigger(RemoteControlEvent {
            entity: *property,
            aspect_id: *aspect_id,
            value: value.clone(),
        }),
        RemoteControlEventInternal::QuitRequested => {
            commands.trigger(TephExit);
        }
    }
}

#[cfg(test)]
mod tests;

pub mod prelude {
    pub use super::RemoteControlDefinitions;
    pub use super::RemoteControlPlugin;
    pub use super::events::RemoteControlEvent;
    pub use super::property::PropertyControl;
    pub use super::property::PropertyDefinition;

    pub use super::use_cases::*;
}
