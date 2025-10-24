use std::process::Child;

use bevy::{
    app::{App, TerminalCtrlCHandlerPlugin},
    prelude::*,
};

use crate::{
    common::Head,
    config::get_logic_configuration,
    multiprocess::app::make_common_app,
    prelude::Replicated,
    replication::instruction::{Halt, ServerInstruction},
    serialize::{
        FastWrite,
        transcript_writer::{TranscriptWriteStateResource, TranscriptWriterResource},
    },
    vrpn::VRPNLink,
};

pub(crate) fn setup() -> App {
    // session
    let session_id = crate::multiprocess::generate_session_id();

    crate::multiprocess::install_session_id(&session_id);

    // build bevy application
    let mut app = make_common_app();

    app.add_plugins(TerminalCtrlCHandlerPlugin);

    // only now are logs enabled!

    // set up MP state. In the future, this will be pulled from the config. but that is not finalized yet.
    //let child_count = 1;
    //let child_count = 12;
    let child_count = get_logic_configuration().child_count;

    app.add_plugins(crate::vrpn::VRPNPlugin);

    app.add_plugins(crate::replication::ReplicationWriterPlugin::new(
        child_count,
    ));

    app.add_systems(Startup, setup_tracked_head);

    // set up children

    let current_exe = std::env::current_exe()
        .expect("determine current executable")
        .to_owned();

    debug!("Using session id as {session_id:?}");

    info!("Launching {child_count} render processes");

    let children: Vec<_> = (0..child_count)
        .map(|i| {
            let current_exe = current_exe.clone();
            let session_clone = session_id.clone();
            info!("Spawning {i}...");

            let mut command = std::process::Command::new(current_exe);
            crate::multiprocess::install_ids(&mut command, &session_clone, i);

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

    let h = get_logic_configuration().vrpn_config.head.clone();

    commands.spawn((Replicated, Transform::default(), Head, VRPNLink::new(h)));
}

fn send_stop(app: &mut App) -> Option<()> {
    let mut state = app
        .world_mut()
        .remove_non_send_resource::<TranscriptWriteStateResource>()?;

    unsafe { ServerInstruction::Halt(Halt).write_fast(&mut state) };

    let mut res = app
        .world_mut()
        .remove_non_send_resource::<TranscriptWriterResource>()?;

    res.commit(state);

    Some(())
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

                if duration.as_secs_f32() > 1.0 {
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

    // send stop
    // if let None = send_stop(&mut app) {
    //     warn!("Unable to send shutdown signal to child processes");
    // }

    let res = app.world_mut().remove_resource::<ChildProcessResource>()?;

    for child in res.children {
        destroy_child_process(child);
    }

    Some(())
}
