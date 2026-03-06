use std::process::Child;

use bevy::{app::App, prelude::*};

use crate::{
    common::Head, config::get_logic_configuration, input::Interactor, input::InteractorState,
    multiprocess::app::make_common_app, prelude::Replicated, vrpn::VRPNObject,
};

pub(crate) fn setup() -> App {
    // session
    let session_id = crate::multiprocess::generate_session_id();

    crate::multiprocess::install_session_id(&session_id);

    // build bevy application
    let mut app = make_common_app();

    // only now are logs enabled!

    let use_simulator_mode = get_logic_configuration().child_count == 0;

    // Having zero children makes no sense
    let child_count = get_logic_configuration().child_count.max(1);

    app.add_plugins(crate::input::InputPlugin);

    app.add_plugins(crate::vrpn::VRPNPlugin);

    app.add_plugins(crate::remote_control::RemoteControlPlugin::default());

    app.add_plugins(crate::replication::ReplicationWriterPlugin::new(
        child_count,
    ));

    app.add_plugins(crate::multiprocess::app::control_c_watcher);

    // this adds AABB calc, and visibility
    app.add_plugins(bevy::camera::visibility::VisibilityPlugin);

    app.add_systems(Startup, setup_tracked_head);

    if get_logic_configuration().vrpn_config.debug_head {
        app.add_systems(Update, debug_head);
    }

    if use_simulator_mode {
        app.add_plugins(crate::simulator::SimulatorPlugin);
    }

    // set up children

    let current_exe = std::env::current_exe()
        .expect("determine current executable")
        .to_owned();

    debug!("Using session id as {session_id:?}");

    info!("Launching {child_count} render processes");

    let install_debug_env_var = get_logic_configuration().debug_renderer;

    let children: Vec<_> = (0..child_count)
        .map(|i| {
            let current_exe = current_exe.clone();
            let session_clone = session_id.clone();
            //info!("Spawning {i}...");

            let mut command = std::process::Command::new(current_exe);
            crate::multiprocess::install_ids(&mut command, &session_clone, i);

            if install_debug_env_var {
                command.env("TEPH_DEBUG", "1");
            }

            command.spawn().expect("launching render process")
        })
        .collect();

    app.insert_resource(ChildProcessResource { children });

    app
}

#[derive(Debug, Resource)]
struct ChildProcessResource {
    children: Vec<std::process::Child>,
}

fn setup_tracked_head(mut commands: Commands) {
    debug!("Setup tracked head");

    let config = get_logic_configuration();

    if let Some(h) = config.vrpn_config.head.clone() {
        commands.spawn((Replicated, Transform::default(), Head, VRPNObject(vec![h])));
    } else {
        commands.spawn((Replicated, Transform::default(), Head));
    }

    // TODO we should reconsider our config with a dedicated sim mode flag or something.
    // but we need to be smart; if no config, auto sim mode?

    if let Some(js) = &config.vrpn_config.joystick {
        commands.spawn((
            Transform::default(),
            Interactor,
            InteractorState::default(),
            Name::new("Joystick"),
            VRPNObject(js.clone()),
        ));
    } else {
        commands.spawn((
            Transform::default(),
            Interactor,
            InteractorState::default(),
            Name::new("Joystick"),
        ));
    }
}

fn debug_head(
    mut query: Query<&mut Transform, With<Head>>,
    time: Res<Time>,
    mut local: Local<f32>,
) {
    *local += 0.5 * time.delta_secs();

    let new_head_x = (local).sin() * 2.0 - 1.0;

    let head_pos = vec3(new_head_x, 1.5, 2.0);

    for mut q in query.iter_mut() {
        q.translation = head_pos;
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

    Some(())
}
