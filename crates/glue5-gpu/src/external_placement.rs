//! Validated CPU-side draw packets for externally supplied PC placements.
//!
//! This is the renderer integration boundary only. It deliberately does not
//! claim to issue GPU draw calls: packets still need to be consumed by the GPU
//! command thread and bound to a renderer-owned pipeline/material registry.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlacementTransform {
    pub scale: [f32; 3],
    pub rotation_xyzw: [f32; 4],
    pub translation: [f32; 3],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlacementDrawError {
    EmptyMesh,
    InvalidTransform,
    InvalidMaterialKey,
    IndexCountExceedsBuffer,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ExternalPlacementDraw {
    pub resource_id: u64,
    pub vertex_count: u32,
    pub index_count: u32,
    pub vertex_stride: u32,
    pub transform: PlacementTransform,
    /// Stable material lookup key; actual material resolution remains in the renderer.
    pub material_key: String,
}

impl ExternalPlacementDraw {
    pub fn validate(&self, available_index_count: u32) -> Result<(), PlacementDrawError> {
        if self.vertex_count == 0 || self.vertex_stride == 0 {
            return Err(PlacementDrawError::EmptyMesh);
        }
        if self.index_count > available_index_count {
            return Err(PlacementDrawError::IndexCountExceedsBuffer);
        }
        if self.material_key.trim().is_empty() {
            return Err(PlacementDrawError::InvalidMaterialKey);
        }
        let values = self.transform.scale.into_iter()
            .chain(self.transform.rotation_xyzw)
            .chain(self.transform.translation);
        if values.into_iter().any(|value| !value.is_finite()) {
            return Err(PlacementDrawError::InvalidTransform);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn packet() -> ExternalPlacementDraw {
        ExternalPlacementDraw {
            resource_id: 7,
            vertex_count: 3,
            index_count: 3,
            vertex_stride: 12,
            transform: PlacementTransform {
                scale: [1.0; 3],
                rotation_xyzw: [0.0, 0.0, 0.0, 1.0],
                translation: [0.0; 3],
            },
            material_key: "fallback/unlit".into(),
        }
    }

    #[test]
    fn accepts_bounded_finite_placement_draw_packet() {
        assert_eq!(packet().validate(3), Ok(()));
    }

    #[test]
    fn rejects_non_finite_transform_and_index_overrun() {
        let mut bad_transform = packet();
        bad_transform.transform.translation[0] = f32::NAN;
        assert_eq!(bad_transform.validate(3), Err(PlacementDrawError::InvalidTransform));
        assert_eq!(packet().validate(2), Err(PlacementDrawError::IndexCountExceedsBuffer));
    }
}


/// Build a renderer packet from a validated JSON placement and an uploaded mesh.
///
/// This is the explicit bridge between the PC-native transform parser and the
/// GPU upload API. It does not submit a draw: the render command thread must
/// still resolve the material key and record the packet in a render pass.
pub fn packet_from_json_placement(
    resource_id: u64,
    placement: &glue5_pc_native::JsonPlacement,
    vertex_count: u32,
    index_count: u32,
    vertex_stride: u32,
    material_key: impl Into<String>,
) -> Result<ExternalPlacementDraw, PlacementDrawError> {
    let transform = glue5_pc_native::renderer_transform(placement)
        .map_err(|_| PlacementDrawError::InvalidTransform)?;
    let packet = ExternalPlacementDraw {
        resource_id,
        vertex_count,
        index_count,
        vertex_stride,
        transform: PlacementTransform {
            scale: transform.scale,
            rotation_xyzw: transform.rotation_xyzw,
            translation: transform.translation,
        },
        material_key: material_key.into(),
    };
    packet.validate(index_count)?;
    Ok(packet)
}


#[cfg(test)]
mod bridge_tests {
    use super::*;

    #[test]
    fn converts_json_placement_into_renderer_packet() {
        let placement = glue5_pc_native::JsonPlacement {
            asset: "props/test.ydr".into(),
            position: [4.0, 5.0, 6.0],
            rotation: [0.0, 0.0, 90.0],
            scale: [1.0, 1.0, 1.0],
        };
        let packet = packet_from_json_placement(9, &placement, 3, 3, 12, "fallback/unlit").unwrap();
        assert_eq!(packet.resource_id, 9);
        assert_eq!(packet.vertex_count, 3);
        assert_eq!(packet.transform.translation, [4.0, 5.0, 6.0]);
        assert!((packet.transform.rotation_xyzw[2] - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-5);
    }

    #[test]
    fn rejects_empty_mesh_before_renderer_handoff() {
        let placement = glue5_pc_native::JsonPlacement {
            asset: "props/test.ydr".into(),
            position: [0.0; 3],
            rotation: [0.0; 3],
            scale: [1.0; 3],
        };
        assert_eq!(
            packet_from_json_placement(1, &placement, 0, 0, 12, "fallback/unlit"),
            Err(PlacementDrawError::EmptyMesh)
        );
    }
}


// Batch preparation is deliberately atomic: every placement is converted and
// validated before the caller uploads any mesh or mutates the world.
#[derive(Clone, Debug)]
pub struct JsonPlacementMesh<'a> {
    pub resource_id: u64,
    pub placement: &'a glue5_pc_native::JsonPlacement,
    pub vertex_count: u32,
    pub index_count: u32,
    pub vertex_stride: u32,
    pub material_key: &'a str,
}

/// Convert a complete JSON placement document into renderer packets.
/// No GPU allocation or world mutation occurs here; callers can safely proceed
/// only after this function returns the entire ordered batch.
pub fn packets_from_json_placements(
    document: &glue5_pc_native::JsonPlacementDocument,
    meshes: &[JsonPlacementMesh<'_>],
) -> Result<Vec<ExternalPlacementDraw>, (usize, PlacementDrawError)> {
    if document.placements.len() != meshes.len() {
        return Err((document.placements.len().min(meshes.len()), PlacementDrawError::EmptyMesh));
    }
    let mut packets = Vec::with_capacity(meshes.len());
    for (index, mesh) in meshes.iter().enumerate() {
        if !std::ptr::eq(mesh.placement, &document.placements[index]) {
            return Err((index, PlacementDrawError::InvalidTransform));
        }
        let packet = packet_from_json_placement(
            mesh.resource_id,
            mesh.placement,
            mesh.vertex_count,
            mesh.index_count,
            mesh.vertex_stride,
            mesh.material_key,
        ).map_err(|error| (index, error))?;
        packets.push(packet);
    }
    Ok(packets)
}

#[cfg(test)]
mod atomic_batch_tests {
    use super::*;

    #[test]
    fn prepares_complete_ordered_json_batch() {
        let document = glue5_pc_native::JsonPlacementDocument {
            version: 1,
            placements: vec![
                glue5_pc_native::JsonPlacement { asset: "a.ydr".into(), position: [1.0, 2.0, 3.0], rotation: [0.0; 3], scale: [1.0; 3] },
                glue5_pc_native::JsonPlacement { asset: "b.ydr".into(), position: [4.0, 5.0, 6.0], rotation: [0.0; 3], scale: [1.0; 3] },
            ],
        };
        let meshes = vec![
            JsonPlacementMesh { resource_id: 11, placement: &document.placements[0], vertex_count: 3, index_count: 3, vertex_stride: 12, material_key: "fallback/unlit" },
            JsonPlacementMesh { resource_id: 12, placement: &document.placements[1], vertex_count: 4, index_count: 6, vertex_stride: 12, material_key: "fallback/unlit" },
        ];
        let packets = packets_from_json_placements(&document, &meshes).unwrap();
        assert_eq!(packets.len(), 2);
        assert_eq!(packets[0].resource_id, 11);
        assert_eq!(packets[1].resource_id, 12);
        assert_eq!(packets[1].transform.translation, [4.0, 5.0, 6.0]);
    }

    #[test]
    fn rejects_invalid_later_item_without_returning_partial_batch() {
        let document = glue5_pc_native::JsonPlacementDocument {
            version: 1,
            placements: vec![
                glue5_pc_native::JsonPlacement { asset: "a.ydr".into(), position: [1.0, 2.0, 3.0], rotation: [0.0; 3], scale: [1.0; 3] },
                glue5_pc_native::JsonPlacement { asset: "b.ydr".into(), position: [4.0, 5.0, 6.0], rotation: [0.0; 3], scale: [1.0; 3] },
            ],
        };
        let meshes = vec![
            JsonPlacementMesh { resource_id: 11, placement: &document.placements[0], vertex_count: 3, index_count: 3, vertex_stride: 12, material_key: "fallback/unlit" },
            JsonPlacementMesh { resource_id: 12, placement: &document.placements[1], vertex_count: 0, index_count: 0, vertex_stride: 12, material_key: "fallback/unlit" },
        ];
        assert_eq!(packets_from_json_placements(&document, &meshes), Err((1, PlacementDrawError::EmptyMesh)));
    }
}


/// Validate uploaded mesh byte streams against a placement packet before the
/// renderer records a draw. Index data is interpreted as little-endian u32.
pub fn validate_external_mesh_bytes(
    packet: &ExternalPlacementDraw,
    vertex_bytes: &[u8],
    index_bytes: Option<&[u8]>,
) -> Result<(), PlacementDrawError> {
    packet.validate(index_bytes.map_or(0, |bytes| (bytes.len() / 4) as u32))?;
    let expected_vertex_bytes = (packet.vertex_count as usize)
        .checked_mul(packet.vertex_stride as usize)
        .ok_or(PlacementDrawError::EmptyMesh)?;
    if vertex_bytes.len() != expected_vertex_bytes || vertex_bytes.is_empty() {
        return Err(PlacementDrawError::EmptyMesh);
    }
    match (packet.index_count, index_bytes) {
        (0, None) => Ok(()),
        (0, Some(bytes)) if bytes.is_empty() => Ok(()),
        (count, Some(bytes)) if count > 0 && bytes.len() == (count as usize).saturating_mul(4) => {
            for (index, chunk) in bytes.chunks_exact(4).enumerate() {
                let value = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                if value >= packet.vertex_count {
                    return Err(PlacementDrawError::IndexCountExceedsBuffer);
                }
                let _ = index;
            }
            Ok(())
        }
        _ => Err(PlacementDrawError::IndexCountExceedsBuffer),
    }
}

#[cfg(test)]
mod mesh_byte_validation_tests {
    use super::*;

    fn packet() -> ExternalPlacementDraw {
        ExternalPlacementDraw {
            resource_id: 1,
            vertex_count: 3,
            index_count: 3,
            vertex_stride: 4,
            transform: PlacementTransform {
                scale: [1.0; 3], rotation_xyzw: [0.0, 0.0, 0.0, 1.0],
                translation: [0.0; 3],
            },
            material_key: "fallback/unlit".into(),
        }
    }

    #[test]
    fn accepts_exact_vertex_and_valid_index_streams() {
        let vertices = [0u8; 12];
        let indices = [0u8, 0, 0, 0, 1, 0, 0, 0, 2, 0, 0, 0];
        assert_eq!(validate_external_mesh_bytes(&packet(), &vertices, Some(&indices)), Ok(()));
    }

    #[test]
    fn rejects_vertex_size_mismatch_and_out_of_range_indices() {
        let short_vertices = [0u8; 11];
        let valid_indices = [0u8, 0, 0, 0, 1, 0, 0, 0, 2, 0, 0, 0];
        assert_eq!(validate_external_mesh_bytes(&packet(), &short_vertices, Some(&valid_indices)), Err(PlacementDrawError::EmptyMesh));
        let bad_indices = [0u8, 0, 0, 0, 1, 0, 0, 0, 3, 0, 0, 0];
        assert_eq!(validate_external_mesh_bytes(&packet(), &[0u8; 12], Some(&bad_indices)), Err(PlacementDrawError::IndexCountExceedsBuffer));
    }
}
