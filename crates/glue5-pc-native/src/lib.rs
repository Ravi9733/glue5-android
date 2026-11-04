//! Capability gate and manifest validation for a future native PC-game backend.
use sha2::{Digest, Sha256};
use flate2::read::DeflateDecoder;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Component, Path};

/// Versioned JSON placement-mod parser for the experimental PC content layer.
pub mod placement;
pub mod mount;
pub use placement::{apply_json_placements, apply_resolved_json_placements, parse_json_placements, validate_json_placement_assets, JsonPlacement, JsonPlacementDocument, PlacementApplyError, PlacementAssetError, PlacementError, PlacementSink, ResolvedPlacementSink};
pub use mount::{add_rpf7_inventory_to_mount, add_rpf7_inventory_to_mount_at, AssetMountIndex, MountError, MountedAsset, MountedAssetKind};

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

/// Minimal, opaque RSC7 resource-header probe.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rsc7Header {
    pub magic: [u8; 4],
    /// Preserved verbatim; semantics are intentionally not assumed.
    pub opaque_word: u32,
    pub header_bytes: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Rsc7ResourceStatus { Valid, InvalidRange, HeaderTruncated, WrongMagic }

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rsc7ResourceReport {
    pub entry_index: u32,
    pub name: String,
    pub status: Rsc7ResourceStatus,
    pub header: Option<Rsc7Header>,
}

/// Inspect only the first 16 bytes of bounded RPF7 resource entries.
/// This is diagnostic metadata: it does not decode page maps or asset data.
pub fn inspect_rpf7_rsc7_headers(
    archive: &[u8],
    ranges: &[Rpf7PayloadRange],
) -> Vec<Rsc7ResourceReport> {
    ranges.iter().filter(|range| range.resource_page_sizes.is_some()).map(|range| {
        let mut report = Rsc7ResourceReport {
            entry_index: range.entry_index,
            name: range.name.clone(),
            status: Rsc7ResourceStatus::InvalidRange,
            header: None,
        };
        if !range.in_bounds || range.overlaps_metadata || range.offset_is_sentinel {
            return report;
        }
        let Some(size) = range.resolved_size else { return report; };
        if size < 16 || range.end.checked_sub(range.start).is_none_or(|length| length < 16) {
            report.status = Rsc7ResourceStatus::HeaderTruncated;
            return report;
        }
        let Ok(start) = usize::try_from(range.start) else { return report; };
        let Some(end_u64) = range.start.checked_add(16) else { return report; };
        let Ok(end) = usize::try_from(end_u64) else { return report; };
        let Some(payload) = archive.get(start..end) else { return report; };
        match probe_rsc7_resource_header(payload) {
            Ok(header) => {
                report.status = Rsc7ResourceStatus::Valid;
                report.header = Some(header);
            }
            Err(error) if error.0.contains("truncated") => report.status = Rsc7ResourceStatus::HeaderTruncated,
            Err(_) => report.status = Rsc7ResourceStatus::WrongMagic,
        }
        report
    }).collect()
}

/// Validate RSC7 magic and the minimum 16-byte header. This does not decode
/// page tables, decompress, decrypt, or interpret resource payload contents.
pub fn probe_rsc7_resource_header(payload: &[u8]) -> Result<Rsc7Header, ManifestError> {
    if payload.len() < 16 {
        return Err(ManifestError("RSC7 resource header is truncated (need 16 bytes)".into()));
    }
    let magic: [u8; 4] = payload[0..4].try_into().unwrap();
    if &magic != b"RSC7" {
        return Err(ManifestError("resource payload does not have RSC7 magic".into()));
    }
    Ok(Rsc7Header {
        magic,
        opaque_word: u32::from_le_bytes(payload[4..8].try_into().unwrap()),
        header_bytes: 16,
    })
}

/// A fully inflated RSC7 resource body, split into system and graphics segments.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodedRsc7Resource {
    pub version: u32,
    pub page_sizes: Rpf7ResourcePageSizes,
    /// Raw-DEFLATE output: system segment followed by graphics segment.
    pub body: Vec<u8>,
}

/// Inflate an RSC7 resource with strict output bounds derived from its page flags.
/// The caller supplies a hard cap to prevent malicious compressed-size expansion.
/// This returns the packed page image; it does not relocate virtual pointers or
/// decode GTA object structures.
pub fn decode_rsc7_resource(
    payload: &[u8],
    max_output_bytes: usize,
) -> Result<DecodedRsc7Resource, ManifestError> {
    if payload.len() < 16 {
        return Err(ManifestError("RSC7 resource header is truncated (need 16 bytes)".into()));
    }
    if &payload[0..4] != b"RSC7" {
        return Err(ManifestError("resource payload does not have RSC7 magic".into()));
    }
    let version = u32::from_le_bytes(payload[4..8].try_into().unwrap());
    let system_flags = u32::from_le_bytes(payload[8..12].try_into().unwrap());
    let graphics_flags = u32::from_le_bytes(payload[12..16].try_into().unwrap());
    let page_sizes = rpf7_resource_page_sizes(system_flags, graphics_flags)?;
    let expected = page_sizes.system_bytes.checked_add(page_sizes.graphics_bytes)
        .ok_or_else(|| ManifestError("RSC7 decompressed size overflow".into()))?;
    let expected = usize::try_from(expected)
        .map_err(|_| ManifestError("RSC7 decompressed size does not fit this platform".into()))?;
    if expected == 0 {
        return Err(ManifestError("RSC7 page flags describe an empty resource".into()));
    }
    if expected > max_output_bytes {
        return Err(ManifestError(format!("RSC7 output size {expected} exceeds configured limit {max_output_bytes}")));
    }
    let decoder = DeflateDecoder::new(&payload[16..]);
    let mut bounded = decoder.take((expected as u64).saturating_add(1));
    let mut body = Vec::with_capacity(expected);
    bounded.read_to_end(&mut body)
        .map_err(|error| ManifestError(format!("RSC7 raw-DEFLATE stream is invalid: {error}")))?;
    if body.len() != expected {
        return Err(ManifestError(format!("RSC7 decompressed size mismatch: expected {expected}, got {}", body.len())));
    }
    Ok(DecodedRsc7Resource { version, page_sizes, body })
}

/// Metadata returned after streaming an RSC7 resource into a caller-owned writer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rsc7StreamReport {
    pub version: u32,
    pub page_sizes: Rpf7ResourcePageSizes,
    pub bytes_written: u64,
}

/// Decode one RSC7 payload directly from a seekable archive without retaining
/// the decompressed resource in memory. The output writer may contain partial
/// Borrow the decompressed RSC7 system and graphics page regions.
/// The body must exactly match the sizes declared by the RSC7 header; this
/// adapter does not infer GTA object layouts or relocate pointers.
pub fn split_rsc7_resource_pages<'a>(
    body: &'a [u8],
    page_sizes: Rpf7ResourcePageSizes,
) -> Result<(&'a [u8], &'a [u8]), ManifestError> {
    let system_len = usize::try_from(page_sizes.system_bytes)
        .map_err(|_| ManifestError("RSC7 system page size does not fit this platform".into()))?;
    let graphics_len = usize::try_from(page_sizes.graphics_bytes)
        .map_err(|_| ManifestError("RSC7 graphics page size does not fit this platform".into()))?;
    let expected = system_len.checked_add(graphics_len)
        .ok_or_else(|| ManifestError("RSC7 page region length overflow".into()))?;
    if body.len() != expected {
        return Err(ManifestError(format!(
            "RSC7 body length {} does not match declared page regions {}", body.len(), expected
        )));
    }
    let (system, graphics) = body.split_at(system_len);
    Ok((system, graphics))
}

#[cfg(test)]
mod rsc7_page_split_tests {
    use super::*;

    #[test]
    fn splits_system_and_graphics_pages_at_declared_boundary() {
        let sizes = Rpf7ResourcePageSizes { system_bytes: 6, graphics_bytes: 4 };
        let (system, graphics) = split_rsc7_resource_pages(b"SYSTEMGPU!", sizes).unwrap();
        assert_eq!(system, b"SYSTEM");
        assert_eq!(graphics, b"GPU!");
    }

    #[test]
    fn rejects_short_long_and_overflowing_page_regions() {
        let sizes = Rpf7ResourcePageSizes { system_bytes: 6, graphics_bytes: 4 };
        assert!(split_rsc7_resource_pages(b"short", sizes).is_err());
        assert!(split_rsc7_resource_pages(b"SYSTEMGPU!!", sizes).is_err());
        let overflow = Rpf7ResourcePageSizes { system_bytes: u64::MAX, graphics_bytes: 1 };
        assert!(split_rsc7_resource_pages(b"", overflow).is_err());
    }
}

/// bytes if the compressed stream is corrupt or has an incorrect final size.
pub fn decode_rsc7_resource_stream<R: Read + Seek, W: Write>(
    reader: &mut R,
    payload_offset: u64,
    payload_size: u64,
    output: &mut W,
    max_output_bytes: usize,
) -> Result<Rsc7StreamReport, ManifestError> {
    if payload_size < 16 {
        return Err(ManifestError("RSC7 resource header is truncated (need 16 bytes)".into()));
    }
    reader.seek(SeekFrom::Start(payload_offset))
        .map_err(|e| ManifestError(format!("cannot seek to RSC7 payload: {e}")))?;
    let mut header = [0_u8; 16];
    reader.read_exact(&mut header)
        .map_err(|e| ManifestError(format!("cannot read RSC7 header: {e}")))?;
    if &header[0..4] != b"RSC7" {
        return Err(ManifestError("resource payload does not have RSC7 magic".into()));
    }
    let version = u32::from_le_bytes(header[4..8].try_into().unwrap());
    let page_sizes = rpf7_resource_page_sizes(
        u32::from_le_bytes(header[8..12].try_into().unwrap()),
        u32::from_le_bytes(header[12..16].try_into().unwrap()),
    )?;
    let expected = page_sizes.system_bytes.checked_add(page_sizes.graphics_bytes)
        .ok_or_else(|| ManifestError("RSC7 decompressed size overflow".into()))?;
    let expected = usize::try_from(expected)
        .map_err(|_| ManifestError("RSC7 decompressed size does not fit this platform".into()))?;
    if expected == 0 {
        return Err(ManifestError("RSC7 page flags describe an empty resource".into()));
    }
    if expected > max_output_bytes {
        return Err(ManifestError(format!("RSC7 output size {expected} exceeds configured limit {max_output_bytes}")));
    }
    let compressed_size = payload_size - 16;
    let limited = (&mut *reader).take(compressed_size);
    let mut decoder = DeflateDecoder::new(limited);
    let mut buffer = [0_u8; 64 * 1024];
    let mut written = 0_u64;
    loop {
        let count = decoder.read(&mut buffer)
            .map_err(|e| ManifestError(format!("RSC7 raw-DEFLATE stream is invalid: {e}")))?;
        if count == 0 { break; }
        written = written.checked_add(count as u64)
            .ok_or_else(|| ManifestError("RSC7 output byte count overflow".into()))?;
        if written > expected as u64 {
            return Err(ManifestError("RSC7 decompressed size exceeds page allocation metadata".into()));
        }
        output.write_all(&buffer[..count])
            .map_err(|e| ManifestError(format!("cannot write decoded RSC7 bytes: {e}")))?;
    }
    if written != expected as u64 {
        return Err(ManifestError(format!("RSC7 decompressed size mismatch: expected {expected}, got {written}")));
    }
    Ok(Rsc7StreamReport { version, page_sizes, bytes_written: written })
}


