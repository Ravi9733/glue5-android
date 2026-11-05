//! Bounded raw payload loading for fully resolved JSON placement plans.
//!
//! This stage reads stored bytes only. RSC7 decompression, GTA structure
//! decoding, GPU upload, and world spawning remain separate stages.
use crate::{ManifestError, ResolvedJsonPlacementPlan, MountedAsset, MountedAssetKind};
use std::io::{Read, Seek, SeekFrom};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedPlacementPayload {
    pub placement_index: usize,
    pub virtual_path: String,
    pub asset: MountedAsset,
    /// Raw bytes as stored in the backing file (RSC7 resources remain encoded).
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedPlacementPayloadBatch {
    pub payloads: Vec<ResolvedPlacementPayload>,
    pub total_bytes: u64,
}

/// Read every asset referenced by a resolved plan from one backing file.
/// Absolute offsets support both root and nested RPF7 mounts. All ranges and
/// aggregate budgets are checked before allocation; a short read rejects the
/// whole batch. Resource entries are returned as stored RSC7 bytes.
pub fn read_resolved_json_placement_payloads<R: Read + Seek>(
    reader: &mut R,
    plan: &ResolvedJsonPlacementPlan,
    max_asset_bytes: u64,
    max_total_bytes: u64,
) -> Result<ResolvedPlacementPayloadBatch, ManifestError> {
    let backing_len = reader.seek(SeekFrom::End(0))
        .map_err(|e| ManifestError(format!("cannot seek to placement backing-file end: {e}")))?;
    let mut total = 0_u64;
    let mut payloads = Vec::new();
    payloads.try_reserve_exact(plan.placements.len())
        .map_err(|e| ManifestError(format!("cannot reserve placement payload list: {e}")))?;

    for item in &plan.placements {
        let asset = &item.asset;
        if asset.length == 0 || asset.length > max_asset_bytes {
            return Err(ManifestError(format!(
                "placement {} asset length is empty or exceeds per-asset limit ({} > {})",
                item.placement_index, asset.length, max_asset_bytes
            )));
        }
        let end = asset.offset.checked_add(asset.length)
            .ok_or_else(|| ManifestError(format!("placement {} asset range overflows", item.placement_index)))?;
        if end > backing_len {
            return Err(ManifestError(format!(
                "placement {} asset range exceeds backing file (end {} > {})",
                item.placement_index, end, backing_len
            )));
        }
        total = total.checked_add(asset.length)
            .ok_or_else(|| ManifestError("placement payload aggregate length overflows".into()))?;
        if total > max_total_bytes {
            return Err(ManifestError(format!(
                "placement payload batch exceeds aggregate limit ({} > {})",
                total, max_total_bytes
            )));
        }

        let size = usize::try_from(asset.length)
            .map_err(|_| ManifestError(format!("placement {} asset does not fit address space", item.placement_index)))?;
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(size)
            .map_err(|e| ManifestError(format!("cannot allocate placement {} payload: {e}", item.placement_index)))?;
        bytes.resize(size, 0);
        reader.seek(SeekFrom::Start(asset.offset))
            .map_err(|e| ManifestError(format!("cannot seek to placement {} payload: {e}", item.placement_index)))?;
        reader.read_exact(&mut bytes)
            .map_err(|e| ManifestError(format!("cannot read complete placement {} payload: {e}", item.placement_index)))?;

        // Keep the kind explicit for the next decoder; no format conversion is
        // implied by copying the bytes.
        match asset.kind {
            MountedAssetKind::Binary | MountedAssetKind::Resource => {}
        }
        payloads.push(ResolvedPlacementPayload {
            placement_index: item.placement_index,
            virtual_path: item.virtual_path.clone(),
            asset: asset.clone(),
            bytes,
        });
    }

    Ok(ResolvedPlacementPayloadBatch { payloads, total_bytes: total })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{JsonPlacement, ResolvedJsonPlacement};

    fn plan() -> ResolvedJsonPlacementPlan {
        ResolvedJsonPlacementPlan { placements: vec![
            ResolvedJsonPlacement {
                placement_index: 0,
                placement: JsonPlacement {
                    asset: "props/a.ydr".into(),
                    position: [0.0, 0.0, 0.0],
                    rotation: [0.0, 0.0, 0.0],
                    scale: [1.0, 1.0, 1.0],
                },
                virtual_path: "props/a.ydr".into(),
                asset: MountedAsset {
                    archive: "mods/a.rpf".into(), entry_index: 1,
                    offset: 4, length: 3, kind: MountedAssetKind::Resource,
                },
            },
            ResolvedJsonPlacement {
                placement_index: 1,
                placement: JsonPlacement {
                    asset: "props/b.ydr".into(),
                    position: [1.0, 2.0, 3.0],
                    rotation: [0.0, 0.0, 0.0],
                    scale: [1.0, 1.0, 1.0],
                },
                virtual_path: "props/b.ydr".into(),
                asset: MountedAsset {
                    archive: "mods/a.rpf".into(), entry_index: 2,
                    offset: 10, length: 2, kind: MountedAssetKind::Binary,
                },
            },
        ] }
    }

    #[test]
    fn reads_ordered_payloads_from_absolute_offsets() {
        let mut reader = std::io::Cursor::new(b"xxxxABCxxxDEzz".to_vec());
        let batch = read_resolved_json_placement_payloads(&mut reader, &plan(), 8, 5).unwrap();
        assert_eq!(batch.total_bytes, 5);
        assert_eq!(batch.payloads[0].bytes, b"ABC");
        assert_eq!(batch.payloads[1].bytes, b"DE");
        assert_eq!(batch.payloads[1].placement_index, 1);
    }

    #[test]
    fn rejects_per_asset_and_aggregate_budget_overruns() {
        let mut reader = std::io::Cursor::new(vec![0; 32]);
        assert!(read_resolved_json_placement_payloads(&mut reader, &plan(), 2, 32)
            .unwrap_err().0.contains("per-asset"));
        assert!(read_resolved_json_placement_payloads(&mut reader, &plan(), 8, 4)
            .unwrap_err().0.contains("aggregate"));
    }

    #[test]
    fn rejects_ranges_outside_backing_file_and_truncated_reads() {
        let mut invalid = plan();
        invalid.placements[0].asset.offset = 30;
        let mut reader = std::io::Cursor::new(vec![0; 32]);
        assert!(read_resolved_json_placement_payloads(&mut reader, &invalid, 8, 32)
            .unwrap_err().0.contains("backing file"));

        // Simulate a seekable backing source whose reported length becomes
        // stale between metadata lookup and the exact payload read.
        struct ShortReader(std::io::Cursor<Vec<u8>>);
        impl std::io::Read for ShortReader {
            fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
                std::io::Read::read(&mut self.0, buf)
            }
        }
        impl std::io::Seek for ShortReader {
            fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
                if matches!(pos, std::io::SeekFrom::End(0)) {
                    Ok(64)
                } else {
                    std::io::Seek::seek(&mut self.0, pos)
                }
            }
        }
        let mut short = ShortReader(std::io::Cursor::new(b"xxxxABC".to_vec()));
        let mut one = plan();
        one.placements.truncate(1);
        one.placements[0].asset.offset = 4;
        one.placements[0].asset.length = 8;
        assert!(read_resolved_json_placement_payloads(&mut short, &one, 16, 16)
            .unwrap_err().0.contains("complete"));
    }
}
