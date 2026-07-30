use bevy::prelude::*;

use crate::prelude::PointsMaterial;
use crate::replication::registry::ReplicationRegistry;

pub(crate) fn register_builtin_assets(registry: &mut ReplicationRegistry) {
    registry
        .register_asset::<Mesh>()
        .register_asset::<StandardMaterial>()
        .register_asset::<PointsMaterial>()
        .register_asset::<Font>()
        .register_asset::<Image>();
}
