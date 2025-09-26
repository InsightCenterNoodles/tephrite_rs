//! Strict-lockstep shared-memory publisher with per-consumer acks.
//! Works cross-process on Linux/macOS (Windows later with same atomics).
//! Hot path = Acquire/Release atomics; no global barriers.
//! Children are assumed to NOT attach late

use core::mem::size_of;
use core::sync::atomic::{
    AtomicU32, AtomicU64,
    Ordering::{Acquire, Relaxed, Release},
};
use std::io::Result;
use std::time::Duration;
use std::{ptr, thread};

use crate::multiprocess::shared_mem::SharedMemory;

const MAX_CONSUMERS: usize = 16; // This should be enough...
const MAGIC: u64 = 0x53584C4F434B5354; // "SXLOCKST" for sanity

// Bookkeeping is stored in a region of this size at the start of the block.
const SHMEM_DATA_OFFSET: usize = 8192;

pub fn compute_shmem_allocation_size(buf_count: usize, buf_size: usize) -> usize {
    const MAX_PAGE: usize = 2u32.pow(14) as usize;
    let page_size: usize = unsafe {
        libc::sysconf(libc::_SC_PAGESIZE).try_into().map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::Other, "Unable to determine page size")
        })
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
struct ControlBlock {
    magic: AtomicU64,   // sanity check
    buf_count: u32,     // N buffers (>= 2, ideally 3+)
    buf_size: u32,      // bytes per buffer
    num_consumers: u32, // <= MAX_CONSUMERS
    _pad0: u32,

    // Publication side:
    publish_gen: AtomicU64, // monotonically increasing frame number (starts at 0)
    publish_idx: AtomicU32, // which buffer currently holds publish_gen
    _pad1: u32,

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

const _: () = {
    // Ensure that our data offset is large enough...
    assert!(SHMEM_DATA_OFFSET >= core::mem::size_of::<ControlBlock>());

    // keep buffers cacheline-aligned
    assert!(SHMEM_DATA_OFFSET % 128 == 0);
};

// Layout:
// [ControlBlock (aligned to CACHELINE)]
// [padding to fill SHMEM_DATA_OFFSET]
// [buffer 0][buffer 1]...[buffer N-1]

/// Helper: pointer to buffer slot `idx`
#[inline]
pub fn buffer_ptr(data_base: *mut u8, buf_size: usize, idx: u32) -> *mut u8 {
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
    cb: *mut ControlBlock,
    pub data_base: *mut u8,
    buf_size: usize,
    last_published: u64,
}

impl Producer {
    pub fn new(
        key: &str,
        num_buffers: usize,
        buffer_size: usize,
        num_consumers: usize,
    ) -> Result<Self> {
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

        cb.magic.store(MAGIC, Release);
        cb.buf_count = num_buffers.try_into().unwrap();
        cb.buf_size = buffer_size.try_into().unwrap();
        cb.num_consumers = num_consumers.try_into().unwrap();

        cb.publish_idx.store(0, Relaxed);
        cb.publish_gen.store(0, Relaxed);
        for i in 0..(num_consumers as usize) {
            cb.consumer_gen[i].0.store(0, Relaxed);
        }

        let data_ptr = unsafe { (pointer_base as *mut u8).byte_add(SHMEM_DATA_OFFSET) };

        Ok(Self {
            shared: shmem,
            cb,
            data_base: data_ptr,
            buf_size: cb.buf_size as usize,
            last_published: cb.publish_gen.load(Relaxed),
        })
    }

    #[inline]
    fn control_block(&self) -> &ControlBlock {
        return unsafe { &*self.cb };
    }

    #[inline]
    fn control_block_mut(&mut self) -> &mut ControlBlock {
        return unsafe { &mut *self.cb };
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

    /// Write into the chosen slot using `write_fn`, then publish it as the next generation
    /// and wait for all consumers to ack (strict lockstep).
    pub fn publish_frame_strict<F: FnOnce(u64, u32, &mut [u8])>(
        &mut self,
        write_fn: F,
    ) -> (u64, u32) {
        let gen_next = self.last_published + 1;
        let slot = self.choose_slot_rr(gen_next);
        let ptr = buffer_ptr(self.data_base, self.buf_size, slot);

        // 1) Producer has exclusive write: fill buffer content.
        let buf = unsafe { core::slice::from_raw_parts_mut(ptr, self.buf_size) };
        write_fn(gen_next, slot, buf);

        // 2) Make data visible: publish idx then bump gen with Release ordering.
        self.control_block_mut().publish_idx.store(slot, Relaxed);
        self.control_block_mut()
            .publish_gen
            .store(gen_next, Release);

        // 3) Strict lockstep: wait for all consumers to ack this generation.
        wait_until_min_acked(self.control_block(), gen_next);

        self.last_published = gen_next;
        (gen_next, slot)
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

        let data_ptr = unsafe { (pointer_base as *mut u8).byte_add(SHMEM_DATA_OFFSET) };

        assert!(consumer_id < cb.num_consumers as usize);
        let last = cb.publish_gen.load(Relaxed); // join at current frontier
        // ack "gen 0" initially, or last if you want immediate alignment
        cb.consumer_gen[consumer_id].0.store(last, Relaxed);

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
        return unsafe { &*self.cb };
    }

    #[inline]
    fn control_block_mut(&mut self) -> &mut ControlBlock {
        return unsafe { &mut *self.cb };
    }

    /// Block (spin/backoff) until a new buffer is published. Runs given function on the provided buffer.
    pub fn consume_next<F>(&mut self, mut f: F)
    where
        F: FnMut(u64, u32, &[u8]),
    {
        let (gen_id, slot, ptr) = self.wait_for_next();

        let buf_size = self.control_block().buf_size as usize;

        // Safety: We have already ensured that the ptr is pointing to a buffer of _at least_ this size.
        f(gen_id, slot, unsafe {
            core::slice::from_raw_parts(ptr, buf_size)
        });

        self.ack(gen_id);
    }

    /// Block (spin/backoff) until a new generation is published, then return (gen, slot, ptr).
    fn wait_for_next(&mut self) -> (u64, u32, *const u8) {
        let mut spins = 0u32;
        loop {
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
                return (g1, slot, ptr);
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

/// Spin/backoff helper: fast spins, then yield, then short sleeps.
#[inline]
fn adaptive_pause(spins: &mut u32) {
    *spins += 1;
    if *spins < 64 {
        std::hint::spin_loop();
    } else if *spins < 256 {
        thread::yield_now();
    } else {
        // Tune as needed; keep tiny to preserve latency.
        thread::sleep(Duration::from_micros(50));
    }
}

/// Producer-side wait: strict lockstep requires all consumers to ack `target`.
fn wait_until_min_acked(cb: &ControlBlock, target: u64) {
    let n = cb.num_consumers as usize;
    let mut spins = 0u32;
    loop {
        if cb.min_acked(n) >= target {
            break;
        }
        adaptive_pause(&mut spins);
    }
}
