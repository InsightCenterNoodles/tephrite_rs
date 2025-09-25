//! This module helps build multiprocess environments for efficient distribution of scene transcripts. To do this, we build a shared memory region that all processes can see. A process writes into this region, and, using a barrier, the child processes can read directly from there.
//!
//! This region is structured with a header block containing a barrier data structure, and after an offset, a data region.
//!
//! TODO: Move to shared_memory and raw_sync crates

use bevy::log::{debug, warn};
use libc::*;
use std::{ffi::CString, sync::LazyLock};

// Name of shared memory region
const SHMEM_NAME_PREFIX: &str = "TEPH_SHMEM";

// Size of shared memory region. Not resizable at this time.
// Since we are sending large textures, meshes, and huge instance lists, this is a 'safe' bound.
// Previous versions would break at 2 gigs. In the future, we should shard this.
const SHMEM_DEFAULT_BLOCK_SIZE: u64 = 2u64.pow(33);

// Size of shared memory region for testing purposes.
const SHMEM_TESTING_BLOCK_SIZE: u64 = 2u64.pow(17);

// Bookkeeping is stored in a region of this size at the start of the block.
const SHMEM_DATA_OFFSET: usize = 1024;

// At the moment, the header consists only of a shared barrier, this must be at offset 0. The header MUST be larger than this.

// Amount of usable memory in block
static SHMEM_BLOCK_SIZE: LazyLock<usize> = LazyLock::new(|| {
    if std::env::var("TEPHRITE_TEST_PROCESS").is_ok() {
        (SHMEM_TESTING_BLOCK_SIZE) as usize
    } else {
        (SHMEM_DEFAULT_BLOCK_SIZE) as usize
    }
});

// Amount of usable memory in block
static SHMEM_USABLE_SIZE: LazyLock<usize> = LazyLock::new(|| {
    let blk_size = *SHMEM_BLOCK_SIZE;
    assert!(blk_size > SHMEM_DATA_OFFSET);
    println!("BLOCK SIZE {blk_size}");
    blk_size - SHMEM_DATA_OFFSET
});

fn get_shared_mem_block_name() -> CString {
    // This is a UUID string under the hood
    let session_id = super::session_id();

    let session_id: String = session_id
        .as_str()
        .chars()
        .skip_while(|x| !x.is_ascii_alphanumeric())
        .collect();

    let formatted = format!("/{SHMEM_NAME_PREFIX}.{}", session_id.as_str());
    //let formatted = format!("/{SHMEM_NAME_PREFIX}");

    if cfg!(target_os = "macos") {
        // Truncate the name. Mac os X limits this to 31!
        // This _should_ be ok, as we are not going to be running this app multiple times here
        let mut f = formatted.clone();
        f.truncate(31);
        println!("KEY: {f}");
        return CString::new(f).unwrap();
    }

    println!("KEY: {formatted}");

    CString::new(formatted).unwrap()
}

// In the future can we just wrap the call somehow?
#[inline]
fn check_pthread_call(ret: i32, region: &'static str) {
    if ret == 0 {
        return;
    }

    panic!("Pthread call failed: {region}, errorcode {ret}");
}

/// State for multiprocess communication
pub struct MPCommunicator {
    /// Address of shmem block
    shmem_addr: *mut c_void,
    /// Address of data region in block
    shmem_data_addr: *mut c_void,
    /// Address of barrier in the bookkeeping region
    barrier: *mut pthread_barrier_t,
    /// Are we the owner of the shmem?
    owner: bool,
}

impl MPCommunicator {
    /// Create a new multiprocess state. This should only be called by the root process.
    pub fn create(process_count: u32) -> Self {
        println!("Creating Communicator {}", unsafe { getpid() });
        let handle = create_shmem_handle();

        truncate_handle(handle);

        let map_result = map_shmem_handle(handle);

        unsafe { close(handle) }; // Do not leak FDs

        let barrier = create_and_install_barrier(map_result, process_count);

        Self {
            shmem_addr: map_result,
            shmem_data_addr: unsafe { map_result.byte_add(SHMEM_DATA_OFFSET) },
            barrier,
            owner: true,
        }
    }

