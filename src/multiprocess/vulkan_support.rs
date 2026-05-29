use std::{
    ffi::{CStr, CString, c_char, c_int, c_uint, c_void},
    path::{Path, PathBuf},
    ptr::NonNull,
};

use bevy::log::debug;

use crate::config::VulkanSupport;

const LIBRARY_NAME: &CStr = c"libvktephsupport.so";
const HOST_INIT_SYMBOL: &CStr = c"teph_host_support_init";
const HOST_DEINIT_SYMBOL: &CStr = c"teph_host_support_deinit";
const CLIENT_INIT_SYMBOL: &CStr = c"teph_client_support_init";
const CLIENT_DEINIT_SYMBOL: &CStr = c"teph_client_support_deinit";

pub(crate) const SUPPORT_KEY_ENV: &str = "TEPH_SUPPORT_KEY";

const ENABLE_SUPPORT_LAYER_ENV: &str = "ENABLE_SUPPORT_LAYER";
const ENABLE_SWAP_BARRIER_ENV: &str = "ENABLE_SWAP_BARRIER";
const SUPPORT_DEBUG_ENV: &str = "TEPH_SUPPORT_DEBUG";
const VK_ADD_IMPLICIT_LAYER_PATH_ENV: &str = "VK_ADD_IMPLICIT_LAYER_PATH";
const LD_LIBRARY_PATH_ENV: &str = "LD_LIBRARY_PATH";
const VULKAN_DEVICE_INDEX_ENV: &str = "VULKAN_DEVICE_INDEX";

type HostInit = unsafe extern "C" fn(*const c_char, c_uint) -> c_int;
type HostDeinit = unsafe extern "C" fn(*const c_char);
type ClientInit = unsafe extern "C" fn(*const c_char) -> c_int;
type ClientDeinit = unsafe extern "C" fn(*const c_char);

#[derive(Debug)]
pub(crate) struct VulkanSupportHost {
    _library: VulkanSupportLibrary,
    key: CString,
    host_deinit: HostDeinit,
}

impl VulkanSupportHost {
    pub(crate) fn key(&self) -> &CStr {
        &self.key
    }
}

impl Drop for VulkanSupportHost {
    fn drop(&mut self) {
        unsafe {
            (self.host_deinit)(self.key.as_ptr());
        }
        debug!("Deinitialized Vulkan support host");
    }
}

#[derive(Debug)]
pub(crate) struct VulkanSupportClient {
    _library: VulkanSupportLibrary,
    key: CString,
    client_deinit: ClientDeinit,
}

impl Drop for VulkanSupportClient {
    fn drop(&mut self) {
        unsafe {
            (self.client_deinit)(self.key.as_ptr());
        }
        debug!("Deinitialized Vulkan support client");
    }
}

#[derive(Debug)]
struct VulkanSupportLibrary {
    handle: NonNull<c_void>,
}

unsafe impl Send for VulkanSupportLibrary {}
unsafe impl Sync for VulkanSupportLibrary {}

impl VulkanSupportLibrary {
    fn open(config: &VulkanSupport) -> Self {
        let library = library_name(config);
        let library = CString::new(library.to_string_lossy().as_bytes())
            .expect("Vulkan support library path contains nul byte");

        let handle = unsafe { libc::dlopen(library.as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL) };
        let Some(handle) = NonNull::new(handle) else {
            panic!(
                "Unable to load Vulkan support library {}: {}",
                library.to_string_lossy(),
                dl_error()
            );
        };

        Self { handle }
    }

    fn symbol<T>(&self, symbol: &CStr) -> T {
        let ptr = unsafe { libc::dlsym(self.handle.as_ptr(), symbol.as_ptr()) };
        if ptr.is_null() {
            panic!(
                "Unable to load Vulkan support symbol {}: {}",
                symbol.to_string_lossy(),
                dl_error()
            );
        }

        unsafe { std::mem::transmute_copy(&ptr) }
    }
}

impl Drop for VulkanSupportLibrary {
    fn drop(&mut self) {
        unsafe {
            libc::dlclose(self.handle.as_ptr());
        }
    }
}

