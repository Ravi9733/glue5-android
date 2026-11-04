//! Decode mounted placement payloads after bounded backing-file reads.
//!
//! Binary entries remain byte-for-byte data. Resource entries are inflated as
//! RSC7 into bounded system/graphics page images; this is not GTA object
//! parsing, pointer relocation, GPU upload, or world spawning.
use crate::{decode_rsc7_resource, ManifestError, ResolvedPlacementPayloadBatch, MountedAssetKind, DecodedRsc7Resource};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DecodedPlacementContent {
    Binary(Vec<u8>),
    Rsc7(DecodedRsc7Resource),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodedPlacementAsset {
    pub placement_index: usize,
    pub virtual_path: String,
    pub content: DecodedPlacementContent,
    pub decoded_bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodedPlacementAssetBatch {
    pub assets: Vec<DecodedPlacementAsset>,
    pub total_decoded_bytes: u64,
}

/// Decode each resource payload in a previously bounded placement batch.
/// Per-resource and aggregate decompressed byte limits are enforced. The
/// function returns no partial batch if any resource is malformed or exceeds
/// its budget.
pub fn decode_resolved_placement_payloads(
    batch: &ResolvedPlacementPayloadBatch,
    max_resource_output_bytes: usize,
    max_total_decoded_bytes: u64,
) -> Result<DecodedPlacementAssetBatch, ManifestError> {
    let mut assets = Vec::new();
    assets.try_reserve_exact(batch.payloads.len())
        .map_err(|e| ManifestError(format!("cannot reserve decoded placement list: {e}")))?;
    let mut total = 0_u64;

    for payload in &batch.payloads {
        let (content, decoded_bytes) = match payload.asset.kind {
            MountedAssetKind::Binary => {
                (DecodedPlacementContent::Binary(payload.bytes.clone()), payload.bytes.len() as u64)
            }
            MountedAssetKind::Resource => {
                let decoded = decode_rsc7_resource(&payload.bytes, max_resource_output_bytes)
                    .map_err(|e| ManifestError(format!(
                        "placement {} ({}) RSC7 decode failed: {}",
                        payload.placement_index, payload.virtual_path, e.0
                    )))?;
                let count = decoded.body.len() as u64;
                (DecodedPlacementContent::Rsc7(decoded), count)
            }
        };
        total = total.checked_add(decoded_bytes)
            .ok_or_else(|| ManifestError("decoded placement aggregate size overflow".into()))?;
        if total > max_total_decoded_bytes {
            return Err(ManifestError(format!(
                "decoded placement batch exceeds aggregate limit ({} > {})",
                total, max_total_decoded_bytes
            )));
        }
        assets.push(DecodedPlacementAsset {
            placement_index: payload.placement_index,
            virtual_path: payload.virtual_path.clone(),
            content,
            decoded_bytes,
        });
    }

    Ok(DecodedPlacementAssetBatch { assets, total_decoded_bytes: total })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MountedAsset, ResolvedPlacementPayload};
    use flate2::{write::DeflateEncoder, Compression};
    use std::io::Write;

    fn resource_fixture() -> Vec<u8> {
        let body = vec![0x3C; 512];
        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&body).unwrap();
        let compressed = encoder.finish().unwrap();
        let mut payload = b"RSC7".to_vec();
        payload.extend_from_slice(&1_u32.to_le_bytes());
        payload.extend_from_slice(&(1_u32 << 27).to_le_bytes());
        payload.extend_from_slice(&0_u32.to_le_bytes());
        payload.extend_from_slice(&compressed);
        payload
    }

    fn batch(bytes: Vec<u8>, kind: MountedAssetKind) -> ResolvedPlacementPayloadBatch {
        let len = bytes.len() as u64;
        ResolvedPlacementPayloadBatch {
            payloads: vec![ResolvedPlacementPayload {
                placement_index: 4,
                virtual_path: "props/test.ydr".into(),
                asset: MountedAsset {
                    archive: "test.rpf".into(), entry_index: 9,
                    offset: 64, length: len, kind,
                },
                bytes,
            }],
            total_bytes: len,
        }
    }

    #[test]
    fn decodes_resource_pages_and_preserves_placement_identity() {
        let result = decode_resolved_placement_payloads(
            &batch(resource_fixture(), MountedAssetKind::Resource), 1024, 1024
        ).unwrap();
        assert_eq!(result.total_decoded_bytes, 512);
        assert_eq!(result.assets[0].placement_index, 4);
        assert_eq!(result.assets[0].virtual_path, "props/test.ydr");
        match &result.assets[0].content {
            DecodedPlacementContent::Rsc7(decoded) => {
                assert_eq!(decoded.body, vec![0x3C; 512]);
                assert_eq!(decoded.page_sizes.system_bytes, 512);
            }
            _ => panic!("resource payload was not decoded as RSC7"),
        }
    }

    #[test]
    fn preserves_binary_payload_and_rejects_bad_resource_or_caps() {
        let binary = batch(b"plain".to_vec(), MountedAssetKind::Binary);
        let preserved = decode_resolved_placement_payloads(&binary, 16, 16).unwrap();
        assert!(matches!(&preserved.assets[0].content, DecodedPlacementContent::Binary(v) if v == b"plain"));

        let resource = batch(resource_fixture(), MountedAssetKind::Resource);
        assert!(decode_resolved_placement_payloads(&resource, 128, 1024).unwrap_err().0.contains("exceeds configured limit"));
        assert!(decode_resolved_placement_payloads(&resource, 1024, 128).unwrap_err().0.contains("aggregate limit"));
        assert!(decode_resolved_placement_payloads(
            &batch(b"not RSC7".to_vec(), MountedAssetKind::Resource), 1024, 1024
        ).unwrap_err().0.contains("RSC7 decode failed"));
    }
}
