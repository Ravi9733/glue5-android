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
        && value.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        && !value.starts_with('.')
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
    let object_type = u16::from_le_bytes([bytes[16], bytes[17]]);
    if object_type != 3 {
        return Err("native plugin ELF must be a shared object (ET_DYN)".into());
    }
    let machine = u16::from_le_bytes([bytes[18], bytes[19]]);
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
        bytes[16..18].copy_from_slice(&3u16.to_le_bytes());
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


/// Inspect a native plugin ZIP without executing its code. The archive must
/// contain a root manifest and the single ARM64 .so named by that manifest.
/// This is deliberately separate from the PC ZIP importer: ASI/DLL files are
/// never treated as Android native libraries.
pub fn inspect_zip(bytes: &[u8]) -> Result<NativePluginReport, String> {
    use std::io::Read;
    const MAX_ENTRIES: usize = 512;
    const MAX_FILE: u64 = 64 * 1024 * 1024;
    const MAX_TOTAL: u64 = 256 * 1024 * 1024;

    if bytes.is_empty() {
        return Err("native plugin ZIP is empty".into());
    }
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes))
        .map_err(|e| format!("invalid native plugin ZIP: {e}"))?;
    if archive.is_empty() || archive.len() > MAX_ENTRIES {
        return Err(format!("native plugin ZIP must contain 1–{MAX_ENTRIES} entries"));
    }

    let mut total = 0u64;
    let mut names = std::collections::HashSet::new();
    let mut manifest_bytes: Option<Vec<u8>> = None;
    let mut library_bytes: Option<Vec<u8>> = None;
    let mut manifest_library: Option<String> = None;

    for index in 0..archive.len() {
        let mut file = archive.by_index(index).map_err(|e| format!("ZIP entry {index}: {e}"))?;
        let name = file.name().to_owned();
        if name.starts_with('/') || name.contains('\\\\') || name.contains(':')
            || name.split('/').any(|part| part == ".." || part == "." || part.is_empty() && !name.ends_with('/'))
        {
            return Err(format!("unsafe native plugin ZIP path: {name}"));
        }
        if !names.insert(name.clone()) {
            return Err(format!("duplicate native plugin ZIP path: {name}"));
        }
        if file.unix_mode().is_some_and(|mode| mode & 0o170000 == 0o120000) {
            return Err(format!("symbolic links are not allowed: {name}"));
        }
        if file.is_dir() {
            continue;
        }
        if file.size() > MAX_FILE {
            return Err(format!("native plugin file is too large: {name}"));
        }
        total = total.checked_add(file.size()).ok_or("native plugin ZIP size overflow")?;
        if total > MAX_TOTAL {
            return Err("native plugin ZIP expands beyond 256 MiB".into());
        }

        if name == MANIFEST_FILE {
            let mut data = Vec::with_capacity(file.size() as usize);
            file.take(MAX_MANIFEST_BYTES as u64 + 1).read_to_end(&mut data)
                .map_err(|e| format!("cannot read native plugin manifest: {e}"))?;
            if data.len() > MAX_MANIFEST_BYTES {
                return Err("native plugin manifest is too large".into());
            }
            let parsed = parse_manifest(&data)?;
            manifest_library = Some(parsed.library);
            manifest_bytes = Some(data);
        } else if name.ends_with(".so") && !name.contains('/') {
            if library_bytes.is_some() {
                return Err("native plugin ZIP must contain only one root .so library".into());
            }
            let mut data = Vec::with_capacity(file.size() as usize);
            file.take(MAX_FILE + 1).read_to_end(&mut data)
                .map_err(|e| format!("cannot read native plugin library: {e}"))?;
            if data.len() as u64 != file.size() {
                return Err("native plugin library size does not match ZIP metadata".into());
            }
            library_bytes = Some(data);
        } else {
            return Err(format!("unsupported file in native plugin ZIP: {name}"));
        }
    }

    let manifest = manifest_bytes.ok_or_else(|| format!("ZIP is missing {MANIFEST_FILE}"))?;
    let library_name = manifest_library.ok_or("native plugin manifest is invalid")?;
    let library = library_bytes.ok_or("ZIP is missing its root Android .so library")?;
    if !names.contains(&library_name) {
        return Err(format!("ZIP is missing manifest library: {library_name}"));
    }
    inspect(&manifest, &library)
}

/// Install a validated native plugin package into the app-private mods area.
/// Installation does not load or execute the library; a separate explicit
/// enable/trust step and game-thread runtime are required.
pub fn install_zip(bytes: &[u8]) -> Result<NativePluginReport, String> {
    let report = inspect_zip(bytes)?;
    let root = crate::dir().ok_or("mods are off")?.join(".native-plugins");
    std::fs::create_dir_all(&root).map_err(|e| format!("cannot create native plugin folder: {e}"))?;
    let target = root.join(&report.manifest.id);
    if target.exists() {
        return Err(format!("native plugin {} is already installed", report.manifest.id));
    }

    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes))
        .map_err(|e| format!("invalid native plugin ZIP: {e}"))?;
    let staging = root.join(format!(".stage-{}", report.manifest.id));
    if staging.exists() {
        std::fs::remove_dir_all(&staging).map_err(|e| format!("cannot clear staging folder: {e}"))?;
    }
    std::fs::create_dir(&staging).map_err(|e| format!("cannot create staging folder: {e}"))?;
    let result = (|| {
        for index in 0..archive.len() {
            let mut file = archive.by_index(index).map_err(|e| format!("ZIP entry {index}: {e}"))?;
            if file.is_dir() { continue; }
            let name = file.name().to_owned();
            if name != MANIFEST_FILE && name != report.manifest.library {
                return Err(format!("unsupported native plugin ZIP path: {name}"));
            }
            let mut data = Vec::with_capacity(file.size() as usize);
            file.read_to_end(&mut data).map_err(|e| format!("cannot extract {name}: {e}"))?;
            std::fs::write(staging.join(&name), data).map_err(|e| format!("cannot write {name}: {e}"))?;
        }
        std::fs::rename(&staging, &target).map_err(|e| format!("cannot finalize native plugin install: {e}"))?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_dir_all(&staging);
    }
    result?;
    Ok(report)
}
