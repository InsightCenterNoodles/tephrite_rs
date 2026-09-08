use std::time::{Duration, Instant};

use bevy::{
    app::TaskPoolThreadAssignmentPolicy,
    camera::{Hdr, visibility::RenderLayers},
    core_pipeline::{
        Skybox,
        core_3d::{prepare_core_3d_depth_textures, prepare_prepass_textures},
        oit::OrderIndependentTransparencySettings,
        tonemapping::Tonemapping,
    },
    diagnostic::{FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin},
    log::{Level, LogPlugin},
    pbr::{
        DefaultOpaqueRendererMethod, ScreenSpaceAmbientOcclusion, ScreenSpaceReflections,
        prepare_clusters_for_cpu_clustering, prepare_fog,
    },
    prelude::*,
    render::{
        ExtractSchedule, Render, RenderApp, RenderSystems,
        batching::gpu_preprocessing::clear_bin_unpacking_buffers, camera::TemporalJitter,
        pipelined_rendering::PipelinedRenderingPlugin, renderer::render_system,
        view::prepare_view_uniforms,
    },
    window::EnabledButtons,
    winit::WinitSettings,
};

const SLOW_RENDER_SCHEDULE_LOG_AFTER: Duration = Duration::from_millis(16);

use crate::{
    common::{
        DeferredRendering, EnvironmentLighting, OffAxisProjectionSettings,
        OrderIndependentTransparency, ScreenSpaceAmbientOcclusionSettings,
        ScreenSpaceReflectionsSettings,
    },
    config::get_render_configuration,
};

