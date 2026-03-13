use bevy::{
    core_pipeline::{Skybox, tonemapping::Tonemapping},
    diagnostic::{FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin},
    log::{Level, LogPlugin},
    pbr::{DefaultOpaqueRendererMethod, ScreenSpaceAmbientOcclusion, ScreenSpaceReflections},
    prelude::*,
    render::{camera::TemporalJitter, view::Hdr},
    window::EnabledButtons,
};

use crate::{common::EnvironmentLighting, config::get_render_configuration};

/// Function to run a render (or child) process
pub(crate) fn run() -> AppExit {
    let mut app = App::new();

    app.insert_resource(DefaultOpaqueRendererMethod::deferred());

    // Get child config
    let child_config = get_render_configuration();
    let rank = child_config.process_rank;

    if child_config.use_offaxis {
        unsafe {
            // try to set here
            if let Some(index) = child_config.card_index {
                let index = index.to_string();
                std::env::set_var("ENABLE_DEVICE_CHOOSER_LAYER", "1");
                std::env::set_var("VULKAN_DEVICE_INDEX", index);
            }

            if let Some(display) = &child_config.display_name {
                std::env::set_var("DISPLAY", display);
            }
        }
    }

    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    present_mode: bevy::window::PresentMode::Fifo,
                    mode: if child_config.fullscreen {
                        bevy::window::WindowMode::BorderlessFullscreen(MonitorSelection::Primary)
                    } else {
                        bevy::window::WindowMode::Windowed
                    },
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
            })
            .set(LogPlugin {
                level: Level::WARN,
                ..Default::default()
            }),
    );
    //info!("{rank}: Running render process {}", std::process::id());

    // if child_config.process_rank == 0 {
    //     app.add_plugins(LogDiagnosticsPlugin::default())
    //         .add_plugins(FrameTimeDiagnosticsPlugin::default());
    // }

    if child_config.use_offaxis {
        app.add_plugins(crate::render::OffAxisPlugin);
    } else {
        app.add_systems(PreUpdate, sync_cam_to_head);
    }

    //app.add_plugins(bevy::camera::visibility::VisibilityPlugin);

    app.add_systems(PreStartup, setup);

    app.add_systems(Update, env_change_watch);

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

        //let testtf = Transform::from_xyz(5.0, 5.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y);
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
            // ScreenSpaceAmbientOcclusion {
            //     quality_level: bevy::pbr::ScreenSpaceAmbientOcclusionQualityLevel::Medium,
            //     constant_object_thickness: 0.25,
            // },
            // ScreenSpaceReflections {
            //     perceptual_roughness_threshold: 0.25,
            //     thickness: 0.08,
            //     linear_steps: 32,
            //     linear_march_exponent: 2.0,
            //     bisection_steps: 5,
            //     use_secant: true,
            // },
            TemporalJitter::default(),
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
                !child_config.is_right,
            )),
        );
    }
}

fn env_change_watch(
    env: Option<Res<EnvironmentLighting>>,
    mut cam_q: Query<Entity, With<Camera3d>>,
    mut commands: Commands,
    //assets: Res<Assets<Image>>,
) {
    let Some(env) = env else {
        return;
    };

    if !env.is_changed() {
        return;
    }

    for cam in cam_q.iter_mut() {
        let mut ec = commands.entity(cam);
        ec.insert(EnvironmentMapLight {
            diffuse_map: env.diffuse.clone(),
            specular_map: env.specular.clone(),
            intensity: env.intensity,
            ..Default::default()
        });

        if let Some(color) = env.skybox_color {
            commands.insert_resource(ClearColor(color));
        } else {
            ec.insert(Skybox {
                image: env.specular.clone(),
                brightness: env.intensity,
                ..Default::default()
            });
        }
    }
}
