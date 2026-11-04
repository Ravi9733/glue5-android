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
        let actual = hasher.finalize().iter().map(|byte| format!("{byte:02x}")).collect::<String>();
        if actual != entry.sha256 {
            return Err(ManifestError(format!("{}: SHA-256 mismatch", entry.path)));
        }
    }
    Ok(())
}


/// GTA V Legacy RPF7 header fields. This is an inventory-only parser:
/// it does not decrypt the table of contents or extract archive contents.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rpf7Header {
    pub entry_count: u32,
    pub names_length: u32,
    pub names_flags: u8,
    pub encryption_tag: u32,
    pub toc_bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RpfProbe {
    GtaVLegacy(Rpf7Header),
    GtaVEnhancedOrRdr2 { magic: u32 },
    UnsupportedOrUnknown { magic: u32 },
}

const RPF7_MAGIC: u32 = 0x5250_4637;
const RPF8_MAGIC: u32 = 0x5250_4638;

/// Probe the first 16 bytes of an archive header using little-endian fields.
/// RPF7 names length uses the low 28 bits; the upper four bits are preserved
/// separately instead of being misreported as part of the byte length.
pub fn probe_rpf_header(header: &[u8]) -> Result<RpfProbe, ManifestError> {
    if header.len() < 16 {
        return Err(ManifestError("RPF header is truncated (need 16 bytes)".into()));
    }
    let magic = u32::from_le_bytes(header[0..4].try_into().unwrap());
    if magic == RPF8_MAGIC {
        return Ok(RpfProbe::GtaVEnhancedOrRdr2 { magic });
    }
    if magic != RPF7_MAGIC {
        return Ok(RpfProbe::UnsupportedOrUnknown { magic });
    }

    let entry_count = u32::from_le_bytes(header[4..8].try_into().unwrap());
    let packed_names = u32::from_le_bytes(header[8..12].try_into().unwrap());
    let encryption_tag = u32::from_le_bytes(header[12..16].try_into().unwrap());
    let names_length = packed_names & 0x0fff_ffff;
    let names_flags = (packed_names >> 28) as u8;
    let toc_bytes = u64::from(entry_count).checked_mul(16)
        .ok_or_else(|| ManifestError("RPF7 TOC byte count overflow".into()))?;

    Ok(RpfProbe::GtaVLegacy(Rpf7Header {
        entry_count, names_length, names_flags, encryption_tag, toc_bytes,
    }))
}


/// Decoded page allocation metadata for an RPF7 resource entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rpf7ResourcePageSizes {
    pub system_bytes: u64,
    pub graphics_bytes: u64,
}

/// Decode the page-count bitfields used by RPF7 resource entries.
/// This reports allocation metadata only; it does not decode RSC7 pages.
pub fn rpf7_resource_page_sizes(system_flags: u32, graphics_flags: u32) -> Result<Rpf7ResourcePageSizes, ManifestError> {
    fn decode(flags: u32) -> Result<u64, ManifestError> {
        let base = 0x200_u64.checked_shl(flags & 0x0f)
            .ok_or_else(|| ManifestError("RPF7 resource page base-size overflow".into()))?;
        let multiplier = u64::from((flags >> 27) & 1)
            + u64::from((flags >> 26) & 1) * 2
            + u64::from((flags >> 25) & 1) * 4
            + u64::from((flags >> 24) & 1) * 8
            + u64::from((flags >> 17) & 0x7f) * 16
            + u64::from((flags >> 11) & 0x3f) * 32
            + u64::from((flags >> 7) & 0x0f) * 64
            + u64::from((flags >> 5) & 0x03) * 128
            + u64::from((flags >> 4) & 0x01) * 256;
        base.checked_mul(multiplier)
            .ok_or_else(|| ManifestError("RPF7 resource page-size overflow".into()))
    }

    Ok(Rpf7ResourcePageSizes {
        system_bytes: decode(system_flags)?,
        graphics_bytes: decode(graphics_flags)?,
    })
}

