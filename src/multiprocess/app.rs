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
        bevy::pbr::PbrPlugin::default(),
        bevy::text::TextPlugin,
    ));

    app.world_mut()
        .insert_resource(CompressedImageFormatSupport(CompressedImageFormats::BC));

    app
}