    /// Attach to a previously created multiprocess state. This should only be called by child processes
    pub fn attach() -> Self {
        println!("Attaching Communicator {}", unsafe { getpid() });
        let handle = open_shmem_handle();

        let map_result = map_shmem_handle(handle);

        // can close handle after mapping
        unsafe { close(handle) };

        let barrier = map_result as *mut pthread_barrier_t;

        Self {
            shmem_addr: map_result,
            shmem_data_addr: unsafe { map_result.byte_add(SHMEM_DATA_OFFSET) },
            barrier,
            owner: false,
        }
    }

    pub fn barrier(&self) {
        println!("Waiting barrier {}", unsafe { getpid() });

        let hdr = self.shmem_addr as *const u64;
        unsafe {
            println!(
                "pid {} hdr[0..2]={:x} {:x}",
                libc::getpid(),
                *hdr,
                *hdr.add(1)
            );
        }

        let waitval = unsafe { pthread_barrier_wait(self.barrier) };

        #[cfg(any(target_os = "linux"))]
        const SERIAL: i32 = libc::PTHREAD_BARRIER_SERIAL_THREAD;
        #[cfg(target_os = "macos")]
        const SERIAL: i32 = PTHREAD_BARRIER_SERIAL_THREAD;

        if waitval != 0 && waitval != SERIAL {
            panic!("barrier wait failed: {waitval}");
        }
    }

    /// Obtain a const slice of the data exchange region
    #[inline]
    pub fn data_slice(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.shmem_data_addr as *const u8, *SHMEM_USABLE_SIZE) }
    }

    /// Obtain a mutable slice of the data exchange region
    #[allow(dead_code)]
    #[inline]
    pub fn data_slice_mut(&mut self) -> &mut [u8] {
        unsafe {
            std::slice::from_raw_parts_mut(self.shmem_data_addr as *mut u8, *SHMEM_USABLE_SIZE)
        }
    }

    /// Obtain a raw pointer and size to the data region. These MUST NOT be stored, and are
    /// invalid as soon as the communicator is dropped!
    #[inline]
    pub(crate) unsafe fn parts(&self) -> (*mut c_void, size_t) {
        (self.shmem_data_addr, *SHMEM_USABLE_SIZE)
    }
}

/// Constructs a barrier at offset zero of a mapped address, for a given number of processes
fn create_and_install_barrier(
    map_result: *mut c_void,
    process_count: u32,
) -> *mut pthread_barrier_t {
    unsafe {
        use std::mem::MaybeUninit;
        let mut battr = MaybeUninit::zeroed().assume_init();

        check_pthread_call(
            pthread_barrierattr_init(&mut battr),
            "barrier attribute init",
        );

        check_pthread_call(
            pthread_barrierattr_setpshared(&mut battr, PTHREAD_PROCESS_SHARED),
            "attr: set process shared",
        );

        let barrier = map_result as *mut pthread_barrier_t;

        check_pthread_call(
            pthread_barrier_init(barrier, &battr, process_count),
            "init barrier",
        );

        println!("NEW BARRIER {process_count}");

        // we can now destroy the barrier attr
        pthread_barrierattr_destroy(&mut battr);

        barrier
    }
}

const OPEN_MODES: c_uint = (S_IRUSR | S_IWUSR) as c_uint;

/// Opens an existing shared memory region, and returns a handle
fn open_shmem_handle() -> i32 {
    let handle = unsafe {
        let name = get_shared_mem_block_name();
        println!("SHM NAME (child): {name:?}");
        shm_open(name.as_ptr(), O_RDWR, OPEN_MODES)
    };

    if handle < 0 {
        panic!(
            "Unable to attach to shared memory block: {:?}",
            std::io::Error::last_os_error()
        );
    }
    handle
}

