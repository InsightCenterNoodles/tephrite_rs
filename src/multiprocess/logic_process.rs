use bevy::{app::App, prelude::*};

use crate::{
    common::Head, config::get_child_configuration, multiprocess::app::make_common_app,
    prelude::Replicated, vrpn::VRPNLink,
};

pub(crate) fn setup() -> App {
    // set up MP state
    //let child_count = 1;
    //let child_count = 12;
    let child_count = 2;

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

    let _child_list: Vec<_> = (0..child_count)
        .map(|i| {
            let current_exe = current_exe.clone();
            std::thread::spawn(move || {
                println!("Spawning {i}...");
                let mut c = std::process::Command::new(current_exe)
                    .env(crate::multiprocess::CHILD_ENV_VARIABLE, i.to_string())
                    .spawn()
                    .expect("launching render process");

                let status = c.wait().unwrap();
                println!("Completed {i} {status}");
            })
        })
        .collect();

    println!("Done spawning");

    app
}

fn setup_tracked_head(mut commands: Commands) {
    println!("Setup tracked head");

    let h = get_child_configuration().vrpn_config.head.clone();

    commands.spawn((Replicated, Transform::default(), Head, VRPNLink::new(h)));
}
