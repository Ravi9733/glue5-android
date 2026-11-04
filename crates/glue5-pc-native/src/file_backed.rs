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


/// Copy one unencrypted, uncompressed binary entry directly from a file-backed
/// RPF7 archive. Reads and writes in 64 KiB chunks; payload size is capped.
pub fn copy_rpf7_open_binary_entry_from_reader<R: Read + Seek, W: std::io::Write>(
    reader: &mut R,
    entry_index: u32,
    output: &mut W,
    max_output_bytes: u64,
) -> Result<u64, ManifestError> {
    use crate::Rpf7EntryKind;
    let inventory = inventory_rpf7_open_reader(reader)?;
    let entry = inventory.inventory.entries.iter().find(|e| e.index == entry_index)
        .ok_or_else(|| ManifestError(format!("RPF7 entry {entry_index} does not exist")))?;
    let (size, offset_blocks) = match &entry.kind {
        Rpf7EntryKind::BinaryFile { file_size, file_offset_blocks, uncompressed_size, encrypted } => {
            if *encrypted { return Err(ManifestError("RPF7 binary payload is encrypted".into())); }
            if *file_size != 0 && file_size != uncompressed_size {
                return Err(ManifestError("RPF7 binary payload is compressed; decompression is not supported".into()));
            }
            (if *file_size == 0 { u64::from(*uncompressed_size) } else { u64::from(*file_size) }, u64::from(*file_offset_blocks))
        }
        Rpf7EntryKind::ResourceFile { .. } => return Err(ManifestError("RPF7 entry is a resource, not a binary file".into())),
        Rpf7EntryKind::Directory { .. } => return Err(ManifestError("RPF7 directory has no payload".into())),
    };
    if size > max_output_bytes {
        return Err(ManifestError(format!("RPF7 binary payload exceeds extraction limit ({size} > {max_output_bytes})")));
    }
    let start = offset_blocks.checked_mul(512)
        .ok_or_else(|| ManifestError("RPF7 payload offset overflow".into()))?;
    let end = start.checked_add(size)
        .ok_or_else(|| ManifestError("RPF7 payload end overflow".into()))?;
    let metadata_end = 16_u64 + inventory.inventory.header.toc_bytes
        + u64::from(inventory.inventory.header.names_length);
    if start < metadata_end || end > inventory.archive_len {
        return Err(ManifestError("RPF7 binary payload range is outside archive payload bounds".into()));
    }
    reader.seek(SeekFrom::Start(start))
        .map_err(|e| ManifestError(format!("cannot seek to RPF7 binary payload: {e}")))?;
    let mut remaining = size;
    let mut buffer = [0_u8; 64 * 1024];
    while remaining > 0 {
        let want = usize::try_from(remaining.min(buffer.len() as u64)).unwrap();
        reader.read_exact(&mut buffer[..want])
            .map_err(|e| ManifestError(format!("cannot read RPF7 binary payload: {e}")))?;
        output.write_all(&buffer[..want])
            .map_err(|e| ManifestError(format!("cannot write RPF7 binary payload: {e}")))?;
        remaining -= want as u64;
    }
    Ok(size)
}

/// Decode one explicitly-sized RSC7 resource directly from a file-backed
/// archive. The legacy 0xFFFFFF size sentinel is rejected until its packed
/// resource-header size can be resolved safely by the file-backed path.
pub fn extract_rpf7_rsc7_entry_from_reader<R: Read + Seek, W: std::io::Write>(
    reader: &mut R,
    entry_index: u32,
    output: &mut W,
    max_output_bytes: usize,
) -> Result<crate::Rsc7StreamReport, ManifestError> {
    use crate::{decode_rsc7_resource_stream, Rpf7EntryKind};
    let inventory = inventory_rpf7_open_reader(reader)?;
    let entry = inventory.inventory.entries.iter().find(|e| e.index == entry_index)
        .ok_or_else(|| ManifestError(format!("RPF7 entry {entry_index} does not exist")))?;
    let (size, offset_blocks) = match &entry.kind {
        Rpf7EntryKind::ResourceFile { file_size, file_offset_blocks, .. } => {
            if *file_size == 0x00FF_FFFF {
                return Err(ManifestError("RPF7 resource size sentinel is not supported by file-backed extraction yet".into()));
            }
            (u64::from(*file_size), u64::from(*file_offset_blocks))
        }
        Rpf7EntryKind::BinaryFile { .. } => return Err(ManifestError("TOC entry is not an RSC7 resource".into())),
        Rpf7EntryKind::Directory { .. } => return Err(ManifestError("RPF7 directory has no payload".into())),
    };
    let start = offset_blocks.checked_mul(512)
        .ok_or_else(|| ManifestError("RPF7 resource offset overflow".into()))?;
    let end = start.checked_add(size)
        .ok_or_else(|| ManifestError("RPF7 resource end overflow".into()))?;
    let metadata_end = 16_u64 + inventory.inventory.header.toc_bytes
        + u64::from(inventory.inventory.header.names_length);
    if size < 16 || start < metadata_end || end > inventory.archive_len {
        return Err(ManifestError("RPF7 resource range is truncated, overlaps metadata, or exceeds archive bounds".into()));
    }
    decode_rsc7_resource_stream(reader, start, size, output, max_output_bytes)
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
        // The reader is left immediately after the metadata region; payload bytes remain unread.
        assert_eq!(reader.position(), 16 + 32 + 14);
    }

    #[test]
    fn streams_binary_payload_from_file_backed_archive() {
        let mut bytes = fixture();
        bytes[512..516].copy_from_slice(b"DATA");
        let mut reader = Cursor::new(bytes);
        let mut output = Vec::new();
        let written = copy_rpf7_open_binary_entry_from_reader(&mut reader, 1, &mut output, 16).unwrap();
        assert_eq!(written, 4);
        assert_eq!(output, b"DATA");
    }

    #[test]
    fn rejects_file_backed_binary_output_over_cap() {
        let mut bytes = fixture();
        bytes[512..516].copy_from_slice(b"DATA");
        let mut reader = Cursor::new(bytes);
        let mut output = Vec::new();
        assert!(copy_rpf7_open_binary_entry_from_reader(&mut reader, 1, &mut output, 3).is_err());
        assert!(output.is_empty());
    }

    #[test]
    fn rejects_truncated_metadata_and_non_rpf7_archives() {
        let mut truncated = Cursor::new(fixture()[..48].to_vec());
        assert!(inventory_rpf7_open_reader(&mut truncated).unwrap_err().0.contains("truncated"));
        let mut unknown = Cursor::new(vec![0_u8; 32]);
        assert!(inventory_rpf7_open_reader(&mut unknown).unwrap_err().0.contains("unsupported archive magic"));
    }
}
