//! Mods in the engine: each runs in a script thread of its own, so its
//! natives run in a real script context (the entities it creates belong to
//! its thread, and the engine cleans them up when the thread ends).
//!
//! The thread's program is ours, built in wasm memory: `loop: WAIT(0)`.
//! Every frame the engine calls WAIT for every script thread; the hook on
//! it resumes the mod's Lua when the thread is a mod's and is about to wait,
//! and writes how long it waits into WAIT's argument. Starting and killing
//! threads happens from the same hook, in whatever thread waits next, so it
//! all stays on the engine's script update.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::Ordering;
use std::time::Instant;

use glue5_game::{Env, Hooks, orig};
use glue5_runtime::memory::Mem;

use crate::vm::{Jobs, Step, Vm};
use crate::{Command, LogLevel, ModState, SHARED, log_line, natives};

/// The REPL's thread: `eval` runs there, as a mod of its own.
const REPL: &str = "~eval";

pub(crate) fn hooks(h: &mut Hooks) {
    h.script_wait = Some(script_wait);
    h.packfile_load_rpf = Some(packfile_load_rpf);
    h.packfile_init = Some(packfile_init);
    h.packfile_mount_as = Some(packfile_mount_as);
    h.datafile_add_rpf = Some(datafile_add_rpf);
    h.packfile_add_all_rpf_files = Some(packfile_add_all_rpf_files);
    h.packfile_get_streaming_module = Some(packfile_get_streaming_module);
    h.packfile_register_individual_file = Some(packfile_register_individual_file);
    h.stream_request_object = Some(stream_request_object);
    h.stream_load_object = Some(stream_load_object);
    h.stream_index_by_filename = Some(stream_index_by_filename);
    h.model_get_base_info = Some(model_get_base_info);
    h.model_request_assets = Some(model_request_assets);
    h.model_have_assets_loaded = Some(model_have_assets_loaded);
    h.vehicle_factory_create = Some(vehicle_factory_create);
    h.device_mount = Some(device_mount);
    h.device_unmount = Some(device_unmount);
    h.device_relative_init = Some(device_relative_init);
    h.device_relative_mount_as = Some(device_relative_mount_as);
    h.device_get_device = Some(device_get_device);
    h.device_relative_low_level = Some(device_relative_low_level);
    h.device_relative_rpf = Some(device_relative_rpf);
    h.device_relative_debug_name = Some(device_relative_debug_name);
    h.device_relative_open = Some(device_relative_open);
    h.device_relative_find_begin = Some(device_relative_find_begin);
    h.device_relative_file_size = Some(device_relative_file_size);
    h.mountable_content_ctor = Some(mountable_content_ctor);
    h.mountable_content_load_setup = Some(mountable_content_load_setup);
    h.mountable_content_mount = Some(mountable_content_mount);
    h.mountable_content_mount_content = Some(mountable_content_mount_content);
    h.mountable_content_unmount = Some(mountable_content_unmount);
    h.extra_content_add_folder = Some(extra_content_add_folder);
    h.extra_content_load = Some(extra_content_load);
    h.extra_content_get_by_device = Some(extra_content_get_by_device);
    h.extra_content_get_by_hash = Some(extra_content_get_by_hash);
    h.extra_content_recursive_enum = Some(extra_content_recursive_enum);
    h.extra_content_begin_enumerate = Some(extra_content_begin_enumerate);
    h.extra_content_update_array = Some(extra_content_update_array);
    h.extra_content_execute_group = Some(extra_content_execute_group);
    h.extra_content_execute_internal = Some(extra_content_execute_internal);
    h.fileloader_execute_changeset = Some(fileloader_execute_changeset);
    h.fileloader_revert_changeset = Some(fileloader_revert_changeset);
    h.dlc_ittyp_load_datafile = Some(dlc_ittyp_load_datafile);
    h.dlc_ittyp_unload_datafile = Some(dlc_ittyp_unload_datafile);
    h.extra_content_execute_changeset = Some(extra_content_execute_changeset);
    h.model_post_load = Some(model_post_load);
    h.model_get_local_info = Some(model_get_local_info);
    h.extra_content_manager_dtor = Some(extra_content_manager_dtor);
    h.mountable_content_shutdown = Some(mountable_content_shutdown);
    h.mountable_content_execute_changeset = Some(mountable_content_execute_changeset);
    h.mountable_content_revert_changeset = Some(mountable_content_revert_changeset);
}

/// Observe the real game's RPF load path without changing its behavior.
/// The exact symbol names and ABIs are resolved from the pinned game.wasm.
unsafe extern "C" fn packfile_load_rpf(env: *mut Env, path: u64, index: u32) {
    DLC_RPF_LOAD_COUNT.fetch_add(1, Ordering::Relaxed);
    let name = unsafe { Mem::of(env).str(path) };
    if name.to_ascii_lowercase().ends_with(".rpf") {
        remember_loader_path(&DLC_LAST_RPF_PATH, &name);
        log_line("vehicle-loader", LogLevel::Info,
            &format!("LoadRpf index={index} path={name}"));
    }
    unsafe { orig::packfile_load_rpf(env, path, index) };
}

/// Observe packfile initialization (including encryption/cache mode), then forward unchanged.
unsafe extern "C" fn packfile_init(
    env: *mut Env,
    packfile: u64,
    path: u64,
    encrypted: u32,
    cache_mode: u32,
    key_path: u64,
) -> u32 {
    DLC_RPF_INIT_COUNT.fetch_add(1, Ordering::Relaxed);
    let mem = unsafe { Mem::of(env) };
    let name = mem.str(path);
    let key = mem.str(key_path);
    if name.to_ascii_lowercase().ends_with(".rpf") {
        remember_loader_path(&DLC_LAST_RPF_PATH, &name);
        log_line("vehicle-loader", LogLevel::Info,
            &format!("fiPackfile::Init object=0x{packfile:x} encrypted={} cache={cache_mode} path={name} key={key}",
                encrypted != 0));
    }
    unsafe { orig::packfile_init(env, packfile, path, encrypted, cache_mode, key_path) }
}

/// Passive probe for the RPF's virtual mount point. Never changes mount behavior.
unsafe extern "C" fn packfile_mount_as(env: *mut Env, packfile: u64, mount_path: u64) -> u32 {
    DLC_RPF_MOUNT_COUNT.fetch_add(1, Ordering::Relaxed);
    let path = unsafe { Mem::of(env).str(mount_path) };
    remember_loader_path(&DLC_LAST_MOUNT_PATH, &path);
    let result = unsafe { orig::packfile_mount_as(env, packfile, mount_path) };
    log_line("vehicle-loader", LogLevel::Info,
        &format!("fiPackfile::MountAs object=0x{packfile:x} result={result} mount={path}"));
    result
}

/// Observe the game's metadata-to-RPF handoff; the data-file object remains engine-owned.
unsafe extern "C" fn datafile_add_rpf(env: *mut Env, data_file: u64, load: u32) -> u32 {
    DLC_DATAFILE_ADD_COUNT.fetch_add(1, Ordering::Relaxed);
    let result = unsafe { orig::datafile_add_rpf(env, data_file, load) };
    log_line("vehicle-loader", LogLevel::Info,
        &format!("CFileLoader::AddRpfFile data_file=0x{data_file:x} load={} result={result}", load != 0));
    result
}

