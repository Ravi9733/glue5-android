//! Conservative YDR/YDD structure inspection over decoded RSC7 pages.
//!
//! Layout offsets are cross-checked against the public rage-formats parser.
//! This module reports drawable/model/geometry metadata only; it does not
//! decode vertex declarations, materials, shaders, or create GPU resources.

use crate::{GtaResourceCandidateKind, ManifestError, MountedGtaDrawablePages};

const SYSTEM_BASE: u64 = 0x5000_0000;
const MAX_DRAWABLES: usize = 1024;
const MAX_MODELS_PER_LOD: usize = 4096;
const MAX_GEOMETRIES_PER_MODEL: usize = 4096;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GtaGeometrySummary {
    pub indices_count: u32,
    pub triangles_count: u32,
    pub vertices_count: u16,
    pub vertex_stride: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GtaModelSummary {
    pub geometry_count: usize,
    pub geometries: Vec<GtaGeometrySummary>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GtaDrawableSummary {
    pub name: String,
    pub lod_model_counts: [usize; 4],
    pub models: Vec<GtaModelSummary>,
    pub geometry_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GtaDrawableResourceSummary {
    pub virtual_path: String,
    pub candidate_kind: GtaResourceCandidateKind,
    pub resource_version: u32,
    pub drawables: Vec<GtaDrawableSummary>,
    pub model_count: usize,
    pub geometry_count: usize,
}

struct PageReader<'a> { system: &'a [u8] }

impl<'a> PageReader<'a> {
    fn bytes(&self, va: u64, len: usize) -> Result<&'a [u8], ManifestError> {
        if va < SYSTEM_BASE {
            return Err(ManifestError("drawable pointer is not in the RSC7 system page".into()));
        }
        let offset = usize::try_from(va - SYSTEM_BASE)
            .map_err(|_| ManifestError("drawable pointer offset does not fit platform".into()))?;
        let end = offset.checked_add(len)
            .ok_or_else(|| ManifestError("drawable structure range overflow".into()))?;
        self.system.get(offset..end)
            .ok_or_else(|| ManifestError("drawable structure points outside the system page".into()))
    }
    fn u16(&self, va: u64, offset: usize) -> Result<u16, ManifestError> {
        let at = va.checked_add(offset as u64).ok_or_else(|| ManifestError("pointer overflow".into()))?;
        Ok(u16::from_le_bytes(self.bytes(at, 2)?.try_into().unwrap()))
    }
    fn u32(&self, va: u64, offset: usize) -> Result<u32, ManifestError> {
        let at = va.checked_add(offset as u64).ok_or_else(|| ManifestError("pointer overflow".into()))?;
        Ok(u32::from_le_bytes(self.bytes(at, 4)?.try_into().unwrap()))
    }
    fn u64(&self, va: u64, offset: usize) -> Result<u64, ManifestError> {
        let at = va.checked_add(offset as u64).ok_or_else(|| ManifestError("pointer overflow".into()))?;
        Ok(u64::from_le_bytes(self.bytes(at, 8)?.try_into().unwrap()))
    }
    fn list_header(&self, va: u64) -> Result<(u64, usize), ManifestError> {
        let ptr = self.u64(va, 0)?;
        let count = self.u16(va, 8)? as usize;
        let capacity = self.u16(va, 10)? as usize;
        if count > capacity || capacity > MAX_MODELS_PER_LOD {
            return Err(ManifestError("drawable pointer-list count/capacity is invalid".into()));
        }
        let actual = if count == 0 { capacity } else { count };
        if actual > 0 && ptr == 0 {
            return Err(ManifestError("drawable pointer-list has a null backing pointer".into()));
        }
        Ok((ptr, actual))
    }
    fn pointer_array(&self, ptr: u64, count: usize) -> Result<Vec<u64>, ManifestError> {
        if count == 0 { return Ok(Vec::new()); }
        let len = count.checked_mul(8).ok_or_else(|| ManifestError("pointer array length overflow".into()))?;
        let bytes = self.bytes(ptr, len)?;
        Ok(bytes.chunks_exact(8).map(|b| u64::from_le_bytes(b.try_into().unwrap())).collect())
    }
}

fn parse_drawable(reader: &PageReader<'_>, va: u64) -> Result<GtaDrawableSummary, ManifestError> {
    reader.bytes(va, 0xD0)?;
    let lod_ptrs = [
        if reader.u64(va, 0x50)? == 0 { reader.u64(va, 0xA0)? } else { reader.u64(va, 0x50)? },
        reader.u64(va, 0x58)?,
        reader.u64(va, 0x60)?,
        reader.u64(va, 0x68)?,
    ];
    let mut lod_model_counts = [0usize; 4];
    let mut models = Vec::new();
    for (lod_index, lod_ptr) in lod_ptrs.into_iter().enumerate() {
        if lod_ptr == 0 { continue; }
        let (array_ptr, count) = reader.list_header(lod_ptr)?;
        let model_ptrs = reader.pointer_array(array_ptr, count)?;
        lod_model_counts[lod_index] = model_ptrs.iter().filter(|p| **p != 0).count();
        for model_ptr in model_ptrs.into_iter().filter(|p| *p != 0) {
            reader.bytes(model_ptr, 0x30)?;
            let geometry_ptr = reader.u64(model_ptr, 0x08)?;
            let geometry_count = reader.u16(model_ptr, 0x10)? as usize;
            if geometry_count > MAX_GEOMETRIES_PER_MODEL {
                return Err(ManifestError("drawable model geometry count exceeds limit".into()));
            }
            let mut geometries = Vec::with_capacity(geometry_count);
            if geometry_count > 0 {
                if geometry_ptr == 0 { return Err(ManifestError("drawable model geometry array is null".into())); }
                let ptrs = reader.pointer_array(geometry_ptr, geometry_count)?;
                for ptr in ptrs {
                    if ptr == 0 { continue; }
                    reader.bytes(ptr, 0x98)?;
                    geometries.push(GtaGeometrySummary {
                        indices_count: reader.u32(ptr, 0x58)?,
                        triangles_count: reader.u32(ptr, 0x5C)?,
                        vertices_count: reader.u16(ptr, 0x60)?,
                        vertex_stride: reader.u16(ptr, 0x70)?,
                    });
                }
            }
            models.push(GtaModelSummary { geometry_count: geometries.len(), geometries });
        }
    }
    let geometry_count = models.iter().map(|m| m.geometry_count).sum();
    Ok(GtaDrawableSummary { name: String::new(), lod_model_counts, models, geometry_count })
}

/// Inspect root YDR or YDD drawable/model/geometry structure after RSC7 decode.
/// All pointers and arrays are bounded to the supplied system page.
pub fn inspect_gta_drawable_structure(
    pages: &MountedGtaDrawablePages,
) -> Result<GtaDrawableResourceSummary, ManifestError> {
    if !matches!(pages.candidate_kind, GtaResourceCandidateKind::DrawableYdr | GtaResourceCandidateKind::DrawableDictionaryYdd) {
        return Err(ManifestError("resource is not a YDR or YDD drawable".into()));
    }
    let reader = PageReader { system: &pages.system_page };
    let drawables = if pages.candidate_kind == GtaResourceCandidateKind::DrawableYdr {
        vec![parse_drawable(&reader, SYSTEM_BASE)?]
    } else {
        reader.bytes(SYSTEM_BASE, 0x40)?;
        let array_ptr = reader.u64(SYSTEM_BASE, 0x30)?;
        let count = reader.u16(SYSTEM_BASE, 0x38)? as usize;
        let capacity = reader.u16(SYSTEM_BASE, 0x3A)? as usize;
        if count > capacity || capacity > MAX_DRAWABLES {
            return Err(ManifestError("YDD drawable count/capacity is invalid".into()));
        }
        reader.pointer_array(array_ptr, count)?.into_iter()
            .filter(|p| *p != 0)
            .map(|ptr| parse_drawable(&reader, ptr))
            .collect::<Result<Vec<_>, _>>()?
    };
    let model_count = drawables.iter().map(|d| d.models.len()).sum();
    let geometry_count = drawables.iter().map(|d| d.geometry_count).sum();
    Ok(GtaDrawableResourceSummary {
        virtual_path: pages.virtual_path.clone(),
        candidate_kind: pages.candidate_kind,
        resource_version: pages.resource_version,
        drawables,
        model_count,
        geometry_count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put_u16(b: &mut [u8], at: usize, v: u16) { b[at..at+2].copy_from_slice(&v.to_le_bytes()); }
    fn put_u32(b: &mut [u8], at: usize, v: u32) { b[at..at+4].copy_from_slice(&v.to_le_bytes()); }
    fn put_u64(b: &mut [u8], at: usize, v: u64) { b[at..at+8].copy_from_slice(&v.to_le_bytes()); }

    fn fixture(kind: GtaResourceCandidateKind) -> MountedGtaDrawablePages {
        let mut s = vec![0u8; 1024];
        let (drawable_at, lod_at, model_array, model_at, geom_array, geom_at) =
            if kind == GtaResourceCandidateKind::DrawableYdr {
                (0usize, 0x100usize, 0x120usize, 0x140usize, 0x180usize, 0x1A0usize)
            } else {
                (0x80usize, 0x100usize, 0x120usize, 0x140usize, 0x180usize, 0x1A0usize)
            };
        if kind == GtaResourceCandidateKind::DrawableDictionaryYdd {
            put_u64(&mut s, 0x30, SYSTEM_BASE + 0x40);
            put_u16(&mut s, 0x38, 1); put_u16(&mut s, 0x3A, 1);
            put_u64(&mut s, 0x40, SYSTEM_BASE + drawable_at as u64);
        }
        put_u64(&mut s, drawable_at + 0x50, SYSTEM_BASE + lod_at as u64);
        put_u64(&mut s, lod_at, SYSTEM_BASE + model_array as u64);
        put_u16(&mut s, lod_at + 8, 1); put_u16(&mut s, lod_at + 10, 1);
        put_u64(&mut s, model_array, SYSTEM_BASE + model_at as u64);
        put_u64(&mut s, model_at + 8, SYSTEM_BASE + geom_array as u64);
        put_u16(&mut s, model_at + 0x10, 1);
        put_u64(&mut s, geom_array, SYSTEM_BASE + geom_at as u64);
        put_u32(&mut s, geom_at + 0x58, 36);
        put_u32(&mut s, geom_at + 0x5C, 12);
        put_u16(&mut s, geom_at + 0x60, 24);
        put_u16(&mut s, geom_at + 0x70, 32);
        MountedGtaDrawablePages {
            virtual_path: if kind == GtaResourceCandidateKind::DrawableYdr { "test.ydr" } else { "test.ydd" }.into(),
            candidate_kind: kind, resource_version: 165,
            page_sizes: crate::Rpf7ResourcePageSizes { system_bytes: 1024, graphics_bytes: 0 },
            system_page: s, graphics_page: Vec::new(),
        }
    }

    #[test]
    fn reads_yDR_model_and_geometry_metadata() {
        let out = inspect_gta_drawable_structure(&fixture(GtaResourceCandidateKind::DrawableYdr)).unwrap();
        assert_eq!(out.model_count, 1); assert_eq!(out.geometry_count, 1);
        assert_eq!(out.drawables[0].models[0].geometries[0].vertices_count, 24);
        assert_eq!(out.drawables[0].models[0].geometries[0].indices_count, 36);
    }

    #[test]
    fn reads_ydd_dictionary_drawable_entries() {
        let out = inspect_gta_drawable_structure(&fixture(GtaResourceCandidateKind::DrawableDictionaryYdd)).unwrap();
        assert_eq!(out.drawables.len(), 1); assert_eq!(out.model_count, 1);
    }

    #[test]
    fn rejects_invalid_pointer_and_excessive_counts() {
        let mut bad = fixture(GtaResourceCandidateKind::DrawableYdr);
        put_u64(&mut bad.system_page, 0x50, 0x5000_FFFF);
        assert!(inspect_gta_drawable_structure(&bad).is_err());
        let mut excessive = fixture(GtaResourceCandidateKind::DrawableYdr);
        put_u16(&mut excessive.system_page, 0x108, u16::MAX);
        assert!(inspect_gta_drawable_structure(&excessive).is_err());
    }
}
