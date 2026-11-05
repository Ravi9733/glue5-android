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
