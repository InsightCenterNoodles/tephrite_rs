//! Mirroring ECS state from the logic process to render processes.
//!
//! Replication is table-driven. During shared Tephrite app configuration, both
//! logic and render apps register the same mirrored components, assets, and
//! resources in the same order. Runtime transcript frames then use compact table
//! IDs to dispatch type-specific serialization and application functions.
//!
//! The transcript is intentionally not a schema negotiation channel. Because
//! both process roles come from the same binary and both execute
//! [`crate::TephriteApp::configure_tephrite`], matching tables are guaranteed by
//! construction when applications use [`crate::run`].

pub mod components;
pub mod instruction;
pub mod reader;
pub mod registry;
pub(crate) mod replicated_assets;
pub mod replicated_components;
mod replicated_resources;
pub mod sets;
pub mod writer;

pub use registry::ReplicationRegistryAppExt;
pub use writer::ReplicationWriterPlugin;

pub(crate) fn register_builtin_replication_types(world: &mut bevy::prelude::World) {
    if !world.contains_resource::<registry::ReplicationRegistry>() {
        world.insert_resource(registry::ReplicationRegistry::default());
    }

    let mut registry = world.resource_mut::<registry::ReplicationRegistry>();
    replicated_components::register_builtin_components(&mut registry);
    replicated_assets::register_builtin_assets(&mut registry);
    replicated_resources::register_builtin_resources(&mut registry);
}