/// Writer adapter that routes the decoded RSC7 stream into distinct page buffers.
struct Rsc7PageWriters<'a, WS: Write, WG: Write> {
    system: &'a mut WS,
    graphics: &'a mut WG,
    system_remaining: u64,
    system_written: u64,
    graphics_written: u64,
}

impl<WS: Write, WG: Write> Write for Rsc7PageWriters<'_, WS, WG> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.is_empty() { return Ok(0); }
        if self.system_remaining > 0 {
            let count = bytes.len().min(usize::try_from(self.system_remaining).unwrap_or(usize::MAX));
            self.system.write_all(&bytes[..count])?;
            self.system_remaining -= count as u64;
            self.system_written += count as u64;
            if count < bytes.len() {
                self.graphics.write_all(&bytes[count..])?;
                self.graphics_written += (bytes.len() - count) as u64;
            }
        } else {
            self.graphics.write_all(bytes)?;
            self.graphics_written += bytes.len() as u64;
        }
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.system.flush()?;
        self.graphics.flush()
    }
}

/// Stream an RSC7 resource into separate system and graphics page writers.
/// The page boundary comes only from the resource header; no GTA object layout
/// or GPU allocation is inferred. Both outputs may contain partial data on error.
pub fn decode_rsc7_resource_pages_stream<R: Read + Seek, WS: Write, WG: Write>(
    reader: &mut R,
    payload_offset: u64,
    payload_size: u64,
    system_output: &mut WS,
    graphics_output: &mut WG,
    max_output_bytes: usize,
) -> Result<Rsc7StreamReport, ManifestError> {
    if payload_size < 16 {
        return Err(ManifestError("RSC7 resource header is truncated (need 16 bytes)".into()));
    }
    reader.seek(SeekFrom::Start(payload_offset))
        .map_err(|e| ManifestError(format!("cannot seek to RSC7 payload: {e}")))?;
    let mut header = [0_u8; 16];
    reader.read_exact(&mut header)
        .map_err(|e| ManifestError(format!("cannot read RSC7 header: {e}")))?;
    if &header[..4] != b"RSC7" {
        return Err(ManifestError("resource payload does not have RSC7 magic".into()));
    }
    let page_sizes = rpf7_resource_page_sizes(
        u32::from_le_bytes(header[8..12].try_into().unwrap()),
        u32::from_le_bytes(header[12..16].try_into().unwrap()),
    )?;
    let expected = page_sizes.system_bytes.checked_add(page_sizes.graphics_bytes)
        .ok_or_else(|| ManifestError("RSC7 decompressed size overflow".into()))?;
    if expected > max_output_bytes as u64 {
        return Err(ManifestError(format!("RSC7 output size {expected} exceeds configured limit {max_output_bytes}")));
    }
    let mut pages = Rsc7PageWriters {
        system: system_output,
        graphics: graphics_output,
        system_remaining: page_sizes.system_bytes,
        system_written: 0,
        graphics_written: 0,
    };
    let report = decode_rsc7_resource_stream(
        reader, payload_offset, payload_size, &mut pages, max_output_bytes,
    )?;
    if pages.system_remaining != 0
        || pages.system_written != page_sizes.system_bytes
        || pages.graphics_written != page_sizes.graphics_bytes
    {
        return Err(ManifestError("RSC7 page writer byte counts do not match declared allocations".into()));
    }
    Ok(report)
}


#[cfg(test)]
mod rsc7_split_stream_tests {
    use super::*;
    use flate2::{write::DeflateEncoder, Compression};
    use std::io::Cursor;

    fn fixture() -> Vec<u8> {
        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&vec![b'S'; 512]).unwrap();
        encoder.write_all(&vec![b'G'; 512]).unwrap();
        let compressed = encoder.finish().unwrap();
        let mut payload = b"RSC7".to_vec();
        payload.extend_from_slice(&1_u32.to_le_bytes());
        payload.extend_from_slice(&(1_u32 << 27).to_le_bytes());
        payload.extend_from_slice(&(1_u32 << 27).to_le_bytes());
        payload.extend_from_slice(&compressed);
        payload
    }

    #[test]
    fn streams_system_and_graphics_pages_to_separate_writers() {
        let payload = fixture();
        let mut reader = Cursor::new(payload.clone());
        let mut system = Vec::new();
        let mut graphics = Vec::new();
        let report = decode_rsc7_resource_pages_stream(
            &mut reader, 0, payload.len() as u64, &mut system, &mut graphics, 2048,
        ).unwrap();
        assert_eq!(report.bytes_written, 1024);
        assert_eq!(system, vec![b'S'; 512]);
        assert_eq!(graphics, vec![b'G'; 512]);
    }

    #[test]
    fn rejects_page_stream_output_cap_before_writing() {
        let payload = fixture();
        let mut reader = Cursor::new(payload.clone());
        let mut system = Vec::new();
        let mut graphics = Vec::new();
        assert!(decode_rsc7_resource_pages_stream(
            &mut reader, 0, payload.len() as u64, &mut system, &mut graphics, 512,
        ).unwrap_err().0.contains("exceeds configured limit"));
        assert!(system.is_empty());
        assert!(graphics.is_empty());
    }
}

/// Outcome for extracting one bounded RSC7 resource payload from an RPF7 archive.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Rsc7ExtractStatus {
    Decoded,
    InvalidRange,
    OutputLimitExceeded,
    DecodeFailed,
}

/// Per-entry result. A failed entry is reported without aborting other archive work.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rsc7ExtractReport {
    pub entry_index: u32,
    pub name: String,
    pub status: Rsc7ExtractStatus,
    pub resource: Option<DecodedRsc7Resource>,
    pub error: Option<String>,
}

/// Extract and decode one previously validated resource range.
/// The caller controls the maximum decompressed size; no archive-wide allocation
/// or batch retention is performed by this function.
pub fn extract_rpf7_rsc7_resource(
    archive: &[u8],
    range: &Rpf7PayloadRange,
    max_output_bytes: usize,
) -> Rsc7ExtractReport {
    let mut report = Rsc7ExtractReport {
        entry_index: range.entry_index,
        name: range.name.clone(),
        status: Rsc7ExtractStatus::InvalidRange,
        resource: None,
        error: None,
    };
    let fail = |mut report: Rsc7ExtractReport, status, message: String| {
        report.status = status;
        report.error = Some(message);
        report
    };
    if range.resource_page_sizes.is_none() {
        return fail(report, Rsc7ExtractStatus::InvalidRange, "RPF7 entry is not a resource payload".into());
    }
    if !range.in_bounds || range.overlaps_metadata || range.offset_is_sentinel {
        return fail(report, Rsc7ExtractStatus::InvalidRange, "RPF7 resource range is invalid or overlaps archive metadata".into());
    }
    let Some(size) = range.resolved_size else {
        return fail(report, Rsc7ExtractStatus::InvalidRange, "RPF7 resource payload size is unresolved".into());
    };
    if range.end < range.start || range.end - range.start != size {
        return fail(report, Rsc7ExtractStatus::InvalidRange, "RPF7 resource range length is inconsistent".into());
    }
    let (Ok(start), Ok(end)) = (usize::try_from(range.start), usize::try_from(range.end)) else {
        return fail(report, Rsc7ExtractStatus::InvalidRange, "RPF7 resource range does not fit this platform".into());
    };
    let Some(payload) = archive.get(start..end) else {
        return fail(report, Rsc7ExtractStatus::InvalidRange, "RPF7 resource range exceeds archive bytes".into());
    };
    match decode_rsc7_resource(payload, max_output_bytes) {
        Ok(resource) => {
            report.status = Rsc7ExtractStatus::Decoded;
            report.resource = Some(resource);
            report
        }
        Err(error) => {
            let status = if error.0.contains("exceeds configured limit") {
                Rsc7ExtractStatus::OutputLimitExceeded
            } else {
                Rsc7ExtractStatus::DecodeFailed
            };
            fail(report, status, error.0)
        }
    }
}

/// Safely select an RSC7 resource by its TOC entry index. The archive is
/// inventoried from its own bytes inside this API, so callers cannot supply a
/// forged payload range or mismatched inventory.
pub fn extract_rpf7_rsc7_entry(
    archive: &[u8],
    entry_index: u32,
    max_output_bytes: usize,
) -> Rsc7ExtractReport {
    let invalid = |name: String, message: String| Rsc7ExtractReport {
        entry_index,
        name,
        status: Rsc7ExtractStatus::InvalidRange,
        resource: None,
        error: Some(message),
    };
    let inventory = match inventory_rpf7_open(archive) {
        Ok(inventory) => inventory,
        Err(error) => return invalid(String::new(), error.0),
    };
    let ranges = validate_rpf7_payload_ranges(archive, &inventory);
    let Some(range) = ranges.iter().find(|range| range.entry_index == entry_index) else {
        let name = inventory.entries.iter().find(|entry| entry.index == entry_index)
            .map(|entry| entry.name.clone()).unwrap_or_default();
        return invalid(name, "TOC entry is not a file payload in this archive".into());
    };
    extract_rpf7_rsc7_resource(archive, range, max_output_bytes)
}

