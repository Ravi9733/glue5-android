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
