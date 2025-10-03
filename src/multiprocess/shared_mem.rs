//! Module for building shared memory regions

use bevy::log::{debug, warn};
use libc::*;
use std::ffi::{CStr, CString};

use std::io::Result;

/// Shared memory handle. When dropped, will attempt to deallocate the region
pub struct SharedMemory {
    /// Address of shmem block
    shmem_addr: *mut c_void,

    /// Size of shmem block,
    shmem_size: usize,

    /// Are we the owner of the shmem?
    owner: bool,

    /// Name of the shmem region
    key: CString,
}

impl SharedMemory {
    /// Create a new multiprocess state. This should only be called by the root process.
    /// Note that the key here MUST start with a '/' AND must be less than 31 chars due to mac restrictions
    pub fn create(key: &str, size: usize) -> Result<Self> {
        //println!("Creating Communicator {}", unsafe { getpid() });

        if !key.starts_with('/') || key[1..].contains('/') {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "key must be '/name'",
            ));
        }
        if key.len() > 255 {
            // conservative
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "key too long",
            ));
        }

        let page_size: usize = unsafe {
            libc::sysconf(libc::_SC_PAGESIZE).try_into().map_err(|_| {
                std::io::Error::new(std::io::ErrorKind::Other, "Unable to determine page size")
            })
        }?;

        let size = size.next_multiple_of(page_size);

        {
            assert_eq!(key.chars().next().unwrap(), '/');
            assert_eq!(key.chars().filter(|x| *x == '/').count(), 1);
        }

        let local_key = CString::new(key).expect("create shmem");

        let handle = create_shmem_handle(&local_key)?;

        truncate_handle(handle, size)?;

        let map_result = map_shmem_handle(handle, size)?;

        unsafe { close(handle) }; // Do not leak FDs

        Ok(Self {
            shmem_addr: map_result,
            shmem_size: size,
            owner: true,
            key: local_key,
        })
    }

    /// Attach to a previously created multiprocess state. This should only be called by child processes
    pub fn attach(key: &str) -> Result<Self> {
        println!("Attaching Communicator {}", unsafe { getpid() });

        let local_key = CString::new(key).expect("create shmem");

        let handle = open_shmem_handle(&local_key)?;

        // find the size of the region

        let size: usize = {
            let mut buf = std::mem::MaybeUninit::<libc::stat>::uninit();

            let ret = unsafe { libc::fstat(handle, buf.as_mut_ptr()) };

            if ret == -1 {
                return Err(std::io::Error::last_os_error());
            }

            let buf = unsafe { buf.assume_init() };

            buf.st_size
                .try_into()
                .map_err(|_| std::io::Error::from_raw_os_error(libc::EINVAL))?
        };

        let map_result = map_shmem_handle(handle, size)?;

        // can close handle after mapping
        unsafe { close(handle) };

        Ok(Self {
            shmem_addr: map_result,
            shmem_size: size,
            owner: false,
            key: local_key,
        })
    }

    /// Obtain a const slice of the data exchange region
    #[inline]
    pub fn as_slice(&self) -> &[u8] {
        unsafe {
            std::slice::from_raw_parts(self.shmem_addr as *const u8, self.shmem_size as usize)
        }
    }

    /// Obtain a mutable slice of the data exchange region
    #[allow(dead_code)]
    #[inline]
    pub fn as_slice_mut(&mut self) -> &mut [u8] {
        unsafe {
            std::slice::from_raw_parts_mut(self.shmem_addr as *mut u8, self.shmem_size as usize)
        }
    }

    /// Obtain a raw pointer and size to the data region. These MUST NOT be stored, and are
    /// invalid as soon as the communicator is dropped!
    #[inline]
    pub(crate) unsafe fn parts(&self) -> (*mut c_void, size_t) {
        (self.shmem_addr, self.shmem_size as size_t)
    }
}

impl Drop for SharedMemory {
    fn drop(&mut self) {
        debug!("Destroying Communicator");
        // we don't technically HAVE to do this, but it feels good.
        let ret = unsafe {
            // if self.owner {
            //     pthread_barrier_destroy(self.barrier);
            // }
            munmap(self.shmem_addr, self.shmem_size.try_into().unwrap())
        };

        if ret != 0 {
            warn!("unable to unmap shared memory block");
        }

        if self.owner {
            let ret = unsafe { shm_unlink(self.key.as_ptr()) };

            if ret != 0 {
                warn!("unable to unlink shared memory block");
            }
        }
    }
}

fn to_i64(size: usize) -> Result<i64> {
    size.try_into()
        .map_err(|_| std::io::Error::from_raw_os_error(libc::EINVAL))
}

// We dont use mode_t here as all the APIs take c_uint already
const OPEN_MODES: c_uint = (S_IRUSR | S_IWUSR) as c_uint;

/// Opens an existing shared memory region, and returns a handle
fn open_shmem_handle(key: &CStr) -> Result<i32> {
    let handle = unsafe {
        //println!("SHM NAME (child): {key:?}");
        shm_open(key.as_ptr(), O_RDWR | O_CREAT | O_CLOEXEC, OPEN_MODES)
    };

    if handle < 0 {
        return Err(std::io::Error::last_os_error());
    }

    Ok(handle)
}

/// Resizes a shared memory region
fn truncate_handle(handle: i32, size: usize) -> Result<()> {
    let off: libc::off_t = to_i64(size)?;

    let truncate_result = unsafe { ftruncate(handle, off) };

    if truncate_result < 0 {
        return Err(std::io::Error::last_os_error());
    }

    Ok(())
}

/// Constructs a new shared memory region, and returns a handle. Note that this will UNLINK any region
/// with the given key. Watch out!
fn create_shmem_handle(key: &CStr) -> Result<i32> {
    let handle = unsafe {
        //println!("SHM NAME (parent): {key:?}");

        shm_unlink(key.as_ptr());

        shm_open(
            key.as_ptr(),
            O_RDWR | O_CREAT | O_TRUNC | O_CLOEXEC,
            OPEN_MODES,
        )
    };

    if handle < 0 {
        return Err(std::io::Error::last_os_error());
    }

    Ok(handle)
}

/// Map a given shared memory handle to our address space
fn map_shmem_handle(handle: i32, size: usize) -> Result<*mut c_void> {
    let map_result = unsafe {
        mmap(
            std::ptr::null_mut(),
            size,
            PROT_READ | PROT_WRITE,
            MAP_SHARED,
            handle,
            0,
        )
    };

    if map_result == MAP_FAILED {
        return Err(std::io::Error::last_os_error());
    }

    Ok(map_result)
}
