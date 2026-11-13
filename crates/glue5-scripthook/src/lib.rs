//! Android-native ScriptHook host and game script-thread callback bridge.
use glue5_game::{Env, Hooks, exports, orig};
use glue5_mods::{natives::{self, Dir, Kind}, world::{self, Pool}};
use libloading::Library;
use std::ffi::{c_char, c_void, CStr};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

pub const ABI_VERSION: u32 = 12;
const ENTRYPOINT: &[u8] = b"glue5_scripthook_plugin_init\0";
pub type ScriptCallback = unsafe extern "C" fn(env: *mut c_void, info: u64, user: *mut c_void);
pub type NativeResolver = fn(hash: u64) -> Option<u64>;

/// Scalar native argument: 0=signed integer/bool, 1=unsigned/hash, 2=f32 bits. Complex pointer arguments are rejected.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct NativeArg { pub kind: u32, pub reserved: u32, pub value: u64 }

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct NativeMetadata { pub hash: u64, pub handler: u64, pub parameter_count: u32, pub return_kind: u32 }

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
    pub call_native_marshaled: Option<unsafe extern "C" fn(env: *mut c_void, hash: u64, args: *const NativeArg, count: u32, result: *mut u64) -> i32>,
    pub native_metadata: Option<unsafe extern "C" fn(hash: u64, output: *mut NativeMetadata) -> i32>,
    pub native_hash_by_name: Option<unsafe extern "C" fn(name: *const c_char, output: *mut u64) -> i32>,
    pub resolve_model_name: Option<unsafe extern "C" fn(name: *const c_char, expected_kind: u32, output_hash: *mut u32) -> i32>,
    /// Request a model; returns 1 when loaded, 0 while streaming, or a negative error.
    pub request_model: Option<unsafe extern "C" fn(env: *mut c_void, model_hash: u32) -> i32>,
    /// Release a model after it is no longer needed.
    pub release_model: Option<unsafe extern "C" fn(env: *mut c_void, model_hash: u32) -> i32>,
    /// GTA control polling using the generated native signatures.
    pub is_control_just_pressed: Option<unsafe extern "C" fn(env: *mut c_void, control_group: i32, control_id: i32) -> i32>,
    pub is_control_pressed: Option<unsafe extern "C" fn(env: *mut c_void, control_group: i32, control_id: i32) -> i32>,
    /// Raw f32 bits from GET_CONTROL_NORMAL.
    pub get_control_normal: Option<unsafe extern "C" fn(env: *mut c_void, control_group: i32, control_id: i32, output_bits: *mut u32) -> i32>,
    /// Calls a verified CREATE_/CLONE_ native and returns a live typed entity handle.
    pub create_entity: Option<unsafe extern "C" fn(env: *mut c_void, native_hash: u64, args: *const NativeArg, count: u32, expected_pool: u32, output_handle: *mut i32) -> i32>,
    /// Deletes a live typed entity using the generated DELETE_* signature.
    pub delete_entity: Option<unsafe extern "C" fn(env: *mut c_void, handle: i32, expected_pool: u32) -> i32>,
    pub validate_entity: Option<unsafe extern "C" fn(env: *mut c_void, handle: i32, expected_pool: u32) -> i32>,
    pub native_parameter: Option<unsafe extern "C" fn(hash: u64, index: u32, kind: *mut u32, direction: *mut u32) -> i32>,
    pub enumerate_entities: Option<unsafe extern "C" fn(env: *mut c_void, pool: u32, output: *mut i32, capacity: u32) -> i32>,
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
/// Signature-aware marshalling for generated native metadata.
/// NativeArg.kind: 0=i32, 1=u32/hash, 2=f32 bits, 3=wasm-memory offset,
/// 4=bool. For OUT parameters, kind 3 supplies the destination offset.
unsafe extern "C" fn call_native_marshaled(
    env: *mut c_void, hash: u64, args: *const NativeArg, count: u32, result: *mut u64,
) -> i32 {
    if env.is_null() || result.is_null() || (count != 0 && args.is_null()) || count > 64 { return -1; }
    let Some(native) = natives::by_hash(hash) else { return -3; };
    if native.params.len() != count as usize { return -10; }
    let env = env.cast::<Env>();
    let memory = unsafe { (*env).memory };
    if memory.is_null() { return -4; }
    let data = unsafe { (*memory).data };
    let memory_size = unsafe { (*memory).size };
    if data.is_null() || memory_size == 0 { return -4; }
    let top = match unsafe { exports::emscripten_stack_get_current(env) } { Ok(v) => v, Err(_) => return -5 };
    let outcome = (|| -> Result<u64, i32> {
        let alloc = |size: u64| -> Result<u64, i32> {
            let rounded = size.checked_add(15).ok_or(-6)? & !15;
            let at = unsafe { exports::_emscripten_stack_alloc(env, rounded) }.map_err(|_| -5)?;
            let end = at.checked_add(size).ok_or(-6)?;
            if end > memory_size { return Err(-6); }
            unsafe { std::ptr::write_bytes(data.add(at as usize), 0, size as usize); }
            Ok(at)
        };
        let check_range = |offset: u64, size: u64| -> Result<(), i32> {
            if offset.checked_add(size).is_some_and(|end| end <= memory_size) { Ok(()) } else { Err(-7) }
        };
        let read_u32 = |offset: u64| -> Result<u32, i32> {
            check_range(offset, 4)?;
            Ok(unsafe { std::ptr::read_unaligned(data.add(offset as usize).cast::<u32>()) })
        };
        let write_u32 = |offset: u64, value: u32| -> Result<(), i32> {
            check_range(offset, 4)?;
            unsafe { std::ptr::write_unaligned(data.add(offset as usize).cast::<u32>(), value); }
            Ok(())
        };
        let info = alloc(128)?;
        let slots: u64 = native.params.iter().map(|p| if p.kind == Kind::Vec3 && p.dir == Dir::In { 3 } else { 1 }).sum();
        let params = alloc(slots.max(1) * 8)?;
        let native_result = alloc(32)?;
        let mut outputs: Vec<(u64, u64, Kind)> = Vec::new();
        let mut slot = params;
        for (index, parameter) in native.params.iter().enumerate() {
            let arg = unsafe { *args.add(index) };
            let target = if parameter.dir == Dir::In {
                slot
            } else {
                let width = match parameter.kind { Kind::Vec3 => 12, Kind::Str => 8, _ => 4 };
                check_range(arg.value, width)?;
                if parameter.dir == Dir::Out && arg.kind != 3 { return Err(-13); }
                if parameter.dir == Dir::InOut {
                    let expected = match parameter.kind {
                        Kind::Int | Kind::Any | Kind::Handle(_) => 0,
                        Kind::Hash => 1,
                        Kind::Float => 2,
                        Kind::Bool => 4,
                        Kind::Str | Kind::Vec3 => 3,
                    };
                    if arg.kind != expected { return Err(-13); }
                }
                let buffer = alloc(24)?;
                if parameter.dir == Dir::InOut {
                    match parameter.kind {
                        Kind::Int | Kind::Any | Kind::Handle(_) | Kind::Hash | Kind::Float | Kind::Bool => {
                            let bits = read_u32(arg.value)?;
                            write_u32(buffer, bits)?;
                        }
                        Kind::Str => {
                            let pointer = unsafe { std::ptr::read_unaligned(data.add(arg.value as usize).cast::<u64>()) };
                            if pointer != 0 {
                                check_range(pointer, 1)?;
                                let mut end = pointer;
                                while end < memory_size && unsafe { *data.add(end as usize) } != 0 { end += 1; }
                                if end == memory_size { return Err(-7); }
                            }
                            unsafe { std::ptr::write_unaligned(data.add(buffer as usize).cast::<u64>(), pointer); }
                        }
                        Kind::Vec3 => {
                            for component in 0..3u64 {
                                let bits = read_u32(arg.value + component * 4)?;
                                write_u32(buffer + component * 8, bits)?;
                            }
                        }
                    }
                }
                unsafe { std::ptr::write_unaligned(data.add(slot as usize).cast::<u64>(), buffer); }
                outputs.push((buffer, arg.value, parameter.kind));
                buffer
            };
            if parameter.dir != Dir::Out {
                match parameter.kind {
                    Kind::Int | Kind::Any | Kind::Handle(_) => {
                        if arg.kind != 0 { return Err(-13); }
                        write_u32(target, if parameter.dir == Dir::InOut { read_u32(arg.value)? } else { arg.value as u32 })?;
                    }
                    Kind::Hash => {
                        if arg.kind != 1 { return Err(-13); }
                        write_u32(target, if parameter.dir == Dir::InOut { read_u32(arg.value)? } else { arg.value as u32 })?;
                    }
                    Kind::Float => {
                        if arg.kind != 2 { return Err(-13); }
                        write_u32(target, if parameter.dir == Dir::InOut { read_u32(arg.value)? } else { arg.value as u32 })?;
                    }
                    Kind::Bool => {
                        let value = if parameter.dir == Dir::InOut { read_u32(arg.value)? } else { arg.value as u32 };
                        if arg.kind != 4 || value > 1 { return Err(-14); }
                        write_u32(target, value)?;
                    }
                    Kind::Str => {
                        if arg.kind != 3 { return Err(-13); }
                        let pointer = if parameter.dir == Dir::InOut { unsafe { std::ptr::read_unaligned(data.add(arg.value as usize).cast::<u64>()) } } else { arg.value };
                        if pointer != 0 {
                            check_range(pointer, 1)?;
                            let mut end = pointer;
                            while end < memory_size && unsafe { *data.add(end as usize) } != 0 { end += 1; }
                            if end == memory_size { return Err(-7); }
                        }
                        unsafe { std::ptr::write_unaligned(data.add(target as usize).cast::<u64>(), pointer); }
                    }
                    Kind::Vec3 => {
                        if arg.kind != 3 { return Err(-13); }
                        check_range(arg.value, 12)?;
                        for component in 0..3u64 {
                            let bits = read_u32(arg.value + component * 4)?;
                            write_u32(target + component * 8, bits)?;
                        }
                    }
                }
            }
            slot += if parameter.kind == Kind::Vec3 && parameter.dir == Dir::In { 24 } else { 8 };
        }
        if native.name.starts_with("CREATE_") || native.name.starts_with("CLONE_") {
            let pool = match native.ret {
                Some(Kind::Handle(glue5_mods::natives::Handle::Vehicle)) => Some(Pool::Vehicles),
                Some(Kind::Handle(glue5_mods::natives::Handle::Ped)) => Some(Pool::Peds),
                Some(Kind::Handle(glue5_mods::natives::Handle::Object)) => Some(Pool::Objects),
                _ => None,
            };
            if let Some(pool) = pool {
                if world::free(unsafe { glue5_runtime::memory::Mem::of(env) }, pool) < 8 { return Err(-16); }
            }
        }
        unsafe {
            std::ptr::write_unaligned(data.add(info as usize).cast::<u64>(), native_result);
            std::ptr::write_unaligned(data.add((info + 8) as usize).cast::<u32>(), slots as u32);
            std::ptr::write_unaligned(data.add((info + 16) as usize).cast::<u64>(), params);
            std::ptr::write_unaligned(data.add((info + 24) as usize).cast::<i32>(), 0);
        }
        glue5_game::call_native(env, native.handler, info).map_err(|_| -8)?;
        // Match Glue 5's existing Vec3 reference write-back protocol.
        let vector_refs = (unsafe { std::ptr::read_unaligned(data.add((info + 24) as usize).cast::<i32>()) }).clamp(0, 4) as u64;
        for index in 0..vector_refs {
            let original = unsafe { std::ptr::read_unaligned(data.add((info + 32 + 8 * index) as usize).cast::<u64>()) };
            check_range(original, 24)?;
            let vector_buffer = info + 64 + 16 * index;
            check_range(vector_buffer, 12)?;
            for component in 0..3u64 {
                let bits = read_u32(vector_buffer + component * 4)?;
                write_u32(original + component * 8, bits)?;
            }
        }
        for (buffer, destination, kind) in outputs {
            match kind {
                Kind::Vec3 => {
                    for component in 0..3u64 {
                        let bits = read_u32(buffer + component * 8)?;
                        write_u32(destination + component * 4, bits)?;
                    }
                }
                Kind::Str => {
                    check_range(destination, 8)?;
                    let pointer = unsafe { std::ptr::read_unaligned(data.add(buffer as usize).cast::<u64>()) };
                    unsafe { std::ptr::write_unaligned(data.add(destination as usize).cast::<u64>(), pointer); }
                }
                _ => {
                    let bits = read_u32(buffer)?;
                    write_u32(destination, bits)?;
                }
            }
        }
        Ok(unsafe { std::ptr::read_unaligned(data.add(native_result as usize).cast::<u64>()) })
    })();
    if unsafe { exports::_emscripten_stack_restore(env, top) }.is_err() { return -9; }
    match outcome {
        Ok(value) => { unsafe { result.write(value); } 0 }
        Err(code) => code,
    }
}

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
        let native = natives::by_hash(hash).ok_or(-3)?;
        if native.params.len() != count as usize { return Err(-10); }
        for (index, parameter) in native.params.iter().enumerate() {
            // This entrypoint is deliberately scalar-only. Complex pointer,
            // string, vector and output parameters must use a future typed ABI.
            if parameter.dir != Dir::In { return Err(-11); }
            let arg = unsafe { *args.add(index) };
            let expected = match parameter.kind {
                Kind::Int | Kind::Any | Kind::Handle(_) => 0,
                Kind::Hash => 1,
                Kind::Float => 2,
                Kind::Bool => 0,
                Kind::Str | Kind::Vec3 => return Err(-12),
            };
            if arg.kind != expected { return Err(-13); }
            if parameter.kind == Kind::Bool && arg.value > 1 { return Err(-14); }
            // A raw wasm offset is not a valid scalar value; accepting it
            // here would silently pass an address as an integer.
            if arg.kind == 3 { return Err(-15); }
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

fn kind_code(kind: Option<Kind>) -> u32 {
    match kind { None => 0, Some(Kind::Int) => 1, Some(Kind::Float) => 2, Some(Kind::Bool) => 3,
        Some(Kind::Hash) => 4, Some(Kind::Str) => 5, Some(Kind::Vec3) => 6, Some(Kind::Any) => 7,
        Some(Kind::Handle(_)) => 8 }
}
fn find_native(hash: u64) -> Option<&'static glue5_mods::natives::Native> {
    natives::by_hash(hash)
}
unsafe extern "C" fn native_hash_by_name(name: *const c_char, output: *mut u64) -> i32 {
    if name.is_null() || output.is_null() { return -1; }
    let Ok(name) = unsafe { CStr::from_ptr(name) }.to_str() else { return -2; };
    let Some(native) = natives::NATIVES.iter().find(|native| native.name == name) else { return -3; };
    unsafe { output.write(native.hash); }
    0
}
unsafe extern "C" fn resolve_model_name(name: *const c_char, expected_kind: u32, output_hash: *mut u32) -> i32 {
    if name.is_null() || output_hash.is_null() { return -1; }
    let Ok(name) = unsafe { CStr::from_ptr(name) }.to_str() else { return -2; };
    if name.is_empty() || name.len() > 64 || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-') { return -3; }
    let Some(catalog) = world::catalog() else { return -4; };
    let lower = name.to_ascii_lowercase();
    let found = match expected_kind {
        1 => catalog.vehicles.iter().any(|item| item == &lower),
        2 => catalog.peds.iter().any(|item| item == &lower),
        0 => catalog.vehicles.iter().any(|item| item == &lower) || catalog.peds.iter().any(|item| item == &lower),
        _ => return -5,
    };
    if !found { return 0; }
    unsafe { output_hash.write(natives::joaat(&lower)); }
    1
}
/// Calls one of the model-lifecycle natives using the generated signature table.
unsafe fn call_model_native(env: *mut c_void, name: &str, model_hash: u32) -> Result<u64, i32> {
    if env.is_null() { return Err(-1); }
    let native = natives::NATIVES.iter().find(|native| native.name == name).ok_or(-2)?;
    let mut args = Vec::with_capacity(native.params.len());
    for (index, param) in native.params.iter().enumerate() {
        if param.dir != Dir::In { return Err(-3); }
        let arg = match (index, param.kind) {
            (0, Kind::Hash) => NativeArg { kind: 1, reserved: 0, value: u64::from(model_hash) },
            (_, Kind::Bool) => NativeArg { kind: 4, reserved: 0, value: 0 },
            (_, Kind::Int | Kind::Any) => NativeArg { kind: 0, reserved: 0, value: 0 },
            _ => return Err(-4),
        };
        args.push(arg);
    }
    let mut result = 0u64;
    let status = unsafe {
        call_native_marshaled(env, native.hash, args.as_ptr(), args.len() as u32, &mut result)
    };
    if status != 0 { return Err(status); }
    Ok(result)
}