/// One entry in a safely inventoried, unencrypted GTA V Legacy RPF7 archive.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rpf7Entry { pub index: u32, pub name: String, pub kind: Rpf7EntryKind }

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Rpf7EntryKind {
    Directory { entries_index: u32, entries_count: u32 },
    BinaryFile { file_size: u32, file_offset_blocks: u32, uncompressed_size: u32, encrypted: bool },
    ResourceFile { file_size: u32, file_offset_blocks: u32, system_flags: u32, graphics_flags: u32 },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rpf7Inventory { pub header: Rpf7Header, pub entries: Vec<Rpf7Entry> }

const RPF_NONE: u32 = 0;
const RPF_OPEN: u32 = 0x4E45_504F;
const MAX_RPF7_ENTRIES: u32 = 1_000_000;
const MAX_RPF7_NAMES: u32 = 64 * 1024 * 1024;

/// Parse only NONE/OPEN RPF7 tables of contents. AES/NG archives are rejected
/// until a separately reviewed key/decryption path exists. Metadata only.
pub fn inventory_rpf7_open(archive: &[u8]) -> Result<Rpf7Inventory, ManifestError> {
    let header = match probe_rpf_header(archive)? {
        RpfProbe::GtaVLegacy(header) => header,
        RpfProbe::GtaVEnhancedOrRdr2 { .. } =>
            return Err(ManifestError("RPF8 is not a GTA V Legacy RPF7 archive".into())),
        RpfProbe::UnsupportedOrUnknown { magic } =>
            return Err(ManifestError(format!("unsupported archive magic: {magic:#010x}"))),
    };
    if header.entry_count > MAX_RPF7_ENTRIES {
        return Err(ManifestError("RPF7 entry count exceeds safety limit".into()));
    }
    if header.names_length > MAX_RPF7_NAMES {
        return Err(ManifestError("RPF7 name table exceeds safety limit".into()));
    }
    if !matches!(header.encryption_tag, RPF_NONE | RPF_OPEN) {
        return Err(ManifestError(format!(
            "RPF7 TOC encryption {:#010x} is unsupported; AES/NG keys are required",
            header.encryption_tag
        )));
    }

    let entries_len = usize::try_from(header.toc_bytes)
        .map_err(|_| ManifestError("RPF7 TOC length does not fit this platform".into()))?;
    let names_start = 16_usize.checked_add(entries_len)
        .ok_or_else(|| ManifestError("RPF7 TOC offset overflow".into()))?;
    let names_len = usize::try_from(header.names_length)
        .map_err(|_| ManifestError("RPF7 name table length does not fit this platform".into()))?;
    let end = names_start.checked_add(names_len)
        .ok_or_else(|| ManifestError("RPF7 name table offset overflow".into()))?;
    if end > archive.len() {
        return Err(ManifestError("RPF7 TOC or name table is truncated".into()));
    }
    let toc = &archive[16..names_start];
    let names = &archive[names_start..end];
    let mut entries = Vec::with_capacity(header.entry_count as usize);

    for index in 0..header.entry_count as usize {
        let start = index.checked_mul(16)
            .ok_or_else(|| ManifestError("RPF7 entry offset overflow".into()))?;
        let record = toc.get(start..start + 16)
            .ok_or_else(|| ManifestError(format!("RPF7 entry {index} is truncated")))?;
        let second = read_u32_le(record, 4)?;
        let (name_offset, kind) = if second == 0x7FFF_FF00 {
            let name_offset = read_u32_le(record, 0)? as usize;
            let entries_index = read_u32_le(record, 8)?;
            let entries_count = read_u32_le(record, 12)?;
            let child_end = entries_index.checked_add(entries_count)
                .ok_or_else(|| ManifestError(format!("RPF7 directory {index} child range overflows")))?;
            if child_end > header.entry_count {
                return Err(ManifestError(format!("RPF7 directory {index} child range is out of bounds")));
            }
            (name_offset, Rpf7EntryKind::Directory { entries_index, entries_count })
        } else {
            let name_offset = u16::from_le_bytes([record[0], record[1]]) as usize;
            let file_size = u32::from(record[2]) | (u32::from(record[3]) << 8) | (u32::from(record[4]) << 16);
            let raw_offset = u32::from(record[5]) | (u32::from(record[6]) << 8) | (u32::from(record[7]) << 16);
            if second & 0x8000_0000 == 0 {
                let uncompressed_size = read_u32_le(record, 8)?;
                let encryption_type = read_u32_le(record, 12)?;
                if encryption_type > 1 {
                    return Err(ManifestError(format!("RPF7 binary entry {index} has invalid encryption flag")));
                }
                (name_offset, Rpf7EntryKind::BinaryFile {
                    file_size, file_offset_blocks: raw_offset, uncompressed_size,
                    encrypted: encryption_type == 1,
                })
            } else {
                let system_flags = read_u32_le(record, 8)?;
                let graphics_flags = read_u32_le(record, 12)?;
                (name_offset, Rpf7EntryKind::ResourceFile {
                    file_size, file_offset_blocks: raw_offset & 0x7F_FFFF, system_flags, graphics_flags,
                })
            }
        };
        let name = read_rpf7_name(names, name_offset)
            .map_err(|e| ManifestError(format!("RPF7 entry {index} name: {e}")))?;
        entries.push(Rpf7Entry { index: index as u32, name, kind });
    }
    Ok(Rpf7Inventory { header, entries })
}

fn read_u32_le(bytes: &[u8], offset: usize) -> Result<u32, ManifestError> {
    let end = offset.checked_add(4)
        .ok_or_else(|| ManifestError("integer field offset overflow".into()))?;
    let field = bytes.get(offset..end)
        .ok_or_else(|| ManifestError("truncated RPF7 integer field".into()))?;
    Ok(u32::from_le_bytes(field.try_into().unwrap()))
}

fn read_rpf7_name(names: &[u8], offset: usize) -> Result<String, &'static str> {
    let tail = names.get(offset..).ok_or("name offset is outside the name table")?;
    let terminator = tail.iter().position(|byte| *byte == 0)
        .ok_or("name is not NUL-terminated inside the name table")?;
    if terminator == 0 { return Err("name is empty"); }
    let name = std::str::from_utf8(&tail[..terminator]).map_err(|_| "name is not valid UTF-8")?;
    if name.contains('/') || name.contains('\\') || name == "." || name == ".." {
        return Err("entry name contains a path separator or traversal component");
    }
    Ok(name.to_owned())
}


