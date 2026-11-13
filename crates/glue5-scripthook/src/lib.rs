//! Android-native ScriptHook host and game script-thread callback bridge.
//!
//! Plugins are Android ARM64 shared libraries, not Windows ASI/DLL files.
//! They export glue5_scripthook_plugin_init and register callbacks through
//! the versioned HostApi. Callbacks run from the game's script WAIT hook.

use glue5_game::{Env, Hooks, orig};
use libloading::Library;
use std::ffi::{c_char, c_void, CStr};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

pub const ABI_VERSION: u32 = 2;
const ENTRYPOINT: &[u8] = b"glue5_scripthook_plugin_init\0";

pub type ScriptCallback = unsafe extern "C" fn(env: *mut c_void, info: u64, user: *mut c_void);

#[repr(C)]
pub struct HostApi {
    pub abi_version: u32,
    pub log: Option<unsafe extern "C" fn(level: u32, message: *const c_char)>,
    pub register_script_callback:
        Option<unsafe extern "C" fn(callback: Option<ScriptCallback>, user: *mut c_void) -> i32>,
}

pub type PluginInit = unsafe extern "C" fn(host: *const HostApi) -> i32;

#[derive(Clone, Copy)]
struct Callback { function: ScriptCallback, user: usize }

static CALLBACKS: Mutex<Vec<Callback>> = Mutex::new(Vec::new());
static LOADED: Mutex<Vec<Library>> = Mutex::new(Vec::new());
static INSTALLED: AtomicBool = AtomicBool::new(false);

unsafe extern "C" fn host_log(level: u32, message: *const c_char) {
    if message.is_null() { return; }
    let message = unsafe { CStr::from_ptr(message) }.to_string_lossy();
    match level {
        0 => log::debug!(target: "scripthook-plugin", "{message}"),
        1 => log::info!(target: "scripthook-plugin", "{message}"),
        2 => log::warn!(target: "scripthook-plugin", "{message}"),
        _ => log::error!(target: "scripthook-plugin", "{message}"),
    }
}

unsafe extern "C" fn register_script_callback(
    callback: Option<ScriptCallback>, user: *mut c_void,
) -> i32 {
    let Some(function) = callback else { return -1 };
    match CALLBACKS.lock() {
        Ok(mut callbacks) => {
            callbacks.push(Callback { function, user: user as usize });
            0
        }
        Err(_) => -2,
    }
}

static HOST_API: HostApi = HostApi {
    abi_version: ABI_VERSION,
    log: Some(host_log),
    register_script_callback: Some(register_script_callback),
};

/// Loads plugins and installs the WAIT hook when the Luau hook is absent.
/// When the existing mods hook is active, it calls on_script_wait directly so
/// the two systems share one engine hook rather than overwriting each other.
pub fn install(mods_hook_installed: bool, plugin_directory: &Path) -> LoadReport {
    if !INSTALLED.swap(true, Ordering::AcqRel) && !mods_hook_installed {
        glue5_runtime::add_hooks(hooks);
    }
    let report = load_directory(plugin_directory);
    for path in &report.loaded { log::info!("ScriptHook plugin ready: {}", path.display()); }
    for (path, error) in &report.rejected { log::warn!("ScriptHook plugin rejected {}: {}", path.display(), error); }
    report
}

fn hooks(hooks: &mut Hooks) { hooks.script_wait = Some(script_wait); }

/// Runs registered plugin callbacks from the game's script WAIT context.
pub unsafe fn on_script_wait(env: *mut Env, info: u64) {
    let callbacks = CALLBACKS.lock().map(|items| items.clone()).unwrap_or_default();
    for callback in callbacks {
        unsafe { (callback.function)(env.cast(), info, callback.user as *mut c_void); }
    }
}

unsafe extern "C" fn script_wait(env: *mut Env, info: u64) {
    unsafe { on_script_wait(env, info); }
    unsafe { orig::script_wait(env, info) };
}

#[derive(Debug, Default)]
pub struct LoadReport {
    pub loaded: Vec<PathBuf>,
    pub rejected: Vec<(PathBuf, String)>,
}

/// Loads Android .so plugins in deterministic filename order.
/// .asi, .dll, and .NET assemblies are intentionally not loaded here.
pub fn load_directory(directory: &Path) -> LoadReport {
    let mut report = LoadReport::default();
    let entries = match std::fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) => {
            report.rejected.push((directory.to_path_buf(), error.to_string()));
            return report;
        }
    };
    let mut candidates: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("so")))
        .collect();
    candidates.sort();
    for path in candidates {
        match unsafe { load_one(&path) } {
            Ok(()) => report.loaded.push(path),
            Err(error) => report.rejected.push((path, error)),
        }
    }
    report
}

unsafe fn load_one(path: &Path) -> Result<(), String> {
    let library = unsafe { Library::new(path) }.map_err(|error| error.to_string())?;
    let init = unsafe { library.get::<PluginInit>(ENTRYPOINT) }
        .map_err(|error| format!("missing Glue 5 ScriptHook entrypoint: {error}"))?;
    let status = unsafe { init(&HOST_API) };
    if status != 0 { return Err(format!("plugin initialization returned status {status}")); }
    LOADED.lock().map_err(|_| "plugin registry is poisoned".to_string())?.push(library);
    log::info!("ScriptHook plugin loaded: {}", path.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn abi_version_is_stable_and_nonzero() { assert_eq!(ABI_VERSION, 2); }
    #[test]
    fn missing_directory_is_reported_without_panicking() {
        let report = load_directory(Path::new("/definitely/not/a/glue5/plugin/folder"));
        assert!(report.loaded.is_empty());
        assert_eq!(report.rejected.len(), 1);
    }
}
