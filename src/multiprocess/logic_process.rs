use std::process::Child;

use bevy::{app::App, prelude::*};

use crate::{
    common::{Head, TephExit},
    config::{InteractorType, get_configuration},
    input::{Interactor, InteractorState},
    multiprocess::app::make_common_app,
    serialize::transcript_writer::TranscriptWriterResource,
    vrpn::VRPNObject,
};

const SCENE_DEBUG_ENV: &str = "TEPH_SCENE_DEBUG";

pub(crate) fn setup() -> App {
    // session
    let session_id = crate::multiprocess::generate_session_id();

    crate::multiprocess::install_session_id(&session_id);

    // build bevy application
    let mut app = make_common_app();

    // only now are logs enabled!

    let config = get_configuration();
    let use_simulator_mode = config.child_count() == 0;

    // Having zero children makes no sense
    let child_count = config.child_count().max(1);

    app.add_plugins(crate::input::InputPlugin);

    app.add_plugins(crate::vrpn::VRPNPlugin);

    let brp_port = std::env::var(SCENE_DEBUG_ENV)
        .is_ok()
        .then_some(bevy::remote::http::DEFAULT_PORT);
    app.add_plugins(crate::remote_control::RemoteControlPlugin {
        brp_port,
        ..Default::default()
    });

    app.add_plugins(crate::material::builtin_materials_plugin);
    app.add_plugins(crate::environment::environment_plugin);

    // this adds AABB calc, and visibility
    app.add_plugins(bevy::camera::visibility::VisibilityPlugin);

    app.add_systems(Startup, setup_tracked_head);

    if use_simulator_mode || std::env::var("TEPH_FORCE_SIMULATOR").is_ok() {
        app.add_plugins(crate::simulator::SimulatorPlugin);
    }

    app.add_observer(on_exit);

    app.insert_resource(LogicLaunchState {
        session_id,
        child_count,
    });

    app
}

#[derive(Debug, Resource)]
struct LogicLaunchState {
    session_id: crate::multiprocess::SessionID,
    child_count: u32,
}

pub(crate) fn finish_setup(app: &mut App) {
    let launch = app.world().resource::<LogicLaunchState>();
    let session_id = launch.session_id.clone();
    let child_count = launch.child_count;

    app.add_plugins(crate::replication::ReplicationWriterPlugin::new(
        child_count,
    ));

    let config = get_configuration();
    let vulkan_support_host =
        crate::multiprocess::vulkan_support::init_host(&config.vulkan_support, child_count);

    let current_exe = std::env::current_exe()
        .expect("determine current executable")
        .to_owned();

    debug!("Using session id as {session_id:?}");

    info!("Launching {child_count} render processes");

    let children: Vec<_> = (0..child_count)
        .map(|i| {
            let current_exe = current_exe.clone();
            let session_clone = session_id.clone();

            let mut command = std::process::Command::new(current_exe);
            crate::multiprocess::install_ids(&mut command, &session_clone, i);
            crate::multiprocess::vulkan_support::install_child_env(
                &mut command,
                &config.vulkan_support,
                vulkan_support_host.as_ref(),
                config.render_configuration(i).card_index,
            );

            command.spawn().expect("launching render process")
        })
        .collect();

    app.insert_resource(ChildProcessResource {
        children,
        vulkan_support_host,
    });
}

#[derive(Debug, Resource)]
struct ChildProcessResource {
    children: Vec<std::process::Child>,
    vulkan_support_host: Option<crate::multiprocess::vulkan_support::VulkanSupportHost>,
}

fn setup_tracked_head(mut commands: Commands) {
    debug!("Setup tracked head");

    let config = get_configuration();

    if let Some(h) = config.vrpn.head.clone() {
        commands.spawn((
            Transform::default(),
            Head,
            VRPNObject(vec![h]),
            Name::new("Head"),
        ));
    } else {
        commands.spawn((Transform::default(), Head, Name::new("Head")));
    }

    // TODO we should reconsider our config with a dedicated sim mode flag or something.
    // but we need to be smart; if no config, auto sim mode?

    let interactor = config.interactor();
    let interactor_type = match interactor.as_ref().map(|x| x.ty).unwrap_or_default() {
        InteractorType::Controller => Interactor::Controller,
        InteractorType::Flystick => Interactor::Flystick,
    };

    let id = commands
        .spawn((
            Transform::default(),
            interactor_type,
            InteractorState::default(),
            Name::new("Joystick"),
            InheritedVisibility::default(),
        ))
        .id();

    if let Some(interactor) = interactor {
        commands.entity(id).insert(VRPNObject(interactor.addresses));
    }
}

fn destroy_child_process(mut child: Child) {
    let now = std::time::Instant::now();

    loop {
        let check = child.try_wait();

        match check {
            Ok(Some(_)) => {
                // exited
                return;
            }
            Ok(None) => {
                // still running
                let duration = now - std::time::Instant::now();

                if duration.as_secs_f32() > 10.0 {
                    warn!("Child process {} timeout, killing", child.id());
                    let _ = child.kill();
                    return;
                }
            }
            Err(_) => {
                warn!("Child process {} IO error, killing", child.id());
                // unable to wait. issue a kill and move on
                // given that we are operating in a weird world where we cant tell if the
                // child is alive or not, do not check the result
                let _ = child.kill();

                return;
            }
        }
    }
}

pub(crate) fn cleanup(mut app: App) -> Option<()> {
    debug!("Cleaning up");

    let res = app.world_mut().remove_resource::<ChildProcessResource>()?;

    for child in res.children {
        destroy_child_process(child);
    }

    drop(res.vulkan_support_host);

    Some(())
}

pub(crate) fn on_exit(
    _on: On<TephExit>,
    mut writer: NonSendMut<TranscriptWriterResource>,
    mut exit_writer: MessageWriter<AppExit>,
) {
    writer.shutdown();
    exit_writer.write(AppExit::Success);
}