/// Observe the game's bulk RPF registration phase without changing it.
unsafe extern "C" fn packfile_add_all_rpf_files(env: *mut Env, load: u32) {
    DLC_ADD_ALL_COUNT.fetch_add(1, Ordering::Relaxed);
    log_line("vehicle-loader", LogLevel::Info,
        &format!("CFileLoader::AddAllRpfFiles begin load={}", load != 0));
    unsafe { orig::packfile_add_all_rpf_files(env, load) };
    log_line("vehicle-loader", LogLevel::Info, "CFileLoader::AddAllRpfFiles complete");
}

/// Capture the engine-owned streaming module pointer for later ABI/layout research.
unsafe extern "C" fn packfile_get_streaming_module(env: *mut Env) -> u64 {
    DLC_STREAM_MODULE_COUNT.fetch_add(1, Ordering::Relaxed);
    let module = unsafe { orig::packfile_get_streaming_module(env) };
    if module != 0 {
        log_line("vehicle-loader", LogLevel::Info,
            &format!("strPackfileManager::GetStreamingModule module=0x{module:x}"));
    }
    module
}

/// Observe model streaming requests used before vehicle creation; forward unchanged.
unsafe extern "C" fn model_request_assets(env: *mut Env, model_id: u64, flags: u32) -> u32 {
    let result = unsafe { orig::model_request_assets(env, model_id, flags) };
    let sample = VEHICLE_MODEL_REQUEST_SAMPLES.fetch_add(1, Ordering::Relaxed);
    if sample % 64 == 0 {
        log_line("vehicle-loader", LogLevel::Info,
            &format!("CModelInfo::RequestAssets sample={sample} model_id=0x{model_id:x} flags={flags} result={result}"));
    }
    result
}

/// Observe whether a requested vehicle model has completed streaming.
unsafe extern "C" fn model_have_assets_loaded(env: *mut Env, model_id: u64) -> u32 {
    let result = unsafe { orig::model_have_assets_loaded(env, model_id) };
    let sample = VEHICLE_MODEL_READY_SAMPLES.fetch_add(1, Ordering::Relaxed);
    if sample % 64 == 0 || result != 0 {
        log_line("vehicle-loader", LogLevel::Info,
            &format!("CModelInfo::HaveAssetsLoaded sample={sample} model_id=0x{model_id:x} ready={result}"));
    }
    result
}

/// Observe the native factory's actual vehicle creation path. This hook never
/// creates a vehicle itself; it records exact runtime arguments and forwards.
unsafe extern "C" fn vehicle_factory_create(
    env: *mut Env,
    factory: u64,
    model_id: u64,
    owner: u32,
    flags: u32,
    matrix: u64,
    arg_a: u32,
    arg_b: u32,
) -> u64 {
    let count = VEHICLE_FACTORY_CREATE_COUNT.fetch_add(1, Ordering::Relaxed) + 1;
    log_line("vehicle-loader", LogLevel::Info,
        &format!("CVehicleFactory::Create count={count} factory=0x{factory:x} model_id=0x{model_id:x} owner={owner} flags={flags} matrix=0x{matrix:x} arg_a={arg_a} arg_b={arg_b}"));
    unsafe { orig::vehicle_factory_create(env, factory, model_id, owner, flags, matrix, arg_a, arg_b) }
}

/// Observe model-info resolution for requested vehicle models; never alter the lookup.
unsafe extern "C" fn model_get_base_info(env: *mut Env, model_id: u64) -> u64 {
    DLC_MODEL_LOOKUP_COUNT.fetch_add(1, Ordering::Relaxed);
    let result = unsafe { orig::model_get_base_info(env, model_id) };
    if result != 0 {
        log_line("vehicle-loader", LogLevel::Info,
            &format!("CModelInfo::GetBaseModelInfo model_id=0x{model_id:x} model_info=0x{result:x}"));
    }
    result
}

/// Observe model-file lookup and its streaming index; forward unchanged.
unsafe extern "C" fn stream_index_by_filename(env: *mut Env, filename: u64, flags: u32) -> u32 {
    DLC_STREAM_INDEX_COUNT.fetch_add(1, Ordering::Relaxed);
    let name = unsafe { Mem::of(env).str(filename) };
    let result = unsafe { orig::stream_index_by_filename(env, filename, flags) };
    if name.to_ascii_lowercase().ends_with(".yft") || name.to_ascii_lowercase().ends_with(".ydr") {
        log_line("vehicle-loader", LogLevel::Info,
            &format!("GetStreamingIndexByFilename flags={flags} result={result} file={name}"));
    }
    result
}

/// Sample streaming object requests so loader diagnostics do not write a
/// log line for every model/texture request during normal gameplay.
static STREAM_REQUEST_SAMPLES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
unsafe extern "C" fn stream_request_object(
    env: *mut Env,
    local_index: u32,
    flags: u32,
    priority: u32,
) -> u32 {
    DLC_STREAM_REQUEST_COUNT.fetch_add(1, Ordering::Relaxed);
    let result = unsafe {
        orig::stream_request_object(env, local_index, flags, priority)
    };
    let sample = STREAM_REQUEST_SAMPLES.fetch_add(1, Ordering::Relaxed);
    if sample % 256 == 0 {
        log_line("vehicle-loader", LogLevel::Info,
            &format!("CStreaming::RequestObject sample={} index={} flags={} priority={} result={}",
                sample, local_index, flags, priority, result));
    }
    result
}

/// Sample the engine's object-load path without changing its behavior.
/// The pinned wasm lowers this symbol to exactly two i32 arguments.
static STREAM_LOAD_SAMPLES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
unsafe extern "C" fn stream_load_object(env: *mut Env, local_index: u32, flags: u32) -> u32 {
    DLC_STREAM_LOAD_COUNT.fetch_add(1, Ordering::Relaxed);
    let result = unsafe { orig::stream_load_object(env, local_index, flags) };
    let sample = STREAM_LOAD_SAMPLES.fetch_add(1, Ordering::Relaxed);
    if sample % 256 == 0 {
        log_line("vehicle-loader", LogLevel::Info,
            &format!("CStreaming::LoadObject sample={} index={} flags={} result={}", sample, local_index, flags, result));
    }
    result
}

/// Observe individual-file streaming registrations; forward every argument unchanged.
unsafe extern "C" fn packfile_register_individual_file(
    env: *mut Env,
    manager: u64,
    is_dlc: u32,
    path: u64,
    flag_a: u32,
    flag_b: u32,
) -> u32 {
    DLC_REGISTER_FILE_COUNT.fetch_add(1, Ordering::Relaxed);
    let name = unsafe { Mem::of(env).str(path) };
    let result = unsafe {
        orig::packfile_register_individual_file(env, manager, is_dlc, path, flag_a, flag_b)
    };
    log_line("vehicle-loader", LogLevel::Info,
        &format!("RegisterIndividualFile manager=0x{manager:x} dlc={} flag_a={} flag_b={} result={} path={name}",
            is_dlc != 0, flag_a != 0, flag_b != 0, result));
    result
}

/// Observe device mount/unmount calls without taking ownership of engine devices.
unsafe extern "C" fn device_mount(env: *mut Env, path: u64, device: u64, create: u32) -> u32 {
    DLC_DEVICE_MOUNT_COUNT.fetch_add(1, Ordering::Relaxed);
    let name = unsafe { Mem::of(env).str(path) };
    let result = unsafe { orig::device_mount(env, path, device, create) };
    log_line("vehicle-loader", LogLevel::Info,
        &format!("fiDevice::Mount path={name} device=0x{device:x} create={} result={result}", create != 0));
    result
}

