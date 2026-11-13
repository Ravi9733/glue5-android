//! Android-native ScriptHook host and game script-thread callback bridge.
use glue5_game::{Env, Hooks, exports, orig};
use libloading::Library;
use std::ffi::{c_char, c_void, CStr};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

pub const ABI_VERSION: u32 = 4;
const ENTRYPOINT: &[u8] = b"glue5_scripthook_plugin_init\0";
pub type ScriptCallback = unsafe extern "C" fn(env: *mut c_void, info: u64, user: *mut c_void);
pub type NativeResolver = fn(hash: u64) -> Option<u64>;

/// Scalar native argument: 0=signed integer, 1=unsigned/hash, 2=f32 bits, 3=wasm-memory pointer offset.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct NativeArg { pub kind: u32, pub reserved: u32, pub value: u64 }

#[repr(C)]
pub struct HostApi {
    pub abi_version: u32,
    pub log: Option<unsafe extern "C" fn(level: u32, message: *const c_char)>,
    pub register_script_callback: Option<unsafe extern "C" fn(callback: Option<ScriptCallback>, user: *mut c_void) -> i32>,
    pub script_stack_top: Option<unsafe extern "C" fn(env: *mut c_void) -> u64>,
    pub script_stack_alloc: Option<unsafe extern "C" fn(env: *mut c_void, size: u64) -> u64>,
    pub script_stack_restore: Option<unsafe extern "C" fn(env: *mut c_void, top: u64) -> i32>,
    /// Low-level call: info must point to a prepared scrThread::Info.
    pub call_native: Option<unsafe extern "C" fn(env: *mut c_void, hash: u64, info: u64) -> i32>,
    /// Typed scalar call; result is returned as raw 64-bit bits.
    pub call_native_typed: Option<unsafe extern "C" fn(env: *mut c_void, hash: u64, args: *const NativeArg, count: u32, result: *mut u64) -> i32>,
}
pub type PluginInit = unsafe extern "C" fn(host: *const HostApi) -> i32;
#[derive(Clone, Copy)]
struct Callback { function: ScriptCallback, user: usize }
static CALLBACKS: Mutex<Vec<Callback>> = Mutex::new(Vec::new());
static LOADED: Mutex<Vec<Library>> = Mutex::new(Vec::new());
static INSTALLED: AtomicBool = AtomicBool::new(false);
static NATIVE_RESOLVER: OnceLock<NativeResolver> = OnceLock::new();

pub fn set_native_resolver(resolver: NativeResolver) { let _ = NATIVE_RESOLVER.set(resolver); }

unsafe extern "C" fn host_log(level: u32, message: *const c_char) {
    if message.is_null() { return; }
    let message = unsafe { CStr::from_ptr(message) }.to_string_lossy();
    match level { 0 => log::debug!(target: "scripthook-plugin", "{message}"), 1 => log::info!(target: "scripthook-plugin", "{message}"), 2 => log::warn!(target: "scripthook-plugin", "{message}"), _ => log::error!(target: "scripthook-plugin", "{message}") }
}
unsafe extern "C" fn register_script_callback(callback: Option<ScriptCallback>, user: *mut c_void) -> i32 {
    let Some(function) = callback else { return -1 };
    match CALLBACKS.lock() { Ok(mut callbacks) => { callbacks.push(Callback { function, user: user as usize }); 0 }, Err(_) => -2 }
}
unsafe extern "C" fn script_stack_top(env: *mut c_void) -> u64 {
    if env.is_null() { return 0; }
    unsafe { exports::emscripten_stack_get_current(env.cast()) }.unwrap_or(0)
}
unsafe extern "C" fn script_stack_alloc(env: *mut c_void, size: u64) -> u64 {
    if env.is_null() || size == 0 { return 0; }
    unsafe { exports::_emscripten_stack_alloc(env.cast(), size.next_multiple_of(16)) }.unwrap_or(0)
}
unsafe extern "C" fn script_stack_restore(env: *mut c_void, top: u64) -> i32 {
    if env.is_null() { return -1; }
    match unsafe { exports::_emscripten_stack_restore(env.cast(), top) } { Ok(()) => 0, Err(_) => -2 }
}
unsafe extern "C" fn call_native(env: *mut c_void, hash: u64, info: u64) -> i32 {
    if env.is_null() || info == 0 { return -1; }
    let Some(resolver) = NATIVE_RESOLVER.get() else { return -2 };
    let Some(handler) = resolver(hash) else { return -3 };
    match unsafe { glue5_game::call_native(env.cast(), handler, info) } { Ok(()) => 0, Err(_) => -4 }
}

