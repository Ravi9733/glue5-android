//! Bounded CPU-side vertex/index decoding for inspected GTA drawable buffers.
//!
//! This module consumes an explicit vertex layout supplied by a format-specific
//! declaration parser. It does not claim to parse Rockstar vertex declarations
//! or material/shader state itself.

use crate::ManifestError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VertexElementFormat {
    Float32x2,
    Float32x3,
    Float32x4,
    Unorm8x4,
    Snorm8x4,
    Uint16x2,
    Uint16x4,
}

impl VertexElementFormat {
    fn byte_len(self) -> usize {
        match self {
            Self::Float32x2 => 8,
            Self::Float32x3 => 12,
            Self::Float32x4 => 16,
            Self::Unorm8x4 | Self::Snorm8x4 => 4,
            Self::Uint16x2 => 4,
            Self::Uint16x4 => 8,
        }
    }

    fn component_count(self) -> usize {
        match self {
            Self::Float32x2 | Self::Uint16x2 => 2,
            Self::Float32x3 => 3,
            Self::Float32x4 | Self::Unorm8x4 | Self::Snorm8x4 | Self::Uint16x4 => 4,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VertexElement {
    pub semantic: String,
    pub offset: u16,
    pub format: VertexElementFormat,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DecodedVertexAttribute {
    pub semantic: String,
    pub components: Vec<f32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DecodedVertex {
    pub attributes: Vec<DecodedVertexAttribute>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IndexElementFormat {
    U16,
    U32,
}

/// Decode interleaved vertex bytes using an explicit, pre-validated element
/// layout. All offsets and reads are checked against the supplied stride.
pub fn decode_interleaved_vertices(
    bytes: &[u8],
    vertex_count: u32,
    stride: u16,
    elements: &[VertexElement],
    max_vertices: u32,
    max_output_bytes: usize,
) -> Result<Vec<DecodedVertex>, ManifestError> {
    if vertex_count == 0 || vertex_count > max_vertices || stride == 0 || elements.is_empty() {
        return Err(ManifestError("vertex decode count, stride, or layout is invalid".into()));
    }
    let expected = (vertex_count as usize).checked_mul(stride as usize)
        .ok_or_else(|| ManifestError("vertex input length overflow".into()))?;
    if expected > max_output_bytes || bytes.len() != expected {
        return Err(ManifestError("vertex input length does not match bounded count and stride".into()));
    }
    for element in elements {
        let end = usize::from(element.offset).checked_add(element.format.byte_len())
            .ok_or_else(|| ManifestError("vertex element range overflow".into()))?;
        if end > usize::from(stride) || element.semantic.is_empty() {
            return Err(ManifestError("vertex element exceeds stride or has empty semantic".into()));
        }
    }

    let mut decoded = Vec::with_capacity(vertex_count as usize);
    for vertex_bytes in bytes.chunks_exact(stride as usize) {
        let mut attributes = Vec::with_capacity(elements.len());
        for element in elements {
            let start = usize::from(element.offset);
            let raw = &vertex_bytes[start..start + element.format.byte_len()];
            let components: Vec<f32> = match element.format {
                VertexElementFormat::Float32x2 | VertexElementFormat::Float32x3 | VertexElementFormat::Float32x4 => {
                    raw.chunks_exact(4).map(|v| f32::from_le_bytes(v.try_into().unwrap())).collect()
                }
                VertexElementFormat::Unorm8x4 => raw.iter().map(|v| f32::from(*v) / 255.0).collect(),
                VertexElementFormat::Snorm8x4 => raw.iter().map(|v| ( (*v as i8) as f32 / 127.0).max(-1.0)).collect(),
                VertexElementFormat::Uint16x2 | VertexElementFormat::Uint16x4 => {
                    raw.chunks_exact(2).map(|v| u16::from_le_bytes(v.try_into().unwrap()) as f32).collect()
                }
            };
            debug_assert_eq!(components.len(), element.format.component_count());
            if components.iter().any(|v| !v.is_finite()) {
                return Err(ManifestError("vertex attribute contains a non-finite float".into()));
            }
            attributes.push(DecodedVertexAttribute { semantic: element.semantic.clone(), components });
        }
        decoded.push(DecodedVertex { attributes });
    }
    Ok(decoded)
}

/// Convert a bounded little-endian index stream to u32 indices and ensure every
/// index addresses an existing vertex.
pub fn decode_drawable_indices(
    bytes: &[u8],
    format: IndexElementFormat,
    index_count: u32,
    vertex_count: u32,
    max_indices: u32,
) -> Result<Vec<u32>, ManifestError> {
    if index_count == 0 || index_count > max_indices || vertex_count == 0 {
        return Err(ManifestError("index or vertex count is invalid".into()));
    }
    let width = match format { IndexElementFormat::U16 => 2, IndexElementFormat::U32 => 4 };
    let expected = (index_count as usize).checked_mul(width)
        .ok_or_else(|| ManifestError("index stream length overflow".into()))?;
    if bytes.len() != expected {
        return Err(ManifestError("index stream length does not match declared count".into()));
    }
    let mut indices = Vec::with_capacity(index_count as usize);
    for raw in bytes.chunks_exact(width) {
        let index = if width == 2 {
            u16::from_le_bytes(raw.try_into().unwrap()) as u32
        } else {
            u32::from_le_bytes(raw.try_into().unwrap())
        };
        if index >= vertex_count {
            return Err(ManifestError("index references a vertex outside the vertex buffer".into()));
        }
        indices.push(index);
    }
    Ok(indices)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_float_position_and_normalized_color() {
        let mut bytes = Vec::new();
        for value in [1.0_f32, 2.0, 3.0] { bytes.extend_from_slice(&value.to_le_bytes()); }
        bytes.extend_from_slice(&[0, 127, 255, 64]);
        let vertices = decode_interleaved_vertices(&bytes, 1, 16, &[
            VertexElement { semantic: "POSITION".into(), offset: 0, format: VertexElementFormat::Float32x3 },
            VertexElement { semantic: "COLOR".into(), offset: 12, format: VertexElementFormat::Unorm8x4 },
        ], 4, 64).unwrap();
        assert_eq!(vertices[0].attributes[0].components, vec![1.0, 2.0, 3.0]);
        assert_eq!(vertices[0].attributes[1].components[0], 0.0);
        assert!((vertices[0].attributes[1].components[2] - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn rejects_bad_layout_non_finite_values_and_out_of_range_indices() {
        assert!(decode_interleaved_vertices(&[0; 4], 1, 4, &[
            VertexElement { semantic: "POSITION".into(), offset: 0, format: VertexElementFormat::Float32x3 },
        ], 4, 64).is_err());
        let mut non_finite = f32::NAN.to_le_bytes().to_vec();
        non_finite.extend_from_slice(&0.0_f32.to_le_bytes());
        assert!(decode_interleaved_vertices(&non_finite, 1, 8, &[
            VertexElement { semantic: "VALUE".into(), offset: 0, format: VertexElementFormat::Float32x2 },
        ], 4, 64).is_err());
        assert!(decode_drawable_indices(&[2, 0], IndexElementFormat::U16, 1, 2, 8).is_err());
    }

    #[test]
    fn decodes_u16_and_u32_index_streams() {
        assert_eq!(decode_drawable_indices(&[0, 0, 1, 0, 2, 0], IndexElementFormat::U16, 3, 3, 8).unwrap(), vec![0, 1, 2]);
        assert_eq!(decode_drawable_indices(&[0, 0, 0, 0], IndexElementFormat::U32, 1, 1, 8).unwrap(), vec![0]);
    }
}