unsafe extern "C" fn device_unmount(env: *mut Env, path: u64) -> u32 {
    DLC_DEVICE_UNMOUNT_COUNT.fetch_add(1, Ordering::Relaxed);
    let name = unsafe { Mem::of(env).str(path) };
    let result = unsafe { orig::device_unmount(env, path) };
    log_line("vehicle-loader", LogLevel::Info,
        &format!("fiDevice::Unmount path={name} result={result}"));
    result
}

/// Observe the engine's exact-build relative-device initialization. This is
/// the RAGE VFS bridge candidate for exposing an extracted host directory.
unsafe extern "C" fn device_relative_init(
    env: *mut Env,
    device: u64,
    path: u64,
    allow_root: u32,
    base_device: u64,
) {
    let name = unsafe { Mem::of(env).str(path) };
    DLC_RELATIVE_INIT_COUNT.fetch_add(1, Ordering::Relaxed);
    remember_loader_path(&DLC_LAST_RELATIVE_ROOT, &name);
    unsafe { orig::device_relative_init(env, device, path, allow_root, base_device) };

    // Correlate the relative device with its resolved backing device only
    // after the engine has initialized it. This remains observational: no
    // object allocation, vtable mutation, mount, or ownership transfer.
    let low_level = unsafe { orig::device_relative_low_level(env, device) };
    let rpf_device = unsafe { orig::device_relative_rpf(env, device) };
    let debug_ptr = unsafe { orig::device_relative_debug_name(env, device) };
    let debug_name = if debug_ptr == 0 {
        String::new()
    } else {
        unsafe { Mem::of(env).str(debug_ptr) }
    };
    if base_device != 0 { DLC_RELATIVE_BASE_DEVICE_PRESENT.fetch_add(1, Ordering::Relaxed); }
    if low_level != 0 { DLC_RELATIVE_LOW_LEVEL_PRESENT.fetch_add(1, Ordering::Relaxed); }
    if rpf_device != 0 { DLC_RELATIVE_RPF_DEVICE_PRESENT.fetch_add(1, Ordering::Relaxed); }
    if !debug_name.is_empty() { DLC_RELATIVE_DEBUG_NAME_PRESENT.fetch_add(1, Ordering::Relaxed); }
    remember_loader_path(&DLC_LAST_RELATIVE_DEBUG_NAME, &debug_name);
    log_line("vehicle-loader", LogLevel::Info,
        &format!("fiDeviceRelative::InitResolved object=0x{device:x} root={name} allow_root={} base_device=0x{base_device:x} low_level=0x{low_level:x} rpf_device=0x{rpf_device:x} debug_name={debug_name}",
            allow_root != 0));
}

/// Observe the virtual mount name selected for each relative device.
unsafe extern "C" fn device_relative_mount_as(env: *mut Env, device: u64, mount: u64) -> u32 {
    let name = unsafe { Mem::of(env).str(mount) };
    DLC_RELATIVE_MOUNT_COUNT.fetch_add(1, Ordering::Relaxed);
    remember_loader_path(&DLC_LAST_RELATIVE_MOUNT, &name);
    let result = unsafe { orig::device_relative_mount_as(env, device, mount) };
    log_line("vehicle-loader", LogLevel::Info,
        &format!("fiDeviceRelative::MountAs object=0x{device:x} mount={name} result={result}"));
    result
}

/// Observe path resolution through the engine's global device registry.
unsafe extern "C" fn device_get_device(env: *mut Env, path: u64, create: u32) -> u64 {
    let name = unsafe { Mem::of(env).str(path) };
    let result = unsafe { orig::device_get_device(env, path, create) };
    log_line("vehicle-loader", LogLevel::Info,
        &format!("fiDevice::GetDevice path={name} create={} result=0x{result:x}", create != 0));
    result
}

/// Identify the underlying host device selected by each relative RAGE device.
unsafe extern "C" fn device_relative_low_level(env: *mut Env, device: u64) -> u64 {
    let result = unsafe { orig::device_relative_low_level(env, device) };
    log_line("vehicle-loader", LogLevel::Info,
        &format!("fiDeviceRelative::GetLowLevelDevice object=0x{device:x} result=0x{result:x}"));
    result
}

/// Identify whether a relative device resolves to an RPF-backed device.
unsafe extern "C" fn device_relative_rpf(env: *mut Env, device: u64) -> u64 {
    let result = unsafe { orig::device_relative_rpf(env, device) };
    log_line("vehicle-loader", LogLevel::Info,
        &format!("fiDeviceRelative::GetRpfDevice object=0x{device:x} result=0x{result:x}"));
    result
}


/// Read the engine-owned debug name for a relative device.
unsafe extern "C" fn device_relative_debug_name(env: *mut Env, device: u64) -> u64 {
    let result = unsafe { orig::device_relative_debug_name(env, device) };
    let name = if result == 0 { String::new() } else { unsafe { Mem::of(env).str(result) } };
    remember_loader_path(&DLC_LAST_RELATIVE_DEBUG_NAME, &name);
    log_line("vehicle-loader", LogLevel::Info,
        &format!("fiDeviceRelative::GetDebugName object=0x{device:x} name={name}"));
    result
}

/// Trace DLC-relevant file opens without logging normal texture traffic.
unsafe extern "C" fn device_relative_open(env: *mut Env, device: u64, path: u64, read_only: u32) -> u64 {
    let name = unsafe { Mem::of(env).str(path) };
    let lower = name.to_ascii_lowercase();
    let interesting = lower.ends_with(".rpf") || lower.ends_with("setup2.xml")
        || lower.ends_with("content.xml") || lower.ends_with("dlclist.xml")
        || lower.ends_with(".meta");
    DLC_RELATIVE_OPEN_COUNT.fetch_add(1, Ordering::Relaxed);
    if lower.ends_with(".rpf") { DLC_RELATIVE_RPF_OPEN_COUNT.fetch_add(1, Ordering::Relaxed); }
    if interesting { remember_loader_path(&DLC_LAST_RELATIVE_OPEN, &name); }
    let result = unsafe { orig::device_relative_open(env, device, path, read_only) };
    if interesting {
        log_line("vehicle-loader", LogLevel::Info,
            &format!("fiDeviceRelative::Open object=0x{device:x} read_only={} handle=0x{result:x} path={name}",
                read_only != 0));
    }
    result
}

/// Observe directory enumeration requests used to discover DLC folders.
unsafe extern "C" fn device_relative_find_begin(env: *mut Env, device: u64, path: u64, find_data: u64) -> u64 {
    let name = unsafe { Mem::of(env).str(path) };
    DLC_RELATIVE_FIND_COUNT.fetch_add(1, Ordering::Relaxed);
    remember_loader_path(&DLC_LAST_RELATIVE_ENUM, &name);
    let result = unsafe { orig::device_relative_find_begin(env, device, path, find_data) };
    log_line("vehicle-loader", LogLevel::Info,
        &format!("fiDeviceRelative::FindFileBegin object=0x{device:x} iterator=0x{result:x} path={name}"));
    result
}

