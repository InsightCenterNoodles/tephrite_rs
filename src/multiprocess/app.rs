use bevy::{
    app::{App, PanicHandlerPlugin, ScheduleRunnerPlugin},
    diagnostic::DiagnosticsPlugin,
    log::LogPlugin,
    prelude::*,
    time::TimePlugin,
};

pub(crate) fn make_common_app() -> App {
    // build bevy application
    let mut app = App::new();

    app.add_plugins(ScheduleRunnerPlugin::run_loop(
        std::time::Duration::from_secs_f64(1.0 / 60.0),
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
        bevy::mesh::MeshPlugin,
        bevy::image::ImagePlugin::default(),
        bevy::pbr::MaterialPlugin::<StandardMaterial>::default(),
        bevy::gltf::GltfPlugin::default(),
    ));

    app.register_type::<bevy::camera::primitives::Aabb>();
    app.register_type::<bevy::camera::visibility::Visibility>();
    app.register_type::<bevy::camera::visibility::InheritedVisibility>();
    app.register_type::<bevy::camera::visibility::ViewVisibility>();
    app.register_type::<bevy::camera::visibility::VisibilityClass>();

    app
}
