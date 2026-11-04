//! Capability gate and manifest validation for a future native PC-game backend.
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Read;
use std::path::{Component, Path};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackendState { MissingAndroidEngine, MissingVulkanTranslationLayer, MissingPreparedGameData, ReadyForRuntimeIntegration }

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadinessReport {
    pub state: BackendState,
    pub android_engine_present: bool,
    pub vulkan_translation_present: bool,
    pub prepared_data_present: bool,
    pub blockers: Vec<&'static str>,
}

pub fn inspect_layout(root: &Path, engine_path: &Path, vulkan_path: &Path, data_path: &Path) -> ReadinessReport {
    let engine = root.join(engine_path).is_file();
    let vulkan = root.join(vulkan_path).is_file();
    let data = root.join(data_path).is_dir();
    let mut blockers = Vec::new();
    if !engine { blockers.push("Android ARM64 native game engine is missing"); }
    if !vulkan { blockers.push("Android Vulkan graphics translation layer is missing"); }
    if !data { blockers.push("Prepared and version-checked PC game data is missing"); }
    let state = if !engine { BackendState::MissingAndroidEngine }
        else if !vulkan { BackendState::MissingVulkanTranslationLayer }
        else if !data { BackendState::MissingPreparedGameData }
        else { BackendState::ReadyForRuntimeIntegration };
    ReadinessReport { state, android_engine_present: engine, vulkan_translation_present: vulkan, prepared_data_present: data, blockers }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DataFile { pub path: String, pub size: u64, pub sha256: String }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DataManifest { pub schema_version: u32, pub game_build: String, pub preparation_tool: String, pub files: Vec<DataFile> }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ManifestError(pub String);
impl std::fmt::Display for ManifestError { fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str(&self.0) } }
impl std::error::Error for ManifestError {}

/// Parse a versioned JSON manifest. This validates metadata only; it does not
/// hash, extract, transform, or execute game files.
pub fn parse_data_manifest(input: &str) -> Result<DataManifest, ManifestError> {
    let root: serde_json::Value = serde_json::from_str(input).map_err(|e| ManifestError(format!("invalid JSON: {e}")))?;
    let obj = root.as_object().ok_or_else(|| ManifestError("manifest root must be an object".into()))?;
    let schema_version = obj.get("schema_version").and_then(|v| v.as_u64()).and_then(|v| u32::try_from(v).ok())
        .ok_or_else(|| ManifestError("schema_version must be an unsigned integer".into()))?;
    if schema_version != 1 { return Err(ManifestError(format!("unsupported schema_version: {schema_version}"))); }
    let game_build = string_field(obj, "game_build")?;
    let preparation_tool = string_field(obj, "preparation_tool")?;
    if game_build.trim().is_empty() || preparation_tool.trim().is_empty() { return Err(ManifestError("game_build and preparation_tool cannot be empty".into())); }
    let entries = obj.get("files").and_then(|v| v.as_array()).ok_or_else(|| ManifestError("files must be an array".into()))?;
    if entries.is_empty() { return Err(ManifestError("files must contain at least one entry".into())); }
    let mut seen = std::collections::HashSet::new();
    let mut files = Vec::with_capacity(entries.len());
    for (i, value) in entries.iter().enumerate() {
        let item = value.as_object().ok_or_else(|| ManifestError(format!("files[{i}] must be an object")))?;
        let path = item.get("path").and_then(|v| v.as_str()).ok_or_else(|| ManifestError(format!("files[{i}].path must be a string")))?;
        validate_path(path).map_err(|e| ManifestError(format!("files[{i}].path: {e}")))?;
        if !seen.insert(path.to_owned()) { return Err(ManifestError(format!("duplicate path: {path}"))); }
        let size = item.get("size").and_then(|v| v.as_u64()).ok_or_else(|| ManifestError(format!("files[{i}].size must be an unsigned integer")))?;
        let hash = item.get("sha256").and_then(|v| v.as_str()).ok_or_else(|| ManifestError(format!("files[{i}].sha256 must be a string")))?;
        if hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) { return Err(ManifestError(format!("files[{i}].sha256 must be 64 hexadecimal characters"))); }
        files.push(DataFile { path: path.to_owned(), size, sha256: hash.to_ascii_lowercase() });
    }
    Ok(DataManifest { schema_version, game_build, preparation_tool, files })
}

fn string_field(obj: &serde_json::Map<String, serde_json::Value>, key: &str) -> Result<String, ManifestError> {
    obj.get(key).and_then(|v| v.as_str()).map(str::to_owned).ok_or_else(|| ManifestError(format!("{key} must be a string")))
}

fn validate_path(value: &str) -> Result<(), &'static str> {
    if value.is_empty() || value.contains('\\') || value.starts_with('/') { return Err("must be a non-empty relative path using forward slashes"); }
    if Path::new(value).components().any(|c| !matches!(c, Component::Normal(_))) { return Err("must not contain traversal, root, or prefix components"); }
    if value.split('/').any(str::is_empty) { return Err("must not contain empty path segments"); }
    Ok(())
}