/// Capture file-size queries for RPF and DLC metadata paths.
unsafe extern "C" fn device_relative_file_size(env: *mut Env, device: u64, path: u64) -> u64 {
    let name = unsafe { Mem::of(env).str(path) };
    DLC_RELATIVE_SIZE_COUNT.fetch_add(1, Ordering::Relaxed);
    let result = unsafe { orig::device_relative_file_size(env, device, path) };
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".rpf") || lower.ends_with("setup2.xml")
        || lower.ends_with("content.xml") || lower.ends_with(".meta")
    {
        log_line("vehicle-loader", LogLevel::Info,
            &format!("fiDeviceRelative::GetFileSize object=0x{device:x} size={result} path={name}"));
    }
    result
}

/// The program registry's slots (`scrProgram::GetProgram`): 176 u64.
const PROGRAMS: u64 = 15406680;
const PROGRAM_SLOTS: u64 = 176;
/// The program size we copy from a loaded one; the fields set below are
/// all the engine reads to start and run a thread.
const PROGRAM_SIZE: u64 = 128;
/// scrThread fields.
const THREAD_STATE: u64 = 32;
const STATE_WAITING: u32 = 1;
/// The stack size of the mods' threads, one of gameconfig.xml's
/// (MICRO): a mod keeps its state in Lua.
const STACK: u32 = 128;
/// Opcodes (the b2802+ table).
const OP_ENTER: u8 = 45;
const OP_NATIVE: u8 = 44;
const OP_PUSH_0: u8 = 113;
const OP_J: u8 = 85;

struct Running {
    name: String,
    thread_id: u32,
    thread: u64,
    vm: Vm,
    /// Lua ended (or failed): the thread is killed from the next wait
    /// outside it.
    ending: bool,
    cpu: f32,
    waiting: bool,
}

#[derive(Default)]
struct Engine {
    /// The program text and natives table every mod program shares.
    code: Option<(u64, u64)>,
    /// Program id by mod name.
    programs: HashMap<String, u32>,
    running: Vec<Running>,
    /// The REPL's snippets.
    jobs: Jobs,
    /// The catalog of names was read (once the game has loaded), and when
    /// that was last checked.
    catalog: bool,
    catalog_checked: Option<Instant>,
}

thread_local! {
    static ENGINE: RefCell<Engine> = RefCell::default();
}

unsafe extern "C" fn script_wait(env: *mut Env, info: u64) {
    let killed = ENGINE.with(|engine| {
        // A mod's native that waits would come back here: not for us then.
        let Ok(mut engine) = engine.try_borrow_mut() else { return false };
        unsafe { engine.wait(env, info) }
    });
    if !killed {
        unsafe { orig::script_wait(env, info) }
    }
}

impl Engine {
    /// Returns whether the active thread was killed (WAIT must not run).
    unsafe fn wait(&mut self, env: *mut Env, info: u64) -> bool {
        let mem = unsafe { Mem::of(env) };
        let active = unsafe { orig::script_active_thread(env) };
        if SHARED.pending.swap(false, Ordering::Acquire) {
            let commands = std::mem::take(&mut SHARED.state.lock().unwrap().commands);
            for command in commands {
                unsafe { self.command(env, command) };
            }
        }
        if !self.catalog && self.catalog_checked.is_none_or(|t| t.elapsed().as_secs() >= 1) {
            self.catalog_checked = Some(Instant::now());
            let ready = unsafe {
                natives::call_int(env, "GET_IS_LOADING_SCREEN_ACTIVE") == Some(0)
                    && natives::call_int(env, "IS_SCREEN_FADED_IN") == Some(1)
                    && natives::call_int(env, "PLAYER_PED_ID").is_some_and(|p| p != 0)
            };
            if ready {
                self.catalog = true;
                crate::world::set_catalog(unsafe { crate::world::read_catalog(env) });
            }
        }
        // Ended mods' threads, from outside them.
        let mut i = 0;
        while i < self.running.len() {
            let r = &self.running[i];
            if r.ending && r.thread != active {
                unsafe { orig::script_kill_thread(env, r.thread_id) };
                crate::menus::remove_mod(&r.name);
                self.running.remove(i);
            } else {
                i += 1;
            }
        }
        let Some(r) = self.running.iter_mut().find(|r| r.thread == active) else { return false };
        // WAIT runs every frame while the thread waits; the mod runs when
        // it is about to start one.
        if r.ending || mem.u32(active + THREAD_STATE) == STATE_WAITING {
            return false;
        }
        let start = Instant::now();
        let now = unsafe { natives::call_int(env, "GET_GAME_TIMER") }.unwrap_or(0) as u32 as u64;
        *crate::RUNNING_MOD.lock().unwrap() = r.name.clone();
        let step = natives::with_env(env, || r.vm.step(now));
        crate::RUNNING_MOD.lock().unwrap().clear();
        r.cpu = r.cpu * 0.9 + start.elapsed().as_secs_f32() * 1000.0 * 0.1;
        let ms = match step {
            Step::Wait(ms) => {
                if r.waiting != (ms > 0) {
                    r.waiting = ms > 0;
                    set_waiting(&r.name, r.waiting);
                }
                ms
            }
            Step::Finished => {
                log_line(&r.name, LogLevel::Info, "finished");
                set_state(&r.name, ModState::Stopped, None);
                r.ending = true;
                0
            }
            Step::Failed(e) => {
                log_line(&r.name, LogLevel::Error, &e);
                set_state(&r.name, ModState::Error, Some(e));
                r.ending = true;
                0
            }
        };
        // WAIT's argument, in the thread's stack.
        mem.set_i32(mem.u64(info + 16), ms.min(i32::MAX as u32) as i32);
        if let Ok(mut s) = SHARED.state.lock()
            && let Some(m) = s.mods.iter_mut().find(|m| m.name == r.name)
        {
            m.cpu_ms = r.cpu;
            m.memory = r.vm.memory();
        }
        false
    }

    unsafe fn command(&mut self, env: *mut Env, command: Command) {
        match command {
            Command::Start(name) => {
                if self.running.iter().any(|r| r.name == name && !r.ending) {
                    return;
                }
                if let Err(e) = unsafe { self.start(env, &name) } {
                    log_line(&name, LogLevel::Error, &e);
                    set_state(&name, ModState::Error, Some(e));
                }
            }
            Command::Eval(code, reply) => {
                if !self.running.iter().any(|r| r.name == REPL && !r.ending)
                    && let Err(e) = unsafe { self.start(env, REPL) }
                {
                    let _ = reply.send(Err(e));
                    return;
                }
                self.jobs.borrow_mut().push_back((code, reply));
            }
            Command::Stop(name) => {
                if let Some(r) = self.running.iter_mut().find(|r| r.name == name) {
                    r.ending = true;
                    log_line(&name, LogLevel::Info, "stopped");
                    set_state(&name, ModState::Stopped, None);
                }
            }
        }
    }

    unsafe fn start(&mut self, env: *mut Env, name: &str) -> Result<(), String> {
        let dir = SHARED.dir.get().ok_or("no mods folder")?.join(name);
        let log_name = if name == REPL { "eval".to_string() } else { name.to_string() };
        let log: Rc<dyn Fn(&str)> = Rc::new(move |text| log_line(&log_name, LogLevel::Info, text));
        let vm = if name == REPL { Vm::repl(self.jobs.clone(), log) } else { Vm::new(name, &dir, log) }
            .map_err(|e| e.to_string())?;
        let program = unsafe { self.program(env, name)? };
        let thread_name = unsafe { wasm_string(env, &format!("glue5:{name}")) };
        let thread_id = unsafe { orig::script_start_program(env, program, 0, 0, STACK as i32 as u32, thread_name) };
        if thread_id == 0 {
            return Err("the game has no free script thread for it".into());
        }
        let thread = unsafe { orig::script_get_thread(env, thread_id) };
        if thread == 0 {
            return Err("its script thread is gone".into());
        }
        self.running.push(Running {
            name: name.to_string(),
            thread_id,
            thread,
            vm,
            ending: false,
            cpu: 0.0,
            waiting: false,
        });
        if name != REPL {
            log_line(name, LogLevel::Info, "started");
            set_state(name, ModState::Running, None);
        }
        Ok(())
    }

