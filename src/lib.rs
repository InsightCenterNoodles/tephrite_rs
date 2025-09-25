pub(crate) mod backfill;
pub(crate) mod backfill_link;
pub(crate) mod common;
pub(crate) mod config;
pub mod multiprocess;
pub(crate) mod replication;
pub(crate) mod transcript;
pub(crate) mod vrpn;

use bevy::app::Plugin;

pub mod prelude {
    pub use super::run;
    pub use crate::replication::components::Replicated;
}

/// Primary entry point for your application
///
/// As this is a multiprocess application, we may need to steal control from
/// your main process. This function takes care of this for you; pass in a
/// function that acts like a main. See examples for demonstrations of this
/// approach.
///
pub fn run(user_plugin: impl Plugin) -> bevy::app::AppExit {
    if multiprocess::is_child_process() {
        multiprocess::render_process::run()
    } else {
        let mut app = multiprocess::logic_process::setup();

        app.add_plugins(user_plugin);

        app.run()
    }
}
