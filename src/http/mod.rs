use bevy::prelude::*;

pub mod client;
pub mod resources;
pub(crate) mod server;

pub use resources::{HTTPNodeHandler, HTTPResources};
pub use server::HTTPServer;

pub struct HTTPPlugin;

impl Plugin for HTTPPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        app.init_resource::<resources::HTTPResources>();
        app.add_systems(
            Update,
            (
                server::http_service_system,
                client::http_client_service_system,
            )
                .chain(),
        );
    }
}