/// Resizes a shared memory region
fn truncate_handle(handle: i32) {
    let truncate_result = unsafe { ftruncate(handle, *SHMEM_BLOCK_SIZE as i64) };

    if truncate_result < 0 {
        panic!(
            "Unable to resize shared memory block: {:?}",
            std::io::Error::last_os_error()
        );
    }
}

/// Constructs a new shared memory region, and returns a handle
fn create_shmem_handle() -> i32 {
    let handle = unsafe {
        // first attempt to unlink if there are stale objects around
        let name = get_shared_mem_block_name();

        println!("SHM NAME (parent): {name:?}");

        shm_unlink(name.as_ptr());

        shm_open(name.as_ptr(), O_RDWR | O_CREAT | O_TRUNC, OPEN_MODES)
    };

    if handle < 0 {
        panic!(
            "Unable to create shared memory block: {:?}",
            std::io::Error::last_os_error()
        );
    }
    handle
}

/// Map a given shared memory handle to our address space
fn map_shmem_handle(handle: i32) -> *mut c_void {
    let map_result = unsafe {
        #[cfg(target_os = "linux")]
        {
            mmap64(
                std::ptr::null_mut(),
                SHMEM_BLOCK_SIZE.try_into().unwrap(),
                PROT_READ | PROT_WRITE,
                MAP_SHARED,
                handle,
                0,
            )
        }
        #[cfg(target_os = "macos")]
        {
            mmap(
                std::ptr::null_mut(),
                (*SHMEM_BLOCK_SIZE).try_into().unwrap(),
                PROT_READ | PROT_WRITE,
                MAP_SHARED | libc::MAP_HASSEMAPHORE,
                handle,
                0,
            )
        }
    };

    if map_result == MAP_FAILED {
        panic!(
            "Unable to map shared block: {:?}",
            std::io::Error::last_os_error()
        );
    }
    map_result
}

impl Drop for MPCommunicator {
    fn drop(&mut self) {
        debug!("Destroying Communicator");
        // we don't technically HAVE to do this, but it feels good.
        let ret = unsafe {
            if self.owner {
                pthread_barrier_destroy(self.barrier);
            }
            munmap(self.shmem_addr, (*SHMEM_BLOCK_SIZE).try_into().unwrap())
        };

        if ret != 0 {
            warn!("unable to unmap shared memory block");
        }

        if self.owner {
            let name = get_shared_mem_block_name();
            let ret = unsafe { shm_unlink(name.as_ptr()) };

            if ret != 0 {
                warn!("unable to unlink shared memory block");
            }
        }
    }
}

#[allow(non_camel_case_types)]
#[cfg(target_os = "macos")]
mod emulate {
    #[repr(C)]
    pub struct pthread_barrier_t {
        mutex: libc::pthread_mutex_t,
        cond: libc::pthread_cond_t,
        count: u32,
        left: u32,
        round: u32,
    }

    pub type pthread_barrierattr_t = libc::pthread_mutexattr_t;

    pub unsafe fn pthread_barrier_init(
        barrier: *mut pthread_barrier_t,
        attr: *const pthread_barrierattr_t,
        count: u32,
    ) -> i32 {
        if count == 0 {
            return libc::EINVAL;
        }

        let mut ret;

        let mut condattr =
            unsafe { std::mem::MaybeUninit::<libc::pthread_condattr_t>::zeroed().assume_init() };

        unsafe { libc::pthread_condattr_init(&mut condattr) };

        if !attr.is_null() {
            let mut pshared = 0;
            ret = unsafe { pthread_barrierattr_getpshared(attr, &mut pshared) };
            if ret != 0 {
                return ret;
            }
            ret = unsafe { libc::pthread_condattr_setpshared(&mut condattr, pshared) };
            if ret != 0 {
                return ret;
            }
        }

        ret = unsafe { libc::pthread_mutex_init(&mut (*barrier).mutex, attr) };

        if ret != 0 {
            return ret;
        }

        ret = unsafe { libc::pthread_cond_init(&mut (*barrier).cond, &condattr) };

        if ret != 0 {
            unsafe { libc::pthread_mutex_destroy(&mut (*barrier).mutex) };
            return ret;
        }

        unsafe {
            (*barrier).count = count;
            (*barrier).left = count;
            (*barrier).round = 0;
        }

        0
    }

