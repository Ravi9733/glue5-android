//! Bounded RSC7 page inspection for mounted GTA drawable candidates.
//!
//! This stage only inflates the resource and separates system/graphics pages.
//! It does not parse YDR/YDD geometry, skeletons, shaders, or GPU objects.

use crate::{
    classify_gta_resource_candidate, decode_rsc7_resource, split_rsc7_resource_pages,
    GtaResourceCandidateKind, ManifestError, MountedGtaDrawablePayload,
    Rpf7ResourcePageSizes,
};

/// Decoded page buffers for a mounted YDR/YDD candidate.
///
/// These are raw RSC7 page images. They are not decoded GTA drawable objects.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MountedGtaDrawablePages {
    pub virtual_path: String,
    pub candidate_kind: GtaResourceCandidateKind,
    pub resource_version: u32,
    pub page_sizes: Rpf7ResourcePageSizes,
    pub system_page: Vec<u8>,
    pub graphics_page: Vec<u8>,
}

/// Inflate a previously bounded mounted YDR/YDD payload and split its RSC7
/// body into the declared system and graphics page regions.
pub fn inspect_mounted_gta_drawable_pages(
    payload: &MountedGtaDrawablePayload,
    max_output_bytes: usize,
) -> Result<MountedGtaDrawablePages, ManifestError> {
    let path_kind = classify_gta_resource_candidate(&payload.virtual_path)
        .ok_or_else(|| ManifestError("mounted drawable path is not a recognized GTA resource".into()))?;
    if path_kind != payload.candidate_kind {
        return Err(ManifestError("mounted drawable candidate kind does not match its path".into()));
    }
    if !matches!(path_kind, GtaResourceCandidateKind::DrawableYdr | GtaResourceCandidateKind::DrawableDictionaryYdd) {
        return Err(ManifestError("mounted candidate is not a YDR or YDD drawable resource".into()));
    }

    let decoded = decode_rsc7_resource(&payload.rsc7_bytes, max_output_bytes)?;
    let (system, graphics) = split_rsc7_resource_pages(&decoded.body, decoded.page_sizes)?;
    Ok(MountedGtaDrawablePages {
        virtual_path: payload.virtual_path.clone(),
        candidate_kind: path_kind,
        resource_version: decoded.version,
        page_sizes: decoded.page_sizes,
        system_page: system.to_vec(),
        graphics_page: graphics.to_vec(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::{write::DeflateEncoder, Compression};
    use std::io::Write;

    fn payload(path: &str, kind: GtaResourceCandidateKind) -> MountedGtaDrawablePayload {
        let system = vec![0x11; 512];
        let graphics = vec![0x22; 512];
        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&system).unwrap();
        encoder.write_all(&graphics).unwrap();
        let compressed = encoder.finish().unwrap();
        let mut bytes = b"RSC7".to_vec();
        bytes.extend_from_slice(&7_u32.to_le_bytes());
        bytes.extend_from_slice(&(1_u32 << 27).to_le_bytes());
        bytes.extend_from_slice(&(1_u32 << 27).to_le_bytes());
        bytes.extend_from_slice(&compressed);
        MountedGtaDrawablePayload {
            virtual_path: path.into(),
            candidate_kind: kind,
            rsc7_bytes: bytes,
        }
    }

    #[test]
    fn decodes_ydd_and_separates_declared_page_regions() {
        let input = payload("stream/props.ydd", GtaResourceCandidateKind::DrawableDictionaryYdd);
        let result = inspect_mounted_gta_drawable_pages(&input, 2048).unwrap();
        assert_eq!(result.resource_version, 7);
        assert_eq!(result.page_sizes.system_bytes, 512);
        assert_eq!(result.page_sizes.graphics_bytes, 512);
        assert_eq!(result.system_page, vec![0x11; 512]);
        assert_eq!(result.graphics_page, vec![0x22; 512]);
    }

    #[test]
    fn accepts_ydr_and_rejects_ytd_or_mismatched_candidate_before_decode() {
        let ydr = payload("stream/prop.ydr", GtaResourceCandidateKind::DrawableYdr);
        assert!(inspect_mounted_gta_drawable_pages(&ydr, 2048).is_ok());

        let ytd = payload("stream/textures.ytd", GtaResourceCandidateKind::TextureDictionaryYtd);
        assert!(inspect_mounted_gta_drawable_pages(&ytd, 2048).unwrap_err().0.contains("not a YDR or YDD"));

        let mismatch = payload("stream/prop.ydr", GtaResourceCandidateKind::DrawableDictionaryYdd);
        assert!(inspect_mounted_gta_drawable_pages(&mismatch, 2048).unwrap_err().0.contains("does not match"));
    }

    #[test]
    fn enforces_output_cap_and_rejects_invalid_rsc7_payload() {
        let input = payload("stream/prop.ydr", GtaResourceCandidateKind::DrawableYdr);
        assert!(inspect_mounted_gta_drawable_pages(&input, 1024).is_ok());
        assert!(inspect_mounted_gta_drawable_pages(&input, 1000).unwrap_err().0.contains("exceeds configured limit"));

        let mut invalid = input;
        invalid.rsc7_bytes[0..4].copy_from_slice(b"NOPE");
        assert!(inspect_mounted_gta_drawable_pages(&invalid, 2048).unwrap_err().0.contains("magic"));
    }
}