/// Verify each manifest entry against files under the supplied data root.
/// Hashing is streamed in bounded memory; symlinks escaping the data root are rejected.
pub fn verify_data_files(root: &Path, manifest: &DataManifest) -> Result<(), ManifestError> {
    let canonical_root = root.canonicalize().map_err(|e| ManifestError(format!("cannot resolve data root: {e}")))?;
    if !canonical_root.is_dir() { return Err(ManifestError("data root is not a directory".into())); }

    for entry in &manifest.files {
        let candidate = canonical_root.join(&entry.path);
        let resolved = candidate.canonicalize()
            .map_err(|e| ManifestError(format!("{}: cannot resolve file: {e}", entry.path)))?;
        if !resolved.starts_with(&canonical_root) {
            return Err(ManifestError(format!("{}: resolved path escapes data root", entry.path)));
        }
        let metadata = resolved.metadata()
            .map_err(|e| ManifestError(format!("{}: cannot read metadata: {e}", entry.path)))?;
        if !metadata.is_file() {
            return Err(ManifestError(format!("{}: expected a regular file", entry.path)));
        }
        if metadata.len() != entry.size {
            return Err(ManifestError(format!("{}: size mismatch (expected {}, found {})", entry.path, entry.size, metadata.len())));
        }

        let mut file = File::open(&resolved)
            .map_err(|e| ManifestError(format!("{}: cannot open file: {e}", entry.path)))?;
        let mut hasher = Sha256::new();
        let mut buffer = [0_u8; 1024 * 1024];
        loop {
            let count = file.read(&mut buffer)
                .map_err(|e| ManifestError(format!("{}: read failed: {e}", entry.path)))?;
            if count == 0 { break; }
            hasher.update(&buffer[..count]);
        }
        let actual = format!("{:x}", hasher.finalize());
        if actual != entry.sha256 {
            return Err(ManifestError(format!("{}: SHA-256 mismatch", entry.path)));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    const VALID: &str = r#"{"schema_version":1,"game_build":"build-2699","preparation_tool":"glue5-data-prep/0.1","files":[{"path":"update/update.rpf","size":1234,"sha256":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"}]}"#;

    fn temp_dir() -> std::path::PathBuf {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        std::env::temp_dir().join(format!("glue5-pc-native-{}-{nonce}", std::process::id()))
    }

    #[test] fn parses_manifest_and_normalizes_hash() { let m=parse_data_manifest(VALID).unwrap(); assert_eq!(m.files[0].sha256, "a".repeat(64)); }
    #[test] fn rejects_unknown_schema() { assert!(parse_data_manifest(&VALID.replace("\"schema_version\":1", "\"schema_version\":2")).is_err()); }
    #[test] fn rejects_traversal_absolute_and_backslash_paths() {
        for bad in ["../escape", "/absolute", "a/../../b", "a\\b"] { assert!(parse_data_manifest(&VALID.replace("update/update.rpf", bad)).is_err(), "{bad}"); }
    }
    #[test] fn rejects_bad_hash_and_duplicate_paths() {
        assert!(parse_data_manifest(&VALID.replace(&"A".repeat(64), "bad")).is_err());
        let dup = VALID.replace("}]}", "},{\"path\":\"update/update.rpf\",\"size\":1,\"sha256\":\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"}]}");
        assert!(parse_data_manifest(&dup).is_err());
    }
    #[test] fn rejects_empty_files() { assert!(parse_data_manifest(&VALID.replace("\"files\":[{\"path\":\"update/update.rpf\",\"size\":1234,\"sha256\":\"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA\"}]", "\"files\":[]")).is_err()); }

    #[test]
    fn verifies_real_file_size_and_sha256() {
        let dir = temp_dir();
        fs::create_dir_all(dir.join("update")).unwrap();
        let bytes = b"owned game data test";
        fs::write(dir.join("update/update.rpf"), bytes).unwrap();
        let manifest = DataManifest {
            schema_version: 1,
            game_build: "test-build".into(),
            preparation_tool: "test".into(),
            files: vec![DataFile {
                path: "update/update.rpf".into(),
                size: bytes.len() as u64,
                sha256: format!("{:x}", Sha256::digest(bytes)),
            }],
        };
        assert!(verify_data_files(&dir, &manifest).is_ok());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn detects_modified_file() {
        let dir = temp_dir();
        fs::create_dir_all(dir.join("update")).unwrap();
        fs::write(dir.join("update/update.rpf"), b"changed").unwrap();
        let manifest = DataManifest {
            schema_version: 1,
            game_build: "test-build".into(),
            preparation_tool: "test".into(),
            files: vec![DataFile { path: "update/update.rpf".into(), size: 7, sha256: "0".repeat(64) }],
        };
        assert!(verify_data_files(&dir, &manifest).unwrap_err().0.contains("SHA-256 mismatch"));
        fs::remove_dir_all(dir).unwrap();
    }
}