/// Starts streaming a model and checks its state on this script tick.
/// Returns 1 when loaded, 0 while pending, negative on invalid input/native error.
unsafe extern "C" fn request_model(env: *mut c_void, model_hash: u32) -> i32 {
    if model_hash == 0 { return -1; }
    if unsafe { call_model_native(env, "REQUEST_MODEL", model_hash) }.is_err() { return -2; }
    match unsafe { call_model_native(env, "HAS_MODEL_LOADED", model_hash) } {
        Ok(bits) => if bits != 0 { 1 } else { 0 },
        Err(code) => code,
    }
}

/// Releases a model after the plugin has finished creating/using its entities.
unsafe extern "C" fn release_model(env: *mut c_void, model_hash: u32) -> i32 {
    if model_hash == 0 { return -1; }
    match unsafe { call_model_native(env, "SET_MODEL_AS_NO_LONGER_NEEDED", model_hash) } {
        Ok(_) => 0,
        Err(code) => code,
    }
}
/// Calls a creation native only when its generated return type matches the
/// requested entity pool, then validates the returned script handle.
/// Reads one of GTA's two-integer control natives after checking its generated signature.
unsafe fn call_control_native(env: *mut c_void, name: &str, group: i32, control: i32, expected_return: Kind) -> Result<u64, i32> {
    if env.is_null() { return Err(-1); }
    let native = natives::NATIVES.iter().find(|native| native.name == name).ok_or(-2)?;
    if native.params.len() != 2 || native.params.iter().any(|p| p.dir != Dir::In || p.kind != Kind::Int)
        || native.ret != Some(expected_return) { return Err(-3); }
    let args = [
        NativeArg { kind: 0, reserved: 0, value: group as u32 as u64 },
        NativeArg { kind: 0, reserved: 0, value: control as u32 as u64 },
    ];
    let mut result = 0u64;
    let status = unsafe { call_native_marshaled(env, native.hash, args.as_ptr(), 2, &mut result) };
    if status == 0 { Ok(result) } else { Err(status) }
}
unsafe extern "C" fn is_control_just_pressed(env: *mut c_void, group: i32, control: i32) -> i32 {
    match unsafe { call_control_native(env, "IS_CONTROL_JUST_PRESSED", group, control, Kind::Bool) } {
        Ok(bits) => if bits != 0 { 1 } else { 0 }, Err(code) => code,
    }
}
unsafe extern "C" fn is_control_pressed(env: *mut c_void, group: i32, control: i32) -> i32 {
    match unsafe { call_control_native(env, "IS_CONTROL_PRESSED", group, control, Kind::Bool) } {
        Ok(bits) => if bits != 0 { 1 } else { 0 }, Err(code) => code,
    }
}
unsafe extern "C" fn get_control_normal(env: *mut c_void, group: i32, control: i32, output_bits: *mut u32) -> i32 {
    if output_bits.is_null() { return -1; }
    match unsafe { call_control_native(env, "GET_CONTROL_NORMAL", group, control, Kind::Float) } {
        Ok(bits) => { unsafe { output_bits.write(bits as u32); } 0 }, Err(code) => code,
    }
}

