//! Pack transformed placement meshes into a conservative GPU upload payload.
//!
//! This bridge emits tightly packed world-space float3 positions and optional
//! u32 indices. It is suitable for the renderer's external mesh upload API;
//! it does not issue a render pass or draw call.
use crate::{ManifestError, PlacementSceneBatch};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlacementGpuMeshPayload {
    pub placement_index: usize,
    pub virtual_path: String,
    pub geometry_index: usize,
    pub vertex_bytes: Vec<u8>,
    pub index_bytes: Option<Vec<u8>>,
    pub vertex_count: u32,
    pub index_count: u32,
    pub vertex_stride: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct PlacementGpuPayloadBatch {
    pub meshes: Vec<PlacementGpuMeshPayload>,
    pub total_upload_bytes: usize,
}

/// Convert every transformed mesh to a bounded position-only upload payload.
pub fn build_placement_gpu_payloads(
    scene: &PlacementSceneBatch,
    max_meshes: usize,
    max_vertices: usize,
    max_indices: usize,
    max_total_upload_bytes: usize,
) -> Result<PlacementGpuPayloadBatch, ManifestError> {
    if scene.meshes.len() > max_meshes {
        return Err(ManifestError("placement GPU payload exceeds mesh count limit".into()));
    }
    let mut result = PlacementGpuPayloadBatch::default();
    let mut vertices_total = 0usize;
    let mut indices_total = 0usize;
    for mesh in &scene.meshes {
        let vertex_count = mesh.vertices.len();
        vertices_total = vertices_total.checked_add(vertex_count)
            .ok_or_else(|| ManifestError("placement GPU vertex count overflow".into()))?;
        if vertex_count == 0 || vertices_total > max_vertices {
            return Err(ManifestError("placement GPU vertex count is empty or exceeds limit".into()));
        }
        let mut vertex_bytes = Vec::with_capacity(vertex_count.checked_mul(12)
            .ok_or_else(|| ManifestError("placement GPU vertex byte count overflow".into()))?);
        for vertex in &mesh.vertices {
            let position = vertex.attributes.iter()
                .find(|attribute| attribute.semantic.eq_ignore_ascii_case("position"))
                .ok_or_else(|| ManifestError("placement GPU mesh has no Position attribute".into()))?;
            if position.components.len() < 3 || position.components[..3].iter().any(|v| !v.is_finite()) {
                return Err(ManifestError("placement GPU Position must contain three finite components".into()));
            }
            for component in &position.components[..3] {
                vertex_bytes.extend_from_slice(&component.to_le_bytes());
            }
        }
        let (index_bytes, index_count) = match &mesh.indices {
            Some(indices) => {
                indices_total = indices_total.checked_add(indices.len())
                    .ok_or_else(|| ManifestError("placement GPU index count overflow".into()))?;
                if indices.is_empty() || indices_total > max_indices
                    || indices.iter().any(|index| *index as usize >= vertex_count)
                {
                    return Err(ManifestError("placement GPU indices are empty, out of range, or exceed limit".into()));
                }
                let mut bytes = Vec::with_capacity(indices.len().checked_mul(4)
                    .ok_or_else(|| ManifestError("placement GPU index byte count overflow".into()))?);
                for index in indices { bytes.extend_from_slice(&index.to_le_bytes()); }
                let count = u32::try_from(indices.len())
                    .map_err(|_| ManifestError("placement GPU index count exceeds u32".into()))?;
                (Some(bytes), count)
            }
            None => (None, 0),
        };
        let count = u32::try_from(vertex_count)
            .map_err(|_| ManifestError("placement GPU vertex count exceeds u32".into()))?;
        let upload = vertex_bytes.len().checked_add(index_bytes.as_ref().map_or(0, Vec::len))
            .ok_or_else(|| ManifestError("placement GPU upload byte count overflow".into()))?;
        result.total_upload_bytes = result.total_upload_bytes.checked_add(upload)
            .ok_or_else(|| ManifestError("placement GPU aggregate upload overflow".into()))?;
        if result.total_upload_bytes > max_total_upload_bytes {
            return Err(ManifestError("placement GPU payload exceeds upload byte budget".into()));
        }
        result.meshes.push(PlacementGpuMeshPayload {
            placement_index: mesh.placement_index,
            virtual_path: mesh.virtual_path.clone(),
            geometry_index: mesh.geometry_index,
            vertex_bytes,
            index_bytes,
            vertex_count: count,
            index_count,
            vertex_stride: 12,
        });
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DecodedVertex, DecodedVertexAttribute, PlacedCpuMesh};

    fn scene() -> PlacementSceneBatch {
        PlacementSceneBatch {
            meshes: vec![PlacedCpuMesh {
                placement_index: 3, virtual_path: "props/test.ydr".into(), geometry_index: 2,
                vertices: vec![
                    DecodedVertex { attributes: vec![DecodedVertexAttribute { semantic: "Position".into(), components: vec![1.0, 2.0, 3.0] }] },
                    DecodedVertex { attributes: vec![DecodedVertexAttribute { semantic: "Position".into(), components: vec![4.0, 5.0, 6.0] }] },
                ],
                indices: Some(vec![0, 1, 0]),
            }],
            placement_count: 1, source_mesh_count: 1,
        }
    }

    #[test]
    fn packs_transformed_positions_and_u32_indices() {
        let payload = build_placement_gpu_payloads(&scene(), 4, 8, 8, 1024).unwrap();
        let mesh = &payload.meshes[0];
        assert_eq!(mesh.vertex_stride, 12);
        assert_eq!(mesh.vertex_count, 2);
        assert_eq!(mesh.index_count, 3);
        assert_eq!(mesh.vertex_bytes.len(), 24);
        assert_eq!(mesh.index_bytes.as_ref().unwrap(), &[0,0,0,0,1,0,0,0,0,0,0,0]);
        assert_eq!(payload.total_upload_bytes, 36);
    }

    #[test]
    fn rejects_missing_positions_and_upload_budget_overflow() {
        let mut input = scene();
        input.meshes[0].vertices[0].attributes.clear();
        assert!(build_placement_gpu_payloads(&input, 4, 8, 8, 1024).is_err());
        assert!(build_placement_gpu_payloads(&scene(), 4, 8, 8, 35).is_err());
    }
}