/// Validate an RPF7 TOC entry and stream its RSC7 output to a writer.
/// The archive is scanned for metadata/range validation, while decompressed
/// bytes are emitted in fixed-size chunks rather than retained in a Vec.
pub fn extract_rpf7_rsc7_entry_to_writer<W: Write>(
    archive: &[u8],
    entry_index: u32,
    output: &mut W,
    max_output_bytes: usize,
) -> Result<Rsc7StreamReport, ManifestError> {
    let inventory = inventory_rpf7_open(archive)?;
    let ranges = validate_rpf7_payload_ranges(archive, &inventory);
    let range = ranges.iter().find(|range| range.entry_index == entry_index)
        .ok_or_else(|| ManifestError("TOC entry is not a file payload in this archive".into()))?;
    if range.resource_page_sizes.is_none() {
        return Err(ManifestError("TOC entry is not an RSC7 resource payload".into()));
    }
    if !range.in_bounds || range.overlaps_metadata || range.offset_is_sentinel {
        return Err(ManifestError("RPF7 resource range is invalid or overlaps archive metadata".into()));
    }
    let size = range.resolved_size
        .ok_or_else(|| ManifestError("RPF7 resource payload size is unresolved".into()))?;
    if range.end < range.start || range.end - range.start != size || range.end > archive.len() as u64 {
        return Err(ManifestError("RPF7 resource range length is inconsistent or out of bounds".into()));
    }
    let mut reader = std::io::Cursor::new(archive);
    decode_rsc7_resource_stream(&mut reader, range.start, size, output, max_output_bytes)
}

impl DecodedRsc7Resource {
    /// Resolve and borrow a virtual pointer from this decoded resource.
    /// Refuses to read if the body length disagrees with its page metadata.
    pub fn read_pointer(&self, pointer: u64, length: u32) -> Result<Option<&[u8]>, ManifestError> {
        let _ = self.segments()?;
        read_rsc7_pointer(&self.body, pointer, length, self.page_sizes)
    }

    /// Read a pointer array and its target ranges from this decoded resource.
    pub fn read_pointer_array(
        &self,
        array_pointer: u64,
        count: u16,
        max_count: u16,
        target_length: u32,
    ) -> Result<Vec<Option<&[u8]>>, ManifestError> {
        let _ = self.segments()?;
        read_rsc7_pointer_array(
            &self.body, array_pointer, count, max_count, target_length, self.page_sizes,
        )
    }

    /// Borrow the decompressed body as separate [system, graphics] segments.
    /// The body length must exactly match the page allocation metadata.
    pub fn segments(&self) -> Result<(&[u8], &[u8]), ManifestError> {
        let system_len = usize::try_from(self.page_sizes.system_bytes)
            .map_err(|_| ManifestError("RSC7 system segment size does not fit this platform".into()))?;
        let graphics_len = usize::try_from(self.page_sizes.graphics_bytes)
            .map_err(|_| ManifestError("RSC7 graphics segment size does not fit this platform".into()))?;
        let expected = system_len.checked_add(graphics_len)
            .ok_or_else(|| ManifestError("RSC7 segment length overflow".into()))?;
        if self.body.len() != expected {
            return Err(ManifestError(format!(
                "RSC7 body length mismatch: page metadata requires {expected}, got {}",
                self.body.len()
            )));
        }
        let (system, graphics) = self.body.split_at(system_len);
        Ok((system, graphics))
    }
}

/// Which RSC7 segment a virtual pointer addresses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rsc7Segment { System, Graphics }

/// A validated byte range inside one decompressed RSC7 segment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rsc7ResolvedPointer {
    pub segment: Rsc7Segment,
    pub offset: u32,
    pub length: u32,
}

/// Resolve a RAGE virtual pointer (0x5xxxxxxx system, 0x6xxxxxxx graphics).
/// The pointer is a u64 in serialized structures but its address is encoded
/// in the low u32. Zero is a null pointer. The requested range must fit wholly
/// inside the selected decompressed segment.
pub fn resolve_rsc7_pointer(
    pointer: u64,
    length: u32,
    page_sizes: Rpf7ResourcePageSizes,
) -> Result<Option<Rsc7ResolvedPointer>, ManifestError> {
    if pointer == 0 { return Ok(None); }
    if pointer >> 32 != 0 {
        return Err(ManifestError(format!("RSC7 virtual pointer has non-zero upper bits: {pointer:#018x}")));
    }
    let address = pointer as u32;
    let segment = match address & 0xF000_0000 {
        0x5000_0000 => Rsc7Segment::System,
        0x6000_0000 => Rsc7Segment::Graphics,
        _ => return Err(ManifestError(format!("unsupported RSC7 virtual pointer {address:#010x}"))),
    };
    let offset = address & 0x0FFF_FFFF;
    let segment_size = match segment {
        Rsc7Segment::System => page_sizes.system_bytes,
        Rsc7Segment::Graphics => page_sizes.graphics_bytes,
    };
    let end = u64::from(offset).checked_add(u64::from(length))
        .ok_or_else(|| ManifestError("RSC7 pointer range overflow".into()))?;
    if end > segment_size {
        return Err(ManifestError(format!(
            "RSC7 pointer range {offset:#x}..{end:#x} exceeds {:?} segment size {segment_size:#x}",
            segment
        )));
    }
    Ok(Some(Rsc7ResolvedPointer { segment, offset, length }))
}

/// Read a resolved pointer range from the concatenated [system | graphics] body.
pub fn read_rsc7_pointer<'a>(
    body: &'a [u8],
    pointer: u64,
    length: u32,
    page_sizes: Rpf7ResourcePageSizes,
) -> Result<Option<&'a [u8]>, ManifestError> {
    let Some(resolved) = resolve_rsc7_pointer(pointer, length, page_sizes)? else { return Ok(None); };
    let segment_base = match resolved.segment {
        Rsc7Segment::System => 0_u64,
        Rsc7Segment::Graphics => page_sizes.system_bytes,
    };
    let start = segment_base.checked_add(u64::from(resolved.offset))
        .ok_or_else(|| ManifestError("RSC7 body offset overflow".into()))?;
    let end = start.checked_add(u64::from(resolved.length))
        .ok_or_else(|| ManifestError("RSC7 body range overflow".into()))?;
    let start = usize::try_from(start).map_err(|_| ManifestError("RSC7 body offset does not fit this platform".into()))?;
    let end = usize::try_from(end).map_err(|_| ManifestError("RSC7 body end does not fit this platform".into()))?;
    body.get(start..end).map(Some)
        .ok_or_else(|| ManifestError("RSC7 decompressed body is shorter than its page flags".into()))
}

/// Read an RSC7 virtual pointer directly from separately held page buffers.
/// This avoids requiring callers to concatenate system and graphics pages.
pub fn read_rsc7_pointer_from_pages<'a>(
    system: &'a [u8],
    graphics: &'a [u8],
    pointer: u64,
    length: u32,
    page_sizes: Rpf7ResourcePageSizes,
) -> Result<Option<&'a [u8]>, ManifestError> {
    if system.len() as u64 != page_sizes.system_bytes
        || graphics.len() as u64 != page_sizes.graphics_bytes
    {
        return Err(ManifestError("RSC7 page buffers do not match declared page sizes".into()));
    }
    let Some(resolved) = resolve_rsc7_pointer(pointer, length, page_sizes)? else {
        return Ok(None);
    };
    let pages = match resolved.segment {
        Rsc7Segment::System => system,
        Rsc7Segment::Graphics => graphics,
    };
    let start = usize::try_from(resolved.offset)
        .map_err(|_| ManifestError("RSC7 page offset does not fit this platform".into()))?;
    let end = start.checked_add(usize::try_from(resolved.length)
        .map_err(|_| ManifestError("RSC7 page range does not fit this platform".into()))?)
        .ok_or_else(|| ManifestError("RSC7 page range overflow".into()))?;
    pages.get(start..end).map(Some)
        .ok_or_else(|| ManifestError("RSC7 page buffer is shorter than declared bounds".into()))
}

/// Read a serialized pointer array from separate system/graphics page buffers.
/// Each non-null target is independently range-checked; no concatenated page image is built.
pub fn read_rsc7_pointer_array_from_pages<'a>(
    system: &'a [u8],
    graphics: &'a [u8],
    array_pointer: u64,
    count: u16,
    max_count: u16,
    target_length: u32,
    page_sizes: Rpf7ResourcePageSizes,
) -> Result<Vec<Option<&'a [u8]>>, ManifestError> {
    if count > max_count {
        return Err(ManifestError(format!(
            "RSC7 pointer-array count {} exceeds configured limit {}", count, max_count
        )));
    }
    if system.len() as u64 != page_sizes.system_bytes
        || graphics.len() as u64 != page_sizes.graphics_bytes
    {
        return Err(ManifestError("RSC7 page buffers do not match declared page sizes".into()));
    }
    if count == 0 { return Ok(Vec::new()); }
    let array_length = u32::from(count).checked_mul(4)
        .ok_or_else(|| ManifestError("RSC7 pointer-array byte length overflow".into()))?;
    let array = read_rsc7_pointer_from_pages(
        system, graphics, array_pointer, array_length, page_sizes
    )?.ok_or_else(|| ManifestError("RSC7 pointer array has a null address".into()))?;
    let mut result = Vec::with_capacity(usize::from(count));
    for (index, raw) in array.chunks_exact(4).enumerate() {
        let pointer = u64::from(u32::from_le_bytes(raw.try_into().unwrap()));
        result.push(read_rsc7_pointer_from_pages(
            system, graphics, pointer, target_length, page_sizes
        ).map_err(|error| ManifestError(format!("RSC7 pointer-array item {index}: {}", error.0)))?);
    }
    Ok(result)
}

/// Read a candidate 8-byte pointer-list descriptor and its bounded element
/// storage from separate RSC7 page buffers. Element layout remains caller-defined.
pub fn read_rsc7_pointer_list_from_pages<'a>(
    system: &'a [u8],
    graphics: &'a [u8],
    header_pointer: u64,
    element_size: u32,
    max_elements: u16,
    page_sizes: Rpf7ResourcePageSizes,
) -> Result<Option<&'a [u8]>, ManifestError> {
    if system.len() as u64 != page_sizes.system_bytes
        || graphics.len() as u64 != page_sizes.graphics_bytes
    {
        return Err(ManifestError("RSC7 page buffers do not match declared page sizes".into()));
    }
    let raw = read_rsc7_pointer_from_pages(
        system, graphics, header_pointer, 8, page_sizes
    )?.ok_or_else(|| ManifestError("RSC7 pointer-list header has a null address".into()))?;
    let header = parse_rsc7_pointer_list_header(raw)?;
    if header.count > max_elements {
        return Err(ManifestError(format!(
            "RSC7 pointer-list count {} exceeds configured limit {}",
            header.count, max_elements
        )));
    }
    if header.count == 0 { return Ok(None); }
    if header.pointer == 0 {
        return Err(ManifestError("RSC7 pointer-list has items but a null data pointer".into()));
    }
    if element_size == 0 {
        return Err(ManifestError("RSC7 pointer-list element size cannot be zero".into()));
    }
    let byte_length = u32::from(header.count).checked_mul(element_size)
        .ok_or_else(|| ManifestError("RSC7 pointer-list byte length overflow".into()))?;
    read_rsc7_pointer_from_pages(
        system, graphics, u64::from(header.pointer), byte_length, page_sizes
    )?.map(Some).ok_or_else(|| ManifestError("RSC7 pointer-list data pointer is null".into()))
}