pub(crate) fn init_host(config: &VulkanSupport, child_count: u32) -> Option<VulkanSupportHost> {
    if !config.enabled {
        return None;
    }

    let key = CString::new(make_support_key()).expect("support key contains nul byte");
    let library = VulkanSupportLibrary::open(config);
    let host_init: HostInit = library.symbol(HOST_INIT_SYMBOL);
    let host_deinit: HostDeinit = library.symbol(HOST_DEINIT_SYMBOL);

    let result = unsafe { host_init(key.as_ptr(), child_count) };
    if result < 0 {
        panic!(
            "Unable to initialize Vulkan support host for key {}",
            key.to_string_lossy()
        );
    }

    debug!(
        "Initialized Vulkan support host for {} clients with key {}",
        child_count,
        key.to_string_lossy()
    );

    Some(VulkanSupportHost {
        _library: library,
        key,
        host_deinit,
    })
}

pub(crate) fn init_client(config: &VulkanSupport) -> Option<VulkanSupportClient> {
    if !config.enabled {
        return None;
    }

    let key = std::env::var(SUPPORT_KEY_ENV)
        .unwrap_or_else(|_| panic!("Vulkan support enabled but {SUPPORT_KEY_ENV} is not set"));
    let key = CString::new(key).expect("support key contains nul byte");

    let library = VulkanSupportLibrary::open(config);
    let client_init: ClientInit = library.symbol(CLIENT_INIT_SYMBOL);
    let client_deinit: ClientDeinit = library.symbol(CLIENT_DEINIT_SYMBOL);

    let result = unsafe { client_init(key.as_ptr()) };
    if result < 0 {
        panic!(
            "Unable to initialize Vulkan support client for key {}",
            key.to_string_lossy()
        );
    }

    debug!(
        "Initialized Vulkan support client with key {}",
        key.to_string_lossy()
    );

    Some(VulkanSupportClient {
        _library: library,
        key,
        client_deinit,
    })
}

pub(crate) fn install_child_env(
    command: &mut std::process::Command,
    config: &VulkanSupport,
    host: Option<&VulkanSupportHost>,
    card_index: Option<u32>,
) {
    if !config.enabled {
        return;
    }

    let host = host.expect("Vulkan support enabled without host state");
    command.env(ENABLE_SUPPORT_LAYER_ENV, "1");
    command.env(SUPPORT_KEY_ENV, host.key().to_string_lossy().as_ref());

    if let Some(card_index) = card_index {
        command.env(VULKAN_DEVICE_INDEX_ENV, card_index.to_string());
    }

    if config.enable_swap_barrier {
        command.env(ENABLE_SWAP_BARRIER_ENV, "1");
    }

    if config.debug {
        command.env(SUPPORT_DEBUG_ENV, "1");
    }

    if let Some(layer_path) = &config.layer_path {
        prepend_env(command, VK_ADD_IMPLICIT_LAYER_PATH_ENV, layer_path);
    }

    if let Some(library_dir) = &config.library_dir {
        prepend_env(command, LD_LIBRARY_PATH_ENV, library_dir);
    }
}

fn prepend_env(command: &mut std::process::Command, name: &str, value: &str) {
    let joined = match std::env::var(name) {
        Ok(existing) if !existing.is_empty() => format!("{value}:{existing}"),
        _ => value.to_owned(),
    };

    command.env(name, joined);
}

fn library_name(config: &VulkanSupport) -> PathBuf {
    config
        .library_dir
        .as_ref()
        .map(|dir| Path::new(dir).join(LIBRARY_NAME.to_string_lossy().as_ref()))
        .unwrap_or_else(|| PathBuf::from(LIBRARY_NAME.to_string_lossy().as_ref()))
}

fn make_support_key() -> String {
    let id = bevy::asset::uuid::Uuid::new_v4().to_string();
    let suffix = id
        .chars()
        .filter(|ch| *ch != '-')
        .take(8)
        .collect::<String>();
    format!("/TEPHSP_{suffix}")
}

fn dl_error() -> String {
    let error = unsafe { libc::dlerror() };
    if error.is_null() {
        "unknown dlerror".to_string()
    } else {
        unsafe { CStr::from_ptr(error) }
            .to_string_lossy()
            .into_owned()
    }
}
