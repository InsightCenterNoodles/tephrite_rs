use bevy::prelude::*;

use crate::backfill;

#[derive(Component)]
#[component(immutable)]
pub struct BReplicate;

/// The Backfill entity for this entity
#[derive(Component)]
#[component(immutable)]
pub(crate) struct BEntity(pub(crate) backfill::EntityId);

/// Component to indicate that we have set the renderable bindings for this entity
#[derive(Component, Debug)]
pub struct BRenderBinding {
    pub(crate) mesh_handle: AssetId<Mesh>,
    pub(crate) mat_handle: AssetId<StandardMaterial>,
}