/// Returns a spawn point a short distance in front of the local player.
/// output is four native floats: x, y, z, heading-degrees.
unsafe extern "C" fn get_player_spawn_transform(env: *mut c_void, distance: f32, output: *mut f32) -> i32 {
    if env.is_null() || output.is_null() || !distance.is_finite() || !(0.5..=25.0).contains(&distance) { return -1; }
    let Some(player_native) = natives::NATIVES.iter().find(|n| n.name == "PLAYER_PED_ID") else { return -2; };
    if !player_native.params.is_empty() || player_native.ret != Some(Kind::Handle(natives::Handle::Ped)) { return -3; }
    let mut player_bits = 0u64;
    let status = unsafe { call_native_marshaled(env, player_native.hash, std::ptr::null(), 0, &mut player_bits) };
    if status != 0 { return status; }
    let player = player_bits as u32 as i32;
    if player <= 0 || unsafe { validate_entity(env, player, 1) } != 1 { return -4; }

    let Some(matrix_native) = natives::NATIVES.iter().find(|n| n.name == "GET_ENTITY_MATRIX") else { return -5; };
    if matrix_native.params.len() != 5
        || matrix_native.params[0].kind != Kind::Handle(natives::Handle::Entity)
        || matrix_native.params[0].dir != Dir::In
        || matrix_native.params[1..].iter().any(|p| p.kind != Kind::Vec3 || p.dir != Dir::Out)
        || matrix_native.ret.is_some() { return -6; }

    let game_env = env.cast::<Env>();
    let memory = unsafe { (*game_env).memory };
    if memory.is_null() { return -7; }
    let data = unsafe { (*memory).data };
    let memory_size = unsafe { (*memory).size };
    if data.is_null() { return -7; }
    let top = match unsafe { exports::emscripten_stack_get_current(game_env) } { Ok(v) => v, Err(_) => return -8 };
    let result = (|| -> Result<[f32; 4], i32> {
        let at = unsafe { exports::_emscripten_stack_alloc(game_env, 64) }.map_err(|_| -8)?;
        if at.checked_add(64).is_none_or(|end| end > memory_size) { return Err(-9); }
        let args = [
            NativeArg { kind: 0, reserved: 0, value: player as u32 as u64 },
            NativeArg { kind: 3, reserved: 0, value: at },
            NativeArg { kind: 3, reserved: 0, value: at + 12 },
            NativeArg { kind: 3, reserved: 0, value: at + 24 },
            NativeArg { kind: 3, reserved: 0, value: at + 36 },
        ];
        let mut ignored = 0u64;
        let status = unsafe { call_native_marshaled(env, matrix_native.hash, args.as_ptr(), 5, &mut ignored) };
        if status != 0 { return Err(status); }
        let mem = unsafe { glue5_runtime::memory::Mem::of(game_env) };
        let read_vec = |offset: u64| -> [f32; 3] {
            [0, 1, 2].map(|i| f32::from_bits(mem.u32(offset + i * 4)))
        };
        let forward = read_vec(at);
        let position = read_vec(at + 36);
        if !forward.iter().chain(position.iter()).all(|v| v.is_finite()) { return Err(-10); }
        let heading = forward[1].atan2(forward[0]).to_degrees();
        Ok([
            position[0] + forward[0] * distance,
            position[1] + forward[1] * distance,
            position[2] + forward[2] * distance + 0.5,
            heading,
        ])
    })();
    if unsafe { exports::_emscripten_stack_restore(game_env, top) }.is_err() { return -11; }
    match result {
        Ok(values) => { unsafe { std::ptr::copy_nonoverlapping(values.as_ptr(), output, 4); } 0 }
        Err(code) => code,
    }
}