#[cfg(test)]
mod rsc7_separate_page_list_tests {
    use super::*;

    #[test]
    fn reads_candidate_list_descriptor_and_elements_from_pages() {
        let sizes = Rpf7ResourcePageSizes { system_bytes: 32, graphics_bytes: 0 };
        let mut system = vec![0_u8; 32];
        system[0..4].copy_from_slice(&0x5000_0010_u32.to_le_bytes());
        system[4..6].copy_from_slice(&2_u16.to_le_bytes());
        system[6..8].copy_from_slice(&3_u16.to_le_bytes());
        system[16..24].copy_from_slice(b"AAAABBBB");
        assert_eq!(read_rsc7_pointer_list_from_pages(
            &system, &[], 0x5000_0000, 4, 4, sizes
        ).unwrap(), Some(&b"AAAABBBB"[..]));
    }

    #[test]
    fn rejects_invalid_candidate_list_descriptors_and_bounds() {
        let sizes = Rpf7ResourcePageSizes { system_bytes: 32, graphics_bytes: 0 };
        let mut system = vec![0_u8; 32];
        system[0..4].copy_from_slice(&0x5000_0010_u32.to_le_bytes());
        system[4..6].copy_from_slice(&2_u16.to_le_bytes());
        system[6..8].copy_from_slice(&1_u16.to_le_bytes());
        assert!(read_rsc7_pointer_list_from_pages(
            &system, &[], 0x5000_0000, 4, 4, sizes
        ).is_err());
        system[4..6].copy_from_slice(&1_u16.to_le_bytes());
        system[6..8].copy_from_slice(&1_u16.to_le_bytes());
        system[0..4].copy_from_slice(&0x5000_001F_u32.to_le_bytes());
        assert!(read_rsc7_pointer_list_from_pages(
            &system, &[], 0x5000_0000, 4, 4, sizes
        ).is_err());
        system[0..4].copy_from_slice(&0_u32.to_le_bytes());
        assert!(read_rsc7_pointer_list_from_pages(
            &system, &[], 0x5000_0000, 4, 4, sizes
        ).is_err());
    }
}

#[cfg(test)]
mod rsc7_separate_page_array_tests {
    use super::*;

    #[test]
    fn reads_array_and_targets_across_separate_pages() {
        let sizes = Rpf7ResourcePageSizes { system_bytes: 16, graphics_bytes: 8 };
        let mut system = vec![0_u8; 16];
        let graphics = b"GPU-DATA";
        system[0..4].copy_from_slice(&0x5000_000C_u32.to_le_bytes());
        system[4..8].copy_from_slice(&0x6000_0004_u32.to_le_bytes());
        system[12..16].copy_from_slice(b"SYS!");
        assert_eq!(
            read_rsc7_pointer_array_from_pages(
                &system, graphics, 0x5000_0000, 2, 4, 4, sizes
            ).unwrap(),
            vec![Some(&b"SYS!"[..]), Some(&b"DATA"[..])]
        );
    }

    #[test]
    fn rejects_null_array_oversized_count_and_invalid_target() {
        let sizes = Rpf7ResourcePageSizes { system_bytes: 16, graphics_bytes: 0 };
        let system = vec![0_u8; 16];
        assert!(read_rsc7_pointer_array_from_pages(
            &system, &[], 0, 1, 4, 4, sizes
        ).is_err());
        assert!(read_rsc7_pointer_array_from_pages(
            &system, &[], 0x5000_0000, 2, 1, 4, sizes
        ).is_err());
        let mut invalid = system.clone();
        invalid[0..4].copy_from_slice(&0x5000_0010_u32.to_le_bytes());
        assert!(read_rsc7_pointer_array_from_pages(
            &invalid, &[], 0x5000_0000, 1, 1, 4, sizes
        ).is_err());
        assert!(read_rsc7_pointer_array_from_pages(
            &system[..8], &[], 0x5000_0000, 1, 1, 4, sizes
        ).is_err());
    }
}

#[cfg(test)]
mod rsc7_separate_page_reader_tests {
    use super::*;

    #[test]
    fn reads_system_and_graphics_pointers_without_concatenation() {
        let system = b"SYS-DATA";
        let graphics = b"GPU-DATA";
        let sizes = Rpf7ResourcePageSizes { system_bytes: 8, graphics_bytes: 8 };
        assert_eq!(read_rsc7_pointer_from_pages(system, graphics, 0x5000_0004, 4, sizes).unwrap(), Some(&b"DATA"[..]));
        assert_eq!(read_rsc7_pointer_from_pages(system, graphics, 0x6000_0004, 4, sizes).unwrap(), Some(&b"DATA"[..]));
        assert_eq!(read_rsc7_pointer_from_pages(system, graphics, 0, 4, sizes).unwrap(), None);
    }

    #[test]
    fn rejects_mismatched_page_buffers_and_out_of_range_pointers() {
        let system = b"SYSTEM!!";
        let graphics = b"GPU-DATA";
        let sizes = Rpf7ResourcePageSizes { system_bytes: 8, graphics_bytes: 8 };
        assert!(read_rsc7_pointer_from_pages(system, graphics, 0x5000_0007, 2, sizes).is_err());
        assert!(read_rsc7_pointer_from_pages(system, b"short", 0x6000_0000, 1, sizes).is_err());
    }
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
    pub resource_page_sizes: Option<Rpf7ResourcePageSizes>,
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
        let (file_size, offset_blocks, size_is_sentinel, resource_page_sizes) = match &entry.kind {
            Rpf7EntryKind::BinaryFile { file_size, file_offset_blocks, uncompressed_size, .. } =>
                (if *file_size == 0 { *uncompressed_size as u64 } else { *file_size as u64 },
                 *file_offset_blocks as u64, false, None),
            Rpf7EntryKind::ResourceFile { file_size, file_offset_blocks, system_flags, graphics_flags } =>
                (*file_size as u64, *file_offset_blocks as u64, *file_size == 0x00FF_FFFF,
                 rpf7_resource_page_sizes(*system_flags, *graphics_flags).ok()),
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
            resource_page_sizes,
        })
    }).collect()
}


/// A pair of valid payload ranges that overlap in the same archive.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rpf7PayloadOverlap {
    pub first_entry_index: u32,
    pub first_name: String,
    pub second_entry_index: u32,
    pub second_name: String,
    pub overlap_start: u64,
    pub overlap_end: u64,
}

/// Report overlapping valid payload ranges without extracting or modifying data.
/// Invalid, sentinel, metadata-overlapping and out-of-bounds ranges are ignored.
/// max_reports bounds memory use when inspecting adversarial archives.
pub fn find_rpf7_payload_overlaps(ranges: &[Rpf7PayloadRange], max_reports: usize) -> Vec<Rpf7PayloadOverlap> {
    let mut valid: Vec<&Rpf7PayloadRange> = ranges.iter()
        .filter(|r| r.in_bounds && !r.overlaps_metadata && !r.offset_is_sentinel
            && r.resolved_size.is_some() && r.start < r.end).collect();
    valid.sort_by_key(|r| (r.start, r.end, r.entry_index));
    let mut found = Vec::new();
    for (i, first) in valid.iter().enumerate() {
        for second in valid.iter().skip(i + 1) {
            if second.start >= first.end { break; }
            if first.entry_index == second.entry_index { continue; }
            found.push(Rpf7PayloadOverlap {
                first_entry_index: first.entry_index, first_name: first.name.clone(),
                second_entry_index: second.entry_index, second_name: second.name.clone(),
                overlap_start: first.start.max(second.start),
                overlap_end: first.end.min(second.end),
            });
            if found.len() >= max_reports { return found; }
        }
    }
    found
}

/// Copy one unencrypted, uncompressed RPF7 binary entry into a bounded buffer.
/// Compressed, encrypted, resource, metadata-overlapping and out-of-range
/// payloads are rejected; this does not decompress or decode RSC7 resources.
/// Stream one unencrypted, uncompressed RPF7 binary entry to a caller-owned
/// writer in bounded chunks. No output-sized intermediate buffer is allocated.
pub fn copy_rpf7_open_binary_entry<W: Write>(
    archive: &[u8],
    inventory: &Rpf7Inventory,
    entry_index: u32,
    max_output_bytes: usize,
    writer: &mut W,
) -> Result<u64, ManifestError> {
    let entry = inventory.entries.iter().find(|entry| entry.index == entry_index)
        .ok_or_else(|| ManifestError(format!("RPF7 entry {entry_index} does not exist")))?;
    let (file_size, uncompressed_size, encrypted) = match &entry.kind {
        Rpf7EntryKind::BinaryFile { file_size, uncompressed_size, encrypted, .. } =>
            (*file_size, *uncompressed_size, *encrypted),
        Rpf7EntryKind::ResourceFile { .. } =>
            return Err(ManifestError("RPF7 resource payload extraction is not supported".into())),
        Rpf7EntryKind::Directory { .. } =>
            return Err(ManifestError("RPF7 directory has no payload".into())),
    };
    if encrypted {
        return Err(ManifestError("RPF7 binary payload is encrypted".into()));
    }
    if file_size != 0 && file_size != uncompressed_size {
        return Err(ManifestError("RPF7 binary payload is compressed; decompression is not supported".into()));
    }
    let range = validate_rpf7_payload_ranges(archive, inventory).into_iter()
        .find(|range| range.entry_index == entry_index)
        .ok_or_else(|| ManifestError("RPF7 binary payload range is unavailable".into()))?;
    if range.offset_is_sentinel || !range.in_bounds || range.overlaps_metadata {
        return Err(ManifestError("RPF7 binary payload range is invalid".into()));
    }
    let size = usize::try_from(range.resolved_size.unwrap_or(0))
        .map_err(|_| ManifestError("RPF7 binary payload size does not fit this platform".into()))?;
    if size > max_output_bytes {
        return Err(ManifestError(format!("RPF7 binary payload exceeds extraction limit ({size} > {max_output_bytes})")));
    }
    let start = usize::try_from(range.start)
        .map_err(|_| ManifestError("RPF7 binary payload offset does not fit this platform".into()))?;
    let end = usize::try_from(range.end)
        .map_err(|_| ManifestError("RPF7 binary payload end does not fit this platform".into()))?;
    let payload = archive.get(start..end)
        .ok_or_else(|| ManifestError("RPF7 binary payload slice is unavailable".into()))?;
    for chunk in payload.chunks(64 * 1024) {
        writer.write_all(chunk)
            .map_err(|error| ManifestError(format!("writing RPF7 payload failed: {error}")))?;
    }
    Ok(size as u64)
}

