//! Reader side for binary transcripts over the shared memory ring buffer.
//!
//! The transcript is a sequence of frames written by the logic process and
//! consumed by render workers. This resource wraps the low-level
//! `multiprocess::shared_buffer::Consumer` and exposes a callback-based API to
//! consume frames without allocations.
use crate::multiprocess::{child_process_id, get_shared_mem_block_name, shared_buffer::Consumer};

/// Resource that pulls serialized frames from shared memory.
pub struct TranscriptReaderResource {
    multiprocess_comm: Consumer,
}

impl TranscriptReaderResource {
    /// Create a reader bound to the per-child shared memory segment.
    pub fn new() -> Self {
        let state =
            Consumer::new(&get_shared_mem_block_name(), child_process_id() as usize).unwrap();

        Self {
            multiprocess_comm: state,
        }
    }

    /// Consume the next available frame, invoking `f` for each message.
    ///
    /// The callback receives `(timestamp, channel_id, bytes)` for each item
    /// within the frame.
    pub fn consume_next<F>(&mut self, f: F)
    where
        F: FnMut(u64, u32, &[u8]),
    {
        self.multiprocess_comm.consume_next(f);
    }
}
