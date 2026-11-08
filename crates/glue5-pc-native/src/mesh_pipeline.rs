//! Bounded CPU mesh assembly from already-inspected drawable buffer streams.
//! This is an integration seam, not a complete GTA renderer or retail layout validator.
use crate::{
    decode_drawable_indices, decode_interleaved_vertices, DecodedVertex,
    GtaGeometryBufferData, GtaIndexBufferLayout, IndexElementFormat, ManifestError,
    VertexElement,
};

#[derive(Clone, Debug, PartialEq)]
pub struct GtaCpuMesh {
    pub vertices: Vec<DecodedVertex>,
    pub indices: Option<Vec<u32>>,
    pub source_vertex_count: u32,
    pub source_index_count: u32,
}

/// Assemble decoded vertices and optional validated indices from bounded raw streams.
pub fn assemble_gta_cpu_mesh(
    buffers: &GtaGeometryBufferData,
    declaration: &[VertexElement],
    max_vertices: u32,
    max_indices: u32,
    max_output_bytes: usize,
) -> Result<GtaCpuMesh, ManifestError> {
    let vertex_data = buffers.vertex_data.as_ref()
        .ok_or_else(|| ManifestError("geometry has no readable vertex stream".into()))?;
    let vertices = decode_interleaved_vertices(
        &vertex_data.bytes, buffers.vertex_count, buffers.vertex_stride,
        declaration, max_vertices, max_output_bytes,
    )?;
    let indices = match buffers.index_data.as_ref() {
        None => None,
        Some(data) => {
            let format = match (buffers.index_layout, buffers.index_element_size) {
                (Some(GtaIndexBufferLayout::Legacy16 | GtaIndexBufferLayout::Gen9_16), 2) => IndexElementFormat::U16,
                (Some(GtaIndexBufferLayout::Gen9_32), 4) => IndexElementFormat::U32,
                _ => return Err(ManifestError("index layout and element size are inconsistent".into())),
            };
            if data.bytes.len() > max_output_bytes {
                return Err(ManifestError("index stream exceeds configured output byte limit".into()));
            }
            Some(decode_drawable_indices(
                &data.bytes, format, buffers.index_count, buffers.vertex_count, max_indices,
            )?)
        }
    };
    Ok(GtaCpuMesh {
        vertices, indices,
        source_vertex_count: buffers.vertex_count,
        source_index_count: buffers.index_count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{GtaBufferPage, GtaDrawableBufferData, GtaVertexBufferLayout, VertexElementFormat};

    #[test]
    fn assembles_cpu_mesh_from_position_and_index_streams() {
        let mut vertex_bytes = Vec::new();
        for xyz in [[0.0_f32,0.0,0.0],[1.0,0.0,0.0],[0.0,1.0,0.0]] {
            for value in xyz { vertex_bytes.extend_from_slice(&value.to_le_bytes()); }
        }
        let buffers = GtaGeometryBufferData {
            vertex_layout: Some(GtaVertexBufferLayout::LegacyData1), vertex_count: 3, vertex_stride: 12,
            vertex_data: Some(GtaDrawableBufferData { page: GtaBufferPage::Graphics, virtual_address: 0x6000_0000, bytes: vertex_bytes }),
            index_layout: Some(GtaIndexBufferLayout::Legacy16), index_count: 3, index_element_size: 2,
            index_data: Some(GtaDrawableBufferData { page: GtaBufferPage::Graphics, virtual_address: 0x6000_0040, bytes: vec![0,0,1,0,2,0] }),
        };
        let declaration = [VertexElement { semantic: "Position".into(), offset: 0, format: VertexElementFormat::Float32x3 }];
        let mesh = assemble_gta_cpu_mesh(&buffers, &declaration, 8, 8, 1024).unwrap();
        assert_eq!(mesh.vertices.len(), 3);
        assert_eq!(mesh.vertices[1].attributes[0].components, vec![1.0,0.0,0.0]);
        assert_eq!(mesh.indices, Some(vec![0,1,2]));
    }

    #[test]
    fn rejects_missing_vertices_and_inconsistent_index_metadata() {
        let declaration = [VertexElement { semantic: "Position".into(), offset: 0, format: VertexElementFormat::Float32x3 }];
        let mut buffers = GtaGeometryBufferData {
            vertex_layout: None, vertex_count: 0, vertex_stride: 0, vertex_data: None,
            index_layout: None, index_count: 0, index_element_size: 0, index_data: None,
        };
        assert!(assemble_gta_cpu_mesh(&buffers, &declaration, 8, 8, 1024).is_err());
        buffers.vertex_count = 1; buffers.vertex_stride = 12;
        buffers.vertex_data = Some(GtaDrawableBufferData { page: GtaBufferPage::System, virtual_address: 0x5000_0000, bytes: vec![0;12] });
        buffers.index_count = 1; buffers.index_element_size = 2;
        buffers.index_data = Some(GtaDrawableBufferData { page: GtaBufferPage::System, virtual_address: 0x5000_0010, bytes: vec![0,0] });
        assert!(assemble_gta_cpu_mesh(&buffers, &declaration, 8, 8, 1024).is_err());
    }
}