/// Marshals scalar arguments into the scrThread::Info layout used by this game build.
/// All addresses in the argument list are offsets in wasm memory, never host pointers.
unsafe extern "C" fn call_native_typed(
    env: *mut c_void, hash: u64, args: *const NativeArg, count: u32, result: *mut u64,
) -> i32 {
    if env.is_null() || result.is_null() || (count != 0 && args.is_null()) || count > 64 { return -1; }
    let Some(resolver) = NATIVE_RESOLVER.get() else { return -2 };
    let Some(handler) = resolver(hash) else { return -3 };
    let env = env.cast::<Env>();
    let memory = unsafe { (*env).memory };
    if memory.is_null() { return -4; }
    let data = unsafe { (*memory).data };
    let memory_size = unsafe { (*memory).size };
    if data.is_null() || memory_size == 0 { return -4; }
    let top = match unsafe { exports::emscripten_stack_get_current(env) } { Ok(v) => v, Err(_) => return -5 };
    let outcome = (|| -> Result<u64, i32> {
        let alloc = |size: u64| -> Result<u64, i32> {
            let at = unsafe { exports::_emscripten_stack_alloc(env, size.next_multiple_of(16)) }.map_err(|_| -5)?;
            let end = at.checked_add(size).ok_or(-6)?;
            if end > memory_size { return Err(-6); }
            unsafe { std::ptr::write_bytes(data.add(at as usize), 0, size as usize); }
            Ok(at)
        };
        let info = alloc(128)?;
        let params = alloc(u64::from(count.max(1)) * 8)?;
        let native_result = alloc(32)?;
        for index in 0..count as usize {
            let arg = unsafe { *args.add(index) };
            if arg.kind > 3 { return Err(-7); }
            let at = params + index as u64 * 8;
            unsafe { std::ptr::write_unaligned(data.add(at as usize).cast::<u64>(), arg.value); }
        }
        unsafe {
            std::ptr::write_unaligned(data.add(info as usize).cast::<u64>(), native_result);
            std::ptr::write_unaligned(data.add((info + 8) as usize).cast::<u32>(), count);
            std::ptr::write_unaligned(data.add((info + 16) as usize).cast::<u64>(), params);
            std::ptr::write_unaligned(data.add((info + 24) as usize).cast::<i32>(), 0);
        }
        unsafe { glue5_game::call_native(env, handler, info) }.map_err(|_| -8)?;
        Ok(unsafe { std::ptr::read_unaligned(data.add(native_result as usize).cast::<u64>()) })
    })();
    let restored = unsafe { exports::_emscripten_stack_restore(env, top) };
    if restored.is_err() { return -9; }
    match outcome {
        Ok(value) => { unsafe { result.write(value); } 0 }
        Err(code) => code,
    }
}

static HOST_API: HostApi = HostApi {
    abi_version: ABI_VERSION, log: Some(host_log), register_script_callback: Some(register_script_callback),
    script_stack_top: Some(script_stack_top), script_stack_alloc: Some(script_stack_alloc),
    script_stack_restore: Some(script_stack_restore), call_native: Some(call_native),
    call_native_typed: Some(call_native_typed),
};
pub fn install(mods_hook_installed: bool, plugin_directory: &Path) -> LoadReport {
    if !INSTALLED.swap(true, Ordering::AcqRel) && !mods_hook_installed { glue5_runtime::add_hooks(hooks); }
    let report = load_directory(plugin_directory);
    for path in &report.loaded { log::info!("ScriptHook plugin ready: {}", path.display()); }
    for (path, error) in &report.rejected { log::warn!("ScriptHook plugin rejected {}: {}", path.display(), error); }
    report
}
fn hooks(hooks: &mut Hooks) { hooks.script_wait = Some(script_wait); }
pub unsafe fn on_script_wait(env: *mut Env, info: u64) {
    let callbacks = CALLBACKS.lock().map(|items| items.clone()).unwrap_or_default();
    for callback in callbacks { unsafe { (callback.function)(env.cast(), info, callback.user as *mut c_void); } }
}
unsafe extern "C" fn script_wait(env: *mut Env, info: u64) {
    unsafe { on_script_wait(env, info); }
    unsafe { orig::script_wait(env, info) };
}
#[derive(Debug, Default)]
pub struct LoadReport { pub loaded: Vec<PathBuf>, pub rejected: Vec<(PathBuf, String)> }
pub fn load_directory(directory: &Path) -> LoadReport {
    let mut report = LoadReport::default();
    let entries = match std::fs::read_dir(directory) { Ok(entries) => entries, Err(error) => { report.rejected.push((directory.to_path_buf(), error.to_string())); return report; } };
    let mut candidates: Vec<PathBuf> = entries.filter_map(Result::ok).map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("so"))).collect();
    candidates.sort();
    for path in candidates { match unsafe { load_one(&path) } { Ok(()) => report.loaded.push(path), Err(error) => report.rejected.push((path, error)) } }
    report
}
unsafe fn load_one(path: &Path) -> Result<(), String> {
    let library = unsafe { Library::new(path) }.map_err(|e| e.to_string())?;
    let init = unsafe { library.get::<PluginInit>(ENTRYPOINT) }.map_err(|e| format!("missing Glue 5 ScriptHook entrypoint: {e}"))?;
    let status = unsafe { init(&HOST_API) };
    if status != 0 { return Err(format!("plugin initialization returned status {status}")); }
    LOADED.lock().map_err(|_| "plugin registry is poisoned".to_string())?.push(library);
    log::info!("ScriptHook plugin loaded: {}", path.display());
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn abi_version_is_stable_and_nonzero() { assert_eq!(ABI_VERSION, 4); }
    #[test] fn native_arg_layout_is_stable() { assert_eq!(std::mem::size_of::<NativeArg>(), 16); }
    #[test] fn missing_directory_is_reported_without_panicking() {
        let report = load_directory(Path::new("/definitely/not/a/glue5/plugin/folder"));
        assert!(report.loaded.is_empty()); assert_eq!(report.rejected.len(), 1);
    }
}
