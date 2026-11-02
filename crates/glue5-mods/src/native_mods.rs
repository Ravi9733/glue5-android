//! Android-native plugin package validation.
//!
//! This is the first stage of the native mod system: it validates the explicit
//! Glue5 ABI manifest and confirms that the plugin is an Android ARM64 ELF.
//! It deliberately does not dlopen untrusted code. Loading requires the
//! versioned host ABI, lifecycle/thread contract and user trust decision to
//! be wired into the Android host.

use serde::Deserialize;

pub const MANIFEST_FILE: &str = "glue5-native-mod.json";
pub const ABI_V1: &str = "glue5-native-v1";
pub const MAX_MANIFEST_BYTES: usize = 16 * 1024;
const ELF_HEADER_MIN: usize = 64;
const ELFCLASS64: u8 = 2;
const ELFDATA2LSB: u8 = 1;
const EM_AARCH64: u16 = 183;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct NativePluginManifest {
    pub format_version: u32,
    pub id: String,
    pub name: String,
    pub version: String,
    pub abi: String,
    pub library: String,
    pub entry: String,
    pub min_engine: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativePluginReport {
    pub manifest: NativePluginManifest,
    pub architecture: String,
    /// Validation only; this does not mean the library has been loaded.
    pub compatible_binary: bool,
}

fn safe_component(value: &str, max: usize) -> bool {
    !value.is_empty()
        && value.len() <= max
        && value.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.')
        && !value.starts_with('.')
        && !value.contains("..")
}

pub fn parse_manifest(bytes: &[u8]) -> Result<NativePluginManifest, String> {
    if bytes.is_empty() || bytes.len() > MAX_MANIFEST_BYTES {
        return Err(format!("native plugin manifest must be 1–{MAX_MANIFEST_BYTES} bytes"));
    }
    let manifest: NativePluginManifest =
        serde_json::from_slice(bytes).map_err(|e| format!("invalid native plugin manifest: {e}"))?;
    if manifest.format_version != 1 {
        return Err(format!("unsupported native plugin manifest version: {}", manifest.format_version));
    }
    if !safe_component(&manifest.id, 64) {
        return Err("native plugin id must be a safe 1–64 character identifier".into());
    }
    if manifest.name.trim().is_empty() || manifest.name.len() > 96 {
        return Err("native plugin name must be 1–96 characters".into());
    }
    if manifest.version.trim().is_empty() || manifest.version.len() > 48 {
        return Err("native plugin version must be 1–48 characters".into());
    }
    if manifest.abi != ABI_V1 {
        return Err(format!("unsupported native plugin ABI: {}", manifest.abi));
    }
    if !safe_component(&manifest.library, 96) || !manifest.library.ends_with(".so") {
        return Err("library must be a plain .so filename inside the package".into());
    }
    if !safe_component(&manifest.entry, 96) {
        return Err("entry must be a plain exported symbol name".into());
    }
    if manifest.min_engine.trim().is_empty() || manifest.min_engine.len() > 64 {
        return Err("min_engine must be 1–64 characters".into());
    }
    Ok(manifest)
}

/// Verify ELF class, endianness and machine type for Android ARM64.
/// ELF metadata is untrusted input; no code is mapped or executed here.
pub fn inspect_android_arm64_elf(bytes: &[u8]) -> Result<String, String> {
    if bytes.len() < ELF_HEADER_MIN || &bytes[..4] != b"\x7fELF" {
        return Err("native plugin library is not an ELF binary".into());
    }
    if bytes[4] != ELFCLASS64 {
        return Err("native plugin must be 64-bit ELF".into());
    }
    if bytes[5] != ELFDATA2LSB {
        return Err("native plugin must use little-endian ELF".into());
    }
    let object_type = u16::from_le_bytes([bytes[16], bytes[17]]);\n    if object_type != 3 {\n        return Err("native plugin ELF must be a shared object (ET_DYN)".into());\n    }\n    let machine = u16::from_le_bytes([bytes[18], bytes[19]]);
    if machine != EM_AARCH64 {
        return Err(format!("native plugin ELF machine {machine} is not Android ARM64 (AArch64)"));
    }
    Ok("aarch64-android".into())
}

/// Validate a manifest and its referenced library bytes from an extracted
/// package. The caller must resolve the library beneath the package root.
pub fn inspect(manifest_bytes: &[u8], library_bytes: &[u8]) -> Result<NativePluginReport, String> {
    let manifest = parse_manifest(manifest_bytes)?;
    let architecture = inspect_android_arm64_elf(library_bytes)?;
    Ok(NativePluginReport { manifest, architecture, compatible_binary: true })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest() -> Vec<u8> {
        br#"{
          "format_version": 1,
          "id": "sample-native",
          "name": "Sample Native Mod",
          "version": "1.0.0",
          "abi": "glue5-native-v1",
          "library": "libsample.so",
          "entry": "glue5_mod_init",
          "min_engine": "8b0b5899ed"
        }"#.to_vec()
    }

    fn arm64_elf() -> Vec<u8> {
        let mut bytes = vec![0u8; ELF_HEADER_MIN];
        bytes[..4].copy_from_slice(b"\x7fELF");
        bytes[4] = ELFCLASS64;
        bytes[5] = ELFDATA2LSB;
        bytes[18..20].copy_from_slice(&EM_AARCH64.to_le_bytes());
        bytes
    }

    #[test]
    fn accepts_versioned_arm64_manifest_and_elf_header() {
        let report = inspect(&manifest(), &arm64_elf()).unwrap();
        assert_eq!(report.manifest.id, "sample-native");
        assert_eq!(report.architecture, "aarch64-android");
        assert!(report.compatible_binary);
    }

    #[test]
    fn rejects_windows_pe_and_non_elf_libraries() {
        assert!(inspect(&manifest(), b"MZ\x90\0").unwrap_err().contains("ELF"));
    }

    #[test]
    fn rejects_x86_64_elf() {
        let mut elf = arm64_elf();
        elf[18..20].copy_from_slice(&62u16.to_le_bytes());
        assert!(inspect(&manifest(), &elf).unwrap_err().contains("not Android ARM64"));
    }

    #[test]
    fn rejects_path_traversal_and_unknown_manifest_fields() {
        let bad = String::from_utf8(manifest()).unwrap().replace("libsample.so", "../libsample.so");
        assert!(parse_manifest(bad.as_bytes()).unwrap_err().contains("library"));
        let unknown = br#"{"format_version":1,"id":"x","name":"x","version":"1","abi":"glue5-native-v1","library":"x.so","entry":"init","min_engine":"x","extra":true}"#;
        assert!(parse_manifest(unknown).is_err());
    }
}
