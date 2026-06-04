//! Strict-lockstep shared-memory publisher with per-consumer acks.
//! Works cross-process on Linux/macOS (Windows later with same atomics).
//! Hot path = Acquire/Release atomics; no global barriers.
//! Children are assumed to NOT attach late

use core::mem::size_of;
use core::sync::atomic::{
    AtomicU32, AtomicU64,
    Ordering::{AcqRel, Acquire, Relaxed, Release},
};
use std::io::Result;
use std::time::Duration;
use std::{ptr, thread};

use bevy::log::{debug, warn};

use crate::multiprocess::shared_mem::SharedMemory;

const MAX_CONSUMERS: usize = 16; // This should be enough...
const MAGIC: u64 = 0x53584C4F434B5354; // "SXLOCKST" for sanity

// Bookkeeping is stored in a region of this size at the start of the block.
const SHMEM_DATA_OFFSET: usize = 8192;

pub fn compute_shmem_allocation_size(buf_count: usize, buf_size: usize) -> usize {
    const MAX_PAGE: usize = 2u32.pow(14) as usize;
    let page_size: usize = unsafe {
        libc::sysconf(libc::_SC_PAGESIZE)
            .try_into()
            .map_err(|_| std::io::Error::other("Unable to determine page size"))
    }
    .unwrap_or(MAX_PAGE);

    SHMEM_DATA_OFFSET
        .checked_add(buf_size.checked_mul(buf_count).expect("size overflow"))
        .expect("size overflow")
        .next_multiple_of(page_size)
}

#[repr(align(128))]
struct Padded64(AtomicU64);

#[repr(C)]
pub(crate) struct ControlBlock {
    magic: AtomicU64,   // sanity check
    buf_count: u32,     // N buffers (>= 2, ideally 3+)
    buf_size: u64,      // bytes per buffer
    num_consumers: u32, // <= MAX_CONSUMERS

    // Rendezvous
    ready_count: AtomicU32, // number of consumers that joined
    started: AtomicU32,     // 0 = not started, 1 = started

    // 64 bit boundary
    shutdown: AtomicU32, // if set, begin shutdown
    _pad0: u32,

    // Publication side:
    publish_gen: AtomicU64, // monotonically increasing frame number (starts at 0)
    publish_idx: AtomicU32, // which buffer currently holds publish_gen
    _pad1: u32,

    // Consumer side general barrier:
    consumer_barrier: Barrier,

    // Per-consumer last seen/acked generation (one cache line each)
    consumer_gen: [Padded64; MAX_CONSUMERS],
    // (optional) stats/flags space you can extend later
}

impl ControlBlock {
    fn min_acked(&self, n: usize) -> u64 {
        let mut m = u64::MAX;
        for i in 0..n {
            // separate cache lines to avoid false sharing
            let g = self.consumer_gen[i].0.load(Acquire);
            if g < m {
                m = g;
            }
        }
        m
    }
}

struct Barrier {
    thread_count: u32,
    state: AtomicU32,
    generation: AtomicU64,
}

impl Barrier {
    fn init(&mut self, child_count: u32) {
        self.thread_count = child_count;
        self.state = AtomicU32::new(0);
        self.generation = AtomicU64::new(0);
    }
}

const _: () = {
    // Ensure that our data offset is large enough...
    assert!(SHMEM_DATA_OFFSET >= core::mem::size_of::<ControlBlock>());

    // keep buffers cacheline-aligned
    assert!(SHMEM_DATA_OFFSET.is_multiple_of(128));
};

// Layout:
// [ControlBlock (aligned to CACHELINE)]
// [padding to fill SHMEM_DATA_OFFSET]
// [buffer 0][buffer 1]...[buffer N-1]

/// Helper: pointer to buffer slot `idx`
#[inline]
fn buffer_ptr(data_base: *mut u8, buf_size: usize, idx: u32) -> *mut u8 {
    unsafe { data_base.byte_add(buf_size * (idx as usize)) }
}

