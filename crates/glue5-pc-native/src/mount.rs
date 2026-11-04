//! Case-insensitive virtual asset index for experimental PC content mounts.
//!
//! This is a metadata registry only. Archive scanners must supply already
//! validated payload ranges; this module never decrypts or decodes assets.

use std::collections::HashMap;
use std::path::{Component, Path};

const MAX_VIRTUAL_PATH_BYTES: usize = 512;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MountedAssetKind {
    Binary,
    Resource,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MountedAsset {
    /// Archive identifier chosen by the caller (for example, a DLC RPF path).
    pub archive: String,
    pub entry_index: u32,
    /// Absolute byte range in the source archive, already validated by RPF code.
    pub offset: u64,
    pub length: u64,
    pub kind: MountedAssetKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MountError(pub String);

impl std::fmt::Display for MountError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for MountError {}

/// A deterministic, case-insensitive lookup table for mounted virtual assets.
/// Duplicate paths (including case-only duplicates) are rejected instead of
/// silently depending on mount order.
#[derive(Clone, Debug, Default)]
pub struct AssetMountIndex {
    entries: HashMap<String, (String, MountedAsset)>,
}

impl AssetMountIndex {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Register a virtual path such as "props/maze_bank_ramp.ydr".
    pub fn insert(&mut self, virtual_path: &str, asset: MountedAsset) -> Result<(), MountError> {
        validate_virtual_path(virtual_path)?;
        if asset.archive.is_empty() {
            return Err(MountError("archive identifier cannot be empty".into()));
        }
        if asset.length == 0 {
            return Err(MountError("mounted asset length must be non-zero".into()));
        }
        asset.offset.checked_add(asset.length)
            .ok_or_else(|| MountError("mounted asset byte range overflows".into()))?;

        let key = virtual_path.to_ascii_lowercase();
        if self.entries.contains_key(&key) {
            return Err(MountError(format!("duplicate mounted asset path: {virtual_path}")));
        }
        self.entries.insert(key, (virtual_path.to_owned(), asset));
        Ok(())
    }

    /// Resolve an asset path without extracting it. Lookup is ASCII
    /// case-insensitive to match common GTA asset naming conventions.
    pub fn resolve(&self, virtual_path: &str) -> Result<Option<&MountedAsset>, MountError> {
        validate_virtual_path(virtual_path)?;
        Ok(self.entries.get(&virtual_path.to_ascii_lowercase()).map(|(_, asset)| asset))
    }

    /// Return the original registered spelling and its source descriptor.
    pub fn resolve_with_path(&self, virtual_path: &str) -> Result<Option<(&str, &MountedAsset)>, MountError> {
        validate_virtual_path(virtual_path)?;
        Ok(self.entries.get(&virtual_path.to_ascii_lowercase())
            .map(|(path, asset)| (path.as_str(), asset)))
    }
}

/// Populate a virtual mount index from an OPEN/NONE RPF7 inventory.
///
/// RPF7 TOC names are leaf names, so this adapter places them beneath the
/// caller-provided virtual prefix. It only registers payloads whose storage
/// ranges can be proven against the archive length. Encrypted/compressed
/// binary entries and resource-size sentinels are deliberately skipped.
pub fn add_rpf7_inventory_to_mount(
    index: &mut AssetMountIndex,
    archive_id: &str,
    virtual_prefix: &str,
    archive_len: u64,
    inventory: &crate::Rpf7Inventory,
) -> Result<usize, MountError> {
    use crate::Rpf7EntryKind;

    if archive_id.is_empty() {
        return Err(MountError("archive identifier cannot be empty".into()));
    }
    let metadata_end = 16_u64
        .checked_add(inventory.header.toc_bytes)
        .and_then(|n| n.checked_add(u64::from(inventory.header.names_length)))
        .ok_or_else(|| MountError("RPF7 metadata boundary overflows".into()))?;
    if metadata_end > archive_len {
        return Err(MountError("RPF7 metadata extends beyond archive length".into()));
    }

    let prefix = virtual_prefix.trim_matches('/');
    if !prefix.is_empty() {
        validate_virtual_path(prefix)?;
    }

    let mut added = 0usize;
    for entry in &inventory.entries {
        let (size, offset_blocks, kind) = match &entry.kind {
            Rpf7EntryKind::Directory { .. } => continue,
            Rpf7EntryKind::BinaryFile {
                file_size, file_offset_blocks, uncompressed_size, encrypted,
            } => {
                if *encrypted || (*file_size != 0 && file_size != uncompressed_size) {
                    continue;
                }
                (if *file_size == 0 { u64::from(*uncompressed_size) } else { u64::from(*file_size) },
                 u64::from(*file_offset_blocks), MountedAssetKind::Binary)
            }
            Rpf7EntryKind::ResourceFile { file_size, file_offset_blocks, .. } => {
                if *file_size == 0x00FF_FFFF { continue; }
                (u64::from(*file_size), u64::from(*file_offset_blocks), MountedAssetKind::Resource)
            }
        };
        if size == 0 { continue; }
        let start = offset_blocks.checked_mul(512)
            .ok_or_else(|| MountError(format!("RPF7 entry {} offset overflows", entry.index)))?;
        let end = start.checked_add(size)
            .ok_or_else(|| MountError(format!("RPF7 entry {} range overflows", entry.index)))?;
        if start < metadata_end || end > archive_len { continue; }

        let path = if prefix.is_empty() {
            entry.name.clone()
        } else {
            format!("{prefix}/{}", entry.name)
        };
        index.insert(&path, MountedAsset {
            archive: archive_id.to_owned(),
            entry_index: entry.index,
            offset: start,
            length: size,
            kind,
        })?;
        added += 1;
    }
    Ok(added)
}

fn validate_virtual_path(path: &str) -> Result<(), MountError> {
    if path.is_empty() || path.len() > MAX_VIRTUAL_PATH_BYTES {
        return Err(MountError("virtual asset path is empty or too long".into()));
    }
    if path.starts_with('/') || path.contains('\\') || path.contains(':') {
        return Err(MountError("virtual asset path must be relative and use forward slashes".into()));
    }
    if Path::new(path).components().any(|part| !matches!(part, Component::Normal(_))) {
        return Err(MountError("virtual asset path contains traversal or non-normal component".into()));
    }
    if path.split('/').any(str::is_empty) {
        return Err(MountError("virtual asset path contains an empty component".into()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source() -> MountedAsset {
        MountedAsset {
            archive: "mods/example.rpf".into(),
            entry_index: 7,
            offset: 4096,
            length: 128,
            kind: MountedAssetKind::Resource,
        }
    }

    #[test]
    fn resolves_paths_case_insensitively() {
        let mut index = AssetMountIndex::new();
        index.insert("props/Ramp.YDR", source()).unwrap();
        let found = index.resolve("PROPS/ramp.ydr").unwrap().unwrap();
        assert_eq!(found.archive, "mods/example.rpf");
        assert_eq!(found.entry_index, 7);
        assert_eq!(found.offset, 4096);
        assert_eq!(found.length, 128);
        assert_eq!(found.kind, MountedAssetKind::Resource);
    }

    #[test]
    fn rejects_duplicate_paths_even_when_case_differs() {
        let mut index = AssetMountIndex::new();
        index.insert("props/ramp.ydr", source()).unwrap();
        assert!(index.insert("PROPS/RAMP.YDR", source()).unwrap_err().0.contains("duplicate"));
        assert_eq!(index.len(), 1);
    }

    #[test]
    fn rejects_unsafe_paths_and_invalid_ranges() {
        let mut index = AssetMountIndex::new();
        for path in ["../ramp.ydr", "/ramp.ydr", "props\\ramp.ydr", "C:/ramp.ydr", "props//ramp.ydr"] {
            assert!(index.insert(path, source()).is_err(), "path={path}");
        }
        let mut empty = source();
        empty.length = 0;
        assert!(index.insert("props/empty.ydr", empty).is_err());
        let mut overflow = source();
        overflow.offset = u64::MAX;
        assert!(index.insert("props/overflow.ydr", overflow).is_err());
    }

    #[test]
    fn missing_asset_is_a_normal_lookup_result() {
        let index = AssetMountIndex::new();
        assert!(index.resolve("props/not_mounted.ydr").unwrap().is_none());
    }
}
