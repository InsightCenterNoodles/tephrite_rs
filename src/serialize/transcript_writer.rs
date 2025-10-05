//! Writer side for binary transcripts over the shared memory ring buffer.
//!
//! The writer prepares a frame buffer, encodes messages via the `ByteSink`
//! implementation on `TranscriptWriteStateResource`, and then commits the
//! frame atomically for readers to consume.
use crate::{
    multiprocess::{
        self,
        shared_buffer::{PartialWriteState, Producer},
    },
    serialize::ByteSink,
};

/// Resource that creates and commits serialized frames into shared memory.
pub struct TranscriptWriterResource {
    multiprocess_comm: Producer,
}

impl TranscriptWriterResource {
    /// Create a writer with `child_count` expected consumers.
    pub fn new(child_count: u32) -> Self {
        // let state = MPCommunicator::create(process_count);

        // let (sptr, len) = unsafe { state.parts() };
        Self {
            multiprocess_comm: Producer::new(
                &multiprocess::get_shared_mem_block_name(),
                3,
                multiprocess::SHMEM_DEFAULT_BLOCK_SIZE as usize,
                child_count as usize,
            )
            .unwrap(),
        }
    }

    /// Start a new frame and obtain a writer for it.
    pub fn prepare(&mut self) -> TranscriptWriteStateResource {
        TranscriptWriteStateResource {
            state: self.multiprocess_comm.prepare(),
            pos: 0,
        }
    }

    /// Commit a previously prepared frame; it becomes visible to readers.
    pub fn commit(&mut self, state: TranscriptWriteStateResource) {
        self.multiprocess_comm.commit(state.state);
    }
}

/// ByteSink over a shared memory frame under construction.
pub struct TranscriptWriteStateResource {
    state: PartialWriteState,
    pos: usize,
}

impl ByteSink for TranscriptWriteStateResource {
    #[inline(always)]
    fn put_bytes(&mut self, src: &[u8]) {
        //println!("PUT BYTES pos: {}, {}", self.pos, src.len());
        let Some(end) = self.pos.checked_add(src.len()) else {
            panic!("TranscriptWriter out of bounds")
        };

        let slice = self.state.slice();

        if end > slice.len() {
            panic!("TranscriptWriter out of bounds")
        }
        // Safety: we just bounds-checked
        unsafe {
            std::ptr::copy_nonoverlapping(
                src.as_ptr(),
                slice.as_mut_ptr().add(self.pos),
                src.len(),
            );
        }
        self.pos = end;
        //println!("PUT BYTES DONE {}", self.pos);
    }
}
