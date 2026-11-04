//! End-to-end preparation pipeline for mounted JSON placement content.
//!
//! This joins the existing archive/placement/resource/geometry stages into one
//! bounded operation. It prepares GPU mesh payloads, but deliberately does not
//! issue draw calls, resolve game materials, or spawn engine entities.
use crate::{
    add_rpf7_inventory_to_mount_at, assemble_decoded_placement_cpu_meshes,
    build_placement_gpu_payloads, build_placement_scene_meshes,
    build_resolved_json_placement_plan, decode_resolved_placement_payloads,
    inspect_decoded_placement_gta_assets, read_decoded_placement_geometry_buffers,
    read_resolved_json_placement_payloads, AssetMountIndex, InspectedPlacementGeometryBuffers,
    DecodedPlacementAssetBatch, DecodedPlacementContent, JsonPlacementDocument,
    ManifestError, PlacementGpuPayloadBatch, VertexElement,
};
use std::io::{Read, Seek, SeekFrom};

#[derive(Clone, Debug)]
pub struct PlacementPipelineLimits {
    pub max_placements: usize,
    pub max_asset_bytes: u64,
    pub max_total_source_bytes: u64,
    pub max_resource_output_bytes: usize,
    pub max_total_decoded_bytes: u64,
    pub max_vertices_per_geometry: u32,
    pub max_indices_per_geometry: u32,
    pub max_geometry_bytes: usize,
    pub max_total_geometry_bytes: usize,
    pub max_total_scene_vertices: usize,
    pub max_total_scene_bytes: usize,
    pub max_gpu_meshes: usize,
    pub max_gpu_vertices: usize,
    pub max_gpu_indices: usize,
    pub max_gpu_upload_bytes: usize,
}