/// Convenience wrapper for small payloads. Prefer the streaming copy API for
/// larger entries to avoid allocating an output-sized intermediate buffer.
pub fn extract_rpf7_open_binary_entry(
    archive: &[u8],
    inventory: &Rpf7Inventory,
    entry_index: u32,
    max_output_bytes: usize,
) -> Result<Vec<u8>, ManifestError> {
    let mut output = Vec::new();
    copy_rpf7_open_binary_entry(archive, inventory, entry_index, max_output_bytes, &mut output)?;
    Ok(output)
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

/// Recursively inspect nested, unencrypted, stored RPF7 archives.
///
/// max_depth counts nested levels below the supplied root (1 preserves the
/// legacy one-level behavior). max_archives caps total child archives reported.
/// Payloads are borrowed as bounded slices; no archive contents are extracted.
pub fn scan_nested_rpf7_open_recursive(
    archive: &[u8],
    inventory: &Rpf7Inventory,
    max_depth: usize,
    max_archives: usize,
) -> Vec<NestedRpfReport> {
    fn walk(
        archive: &[u8],
        inventory: &Rpf7Inventory,
        depth: usize,
        max_depth: usize,
        remaining: &mut usize,
        prefix: &str,
        output: &mut Vec<NestedRpfReport>,
    ) {
        if depth >= max_depth || *remaining == 0 { return; }
        let reports = scan_nested_rpf7_open(archive, inventory);
        for mut report in reports {
            if *remaining == 0 { break; }
            *remaining -= 1;
            let local_name = report.name.clone();
            report.name = if prefix.is_empty() {
                local_name.clone()
            } else {
                format!("{prefix}/{local_name}")
            };
            let nested_inventory = match &report.status {
                NestedRpfStatus::InventoriedRpf7(value) => Some(value.clone()),
                _ => None,
            };
            output.push(report.clone());

            let Some(nested_inventory) = nested_inventory else { continue; };
            if depth + 1 >= max_depth || *remaining == 0 { continue; }
            let Some(entry) = inventory.entries.iter().find(|entry| entry.index == report.parent_entry_index) else { continue; };
            let (size, offset_blocks) = match &entry.kind {
                Rpf7EntryKind::BinaryFile { file_size, file_offset_blocks, uncompressed_size, encrypted }
                    if !*encrypted && file_size == uncompressed_size =>
                    (*file_size as usize, *file_offset_blocks as usize),
                _ => continue,
            };
            let Some(start) = offset_blocks.checked_mul(512) else { continue; };
            let Some(end) = start.checked_add(size) else { continue; };
            let metadata_end = 16_usize
                .checked_add(inventory.header.toc_bytes as usize)
                .and_then(|value| value.checked_add(inventory.header.names_length as usize));
            if metadata_end.map_or(true, |boundary| start < boundary) || end > archive.len() { continue; }
            let child = &archive[start..end];
            walk(child, &nested_inventory, depth + 1, max_depth, remaining,
                &report.name, output);
        }
    }

    let mut output = Vec::new();
    let mut remaining = max_archives;
    walk(archive, inventory, 0, max_depth, &mut remaining, "", &mut output);
    output
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
    fn probes_rsc7_header_and_rejects_truncated_or_wrong_magic() {
        let mut payload = [0_u8; 16];
        payload[0..4].copy_from_slice(b"RSC7");
        payload[4..8].copy_from_slice(&0x1234_5678_u32.to_le_bytes());
        let header = probe_rsc7_resource_header(&payload).unwrap();
        assert_eq!(header.magic, *b"RSC7");
        assert_eq!(header.opaque_word, 0x1234_5678);
        assert_eq!(header.header_bytes, 16);
        assert!(probe_rsc7_resource_header(b"RSC7").is_err());
        assert!(probe_rsc7_resource_header(b"NOPE............").is_err());
    }

    #[test]
    fn inspects_rpf7_resource_headers_only_inside_validated_ranges() {
        let mut archive = vec![0_u8; 528];
        archive[512..516].copy_from_slice(b"RSC7");
        archive[516..520].copy_from_slice(&0xAABB_CCDD_u32.to_le_bytes());
        let range = Rpf7PayloadRange {
            entry_index: 7, name: "sample.rsc".into(), start: 512, end: 528,
            archive_len: 528, metadata_end: 64, in_bounds: true,
            overlaps_metadata: false, size_is_sentinel: false,
            resolved_size: Some(16), offset_is_sentinel: false,
            resource_page_sizes: Some(Rpf7ResourcePageSizes { system_bytes: 512, graphics_bytes: 512 }),
        };
        let report = inspect_rpf7_rsc7_headers(&archive, &[range.clone()]);
        assert_eq!(report[0].status, Rsc7ResourceStatus::Valid);
        assert_eq!(report[0].header.as_ref().unwrap().opaque_word, 0xAABB_CCDD);
        archive[512..516].copy_from_slice(b"NOPE");
        assert_eq!(inspect_rpf7_rsc7_headers(&archive, &[range.clone()])[0].status, Rsc7ResourceStatus::WrongMagic);
        let mut truncated = range.clone();
        truncated.resolved_size = Some(8);
        assert_eq!(inspect_rpf7_rsc7_headers(&archive, &[truncated])[0].status, Rsc7ResourceStatus::HeaderTruncated);
        let mut invalid = range;
        invalid.in_bounds = false;
        assert_eq!(inspect_rpf7_rsc7_headers(&archive, &[invalid])[0].status, Rsc7ResourceStatus::InvalidRange);
    }

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
    fn resolves_system_and_graphics_virtual_pointers() {
        let sizes = Rpf7ResourcePageSizes { system_bytes: 1024, graphics_bytes: 512 };
        assert_eq!(resolve_rsc7_pointer(0, 4, sizes).unwrap(), None);
        let system = resolve_rsc7_pointer(0x5000_0020, 8, sizes).unwrap().unwrap();
        assert_eq!(system, Rsc7ResolvedPointer { segment: Rsc7Segment::System, offset: 0x20, length: 8 });
        let graphics = resolve_rsc7_pointer(0x6000_0010, 16, sizes).unwrap().unwrap();
        assert_eq!(graphics, Rsc7ResolvedPointer { segment: Rsc7Segment::Graphics, offset: 0x10, length: 16 });
        assert!(resolve_rsc7_pointer(0x5000_0400, 1, sizes).is_err());
        assert!(resolve_rsc7_pointer(0x7000_0000, 1, sizes).is_err());
        assert!(resolve_rsc7_pointer(0x1_5000_0000, 1, sizes).unwrap_err().0.contains("upper bits"));
    }

    #[test]
    fn reads_pointer_from_concatenated_rsc7_body() {
        let sizes = Rpf7ResourcePageSizes { system_bytes: 4, graphics_bytes: 4 };
        let body = b"SYS!GPU!";
        assert_eq!(read_rsc7_pointer(body, 0x5000_0001, 2, sizes).unwrap(), Some(&b"YS"[..]));
        assert_eq!(read_rsc7_pointer(body, 0x6000_0001, 3, sizes).unwrap(), Some(&b"PU!"[..]));
        assert_eq!(read_rsc7_pointer(body, 0, 3, sizes).unwrap(), None);
        assert!(read_rsc7_pointer(body, 0x6000_0003, 2, sizes).is_err());
    }

    #[test]
    fn decodes_bounded_rsc7_raw_deflate_body() {
        use flate2::write::DeflateEncoder;
        use flate2::Compression;
        let original = vec![0xA5; 512];
        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&original).unwrap();
        let compressed = encoder.finish().unwrap();
        let mut resource = Vec::from(*b"RSC7");
        resource.extend_from_slice(&13_u32.to_le_bytes());
        resource.extend_from_slice(&(1_u32 << 27).to_le_bytes());
        resource.extend_from_slice(&0_u32.to_le_bytes());
        resource.extend_from_slice(&compressed);
        let decoded = decode_rsc7_resource(&resource, 512).unwrap();
        assert_eq!(decoded.version, 13);
        assert_eq!(decoded.page_sizes.system_bytes, 512);
        assert_eq!(decoded.page_sizes.graphics_bytes, 0);
        assert_eq!(decoded.body, original);
        assert!(decode_rsc7_resource(&resource, 511).unwrap_err().0.contains("exceeds configured limit"));
    }

    #[test]
    fn rejects_rsc7_decompression_size_mismatch_and_bad_magic() {
        let mut short = Vec::from(*b"RSC7");
        short.extend_from_slice(&13_u32.to_le_bytes());
        short.extend_from_slice(&(1_u32 << 27).to_le_bytes());
        short.extend_from_slice(&0_u32.to_le_bytes());
        short.extend_from_slice(&[3, 0]);
        assert!(decode_rsc7_resource(&short, 512).is_err());
        short[0] = b'X';
        assert!(decode_rsc7_resource(&short, 512).unwrap_err().0.contains("magic"));
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
    fn reports_overlapping_payload_ranges_and_respects_report_cap() {
        let mut a = Rpf7PayloadRange {
            entry_index: 1, name: "a.bin".into(), start: 512, end: 600,
            archive_len: 2048, metadata_end: 64, in_bounds: true,
            overlaps_metadata: false, size_is_sentinel: false,
            resolved_size: Some(88), offset_is_sentinel: false,
            resource_page_sizes: None,
        };
        let mut b = a.clone();
        b.entry_index = 2; b.name = "b.bin".into(); b.start = 550; b.end = 620;
        b.resolved_size = Some(70);
        let mut c = a.clone();
        c.entry_index = 3; c.start = 600; c.end = 640; c.resolved_size = Some(40);
        let found = find_rpf7_payload_overlaps(&[a.clone(), b, c], 10);
        assert_eq!(found.len(), 2);
        assert_eq!((found[0].overlap_start, found[0].overlap_end), (550, 600));
        assert_eq!((found[1].overlap_start, found[1].overlap_end), (600, 620));
        a.in_bounds = false;
        assert!(find_rpf7_payload_overlaps(&[a], 10).is_empty());
        assert!(find_rpf7_payload_overlaps(&[], 0).is_empty());
    }

    #[test]
    fn streams_bounded_binary_entry_to_writer() {
        let mut data = minimal_open_rpf7();
        data[512..516].copy_from_slice(b"DATA");
        let inventory = inventory_rpf7_open(&data).unwrap();
        let mut output = Vec::new();
        let copied = copy_rpf7_open_binary_entry(&data, &inventory, 1, 4, &mut output).unwrap();
        assert_eq!(copied, 4);
        assert_eq!(output, b"DATA");
    }

    #[test]
    fn extracts_bounded_unencrypted_open_binary_entry() {
        let mut data = minimal_open_rpf7();
        data[512..516].copy_from_slice(b"DATA");
        let inventory = inventory_rpf7_open(&data).unwrap();
        assert_eq!(extract_rpf7_open_binary_entry(&data, &inventory, 1, 4).unwrap(), b"DATA");
        assert!(extract_rpf7_open_binary_entry(&data, &inventory, 1, 3).is_err());
    }

    #[test]
    fn refuses_encrypted_compressed_and_resource_payload_extraction() {
        let mut encrypted = minimal_open_rpf7();
        encrypted[44..48].copy_from_slice(&1_u32.to_le_bytes());
        let inventory = inventory_rpf7_open(&encrypted).unwrap();
        assert!(extract_rpf7_open_binary_entry(&encrypted, &inventory, 1, 64).is_err());

        let mut compressed = minimal_open_rpf7();
        compressed[40..44].copy_from_slice(&8_u32.to_le_bytes());
        let inventory = inventory_rpf7_open(&compressed).unwrap();
        assert!(extract_rpf7_open_binary_entry(&compressed, &inventory, 1, 64).is_err());

        let mut resource = minimal_open_rpf7();
        resource[34..37].copy_from_slice(&[4, 0, 0]);
        resource[39] |= 0x80;
        let inventory = inventory_rpf7_open(&resource).unwrap();
        assert!(extract_rpf7_open_binary_entry(&resource, &inventory, 1, 64).is_err());
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
        assert_eq!(sizes.system_bytes, 1024);
        assert_eq!(sizes.graphics_bytes, 4096);
        assert_eq!(rpf7_resource_page_sizes(0, 0).unwrap(), Rpf7ResourcePageSizes {
            system_bytes: 0, graphics_bytes: 0,
        });
    }

    #[test]
    fn attaches_decoded_page_sizes_to_resource_payload_range() {
        let mut data = minimal_open_rpf7();
        data[34..37].copy_from_slice(&[4, 0, 0]);
        data[39] |= 0x80;
        data[40..44].copy_from_slice(&1_u32.to_le_bytes());
        data[44..48].copy_from_slice(&2_u32.to_le_bytes());
        let inventory = inventory_rpf7_open(&data).unwrap();
        let ranges = validate_rpf7_payload_ranges(&data, &inventory);
        assert_eq!(ranges[0].resource_page_sizes, Some(rpf7_resource_page_sizes(1, 2).unwrap()));
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


    fn wrap_in_open_rpf7(child: &[u8], child_name: &str) -> Vec<u8> {
        let name = format!("root\0{child_name}\0");
        let names_len = name.len() as u32;
        let payload_offset = 1024_usize;
        let mut archive = vec![0_u8; payload_offset + child.len()];
        archive[0..4].copy_from_slice(&RPF7_MAGIC.to_le_bytes());
        archive[4..8].copy_from_slice(&2_u32.to_le_bytes());
        archive[8..12].copy_from_slice(&names_len.to_le_bytes());
        archive[12..16].copy_from_slice(&RPF_OPEN.to_le_bytes());
        archive[16..20].copy_from_slice(&0_u32.to_le_bytes());
        archive[20..24].copy_from_slice(&0x7FFF_FF00_u32.to_le_bytes());
        archive[24..28].copy_from_slice(&1_u32.to_le_bytes());
        archive[28..32].copy_from_slice(&1_u32.to_le_bytes());
        archive[32..34].copy_from_slice(&5_u16.to_le_bytes());
        let len = child.len() as u32;
        archive[34] = len as u8;
        archive[35] = (len >> 8) as u8;
        archive[36] = (len >> 16) as u8;
        archive[37] = 2;
        archive[40..44].copy_from_slice(&len.to_le_bytes());
        archive[44..48].copy_from_slice(&0_u32.to_le_bytes());
        archive[48..48 + name.len()].copy_from_slice(name.as_bytes());
        archive[payload_offset..].copy_from_slice(child);
        archive
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
    fn recursive_nested_scan_respects_depth_and_archive_limits() {
        let outer = outer_with_nested_open_rpf7();
        let inventory = inventory_rpf7_open(&outer).unwrap();
        assert!(scan_nested_rpf7_open_recursive(&outer, &inventory, 0, 8).is_empty());
        let limited = scan_nested_rpf7_open_recursive(&outer, &inventory, 8, 1);
        assert_eq!(limited.len(), 1);
        assert_eq!(limited[0].name, "nested.rpf");
    }

    #[test]
    fn recursively_inventories_three_archive_levels_and_reports_full_paths() {
        let leaf = minimal_open_rpf7();
        let middle = wrap_in_open_rpf7(&leaf, "leaf.rpf");
        let outer = wrap_in_open_rpf7(&middle, "middle.rpf");
        let inventory = inventory_rpf7_open(&outer).unwrap();

        let nested = scan_nested_rpf7_open_recursive(&outer, &inventory, 2, 8);
        assert_eq!(nested.len(), 2);
        assert_eq!(nested[0].name, "middle.rpf");
        assert_eq!(nested[1].name, "middle.rpf/leaf.rpf");
        assert!(matches!(nested[0].status, NestedRpfStatus::InventoriedRpf7(_)));
        match &nested[1].status {
            NestedRpfStatus::InventoriedRpf7(inner) => assert_eq!(inner.entries.len(), 2),
            other => panic!("expected third-level RPF7 inventory, got {other:?}"),
        }

        let one_level = scan_nested_rpf7_open_recursive(&outer, &inventory, 1, 8);
        assert_eq!(one_level.len(), 1);
        assert_eq!(one_level[0].name, "middle.rpf");
    }

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


/// Candidate compact RAGE pointer-list descriptor (32-bit virtual pointer,
/// 16-bit item count, 16-bit capacity). This is a generic bounded primitive,
/// not a claim that every GTA V collection uses this layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rsc7PointerListHeader {
    pub pointer: u32,
    pub count: u16,
    pub capacity: u16,
}

/// Parse an 8-byte pointer-list descriptor without dereferencing its contents.
pub fn parse_rsc7_pointer_list_header(bytes: &[u8]) -> Result<Rsc7PointerListHeader, ManifestError> {
    if bytes.len() < 8 {
        return Err(ManifestError("RSC7 pointer-list header is truncated (need 8 bytes)".into()));
    }
    let header = Rsc7PointerListHeader {
        pointer: u32::from_le_bytes(bytes[0..4].try_into().unwrap()),
        count: u16::from_le_bytes(bytes[4..6].try_into().unwrap()),
        capacity: u16::from_le_bytes(bytes[6..8].try_into().unwrap()),
    };
    if header.count > header.capacity {
        return Err(ManifestError(format!(
            "RSC7 pointer-list count {} exceeds capacity {}",
            header.count, header.capacity
        )));
    }
    Ok(header)
}

/// Resolve a candidate pointer-list's contiguous element bytes with explicit
/// element-size and count caps. Returns None for an empty/null list.
pub fn read_rsc7_pointer_list<'a>(
    body: &'a [u8],
    header: Rsc7PointerListHeader,
    element_size: u32,
    max_elements: u16,
    page_sizes: Rpf7ResourcePageSizes,
) -> Result<Option<&'a [u8]>, ManifestError> {
    if header.count > max_elements {
        return Err(ManifestError(format!(
            "RSC7 pointer-list count {} exceeds configured limit {}",
            header.count, max_elements
        )));
    }
    if header.count == 0 {
        return Ok(None);
    }
    if header.pointer == 0 {
        return Err(ManifestError("RSC7 pointer-list has items but a null data pointer".into()));
    }
    if element_size == 0 {
        return Err(ManifestError("RSC7 pointer-list element size cannot be zero".into()));
    }
    let byte_length = u32::from(header.count).checked_mul(element_size)
        .ok_or_else(|| ManifestError("RSC7 pointer-list byte length overflow".into()))?;
    read_rsc7_pointer(body, u64::from(header.pointer), byte_length, page_sizes)
}



/// Decode a bounded array of serialized 32-bit RSC7 virtual pointers.
/// Each non-null pointer is range-checked for target_length bytes against
/// the resource's system/graphics page allocations.
pub fn decode_rsc7_pointer_array(
    bytes: &[u8],
    count: u16,
    max_count: u16,
    target_length: u32,
    page_sizes: Rpf7ResourcePageSizes,
) -> Result<Vec<Option<Rsc7ResolvedPointer>>, ManifestError> {
    if count > max_count {
        return Err(ManifestError(format!(
            "RSC7 pointer-array count {} exceeds configured limit {}",
            count, max_count
        )));
    }
    let byte_length = usize::from(count).checked_mul(4)
        .ok_or_else(|| ManifestError("RSC7 pointer-array byte length overflow".into()))?;
    if bytes.len() < byte_length {
        return Err(ManifestError(format!(
            "RSC7 pointer-array data is truncated: need {} bytes, got {}",
            byte_length, bytes.len()
        )));
    }
    let mut resolved = Vec::with_capacity(usize::from(count));
    for (index, chunk) in bytes[..byte_length].chunks_exact(4).enumerate() {
        let pointer = u32::from_le_bytes(chunk.try_into().unwrap());
        let item = resolve_rsc7_pointer(u64::from(pointer), target_length, page_sizes)
            .map_err(|e| ManifestError(format!("RSC7 pointer-array item {index}: {}", e.0)))?;
        resolved.push(item);
    }
    Ok(resolved)
}

/// Read a serialized pointer array from a resource and resolve each target range.
/// This composes bounded array access with per-pointer segment validation; it does
/// not infer the target structure's semantic layout.
pub fn read_rsc7_pointer_array<'a>(
    body: &'a [u8],
    array_pointer: u64,
    count: u16,
    max_count: u16,
    target_length: u32,
    page_sizes: Rpf7ResourcePageSizes,
) -> Result<Vec<Option<&'a [u8]>>, ManifestError> {
    if count > max_count {
        return Err(ManifestError(format!(
            "RSC7 pointer-array count {} exceeds configured limit {}",
            count, max_count
        )));
    }
    if count == 0 { return Ok(Vec::new()); }
    let array_bytes = u32::from(count).checked_mul(4)
        .ok_or_else(|| ManifestError("RSC7 pointer-array byte length overflow".into()))?;
    let bytes = read_rsc7_pointer(body, array_pointer, array_bytes, page_sizes)?
        .ok_or_else(|| ManifestError("RSC7 pointer array has a null address".into()))?;
    let resolved = decode_rsc7_pointer_array(bytes, count, max_count, target_length, page_sizes)?;
    resolved.into_iter().map(|item| {
        let Some(item) = item else { return Ok(None); };
        let pointer = match item.segment {
            Rsc7Segment::System => 0x5000_0000_u64 | u64::from(item.offset),
            Rsc7Segment::Graphics => 0x6000_0000_u64 | u64::from(item.offset),
        };
        read_rsc7_pointer(body, pointer, item.length, page_sizes)
    }).collect()
}

