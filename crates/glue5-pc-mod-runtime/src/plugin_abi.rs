//! Versioned C ABI contract for future Glue 5 ARM64 native plugins.
//! This describes a host/plugin boundary only. It does not load PE/ASI files,
//! map executable memory, or invoke plugin callbacks.

use std::ffi::{c_char, c_void};

pub const GLUE5_PLUGIN_ABI_V1: u32 = 1;

pub type PluginLoadFn = unsafe extern "C" fn(host: *const Glue5PluginHostV1) -> i32;
pub type PluginTickFn = unsafe extern "C" fn(context: *mut c_void, delta_seconds: f32);
pub type PluginUnloadFn = unsafe extern "C" fn(context: *mut c_void);
pub type PluginLogFn = unsafe extern "C" fn(level: u32, message: *const c_char);

/// Host functions available to a future native plugin. Keep this table
/// append-only; bump the ABI version for incompatible changes.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Glue5PluginHostV1 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub log: Option<PluginLogFn>,
}

/// Static descriptor returned by the future glue5_plugin_entry_v1 symbol.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Glue5PluginDescriptorV1 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub name: *const c_char,
    pub context: *mut c_void,
    pub on_load: Option<PluginLoadFn>,
    pub on_tick: Option<PluginTickFn>,
    pub on_unload: Option<PluginUnloadFn>,
}

/// Check ABI metadata before any future registration. This does not validate
/// arbitrary pointers or make an untrusted native library safe to execute.
pub fn validate_plugin_abi_header(abi_version: u32, struct_size: u32, minimum_size: u32) -> Result<(), &'static str> {
    if abi_version != GLUE5_PLUGIN_ABI_V1 {
        return Err("unsupported Glue 5 plugin ABI version");
    }
    if minimum_size == 0 || struct_size < minimum_size {
        return Err("Glue 5 plugin ABI structure is too small");
    }
    Ok(())
}

#[cfg(test)]
mod abi_tests {
    use super::*;

    #[test]
    fn accepts_matching_version_and_sufficient_structure_size() {
        assert!(validate_plugin_abi_header(GLUE5_PLUGIN_ABI_V1, 32, 24).is_ok());
    }

    #[test]
    fn rejects_unknown_versions_and_truncated_descriptors() {
        assert!(validate_plugin_abi_header(99, 32, 24).is_err());
        assert!(validate_plugin_abi_header(GLUE5_PLUGIN_ABI_V1, 8, 24).is_err());
    }
}
