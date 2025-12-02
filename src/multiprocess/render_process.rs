use bevy::prelude::*;

use crate::{config::get_render_configuration, multiprocess::app::make_common_app};

/// Function to run a render (or child) process
pub(crate) fn run() -> AppExit {
    let mut app = make_common_app();

    // Get child config
    let child_config = get_render_configuration();
    let rank = child_config.process_rank;

    info!("{rank}: Running render process {}", std::process::id());

    app.add_plugins(crate::backfill_link::BackfillPlugin);

    app.add_plugins(crate::multiprocess::app::control_c_catch);
    app.add_plugins(bevy::camera::visibility::VisibilityPlugin);

    // Add in replication components
    app.add_plugins(crate::replication::reader::ReplicationReaderPlugin);

    debug!("{rank}: Render replication ready...");

    // exec
    let result = app.run();

    debug!("{rank}: Stopping renderer...");

    result
}
