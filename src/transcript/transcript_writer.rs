use std::io::Write;

//use crate::multiprocess::shared_mem::MPCommunicator;

pub struct TranscriptWriter {
    //multiprocess_comm: MPCommunicator,
    start: PtrWrapper,
    avail: isize,
}

impl TranscriptWriter {
    pub fn new(process_count: u32) -> Self {
        todo!();
        // let state = MPCommunicator::create(process_count);

        // let (sptr, len) = unsafe { state.parts() };
        // Self {
        //     multiprocess_comm: state,
        //     start: PtrWrapper(sptr as *mut u8),
        //     avail: len as isize,
        // }
    }

    pub fn reset(&mut self) {
        todo!();
        // let (sptr, len) = unsafe { self.multiprocess_comm.parts() };
        // self.start = PtrWrapper(sptr as *mut u8);
        // self.avail = len as isize;
    }

    pub fn barrier(&self) {
        //self.multiprocess_comm.barrier()
        todo!();
    }
}

impl Write for TranscriptWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        // rust does not yet have non-nightly intrinsics

        let incoming_size = buf.len();

        self.avail -= incoming_size as isize;

        if self.avail <= 0 {
            panic!("Too much data this frame!");
        }

        unsafe {
            std::ptr::copy_nonoverlapping(buf.as_ptr(), self.start.0, incoming_size);
            self.start.0 = self.start.0.byte_add(incoming_size);
        }

        Ok(incoming_size)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

// Wrap a pointer to bless it for multithreading
#[derive(Debug)]
#[repr(transparent)]
struct PtrWrapper(*mut u8);

unsafe impl Send for PtrWrapper {}
unsafe impl Sync for PtrWrapper {}