    /// The program of mod `name`'s thread (its name is the thread's), made
    /// once: a copy of a loaded program with our code, natives and name.
    unsafe fn program(&mut self, env: *mut Env, name: &str) -> Result<u32, String> {
        if let Some(&id) = self.programs.get(name) {
            return Ok(id);
        }
        let mem = unsafe { Mem::of(env) };
        let template =
            (0..PROGRAM_SLOTS).map(|i| mem.u64(PROGRAMS + 8 * i)).find(|&p| p != 0).ok_or("no script program is loaded yet")?;
        let (pages, natives) = match self.code {
            Some(code) => code,
            None => {
                let wait = natives::reserved_handler("WAIT").ok_or("no WAIT native")?;
                // ENTER 0 params, 2 locals, no name; loop: PUSH_0; NATIVE
                // 1 argument, no result, natives[0]; J loop.
                let code: [u8; 13] = [OP_ENTER, 0, 2, 0, 0, OP_PUSH_0, OP_NATIVE, 1 << 2, 0, 0, OP_J, 0xF8, 0xFF];
                let text = unsafe { malloc(env, 64)? };
                mem.bytes_mut(text, 64).fill(0);
                mem.bytes_mut(text, code.len()).copy_from_slice(&code);
                let pages = unsafe { malloc(env, 8)? };
                mem.set_u64(pages, text);
                let natives = unsafe { malloc(env, 8)? };
                mem.set_u64(natives, wait);
                self.code = Some((pages, natives));
                (pages, natives)
            }
        };
        let id = natives::joaat(&format!("glue5:{name}"));
        let program = unsafe { malloc(env, PROGRAM_SIZE)? };
        mem.bytes_mut(program, PROGRAM_SIZE as usize).copy_from_slice(mem.bytes(template, PROGRAM_SIZE as usize));
        mem.set_u64(program + 16, pages); // code pages
        mem.set_u32(program + 28, 13); // code size
        mem.set_u32(program + 36, 0); // statics
        mem.set_u64(program + 48, 0);
        mem.set_u32(program + 44, 1); // natives
        mem.set_u64(program + 64, natives);
        mem.set_u32(program + 72, 0);
        mem.set_u32(program + 88, id);
        mem.set_u32(program + 92, 1 << 24); // references: never released
        mem.set_u64(program + 96, unsafe { wasm_string(env, &format!("glue5:{name}")) });
        mem.set_u64(program + 104, 0); // strings
        mem.set_u32(program + 112, 0);
        unsafe { orig::script_insert_program(env, program) };
        if unsafe { orig::script_get_program(env, id) } != program {
            return Err("the game's program registry is full".into());
        }
        self.programs.insert(name.to_string(), id);
        Ok(id)
    }
}

unsafe fn malloc(env: *mut Env, size: u64) -> Result<u64, String> {
    match unsafe { orig::builtin_malloc(env, size) } {
        0 => Err("out of game memory".into()),
        at => Ok(at),
    }
}

/// A NUL-terminated copy of `s` in the game's heap (kept: names of
/// programs and threads).
unsafe fn wasm_string(env: *mut Env, s: &str) -> u64 {
    let Ok(at) = (unsafe { malloc(env, s.len() as u64 + 1) }) else { return 0 };
    unsafe { Mem::of(env) }.write_str(at, s, s.len() + 1);
    at
}

fn set_state(name: &str, state: ModState, error: Option<String>) {
    if let Ok(mut s) = SHARED.state.lock()
        && let Some(m) = s.mods.iter_mut().find(|m| m.name == name)
    {
        m.state = state;
        m.error = error;
        m.waiting = false;
        if state != ModState::Running {
            m.cpu_ms = 0.0;
        }
        SHARED.generation.fetch_add(1, Ordering::Relaxed);
    }
}

fn set_waiting(name: &str, waiting: bool) {
    if let Ok(mut s) = SHARED.state.lock()
        && let Some(m) = s.mods.iter_mut().find(|m| m.name == name)
    {
        m.waiting = waiting;
        SHARED.generation.fetch_add(1, Ordering::Relaxed);
    }
}

/// Observe the engine's own CMountableContent construction to identify its
/// lifetime and ownership before any runtime DLC mounting is attempted.
/// This hook forwards the constructor unchanged and never allocates an object.
unsafe extern "C" fn mountable_content_ctor(env: *mut Env, object: u64) -> u64 {
    DLC_CTOR_COUNT.fetch_add(1, Ordering::Relaxed);
    let result = unsafe { orig::mountable_content_ctor(env, object) };
    log_line("vehicle-loader", LogLevel::Info,
        &format!("CMountableContent::ctor object=0x{object:x} result=0x{result:x}"));
    result
}

/// Trace the engine-owned setup XML read; the object and path stay engine-owned.
unsafe extern "C" fn mountable_content_load_setup(env: *mut Env, content: u64) -> u32 {
    DLC_SETUP_XML_COUNT.fetch_add(1, Ordering::Relaxed);
    let result = unsafe { orig::mountable_content_load_setup(env, content) };
    log_line("vehicle-loader", LogLevel::Info,
        &format!("CMountableContent::LoadSetupXML object=0x{content:x} result={result}"));
    result
}

/// Observe the native content mount decision without initiating one ourselves.
unsafe extern "C" fn mountable_content_mount(env: *mut Env, content: u64) -> u32 {
    DLC_CONTENT_MOUNT_COUNT.fetch_add(1, Ordering::Relaxed);
    let result = unsafe { orig::mountable_content_mount(env, content) };
    if result != 0 { DLC_CONTENT_MOUNT_SUCCESS_COUNT.fetch_add(1, Ordering::Relaxed); }
    else { DLC_CONTENT_MOUNT_FAILURE_COUNT.fetch_add(1, Ordering::Relaxed); }
    log_line("vehicle-loader", LogLevel::Info,
        &format!("CMountableContent::Mount object=0x{content:x} result={result}"));
    result
}

/// Trace per-pack mount requests and forward all arguments unchanged.
unsafe extern "C" fn mountable_content_mount_content(
    env: *mut Env, content: u64, path: u64
) -> u32 {
    DLC_CONTENT_MOUNT_CONTENT_COUNT.fetch_add(1, Ordering::Relaxed);
    let name = unsafe { Mem::of(env).str(path) };
    let result = unsafe { orig::mountable_content_mount_content(env, content, path) };
    if result != 0 { DLC_CONTENT_MOUNT_CONTENT_SUCCESS_COUNT.fetch_add(1, Ordering::Relaxed); }
    else { DLC_CONTENT_MOUNT_CONTENT_FAILURE_COUNT.fetch_add(1, Ordering::Relaxed); }
    log_line("vehicle-loader", LogLevel::Info,
        &format!("CMountableContent::MountContent object=0x{content:x} result={result} path={name}"));
    result
}

