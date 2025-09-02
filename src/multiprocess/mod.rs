//! Functionality for multiple processes

pub(crate) mod app;
pub mod logic_process;
pub mod render_process;
pub(crate) mod shared_mem;

/// Command line text to determine if a process is a render process
pub(crate) const CHILD_ENV_VARIABLE: &str = "TEPHRITE_CHILD_PROCESS";

/// Ask if this child was launched by a logic process
pub fn is_child_process() -> bool {
    std::env::var(CHILD_ENV_VARIABLE).is_ok()
}

/// If this is a child process, return the rank of the child.
///
/// Panics if asked of a logic process which has no ID.
pub fn child_process_id() -> u32 {
    std::env::var(CHILD_ENV_VARIABLE)
        .expect("obtaining child id from env variable")
        .parse()
        .expect("parsing child id from env string")
}