/// Result of checking a file payload's 512-byte-block range against its archive.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rpf7PayloadRange {
    pub entry_index: u32,
    pub name: String,
    pub start: u64,
    pub end: u64,
    pub archive_len: u64,
    pub metadata_end: u64,
    pub in_bounds: bool,
    pub overlaps_metadata: bool,
    pub size_is_sentinel: bool,
    pub resolved_size: Option<u64>,
    pub offset_is_sentinel: bool,
}

/// Validate binary and resource payload ranges without extracting payloads.
/// Resource size sentinels are resolved from their bounded 16-byte RSC header.
/// Directory entries are omitted.
pub fn validate_rpf7_payload_ranges(archive: &[u8], inventory: &Rpf7Inventory) -> Vec<Rpf7PayloadRange> {
    let archive_len = archive.len() as u64;
    let metadata_end = 16_u64
        .checked_add(inventory.header.toc_bytes)
        .and_then(|value| value.checked_add(u64::from(inventory.header.names_length)))
        .unwrap_or(u64::MAX);
    inventory.entries.iter().filter_map(|entry| {
        let (file_size, offset_blocks, size_is_sentinel) = match &entry.kind {
            Rpf7EntryKind::BinaryFile { file_size, file_offset_blocks, uncompressed_size, .. } =>
                (if *file_size == 0 { *uncompressed_size as u64 } else { *file_size as u64 },
                 *file_offset_blocks as u64, false),
            Rpf7EntryKind::ResourceFile { file_size, file_offset_blocks, .. } =>
                (*file_size as u64, *file_offset_blocks as u64, *file_size == 0x00FF_FFFF),
            Rpf7EntryKind::Directory { .. } => return None,
        };
        let offset_is_sentinel = offset_blocks == 0x007F_FFFF;
        let start_opt = if offset_is_sentinel { None } else { offset_blocks.checked_mul(512) };
        let resolved_size = if size_is_sentinel {
            start_opt.and_then(|start| {
                let header_end = start.checked_add(16)?;
                if start < metadata_end || header_end > archive_len { return None; }
                let header = archive.get(start as usize..header_end as usize)?;
                Some(u64::from(header[7])
                    | (u64::from(header[14]) << 8)
                    | (u64::from(header[5]) << 16)
                    | (u64::from(header[2]) << 24))
            })
        } else {
            Some(file_size)
        };
        let end_opt = start_opt.and_then(|start| resolved_size.and_then(|size| start.checked_add(size)));
        let (start, end, in_bounds) = match (start_opt, end_opt) {
            (Some(start), Some(end)) => (start, end, end <= archive_len),
            _ => (u64::MAX, u64::MAX, false),
        };
        Some(Rpf7PayloadRange {
            entry_index: entry.index,
            name: entry.name.clone(),
            start,
            end,
            archive_len,
            metadata_end,
            in_bounds,
            overlaps_metadata: start < metadata_end,
            size_is_sentinel,
            resolved_size,
            offset_is_sentinel,
        })
    }).collect()
}


