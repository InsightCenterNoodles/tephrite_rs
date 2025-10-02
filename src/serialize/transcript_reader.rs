use crate::multiprocess::{child_process_id, get_shared_mem_block_name, shared_buffer::Consumer};

pub struct TranscriptReaderResource {
    multiprocess_comm: Consumer,
}

impl TranscriptReaderResource {
    pub fn new() -> Self {
        let state =
            Consumer::new(&get_shared_mem_block_name(), child_process_id() as usize).unwrap();

        Self {
            multiprocess_comm: state,
        }
    }

    pub fn consume_next<F>(&mut self, f: F)
    where
        F: FnMut(u64, u32, &[u8]),
    {
        self.multiprocess_comm.consume_next(f);
    }
}
