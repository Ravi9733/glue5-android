//! Bounded bridge from decoded placement YDR/YDD resources to raw geometry streams.
//!
//! The result contains extracted vertex/index bytes only. A GTA vertex
//! declaration is not present in the current geometry summary, so this stage
//! deliberately does not guess a layout or claim to emit renderable meshes.
use crate::{
    classify_gta_resource_candidate, inspect_gta_drawable_structure,
    read_gta_geometry_buffers, split_rsc7_resource_pages, DecodedPlacementAssetBatch,
    DecodedPlacementContent, GtaGeometryBufferData, GtaGeometrySummary,
    GtaResourceCandidateKind, ManifestError, MountedGtaDrawablePages,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InspectedPlacementGeometryBuffers {
    pub placement_index: usize,
    pub virtual_path: String,
    pub geometry_index: usize,
    pub geometry: GtaGeometrySummary,
    pub buffers: GtaGeometryBufferData,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct PlacementGeometryBufferBatch {
    pub geometries: Vec<InspectedPlacementGeometryBuffers>,
    pub ignored_non_drawable_assets: usize,
    pub geometries_without_vertex_stream: usize,
}

/// Extract bounded raw vertex/index streams from every parsed YDR/YDD geometry.
///
/// This function reuses the existing structure and page-buffer readers. It
/// preserves geometry identity and returns entries with missing streams for
/// diagnostics. It does not infer vertex declarations, decode mesh attributes,
/// resolve materials, upload GPU buffers, or spawn world objects.
pub fn read_decoded_placement_geometry_buffers(
    batch: &DecodedPlacementAssetBatch,
    max_vertices_per_geometry: u32,
    max_indices_per_geometry: u32,
    max_buffer_bytes_per_geometry: usize,
    max_total_buffer_bytes: usize,
) -> Result<PlacementGeometryBufferBatch, ManifestError> {
    let mut result = PlacementGeometryBufferBatch::default();
    let mut total_bytes = 0usize;

    for asset in &batch.assets {
        let Some(kind) = classify_gta_resource_candidate(&asset.virtual_path) else {
            result.ignored_non_drawable_assets += 1;
            continue;
        };
        if !matches!(kind, GtaResourceCandidateKind::DrawableYdr | GtaResourceCandidateKind::DrawableDictionaryYdd) {
            result.ignored_non_drawable_assets += 1;
            continue;
        }
        let decoded = match &asset.content {
            DecodedPlacementContent::Rsc7(resource) => resource,
            DecodedPlacementContent::Binary(_) => {
                return Err(ManifestError(format!(
                    "placement {} ({}) is a drawable path but contains binary, not RSC7 data",
                    asset.placement_index, asset.virtual_path
                )));
            }
        };
        let (system, graphics) = split_rsc7_resource_pages(&decoded.body, decoded.page_sizes)?;
        let pages = MountedGtaDrawablePages {
            virtual_path: asset.virtual_path.clone(),
            candidate_kind: kind,
            resource_version: decoded.version,
            page_sizes: decoded.page_sizes,
            system_page: system.to_vec(),
            graphics_page: graphics.to_vec(),
        };
        let summary = inspect_gta_drawable_structure(&pages)?;
        for drawable in &summary.drawables {
            for model in &drawable.models {
                for geometry in &model.geometries {
                    let buffers = read_gta_geometry_buffers(
                        &pages, geometry, max_vertices_per_geometry,
                        max_indices_per_geometry, max_buffer_bytes_per_geometry,
                    )?;
                    let vertex_bytes = buffers.vertex_data.as_ref().map_or(0, |v| v.bytes.len());
                    let index_bytes = buffers.index_data.as_ref().map_or(0, |v| v.bytes.len());
                    let geometry_bytes = vertex_bytes.checked_add(index_bytes)
                        .ok_or_else(|| ManifestError("geometry buffer byte count overflow".into()))?;
                    total_bytes = total_bytes.checked_add(geometry_bytes)
                        .ok_or_else(|| ManifestError("placement geometry aggregate byte count overflow".into()))?;
                    if total_bytes > max_total_buffer_bytes {
                        return Err(ManifestError(format!(
                            "placement geometry buffers exceed aggregate byte limit ({} > {})",
                            total_bytes, max_total_buffer_bytes
                        )));
                    }
                    if buffers.vertex_data.is_none() {
                        result.geometries_without_vertex_stream += 1;
                    }
                    let geometry_index = result.geometries.len();
                    result.geometries.try_reserve(1)
                        .map_err(|e| ManifestError(format!("cannot reserve geometry result: {e}")))?;
                    result.geometries.push(InspectedPlacementGeometryBuffers {
                        placement_index: asset.placement_index,
                        virtual_path: asset.virtual_path.clone(),
                        geometry_index,
                        geometry: geometry.clone(),
                        buffers,
                    });
                }
            }
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DecodedPlacementAsset, DecodedPlacementContent};

    #[test]
    fn ignores_non_drawable_placement_assets() {
        let batch = DecodedPlacementAssetBatch {
            assets: vec![DecodedPlacementAsset {
                placement_index: 5,
                virtual_path: "scripts/config.json".into(),
                content: DecodedPlacementContent::Binary(b"{}".to_vec()),
                decoded_bytes: 2,
            }],
            total_decoded_bytes: 2,
        };
        let result = read_decoded_placement_geometry_buffers(&batch, 100, 100, 4096, 8192).unwrap();
        assert!(result.geometries.is_empty());
        assert_eq!(result.ignored_non_drawable_assets, 1);
    }

    #[test]
    fn rejects_drawable_path_with_non_rsc7_payload() {
        let batch = DecodedPlacementAssetBatch {
            assets: vec![DecodedPlacementAsset {
                placement_index: 6,
                virtual_path: "props/object.ydd".into(),
                content: DecodedPlacementContent::Binary(vec![1, 2, 3]),
                decoded_bytes: 3,
            }],
            total_decoded_bytes: 3,
        };
        assert!(read_decoded_placement_geometry_buffers(&batch, 100, 100, 4096, 8192)
            .unwrap_err().0.contains("binary, not RSC7"));
    }
}