#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NestedRpfStatus {
    InventoriedRpf7(Rpf7Inventory),
    Rpf8Unsupported,
    NotAnRpfArchive,
    SkippedEncryptedPayload,
    SkippedCompressedPayload,
    InvalidPayloadRange,
    EncryptedTocUnsupported,
    InvalidRpf7Toc,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NestedRpfReport {
    pub parent_entry_index: u32,
    pub name: String,
    pub status: NestedRpfStatus,
}

/// Inspect direct .rpf child files in an already parsed RPF7 archive.
/// Only unencrypted, byte-for-byte stored payloads are inspected. This is
/// deliberately one level deep; it does not recurse or extract file contents.
pub fn scan_nested_rpf7_open(archive: &[u8], inventory: &Rpf7Inventory) -> Vec<NestedRpfReport> {
    let mut reports = Vec::new();
    for entry in &inventory.entries {
        if !entry.name.to_ascii_lowercase().ends_with(".rpf") { continue; }
        let (file_size, offset_blocks, uncompressed_size, encrypted) = match &entry.kind {
            Rpf7EntryKind::BinaryFile { file_size, file_offset_blocks, uncompressed_size, encrypted } =>
                (*file_size as usize, *file_offset_blocks as usize, *uncompressed_size as usize, *encrypted),
            _ => continue,
        };

        let status = if encrypted {
            NestedRpfStatus::SkippedEncryptedPayload
        } else if file_size != uncompressed_size {
            NestedRpfStatus::SkippedCompressedPayload
        } else {
            let start = offset_blocks.checked_mul(512);
            let end = start.and_then(|value| value.checked_add(file_size));
            match (start, end) {
                (Some(start), Some(end))
                    if start >= 16 + inventory.header.toc_bytes as usize + inventory.header.names_length as usize
                        && end <= archive.len() => {
                    match probe_rpf_header(&archive[start..end]) {
                        Ok(RpfProbe::GtaVLegacy(_)) => match inventory_rpf7_open(&archive[start..end]) {
                            Ok(nested) => NestedRpfStatus::InventoriedRpf7(nested),
                            Err(error) if error.0.contains("AES/NG keys are required")
                                => NestedRpfStatus::EncryptedTocUnsupported,
                            Err(_) => NestedRpfStatus::InvalidRpf7Toc,
                        },
                        Ok(RpfProbe::GtaVEnhancedOrRdr2 { .. }) => NestedRpfStatus::Rpf8Unsupported,
                        _ => NestedRpfStatus::NotAnRpfArchive,
                    }
                }
                _ => NestedRpfStatus::InvalidPayloadRange,
            }
        };
        reports.push(NestedRpfReport {
            parent_entry_index: entry.index,
            name: entry.name.clone(),
            status,
        });
    }
    reports
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


    #[test]
    fn probes_rpf7_header_and_masks_name_length_flags() {
        let mut h = [0_u8; 16];
        h[0..4].copy_from_slice(&0x5250_4637_u32.to_le_bytes());
        h[4..8].copy_from_slice(&3_u32.to_le_bytes());
        h[8..12].copy_from_slice(&0xA000_0123_u32.to_le_bytes());
        h[12..16].copy_from_slice(&0_u32.to_le_bytes());
        let result = probe_rpf_header(&h).unwrap();
        assert_eq!(result, RpfProbe::GtaVLegacy(Rpf7Header {
            entry_count: 3, names_length: 0x123, names_flags: 0xA,
            encryption_tag: 0, toc_bytes: 48,
        }));
    }

    #[test]
    fn identifies_rpf8_without_treating_it_as_legacy() {
        let mut h = [0_u8; 16];
        h[0..4].copy_from_slice(&0x5250_4638_u32.to_le_bytes());
        assert_eq!(probe_rpf_header(&h).unwrap(), RpfProbe::GtaVEnhancedOrRdr2 { magic: 0x5250_4638 });
    }

    #[test]
    fn rejects_short_rpf_header() {
        assert!(probe_rpf_header(&[0; 15]).is_err());
    }

    #[test] fn parses_manifest_and_normalizes_hash() { let m=parse_data_manifest(VALID).unwrap(); assert_eq!(m.files[0].sha256, "a".repeat(64)); }
    #[test] fn rejects_unknown_schema() { assert!(parse_data_manifest(&VALID.replace("\"schema_version\":1", "\"schema_version\":2")).is_err()); }
    #[test] fn rejects_traversal_absolute_and_backslash_paths() {
        for bad in ["../escape", "/absolute", "a/../../b", "a\\b"] {
            let json_path = serde_json::to_string(bad).unwrap();
            let input = VALID.replace("\"update/update.rpf\"", &json_path);
            assert!(parse_data_manifest(&input).is_err(), "{bad}");
        }
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
                sha256: Sha256::digest(bytes).iter().map(|byte| format!("{byte:02x}")).collect(),
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

    fn minimal_open_rpf7() -> Vec<u8> {
        let mut data = vec![0_u8; 516];
        data[0..4].copy_from_slice(&RPF7_MAGIC.to_le_bytes());
        data[4..8].copy_from_slice(&2_u32.to_le_bytes());
        data[8..12].copy_from_slice(&14_u32.to_le_bytes());
        data[12..16].copy_from_slice(&RPF_OPEN.to_le_bytes());
        data[16..20].copy_from_slice(&0_u32.to_le_bytes());
        data[20..24].copy_from_slice(&0x7FFF_FF00_u32.to_le_bytes());
        data[24..28].copy_from_slice(&1_u32.to_le_bytes());
        data[28..32].copy_from_slice(&1_u32.to_le_bytes());
        data[32..34].copy_from_slice(&5_u16.to_le_bytes());
        data[37] = 1;
        data[40..44].copy_from_slice(&4_u32.to_le_bytes());
        data[44..48].copy_from_slice(&0_u32.to_le_bytes());
        data[48..62].copy_from_slice(b"root\0file.bin\0");
        data
    }

    #[test]
    fn inventories_open_rpf7_directory_and_binary_entry() {
        let inventory = inventory_rpf7_open(&minimal_open_rpf7()).unwrap();
        assert_eq!(inventory.entries.len(), 2);
        assert_eq!(inventory.entries[0].name, "root");
        assert_eq!(inventory.entries[0].kind, Rpf7EntryKind::Directory { entries_index: 1, entries_count: 1 });
        assert_eq!(inventory.entries[1].name, "file.bin");
        assert_eq!(inventory.entries[1].kind, Rpf7EntryKind::BinaryFile {
            file_size: 0, file_offset_blocks: 1, uncompressed_size: 4, encrypted: false,
        });
    }

    #[test]
    fn validates_binary_payload_range_against_archive_length() {
        let data = minimal_open_rpf7();
        let inventory = inventory_rpf7_open(&data).unwrap();
        let ranges = validate_rpf7_payload_ranges(&data, &inventory);
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0].name, "file.bin");
        assert_eq!(ranges[0].start, 512);
        assert_eq!(ranges[0].end, 516);
        assert!(ranges[0].in_bounds);
        assert!(!ranges[0].size_is_sentinel);
        assert_eq!(ranges[0].resolved_size, Some(4));
        assert!(!ranges[0].offset_is_sentinel);
    }

    #[test]
    fn validates_rpf7_resource_payload_range_and_flags() {
        let mut data = minimal_open_rpf7();
        data[34..37].copy_from_slice(&[4, 0, 0]);
        data[39] |= 0x80;
        data[40..44].copy_from_slice(&0x1234_5678_u32.to_le_bytes());
        data[44..48].copy_from_slice(&0x8765_4321_u32.to_le_bytes());
        let inventory = inventory_rpf7_open(&data).unwrap();
        assert_eq!(inventory.entries[1].kind, Rpf7EntryKind::ResourceFile {
            file_size: 4, file_offset_blocks: 1,
            system_flags: 0x1234_5678, graphics_flags: 0x8765_4321,
        });
        let ranges = validate_rpf7_payload_ranges(&data, &inventory);
        assert_eq!(ranges[0].start, 512);
        assert_eq!(ranges[0].end, 516);
        assert!(ranges[0].in_bounds);
    }

    #[test]
    fn decodes_rpf7_resource_system_and_graphics_page_sizes() {
        let sizes = rpf7_resource_page_sizes((1 << 0) | (1 << 27), (1 << 1) | (1 << 26)).unwrap();
        assert_eq!(sizes.system_bytes, 512);
        assert_eq!(sizes.graphics_bytes, 2048);
        assert_eq!(rpf7_resource_page_sizes(0, 0).unwrap(), Rpf7ResourcePageSizes {
            system_bytes: 0, graphics_bytes: 0,
        });
    }

    #[test]
    fn resolves_resource_size_sentinel_from_bounded_rsc_header() {
        let mut data = minimal_open_rpf7();
        data.resize(528, 0);
        data[34..37].copy_from_slice(&[0xFF, 0xFF, 0xFF]);
        data[39] |= 0x80;
        // RSC7 size bytes are stored in a scrambled four-byte order.
        data[512 + 7] = 16;
        let inventory = inventory_rpf7_open(&data).unwrap();
        let ranges = validate_rpf7_payload_ranges(&data, &inventory);
        assert!(ranges[0].size_is_sentinel);
        assert_eq!(ranges[0].resolved_size, Some(16));
        assert_eq!(ranges[0].start, 512);
        assert_eq!(ranges[0].end, 528);
        assert!(ranges[0].in_bounds);
    }

    #[test]
    fn leaves_resource_size_sentinel_unresolved_when_header_is_truncated() {
        let mut data = minimal_open_rpf7();
        data[34..37].copy_from_slice(&[0xFF, 0xFF, 0xFF]);
        data[39] |= 0x80;
        let inventory = inventory_rpf7_open(&data).unwrap();
        let ranges = validate_rpf7_payload_ranges(&data, &inventory);
        assert!(ranges[0].size_is_sentinel);
        assert_eq!(ranges[0].resolved_size, None);
        assert!(!ranges[0].in_bounds);
    }

    #[test]
    fn rejects_rpf7_all_ones_payload_offset_sentinel() {
        let mut data = minimal_open_rpf7();
        data[37..40].copy_from_slice(&[0xFF, 0xFF, 0xFF]);
        let inventory = inventory_rpf7_open(&data).unwrap();
        let ranges = validate_rpf7_payload_ranges(&data, &inventory);
        assert!(ranges[0].offset_is_sentinel);
        assert!(!ranges[0].in_bounds);
    }

    #[test]
    fn reports_payload_that_overlaps_archive_metadata() {
        let mut data = minimal_open_rpf7();
        // Block zero points into the archive header/TOC, not the payload area.
        data[37] = 0;
        let inventory = inventory_rpf7_open(&data).unwrap();
        let ranges = validate_rpf7_payload_ranges(&data, &inventory);
        assert_eq!(ranges.len(), 1);
        assert!(ranges[0].in_bounds);
        assert!(ranges[0].overlaps_metadata);
        assert_eq!(ranges[0].metadata_end, 62);
    }

    #[test]
    fn reports_out_of_bounds_binary_payload_without_extracting() {
        let mut data = minimal_open_rpf7();
        // Point the binary entry at block 2, beyond this 516-byte archive.
        data[37] = 2;
        let inventory = inventory_rpf7_open(&data).unwrap();
        let ranges = validate_rpf7_payload_ranges(&data, &inventory);
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0].start, 1024);
        assert!(!ranges[0].in_bounds);
    }

    #[test]
    fn refuses_encrypted_rpf7_toc_without_keys() {
        let mut data = minimal_open_rpf7();
        data[12..16].copy_from_slice(&0x0FFF_FFF9_u32.to_le_bytes());
        assert!(inventory_rpf7_open(&data).unwrap_err().0.contains("AES/NG keys are required"));
    }

    #[test]
    fn rejects_rpf7_directory_ranges_outside_entry_table() {
        let mut data = minimal_open_rpf7();
        data[28..32].copy_from_slice(&9_u32.to_le_bytes());
        assert!(inventory_rpf7_open(&data).unwrap_err().0.contains("child range is out of bounds"));
    }

    #[test]
    fn rejects_unterminated_rpf7_names() {
        let mut data = minimal_open_rpf7();
        data[61] = b'x';
        assert!(inventory_rpf7_open(&data).unwrap_err().0.contains("NUL-terminated"));
    }


    fn outer_with_nested_open_rpf7() -> Vec<u8> {
        let inner = minimal_open_rpf7();
        let mut outer = vec![0_u8; 1024 + inner.len()];
        outer[0..4].copy_from_slice(&RPF7_MAGIC.to_le_bytes());
        outer[4..8].copy_from_slice(&2_u32.to_le_bytes());
        outer[8..12].copy_from_slice(&16_u32.to_le_bytes());
        outer[12..16].copy_from_slice(&RPF_OPEN.to_le_bytes());
        // Root directory has one child.
        outer[16..20].copy_from_slice(&0_u32.to_le_bytes());
        outer[20..24].copy_from_slice(&0x7FFF_FF00_u32.to_le_bytes());
        outer[24..28].copy_from_slice(&1_u32.to_le_bytes());
        outer[28..32].copy_from_slice(&1_u32.to_le_bytes());
        // Nested archive binary entry, stored uncompressed at block 2.
        outer[32..34].copy_from_slice(&5_u16.to_le_bytes());
        let len = inner.len() as u32;
        outer[34] = len as u8;
        outer[35] = (len >> 8) as u8;
        outer[36] = (len >> 16) as u8;
        outer[37] = 2;
        outer[40..44].copy_from_slice(&len.to_le_bytes());
        outer[44..48].copy_from_slice(&0_u32.to_le_bytes());
        outer[48..64].copy_from_slice(b"root\0nested.rpf\0");
        outer[1024..].copy_from_slice(&inner);
        outer
    }

    #[test]
    fn inventories_direct_nested_open_rpf7_without_extracting() {
        let outer = outer_with_nested_open_rpf7();
        let inventory = inventory_rpf7_open(&outer).unwrap();
        let nested = scan_nested_rpf7_open(&outer, &inventory);
        assert_eq!(nested.len(), 1);
        assert_eq!(nested[0].name, "nested.rpf");
        match &nested[0].status {
            NestedRpfStatus::InventoriedRpf7(inner) => assert_eq!(inner.entries.len(), 2),
            other => panic!("unexpected nested archive result: {other:?}"),
        }
    }

    #[test]
    fn reports_encrypted_nested_rpf7_toc_separately() {
        let mut outer = outer_with_nested_open_rpf7();
        outer[1024 + 12..1024 + 16].copy_from_slice(&0x0FFF_FFF9_u32.to_le_bytes());
        let inventory = inventory_rpf7_open(&outer).unwrap();
        let nested = scan_nested_rpf7_open(&outer, &inventory);
        assert_eq!(nested[0].status, NestedRpfStatus::EncryptedTocUnsupported);
    }

    #[test]
    fn reports_truncated_nested_rpf7_toc_as_invalid() {
        let mut outer = outer_with_nested_open_rpf7();
        // Keep only the 16-byte nested header in the declared payload.
        outer[34] = 16;
        outer[35] = 0;
        outer[36] = 0;
        outer[40..44].copy_from_slice(&16_u32.to_le_bytes());
        let inventory = inventory_rpf7_open(&outer).unwrap();
        let nested = scan_nested_rpf7_open(&outer, &inventory);
        assert_eq!(nested[0].status, NestedRpfStatus::InvalidRpf7Toc);
    }

    #[test]
    fn skips_nested_archive_payload_with_compressed_size_mismatch() {
        let mut outer = outer_with_nested_open_rpf7();
        outer[40..44].copy_from_slice(&1_u32.to_le_bytes());
        let inventory = inventory_rpf7_open(&outer).unwrap();
        let nested = scan_nested_rpf7_open(&outer, &inventory);
        assert_eq!(nested[0].status, NestedRpfStatus::SkippedCompressedPayload);
    }

}
