//! Bounded discovery of nested, unencrypted GTA V Legacy RPF7 archives.
//!
//! This is an inventory/discovery layer only. It follows only uncompressed,
//! unencrypted binary entries ending in .rpf. It does not decrypt NG archives,
//! decompress compressed binary entries, mount game assets, or resolve paths
//! through RPF directory records.

use crate::{copy_rpf7_open_binary_entry, inventory_rpf7_open, ManifestError, Rpf7Inventory};
use sha2::{Digest, Sha256};
use std::io::Cursor;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NestedRpf7Limits {
    pub max_depth: usize,
    pub max_archives: usize,
    pub max_single_archive_bytes: usize,
    pub max_total_archive_bytes: usize,
}

impl Default for NestedRpf7Limits {
    fn default() -> Self {
        Self {
            max_depth: 4,
            max_archives: 128,
            max_single_archive_bytes: 256 * 1024 * 1024,
            max_total_archive_bytes: 1024 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NestedRpf7Archive {
    /// Root is ""; children are represented by their containing entry names.
    /// This is a discovery label, not a complete GTA virtual asset path.
    pub archive_label: String,
    pub depth: usize,
    pub byte_length: usize,
    pub sha256: String,
    pub inventory: Rpf7Inventory,
}

fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Inventory a root RPF7 and recursively discover bounded nested .rpf files.
///
/// Only OPEN/NONE RPF7 archives are accepted by the existing inventory parser.
/// Nested entries must be stored uncompressed and unencrypted. Unsupported
/// entries are skipped; malformed candidate nested archives fail the scan.
pub fn discover_nested_rpf7_archives(
    root_archive: &[u8],
    limits: &NestedRpf7Limits,
) -> Result<Vec<NestedRpf7Archive>, ManifestError> {
    if limits.max_archives == 0 || limits.max_single_archive_bytes < 16
        || limits.max_total_archive_bytes < 16
    {
        return Err(ManifestError("nested RPF7 limits are too small".into()));
    }
    if root_archive.len() > limits.max_single_archive_bytes
        || root_archive.len() > limits.max_total_archive_bytes
    {
        return Err(ManifestError("root RPF7 archive exceeds configured byte budget".into()));
    }

    let mut output = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut total_bytes = 0usize;
    let mut pending = vec![(String::new(), 0usize, root_archive.to_vec())];

    while let Some((label, depth, bytes)) = pending.pop() {
        let hash = digest(&bytes);
        if !seen.insert(hash.clone()) {
            continue;
        }
        if output.len() >= limits.max_archives {
            return Err(ManifestError("nested RPF7 archive count exceeds configured limit".into()));
        }
        total_bytes = total_bytes.checked_add(bytes.len())
            .ok_or_else(|| ManifestError("nested RPF7 aggregate byte count overflow".into()))?;
        if total_bytes > limits.max_total_archive_bytes {
            return Err(ManifestError("nested RPF7 archives exceed aggregate byte budget".into()));
        }

        let inventory = inventory_rpf7_open(&bytes)?;
        let child_candidates: Vec<u32> = inventory.entries.iter().filter_map(|entry| {
            let is_rpf = entry.name.to_ascii_lowercase().ends_with(".rpf");
            match &entry.kind {
                crate::Rpf7EntryKind::BinaryFile { encrypted: false, file_size, uncompressed_size, .. }
                    if is_rpf && (*file_size == 0 || file_size == uncompressed_size) => Some(entry.index),
                _ => None,
            }
        }).collect();

        output.push(NestedRpf7Archive {
            archive_label: label.clone(),
            depth,
            byte_length: bytes.len(),
            sha256: hash,
            inventory: inventory.clone(),
        });

        if depth >= limits.max_depth || child_candidates.is_empty() {
            continue;
        }
        for entry_index in child_candidates.into_iter().rev() {
            let entry = inventory.entries.iter().find(|entry| entry.index == entry_index)
                .ok_or_else(|| ManifestError("nested RPF7 candidate disappeared from inventory".into()))?;
            let mut child = Vec::new();
            let written = copy_rpf7_open_binary_entry(
                &bytes, &inventory, entry_index, limits.max_single_archive_bytes, &mut child,
            )?;
            if written != child.len() as u64 || child.len() < 16 {
                continue;
            }
            if child.len() > limits.max_single_archive_bytes {
                return Err(ManifestError("nested RPF7 child exceeds per-archive byte budget".into()));
            }
            let child_label = if label.is_empty() {
                entry.name.clone()
            } else {
                format!("{label}/{}", entry.name)
            };
            // A malformed .rpf candidate is not silently treated as a valid archive.
            if child.get(0..4) != Some(b"RPF7") {
                continue;
            }
            if child.len() > limits.max_total_archive_bytes.saturating_sub(total_bytes) {
                return Err(ManifestError("nested RPF7 child exceeds remaining aggregate byte budget".into()));
            }
            pending.push((child_label, depth + 1, child));
        }
    }

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_open_rpf7() -> Vec<u8> {
        let mut bytes = b"RPF7".to_vec();
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(&0x4E45_504F_u32.to_le_bytes());
        bytes
    }

    #[test]
    fn inventories_root_open_rpf7_with_bounded_metadata() {
        let root = empty_open_rpf7();
        let result = discover_nested_rpf7_archives(&root, &NestedRpf7Limits::default()).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].archive_label, "");
        assert_eq!(result[0].depth, 0);
        assert_eq!(result[0].byte_length, 16);
        assert!(result[0].inventory.entries.is_empty());
    }

    #[test]
    fn rejects_archive_when_per_archive_budget_is_exceeded() {
        let root = empty_open_rpf7();
        let mut limits = NestedRpf7Limits::default();
        limits.max_single_archive_bytes = 15;
        assert!(discover_nested_rpf7_archives(&root, &limits).unwrap_err().0.contains("byte budget"));
    }

    #[test]
    fn rejects_zero_archive_count_limit() {
        let root = empty_open_rpf7();
        let mut limits = NestedRpf7Limits::default();
        limits.max_archives = 0;
        assert!(discover_nested_rpf7_archives(&root, &limits).is_err());
    }

    #[test]
    fn records_stable_digest_for_root_archive() {
        let root = empty_open_rpf7();
        let result = discover_nested_rpf7_archives(&root, &NestedRpf7Limits::default()).unwrap();
        assert_eq!(result[0].sha256, digest(&root));
    }
}