/// Function to run a render (or child) process
pub(crate) fn run<T: crate::TephriteApp>() -> AppExit {
    // Get child config
    let child_config = get_render_configuration();
    let rank = child_config.process_rank;
    let vulkan_support_client =
        crate::multiprocess::vulkan_support::init_client(&child_config.vulkan_support);

    unsafe {
        // Set process environment before Bevy's render stack has a chance to
        // initialize Vulkan/wgpu.
        if let Some(display) = &child_config.display_name {
            std::env::set_var("DISPLAY", display);
        }
    }

    let mut app = App::new();

    let mut window = Window {
        present_mode: bevy::window::PresentMode::AutoNoVsync,
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

    app.insert_resource(WinitSettings::continuous());
    app.insert_resource(RenderScheduleTiming::new(rank));

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
            .set(AssetPlugin {
                unapproved_path_mode: bevy::asset::UnapprovedPathMode::Allow,
                ..default()
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

    if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
        render_app.insert_resource(RenderSubAppTiming::new(rank));
        render_app.add_systems(First, render_sub_timing_first);
        render_app.add_systems(ExtractSchedule, render_sub_timing_extract);
        render_app.add_systems(
            Render,
            (
                render_sub_timing_render_start.before(RenderSystems::ExtractCommands),
                render_sub_timing_after_extract_commands.after(RenderSystems::ExtractCommands),
                render_sub_timing_after_prepare_assets.after(RenderSystems::PrepareAssets),
                render_sub_timing_after_prepare_meshes.after(RenderSystems::PrepareMeshes),
                render_sub_timing_after_create_views.after(RenderSystems::CreateViews),
                render_sub_timing_after_specialize.after(RenderSystems::Specialize),
                render_sub_timing_after_prepare_views.after(RenderSystems::PrepareViews),
                render_sub_timing_after_queue.after(RenderSystems::Queue),
                render_sub_timing_after_phase_sort.after(RenderSystems::PhaseSort),
                render_sub_timing_before_prepare_resources.before(RenderSystems::PrepareResources),
                render_sub_timing_after_core_3d_depth_textures
                    .in_set(RenderSystems::PrepareResources)
                    .after(prepare_core_3d_depth_textures),
                render_sub_timing_after_prepass_textures
                    .in_set(RenderSystems::PrepareResources)
                    .after(prepare_prepass_textures),
                render_sub_timing_after_view_uniforms
                    .in_set(RenderSystems::PrepareResources)
                    .after(prepare_view_uniforms),
                render_sub_timing_before_prepare_fog
                    .in_set(RenderSystems::PrepareResources)
                    .before(prepare_fog),
                render_sub_timing_after_prepare_fog
                    .in_set(RenderSystems::PrepareResources)
                    .after(prepare_fog),
                render_sub_timing_after_clear_bin_unpacking_buffers
                    .in_set(RenderSystems::PrepareResources)
                    .after(clear_bin_unpacking_buffers),
                render_sub_timing_before_cpu_clustering
                    .in_set(RenderSystems::PrepareResources)
                    .before(prepare_clusters_for_cpu_clustering),
                render_sub_timing_after_cpu_clustering
                    .in_set(RenderSystems::PrepareResources)
                    .after(prepare_clusters_for_cpu_clustering),
            ),
        );
        render_app.add_systems(
            Render,
            (
                render_sub_timing_after_prepare_resources.after(RenderSystems::PrepareResources),
                render_sub_timing_after_prepare_batch_phases
                    .after(RenderSystems::PrepareResourcesBatchPhases),
                render_sub_timing_after_prepare_write_phase_buffers
                    .after(RenderSystems::PrepareResourcesWritePhaseBuffers),
                render_sub_timing_after_prepare_collect_phase_buffers
                    .after(RenderSystems::PrepareResourcesCollectPhaseBuffers),
                render_sub_timing_after_prepare_flush.after(RenderSystems::PrepareResourcesFlush),
                render_sub_timing_after_prepare_bind_groups.after(RenderSystems::PrepareBindGroups),
                render_sub_timing_after_prepare.after(RenderSystems::Prepare),
                render_sub_timing_before_render
                    .in_set(RenderSystems::Render)
                    .before(render_system),
                render_sub_timing_after_render
                    .in_set(RenderSystems::Render)
                    .after(render_system),
                render_sub_timing_render_end.after(RenderSystems::PostCleanup),
            ),
        );
    }

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

    app.add_systems(First, render_timing_first);
    app.add_systems(PreUpdate, render_timing_pre_update);
    app.add_systems(Update, render_timing_update);
    app.add_systems(PostUpdate, render_timing_post_update);
    app.add_systems(Last, render_timing_last);

    app.add_systems(PreStartup, setup);

    app.add_systems(Update, env_change_watch);
    app.add_systems(Update, oit_resource_watch);
    app.add_systems(Update, deferred_rendering_watch);
    app.add_systems(Update, ssao_resource_watch);
    app.add_systems(Update, ssr_resource_watch);

    // Materials
    app.add_plugins(crate::material::builtin_materials_plugin);

    crate::apply_tephrite_config::<T>(&mut app, true);

    // Add in replication components
    app.add_plugins(crate::replication::reader::ReplicationReaderPlugin);

    debug!("{rank}: Render replication ready...");

    // exec
    let result = app.run();

    debug!("{rank}: Stopping renderer...");
    drop(vulkan_support_client);

    result
}

#[derive(Resource)]
struct RenderScheduleTiming {
    rank: u32,
    last_first: Option<Instant>,
    last_marker: Option<(Instant, &'static str)>,
}

impl RenderScheduleTiming {
    fn new(rank: u32) -> Self {
        Self {
            rank,
            last_first: None,
            last_marker: None,
        }
    }

    fn mark(&mut self, marker: &'static str) {
        let now = Instant::now();

        if marker == "First" {
            if let Some(last_first) = self.last_first {
                let elapsed = now.duration_since(last_first);
                if elapsed >= SLOW_RENDER_SCHEDULE_LOG_AFTER {
                    eprintln!(
                        "[teph-sync] render rank {} First-to-First gap took {:.3} ms",
                        self.rank,
                        elapsed.as_secs_f64() * 1000.0
                    );
                }
            }
            self.last_first = Some(now);
        }

        if let Some((last, last_marker)) = self.last_marker {
            let elapsed = now.duration_since(last);
            if elapsed >= SLOW_RENDER_SCHEDULE_LOG_AFTER {
                eprintln!(
                    "[teph-sync] render rank {} {} -> {} took {:.3} ms",
                    self.rank,
                    last_marker,
                    marker,
                    elapsed.as_secs_f64() * 1000.0
                );
            }
        }

        self.last_marker = Some((now, marker));
    }
}

fn render_timing_first(mut timing: ResMut<RenderScheduleTiming>) {
    timing.mark("First");
}

fn render_timing_pre_update(mut timing: ResMut<RenderScheduleTiming>) {
    timing.mark("PreUpdate");
}

fn render_timing_update(mut timing: ResMut<RenderScheduleTiming>) {
    timing.mark("Update");
}

fn render_timing_post_update(mut timing: ResMut<RenderScheduleTiming>) {
    timing.mark("PostUpdate");
}

fn render_timing_last(mut timing: ResMut<RenderScheduleTiming>) {
    timing.mark("Last");
}

#[derive(Resource)]
struct RenderSubAppTiming {
    rank: u32,
    pid: u32,
    last_marker: Option<(Instant, &'static str)>,
}

impl RenderSubAppTiming {
    fn new(rank: u32) -> Self {
        Self {
            rank,
            pid: std::process::id(),
            last_marker: None,
        }
    }

    fn mark(&mut self, marker: &'static str) {
        let now = Instant::now();

        if let Some((last, last_marker)) = self.last_marker {
            let elapsed = now.duration_since(last);
            if elapsed >= SLOW_RENDER_SCHEDULE_LOG_AFTER {
                eprintln!(
                    "[teph-sync] render rank {} pid={} subapp {} -> {} took {:.3} ms",
                    self.rank,
                    self.pid,
                    last_marker,
                    marker,
                    elapsed.as_secs_f64() * 1000.0
                );
            }
        }

        self.last_marker = Some((now, marker));
    }
}

fn render_sub_timing_first(mut timing: ResMut<RenderSubAppTiming>) {
    timing.mark("RenderFirst");
}

fn render_sub_timing_extract(mut timing: ResMut<RenderSubAppTiming>) {
    timing.mark("ExtractSchedule");
}

fn render_sub_timing_render_start(mut timing: ResMut<RenderSubAppTiming>) {
    timing.mark("RenderStart");
}

fn render_sub_timing_after_extract_commands(mut timing: ResMut<RenderSubAppTiming>) {
    timing.mark("AfterExtractCommands");
}

fn render_sub_timing_after_prepare_assets(mut timing: ResMut<RenderSubAppTiming>) {
    timing.mark("AfterPrepareAssets");
}

fn render_sub_timing_after_prepare_meshes(mut timing: ResMut<RenderSubAppTiming>) {
    timing.mark("AfterPrepareMeshes");
}

fn render_sub_timing_after_create_views(mut timing: ResMut<RenderSubAppTiming>) {
    timing.mark("AfterCreateViews");
}

fn render_sub_timing_after_specialize(mut timing: ResMut<RenderSubAppTiming>) {
    timing.mark("AfterSpecialize");
}

fn render_sub_timing_after_prepare_views(mut timing: ResMut<RenderSubAppTiming>) {
    timing.mark("AfterPrepareViews");
}

fn render_sub_timing_after_queue(mut timing: ResMut<RenderSubAppTiming>) {
    timing.mark("AfterQueue");
}

fn render_sub_timing_after_phase_sort(mut timing: ResMut<RenderSubAppTiming>) {
    timing.mark("AfterPhaseSort");
}

fn render_sub_timing_before_prepare_resources(mut timing: ResMut<RenderSubAppTiming>) {
    timing.mark("BeforePrepareResources");
}

fn render_sub_timing_after_core_3d_depth_textures(mut timing: ResMut<RenderSubAppTiming>) {
    timing.mark("AfterCore3dDepthTextures");
}

fn render_sub_timing_after_prepass_textures(mut timing: ResMut<RenderSubAppTiming>) {
    timing.mark("AfterPrepassTextures");
}

fn render_sub_timing_after_view_uniforms(mut timing: ResMut<RenderSubAppTiming>) {
    timing.mark("AfterViewUniforms");
}

fn render_sub_timing_before_prepare_fog(mut timing: ResMut<RenderSubAppTiming>) {
    timing.mark("BeforePrepareFog");
}

fn render_sub_timing_after_prepare_fog(mut timing: ResMut<RenderSubAppTiming>) {
    timing.mark("AfterPrepareFog");
}

fn render_sub_timing_after_clear_bin_unpacking_buffers(mut timing: ResMut<RenderSubAppTiming>) {
    timing.mark("AfterClearBinUnpackingBuffers");
}

fn render_sub_timing_before_cpu_clustering(mut timing: ResMut<RenderSubAppTiming>) {
    timing.mark("BeforeCpuClustering");
}

fn render_sub_timing_after_cpu_clustering(mut timing: ResMut<RenderSubAppTiming>) {
    timing.mark("AfterCpuClustering");
}

fn render_sub_timing_after_prepare_resources(mut timing: ResMut<RenderSubAppTiming>) {
    timing.mark("AfterPrepareResources");
}

fn render_sub_timing_after_prepare_batch_phases(mut timing: ResMut<RenderSubAppTiming>) {
    timing.mark("AfterPrepareBatchPhases");
}

fn render_sub_timing_after_prepare_write_phase_buffers(mut timing: ResMut<RenderSubAppTiming>) {
    timing.mark("AfterPrepareWritePhaseBuffers");
}

fn render_sub_timing_after_prepare_collect_phase_buffers(mut timing: ResMut<RenderSubAppTiming>) {
    timing.mark("AfterPrepareCollectPhaseBuffers");
}

fn render_sub_timing_after_prepare_flush(mut timing: ResMut<RenderSubAppTiming>) {
    timing.mark("AfterPrepareFlush");
}

fn render_sub_timing_after_prepare_bind_groups(mut timing: ResMut<RenderSubAppTiming>) {
    timing.mark("AfterPrepareBindGroups");
}

fn render_sub_timing_after_prepare(mut timing: ResMut<RenderSubAppTiming>) {
    timing.mark("AfterPrepare");
}

fn render_sub_timing_before_render(mut timing: ResMut<RenderSubAppTiming>) {
    timing.mark("BeforeRenderSystem");
}

fn render_sub_timing_after_render(mut timing: ResMut<RenderSubAppTiming>) {
    timing.mark("AfterRenderSystem");
}

fn render_sub_timing_render_end(mut timing: ResMut<RenderSubAppTiming>) {
    timing.mark("RenderEnd");
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
            Camera {
                is_active: true,
                order: 0,
                ..Default::default()
            },
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

    // spawn 2d text camera

    // WE MUST have the same camera settings. Different settings means different rendertarget request, thus black screen.

    commands.spawn((
        Camera {
            is_active: true,
            order: 1,
            clear_color: ClearColorConfig::None,
            ..Default::default()
        },
        Camera2d,
        Msaa::Off,
        Hdr,
        RenderLayers::layer(1),
    ));
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
                image: Some(env.specular.clone()),
                brightness: env.intensity,
                ..Default::default()
            });
        }
    }
}

fn oit_resource_watch(
    oit: Option<Res<OrderIndependentTransparency>>,
    mut cam_q: Query<Entity, With<Camera3d>>,
    mut commands: Commands,
) {
    let Some(oit) = oit else {
        return;
    };

    if !oit.is_changed() {
        return;
    }

    let oit: &OrderIndependentTransparency = &oit;

    for cam in cam_q.iter_mut() {
        let mut ec = commands.entity(cam);
        ec.insert(OrderIndependentTransparencySettings {
            sorted_fragment_max_count: oit.sorted_fragment_max_count,
            fragments_per_pixel_average: oit.fragments_per_pixel_average,
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
            min_perceptual_roughness: ssr.min_perceptual_roughness.clone(),
            max_perceptual_roughness: ssr.max_perceptual_roughness.clone(),
            thickness: ssr.thickness,
            linear_steps: ssr.linear_steps,
            linear_march_exponent: ssr.linear_march_exponent,
            edge_fadeout: ssr.edge_fadeout.clone(),
            bisection_steps: ssr.bisection_steps,
            use_secant: ssr.use_secant,
        });
    }

    *was_enabled = true;
}
