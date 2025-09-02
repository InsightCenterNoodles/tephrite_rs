use std::cell::UnsafeCell;
use std::sync::atomic::{
    AtomicUsize,
    Ordering::{Acquire, Relaxed, Release},
};

#[repr(align(64))] // avoid false sharing
struct Slot<T>(UnsafeCell<T>);

// We promise correctness via the atomics below.
unsafe impl<T: Send> Send for Slot<T> {}
unsafe impl<T: Send> Sync for Slot<T> {}

/// A double buffered data structure with an eye towards presenting the 'latest' value to readers.
/// This can be faster than queues if every sample is not required.
pub struct Mailbox<T: Copy> {
    slots: [Slot<T>; 2],
    idx: AtomicUsize, // 0 or 1 — which slot is currently "published"
}

impl<T: Copy> Mailbox<T> {
    pub fn new(init: T) -> Self {
        Self {
            slots: [Slot(UnsafeCell::new(init)), Slot(UnsafeCell::new(init))],
            idx: AtomicUsize::new(0),
        }
    }

    #[inline]
    pub fn write(&self, val: T) {
        // Write into the non-live slot, then publish it.
        let cur = self.idx.load(Relaxed);
        let next = cur ^ 1; // flip 0 <-> 1
        unsafe {
            *self.slots[next].0.get() = val;
        }
        // Make the write visible before publishing the new index.
        self.idx.store(next, Release);
    }

    #[inline]
    pub fn read(&self) -> T {
        loop {
            // Load which slot is live, copy it, then verify it's still live.
            let i1 = self.idx.load(Acquire);
            let val = unsafe { *self.slots[i1].0.get() }; // T: Copy
            let i2 = self.idx.load(Acquire);
            if i1 == i2 {
                return val; // consistent snapshot
            }
            // Writer flipped during our copy—retry.
            std::hint::spin_loop();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::thread;
    use std::time::{Duration, Instant};

    #[test]
    fn basic_read_write() {
        let latest = Mailbox::new(0u64);
        assert_eq!(latest.read(), 0);
        latest.write(42);
        assert_eq!(latest.read(), 42);

        latest.write(99);
        assert_eq!(latest.read(), 99);
    }

    #[test]
    fn concurrent_progress_monotonic() {
        // Writer increments steadily; reader should never observe a decrease.
        let latest = std::sync::Arc::new(Mailbox::new(0u64));
        let running = std::sync::Arc::new(AtomicBool::new(true));

        // Spawn writer: bump a counter a bunch of times.
        let writer = {
            let latest = latest.clone();
            let running = running.clone();
            thread::spawn(move || {
                for i in 1..=100_000u64 {
                    latest.write(i);
                    // keep it hot; no sleep
                }
                // Tell reader we're done shortly.
                running.store(false, Ordering::Release);
            })
        };

        // Reader: ensure observed values never go backwards.
        let mut last = 0u64;
        while running.load(Ordering::Acquire) {
            let v = latest.read();
            assert!(v >= last, "reader observed a decrease: {} -> {}", last, v);
            last = v;
        }

        writer.join().unwrap();
    }

    #[test]
    fn eventual_last_value_after_writer_finishes() {
        let latest = std::sync::Arc::new(Mailbox::new(0u64));

        // Do N sequential writes on a background thread.
        const N: u64 = 50_000;
        let writer = {
            let latest = latest.clone();
            thread::spawn(move || {
                for i in 1..=N {
                    latest.write(i);
                }
                // writer exits; the final publish should be visible
            })
        };

        writer.join().unwrap();

        // After the writer finishes, the reader should be able to observe N.
        // We allow a short retry window to account for scheduling jitter.
        let deadline = Instant::now() + Duration::from_millis(50);
        loop {
            let v = latest.read();
            if v == N {
                break;
            }
            if Instant::now() > deadline {
                panic!("reader never observed final value N={}, last seen {}", N, v);
            }
            std::hint::spin_loop();
        }
    }

    // Compile-time sanity: Latest<T> is Send + Sync when T: Copy + Send.
    // (If this doesn't compile, the trait bounds are wrong.)
    fn assert_send_sync<T: Send + Sync>() {}
    #[test]
    fn latest_is_send_sync() {
        assert_send_sync::<Mailbox<u64>>();
    }
}
