use crate::multiprocess::shared_mem::MPCommunicator;

pub struct TranscriptReader {
    multiprocess_comm: MPCommunicator,
}

impl TranscriptReader {
    pub fn new() -> Self {
        let state = MPCommunicator::attach();

        Self {
            multiprocess_comm: state,
        }
    }

    pub fn get_slice(&self) -> &[u8] {
        self.multiprocess_comm.data_slice()
    }

    pub fn barrier(&self) {
        self.multiprocess_comm.barrier()
    }
}