/// Trace the mountable-content execution path selected by the game.
unsafe extern "C" fn mountable_content_execute_changeset(
    env: *mut Env, content: u64, group: u64, change_set: u64
) {
    log_line("vehicle-loader", LogLevel::Info,
        &format!("CMountableContent::ExecuteContentChangeSet content=0x{content:x} group=0x{group:x} change_set=0x{change_set:x}"));
    unsafe { orig::mountable_content_execute_changeset(env, content, group, change_set) };
}

/// Trace rollback for a specific mountable-content change set.
unsafe extern "C" fn mountable_content_revert_changeset(
    env: *mut Env, content: u64, group: u64, change_set: u64, flags: u32
) {
    log_line("vehicle-loader", LogLevel::Info,
        &format!("CMountableContent::RevertContentChangeSet content=0x{content:x} group=0x{group:x} change_set=0x{change_set:x} flags={flags}"));
    unsafe { orig::mountable_content_revert_changeset(env, content, group, change_set, flags) };
}

/// Observe unmount calls; no content is unmounted by the diagnostics layer.
unsafe extern "C" fn mountable_content_unmount(env: *mut Env, content: u64, path: u64) {
    DLC_CONTENT_UNMOUNT_COUNT.fetch_add(1, Ordering::Relaxed);
    let name = unsafe { Mem::of(env).str(path) };
    log_line("vehicle-loader", LogLevel::Info,
        &format!("CMountableContent::UnmountContent object=0x{content:x} path={name}"));
    unsafe { orig::mountable_content_unmount(env, content, path) };
}

/// The engine owns this object. Remember only the address observed at the
/// real registration/load boundary; never construct, retain ownership of, or
/// invoke methods on a guessed CExtraContentManager instance.
/// Trace the manager's per-content change-set dispatch. Engine objects remain engine-owned.
unsafe extern "C" fn extra_content_execute_internal(
    env: *mut Env, manager: u64, content: u64, change_set: u64
) {
    log_line("vehicle-loader", LogLevel::Info,
        &format!("CExtraContentManager::ExecuteContentChangeSetGroupInternal manager=0x{manager:x} content=0x{content:x} change_set=0x{change_set:x}"));
    unsafe { orig::extra_content_execute_internal(env, manager, content, change_set) };
}

/// Observe the actual data-file change-set execution boundary and forward unchanged.
unsafe extern "C" fn fileloader_execute_changeset(env: *mut Env, change_set: u64) {
    log_line("vehicle-loader", LogLevel::Info,
        &format!("CFileLoader::ExecuteContentChangeSet change_set=0x{change_set:x}"));
    unsafe { orig::fileloader_execute_changeset(env, change_set) };
}

/// Observe rollback/unmount scheduling for a change set; never initiate it.
unsafe extern "C" fn fileloader_revert_changeset(
    env: *mut Env, change_set: u64, flags: u32
) {
    log_line("vehicle-loader", LogLevel::Info,
        &format!("CFileLoader::RevertContentChangeSet change_set=0x{change_set:x} flags={flags}"));
    unsafe { orig::fileloader_revert_changeset(env, change_set, flags) };
}

/// Observe the engine's full per-content change-set dispatch; never initiate a change.
unsafe extern "C" fn extra_content_execute_changeset(
    env: *mut Env, manager: u64, content: u64, change_set_group: u64, change_set: u64, flags: u32
) {
    log_line("vehicle-loader", LogLevel::Info,
        &format!("CExtraContentManager::ExecuteContentChangeSetInternal manager=0x{manager:x} content=0x{content:x} group=0x{change_set_group:x} change_set=0x{change_set:x} flags={flags}"));
    unsafe { orig::extra_content_execute_changeset(env, manager, content, change_set_group, change_set, flags) };
}

/// Observe archetype post-load registration without altering model metadata.
unsafe extern "C" fn model_post_load(env: *mut Env, archetype: u64) {
    log_line("vehicle-loader", LogLevel::Info,
        &format!("CModelInfo::PostLoad archetype=0x{archetype:x}"));
    unsafe { orig::model_post_load(env, archetype) };
}

/// Observe model-info resolution from a streaming-local index.
unsafe extern "C" fn model_get_local_info(env: *mut Env, local_index: u32) -> u64 {
    let result = unsafe { orig::model_get_local_info(env, local_index) };
    if result != 0 {
        log_line("vehicle-loader", LogLevel::Info,
            &format!("CModelInfo::GetModelInfoFromLocalIndex index={local_index} model_info=0x{result:x}"));
    }
    result
}

/// Observe the native DLC archetype/ityp data-file unload boundary.
unsafe extern "C" fn dlc_ittyp_unload_datafile(env: *mut Env, mounter: u64, data_file: u64) {
    log_line("vehicle-loader", LogLevel::Info,
        &format!("CDLCItypFileMounter::UnloadDataFile mounter=0x{mounter:x} data_file=0x{data_file:x}"));
    unsafe { orig::dlc_ittyp_unload_datafile(env, mounter, data_file) };
}

/// Trace the native DLC archetype/ityp data-file mounter used by vehicle assets.
unsafe extern "C" fn dlc_ittyp_load_datafile(
    env: *mut Env, mounter: u64, data_file: u64
) -> u32 {
    let result = unsafe { orig::dlc_ittyp_load_datafile(env, mounter, data_file) };
    log_line("vehicle-loader", LogLevel::Info,
        &format!("CDLCItypFileMounter::LoadDataFile mounter=0x{mounter:x} data_file=0x{data_file:x} result={result}"));
    result
}

static EXTRA_CONTENT_MANAGER: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

// Bounded, pointer-free counters exposed to the AI Car Hub. These tell us
// whether the game's own DLC lifecycle is reached without retaining engine
// objects or attempting unsafe calls into undocumented methods.
static DLC_RPF_LOAD_COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static DLC_RPF_INIT_COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static DLC_RPF_MOUNT_COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static DLC_DATAFILE_ADD_COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static DLC_ADD_ALL_COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static DLC_STREAM_MODULE_COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static DLC_REGISTER_FILE_COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static DLC_STREAM_INDEX_COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static DLC_STREAM_REQUEST_COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static DLC_STREAM_LOAD_COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static DLC_MODEL_LOOKUP_COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static VEHICLE_MODEL_REQUEST_SAMPLES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static VEHICLE_MODEL_READY_SAMPLES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static VEHICLE_FACTORY_CREATE_COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static DLC_DEVICE_MOUNT_COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static DLC_DEVICE_UNMOUNT_COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static DLC_CTOR_COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static DLC_ADD_FOLDER_COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static DLC_LOAD_COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static DLC_LOAD_MATCH_COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static DLC_DEVICE_LOOKUP_COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static DLC_HASH_LOOKUP_COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static DLC_HASH_LOOKUP_HIT_COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static DLC_ENUM_COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static DLC_LAST_IDENTITY_MATCH: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static DLC_LAST_RPF_PATH: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());
static DLC_LAST_MOUNT_PATH: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());
static DLC_LAST_CONTENT_FOLDER: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());
static DLC_BEGIN_ENUM_COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static DLC_UPDATE_ARRAY_COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static DLC_EXECUTE_GROUP_COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static DLC_MANAGER_DTOR_COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static DLC_CONTENT_SHUTDOWN_COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static DLC_SETUP_XML_COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static DLC_CONTENT_MOUNT_COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static DLC_CONTENT_MOUNT_CONTENT_COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static DLC_CONTENT_MOUNT_SUCCESS_COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static DLC_CONTENT_MOUNT_FAILURE_COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static DLC_CONTENT_MOUNT_CONTENT_SUCCESS_COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static DLC_CONTENT_MOUNT_CONTENT_FAILURE_COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static DLC_CONTENT_UNMOUNT_COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static DLC_RELATIVE_INIT_COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static DLC_RELATIVE_MOUNT_COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static DLC_RELATIVE_OPEN_COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static DLC_RELATIVE_FIND_COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static DLC_RELATIVE_SIZE_COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static DLC_RELATIVE_BASE_DEVICE_PRESENT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static DLC_RELATIVE_LOW_LEVEL_PRESENT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static DLC_RELATIVE_RPF_DEVICE_PRESENT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static DLC_RELATIVE_DEBUG_NAME_PRESENT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static DLC_RELATIVE_RPF_OPEN_COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static DLC_LAST_RELATIVE_ROOT: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());
static DLC_LAST_RELATIVE_MOUNT: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());
static DLC_LAST_RELATIVE_OPEN: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());
static DLC_LAST_RELATIVE_ENUM: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());
static DLC_LAST_RELATIVE_DEBUG_NAME: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());

