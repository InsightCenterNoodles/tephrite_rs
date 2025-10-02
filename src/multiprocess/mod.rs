//! Functionality for multiple processes

use std::hash::{Hash, Hasher};

use bevy::log::debug;

pub(crate) mod app;
pub mod logic_process;
pub mod render_process;
pub mod shared_buffer;
pub mod shared_mem;

/// Environment variable to determine if a process is a render process
const CHILD_ENV_VARIABLE: &str = "TEPHRITE_CHILD_PROCESS";

/// Environment variable to mark which process group we are in
const SESSION_ENV_VARIABLE: &str = "TEPHRITE_PROCESS_GROUP";

// Name of shared memory region
const SHMEM_NAME_PREFIX: &str = "TEPH_";

// Size of shared memory region. Not resizable at this time.
// Since we are sending large textures, meshes, and huge instance lists, this is a 'safe' bound.
// Previous versions would break at 2 gigs. In the future, we should shard this.
pub const SHMEM_DEFAULT_BLOCK_SIZE: u64 = 2u64.pow(32);

// Size of shared memory region for testing purposes.
pub const SHMEM_TESTING_BLOCK_SIZE: u64 = 2u64.pow(17);

/// Ask if this child was launched by a logic process
pub fn is_child_process() -> bool {
    std::env::var(CHILD_ENV_VARIABLE)
        .map(|x| !x.is_empty())
        .unwrap_or_default()
}

/// If this is a child process, return the rank of the child.
///
/// Panics if this is not a child process, or has an invalid ID.
pub fn child_process_id() -> u32 {
    std::env::var(CHILD_ENV_VARIABLE)
        .expect("obtaining child id from env variable")
        .parse()
        .expect("parsing child id from env string")
}

#[derive(Debug, Clone)]
pub struct SessionID(String);

impl SessionID {
    pub fn as_str(&self) -> &str {
        return &self.0;
    }
}

/// What is the session for this child process?
///
/// Panics if this is not a child process
pub fn session_id() -> SessionID {
    SessionID(std::env::var(SESSION_ENV_VARIABLE).expect("child is missing session information"))
}

/// Create a new render session id
pub fn generate_session_id() -> SessionID {
    SessionID(bevy::asset::uuid::Uuid::new_v4().to_string())
}

/// Add child ids to a process
pub fn install_ids<'a>(
    command: &'a mut std::process::Command,
    session_id: &SessionID,
    child_id: u32,
) -> &'a mut std::process::Command {
    command
        .env(SESSION_ENV_VARIABLE, &session_id.0)
        .env(CHILD_ENV_VARIABLE, child_id.to_string())
}

/// Install session id for testing
pub fn install_session_id(session_id: &SessionID) {
    unsafe {
        std::env::set_var(SESSION_ENV_VARIABLE, &session_id.0);
    }
}

pub fn get_shared_mem_block_name() -> String {
    // This is a UUID string under the hood
    let session_id = session_id();

    let session_id: String = session_id
        .as_str()
        .chars()
        .skip_while(|x| !x.is_ascii_alphanumeric())
        .collect();

    let mut hasher = std::hash::DefaultHasher::new();
    session_id.hash(&mut hasher);
    let session_id = hasher.finish();

    let formatted = format!("/{SHMEM_NAME_PREFIX}{}", session_id);

    debug!("KEY: {formatted}");
    formatted
}
