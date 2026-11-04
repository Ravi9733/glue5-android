//! GPU buffer upload bridge for decoded GTA CPU meshes.
//!
//! The supplied wgpu device must come from Glue5's Vulkan backend on Android.
//! This module uploads position-only vertices and optional indices; it does
//! not create a render pipeline, bind materials, or spawn world objects.
use crate::{GtaCpuMesh, ManifestError};
use wgpu::util::DeviceExt;

#[derive(Debug)]
pub struct GpuMeshBuffers {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: Option<wgpu::Buffer>,
    pub vertex_count: u32,
    pub index_count: u32,
    pub vertex_stride: u64,
}

/// Upload decoded CPU mesh positions and optional u32 indices to a wgpu device.
/// Pass the device obtained from Glue5's Vulkan renderer to use Vulkan/Turnip.
pub fn upload_gta_cpu_mesh(
    device: &wgpu::Device,
    mesh: &GtaCpuMesh,
    max_vertices: u32,
    max_indices: u32,
    max_upload_bytes: usize,
) -> Result<GpuMeshBuffers, ManifestError> {
    if mesh.vertices.is_empty() || mesh.vertices.len() > max_vertices as usize {
        return Err(ManifestError("GPU upload vertex count is empty or exceeds limit".into()));
    }
    let mut vertex_bytes = Vec::with_capacity(mesh.vertices.len().saturating_mul(12));
    for vertex in &mesh.vertices {
        let position = vertex.attributes.iter()
            .find(|attribute| attribute.semantic.eq_ignore_ascii_case("position"))
            .ok_or_else(|| ManifestError("GPU upload requires a Position vertex attribute".into()))?;
        if position.components.len() < 3 {
            return Err(ManifestError("Position attribute must contain at least three components".into()));
        }
        for component in position.components.iter().take(3) {
            if !component.is_finite() {
                return Err(ManifestError("Position contains a non-finite component".into()));
            }
            vertex_bytes.extend_from_slice(&component.to_le_bytes());
        }
    }
    let index_bytes = if let Some(indices) = &mesh.indices {
        if indices.is_empty() || indices.len() > max_indices as usize {
            return Err(ManifestError("GPU upload index count is empty or exceeds limit".into()));
        }
        let mut bytes = Vec::with_capacity(indices.len().saturating_mul(4));
        for index in indices {
            if *index as usize >= mesh.vertices.len() {
                return Err(ManifestError("GPU upload index references a missing vertex".into()));
            }
            bytes.extend_from_slice(&index.to_le_bytes());
        }
        Some(bytes)
    } else { None };
    let total = vertex_bytes.len().checked_add(index_bytes.as_ref().map_or(0, Vec::len))
        .ok_or_else(|| ManifestError("GPU upload byte count overflow".into()))?;
    if total > max_upload_bytes {
        return Err(ManifestError("GPU mesh upload exceeds configured byte limit".into()));
    }
    let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("glue5-pc-mod-positions"),
        contents: &vertex_bytes,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
    });
    let index_buffer = index_bytes.as_ref().map(|bytes| device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("glue5-pc-mod-indices"),
        contents: bytes,
        usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
    }));
    Ok(GpuMeshBuffers {
        vertex_buffer,
        index_buffer,
        vertex_count: mesh.vertices.len() as u32,
        index_count: mesh.indices.as_ref().map_or(0, |indices| indices.len() as u32),
        vertex_stride: 12,
    })
}
