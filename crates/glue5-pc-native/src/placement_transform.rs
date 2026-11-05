//! Deterministic conversion of JSON Euler transforms to renderer-ready quaternions.
//!
//! GTA placement JSON stores rotations in degrees. The GPU renderer consumes
//! normalized XYZW quaternions. Keeping this conversion in the native content
//! crate avoids per-backend differences and rejects invalid numeric input
//! before a placement reaches the renderer.

use crate::JsonPlacement;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RendererTransform {
    pub scale: [f32; 3],
    pub rotation_xyzw: [f32; 4],
    pub translation: [f32; 3],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransformError {
    NonFiniteOrOutOfRange,
    FloatPrecisionOverflow,
    InvalidQuaternion,
}

impl std::fmt::Display for TransformError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::NonFiniteOrOutOfRange => "placement transform contains a non-finite or out-of-range value",
            Self::FloatPrecisionOverflow => "placement transform cannot be represented as finite f32 values",
            Self::InvalidQuaternion => "placement rotation did not produce a valid normalized quaternion",
        })
    }
}
impl std::error::Error for TransformError {}

/// Convert XYZ Euler angles (degrees) to a normalized XYZW quaternion.
///
/// Rotation order is intrinsic XYZ (equivalent to qz * qy * qx for column
/// vectors). Translation and scale retain the JSON placement convention.
pub fn renderer_transform(placement: &JsonPlacement) -> Result<RendererTransform, TransformError> {
    let values = placement.position.into_iter()
        .chain(placement.rotation)
        .chain(placement.scale);
    if values.into_iter().any(|v| !v.is_finite() || v.abs() > 1_000_000.0) {
        return Err(TransformError::NonFiniteOrOutOfRange);
    }

    let half = placement.rotation.map(|degrees| (degrees.to_radians() * 0.5) as f32);
    let (sx, cx) = half[0].sin_cos();
    let (sy, cy) = half[1].sin_cos();
    let (sz, cz) = half[2].sin_cos();

    let mut q = [
        sx * cy * cz - cx * sy * sz,
        cx * sy * cz + sx * cy * sz,
        cx * cy * sz - sx * sy * cz,
        cx * cy * cz + sx * sy * sz,
    ];
    let norm_sq = q.iter().map(|v| v * v).sum::<f32>();
    if !norm_sq.is_finite() || norm_sq <= f32::EPSILON {
        return Err(TransformError::InvalidQuaternion);
    }
    let inv_norm = norm_sq.sqrt().recip();
    for value in &mut q { *value *= inv_norm; }

    let result = RendererTransform {
        scale: placement.scale.map(|v| v as f32),
        rotation_xyzw: q,
        translation: placement.position.map(|v| v as f32),
    };
    if result.scale.into_iter().chain(result.rotation_xyzw).chain(result.translation)
        .any(|v| !v.is_finite()) {
        return Err(TransformError::FloatPrecisionOverflow);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn placement(rotation: [f64; 3]) -> JsonPlacement {
        JsonPlacement {
            asset: "props/test.ydr".into(),
            position: [10.0, 20.0, 30.0],
            rotation,
            scale: [1.0, 2.0, 3.0],
        }
    }

    #[test]
    fn identity_rotation_preserves_position_and_scale() {
        let t = renderer_transform(&placement([0.0; 3])).unwrap();
        assert_eq!(t.translation, [10.0, 20.0, 30.0]);
        assert_eq!(t.scale, [1.0, 2.0, 3.0]);
        assert!((t.rotation_xyzw[3] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn ninety_degree_z_rotation_is_normalized() {
        let t = renderer_transform(&placement([0.0, 0.0, 90.0])).unwrap();
        assert!((t.rotation_xyzw[2] - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-5);
        assert!((t.rotation_xyzw[3] - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-5);
        let norm = t.rotation_xyzw.iter().map(|v| v * v).sum::<f32>();
        assert!((norm - 1.0).abs() < 1e-6);
    }

    #[test]
    fn rejects_non_finite_and_f32_overflow() {
        let mut p = placement([0.0; 3]);
        p.position[0] = f64::NAN;
        assert_eq!(renderer_transform(&p), Err(TransformError::NonFiniteOrOutOfRange));
        p.position[0] = 1e40;
        assert_eq!(renderer_transform(&p), Err(TransformError::NonFiniteOrOutOfRange));
    }
}
