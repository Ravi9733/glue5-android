//! Android native plugin host foundation.
//!
//! This is a versioned C ABI, not yet the complete ScriptHookV or
//! ScriptHookVDotNet API. Those require game runtime hooks, native
//! registration, and (for .NET) a managed runtime.

use libloading::Library;
use std::ffi::{c_char, CStr};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub const ABI_VERSION: u32 = 1;
const ENTRYPOINT: &[u8] = b"glue5_scripthook_plugin_init\0";

#[repr(C)]
pub struct HostApi {
    pub abi_version: u32,
    pub log: Option<unsafe extern "C" fn(level: u32, message: *const c_char)>,
}

pub type PluginInit = unsafe extern "C" fn(host: *const HostApi) -> i32;

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

static HOST_API: HostApi = HostApi { abi_version: ABI_VERSION, log: Some(host_log) };
static LOADED: Mutex<Vec<Library>> = Mutex::new(Vec::new());

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
    fn abi_version_is_stable_and_nonzero() { assert_eq!(ABI_VERSION, 1); }
    #[test]
    fn missing_directory_is_reported_without_panicking() {
        let report = load_directory(Path::new("/definitely/not/a/glue5/plugin/folder"));
        assert!(report.loaded.is_empty());
        assert_eq!(report.rejected.len(), 1);
    }
}
