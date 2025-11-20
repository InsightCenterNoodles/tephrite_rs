mod assets;
pub(crate) mod breplicate;
pub(crate) mod components;
pub(crate) mod convert;
mod ibl;
pub(crate) mod lighting;
pub(crate) mod mesh_mat_bind;
pub(crate) mod resources;
pub(crate) mod sets;
pub(crate) mod transform;

use bevy::prelude::*;

use crate::common::Head;

use super::backfill;
use super::backfill::ffi as bffi;

pub struct BackfillPlugin;

impl Plugin for BackfillPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(setup_session);

        app.add_plugins(sets::PipelineOrderPlugin);
        app.add_plugins(breplicate::ReplicationPlugin);
        app.add_plugins(mesh_mat_bind::RenderableBindingPlugin);
        app.add_plugins(transform::TransformPlugin);
        app.add_plugins(lighting::LightBindingPlugin);
        app.add_plugins(ibl::EnvironmentLightPlugin);

        //app.add_systems(FixedLast, run_frame);
        app.add_systems(Last, (run_frame, teardown).chain());
    }
}

fn setup_session(app: &mut App) {
    debug!("Session setup");

    let session = {
        let child_config = crate::config::get_render_configuration();

        let config = backfill::config().unwrap();

        let window_name = format!("Teph Window {}", child_config.process_rank);

        backfill::config_title(&config, &window_name);
        backfill::config_screen(
            &config,
            child_config.resolution.x as i32,
            child_config.resolution.y as i32,
        );

        // let plane = bffi::FScreenPlane {
        //     lower_left: [-2.5, 0.0, -1.768],
        //     lower_right: [2.5, 0.0, -1.768],
        //     upper_right: [2.5, 2.5, -1.768],
        // };

        let plane = bffi::FScreenPlane {
            lower_left: child_config.display_physical.lower_left.into(),
            lower_right: child_config.display_physical.lower_right.into(),
            upper_right: child_config.display_physical.upper_right.into(),
        };

        if child_config.debug_renderer {
            unsafe {
                bffi::fconfig_set_log_debug(config.as_ptr(), 1);
            }
        }

        if child_config.use_offaxis {
            unsafe {
                bffi::fconfig_set_offaxis_plane(config.as_ptr(), &plane);
                bffi::fconfig_set_stereo_eye(
                    config.as_ptr(),
                    if child_config.is_right {
                        bffi::FEye_EYE_RIGHT
                    } else {
                        bffi::FEye_EYE_LEFT
                    },
                );
            }
        }

        unsafe {
            if child_config.fullscreen {
                bffi::fconfig_set_fullscreen(config.as_ptr(), 1);
            }
        };

        if let Some(api) = &child_config.render_api {
            let api = match api.as_str() {
                "opengl" => bffi::FRenderer_R_OPENGL,
                "metal" => bffi::FRenderer_R_METAL,
                "vulkan" => bffi::FRenderer_R_VULKAN,
                x => panic!("Unknown graphics API {x}"),
            };
            unsafe { bffi::fconfig_set_renderer(config.as_ptr(), api) };
        }

        if let Some(d) = &child_config.display_name {
            debug!("Setting display to {d}");
            backfill::config_display(&config, d);
        }

        if let Some(index) = &child_config.card_index {
            debug!("Setting card index to {index}");
            unsafe { bffi::fconfig_set_device(config.as_ptr(), (*index) as i32) };
        }

        backfill::session(&config).unwrap()
    };

    unsafe {
        bffi::fs_set_postprocess(session.as_ptr(), 1);
        bffi::fs_set_skybox_color(
            session.as_ptr(),
            bffi::FColor {
                r: 0.1,
                g: 0.125,
                b: 0.25,
                a: 1.0,
            },
        );
    }

    //setup_lights(&session);
    //setup_ibl(&session);

    // match setup_ibl(&session) {
    //     Ok(x) => {
    //         app.insert_non_send_resource(x);
    //     }
    //     Err(x) => {
    //         error!("Unable to load IBL {x}");
    //     }
    // }

    app.insert_non_send_resource(resources::Session(session));

    println!("Session setup done");
}

fn run_frame(
    session: NonSend<resources::Session>,
    mut writer: MessageWriter<AppExit>,
    head_ent: Query<&Transform, With<Head>>,
) {
    // update head

    if let Ok(x) = head_ent.single() {
        backfill::update_head(&session.0, x.translation, x.rotation);
        //println!("{} HEAD {}", std::process::id(), x.translation);
    }

    let should_exit = backfill::frame(&session.0);

    //println!("{} FRAME", std::process::id());

    if !should_exit {
        info!("Exiting...");
        writer.write(AppExit::Success);
    }
}

fn teardown(
    reader: MessageReader<AppExit>,
    query: Query<Entity, With<components::BEntity>>,
    mut cache: NonSendMut<assets::AssetCache>,
    mut commands: Commands,
) {
    if reader.len() > 0 {
        info!("Exiting...");

        info!("Clearing replicated entities");
        for e in &query {
            if let Ok(mut x) = commands.get_entity(e) {
                x.despawn();
            }
        }

        info!("Clearing replicated resources");
        cache.clear();
    }
}
