//! Common bounded CPU-side representation for decoded GTA drawable and texture data.
//! This is a handoff format for a future renderer, not a retail asset parser.
use crate::{GtaCpuMesh, ManifestError};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GtaCpuTexture {
    pub name: String,
    pub width: u16,
    pub height: u16,
    /// Top-mip pixels in tightly packed RGBA8 order.
    pub rgba8: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GtaCpuAsset {
    pub name: String,
    pub mesh: GtaCpuMesh,
    pub textures: Vec<GtaCpuTexture>,
    pub accounted_bytes: usize,
}

/// Combine an already-decoded mesh and top-mip textures under explicit resource caps.
/// Input texture bytes must already be decoded to tightly packed RGBA8.
pub fn assemble_gta_cpu_asset(
    name: String,
    mesh: GtaCpuMesh,
    textures: Vec<GtaCpuTexture>,
    max_textures: usize,
    max_total_bytes: usize,
) -> Result<GtaCpuAsset, ManifestError> {
    if name.trim().is_empty() || name.len() > 256 {
        return Err(ManifestError("asset name must contain 1..=256 UTF-8 bytes".into()));
    }
    if textures.len() > max_textures {
        return Err(ManifestError("asset texture count exceeds configured limit".into()));
    }

    let mut total = 0usize;
    for vertex in &mesh.vertices {
        total = total.checked_add(vertex.attributes.iter().try_fold(0usize, |sum, attr| {
            sum.checked_add(attr.semantic.len())
                .and_then(|n| n.checked_add(attr.components.len().checked_mul(std::mem::size_of::<f32>())?))
        }).ok_or_else(|| ManifestError("mesh vertex byte accounting overflow".into()))?)
            .ok_or_else(|| ManifestError("asset byte accounting overflow".into()))?;
    }
    if let Some(indices) = &mesh.indices {
        total = total.checked_add(indices.len().checked_mul(std::mem::size_of::<u32>())
            .ok_or_else(|| ManifestError("mesh index byte accounting overflow".into()))?)
            .ok_or_else(|| ManifestError("asset byte accounting overflow".into()))?;
    }

    let mut seen = std::collections::HashSet::new();
    for texture in &textures {
        if texture.name.trim().is_empty() || texture.name.len() > 256 {
            return Err(ManifestError("texture name must contain 1..=256 UTF-8 bytes".into()));
        }
        if !seen.insert(texture.name.to_ascii_lowercase()) {
            return Err(ManifestError(format!("duplicate texture name: {}", texture.name)));
        }
        if texture.width == 0 || texture.height == 0 {
            return Err(ManifestError(format!("texture {} has zero dimensions", texture.name)));
        }
        let expected = usize::from(texture.width).checked_mul(usize::from(texture.height))
            .and_then(|pixels| pixels.checked_mul(4))
            .ok_or_else(|| ManifestError("texture RGBA byte count overflow".into()))?;
        if texture.rgba8.len() != expected {
            return Err(ManifestError(format!("texture {} RGBA byte length mismatch", texture.name)));
        }
        total = total.checked_add(texture.rgba8.len())
            .ok_or_else(|| ManifestError("asset byte accounting overflow".into()))?;
        if total > max_total_bytes {
            return Err(ManifestError("combined CPU asset exceeds configured byte limit".into()));
        }
    }
    if total > max_total_bytes {
        return Err(ManifestError("combined CPU asset exceeds configured byte limit".into()));
    }

    Ok(GtaCpuAsset { name, mesh, textures, accounted_bytes: total })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DecodedVertex;

    fn mesh() -> GtaCpuMesh {
        GtaCpuMesh {
            vertices: vec![DecodedVertex { attributes: vec![] }],
            indices: Some(vec![0, 0, 0]),
            source_vertex_count: 1,
            source_index_count: 3,
        }
    }

    #[test]
    fn combines_mesh_and_rgba_texture_with_byte_accounting() {
        let texture = GtaCpuTexture { name: "diffuse".into(), width: 1, height: 1, rgba8: vec![255, 0, 0, 255] };
        let asset = assemble_gta_cpu_asset("prop_test".into(), mesh(), vec![texture], 4, 1024).unwrap();
        assert_eq!(asset.textures.len(), 1);
        assert_eq!(asset.accounted_bytes, 16);
    }

    #[test]
    fn rejects_bad_texture_length_duplicates_and_budget_overflow() {
        let bad = GtaCpuTexture { name: "diffuse".into(), width: 1, height: 1, rgba8: vec![0; 3] };
        assert!(assemble_gta_cpu_asset("prop".into(), mesh(), vec![bad], 4, 1024).is_err());
        let a = GtaCpuTexture { name: "Diffuse".into(), width: 1, height: 1, rgba8: vec![0; 4] };
        let b = GtaCpuTexture { name: "diffuse".into(), width: 1, height: 1, rgba8: vec![0; 4] };
        assert!(assemble_gta_cpu_asset("prop".into(), mesh(), vec![a.clone(), b], 4, 1024).is_err());
        assert!(assemble_gta_cpu_asset("prop".into(), mesh(), vec![a], 4, 15).is_err());
    }
}
