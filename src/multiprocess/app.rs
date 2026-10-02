use bevy::{
    app::{App, PanicHandlerPlugin},
    diagnostic::DiagnosticsPlugin,
    image::{CompressedImageFormatSupport, CompressedImageFormats},
    log::LogPlugin,
    prelude::*,
    sprite_render::{ColorMaterialPlugin, Mesh2dRenderPlugin},
    time::TimePlugin,
};

pub(crate) fn make_common_app() -> App {
    // build bevy application
    let mut app = App::new();

    app.add_plugins((
        PanicHandlerPlugin,
        LogPlugin {
            filter: "info,bevy_render=off".into(),
            level: if std::env::var("TEPH_DEBUG").is_ok() {
                bevy::log::Level::DEBUG
            } else {
                bevy::log::Level::INFO
            },
            ..Default::default()
        },
        bevy::diagnostic::FrameCountPlugin,
        TaskPoolPlugin::default(),
    ));
    app.add_plugins((
        TimePlugin,
        TransformPlugin,
        DiagnosticsPlugin,
        AssetPlugin {
            unapproved_path_mode: bevy::asset::UnapprovedPathMode::Allow,
            ..Default::default()
        },
        bevy::world_serialization::WorldSerializationPlugin,
        bevy::input::InputPlugin,
    ));

    app.init_asset::<bevy::shader::Shader>()
        .init_asset_loader::<bevy::shader::ShaderLoader>();

    app.add_plugins((
        AnimationPlugin,
        bevy::scene::ScenePlugin,
        bevy::mesh::MeshPlugin,
        bevy::image::ImagePlugin::default(),
        bevy::core_pipeline::CorePipelinePlugin,
        Mesh2dRenderPlugin::default(),
        ColorMaterialPlugin::default(), // we dont use this directly, other things might
        bevy::gltf::GltfPlugin::default(),
        DummyPbrPlugin,
        bevy::render::texture::TexturePlugin,
        bevy::text::TextPlugin,
        bevy::gizmos::GizmoPlugin,
    ));

    app.world_mut()
        .insert_resource(CompressedImageFormatSupport(CompressedImageFormats::BC));

    app
}

/// Placeholder for the reduced PBR setup needed by Tephrite's split-process apps.
#[derive(Default)]
struct DummyPbrPlugin;

impl Plugin for DummyPbrPlugin {
    fn build(&self, app: &mut App) {
        use bevy::pbr::*;

        let debug_flags = bevy::render::RenderDebugFlags::default();

        app.register_asset_reflect::<StandardMaterial>()
            .init_resource::<DefaultOpaqueRendererMethod>()
            .add_plugins((
                MeshRenderPlugin {
                    use_gpu_instance_buffer_builder: true,
                    debug_flags,
                },
                MaterialsPlugin { debug_flags },
                MaterialPlugin::<StandardMaterial> {
                    debug_flags,
                    ..Default::default()
                },
                ScreenSpaceAmbientOcclusionPlugin,
                FogPlugin,
                LightmapPlugin,
                LightProbePlugin,
                VolumetricFogPlugin,
                ScreenSpaceReflectionsPlugin,
                ScreenSpaceTransmissionPlugin,
                ClusteredDecalPlugin,
                ContactShadowsPlugin,
            ))
            .add_plugins((ScatteringMediumPlugin, AtmospherePlugin));

        app.add_plugins(deferred::DeferredPbrLightingPlugin);

        // Initialize the default material handle.
        app.world_mut()
            .resource_mut::<Assets<StandardMaterial>>()
            .insert(
                &Handle::<StandardMaterial>::default(),
                StandardMaterial {
                    base_color: Color::srgb(1.0, 0.0, 0.5),
                    ..Default::default()
                },
            )
            .unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn point_light_can_be_despawned_without_a_render_world() {
        let mut app = make_common_app();
        let light = app.world_mut().spawn(PointLight::default()).id();

        assert!(app.world_mut().despawn(light));
    }
}
