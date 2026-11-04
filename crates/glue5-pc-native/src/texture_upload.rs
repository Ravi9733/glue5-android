//! Bounded upload of already-decoded GTA RGBA8 textures to the active wgpu device.
//!
//! This creates a sampled texture resource. Material/shader binding and draw
//! integration remain the renderer's responsibility.
use crate::{GtaCpuTexture, ManifestError};

#[derive(Debug)]
pub struct GpuTexture {
    /// Keep the allocation alive for as long as its view is used.
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub width: u32,
    pub height: u32,
}

/// Upload one tightly-packed top-mip RGBA8 texture.
/// The caller must pass the device and queue owned by Glue5's active renderer;
/// on Android's Vulkan backend this uses the selected Vulkan driver.
pub fn upload_gta_cpu_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    source: &GtaCpuTexture,
    max_upload_bytes: usize,
) -> Result<GpuTexture, ManifestError> {
    if source.width == 0 || source.height == 0 {
        return Err(ManifestError("texture dimensions must be non-zero".into()));
    }
    let width = u32::from(source.width);
    let height = u32::from(source.height);
    if width > device.limits().max_texture_dimension_2d
        || height > device.limits().max_texture_dimension_2d
    {
        return Err(ManifestError("texture dimensions exceed device limits".into()));
    }
    let expected = usize::try_from(width)
        .ok().and_then(|w| usize::try_from(height).ok().and_then(|h| w.checked_mul(h)))
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| ManifestError("texture upload byte count overflow".into()))?;
    if expected != source.rgba8.len() {
        return Err(ManifestError("RGBA8 texture byte length does not match dimensions".into()));
    }
    if expected > max_upload_bytes {
        return Err(ManifestError("texture upload exceeds configured byte limit".into()));
    }

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("glue5-pc-mod-texture"),
        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &source.rgba8,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(width * 4),
            rows_per_image: Some(height),
        },
        wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
    );
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    Ok(GpuTexture { texture, view, width, height })
}
