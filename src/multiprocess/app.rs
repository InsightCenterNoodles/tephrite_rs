use bevy::{
    app::{App, PanicHandlerPlugin, ScheduleRunnerPlugin, TerminalCtrlCHandlerPlugin, ctrlc},
    diagnostic::DiagnosticsPlugin,
    log::LogPlugin,
    prelude::*,
    time::TimePlugin,
};

/// Watch for an interrupt signal, propagate to Bevy
pub(crate) fn control_c_watcher(app: &mut App) {
    ctrlc::set_handler(|| {
        TerminalCtrlCHandlerPlugin::gracefully_exit();
    })
    .unwrap();

    app.add_systems(PreUpdate, TerminalCtrlCHandlerPlugin::exit_on_flag);
}

/// Catch, and discard, an interrupt
pub(crate) fn control_c_catch(_: &mut App) {
    ctrlc::set_handler(|| {
        debug!("{}: CAUGHT SHUTDOWN", std::process::id());
    })
    .unwrap();
}

pub(crate) fn make_common_app() -> App {
    // build bevy application
    let mut app = App::new();

    app.add_plugins((
        PanicHandlerPlugin,
        LogPlugin {
            level: bevy::log::Level::DEBUG,
            ..Default::default()
        },
        bevy::diagnostic::FrameCountPlugin,
        ScheduleRunnerPlugin::run_loop(std::time::Duration::from_secs_f64(1.0 / 60.0)),
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

    app
}
