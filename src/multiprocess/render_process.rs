use bevy::{
    core_pipeline::{Skybox, oit::OrderIndependentTransparencySettings, tonemapping::Tonemapping},
    diagnostic::{FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin},
    log::{Level, LogPlugin},
    pbr::{DefaultOpaqueRendererMethod, ScreenSpaceAmbientOcclusion, ScreenSpaceReflections},
    prelude::*,
    render::{camera::TemporalJitter, view::Hdr},
    window::EnabledButtons,
};

use crate::{
    common::{EnvironmentLighting, OrderIndependantTransparency},
    config::get_render_configuration,
};

/// Function to run a render (or child) process
pub(crate) fn run() -> AppExit {
    // Get child config
    let child_config = get_render_configuration();
    let rank = child_config.process_rank;

    unsafe {
        // Set process environment before Bevy's render stack has a chance to
        // initialize Vulkan/wgpu.
        if let Some(index) = child_config.card_index {
            let index = index.to_string();
            std::env::set_var("ENABLE_DEVICE_CHOOSER_LAYER", "1");
            std::env::set_var("VULKAN_DEVICE_INDEX", &index);
        }

        if let Some(display) = &child_config.display_name {
            std::env::set_var("DISPLAY", display);
        }
    }

    eprintln!(
        "Renderer env rank={} pid={} DISPLAY={:?} ENABLE_DEVICE_CHOOSER_LAYER={:?} VULKAN_DEVICE_INDEX={:?}",
        rank,
        std::process::id(),
        std::env::var("DISPLAY").ok(),
        std::env::var("ENABLE_DEVICE_CHOOSER_LAYER").ok(),
        std::env::var("VULKAN_DEVICE_INDEX").ok(),
    );

    let mut app = App::new();

    app.insert_resource(DefaultOpaqueRendererMethod::deferred());

    let mut window = Window {
        present_mode: bevy::window::PresentMode::Fifo,
        mode: bevy::window::WindowMode::Windowed,
        title: format!("Tephrite Window {}", std::process::id()),
        resolution: child_config.resolution.into(),
        enabled_buttons: EnabledButtons {
            minimize: false,
            maximize: false,
            close: false,
        },
        position: WindowPosition::At(child_config.placement.as_ivec2()),
        ..Default::default()
    };

    if child_config.fullscreen {
        window.mode = bevy::window::WindowMode::BorderlessFullscreen(MonitorSelection::Primary)
    }

    let window_mode = format!("{:?}", window.mode);

    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(window),
                ..Default::default()
            })
            .set(LogPlugin {
                level: Level::WARN,
                ..Default::default()
            }),
    );

    warn!(
        "Creating render window rank={} pid={} display={:?} card_index={:?} position={:?} resolution={:?} fullscreen={} mode={}",
        rank,
        std::process::id(),
        child_config.display_name,
        child_config.card_index,
        child_config.placement,
        child_config.resolution,
        child_config.fullscreen,
        window_mode,
    );

    //info!("{rank}: Running render process {}", std::process::id());

    if child_config.process_rank == 0 && child_config.debug_renderer {
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

    app.add_systems(Update, env_change_watch);
    app.add_systems(Update, oit_resource_watch);

    // Materials
    app.add_plugins(crate::material::builtin_materials_plugin);

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
            ScreenSpaceAmbientOcclusion {
                quality_level: bevy::pbr::ScreenSpaceAmbientOcclusionQualityLevel::Medium,
                constant_object_thickness: 0.25,
            },
            ScreenSpaceReflections {
                perceptual_roughness_threshold: 0.25,
                thickness: 0.08,
                linear_steps: 8,
                linear_march_exponent: 1.0,
                bisection_steps: 4,
                use_secant: true,
            },
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

fn oit_resource_watch(
    oit: Option<Res<OrderIndependantTransparency>>,
    mut cam_q: Query<Entity, With<Camera3d>>,
    mut commands: Commands,
) {
    let Some(oit) = oit else {
        return;
    };

    if !oit.is_changed() {
        return;
    }

    let oit: &OrderIndependantTransparency = &oit;

    for cam in cam_q.iter_mut() {
        let mut ec = commands.entity(cam);
        ec.insert(OrderIndependentTransparencySettings {
            layer_count: oit.layer_count,
            alpha_threshold: oit.alpha_threshold,
        });
    }
}