/// Strict lockstep: producer must not publish gen (N+1) until ALL consumers have acked gen N.
///
/// Consumers MUST NOT try to map before the Producer has started!
///
/// Flow:
///   - Producer chooses a reusable slot, writes it, then publishes (idx, gen=N+1),
///     then *optionally* wakes consumers, then waits until min_acked == N+1
///     **before** proceeding (strict fence between frames).
///
///   - Consumer waits for publish_gen to change, reads publish_idx, runs the frame,
///     then stores consumer_gen[id] = new_gen (ack).
///
pub struct Producer {
    #[allow(unused)]
    shared: SharedMemory, // keep a reference to shared memory alive
    cached_ready: bool,
    cb: *mut ControlBlock,
    pub data_base: *mut u8,
    buf_size: usize,
    last_published: u64,
    start_shutdown: bool,
}

impl Producer {
    pub fn new(
        key: &str,
        num_buffers: usize,
        buffer_size: usize,
        num_consumers: usize,
    ) -> Result<Self> {
        //println!("Creating new producer: {key} {num_buffers} {buffer_size} {num_consumers}");
        if num_buffers < 2 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "buffer count must be >= 2",
            ));
        }

        if num_consumers == 0 || num_consumers >= MAX_CONSUMERS {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "number of consumers is out of bounds",
            ));
        }

        let total_size = compute_shmem_allocation_size(num_buffers, buffer_size);

        let shmem = SharedMemory::create(key, total_size)?;

        let pointer_base = unsafe { shmem.parts().0 };

        let cb = unsafe { &mut *(pointer_base as *mut ControlBlock) };

        // Zero the whole control block to be safe.
        unsafe {
            ptr::write_bytes(
                cb as *mut ControlBlock as *mut u8,
                0,
                size_of::<ControlBlock>(),
            )
        };

        cb.consumer_barrier.init(num_consumers as u32);

        cb.buf_count = num_buffers.try_into().unwrap();
        cb.buf_size = buffer_size.try_into().unwrap();
        cb.num_consumers = num_consumers.try_into().unwrap();

        cb.publish_idx.store(0, Relaxed);
        cb.publish_gen.store(0, Relaxed);
        for i in 0..num_consumers {
            cb.consumer_gen[i].0.store(0, Relaxed);
        }

        cb.magic.store(MAGIC, Release);

        let data_ptr = unsafe { (pointer_base as *mut u8).byte_add(SHMEM_DATA_OFFSET) };

        Ok(Self {
            shared: shmem,
            cb,
            cached_ready: false,
            data_base: data_ptr,
            buf_size: cb.buf_size as usize,
            last_published: cb.publish_gen.load(Relaxed),
            start_shutdown: false,
        })
    }

    #[inline]
    fn control_block(&self) -> &ControlBlock {
        unsafe { &*self.cb }
    }

    #[inline]
    fn control_block_mut(&mut self) -> &mut ControlBlock {
        unsafe { &mut *self.cb }
    }

    fn wait_until_ready(&mut self) -> RunResult<()> {
        if self.start_shutdown {
            self.control_block_mut()
                .shutdown
                .store(1, std::sync::atomic::Ordering::Relaxed);
            warn!("(wait ready) Caught interrupt!");
            return Err(RunResultError::Interrupt);
        }

        if self.cached_ready {
            return Ok(());
        }
        //println!("PRODUCER WAITING");
        let need = self.control_block().num_consumers;
        let mut spins = 0;
        loop {
            if self.control_block().ready_count.load(Acquire) == need {
                break;
            }

            adaptive_pause(&mut spins);
        }

        self.control_block_mut().started.store(1, Release);
        self.cached_ready = true;

        //println!("PRODUCER READY");
        Ok(())
    }

    /// Choose a reusable buffer slot by tracking which generations are safe to reclaim.
    /// Simple policy: reclaim any slot whose generation < min_acked (you can keep a ring).
    /// For a minimal sketch, just round-robin across N slots; strict lockstep means by
    /// the time we need to reuse, all will be acked.
    #[inline]
    fn choose_slot_rr(&self, gen_next: u64) -> u32 {
        // spread load across buffers; modulo count
        (gen_next as u32) % self.control_block().buf_count
    }

    pub fn shutdown(&mut self) {
        debug!("Sending shutdown to CB");
        self.start_shutdown = true;
    }

    /// Write into the chosen slot using `write_fn`, then publish it as the next generation
    /// and wait for all consumers to ack (strict lockstep).
    pub fn publish_frame_strict<F: FnOnce(u64, u32, &mut [u8])>(
        &mut self,
        write_fn: F,
    ) -> RunResult<(u64, u32)> {
        let gen_next = self.last_published + 1;
        let slot = self.choose_slot_rr(gen_next);
        let ptr = buffer_ptr(self.data_base, self.buf_size, slot);

        self.wait_until_ready()?;

        //println!("PUBLISH {gen_next} {slot} {ptr:?}");

        // 1) Producer has exclusive write: fill buffer content.
        let buf = unsafe { core::slice::from_raw_parts_mut(ptr, self.buf_size) };
        write_fn(gen_next, slot, buf);

        // 2) Make data visible: publish idx then bump gen with Release ordering.
        self.control_block_mut().publish_idx.store(slot, Relaxed);
        self.control_block_mut()
            .publish_gen
            .store(gen_next, Release);

        // 3) Strict lockstep: wait for all consumers to ack this generation.
        wait_until_min_acked(self.control_block(), gen_next)?;

        //println!("COMMIT");

        self.last_published = gen_next;
        Ok((gen_next, slot))
    }

    /// Split Interface
    pub fn prepare(&mut self) -> RunResult<PartialWriteState> {
        let gen_next = self.last_published + 1;
        let slot = self.choose_slot_rr(gen_next);
        let ptr = buffer_ptr(self.data_base, self.buf_size, slot);

        self.wait_until_ready()?;

        //println!("PREPARE {gen_next} {slot} {ptr:?}");

        // Producer has exclusive write: fill buffer content.
        Ok(PartialWriteState {
            gen_next,
            slot,
            ptr,
            size: self.buf_size,
        })
    }

    pub fn commit(&mut self, state: PartialWriteState) -> RunResult<(u64, u32)> {
        // Make data visible: publish idx then bump gen with Release ordering.
        self.control_block_mut()
            .publish_idx
            .store(state.slot, Relaxed);
        self.control_block_mut()
            .publish_gen
            .store(state.gen_next, Release);

        // Strict lockstep: wait for all consumers to ack this generation.
        wait_until_min_acked(self.control_block(), state.gen_next)?;

        //println!("COMMIT");

        self.last_published = state.gen_next;
        Ok((state.gen_next, state.slot))
    }
}

