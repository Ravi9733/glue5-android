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
    mesh: &crate::ExternalMeshBuffers,
    material_key: impl Into<String>,
) -> Result<ExternalPlacementDraw, PlacementDrawError> {
    let transform = glue5_pc_native::renderer_transform(placement)
        .map_err(|_| PlacementDrawError::InvalidTransform)?;
    let packet = ExternalPlacementDraw {
        resource_id,
        vertex_count: mesh.vertex_count,
        index_count: mesh.index_count,
        vertex_stride: mesh.vertex_stride,
        transform: PlacementTransform {
            scale: transform.scale,
            rotation_xyzw: transform.rotation_xyzw,
            translation: transform.translation,
        },
        material_key: material_key.into(),
    };
    packet.validate(mesh.index_count)?;
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
        let mesh = crate::ExternalMeshBuffers {
            vertex: wgpu::Buffer::dummy(),
            index: None,
            vertex_count: 3,
            index_count: 0,
            vertex_stride: 12,
        };
        let packet = packet_from_json_placement(9, &placement, &mesh, "fallback/unlit");
        assert!(matches!(packet, Err(PlacementDrawError::EmptyMesh)));
    }
}