/// Select one fixed-size record from bytes returned by
/// read_rsc7_pointer_list. The caller supplies the validated element count
/// and size because GTA resource types are not inferred by this generic layer.
pub fn rsc7_pointer_list_element<'a>(
    bytes: &'a [u8],
    index: u16,
    count: u16,
    element_size: u32,
) -> Result<&'a [u8], ManifestError> {
    if index >= count {
        return Err(ManifestError(format!(
            "RSC7 list index {} is outside count {}",
            index, count
        )));
    }
    if element_size == 0 {
        return Err(ManifestError("RSC7 list element size cannot be zero".into()));
    }
    let start = u32::from(index).checked_mul(element_size)
        .ok_or_else(|| ManifestError("RSC7 list element offset overflow".into()))?;
    let end = start.checked_add(element_size)
        .ok_or_else(|| ManifestError("RSC7 list element end overflow".into()))?;
    let start = usize::try_from(start)
        .map_err(|_| ManifestError("RSC7 list element offset does not fit this platform".into()))?;
    let end = usize::try_from(end)
        .map_err(|_| ManifestError("RSC7 list element end does not fit this platform".into()))?;
    bytes.get(start..end)
        .ok_or_else(|| ManifestError("RSC7 list bytes are truncated for requested element".into()))
}

