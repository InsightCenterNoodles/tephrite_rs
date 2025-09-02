//! This module helps build multiprocess environments for efficient distribution of scene transcripts. To do this, we build a shared memory region that all processes can see. A process writes into this region, and, using a barrier, the child processes can read directly from there.
//!
//! This region is structured with a header block containing a barrier data structure, and after an offset, a data region.
//!
//! TODO: Alter shmem block name with UUID
//! TODO: Move to shared_memory and raw_sync crates

use libc::*;
use std::ffi::CString;

// Name of shared memory region
const SHMEM_NAME_PREFIX: &str = "/TEPHRITE_SHMEM_BLOCK";
// Size of shared memory region. Not resizable at this time.
const SHMEM_BLOCK_SIZE: i64 = 2i64.pow(33);
// Bookkeeping is stored in a region of this size at the start of the block
const SHMEM_DATA_OFFSET: usize = 1024;
// Amount of usable memory in block
const SHMEM_USABLE_SIZE: usize = (SHMEM_BLOCK_SIZE - SHMEM_DATA_OFFSET as i64) as usize;

fn get_shared_mem_block_name() -> CString {
    let id = unsafe { getuid() };

    let formatted = format!("{SHMEM_NAME_PREFIX}.{id}");

    CString::new(formatted).unwrap()
}

/// State for multiprocess communication
pub(crate) struct MPCommunicator {
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

        let barrier = map_result as *mut pthread_barrier_t;

        Self {
            shmem_addr: map_result,
            shmem_data_addr: unsafe { map_result.byte_add(SHMEM_DATA_OFFSET) },
            barrier,
            owner: false,
        }
    }

    pub fn barrier(&self) {
        //println!("Waiting barrier {}", unsafe { getpid() });
        let waitval = unsafe { pthread_barrier_wait(self.barrier) };

        // we dont seem to have access to the constant PTHREAD_BARRIER_SERIAL_THREAD. This, however appears to be negative on a lot of platforms. On linux, this appears to return positive error codes, so we assert that everything is negative here.
        assert!(waitval <= 0);
    }

    /// Obtain a const slice of the data exchange region
    pub fn data_slice(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.shmem_data_addr as *const u8, SHMEM_USABLE_SIZE) }
    }

    /// Obtain a mutable slice of the data exchange region
    #[allow(dead_code)]
    pub fn data_slice_mut(&mut self) -> &mut [u8] {
        unsafe {
            std::slice::from_raw_parts_mut(self.shmem_data_addr as *mut u8, SHMEM_USABLE_SIZE)
        }
    }

    pub(crate) unsafe fn parts(&self) -> (*mut c_void, size_t) {
        (self.shmem_data_addr, SHMEM_USABLE_SIZE)
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

        assert_eq!(pthread_barrierattr_init(&mut battr), 0);

        assert_eq!(
            pthread_barrierattr_setpshared(&mut battr, PTHREAD_PROCESS_SHARED),
            0
        );

        let barrier = map_result as *mut pthread_barrier_t;

        assert_eq!(pthread_barrier_init(barrier, &battr, process_count), 0);

        // we can now destroy the barrier attr
        pthread_barrierattr_destroy(&mut battr);

        barrier
    }
}

/// Opens an existing shared memory region, and returns a handle
fn open_shmem_handle() -> i32 {
    let handle = unsafe {
        let mode = S_IRWXU | S_IRWXG | S_IRWXO;
        let name = get_shared_mem_block_name();
        shm_open(name.as_ptr(), O_RDWR, mode as c_uint)
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
    let truncate_result = unsafe { ftruncate(handle, SHMEM_BLOCK_SIZE) };

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

        shm_unlink(name.as_ptr());

        let mode = S_IRWXU | S_IRWXG | S_IRWXO;
        shm_open(name.as_ptr(), O_RDWR | O_CREAT | O_TRUNC, mode as c_uint)
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
                SHMEM_BLOCK_SIZE.try_into().unwrap(),
                PROT_READ | PROT_WRITE,
                MAP_SHARED,
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
        println!("Destroying Communicator");
        // we don't technically HAVE to do this, but it feels good.
        let ret = unsafe {
            pthread_barrier_destroy(self.barrier);
            munmap(self.shmem_addr, SHMEM_BLOCK_SIZE.try_into().unwrap())
        };

        if ret != 0 {
            println!("unable to unmap shared memory block");
        }

        if self.owner {
            let name = get_shared_mem_block_name();
            let ret = unsafe { shm_unlink(name.as_ptr()) };

            if ret != 0 {
                println!("unable to unlink shared memory block");
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
