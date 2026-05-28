use std::num::NonZeroU32;

use bevy::{
    app::TaskPoolThreadAssignmentPolicy,
    core_pipeline::{Skybox, oit::OrderIndependentTransparencySettings, tonemapping::Tonemapping},
    diagnostic::{FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin},
    log::{Level, LogPlugin},
    pbr::{DefaultOpaqueRendererMethod, ScreenSpaceAmbientOcclusion, ScreenSpaceReflections},
    prelude::*,
    render::{camera::TemporalJitter, pipelined_rendering::PipelinedRenderingPlugin, view::Hdr},
    window::EnabledButtons,
};

use crate::{
    common::{
        DeferredRendering, EnvironmentLighting, OffAxisProjectionSettings,
        OrderIndependantTransparency, ScreenSpaceAmbientOcclusionSettings,
        ScreenSpaceReflectionsSettings,
    },
    config::get_render_configuration,
};

/// Function to run a render (or child) process
pub(crate) fn run() -> AppExit {
    // Get child config
    let child_config = get_render_configuration();
    let rank = child_config.process_rank;
    let vulkan_support_client =
        crate::multiprocess::vulkan_support::init_client(&child_config.vulkan_support);

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

    let mut app = App::new();

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
        desired_maximum_frame_latency: Some(unsafe { NonZeroU32::new_unchecked(1) }),
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
                level: Level::ERROR,
                ..Default::default()
            })
            .set(TaskPoolPlugin {
                task_pool_options: TaskPoolOptions {
                    min_total_threads: 1,
                    max_total_threads: 8,
                    io: TaskPoolThreadAssignmentPolicy {
                        // say we know our app is i/o intensive (asset streaming?)
                        // so maybe we want lots of i/o threads
                        min_threads: 1,
                        max_threads: 2,
                        percent: 0.5, // use 50% of available threads for I/O
                        on_thread_spawn: None,
                        on_thread_destroy: None,
                    },
                    async_compute: TaskPoolThreadAssignmentPolicy {
                        min_threads: 1,
                        max_threads: 1,
                        percent: 0.0,
                        on_thread_spawn: None,
                        on_thread_destroy: None,
                    },
                    compute: TaskPoolThreadAssignmentPolicy {
                        min_threads: 2,
                        // but limit it to a maximum of 8 threads
                        max_threads: 8,
                        // 1.0 in this case means "use all remaining threads"
                        // (that were not assigned to io/async_compute)
                        // (clamped to min_threads..=max_threads)
                        percent: 1.0,
                        on_thread_spawn: None,
                        on_thread_destroy: None,
                    },
                },
            })
            .build()
            .disable::<PipelinedRenderingPlugin>(),
    );

    debug!(
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
        app.add_systems(
            Update,
            sync_cam_to_head
                .in_set(crate::render::TephriteRenderSystems::UpdateCamera)
                .after(crate::render::TephriteRenderSystems::LateLatchHead),
        );
    }

    if child_config.late_latch_head {
        app.add_plugins(crate::vrpn::RenderHeadTrackerPlugin);
    }

    //app.add_plugins(bevy::camera::visibility::VisibilityPlugin);

    app.add_systems(PreStartup, setup);

    app.add_systems(Update, env_change_watch);
    app.add_systems(Update, oit_resource_watch);
    app.add_systems(Update, deferred_rendering_watch);
    app.add_systems(Update, ssao_resource_watch);
    app.add_systems(Update, ssr_resource_watch);

    // Materials
    app.add_plugins(crate::material::builtin_materials_plugin);

    // Add in replication components
    app.add_plugins(crate::replication::reader::ReplicationReaderPlugin);

    debug!("{rank}: Render replication ready...");

    // exec
    let result = app.run();

    debug!("{rank}: Stopping renderer...");
    drop(vulkan_support_client);

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
            TemporalJitter::default(),
        ))
        .id();

    if child_config.use_offaxis {
        let physical = &child_config.display_physical;
        let projection_settings = OffAxisProjectionSettings::default();

        commands.entity(id).insert(
            // Use our custom projection:
            Projection::custom(crate::render::OffAxisProjection::new(
                physical.lower_left.as_vec3(),
                physical.lower_right.as_vec3(),
                physical.upper_right.as_vec3(),
                projection_settings.near,
                projection_settings.far,
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

fn deferred_rendering_watch(
    deferred: Option<Res<DeferredRendering>>,
    mut commands: Commands,
    mut was_enabled: Local<bool>,
) {
    let Some(deferred) = deferred else {
        if *was_enabled {
            commands.insert_resource(DefaultOpaqueRendererMethod::forward());
            *was_enabled = false;
        }

        return;
    };

    if !deferred.is_changed() && *was_enabled {
        return;
    }

    commands.insert_resource(DefaultOpaqueRendererMethod::deferred());
    *was_enabled = true;
}

fn ssao_resource_watch(
    ssao: Option<Res<ScreenSpaceAmbientOcclusionSettings>>,
    mut cam_q: Query<Entity, With<Camera3d>>,
    mut commands: Commands,
    mut was_enabled: Local<bool>,
) {
    let Some(ssao) = ssao else {
        if *was_enabled {
            for cam in cam_q.iter_mut() {
                commands.entity(cam).remove::<ScreenSpaceAmbientOcclusion>();
            }

            *was_enabled = false;
        }

        return;
    };

    if !ssao.is_changed() {
        return;
    }

    for cam in cam_q.iter_mut() {
        commands.entity(cam).insert(ScreenSpaceAmbientOcclusion {
            quality_level: ssao.quality_level,
            constant_object_thickness: ssao.constant_object_thickness,
        });
    }

    *was_enabled = true;
}

fn ssr_resource_watch(
    ssr: Option<Res<ScreenSpaceReflectionsSettings>>,
    mut cam_q: Query<Entity, With<Camera3d>>,
    mut commands: Commands,
    mut was_enabled: Local<bool>,
) {
    let Some(ssr) = ssr else {
        if *was_enabled {
            for cam in cam_q.iter_mut() {
                commands.entity(cam).remove::<ScreenSpaceReflections>();
            }

            *was_enabled = false;
        }

        return;
    };

    if !ssr.is_changed() {
        return;
    }

    for cam in cam_q.iter_mut() {
        commands.entity(cam).insert(ScreenSpaceReflections {
            perceptual_roughness_threshold: ssr.perceptual_roughness_threshold,
            thickness: ssr.thickness,
            linear_steps: ssr.linear_steps,
            linear_march_exponent: ssr.linear_march_exponent,
            bisection_steps: ssr.bisection_steps,
            use_secant: ssr.use_secant,
        });
    }

    *was_enabled = true;
}