#[cfg(test)]
mod rsc7_pointer_list_tests {
    use super::*;

    #[test]
    fn parses_and_reads_bounded_candidate_pointer_list() {
        let mut raw = Vec::new();
        raw.extend_from_slice(&0x5000_0004_u32.to_le_bytes());
        raw.extend_from_slice(&2_u16.to_le_bytes());
        raw.extend_from_slice(&3_u16.to_le_bytes());
        let header = parse_rsc7_pointer_list_header(&raw).unwrap();
        let sizes = Rpf7ResourcePageSizes { system_bytes: 16, graphics_bytes: 0 };
        assert_eq!(read_rsc7_pointer_list(b"HEADABCDWXYZ1234", header, 4, 4, sizes).unwrap(), Some(&b"ABCDWXYZ"[..]));
    }

    #[test]
    fn decodes_bounded_system_graphics_and_null_pointer_arrays() {
        let mut raw = Vec::new();
        raw.extend_from_slice(&0x5000_0004_u32.to_le_bytes());
        raw.extend_from_slice(&0x6000_0008_u32.to_le_bytes());
        raw.extend_from_slice(&0_u32.to_le_bytes());
        let sizes = Rpf7ResourcePageSizes { system_bytes: 32, graphics_bytes: 32 };
        let result = decode_rsc7_pointer_array(&raw, 3, 4, 4, sizes).unwrap();
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].unwrap().segment, Rsc7Segment::System);
        assert_eq!(result[1].unwrap().segment, Rsc7Segment::Graphics);
        assert_eq!(result[2], None);
        assert!(decode_rsc7_pointer_array(&raw, 3, 2, 4, sizes).is_err());
        assert!(decode_rsc7_pointer_array(&raw[..7], 2, 4, 4, sizes).is_err());
        let invalid = 0x5000_0020_u32.to_le_bytes();
        assert!(decode_rsc7_pointer_array(&invalid, 1, 4, 4, sizes).is_err());
    }

    #[test]
    fn selects_only_in_bounds_fixed_size_list_elements() {
        let bytes = b"AAAABBBBCCCC";
        assert_eq!(rsc7_pointer_list_element(bytes, 1, 3, 4).unwrap(), b"BBBB");
        assert!(rsc7_pointer_list_element(bytes, 3, 3, 4).is_err());
        assert!(rsc7_pointer_list_element(b"short", 1, 2, 4).is_err());
        assert!(rsc7_pointer_list_element(bytes, 0, 3, 0).is_err());
    }

    #[test]
    fn rejects_malformed_or_oversized_candidate_pointer_lists() {
        assert!(parse_rsc7_pointer_list_header(&[0; 7]).is_err());
        let mut raw = Vec::new();
        raw.extend_from_slice(&0x5000_0000_u32.to_le_bytes());
        raw.extend_from_slice(&4_u16.to_le_bytes());
        raw.extend_from_slice(&3_u16.to_le_bytes());
        assert!(parse_rsc7_pointer_list_header(&raw).unwrap_err().0.contains("exceeds capacity"));
        let header = Rsc7PointerListHeader { pointer: 0x5000_0000, count: 5, capacity: 5 };
        assert!(read_rsc7_pointer_list(&[0; 64],
            Rsc7PointerListHeader { pointer: 0, count: 1, capacity: 1 }, 4, 4,
            Rpf7ResourcePageSizes { system_bytes: 64, graphics_bytes: 0 })
            .unwrap_err().0.contains("null data pointer"));
        assert!(read_rsc7_pointer_list(&[0; 64], header, 4, 4,
            Rpf7ResourcePageSizes { system_bytes: 64, graphics_bytes: 0 })
            .unwrap_err().0.contains("configured limit"));
        assert!(read_rsc7_pointer_list(&[0; 64],
            Rsc7PointerListHeader { pointer: 0x5000_0000, count: 1, capacity: 1 }, 0, 4,
            Rpf7ResourcePageSizes { system_bytes: 64, graphics_bytes: 0 }).is_err());
    }
}


#[cfg(test)]
mod rsc7_pointer_array_reader_tests {
    use super::*;

    #[test]
    fn reads_array_and_resolves_each_target_payload() {
        let sizes = Rpf7ResourcePageSizes { system_bytes: 32, graphics_bytes: 16 };
        let mut body = vec![0_u8; 48];
        body[0..4].copy_from_slice(&0x5000_0010_u32.to_le_bytes());
        body[4..8].copy_from_slice(&0x6000_0004_u32.to_le_bytes());
        body[16..20].copy_from_slice(b"SYS!");
        body[36..40].copy_from_slice(b"GPU!");
        let result = read_rsc7_pointer_array(&body, 0x5000_0000, 2, 4, 4, sizes).unwrap();
        assert_eq!(result, vec![Some(&b"SYS!"[..]), Some(&b"GPU!"[..])]);
    }

    #[test]
    fn handles_empty_and_rejects_null_or_oversized_array() {
        let sizes = Rpf7ResourcePageSizes { system_bytes: 16, graphics_bytes: 0 };
        assert!(read_rsc7_pointer_array(&[0; 16], 0, 0, 4, 4, sizes).unwrap().is_empty());
        assert!(read_rsc7_pointer_array(&[0; 16], 0, 1, 4, 4, sizes).is_err());
        assert!(read_rsc7_pointer_array(&[0; 16], 0x5000_0000, 2, 1, 4, sizes).is_err());
    }

    #[test]
    fn preserves_null_array_items_and_rejects_short_target_body() {
        let sizes = Rpf7ResourcePageSizes { system_bytes: 16, graphics_bytes: 0 };
        let mut body = vec![0_u8; 16];
        body[0..4].copy_from_slice(&0_u32.to_le_bytes());
        body[4..8].copy_from_slice(&0x5000_000C_u32.to_le_bytes());
        body[12..16].copy_from_slice(b"DATA");
        let items = read_rsc7_pointer_array(&body, 0x5000_0000, 2, 2, 4, sizes).unwrap();
        assert_eq!(items, vec![None, Some(&b"DATA"[..])]);
        assert!(read_rsc7_pointer_array(&body[..14], 0x5000_0000, 2, 2, 4, sizes).is_err());
    }
}

#[cfg(test)]
mod rpf7_rsc7_extraction_tests {
    use super::*;
    use flate2::{write::DeflateEncoder, Compression};
    use std::io::Write;

    fn fixture() -> (Vec<u8>, Rpf7PayloadRange) {
        let body = vec![0x5A; 512];
        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&body).unwrap();
        let compressed = encoder.finish().unwrap();
        let mut archive = b"RSC7".to_vec();
        archive.extend_from_slice(&1_u32.to_le_bytes());
        archive.extend_from_slice(&(1_u32 << 27).to_le_bytes());
        archive.extend_from_slice(&0_u32.to_le_bytes());
        archive.extend_from_slice(&compressed);
        let size = archive.len() as u64;
        let range = Rpf7PayloadRange {
            entry_index: 7, name: "fixture.ydd".into(), start: 0, end: size,
            archive_len: size, metadata_end: 0, in_bounds: true,
            overlaps_metadata: false, size_is_sentinel: false,
            resolved_size: Some(size), offset_is_sentinel: false,
            resource_page_sizes: Some(Rpf7ResourcePageSizes { system_bytes: 512, graphics_bytes: 0 }),
        };
        (archive, range)
    }

    #[test]
    fn extracts_one_bounded_rsc7_resource_from_archive_range() {
        let (archive, range) = fixture();
        let report = extract_rpf7_rsc7_resource(&archive, &range, 1024);
        assert_eq!(report.status, Rsc7ExtractStatus::Decoded);
        assert_eq!(report.entry_index, 7);
        assert_eq!(report.resource.unwrap().body, vec![0x5A; 512]);
    }

    #[test]
    fn extraction_reports_invalid_ranges_and_output_caps() {
        let (archive, mut range) = fixture();
        assert_eq!(
            extract_rpf7_rsc7_resource(&archive, &range, 128).status,
            Rsc7ExtractStatus::OutputLimitExceeded
        );
        range.overlaps_metadata = true;
        assert_eq!(
            extract_rpf7_rsc7_resource(&archive, &range, 1024).status,
            Rsc7ExtractStatus::InvalidRange
        );
    }
}

