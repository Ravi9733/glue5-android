//! File-backed RPF7 metadata inventory. Reads only the header, TOC, and name
//! table; large archive payloads are never loaded into memory.
use crate::{inventory_rpf7_open, ManifestError, Rpf7Inventory, RpfProbe};
use std::io::{Read, Seek, SeekFrom};

/// An RPF7 inventory paired with the original archive's full byte length.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rpf7FileInventory {
    pub archive_len: u64,
    pub inventory: Rpf7Inventory,
}

/// Inventory an OPEN/NONE RPF7 archive from a seekable file or stream.
///
/// Memory use is bounded by the existing parser limits for TOC entries and
/// names (at most 16 MiB + 64 MiB, plus the fixed header). The archive payload
/// itself is not read or retained.
pub fn inventory_rpf7_open_reader<R: Read + Seek>(
    reader: &mut R,
) -> Result<Rpf7FileInventory, ManifestError> {
    let archive_len = reader.seek(SeekFrom::End(0))
        .map_err(|e| ManifestError(format!("cannot seek to RPF7 archive end: {e}")))?;
    if archive_len < 16 {
        return Err(ManifestError("RPF7 header is truncated (need 16 bytes)".into()));
    }
    reader.seek(SeekFrom::Start(0))
        .map_err(|e| ManifestError(format!("cannot seek to RPF7 archive start: {e}")))?;
    let mut header_bytes = [0_u8; 16];
    reader.read_exact(&mut header_bytes)
        .map_err(|e| ManifestError(format!("cannot read RPF7 header: {e}")))?;
    let header = match crate::probe_rpf_header(&header_bytes)? {
        RpfProbe::GtaVLegacy(header) => header,
        RpfProbe::GtaVEnhancedOrRdr2 { .. } =>
            return Err(ManifestError("RPF8 is not a GTA V Legacy RPF7 archive".into())),
        RpfProbe::UnsupportedOrUnknown { magic } =>
            return Err(ManifestError(format!("unsupported archive magic: {magic:#010x}"))),
    };
    let metadata_len = 16_u64.checked_add(header.toc_bytes)
        .and_then(|n| n.checked_add(u64::from(header.names_length)))
        .ok_or_else(|| ManifestError("RPF7 metadata length overflow".into()))?;
    if metadata_len > archive_len {
        return Err(ManifestError("RPF7 TOC or name table is truncated".into()));
    }
    let metadata_len = usize::try_from(metadata_len)
        .map_err(|_| ManifestError("RPF7 metadata length does not fit this platform".into()))?;
    reader.seek(SeekFrom::Start(0))
        .map_err(|e| ManifestError(format!("cannot rewind RPF7 archive: {e}")))?;
    let mut metadata = Vec::new();
    metadata.try_reserve_exact(metadata_len)
        .map_err(|e| ManifestError(format!("cannot allocate bounded RPF7 metadata buffer: {e}")))?;
    metadata.resize(metadata_len, 0);
    reader.read_exact(&mut metadata)
        .map_err(|e| ManifestError(format!("cannot read RPF7 metadata: {e}")))?;
    let inventory = inventory_rpf7_open(&metadata)?;
    Ok(Rpf7FileInventory { archive_len, inventory })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn fixture() -> Vec<u8> {
        let mut bytes = vec![0_u8; 16 + 32 + 14 + 512];
        bytes[0..4].copy_from_slice(&0x5250_4637_u32.to_le_bytes());
        bytes[4..8].copy_from_slice(&2_u32.to_le_bytes());
        bytes[8..12].copy_from_slice(&14_u32.to_le_bytes());
        bytes[12..16].copy_from_slice(&0x4E45_504F_u32.to_le_bytes());
        bytes[16..20].copy_from_slice(&0_u32.to_le_bytes());
        bytes[20..24].copy_from_slice(&0x7FFF_FF00_u32.to_le_bytes());
        bytes[24..28].copy_from_slice(&1_u32.to_le_bytes());
        bytes[28..32].copy_from_slice(&1_u32.to_le_bytes());
        bytes[32..34].copy_from_slice(&5_u16.to_le_bytes());
        bytes[37] = 1;
        bytes[40..44].copy_from_slice(&4_u32.to_le_bytes());
        bytes[44..48].copy_from_slice(&0_u32.to_le_bytes());
        bytes[48..62].copy_from_slice(b"root\0file.bin\0");
        bytes
    }

    #[test]
    fn inventories_only_metadata_and_preserves_full_archive_length() {
        let bytes = fixture();
        let mut reader = Cursor::new(bytes.clone());
        let result = inventory_rpf7_open_reader(&mut reader).unwrap();
        assert_eq!(result.archive_len, bytes.len() as u64);
        assert_eq!(result.inventory.entries.len(), 2);
        assert_eq!(result.inventory.entries[1].name, "file.bin");
        assert_eq!(reader.position(), 0);
    }

    #[test]
    fn rejects_truncated_metadata_and_non_rpf7_archives() {
        let mut truncated = Cursor::new(fixture()[..48].to_vec());
        assert!(inventory_rpf7_open_reader(&mut truncated).unwrap_err().0.contains("truncated"));
        let mut unknown = Cursor::new(vec![0_u8; 32]);
        assert!(inventory_rpf7_open_reader(&mut unknown).unwrap_err().0.contains("unsupported archive magic"));
    }
}