    pub unsafe fn pthread_barrier_destroy(barrier: *mut pthread_barrier_t) -> i32 {
        if unsafe { (*barrier).count == 0 } {
            return libc::EINVAL;
        }

        unsafe { (*barrier).count = 0 };
        let rm = unsafe { libc::pthread_mutex_destroy(&mut (*barrier).mutex) };
        let rc = unsafe { libc::pthread_cond_destroy(&mut (*barrier).cond) };
        if rm != 0 { rm } else { rc }
    }

    pub const PTHREAD_BARRIER_SERIAL_THREAD: i32 = -1;

    pub unsafe fn pthread_barrier_wait(barrier: *mut pthread_barrier_t) -> i32 {
        unsafe {
            libc::pthread_mutex_lock(&mut (*barrier).mutex);

            (*barrier).left -= 1;

            if (*barrier).left != 0 {
                let round = (*barrier).round;
                while {
                    libc::pthread_cond_wait(&mut (*barrier).cond, &mut (*barrier).mutex);
                    round == (*barrier).round
                } {}
                libc::pthread_mutex_unlock(&mut (*barrier).mutex);
                return 0;
            }

            (*barrier).round += 1;
            (*barrier).left = (*barrier).count;
            libc::pthread_cond_broadcast(&mut (*barrier).cond);
            libc::pthread_mutex_unlock(&mut (*barrier).mutex);

            PTHREAD_BARRIER_SERIAL_THREAD
        }
    }

    pub unsafe fn pthread_barrierattr_init(attr: *mut pthread_barrierattr_t) -> i32 {
        unsafe { libc::pthread_mutexattr_init(attr) }
    }
    pub unsafe fn pthread_barrierattr_destroy(attr: *mut pthread_barrierattr_t) -> i32 {
        unsafe { libc::pthread_mutexattr_destroy(attr) }
    }
    pub unsafe fn pthread_barrierattr_getpshared(
        attr: *const pthread_barrierattr_t,
        pshared: *mut i32,
    ) -> i32 {
        unsafe { libc::pthread_mutexattr_getpshared(attr, pshared) }
    }

    pub unsafe fn pthread_barrierattr_setpshared(
        attr: *mut pthread_barrierattr_t,
        pshared: i32,
    ) -> i32 {
        unsafe { libc::pthread_mutexattr_setpshared(attr, pshared) }
    }
}

#[cfg(target_os = "macos")]
use emulate::*;

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    /// After the owner drops, the shm object should be unlinked and re-open should fail.
    #[test]
    fn unlink_on_drop_removes_segment() {
        unsafe { env::set_var("TEPHRITE_TEST_PROCESS", "1") };

        let session = crate::multiprocess::generate_session_id();

        crate::multiprocess::install_session_id(&session);

        // Create and immediately drop to trigger unlink
        {
            let _owner = MPCommunicator::create(1);
        }
        // Try to open again with the same name: should fail with ENOENT
        let name = super::get_shared_mem_block_name();
        let fd = unsafe { shm_open(name.as_ptr(), O_RDWR, OPEN_MODES) };
        assert!(
            fd < 0,
            "shm_open unexpectedly succeeded after owner drop (fd={fd})"
        );
        let err = std::io::Error::last_os_error();
        // Not all platforms set ENOENT consistently, so just make sure it failed.
        // If you want strictness on Linux:
        #[cfg(target_os = "linux")]
        assert_eq!(err.raw_os_error(), Some(libc::ENOENT));
    }
}