pub struct PartialWriteState {
    gen_next: u64,
    slot: u32,
    ptr: *mut u8,
    size: usize,
}

impl PartialWriteState {
    #[inline(always)]
    pub fn slice(&mut self) -> &mut [u8] {
        unsafe { core::slice::from_raw_parts_mut(self.ptr, self.size) }
    }
}

pub struct Consumer {
    cb: *mut ControlBlock,
    #[allow(unused)]
    shared: SharedMemory, // keep a reference to shared memory alive
    pub data_base: *mut u8,
    buf_size: usize,
    id: usize,
    last_seen: u64,
}

impl Consumer {
    pub fn new(key: &str, consumer_id: usize) -> Result<Self> {
        let shmem = SharedMemory::attach(key)?;

        let pointer_base = unsafe { shmem.parts().0 };

        let cb = unsafe { &mut *(pointer_base as *mut ControlBlock) };

        while cb.magic.load(Acquire) != MAGIC {
            std::hint::spin_loop();
        }

        //println!("CLIENT CB IS READY");

        let data_ptr = unsafe { (pointer_base as *mut u8).byte_add(SHMEM_DATA_OFFSET) };

        // Hard-forbid late attach: increment, then check gate; rollback if closed.
        // This closes the race where the producer flips 'started' between our check and increment.
        {
            if cb.started.load(Acquire) != 0 {
                return Err(std::io::Error::other(
                    "late attach not allowed: producer already started",
                ));
            }
            cb.ready_count.fetch_add(1, AcqRel);

            if cb.started.load(Acquire) != 0 {
                // Gate closed while we were joining; undo and error out.
                cb.ready_count.fetch_sub(1, AcqRel);
                return Err(std::io::Error::other(
                    "late attach not allowed: producer already started",
                ));
            }
        }

        //println!("CONSUMER SIGNALLED");

        // Wait for the producer to actually start
        while cb.started.load(Acquire) == 0 {
            std::hint::spin_loop();
        }

        assert!(consumer_id < cb.num_consumers as usize);
        let last = cb.publish_gen.load(Relaxed); // join at current frontier
        // ack "gen 0" initially, or last if you want immediate alignment
        cb.consumer_gen[consumer_id].0.store(last, Relaxed);

        //println!("CONSUMER READY");

        Ok(Self {
            shared: shmem,
            cb,
            data_base: data_ptr,
            buf_size: cb.buf_size as usize,
            id: consumer_id,
            last_seen: last,
        })
    }