fn remember_loader_path(slot: &std::sync::Mutex<String>, path: &str) {
    if let Ok(mut value) = slot.lock() {
        value.clear();
        value.push_str(&path.chars().take(192).collect::<String>());
    }
}

/// Return safe lifecycle counters; raw engine addresses are intentionally omitted.
pub(crate) fn vehicle_loader_diagnostics() -> serde_json::Value {
    use std::sync::atomic::Ordering::Relaxed;
    serde_json::json!({
        "constructor_calls": DLC_CTOR_COUNT.load(Relaxed),
        "add_folder_calls": DLC_ADD_FOLDER_COUNT.load(Relaxed),
        "load_calls": DLC_LOAD_COUNT.load(Relaxed),
        "load_identity_matches": DLC_LOAD_MATCH_COUNT.load(Relaxed),
        "last_load_identity_match": DLC_LAST_IDENTITY_MATCH.load(Relaxed),
        "device_lookups": DLC_DEVICE_LOOKUP_COUNT.load(Relaxed),
        "content_hash_lookups": DLC_HASH_LOOKUP_COUNT.load(Relaxed),
        "content_hash_hits": DLC_HASH_LOOKUP_HIT_COUNT.load(Relaxed),
        "recursive_enumerations": DLC_ENUM_COUNT.load(Relaxed),
        "rpf_load_calls": DLC_RPF_LOAD_COUNT.load(Relaxed),
        "rpf_init_calls": DLC_RPF_INIT_COUNT.load(Relaxed),
        "rpf_mount_calls": DLC_RPF_MOUNT_COUNT.load(Relaxed),
        "datafile_add_calls": DLC_DATAFILE_ADD_COUNT.load(Relaxed),
        "add_all_rpf_calls": DLC_ADD_ALL_COUNT.load(Relaxed),
        "stream_module_calls": DLC_STREAM_MODULE_COUNT.load(Relaxed),
        "register_file_calls": DLC_REGISTER_FILE_COUNT.load(Relaxed),
        "stream_index_calls": DLC_STREAM_INDEX_COUNT.load(Relaxed),
        "stream_request_calls": DLC_STREAM_REQUEST_COUNT.load(Relaxed),
        "stream_load_calls": DLC_STREAM_LOAD_COUNT.load(Relaxed),
        "model_lookup_calls": DLC_MODEL_LOOKUP_COUNT.load(Relaxed),
        "device_mount_calls": DLC_DEVICE_MOUNT_COUNT.load(Relaxed),
        "device_unmount_calls": DLC_DEVICE_UNMOUNT_COUNT.load(Relaxed),
        "manager_observed": EXTRA_CONTENT_MANAGER.load(Relaxed) != 0,
        "begin_enumerate_calls": DLC_BEGIN_ENUM_COUNT.load(Relaxed),
        "update_content_array_calls": DLC_UPDATE_ARRAY_COUNT.load(Relaxed),
        "execute_change_group_calls": DLC_EXECUTE_GROUP_COUNT.load(Relaxed),
        "manager_destructor_calls": DLC_MANAGER_DTOR_COUNT.load(Relaxed),
        "mountable_shutdown_calls": DLC_CONTENT_SHUTDOWN_COUNT.load(Relaxed),
        "setup_xml_calls": DLC_SETUP_XML_COUNT.load(Relaxed),
        "content_mount_calls": DLC_CONTENT_MOUNT_COUNT.load(Relaxed),
        "content_mount_content_calls": DLC_CONTENT_MOUNT_CONTENT_COUNT.load(Relaxed),
        "content_mount_successes": DLC_CONTENT_MOUNT_SUCCESS_COUNT.load(Relaxed),
        "content_mount_failures": DLC_CONTENT_MOUNT_FAILURE_COUNT.load(Relaxed),
        "content_mount_content_successes": DLC_CONTENT_MOUNT_CONTENT_SUCCESS_COUNT.load(Relaxed),
        "content_mount_content_failures": DLC_CONTENT_MOUNT_CONTENT_FAILURE_COUNT.load(Relaxed),
        "content_unmount_calls": DLC_CONTENT_UNMOUNT_COUNT.load(Relaxed),
        "relative_device_init_calls": DLC_RELATIVE_INIT_COUNT.load(Relaxed),
        "relative_device_mount_calls": DLC_RELATIVE_MOUNT_COUNT.load(Relaxed),
        "relative_device_open_calls": DLC_RELATIVE_OPEN_COUNT.load(Relaxed),
        "relative_device_rpf_open_calls": DLC_RELATIVE_RPF_OPEN_COUNT.load(Relaxed),
        "relative_device_find_calls": DLC_RELATIVE_FIND_COUNT.load(Relaxed),
        "relative_device_size_calls": DLC_RELATIVE_SIZE_COUNT.load(Relaxed),
        "relative_base_device_present": DLC_RELATIVE_BASE_DEVICE_PRESENT.load(Relaxed),
        "relative_low_level_device_present": DLC_RELATIVE_LOW_LEVEL_PRESENT.load(Relaxed),
        "relative_rpf_device_present": DLC_RELATIVE_RPF_DEVICE_PRESENT.load(Relaxed),
        "relative_debug_name_present": DLC_RELATIVE_DEBUG_NAME_PRESENT.load(Relaxed),
        "last_relative_root": DLC_LAST_RELATIVE_ROOT.lock().map(|v| v.clone()).unwrap_or_default(),
        "last_relative_mount": DLC_LAST_RELATIVE_MOUNT.lock().map(|v| v.clone()).unwrap_or_default(),
        "last_relative_open": DLC_LAST_RELATIVE_OPEN.lock().map(|v| v.clone()).unwrap_or_default(),
        "last_relative_enum": DLC_LAST_RELATIVE_ENUM.lock().map(|v| v.clone()).unwrap_or_default(),
        "last_relative_debug_name": DLC_LAST_RELATIVE_DEBUG_NAME.lock().map(|v| v.clone()).unwrap_or_default(),
        "last_rpf_path": DLC_LAST_RPF_PATH.lock().map(|v| v.clone()).unwrap_or_default(),
        "last_mount_path": DLC_LAST_MOUNT_PATH.lock().map(|v| v.clone()).unwrap_or_default(),
        "last_content_folder": DLC_LAST_CONTENT_FOLDER.lock().map(|v| v.clone()).unwrap_or_default()
    })
}

