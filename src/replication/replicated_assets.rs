use bevy::prelude::*;

use crate::prelude::PointsMaterial;
use crate::replication::registry::ReplicationRegistry;

pub(crate) fn register_builtin_assets(registry: &mut ReplicationRegistry) {
    registry
        .register_asset::<Mesh>()
        .register_asset::<Image>()
        .register_asset::<StandardMaterial>()
        .register_asset::<PointsMaterial>()
        .register_asset::<Font>();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn images_are_replicated_before_materials() {
        let mut registry = ReplicationRegistry::default();
        register_builtin_assets(&mut registry);

        let image_index = registry
            .assets()
            .iter()
            .position(|entry| entry.name == std::any::type_name::<Image>())
            .expect("Image should be registered");
        let material_index = registry
            .assets()
            .iter()
            .position(|entry| entry.name == std::any::type_name::<StandardMaterial>())
            .expect("StandardMaterial should be registered");

        assert!(image_index < material_index);
    }
}
