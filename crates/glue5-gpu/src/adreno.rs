//! Android Adreno custom Vulkan driver entry point (Turnip/Adreno Tools).
//!
//! The package and libadrenotools runtime are supplied by the Android app
//! integration. The library and loader intentionally remain loaded for the
//! lifetime of the process.
use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::path::Path;
use std::sync::{Mutex, PoisonError};
use anyhow::{Context, Result, bail};

const DRIVER_CUSTOM: c_int = 1;
type OpenLibvulkan = unsafe extern "C" fn(c_int, c_int, *const c_char, *const c_char, *const c_char, *const c_char, *const c_char, *mut *mut c_void) -> *mut c_void;
static IN_USE: Mutex<Option<String>> = Mutex::new(None);

pub(crate) fn in_use() -> std::sync::MutexGuard<'static, Option<String>> {
    IN_USE.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Create a Vulkan instance using a driver package in app-private storage.
pub fn instance(dir: &Path, library: &str) -> Result<wgpu::Instance> {
    let tools = unsafe { libc::dlopen(c"libadrenotools.so".as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL) };
    if tools.is_null() { bail!("libadrenotools: {}", dl_error()); }
    let open = unsafe { libc::dlsym(tools, c"adrenotools_open_libvulkan".as_ptr()) };
    if open.is_null() { bail!("libadrenotools: {}", dl_error()); }
    let mut info: libc::Dl_info = unsafe { std::mem::zeroed() };
    if unsafe { libc::dladdr(open, &mut info) } == 0 || info.dli_fname.is_null() {
        bail!("libadrenotools: cannot locate hook libraries");
    }
    let path = unsafe { CStr::from_ptr(info.dli_fname) }.to_bytes();
    let hooks = CString::new(&path[..path.iter().rposition(|&b| b == b'/').unwrap_or(0)])?;
    let mut folder = dir.as_os_str().as_encoded_bytes().to_vec();
    folder.push(b'/');
    let (folder, name) = (CString::new(folder)?, CString::new(library)?);
    let open: OpenLibvulkan = unsafe { std::mem::transmute(open) };
    let loader = unsafe { open(libc::RTLD_NOW, DRIVER_CUSTOM, std::ptr::null(), hooks.as_ptr(), folder.as_ptr(), name.as_ptr(), std::ptr::null(), std::ptr::null_mut()) };
    if loader.is_null() { bail!("libadrenotools could not open Vulkan loader for {library}"); }
    let gipa = unsafe { libc::dlsym(loader, c"vkGetInstanceProcAddr".as_ptr()) };
    if gipa.is_null() { bail!("custom Vulkan loader has no vkGetInstanceProcAddr"); }
    let gipa = unsafe { std::mem::transmute::<*mut c_void, ash::vk::PFN_vkGetInstanceProcAddr>(gipa) };
    let entry = unsafe { ash::Entry::from_static_fn(ash::StaticFn { get_instance_proc_addr: gipa }) };
    let flags = wgpu::InstanceFlags::from_build_config().with_env()
        | wgpu::InstanceFlags::ALLOW_UNDERLYING_NONCOMPLIANT_ADAPTER;
    let desc = wgpu::hal::InstanceDescriptor {
        name: "wgpu", flags, memory_budget_thresholds: Default::default(),
        backend_options: wgpu::BackendOptions::from_env_or_default(), telemetry: None, display: None,
    };
    let hal = unsafe { wgpu::hal::vulkan::Instance::init_with_entry(entry, &desc, None) }
        .context("creating Vulkan instance on selected Adreno driver")?;
    let instance = unsafe { wgpu::Instance::from_hal::<wgpu::hal::api::Vulkan>(hal) };
    let adapters = pollster::block_on(instance.enumerate_adapters(wgpu::Backends::VULKAN));
    let Some(adapter) = adapters.first() else { bail!("selected driver found no Vulkan GPU"); };
    if !loaded_from(dir) { bail!("selected driver was not loaded; Android kept the system driver"); }
    let info = adapter.get_info();
    log::info!("GPU driver {} from {}: {} ({}, {})", library, dir.display(), info.name, info.driver, info.driver_info);
    *in_use() = Some(dir.file_name().unwrap_or_default().to_string_lossy().into_owned());
    Ok(instance)
}

fn loaded_from(dir: &Path) -> bool {
    let dirs: Vec<Vec<u8>> = [Some(dir.to_path_buf()), std::fs::canonicalize(dir).ok()].into_iter().flatten().map(|d| {
        let mut d = d.into_os_string().into_encoded_bytes(); d.push(b'/'); d
    }).collect();
    unsafe extern "C" fn each(info: *mut libc::dl_phdr_info, _: usize, dirs: *mut c_void) -> c_int {
        let (name, dirs) = unsafe { ((*info).dlpi_name, &*(dirs as *const Vec<Vec<u8>>)) };
        if name.is_null() { return 0; }
        let name = unsafe { CStr::from_ptr(name) }.to_bytes();
        dirs.iter().any(|d| name.starts_with(d)).into()
    }
    unsafe { libc::dl_iterate_phdr(Some(each), (&raw const dirs).cast_mut().cast()) != 0 }
}
fn dl_error() -> String {
    let e = unsafe { libc::dlerror() };
    if e.is_null() { "unknown error".into() } else { unsafe { CStr::from_ptr(e) }.to_string_lossy().into_owned() }
}
