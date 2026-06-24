use bevy::{
    app::{App, PanicHandlerPlugin, ScheduleRunnerPlugin},
    diagnostic::DiagnosticsPlugin,
    image::{CompressedImageFormatSupport, CompressedImageFormats},
    log::LogPlugin,
    prelude::*,
    sprite_render::ColorMaterialPlugin,
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
        AnimationPlugin,
        bevy::scene::ScenePlugin,
        bevy::mesh::MeshPlugin,
        bevy::image::ImagePlugin::default(),
        bevy::pbr::MaterialPlugin::<StandardMaterial>::default(),
        ColorMaterialPlugin::default(), // we dont use this directly, other things might
        bevy::gltf::GltfPlugin::default(),
        bevy::render::texture::TexturePlugin, // without this, AssetServer does not work.
        bevy::text::TextPlugin,
    ));

    app.world_mut()
        .insert_resource(CompressedImageFormatSupport(CompressedImageFormats::BC));

    app
}
