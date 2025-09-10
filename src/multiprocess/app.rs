use bevy::{
    app::{App, PanicHandlerPlugin, ScheduleRunnerPlugin},
    diagnostic::DiagnosticsPlugin,
    log::LogPlugin,
    prelude::*,
    time::TimePlugin,
};

pub fn make_common_app() -> App {
    // build bevy application
    let mut app = App::new();

    app.add_plugins(ScheduleRunnerPlugin::run_loop(
        std::time::Duration::from_secs_f64(1.0 / 61.0),
    ))
    .insert_resource(Assets::<Shader>::default())
    .add_plugins((
        PanicHandlerPlugin,
        LogPlugin {
            level: bevy::log::Level::DEBUG,
            ..Default::default()
        },
        TaskPoolPlugin::default(),
        TimePlugin,
        TransformPlugin,
        DiagnosticsPlugin,
        AssetPlugin::default(),
        AnimationPlugin,
        bevy::scene::ScenePlugin,
        bevy::render::mesh::MeshPlugin,
        bevy::render::texture::ImagePlugin::default(),
        bevy::pbr::MaterialPlugin::<StandardMaterial>::default(),
        bevy::gltf::GltfPlugin::default(),
    ));

    app.add_plugins(bevy::app::PanicHandlerPlugin);

    app.register_type::<bevy::render::primitives::Aabb>();
    app.register_type::<bevy::render::view::visibility::Visibility>();
    app.register_type::<bevy::render::view::visibility::InheritedVisibility>();
    app.register_type::<bevy::render::view::visibility::ViewVisibility>();
    app.register_type::<bevy::render::view::visibility::VisibilityClass>();

    app
}
