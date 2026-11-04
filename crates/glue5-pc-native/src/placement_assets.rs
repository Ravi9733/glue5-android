//! Connect decoded placement resources to the existing GTA drawable and texture inspectors.
//!
//! This is a bounded inspection adapter only. It does not perform RAGE pointer
//! relocation, GPU upload, material/shader resolution, or world spawning. The
//! YTD layout parser remains experimental and must be validated against retail assets.
use crate::{
    classify_gta_resource_candidate, inspect_gta_drawable_structure,
    parse_ytd_resource_pages, split_rsc7_resource_pages, DecodedPlacementAssetBatch,
    DecodedPlacementContent, GtaDrawableResourceSummary, GtaResourceCandidateKind,
    ManifestError, MountedGtaDrawablePages, YtdDictionary,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlacementGtaAssetInspection {
    Drawable(GtaDrawableResourceSummary),
    TextureDictionary(YtdDictionary),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InspectedPlacementGtaAsset {
    pub placement_index: usize,
    pub virtual_path: String,
    pub inspection: PlacementGtaAssetInspection,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct PlacementGtaAssetInspectionBatch {
    pub assets: Vec<InspectedPlacementGtaAsset>,
    pub ignored_non_gta_assets: usize,
}

/// Inspect decoded .ydr/.ydd/.ytd resources from a placement batch.
///
/// Every recognized GTA extension must contain a decoded RSC7 resource; a
/// recognized path with binary or malformed content fails the entire batch.
/// Unknown extensions are ignored. All page ranges are checked before parsers
/// are called. YTD parsing is explicitly experimental pending retail validation.
pub fn inspect_decoded_placement_gta_assets(
    batch: &DecodedPlacementAssetBatch,
) -> Result<PlacementGtaAssetInspectionBatch, ManifestError> {
    let mut result = PlacementGtaAssetInspectionBatch::default();
    result.assets.try_reserve(batch.assets.len())
        .map_err(|e| ManifestError(format!("cannot reserve placement inspection list: {e}")))?;

    for asset in &batch.assets {
        let Some(kind) = classify_gta_resource_candidate(&asset.virtual_path) else {
            result.ignored_non_gta_assets += 1;
            continue;
        };
        let decoded = match &asset.content {
            DecodedPlacementContent::Rsc7(resource) => resource,
            DecodedPlacementContent::Binary(_) => {
                return Err(ManifestError(format!(
                    "placement {} ({}) is a recognized GTA resource path but contains binary, not RSC7 data",
                    asset.placement_index, asset.virtual_path
                )));
            }
        };
        let (system, graphics) = split_rsc7_resource_pages(&decoded.body, decoded.page_sizes)?;
        let inspection = match kind {
            GtaResourceCandidateKind::DrawableYdr | GtaResourceCandidateKind::DrawableDictionaryYdd => {
                let pages = MountedGtaDrawablePages {
                    virtual_path: asset.virtual_path.clone(),
                    candidate_kind: kind,
                    resource_version: decoded.version,
                    page_sizes: decoded.page_sizes,
                    system_page: system.to_vec(),
                    graphics_page: graphics.to_vec(),
                };
                PlacementGtaAssetInspection::Drawable(inspect_gta_drawable_structure(&pages)?)
            }
            GtaResourceCandidateKind::TextureDictionaryYtd => {
                PlacementGtaAssetInspection::TextureDictionary(
                    parse_ytd_resource_pages(system, graphics, decoded.page_sizes)?
                )
            }
        };
        result.assets.push(InspectedPlacementGtaAsset {
            placement_index: asset.placement_index,
            virtual_path: asset.virtual_path.clone(),
            inspection,
        });
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DecodedPlacementAsset, DecodedRsc7Resource, Rpf7ResourcePageSizes};

    fn decoded(path: &str, kind: GtaResourceCandidateKind, system: Vec<u8>, graphics: Vec<u8>) -> DecodedPlacementAsset {
        let page_sizes = Rpf7ResourcePageSizes {
            system_bytes: system.len() as u64,
            graphics_bytes: graphics.len() as u64,
        };
        let mut body = system;
        body.extend_from_slice(&graphics);
        let _ = kind;
        DecodedPlacementAsset {
            placement_index: 3,
            virtual_path: path.into(),
            decoded_bytes: body.len() as u64,
            content: DecodedPlacementContent::Rsc7(DecodedRsc7Resource {
                version: 1, page_sizes, body,
            }),
        }
    }

    #[test]
    fn rejects_recognized_gta_path_with_binary_content() {
        let batch = DecodedPlacementAssetBatch {
            assets: vec![DecodedPlacementAsset {
                placement_index: 2,
                virtual_path: "props/object.ydr".into(),
                content: DecodedPlacementContent::Binary(vec![1, 2, 3]),
                decoded_bytes: 3,
            }],
            total_decoded_bytes: 3,
        };
        assert!(inspect_decoded_placement_gta_assets(&batch).unwrap_err().0.contains("binary, not RSC7"));
    }

    #[test]
    fn ignores_unrecognized_binary_and_preserves_empty_result() {
        let batch = DecodedPlacementAssetBatch {
            assets: vec![DecodedPlacementAsset {
                placement_index: 0,
                virtual_path: "scripts/readme.txt".into(),
                content: DecodedPlacementContent::Binary(b"ok".to_vec()),
                decoded_bytes: 2,
            }],
            total_decoded_bytes: 2,
        };
        let inspected = inspect_decoded_placement_gta_assets(&batch).unwrap();
        assert!(inspected.assets.is_empty());
        assert_eq!(inspected.ignored_non_gta_assets, 1);
    }

    #[test]
    fn validates_page_lengths_before_invoking_ytd_parser() {
        let mut asset = decoded("textures/test.ytd", GtaResourceCandidateKind::TextureDictionaryYtd, vec![0; 512], vec![]);
        if let DecodedPlacementContent::Rsc7(resource) = &mut asset.content {
            resource.body.pop();
        }
        let batch = DecodedPlacementAssetBatch { assets: vec![asset], total_decoded_bytes: 511 };
        assert!(inspect_decoded_placement_gta_assets(&batch).unwrap_err().0.contains("does not match declared page regions"));
    }
}