    #[inline]
    fn control_block(&self) -> &ControlBlock {
        // Safety: Self is always initialized with a valid pointer
        unsafe { &*self.cb }
    }

    #[inline]
    fn control_block_mut(&mut self) -> &mut ControlBlock {
        // Safety: Self is always initialized with a valid pointer
        unsafe { &mut *self.cb }
    }

    /// Block (spin/backoff) until a new buffer is published. Runs given function on the provided buffer.
    pub fn consume_next<F>(&mut self, mut f: F) -> RunResult<()>
    where
        F: FnMut(u64, u32, &[u8]),
    {
        let (gen_id, slot, ptr) = self.wait_for_next()?;

        //println!("CHILD WAIT {gen_id} {slot} {ptr:?}");

        let buf_size = self.control_block().buf_size as usize;

        // Safety: We have already ensured that the ptr is pointing to a buffer of _at least_ this size.
        f(gen_id, slot, unsafe {
            core::slice::from_raw_parts(ptr, buf_size)
        });

        self.ack(gen_id);

        Ok(())
    }

    /// Block (spin/backoff) until a new generation is published, then return (gen, slot, ptr).
    fn wait_for_next(&mut self) -> RunResult<(u64, u32, *const u8)> {
        let mut spins = 0u32;
        loop {
            // We spin a lot so eventually we should see this
            let shutdown_state = self
                .control_block()
                .shutdown
                .load(std::sync::atomic::Ordering::Acquire);

            //debug!("SS {shutdown_state}");

            if shutdown_state > 0 {
                warn!("(wait next) Caught interrupt!");
                return Err(RunResultError::Interrupt);
            }

            let g1 = self.control_block().publish_gen.load(Acquire);
            if g1 == self.last_seen {
                adaptive_pause(&mut spins);
                continue;
            }
            // After seeing a new gen, read idx (Relaxed is fine; the HB edge is via g1)
            let slot = self.control_block().publish_idx.load(Relaxed);
            // Re-read gen to ensure `(g1, slot)` is consistent
            let g2 = self.control_block().publish_gen.load(Acquire);
            if g1 == g2 {
                let ptr = buffer_ptr(self.data_base, self.buf_size, slot) as *const u8;
                return Ok((g1, slot, ptr));
            }
            // Lost a race with a newer publish; try again.
        }
    }

    /// Ack completion of the current generation (strict barrier semantics).
    fn ack(&mut self, gen_id: u64) {
        let id = self.id;
        self.control_block_mut().consumer_gen[id]
            .0
            .store(gen_id, Release);
        self.last_seen = gen_id;
        // (Optional) OS-specific wake producer if it’s waiting on min_acked.
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RunResultError {
    #[error("the shared buffer has been interrupted")]
    Interrupt,
}

pub type RunResult<T> = std::result::Result<T, RunResultError>;

/// Spin/backoff helper: fast spins, then yield, then short sleeps.
#[inline]
fn adaptive_pause(spins: &mut u32) {
    *spins += 1;
    if *spins < 64 {
        std::hint::spin_loop();
    } else if *spins < 256 {
        thread::yield_now();
    } else {
        thread::sleep(Duration::from_micros(1));
    }
}

/// Producer-side wait: strict lockstep requires all consumers to ack `target`.
#[inline]
#[must_use]
fn wait_until_min_acked(cb: &ControlBlock, target: u64) -> RunResult<()> {
    let n = cb.num_consumers as usize;
    let mut spins = 0u32;
    loop {
        if cb.min_acked(n) >= target {
            break;
        }

        adaptive_pause(&mut spins);

        // We spin a lot so eventually we should see this
        if cb.shutdown.load(std::sync::atomic::Ordering::Relaxed) > 0 {
            warn!("(min ack) Caught interrupt!");
            return Err(RunResultError::Interrupt);
        }
    }

    Ok(())
}