#[cfg(test)]
mod rsc7_segment_view_tests {
    use super::*;

    #[test]
    fn exposes_system_and_graphics_segments_without_copying() {
        let decoded = DecodedRsc7Resource {
            version: 1,
            page_sizes: Rpf7ResourcePageSizes { system_bytes: 4, graphics_bytes: 3 },
            body: b"SYS!GPU".to_vec(),
        };
        let (system, graphics) = decoded.segments().unwrap();
        assert_eq!(system, b"SYS!");
        assert_eq!(graphics, b"GPU");
    }

    #[test]
    fn decoded_resource_reads_virtual_pointers_and_pointer_arrays() {
        let sizes = Rpf7ResourcePageSizes { system_bytes: 16, graphics_bytes: 0 };
        let mut body = vec![0_u8; 16];
        body[0..4].copy_from_slice(&0x5000_000C_u32.to_le_bytes());
        body[12..16].copy_from_slice(b"DATA");
        let decoded = DecodedRsc7Resource { version: 1, page_sizes: sizes, body };
        assert_eq!(decoded.read_pointer(0x5000_000C, 4).unwrap(), Some(&b"DATA"[..]));
        assert_eq!(decoded.read_pointer_array(0x5000_0000, 1, 2, 4).unwrap(), vec![Some(&b"DATA"[..])]);
    }

    #[test]
    fn decoded_resource_pointer_reads_reject_inconsistent_body() {
        let decoded = DecodedRsc7Resource {
            version: 1,
            page_sizes: Rpf7ResourcePageSizes { system_bytes: 16, graphics_bytes: 0 },
            body: vec![0; 8],
        };
        assert!(decoded.read_pointer(0x5000_0000, 4).unwrap_err().0.contains("body length mismatch"));
    }

    #[test]
    fn rejects_body_that_disagrees_with_page_metadata() {
        let decoded = DecodedRsc7Resource {
            version: 1,
            page_sizes: Rpf7ResourcePageSizes { system_bytes: 4, graphics_bytes: 4 },
            body: b"short".to_vec(),
        };
        assert!(decoded.segments().unwrap_err().0.contains("body length mismatch"));
    }
}


#[cfg(test)]
mod rsc7_streaming_decode_tests {
    use super::*;
    use flate2::{write::DeflateEncoder, Compression};
    use std::io::Cursor;

    fn fixture() -> Vec<u8> {
        let body = vec![0x3C; 512];
        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&body).unwrap();
        let compressed = encoder.finish().unwrap();
        let mut payload = b"RSC7".to_vec();
        payload.extend_from_slice(&1_u32.to_le_bytes());
        payload.extend_from_slice(&(1_u32 << 27).to_le_bytes());
        payload.extend_from_slice(&0_u32.to_le_bytes());
        payload.extend_from_slice(&compressed);
        payload
    }

    #[test]
    fn streams_decoded_resource_into_writer_with_bounded_memory() {
        let payload = fixture();
        let mut reader = Cursor::new(payload.clone());
        let mut output = Vec::new();
        let report = decode_rsc7_resource_stream(
            &mut reader, 0, payload.len() as u64, &mut output, 1024
        ).unwrap();
        assert_eq!(report.version, 1);
        assert_eq!(report.bytes_written, 512);
        assert_eq!(output, vec![0x3C; 512]);
    }

    #[test]
    fn resource_page_size_decoder_covers_each_bitfield_and_ignores_reserved_top_bits() {
        let cases = [
            (1_u32 << 27, 512_u64),
            (1_u32 << 26, 1024),
            (1_u32 << 25, 2048),
            (1_u32 << 24, 4096),
            (1_u32 << 17, 8192),
            (1_u32 << 11, 16384),
            (1_u32 << 7, 32768),
            (1_u32 << 5, 65536),
            (1_u32 << 4, 131072),
        ];
        for (flags, expected) in cases {
            let sizes = rpf7_resource_page_sizes(flags, 0).unwrap();
            assert_eq!(sizes.system_bytes, expected, "flags={flags:#010x}");
            assert_eq!(sizes.graphics_bytes, 0);
        }

        let all_fields = 0x0FFF_FFF0_u32;
        let baseline = rpf7_resource_page_sizes(all_fields, 0).unwrap();
        assert_eq!(baseline.system_bytes, 512 * 5663);
        let with_reserved_bits = rpf7_resource_page_sizes(all_fields | 0xF000_0000, 0).unwrap();
        assert_eq!(with_reserved_bits, baseline);
    }

    fn payload_with_body(body: &[u8]) -> Vec<u8> {
        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(body).unwrap();
        let compressed = encoder.finish().unwrap();
        let mut payload = b"RSC7".to_vec();
        payload.extend_from_slice(&1_u32.to_le_bytes());
        payload.extend_from_slice(&(1_u32 << 27).to_le_bytes());
        payload.extend_from_slice(&0_u32.to_le_bytes());
        payload.extend_from_slice(&compressed);
        payload
    }

    #[test]
    fn rejects_corrupt_headers_empty_flags_and_invalid_deflate() {
        let mut short_header = fixture();
        short_header.truncate(15);
        assert!(decode_rsc7_resource(&short_header, 1024).is_err());

        let mut wrong_magic = fixture();
        wrong_magic[0..4].copy_from_slice(b"NOPE");
        assert!(decode_rsc7_resource(&wrong_magic, 1024).unwrap_err().0.contains("magic"));

        let mut empty_flags = fixture();
        empty_flags[8..12].copy_from_slice(&0_u32.to_le_bytes());
        assert!(decode_rsc7_resource(&empty_flags, 1024).unwrap_err().0.contains("empty resource"));

        let mut invalid_deflate = fixture();
        invalid_deflate.truncate(16);
        assert!(decode_rsc7_resource(&invalid_deflate, 1024).is_err());
    }

    #[test]
    fn rejects_short_and_oversized_decompressed_bodies() {
        let short = payload_with_body(&vec![0x3C; 511]);
        assert!(decode_rsc7_resource(&short, 1024).unwrap_err().0.contains("size mismatch"));

        let oversized = payload_with_body(&vec![0x3C; 513]);
        assert!(decode_rsc7_resource(&oversized, 1024).unwrap_err().0.contains("size mismatch"));

        let oversized_stream = payload_with_body(&vec![0x3C; 513]);
        let oversized_len = oversized_stream.len() as u64;
        let mut reader = Cursor::new(oversized_stream);
        let mut output = Vec::new();
        assert!(decode_rsc7_resource_stream(
            &mut reader, 0, oversized_len, &mut output, 1024
        ).is_err());
    }

    #[test]
    fn streaming_decoder_rejects_output_cap_and_truncated_payload() {
        let payload = fixture();
        let mut reader = Cursor::new(payload.clone());
        let mut output = Vec::new();
        assert!(decode_rsc7_resource_stream(
            &mut reader, 0, payload.len() as u64, &mut output, 128
        ).unwrap_err().0.contains("exceeds configured limit"));

        let mut reader = Cursor::new(payload.clone());
        let mut output = Vec::new();
        assert!(decode_rsc7_resource_stream(
            &mut reader, 0, 12, &mut output, 1024
        ).is_err());
    }
}


/// Bounded file-backed archive metadata readers.
pub mod file_backed;
pub use file_backed::{copy_mounted_rpf7_asset_from_inventory_at, copy_mounted_rpf7_asset_from_reader, decode_mounted_rsc7_asset_from_reader, decode_mounted_rsc7_asset_pages_from_reader, extract_rpf7_rsc7_entry_from_reader, mount_rpf7_open_reader_tree, scan_nested_rpf7_open_reader_recursive};


/// Recognized GTA V resource-file extensions for the next-stage asset loader.
/// This is filename classification only; it does not prove that a payload is
/// valid GTA geometry, drawable, dictionary, or texture data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GtaResourceCandidateKind {
    DrawableYdr,
    DrawableDictionaryYdd,
    TextureDictionaryYtd,
}

/// Classify a mounted virtual path by its GTA V resource extension.
/// Matching is ASCII case-insensitive and ignores query-like suffixes only
/// when absent; virtual mount paths are expected to be plain relative paths.
pub fn classify_gta_resource_candidate(path: &str) -> Option<GtaResourceCandidateKind> {
    let extension = path.rsplit_once('.')?.1;
    if extension.contains('/') || extension.contains('\\') {
        return None;
    }
    if extension.eq_ignore_ascii_case("ydr") {
        Some(GtaResourceCandidateKind::DrawableYdr)
    } else if extension.eq_ignore_ascii_case("ydd") {
        Some(GtaResourceCandidateKind::DrawableDictionaryYdd)
    } else if extension.eq_ignore_ascii_case("ytd") {
        Some(GtaResourceCandidateKind::TextureDictionaryYtd)
    } else {
        None
    }
}

#[cfg(test)]
mod gta_resource_candidate_tests {
    use super::*;

    #[test]
    fn classifies_supported_gta_resource_extensions_case_insensitively() {
        assert_eq!(classify_gta_resource_candidate("props/ramp.ydr"), Some(GtaResourceCandidateKind::DrawableYdr));
        assert_eq!(classify_gta_resource_candidate("CHARACTERS/MP/YDD.YDD"), Some(GtaResourceCandidateKind::DrawableDictionaryYdd));
        assert_eq!(classify_gta_resource_candidate("vehicles/shared.ytd"), Some(GtaResourceCandidateKind::TextureDictionaryYtd));
    }

    #[test]
    fn leaves_unrecognized_and_malformed_extensions_unclassified() {
        for path in ["readme.txt", "audio.awc", "props/no_extension", "props/a.ydr/extra", ""] {
            assert_eq!(classify_gta_resource_candidate(path), None, "path={path}");
        }
    }
}
