pub(crate) mod backfill;
pub(crate) mod backfill_link;
pub(crate) mod common;
pub(crate) mod config;
pub mod input;
pub mod multiprocess;
pub mod replication;
pub(crate) mod serialize;
pub(crate) mod vrpn;

use bevy::app::Plugin;

pub mod prelude {
    pub use super::run;
    pub use crate::common::EnvironmentLighting;
    pub use crate::common::Head;
    pub use crate::replication::components::PropagateReplication;
    pub use crate::replication::components::Replicated;

    pub use crate::input::*;
}

/// Primary entry point for your application
///
/// As this is a multiprocess application, we need to steal control from normal execution paths.
/// This function takes care of this for you; pass in a plugin that defines your application.
/// See examples for demonstrations of this approach.
///
pub fn run(user_plugin: impl Plugin) -> bevy::app::AppExit {
    if multiprocess::is_child_process() {
        multiprocess::render_process::run()
    } else {
        let mut app = multiprocess::logic_process::setup();

        app.add_plugins(user_plugin);

        let result = app.run();

        multiprocess::logic_process::cleanup(app);

        result
    }
}
