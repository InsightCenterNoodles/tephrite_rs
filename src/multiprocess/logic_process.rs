use bevy::{app::App, prelude::*};

use crate::{
    common::Head, config::get_logic_configuration, multiprocess::app::make_common_app,
    prelude::Replicated, vrpn::VRPNLink,
};

pub(crate) fn setup() -> App {
    // set up MP state. In the future, this will be pulled from the config. but that is not finalized yet.
    //let child_count = 1;
    //let child_count = 12;
    let child_count = get_logic_configuration().child_count;

    // build bevy application
    let mut app = make_common_app();

    app.add_plugins(crate::vrpn::VRPNPlugin);

    app.add_plugins(crate::replication::ReplicationWriterPlugin::new(
        child_count,
    ));

    app.add_systems(Startup, setup_tracked_head);

    // set up children

    let current_exe = std::env::current_exe()
        .expect("determine current executable")
        .to_owned();

    // session
    let session_id = crate::multiprocess::generate_session_id();

    let _child_list: Vec<_> = (0..child_count)
        .map(|i| {
            let current_exe = current_exe.clone();
            let session_clone = session_id.clone();
            std::thread::spawn(move || {
                info!("Spawning {i}...");

                let mut command = std::process::Command::new(current_exe);
                crate::multiprocess::install_ids(&mut command, &session_clone, i);

                let mut command = command.spawn().expect("launching render process");

                let status = command.wait().unwrap();
                info!("Completed {i} {status}");
            })
        })
        .collect();

    app
}

fn setup_tracked_head(mut commands: Commands) {
    println!("Setup tracked head");

    let h = get_logic_configuration().vrpn_config.head.clone();

    commands.spawn((Replicated, Transform::default(), Head, VRPNLink::new(h)));
}