impl Default for PlacementPipelineLimits {
    fn default() -> Self {
        Self {
            max_placements: 4096,
            max_asset_bytes: 64 * 1024 * 1024,
            max_total_source_bytes: 512 * 1024 * 1024,
            max_resource_output_bytes: 128 * 1024 * 1024,
            max_total_decoded_bytes: 1024 * 1024 * 1024,
            max_vertices_per_geometry: 2_000_000,
            max_indices_per_geometry: 6_000_000,
            max_geometry_bytes: 128 * 1024 * 1024,
            max_total_geometry_bytes: 512 * 1024 * 1024,
            max_total_scene_vertices: 8_000_000,
            max_total_scene_bytes: 1024 * 1024 * 1024,
            max_gpu_meshes: 16384,
            max_gpu_vertices: 8_000_000,
            max_gpu_indices: 24_000_000,
            max_gpu_upload_bytes: 1024 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Debug)]
pub struct PreparedPlacementPipeline {
    pub mount_index: AssetMountIndex,
    pub gpu_payloads: PlacementGpuPayloadBatch,
    pub placement_count: usize,
    pub inspected_gta_assets: usize,
    pub decoded_geometries: usize,
}

/// Build a mount index from an RPF7 inventory, then resolve, read, decode,
/// inspect, assemble, transform and pack JSON placement assets.
/// The declaration resolver must return a verified retail vertex layout for
/// each geometry; this function never guesses a layout from stride alone.
pub fn prepare_rpf7_json_placement_pipeline<R, F>(
    reader: &mut R,
    archive_name: &str,
    virtual_prefix: &str,
    archive_base: u64,
    inventory: &crate::Rpf7Inventory,
    placements: &JsonPlacementDocument,
    limits: &PlacementPipelineLimits,
    mut resolve_declaration: F,
) -> Result<PreparedPlacementPipeline, ManifestError>
where
    R: Read + Seek,
    F: FnMut(&InspectedPlacementGeometryBuffers) -> Result<Vec<VertexElement>, ManifestError>,
{
    let backing_len = reader.seek(SeekFrom::End(0))
        .map_err(|e| ManifestError(format!("cannot seek to RPF7 archive end: {e}")))?;
    let archive_len = backing_len.checked_sub(archive_base)
        .ok_or_else(|| ManifestError("RPF7 archive base is beyond backing file".into()))?;
    let mut mount_index = AssetMountIndex::new();
    add_rpf7_inventory_to_mount_at(&mut mount_index, archive_name, virtual_prefix, archive_base, archive_len, inventory)
        .map_err(|e| ManifestError(format!("RPF7 mount indexing failed: {}", e.0)))?;

    let plan = build_resolved_json_placement_plan(
        placements, &mount_index, limits.max_placements,
    )?;
    let source = read_resolved_json_placement_payloads(
        reader, &plan, limits.max_asset_bytes, limits.max_total_source_bytes,
    )?;
    let decoded = decode_resolved_placement_payloads(
        &source, limits.max_resource_output_bytes, limits.max_total_decoded_bytes,
    )?;
    // Drawable geometry preparation is independent of the experimental YTD
    // parser. Keep texture dictionaries out of this geometry-only preflight so
    // an unsupported texture layout cannot prevent JSON mesh preparation.
    let drawable_assets = decoded.assets.iter().filter(|asset| {
        let path = asset.virtual_path.to_ascii_lowercase();
        path.ends_with(".ydr") || path.ends_with(".ydd")
    }).cloned().collect::<Vec<_>>();
    let drawable_batch = DecodedPlacementAssetBatch {
        assets: drawable_assets,
        total_decoded_bytes: decoded.total_decoded_bytes,
    };
    let inspected = inspect_decoded_placement_gta_assets(&drawable_batch)?;
    let geometry = read_decoded_placement_geometry_buffers(
        &drawable_batch,
        limits.max_vertices_per_geometry,
        limits.max_indices_per_geometry,
        limits.max_geometry_bytes,
        limits.max_total_geometry_bytes,
    )?;
    let cpu_meshes = assemble_decoded_placement_cpu_meshes(
        &geometry,
        |item| resolve_declaration(item),
        limits.max_vertices_per_geometry,
        limits.max_indices_per_geometry,
        limits.max_geometry_bytes,
    )?;
    let scene = build_placement_scene_meshes(
        &plan,
        &cpu_meshes,
        limits.max_placements,
        limits.max_total_scene_vertices,
        limits.max_total_scene_bytes,
    )?;
    let gpu_payloads = build_placement_gpu_payloads(
        &scene,
        limits.max_gpu_meshes,
        limits.max_gpu_vertices,
        limits.max_gpu_indices,
        limits.max_gpu_upload_bytes,
    )?;

    Ok(PreparedPlacementPipeline {
        mount_index,
        gpu_payloads,
        placement_count: plan.placements.len(),
        inspected_gta_assets: inspected.assets.len(),
        decoded_geometries: geometry.geometries.len(),
    })
}


/// Convenience entry point for a standalone, unencrypted OPEN/NONE RPF7 file.
/// The archive is inspected in-place; no archive copy is made. Encrypted/NG
/// archives and RPF8 are rejected by the existing inventory parser.
pub fn prepare_rpf7_archive_json_placement_pipeline<F>(
    archive_bytes: &[u8],
    archive_name: &str,
    virtual_prefix: &str,
    placements: &JsonPlacementDocument,
    limits: &PlacementPipelineLimits,
    resolve_declaration: F,
) -> Result<PreparedPlacementPipeline, ManifestError>
where
    F: FnMut(&InspectedPlacementGeometryBuffers) -> Result<Vec<VertexElement>, ManifestError>,
{
    let inventory = inventory_rpf7_open(archive_bytes)?;
    let mut reader = std::io::Cursor::new(archive_bytes);
    prepare_rpf7_json_placement_pipeline(
        &mut reader,
        archive_name,
        virtual_prefix,
        0,
        &inventory,
        placements,
        limits,
        resolve_declaration,
    )
}
