//! Placement-to-CPU-mesh integration with explicit Rockstar declaration resolution.
//!
//! The current structure inspector does not yet extract declaration flags/types
//! from retail geometry metadata. Callers must provide a verified declaration
//! per geometry; this bridge never guesses one from stride alone.
use crate::{
    assemble_gta_cpu_mesh, InspectedPlacementGeometryBuffers, ManifestError,
    PlacementGeometryBufferBatch, GtaCpuMesh, VertexElement,
};

#[derive(Clone, Debug, PartialEq)]
pub struct InspectedPlacementCpuMesh {
    pub placement_index: usize,
    pub virtual_path: String,
    pub geometry_index: usize,
    pub mesh: GtaCpuMesh,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct PlacementCpuMeshBatch {
    pub meshes: Vec<InspectedPlacementCpuMesh>,
    pub ignored_non_drawable_assets: usize,
    pub geometries_without_vertex_stream: usize,
}

/// Decode every extracted geometry using a caller-supplied, verified layout.
/// The resolver must obtain declarations from a validated GTA parser or trusted
/// metadata. Limits are enforced per mesh; invalid geometry fails the batch.
pub fn assemble_decoded_placement_cpu_meshes<F>(
    buffers: &PlacementGeometryBufferBatch,
    mut resolve_declaration: F,
    max_vertices_per_mesh: u32,
    max_indices_per_mesh: u32,
    max_output_bytes_per_mesh: usize,
) -> Result<PlacementCpuMeshBatch, ManifestError>
where
    F: FnMut(&InspectedPlacementGeometryBuffers) -> Result<Vec<VertexElement>, ManifestError>,
{
    let mut output = PlacementCpuMeshBatch {
        meshes: Vec::with_capacity(buffers.geometries.len()),
        ignored_non_drawable_assets: buffers.ignored_non_drawable_assets,
        geometries_without_vertex_stream: buffers.geometries_without_vertex_stream,
    };
    for geometry in &buffers.geometries {
        let declaration = resolve_declaration(geometry)?;
        if declaration.is_empty() {
            return Err(ManifestError(format!("{} geometry {} has no verified vertex declaration", geometry.virtual_path, geometry.geometry_index)));
        }
        let mesh = assemble_gta_cpu_mesh(&geometry.buffers, &declaration, max_vertices_per_mesh, max_indices_per_mesh, max_output_bytes_per_mesh)?;
        output.meshes.push(InspectedPlacementCpuMesh {
            placement_index: geometry.placement_index,
            virtual_path: geometry.virtual_path.clone(),
            geometry_index: geometry.geometry_index,
            mesh,
        });
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{GtaBufferPage, GtaDrawableBufferData, GtaGeometryBufferData, GtaGeometrySummary, GtaVertexBufferLayout, VertexElementFormat};

    #[test]
    fn connects_extracted_geometry_to_cpu_mesh_with_explicit_layout() {
        let mut vertex_bytes = Vec::new();
        for xyz in [[0.0_f32, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]] {
            for value in xyz { vertex_bytes.extend_from_slice(&value.to_le_bytes()); }
        }
        let geometry = InspectedPlacementGeometryBuffers {
            placement_index: 2, virtual_path: "props/test.ydr".into(), geometry_index: 1,
            geometry: GtaGeometrySummary { virtual_address: 0x5000_0000, vertex_buffer_pointer: 0, index_buffer_pointer: 0, inline_data_pointer: 0, indices_count: 0, triangles_count: 0, vertices_count: 3, vertex_stride: 12 },
            buffers: GtaGeometryBufferData {
                vertex_layout: Some(GtaVertexBufferLayout::LegacyData1), vertex_count: 3, vertex_stride: 12,
                vertex_data: Some(GtaDrawableBufferData { page: GtaBufferPage::Graphics, virtual_address: 0x6000_0000, bytes: vertex_bytes }),
                index_layout: None, index_count: 0, index_element_size: 0, index_data: None,
            },
        };
        let batch = PlacementGeometryBufferBatch { geometries: vec![geometry], ignored_non_drawable_assets: 1, geometries_without_vertex_stream: 0 };
        let result = assemble_decoded_placement_cpu_meshes(&batch, |_| Ok(vec![VertexElement { semantic: "Position".into(), offset: 0, format: VertexElementFormat::Float32x3 }]), 16, 16, 4096).unwrap();
        assert_eq!(result.meshes.len(), 1);
        assert_eq!(result.meshes[0].placement_index, 2);
        assert_eq!(result.meshes[0].mesh.vertices.len(), 3);
    }

    #[test]
    fn rejects_unverified_declaration_for_geometry() {
        let mut vertex_bytes = Vec::new();
        for value in [0.0_f32, 0.0, 0.0] { vertex_bytes.extend_from_slice(&value.to_le_bytes()); }
        let geometry = InspectedPlacementGeometryBuffers {
            placement_index: 0, virtual_path: "props/test.ydr".into(), geometry_index: 0,
            geometry: GtaGeometrySummary { virtual_address: 0x5000_0000, vertex_buffer_pointer: 0, index_buffer_pointer: 0, inline_data_pointer: 0, indices_count: 0, triangles_count: 0, vertices_count: 1, vertex_stride: 12 },
            buffers: GtaGeometryBufferData { vertex_layout: Some(GtaVertexBufferLayout::LegacyData1), vertex_count: 1, vertex_stride: 12, vertex_data: Some(GtaDrawableBufferData { page: GtaBufferPage::Graphics, virtual_address: 0x6000_0000, bytes: vertex_bytes }), index_layout: None, index_count: 0, index_element_size: 0, index_data: None },
        };
        let batch = PlacementGeometryBufferBatch { geometries: vec![geometry], ignored_non_drawable_assets: 0, geometries_without_vertex_stream: 0 };
        assert!(assemble_decoded_placement_cpu_meshes(&batch, |_| Ok(Vec::new()), 8, 8, 128).is_err());
    }
}
