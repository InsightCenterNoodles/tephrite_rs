use bevy::{
    core_pipeline::tonemapping::Tonemapping,
    diagnostic::{FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin},
    pbr::{DefaultOpaqueRendererMethod, ScreenSpaceAmbientOcclusion, ScreenSpaceReflections},
    prelude::*,
    render::{camera::TemporalJitter, view::Hdr},
    window::EnabledButtons,
};

use crate::config::get_render_configuration;

/// Function to run a render (or child) process
pub(crate) fn run() -> AppExit {
    let mut app = App::new();

    app.insert_resource(DefaultOpaqueRendererMethod::deferred());

    // Get child config
    let child_config = get_render_configuration();
    let rank = child_config.process_rank;

    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            present_mode: bevy::window::PresentMode::Mailbox,
            title: format!("Tephrite Window {}", std::process::id()),
            resolution: (1920, 1200).into(),
            enabled_buttons: EnabledButtons {
                minimize: false,
                maximize: false,
                close: false,
            },
            ..Default::default()
        }),
        ..Default::default()
    }));
    //info!("{rank}: Running render process {}", std::process::id());

    if child_config.process_rank == 0 {
        app.add_plugins(LogDiagnosticsPlugin::default())
            .add_plugins(FrameTimeDiagnosticsPlugin::default());
    }

    if child_config.use_offaxis {
        app.add_plugins(crate::render::OffAxisPlugin);
    } else {
        app.add_systems(PreUpdate, sync_cam_to_head);
    }

    //app.add_plugins(bevy::camera::visibility::VisibilityPlugin);

    app.add_systems(PreStartup, setup);

    // Add in replication components
    app.add_plugins(crate::replication::reader::ReplicationReaderPlugin);

    debug!("{rank}: Render replication ready...");

    // exec
    let result = app.run();

    debug!("{rank}: Stopping renderer...");

    result
}

fn sync_cam_to_head(
    head_q: Query<&Transform, (With<crate::common::Head>, Without<Projection>)>,
    mut proj_q: Query<&mut Transform, (Without<crate::common::Head>, With<Camera3d>)>,
) {
    let Some(head_tf) = head_q.iter().next() else {
        return;
    };

    for mut camera_xform in &mut proj_q {
        *camera_xform = *head_tf;

        let testtf = Transform::from_xyz(5.0, 5.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y);
    }
}

fn setup(mut commands: Commands) {
    let child_config = get_render_configuration();

    let id = commands
        .spawn((
            Camera3d::default(),
            Msaa::Off,
            Tonemapping::AcesFitted,
            Hdr,
            Transform::default(),
            //Transform::from_xyz(5.0, 5.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
            ScreenSpaceAmbientOcclusion {
                quality_level: bevy::pbr::ScreenSpaceAmbientOcclusionQualityLevel::Medium,
                constant_object_thickness: 0.25,
            },
            ScreenSpaceReflections::default(),
            TemporalJitter::default(),
            // EnvironmentMapLight {
            //     diffuse_map: assets.load("ibl/workshop_diffuse.ktx2"),
            //     specular_map: assets.load("ibl/workshop_specular.ktx2"),
            //     intensity: 5000.0,
            //     ..Default::default()
            // },
            // Skybox {
            //     image: assets.load("ibl/workshop_diffuse.ktx2"),
            //     brightness: 5000.0,
            //     ..default()
            // },
        ))
        .id();

    if child_config.use_offaxis {
        let physical = &child_config.display_physical;

        commands.entity(id).insert(
            // Use our custom projection:
            Projection::custom(crate::render::OffAxisProjection::new(
                physical.lower_left.as_vec3(),
                physical.lower_right.as_vec3(),
                physical.upper_right.as_vec3(),
                0.01,
                100.0,
            )),
        );
    }
}
