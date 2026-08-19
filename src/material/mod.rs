pub mod instance;
pub mod points;

use bevy::{asset::embedded_asset, prelude::*};

pub use instance::{Instance, InstanceMeshMaterial3d, Instances};
pub use points::{PointsMaterial, PointsShaderSettings};

/// Registers Tephrite's point-cloud material and its embedded shader.
///
/// `tephrite_rs::run` installs this automatically. Users building a custom
/// Bevy app can add this plugin function directly before using
/// [`PointsMaterial`].
pub fn points_material_plugin(app: &mut App) {
    embedded_asset!(app, "points/points.wgsl");
    app.add_plugins(MaterialPlugin::<points::PointsMaterial>::default());
}

/// Registers Tephrite's instancing shader.
///
/// Custom instance support is still under construction; this ensures the
/// shader is available from its embedded asset path.
pub fn instance_material_plugin(app: &mut App) {
    embedded_asset!(app, "instance/instancing.wgsl");
    app.add_plugins(instance::InstancedMaterialPlugin);
}

pub(crate) fn builtin_materials_plugin(app: &mut App) {
    points_material_plugin(app);
    instance_material_plugin(app);
}

#[cfg(test)]
mod tests {
    use bevy::prelude::*;

    use super::*;

    #[test]
    fn points_material_plugin_is_publicly_usable() {
        let mut app = App::new();
        app.add_plugins(AssetPlugin::default());
        app.add_plugins(points_material_plugin);

        assert!(app.world().contains_resource::<Assets<PointsMaterial>>());

        let material = PointsMaterial {
            settings: PointsShaderSettings {
                point_size: 4.0,
                color: bevy::color::LinearRgba::WHITE,
            },
            ..Default::default()
        };
        app.world_mut()
            .resource_mut::<Assets<PointsMaterial>>()
            .add(material);
    }

    #[test]
    fn instance_material_plugin_is_publicly_usable() {
        let mut app = App::new();
        app.add_plugins(AssetPlugin::default());
        app.init_asset::<Shader>();
        app.add_plugins(instance_material_plugin);

        assert_eq!(
            instance::INSTANCING_SHADER_ASSET_PATH,
            "embedded://tephrite_rs/material/instance/instancing.wgsl"
        );
    }
}
