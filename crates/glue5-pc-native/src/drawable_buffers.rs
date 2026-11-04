//! Bounded byte access for drawable vertex/index buffers after RSC7 page decoding.
//!
//! This is a page-addressed buffer reader, not a vertex declaration decoder.
//! Geometry-specific pointer offsets must be validated before calling this API.

use crate::{ManifestError, MountedGtaDrawablePages};

const SYSTEM_BASE: u64 = 0x5000_0000;
const GRAPHICS_BASE: u64 = 0x6000_0000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GtaBufferPage {
    System,
    Graphics,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GtaDrawableBufferData {
    pub page: GtaBufferPage,
    pub virtual_address: u64,
    pub bytes: Vec<u8>,
}

/// Copy a bounded byte range from an RSC7 system or graphics page.
///
/// The requested range must fit entirely within one decoded page. The caller
/// supplies the address and length after parsing the relevant GTA structure;
/// this function does not infer buffer locations or element formats.
pub fn read_gta_drawable_buffer_at(
    pages: &MountedGtaDrawablePages,
    virtual_address: u64,
    length: usize,
    max_bytes: usize,
) -> Result<GtaDrawableBufferData, ManifestError> {
    if length == 0 {
        return Err(ManifestError("drawable buffer length must be non-zero".into()));
    }
    if length > max_bytes {
        return Err(ManifestError("drawable buffer exceeds configured byte limit".into()));
    }

    let (page, base, data, declared_len) = if virtual_address >= GRAPHICS_BASE {
        (GtaBufferPage::Graphics, GRAPHICS_BASE, &pages.graphics_page, pages.page_sizes.graphics_bytes)
    } else if virtual_address >= SYSTEM_BASE {
        (GtaBufferPage::System, SYSTEM_BASE, &pages.system_page, pages.page_sizes.system_bytes)
    } else {
        return Err(ManifestError("drawable buffer pointer is outside RSC7 system/graphics address ranges".into()));
    };

    if u64::try_from(data.len()).ok() != Some(declared_len) {
        return Err(ManifestError("decoded RSC7 page length does not match declared page size".into()));
    }
    let offset = usize::try_from(virtual_address - base)
        .map_err(|_| ManifestError("drawable buffer offset does not fit platform".into()))?;
    let end = offset.checked_add(length)
        .ok_or_else(|| ManifestError("drawable buffer range overflow".into()))?;
    let slice = data.get(offset..end)
        .ok_or_else(|| ManifestError("drawable buffer range exceeds decoded RSC7 page".into()))?;

    Ok(GtaDrawableBufferData {
        page,
        virtual_address,
        bytes: slice.to_vec(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{GtaResourceCandidateKind, Rpf7ResourcePageSizes};

    fn pages() -> MountedGtaDrawablePages {
        MountedGtaDrawablePages {
            virtual_path: "fixture.ydr".into(),
            candidate_kind: GtaResourceCandidateKind::DrawableYdr,
            resource_version: 165,
            page_sizes: Rpf7ResourcePageSizes { system_bytes: 16, graphics_bytes: 16 },
            system_page: (0u8..16).collect(),
            graphics_page: (32u8..48).collect(),
        }
    }

    #[test]
    fn reads_system_and_graphics_ranges() {
        let p = pages();
        let system = read_gta_drawable_buffer_at(&p, SYSTEM_BASE + 4, 4, 8).unwrap();
        assert_eq!(system.page, GtaBufferPage::System);
        assert_eq!(system.bytes, vec![4, 5, 6, 7]);

        let graphics = read_gta_drawable_buffer_at(&p, GRAPHICS_BASE + 8, 4, 8).unwrap();
        assert_eq!(graphics.page, GtaBufferPage::Graphics);
        assert_eq!(graphics.bytes, vec![40, 41, 42, 43]);
    }

    #[test]
    fn rejects_invalid_ranges_and_oversized_reads() {
        let p = pages();
        assert!(read_gta_drawable_buffer_at(&p, SYSTEM_BASE + 15, 2, 8).is_err());
        assert!(read_gta_drawable_buffer_at(&p, GRAPHICS_BASE, 9, 8).is_err());
        assert!(read_gta_drawable_buffer_at(&p, 0x4000_0000, 1, 8).is_err());
        assert!(read_gta_drawable_buffer_at(&p, SYSTEM_BASE, 0, 8).is_err());
    }

    #[test]
    fn rejects_page_metadata_mismatch() {
        let mut p = pages();
        p.page_sizes.graphics_bytes = 15;
        assert!(read_gta_drawable_buffer_at(&p, GRAPHICS_BASE, 1, 8).is_err());
    }
}


#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GtaVertexBufferLayout {
    LegacyData1,
    LegacyData2,
    Gen9,
    GeometryInline,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GtaIndexBufferLayout {
    Legacy16,
    Gen9_16,
    Gen9_32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GtaGeometryBufferData {
    pub vertex_layout: Option<GtaVertexBufferLayout>,
    pub vertex_count: u32,
    pub vertex_stride: u16,
    pub vertex_data: Option<GtaDrawableBufferData>,
    pub index_layout: Option<GtaIndexBufferLayout>,
    pub index_count: u32,
    pub index_element_size: u16,
    pub index_data: Option<GtaDrawableBufferData>,
}

fn read_u16_at(pages: &MountedGtaDrawablePages, va: u64, offset: usize) -> Result<u16, ManifestError> {
    let address = va.checked_add(offset as u64)
        .ok_or_else(|| ManifestError("drawable buffer structure pointer overflow".into()))?;
    let data = read_gta_drawable_buffer_at(pages, address, 2, 2)?;
    Ok(u16::from_le_bytes(data.bytes.try_into().unwrap()))
}

fn read_u32_at(pages: &MountedGtaDrawablePages, va: u64, offset: usize) -> Result<u32, ManifestError> {
    let address = va.checked_add(offset as u64)
        .ok_or_else(|| ManifestError("drawable buffer structure pointer overflow".into()))?;
    let data = read_gta_drawable_buffer_at(pages, address, 4, 4)?;
    Ok(u32::from_le_bytes(data.bytes.try_into().unwrap()))
}

fn read_u64_at(pages: &MountedGtaDrawablePages, va: u64, offset: usize) -> Result<u64, ManifestError> {
    let address = va.checked_add(offset as u64)
        .ok_or_else(|| ManifestError("drawable buffer structure pointer overflow".into()))?;
    let data = read_gta_drawable_buffer_at(pages, address, 8, 8)?;
    Ok(u64::from_le_bytes(data.bytes.try_into().unwrap()))
}

fn checked_buffer_len(count: u32, stride: u16, max_count: u32, max_bytes: usize) -> Result<usize, ManifestError> {
    if count == 0 || count > max_count || stride == 0 {
        return Err(ManifestError("drawable buffer count or stride is invalid".into()));
    }
    let length = (count as usize).checked_mul(stride as usize)
        .ok_or_else(|| ManifestError("drawable buffer byte length overflow".into()))?;
    if length > max_bytes {
        return Err(ManifestError("drawable buffer exceeds configured byte limit".into()));
    }
    Ok(length)
}

/// Resolve the vertex and index byte streams referenced by one parsed geometry.
///
/// Legacy DataPointer1/DataPointer2 and Gen9 vertex layouts are attempted in
/// that order, followed by geometry-inline vertex data. Index data supports
/// legacy 16-bit and Gen9 16/32-bit indices. This copies raw bytes only; it
/// does not decode vertex declarations, topology, or GPU resources.
pub fn read_gta_geometry_buffers(
    pages: &MountedGtaDrawablePages,
    geometry: &crate::GtaGeometrySummary,
    max_vertices: u32,
    max_indices: u32,
    max_buffer_bytes: usize,
) -> Result<GtaGeometryBufferData, ManifestError> {
    let mut out = GtaGeometryBufferData {
        vertex_layout: None,
        vertex_count: 0,
        vertex_stride: 0,
        vertex_data: None,
        index_layout: None,
        index_count: 0,
        index_element_size: 0,
        index_data: None,
    };

    let vb = geometry.vertex_buffer_pointer;
    if vb != 0 {
        // Legacy VertexBuffer: stride +0x08, DataPointer1 +0x10,
        // count +0x18, DataPointer2 +0x20.
        if let Ok(raw) = read_gta_drawable_buffer_at(pages, vb, 0x80, 0x80) {
            let stride = u16::from_le_bytes(raw.bytes[0x08..0x0A].try_into().unwrap());
            let count = u32::from_le_bytes(raw.bytes[0x18..0x1C].try_into().unwrap());
            let p1 = u64::from_le_bytes(raw.bytes[0x10..0x18].try_into().unwrap());
            let p2 = u64::from_le_bytes(raw.bytes[0x20..0x28].try_into().unwrap());
            if let Ok(len) = checked_buffer_len(count, stride, max_vertices, max_buffer_bytes) {
                if let Ok(data) = read_gta_drawable_buffer_at(pages, p1, len, max_buffer_bytes) {
                    out.vertex_layout = Some(GtaVertexBufferLayout::LegacyData1);
                    out.vertex_count = count;
                    out.vertex_stride = stride;
                    out.vertex_data = Some(data);
                } else if let Ok(data) = read_gta_drawable_buffer_at(pages, p2, len, max_buffer_bytes) {
                    out.vertex_layout = Some(GtaVertexBufferLayout::LegacyData2);
                    out.vertex_count = count;
                    out.vertex_stride = stride;
                    out.vertex_data = Some(data);
                }
            }
            // Gen9 VertexBuffer: count +0x08, stride +0x0C, data +0x18.
            if out.vertex_data.is_none() {
                let count = u32::from_le_bytes(raw.bytes[0x08..0x0C].try_into().unwrap());
                let stride = u16::from_le_bytes(raw.bytes[0x0C..0x0E].try_into().unwrap());
                let pointer = u64::from_le_bytes(raw.bytes[0x18..0x20].try_into().unwrap());
                if let Ok(len) = checked_buffer_len(count, stride, max_vertices, max_buffer_bytes) {
                    if let Ok(data) = read_gta_drawable_buffer_at(pages, pointer, len, max_buffer_bytes) {
                        out.vertex_layout = Some(GtaVertexBufferLayout::Gen9);
                        out.vertex_count = count;
                        out.vertex_stride = stride;
                        out.vertex_data = Some(data);
                    }
                }
            }
        }
    }
    if out.vertex_data.is_none() && geometry.inline_data_pointer != 0 && geometry.vertices_count > 0 {
        let count = u32::from(geometry.vertices_count);
        let len = checked_buffer_len(count, geometry.vertex_stride, max_vertices, max_buffer_bytes)?;
        out.vertex_data = Some(read_gta_drawable_buffer_at(pages, geometry.inline_data_pointer, len, max_buffer_bytes)?);
        out.vertex_layout = Some(GtaVertexBufferLayout::GeometryInline);
        out.vertex_count = count;
        out.vertex_stride = geometry.vertex_stride;
    }

    let ib = geometry.index_buffer_pointer;
    if ib != 0 {
        if let Ok(raw) = read_gta_drawable_buffer_at(pages, ib, 0x60, 0x60) {
            let count = u32::from_le_bytes(raw.bytes[0x08..0x0C].try_into().unwrap());
            let legacy_pointer = u64::from_le_bytes(raw.bytes[0x10..0x18].try_into().unwrap());
            if count > 0 && count <= max_indices {
                let legacy_len = (count as usize).checked_mul(2)
                    .ok_or_else(|| ManifestError("index buffer byte length overflow".into()))?;
                if legacy_len <= max_buffer_bytes {
                    if let Ok(data) = read_gta_drawable_buffer_at(pages, legacy_pointer, legacy_len, max_buffer_bytes) {
                        out.index_layout = Some(GtaIndexBufferLayout::Legacy16);
                        out.index_count = count;
                        out.index_element_size = 2;
                        out.index_data = Some(data);
                    }
                }
            }
            if out.index_data.is_none() {
                let pointer = u64::from_le_bytes(raw.bytes[0x18..0x20].try_into().unwrap());
                let element_size = u16::from_le_bytes(raw.bytes[0x0C..0x0E].try_into().unwrap());
                if count > 0 && count <= max_indices && matches!(element_size, 2 | 4) {
                    let len = (count as usize).checked_mul(element_size as usize)
                        .ok_or_else(|| ManifestError("index buffer byte length overflow".into()))?;
                    if len <= max_buffer_bytes {
                        if let Ok(data) = read_gta_drawable_buffer_at(pages, pointer, len, max_buffer_bytes) {
                            out.index_layout = Some(if element_size == 2 { GtaIndexBufferLayout::Gen9_16 } else { GtaIndexBufferLayout::Gen9_32 });
                            out.index_count = count;
                            out.index_element_size = element_size;
                            out.index_data = Some(data);
                        }
                    }
                }
            }
        }
    }

    Ok(out)
}