unsafe extern "C" fn create_entity(
    env: *mut c_void, native_hash: u64, args: *const NativeArg, count: u32,
    expected_pool: u32, output_handle: *mut i32,
) -> i32 {
    if env.is_null() || output_handle.is_null() || (count != 0 && args.is_null()) { return -1; }
    let expected = match expected_pool {
        1 => natives::Handle::Ped,
        2 => natives::Handle::Vehicle,
        3 => natives::Handle::Object,
        _ => return -2,
    };
    let Some(native) = natives::by_hash(native_hash) else { return -3; };
    if !(native.name.starts_with("CREATE_") || native.name.starts_with("CLONE_")) { return -4; }
    if native.ret != Some(Kind::Handle(expected)) { return -5; }
    if native.params.len() != count as usize { return -6; }
    let mut bits = 0u64;
    let status = unsafe { call_native_marshaled(env, native_hash, args, count, &mut bits) };
    if status != 0 { return status; }
    let handle = bits as u32 as i32;
    if handle <= 0 || unsafe { validate_entity(env, handle, expected_pool) } != 1 { return -7; }
    unsafe { output_handle.write(handle); }
    0
}

/// Uses only the exact generated DELETE_* entry and its declared one-handle
/// signature. INOUT handle parameters are staged in wasm memory for writeback.
unsafe extern "C" fn delete_entity(env: *mut c_void, handle: i32, expected_pool: u32) -> i32 {
    if env.is_null() || handle <= 0 { return -1; }
    let (kind, native_name) = match expected_pool {
        1 => (natives::Handle::Ped, "DELETE_PED"),
        2 => (natives::Handle::Vehicle, "DELETE_VEHICLE"),
        3 => (natives::Handle::Object, "DELETE_OBJECT"),
        _ => return -2,
    };
    if unsafe { validate_entity(env, handle, expected_pool) } != 1 { return -3; }
    let Some(native) = natives::NATIVES.iter().find(|n| n.name == native_name) else { return -4; };
    if native.params.len() != 1 || native.params[0].kind != Kind::Handle(kind) { return -5; }
    let mut staged_top = None;
    let arg = match native.params[0].dir {
        Dir::In => NativeArg { kind: 0, reserved: 0, value: handle as u32 as u64 },
        Dir::InOut => {
            let env = env.cast::<Env>();
            let memory = unsafe { (*env).memory };
            if memory.is_null() { return -6; }
            let data = unsafe { (*memory).data };
            let size = unsafe { (*memory).size };
            if data.is_null() { return -6; }
            let top = match unsafe { exports::emscripten_stack_get_current(env) } { Ok(v) => v, Err(_) => return -7 };
            let at = match unsafe { exports::_emscripten_stack_alloc(env, 16) } { Ok(v) => v, Err(_) => return -7 };
            if at.checked_add(4).is_none_or(|end| end > size) {
                let _ = unsafe { exports::_emscripten_stack_restore(env, top) };
                return -8;
            }
            unsafe { std::ptr::write_unaligned(data.add(at as usize).cast::<u32>(), handle as u32); }
            staged_top = Some(top);
            NativeArg { kind: 0, reserved: 0, value: at }
        }
        Dir::Out => return -9,
    };
    let mut result = 0u64;
    let status = unsafe { call_native_marshaled(env, native.hash, &arg, 1, &mut result) };
    if let Some(top) = staged_top {
        let _ = unsafe { exports::_emscripten_stack_restore(env.cast::<Env>(), top) };
    }
    if status == 0 { 0 } else { status }
}
unsafe extern "C" fn validate_entity(env: *mut c_void, handle: i32, expected_pool: u32) -> i32 {
    if env.is_null() { return -1; }
    let Some(kind) = (unsafe { natives::entity_handle_kind(env.cast(), handle) }) else { return 0; };
    let matches = match expected_pool {
        0 => matches!(kind, natives::Handle::Entity | natives::Handle::Ped | natives::Handle::Vehicle | natives::Handle::Object),
        1 => kind == natives::Handle::Ped,
        2 => kind == natives::Handle::Vehicle,
        3 => kind == natives::Handle::Object,
        _ => return -2,
    };
    if matches { 1 } else { 0 }
}
unsafe extern "C" fn native_metadata(hash: u64, output: *mut NativeMetadata) -> i32 {
    if output.is_null() { return -1; }
    let Some(native) = find_native(hash) else { return -2; };
    unsafe { output.write(NativeMetadata { hash, handler: native.handler,
        parameter_count: native.params.len() as u32, return_kind: kind_code(native.ret) }); }
    0
}
unsafe extern "C" fn native_parameter(hash: u64, index: u32, kind: *mut u32, direction: *mut u32) -> i32 {
    if kind.is_null() || direction.is_null() { return -1; }
    let Some(native) = find_native(hash) else { return -2; };
    let Some(param) = native.params.get(index as usize) else { return -3; };
    unsafe { kind.write(kind_code(Some(param.kind))); direction.write(match param.dir { Dir::In => 0, Dir::Out => 1, Dir::InOut => 2 }); }
    0
}
unsafe extern "C" fn enumerate_entities(env: *mut c_void, pool: u32, output: *mut i32, capacity: u32) -> i32 {
    if env.is_null() || (capacity != 0 && output.is_null()) || capacity > 4096 { return -1; }
    let pool = match pool { 0 => Pool::Peds, 1 => Pool::Vehicles, 2 => Pool::Objects, _ => return -2 };
    let handles = unsafe { world::handles(env.cast(), pool, capacity as usize) };
    for (index, handle) in handles.iter().enumerate() { unsafe { output.add(index).write(*handle); } }
    handles.len() as i32
}
static HOST_API: HostApi = HostApi {
    abi_version: ABI_VERSION, log: Some(host_log), register_script_callback: Some(register_script_callback),
    script_stack_top: Some(script_stack_top), script_stack_alloc: Some(script_stack_alloc),
    script_stack_restore: Some(script_stack_restore), call_native: Some(call_native),
    call_native_typed: Some(call_native_typed), call_native_marshaled: Some(call_native_marshaled), native_metadata: Some(native_metadata), native_hash_by_name: Some(native_hash_by_name), resolve_model_name: Some(resolve_model_name), request_model: Some(request_model), release_model: Some(release_model), is_control_just_pressed: Some(is_control_just_pressed), is_control_pressed: Some(is_control_pressed), get_control_normal: Some(get_control_normal), create_entity: Some(create_entity), delete_entity: Some(delete_entity), validate_entity: Some(validate_entity),
    native_parameter: Some(native_parameter), enumerate_entities: Some(enumerate_entities), get_player_spawn_transform: Some(get_player_spawn_transform),
};
pub fn install(mods_hook_installed: bool, plugin_directory: &Path) -> LoadReport {
    // Use the generated native database for this exact game.wasm build; do not
    // guess handler indices or resolve against a different GTA title/build.
    let _ = NATIVE_RESOLVER.set(glue5_mods::natives::handler_by_hash);
    if !INSTALLED.swap(true, Ordering::AcqRel) {
        if mods_hook_installed {
            glue5_runtime::add_script_wait_callback(on_script_wait);
        } else {
            glue5_runtime::add_hooks(hooks);
        }
    }
    if let Err(error) = std::fs::create_dir_all(plugin_directory) {
        log::error!("cannot create ScriptHook plugin directory {}: {error}", plugin_directory.display());
        return LoadReport { loaded: Vec::new(), rejected: vec![(plugin_directory.to_path_buf(), error.to_string())] };
    }
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
    unsafe { glue5_runtime::dispatch_script_wait_callbacks(env, info); }
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
    #[test] fn abi_version_is_stable_and_nonzero() { assert_eq!(ABI_VERSION, 12); }
    #[test] fn native_arg_layout_is_stable() { assert_eq!(std::mem::size_of::<NativeArg>(), 16); }
    #[test] fn native_metadata_layout_is_stable() { assert_eq!(std::mem::size_of::<NativeMetadata>(), 24); }
    #[test] fn host_api_v11_exposes_controls_and_entity_lifecycle() {
        assert_eq!(HOST_API.abi_version, ABI_VERSION);
        assert!(HOST_API.is_control_just_pressed.is_some());
        assert!(HOST_API.is_control_pressed.is_some());
        assert!(HOST_API.get_control_normal.is_some());
        assert!(HOST_API.create_entity.is_some());
        assert!(HOST_API.delete_entity.is_some());
        assert!(HOST_API.request_model.is_some());
        assert!(HOST_API.release_model.is_some());
        assert!(HOST_API.get_player_spawn_transform.is_some());
    }
    #[test] fn host_api_layout_matches_android_arm64_c_header_contract() {
        use std::mem::{offset_of, size_of};
        // Android plugins are arm64-v8a. Pin every public field offset so a
        // Rust-side reorder/type change cannot silently desynchronize the C ABI.
        assert_eq!(size_of::<HostApi>(), 184);
        assert_eq!(offset_of!(HostApi, abi_version), 0);
        assert_eq!(offset_of!(HostApi, log), 8);
        assert_eq!(offset_of!(HostApi, register_script_callback), 16);
        assert_eq!(offset_of!(HostApi, script_stack_top), 24);
        assert_eq!(offset_of!(HostApi, script_stack_alloc), 32);
        assert_eq!(offset_of!(HostApi, script_stack_restore), 40);
        assert_eq!(offset_of!(HostApi, call_native), 48);
        assert_eq!(offset_of!(HostApi, call_native_typed), 56);
        assert_eq!(offset_of!(HostApi, call_native_marshaled), 64);
        assert_eq!(offset_of!(HostApi, native_metadata), 72);
        assert_eq!(offset_of!(HostApi, native_hash_by_name), 80);
        assert_eq!(offset_of!(HostApi, resolve_model_name), 88);
        assert_eq!(offset_of!(HostApi, request_model), 96);
        assert_eq!(offset_of!(HostApi, release_model), 104);
        assert_eq!(offset_of!(HostApi, is_control_just_pressed), 112);
        assert_eq!(offset_of!(HostApi, is_control_pressed), 120);
        assert_eq!(offset_of!(HostApi, get_control_normal), 128);
        assert_eq!(offset_of!(HostApi, create_entity), 136);
        assert_eq!(offset_of!(HostApi, delete_entity), 144);
        assert_eq!(offset_of!(HostApi, validate_entity), 152);
        assert_eq!(offset_of!(HostApi, native_parameter), 160);
        assert_eq!(offset_of!(HostApi, enumerate_entities), 168);
        assert_eq!(offset_of!(HostApi, get_player_spawn_transform), 176);
    }
    #[test] fn missing_directory_is_reported_without_panicking() {
        let report = load_directory(Path::new("/definitely/not/a/glue5/plugin/folder"));
        assert!(report.loaded.is_empty()); assert_eq!(report.rejected.len(), 1);
    }
}
