//! JSON placement transforms applied to decoded CPU meshes before renderer hand-off.
//!
//! This creates world-space CPU geometry only. It deliberately does not claim
//! to create GTA entities or issue draw calls; those still require the runtime
//! world sink and renderer command integration.
use crate::{
    DecodedVertex, ManifestError, PlacementCpuMeshBatch, ResolvedJsonPlacementPlan,
};

#[derive(Clone, Debug, PartialEq)]
pub struct PlacedCpuMesh {
    pub placement_index: usize,
    pub virtual_path: String,
    pub geometry_index: usize,
    pub vertices: Vec<DecodedVertex>,
    pub indices: Option<Vec<u32>>,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct PlacementSceneBatch {
    pub meshes: Vec<PlacedCpuMesh>,
    pub placement_count: usize,
    pub source_mesh_count: usize,
}

/// Apply JSON scale, X/Y/Z Euler rotation (degrees), and translation to every
/// decoded geometry belonging to each resolved placement. Every placement must
/// have at least one matching decoded mesh; the whole scene fails preflight
/// rather than returning a partially transformed batch.
pub fn build_placement_scene_meshes(
    plan: &ResolvedJsonPlacementPlan,
    cpu_meshes: &PlacementCpuMeshBatch,
    max_placements: usize,
    max_vertices: usize,
    max_output_bytes: usize,
) -> Result<PlacementSceneBatch, ManifestError> {
    if plan.placements.len() > max_placements {
        return Err(ManifestError(format!(
            "placement scene contains {} placements; limit is {}",
            plan.placements.len(), max_placements
        )));
    }
    let mut total_vertices = 0usize;
    let mut output_bytes = 0usize;
    let mut output = PlacementSceneBatch {
        meshes: Vec::with_capacity(cpu_meshes.meshes.len()),
        placement_count: plan.placements.len(),
        source_mesh_count: cpu_meshes.meshes.len(),
    };

    for placement in &plan.placements {
        let transform = &placement.placement;
        if transform.position.iter().chain(transform.rotation.iter()).chain(transform.scale.iter())
            .any(|value| !value.is_finite())
        {
            return Err(ManifestError(format!(
                "placement {} has a non-finite transform", placement.placement_index
            )));
        }
        let radians = transform.rotation.map(f64::to_radians);
        let (sx, cx) = radians[0].sin_cos();
        let (sy, cy) = radians[1].sin_cos();
        let (sz, cz) = radians[2].sin_cos();
        let mut matched = 0usize;

        for source in cpu_meshes.meshes.iter().filter(|mesh| {
            mesh.placement_index == placement.placement_index
                && mesh.virtual_path.eq_ignore_ascii_case(&placement.virtual_path)
        }) {
            matched += 1;
            let next_vertices = total_vertices.checked_add(source.mesh.vertices.len())
                .ok_or_else(|| ManifestError("placement scene vertex count overflow".into()))?;
            let mesh_attribute_bytes = source.mesh.vertices.iter().try_fold(0usize, |total, vertex| {
                vertex.attributes.iter().try_fold(total, |bytes, attribute| {
                    attribute.components.len().checked_mul(std::mem::size_of::<f32>())
                        .and_then(|size| bytes.checked_add(size))
                })
            }).ok_or_else(|| ManifestError("placement scene attribute byte count overflow".into()))?;
            let mesh_index_bytes = source.mesh.indices.as_ref().map_or(Some(0usize), |indices| {
                indices.len().checked_mul(std::mem::size_of::<u32>())
            }).ok_or_else(|| ManifestError("placement scene index byte count overflow".into()))?;
            let mesh_bytes = mesh_attribute_bytes.checked_add(mesh_index_bytes)
                .ok_or_else(|| ManifestError("placement scene mesh byte count overflow".into()))?;
            let next_bytes = output_bytes.checked_add(mesh_bytes)
                .ok_or_else(|| ManifestError("placement scene output byte count overflow".into()))?;
            if next_vertices > max_vertices || next_bytes > max_output_bytes {
                return Err(ManifestError("placement scene exceeds configured geometry limits".into()));
            }
            total_vertices = next_vertices;
            output_bytes = next_bytes;

            let mut vertices = source.mesh.vertices.clone();
            for vertex in &mut vertices {
                let position = vertex.attributes.iter_mut()
                    .find(|attribute| attribute.semantic.eq_ignore_ascii_case("position"))
                    .ok_or_else(|| ManifestError(format!(
                        "{} geometry {} has no Position attribute",
                        source.virtual_path, source.geometry_index
                    )))?;
                if position.components.len() < 3 {
                    return Err(ManifestError("placement Position attribute has fewer than three components".into()));
                }
                let x = f64::from(position.components[0]) * transform.scale[0];
                let y = f64::from(position.components[1]) * transform.scale[1];
                let z = f64::from(position.components[2]) * transform.scale[2];

                // Apply Rx, then Ry, then Rz, followed by world translation.
                let y1 = y * cx - z * sx;
                let z1 = y * sx + z * cx;
                let x2 = x * cy + z1 * sy;
                let z2 = -x * sy + z1 * cy;
                let x3 = x2 * cz - y1 * sz + transform.position[0];
                let y3 = x2 * sz + y1 * cz + transform.position[1];
                let z3 = z2 + transform.position[2];
                let world = [x3, y3, z3];
                if world.iter().any(|value| !value.is_finite() || value.abs() > f64::from(f32::MAX)) {
                    return Err(ManifestError("placement transform produced an invalid world position".into()));
                }
                position.components[0] = x3 as f32;
                position.components[1] = y3 as f32;
                position.components[2] = z3 as f32;
            }

            output.meshes.push(PlacedCpuMesh {
                placement_index: placement.placement_index,
                virtual_path: placement.virtual_path.clone(),
                geometry_index: source.geometry_index,
                vertices,
                indices: source.mesh.indices.clone(),
            });
        }
        if matched == 0 {
            return Err(ManifestError(format!(
                "placement {} has no decoded CPU geometry for {}",
                placement.placement_index, placement.virtual_path
            )));
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DecodedVertexAttribute, InspectedPlacementCpuMesh, JsonPlacement, MountedAsset,
        MountedAssetKind, ResolvedJsonPlacement, GtaCpuMesh,
    };

    fn fixture(position: [f64; 3], rotation: [f64; 3], scale: [f64; 3]) -> (ResolvedJsonPlacementPlan, PlacementCpuMeshBatch) {
        let placement = JsonPlacement {
            asset: "props/test.ydr".into(), position, rotation, scale,
        };
        let plan = ResolvedJsonPlacementPlan { placements: vec![ResolvedJsonPlacement {
            placement_index: 0, placement: placement.clone(),
            virtual_path: "props/test.ydr".into(),
            asset: MountedAsset {
                archive: "mods/test.rpf".into(), entry_index: 1, offset: 512,
                length: 64, kind: MountedAssetKind::Resource,
            },
        }] };
        let mesh = GtaCpuMesh {
            vertices: vec![DecodedVertex { attributes: vec![DecodedVertexAttribute {
                semantic: "Position".into(), components: vec![1.0, 0.0, 0.0],
            }]}],
            indices: None, source_vertex_count: 1, source_index_count: 0,
        };
        let cpu = PlacementCpuMeshBatch {
            meshes: vec![InspectedPlacementCpuMesh {
                placement_index: 0, virtual_path: "props/test.ydr".into(),
                geometry_index: 2, mesh,
            }],
            ignored_non_drawable_assets: 0, geometries_without_vertex_stream: 0,
        };
        (plan, cpu)
    }

    #[test]
    fn applies_scale_rotation_and_translation_to_world_position() {
        let (plan, cpu) = fixture([10.0, 20.0, 30.0], [0.0, 0.0, 90.0], [2.0, 1.0, 1.0]);
        let scene = build_placement_scene_meshes(&plan, &cpu, 4, 8, 4096).unwrap();
        let p = &scene.meshes[0].vertices[0].attributes[0].components;
        assert!((p[0] - 10.0).abs() < 0.0001);
        assert!((p[1] - 22.0).abs() < 0.0001);
        assert!((p[2] - 30.0).abs() < 0.0001);
        assert_eq!(scene.placement_count, 1);
    }

    #[test]
    fn rejects_a_placement_without_decoded_geometry() {
        let (plan, mut cpu) = fixture([0.0; 3], [0.0; 3], [1.0; 3]);
        cpu.meshes.clear();
        assert!(build_placement_scene_meshes(&plan, &cpu, 4, 8, 4096)
            .unwrap_err().0.contains("no decoded CPU geometry"));
    }

    #[test]
    fn rejects_non_finite_transform_values() {
        let (mut plan, cpu) = fixture([0.0; 3], [0.0; 3], [1.0; 3]);
        plan.placements[0].placement.position[0] = f64::NAN;
        assert!(build_placement_scene_meshes(&plan, &cpu, 4, 8, 4096).is_err());
    }
}