/// Trace engine-owned folder registration and identify the manager instance.
unsafe extern "C" fn extra_content_add_folder(env: *mut Env, manager: u64, folder: u64) {
    DLC_ADD_FOLDER_COUNT.fetch_add(1, Ordering::Relaxed);
    let path = unsafe { Mem::of(env).str(folder) };
    remember_loader_path(&DLC_LAST_CONTENT_FOLDER, &path);
    if manager != 0 {
        let previous = EXTRA_CONTENT_MANAGER.swap(manager, Ordering::AcqRel);
        if previous != manager {
            log_line("vehicle-loader", LogLevel::Info,
                &format!("extra-content manager candidate changed old=0x{previous:x} new=0x{manager:x}"));
        }
    }
    log_line("vehicle-loader", LogLevel::Info,
        &format!("CExtraContentManager::AddContentFolder manager=0x{manager:x} folder={path}"));
    unsafe { orig::extra_content_add_folder(env, manager, folder) };
}

/// Confirm the manager that actually reaches the engine's load boundary.
/// This is lifecycle evidence only; no mount method is called from the hook.
unsafe extern "C" fn extra_content_load(env: *mut Env, manager: u64, load_a: u32, load_b: u32) {
    DLC_LOAD_COUNT.fetch_add(1, Ordering::Relaxed);
    let candidate = EXTRA_CONTENT_MANAGER.load(Ordering::Acquire);
    let matches = manager != 0 && manager == candidate;
    DLC_LAST_IDENTITY_MATCH.store(matches, Ordering::Relaxed);
    if matches { DLC_LOAD_MATCH_COUNT.fetch_add(1, Ordering::Relaxed); }
    log_line("vehicle-loader", LogLevel::Info,
        &format!("CExtraContentManager::LoadContent begin manager=0x{manager:x} candidate=0x{candidate:x} identity_match={matches} flags={load_a},{load_b}"));
    unsafe { orig::extra_content_load(env, manager, load_a, load_b) };
    log_line("vehicle-loader", LogLevel::Info,
        &format!("CExtraContentManager::LoadContent complete manager=0x{manager:x} identity_match={matches}"));
}

/// Observe the engine's device-to-content lookup. The manager remains
/// engine-owned; this records the device key and returned content pointer.
unsafe extern "C" fn extra_content_get_by_device(env: *mut Env, manager: u64, device: u64) -> u64 {
    DLC_DEVICE_LOOKUP_COUNT.fetch_add(1, Ordering::Relaxed);
    let name = unsafe { Mem::of(env).str(device) };
    let result = unsafe { orig::extra_content_get_by_device(env, manager, device) };
    log_line("vehicle-loader", LogLevel::Info,
        &format!("CExtraContentManager::GetContentByDevice manager=0x{manager:x} device={name} result=0x{result:x}"));
    result
}

/// Observe DLC content-hash resolution; never changes the manager's lookup.
unsafe extern "C" fn extra_content_get_by_hash(env: *mut Env, manager: u64, hash: u32) -> u64 {
    DLC_HASH_LOOKUP_COUNT.fetch_add(1, Ordering::Relaxed);
    let result = unsafe { orig::extra_content_get_by_hash(env, manager, hash) };
    if result != 0 {
        DLC_HASH_LOOKUP_HIT_COUNT.fetch_add(1, Ordering::Relaxed);
        log_line("vehicle-loader", LogLevel::Info,
            &format!("CExtraContentManager::GetContentByHash hit manager=0x{manager:x} hash=0x{hash:08x} content=0x{result:x}"));
    } else {
        let sample = DLC_HASH_LOOKUP_COUNT.load(Ordering::Relaxed);
        if sample <= 32 || sample % 128 == 0 {
            log_line("vehicle-loader", LogLevel::Info,
                &format!("CExtraContentManager::GetContentByHash miss sample={sample} manager=0x{manager:x} hash=0x{hash:08x}"));
        }
    }
    result
}

/// Observe recursive extra-content enumeration without changing its path or
/// result. This helps identify which folders the game actually scans.
unsafe extern "C" fn extra_content_recursive_enum(env: *mut Env, manager: u64, folder: u64) -> u32 {
    DLC_ENUM_COUNT.fetch_add(1, Ordering::Relaxed);
    let path = unsafe { Mem::of(env).str(folder) };
    let result = unsafe { orig::extra_content_recursive_enum(env, manager, folder) };
    log_line("vehicle-loader", LogLevel::Info,
        &format!("CExtraContentManager::RecursiveEnumExtraContent manager=0x{manager:x} folder={path} result={result}"));
    result
}
// Trace engine-owned content enumeration; forward every call unchanged.
unsafe extern "C" fn extra_content_begin_enumerate(env: *mut Env, manager: u64, flags: u32) {
    DLC_BEGIN_ENUM_COUNT.fetch_add(1, Ordering::Relaxed);
    log_line("vehicle-loader", LogLevel::Info,
        &format!("CExtraContentManager::BeginEnumerateContent manager=0x{manager:x} packed_flags={flags}"));
    unsafe { orig::extra_content_begin_enumerate(env, manager, flags) };
}

unsafe extern "C" fn extra_content_update_array(env: *mut Env, manager: u64) {
    DLC_UPDATE_ARRAY_COUNT.fetch_add(1, Ordering::Relaxed);
    log_line("vehicle-loader", LogLevel::Info,
        &format!("CExtraContentManager::UpdateContentArray manager=0x{manager:x} begin"));
    unsafe { orig::extra_content_update_array(env, manager) };
    log_line("vehicle-loader", LogLevel::Info, "CExtraContentManager::UpdateContentArray complete");
}

unsafe extern "C" fn extra_content_execute_group(env: *mut Env, manager: u64, group: u32) {
    DLC_EXECUTE_GROUP_COUNT.fetch_add(1, Ordering::Relaxed);
    log_line("vehicle-loader", LogLevel::Info,
        &format!("CExtraContentManager::ExecuteContentChangeSetGroupForAll manager=0x{manager:x} group={group}"));
    unsafe { orig::extra_content_execute_group(env, manager, group) };
}

unsafe extern "C" fn extra_content_manager_dtor(env: *mut Env, manager: u64) -> u64 {
    DLC_MANAGER_DTOR_COUNT.fetch_add(1, Ordering::Relaxed);

    // The manager pointer is only a diagnostic candidate captured from the
    // engine's AddContentFolder path. Clear it before forwarding destruction
    // so a later lifecycle cannot mistake a freed object for the live manager.
    let was_candidate = EXTRA_CONTENT_MANAGER.compare_exchange(
        manager, 0, Ordering::AcqRel, Ordering::Acquire
    ).is_ok();

    let result = unsafe { orig::extra_content_manager_dtor(env, manager) };
    log_line("vehicle-loader", LogLevel::Info,
        &format!("CExtraContentManager::dtor manager=0x{manager:x} cleared_candidate={was_candidate} result=0x{result:x}"));
    result
}

unsafe extern "C" fn mountable_content_shutdown(env: *mut Env, content: u64) {
    DLC_CONTENT_SHUTDOWN_COUNT.fetch_add(1, Ordering::Relaxed);
    log_line("vehicle-loader", LogLevel::Info,
        &format!("CMountableContent::ShutdownContent object=0x{content:x}"));
    unsafe { orig::mountable_content_shutdown(env, content) };
}


