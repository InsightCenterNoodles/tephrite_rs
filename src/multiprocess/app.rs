use bevy::{
    app::{App, PanicHandlerPlugin, ScheduleRunnerPlugin},
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
        ScheduleRunnerPlugin::run_loop(std::time::Duration::from_secs_f64(1.0 / 60.0)),
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
        //bevy::pbr::PbrPlugin::default(),
        DummyPbrPlugin,
        bevy::render::texture::TexturePlugin,
        bevy::text::TextPlugin,
        bevy::gizmos::GizmoPlugin,
    ));

    let _ = app
        .world_mut()
        .get_resource_or_init::<Assets<StandardMaterial>>()
        .insert(
            &Handle::<StandardMaterial>::default(),
            StandardMaterial {
                base_color: Color::srgb(1.0, 0.0, 0.5),
                ..Default::default()
            },
        );

    app.world_mut()
        .insert_resource(CompressedImageFormatSupport(CompressedImageFormats::BC));

    app
}

struct DummyPbrPlugin;

impl Plugin for DummyPbrPlugin {
    fn build(&self, app: &mut App) {
        use bevy::light::*;
        use bevy::pbr::*;

        app.register_asset_reflect::<StandardMaterial>()
            .init_resource::<bevy::pbr::DefaultOpaqueRendererMethod>()
            .add_plugins((
                MeshRenderPlugin {
                    use_gpu_instance_buffer_builder: true,
                    debug_flags: Default::default(),
                },
                MaterialsPlugin {
                    debug_flags: Default::default(),
                },
                MaterialPlugin::<StandardMaterial> {
                    debug_flags: Default::default(),
                    ..Default::default()
                },
                ScreenSpaceAmbientOcclusionPlugin,
                FogPlugin,
                //ExtractResourcePlugin::<DefaultOpaqueRendererMethod>::default(),
                //SyncComponentPlugin::<ShadowFilteringMethod>::default(),
                LightmapPlugin,
                LightProbePlugin,
                // GpuMeshPreprocessPlugin {
                //     use_gpu_instance_buffer_builder: self.use_gpu_instance_buffer_builder,
                // },
                VolumetricFogPlugin,
                ScreenSpaceReflectionsPlugin,
                ClusteredDecalPlugin,
            ))
            .add_plugins((
                decal::ForwardDecalPlugin,
                // SyncComponentPlugin::<DirectionalLight>::default(),
                // SyncComponentPlugin::<PointLight>::default(),
                // SyncComponentPlugin::<SpotLight>::default(),
                // SyncComponentPlugin::<AmbientLight>::default(),
            ))
            .add_plugins((ScatteringMediumPlugin, AtmospherePlugin))
            .configure_sets(
                PostUpdate,
                (
                    SimulationLightSystems::AddClusters,
                    SimulationLightSystems::AssignLightsToClusters,
                )
                    .chain(),
            );

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
