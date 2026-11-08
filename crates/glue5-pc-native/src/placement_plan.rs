//! Explicit, fully resolved JSON placement plans for a future world/asset backend.
use crate::{AssetMountIndex, JsonPlacement, ManifestError, MountedAsset};

#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedJsonPlacement {
    pub placement_index: usize,
    pub placement: JsonPlacement,
    pub virtual_path: String,
    pub asset: MountedAsset,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedJsonPlacementPlan {
    pub placements: Vec<ResolvedJsonPlacement>,
}

/// Resolve every JSON placement against the mounted index before any world
/// mutation. The result is an immutable-by-convention plan carrying source
/// archive ranges for a later bounded file reader and native world adapter.
/// This does not decode the referenced GTA resource or spawn an entity.
pub fn build_resolved_json_placement_plan(
    document: &crate::JsonPlacementDocument,
    mounts: &AssetMountIndex,
    max_placements: usize,
) -> Result<ResolvedJsonPlacementPlan, ManifestError> {
    if document.placements.len() > max_placements {
        return Err(ManifestError(format!(
            "placement plan contains {} entries; limit is {}",
            document.placements.len(), max_placements
        )));
    }

    let mut resolved = Vec::with_capacity(document.placements.len());
    for (index, placement) in document.placements.iter().enumerate() {
        let (virtual_path, asset) = mounts.resolve_with_path(&placement.asset)
            .map_err(|error| ManifestError(format!("placement {index} asset path invalid: {}", error.0)))?
            .ok_or_else(|| ManifestError(format!(
                "placement {index} asset is not mounted: {}", placement.asset
            )))?;
        resolved.push(ResolvedJsonPlacement {
            placement_index: index,
            placement: placement.clone(),
            virtual_path: virtual_path.to_owned(),
            asset: asset.clone(),
        });
    }
    Ok(ResolvedJsonPlacementPlan { placements: resolved })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{parse_json_placements, MountedAssetKind};

    const SAMPLE: &[u8] = include_bytes!("../examples/maze_bank_ramp.placements.json");

    fn mount_index() -> AssetMountIndex {
        let mut index = AssetMountIndex::new();
        for (path, entry_index, offset) in [
            ("props/maze_bank_ramp.ydr", 17, 8704),
            ("props/maze_bank_barrier.ydr", 23, 11776),
        ] {
            index.insert(path, MountedAsset {
                archive: "mods/maze.rpf".into(),
                entry_index,
                offset,
                length: 4096,
                kind: MountedAssetKind::Resource,
            }).unwrap();
        }
        index
    }

    #[test]
    fn creates_complete_ordered_plan_with_absolute_source_ranges() {
        let doc = parse_json_placements(SAMPLE).unwrap();
        let plan = build_resolved_json_placement_plan(&doc, &mount_index(), 16).unwrap();
        assert_eq!(plan.placements.len(), 2);
        assert_eq!(plan.placements[0].placement_index, 0);
        assert_eq!(plan.placements[0].virtual_path, "props/maze_bank_ramp.ydr");
        assert_eq!(plan.placements[0].asset.offset, 8704);
        assert_eq!(plan.placements[1].placement.asset, "props/maze_bank_barrier.ydr");
        assert_eq!(plan.placements[1].asset.entry_index, 23);
    }

    #[test]
    fn fails_entire_plan_when_any_asset_is_missing() {
        let doc = parse_json_placements(SAMPLE).unwrap();
        let mut mounts = AssetMountIndex::new();
        mounts.insert("props/maze_bank_ramp.ydr", MountedAsset {
            archive: "mods/maze.rpf".into(), entry_index: 17,
            offset: 8704, length: 4096, kind: MountedAssetKind::Resource,
        }).unwrap();
        let error = build_resolved_json_placement_plan(&doc, &mounts, 16).unwrap_err();
        assert!(error.0.contains("placement 1"));
        assert!(error.0.contains("maze_bank_barrier.ydr"));
    }

    #[test]
    fn rejects_plan_count_above_caller_limit() {
        let doc = parse_json_placements(SAMPLE).unwrap();
        assert!(build_resolved_json_placement_plan(&doc, &mount_index(), 1)
            .unwrap_err().0.contains("limit"));
    }
}
