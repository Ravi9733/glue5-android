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


/// Copy the raw stored bytes for a previously mounted RPF7 asset.
///
/// The descriptor is revalidated against the archive's current TOC before any
/// bytes are copied. This prevents stale or caller-forged offsets from being
/// used as arbitrary file reads. Only unencrypted/uncompressed binary entries
/// and fixed-size resource entries are supported; resource bytes remain in
/// their stored RSC7 form and require the separate decoder.
pub fn copy_mounted_rpf7_asset_from_reader<R: Read + Seek, W: std::io::Write>(
    reader: &mut R,
    archive_id: &str,
    asset: &crate::MountedAsset,
    output: &mut W,
    max_output_bytes: u64,
) -> Result<u64, ManifestError> {
    let root = inventory_rpf7_open_reader(reader)?;
    copy_mounted_rpf7_asset_from_inventory_at(
        reader, archive_id, 0, root.archive_len, &root.inventory, asset, output, max_output_bytes,
    )
}

/// Read a mounted entry from an RPF7 archive at a known absolute base offset
/// inside a larger backing file. The supplied inventory must have been
/// produced from this exact child archive; its entry descriptor and child
/// bounds are rechecked before the backing-file read.
pub fn copy_mounted_rpf7_asset_from_inventory_at<R: Read + Seek, W: std::io::Write>(
    reader: &mut R,
    archive_id: &str,
    archive_base: u64,
    archive_len: u64,
    inventory: &crate::Rpf7Inventory,
    asset: &crate::MountedAsset,
    output: &mut W,
    max_output_bytes: u64,
) -> Result<u64, ManifestError> {
    use crate::{MountedAssetKind, Rpf7EntryKind};

    if asset.archive != archive_id {
        return Err(ManifestError("mounted asset archive identifier does not match source archive".into()));
    }
    if asset.length == 0 || asset.length > max_output_bytes {
        return Err(ManifestError(format!(
            "mounted asset length is empty or exceeds extraction limit ({} > {})",
            asset.length, max_output_bytes
        )));
    }
    let archive_end = archive_base.checked_add(archive_len)
        .ok_or_else(|| ManifestError("mounted archive backing-file range overflows".into()))?;
    let backing_len = reader.seek(SeekFrom::End(0))
        .map_err(|e| ManifestError(format!("cannot seek to backing file end: {e}")))?;
    if archive_end > backing_len {
        return Err(ManifestError("mounted child archive exceeds backing file bounds".into()));
    }
    let entry = inventory.entries.iter()
        .find(|entry| entry.index == asset.entry_index)
        .ok_or_else(|| ManifestError(format!("mounted RPF7 entry {} does not exist", asset.entry_index)))?;
    let (expected_kind, size, offset_blocks) = match &entry.kind {
        Rpf7EntryKind::BinaryFile { file_size, file_offset_blocks, uncompressed_size, encrypted } => {
            if *encrypted { return Err(ManifestError("mounted RPF7 binary entry is encrypted".into())); }
            if *file_size != 0 && file_size != uncompressed_size {
                return Err(ManifestError("mounted RPF7 binary entry is compressed".into()));
            }
            (MountedAssetKind::Binary,
             if *file_size == 0 { u64::from(*uncompressed_size) } else { u64::from(*file_size) },
             u64::from(*file_offset_blocks))
        }
        Rpf7EntryKind::ResourceFile { file_size, file_offset_blocks, .. } => {
            if *file_size == 0x00FF_FFFF {
                return Err(ManifestError("mounted RPF7 resource size sentinel must be resolved before reading".into()));
            }
            (MountedAssetKind::Resource, u64::from(*file_size), u64::from(*file_offset_blocks))
        }
        Rpf7EntryKind::Directory { .. } => return Err(ManifestError("mounted RPF7 entry is a directory".into())),
    };
    let relative_offset = offset_blocks.checked_mul(512)
        .ok_or_else(|| ManifestError("mounted RPF7 payload offset overflows".into()))?;
    let relative_end = relative_offset.checked_add(size)
        .ok_or_else(|| ManifestError("mounted RPF7 payload end overflows".into()))?;
    let metadata_end = 16_u64.checked_add(inventory.header.toc_bytes)
        .and_then(|n| n.checked_add(u64::from(inventory.header.names_length)))
        .ok_or_else(|| ManifestError("RPF7 metadata boundary overflows".into()))?;
    if relative_offset < metadata_end || relative_end > archive_len {
        return Err(ManifestError("mounted RPF7 payload range is outside child archive bounds".into()));
    }
    let expected_offset = archive_base.checked_add(relative_offset)
        .ok_or_else(|| ManifestError("mounted RPF7 absolute payload offset overflows".into()))?;
    if asset.kind != expected_kind || asset.offset != expected_offset || asset.length != size {
        return Err(ManifestError("mounted asset descriptor does not match the RPF7 TOC entry".into()));
    }

    reader.seek(SeekFrom::Start(expected_offset))
        .map_err(|e| ManifestError(format!("cannot seek to mounted RPF7 payload: {e}")))?;
    let mut remaining = size;
    let mut buffer = [0_u8; 64 * 1024];
    while remaining > 0 {
        let count = usize::try_from(remaining.min(buffer.len() as u64)).unwrap();
        reader.read_exact(&mut buffer[..count])
            .map_err(|e| ManifestError(format!("cannot read mounted RPF7 payload: {e}")))?;
        output.write_all(&buffer[..count])
            .map_err(|e| ManifestError(format!("cannot write mounted RPF7 payload: {e}")))?;
        remaining -= count as u64;
    }
    Ok(size)
}

/// Decode one RSC7 resource directly from a file-backed archive. The legacy
/// 0xFFFFFF size sentinel is resolved from its bounded 16-byte resource header.
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
    let (stored_size, offset_blocks) = match &entry.kind {
        Rpf7EntryKind::ResourceFile { file_size, file_offset_blocks, .. } =>
            (u64::from(*file_size), u64::from(*file_offset_blocks)),
        Rpf7EntryKind::BinaryFile { .. } => return Err(ManifestError("TOC entry is not an RSC7 resource".into())),
        Rpf7EntryKind::Directory { .. } => return Err(ManifestError("RPF7 directory has no payload".into())),
    };
    let start = offset_blocks.checked_mul(512)
        .ok_or_else(|| ManifestError("RPF7 resource offset overflow".into()))?;
    let metadata_end = 16_u64 + inventory.inventory.header.toc_bytes
        + u64::from(inventory.inventory.header.names_length);
    let size = if stored_size == 0x00FF_FFFF {
        let header_end = start.checked_add(16)
            .ok_or_else(|| ManifestError("RPF7 resource header end overflow".into()))?;
        if start < metadata_end || header_end > inventory.archive_len {
            return Err(ManifestError("RPF7 resource size sentinel header is outside archive payload bounds".into()));
        }
        reader.seek(SeekFrom::Start(start))
            .map_err(|e| ManifestError(format!("cannot seek to RPF7 resource header: {e}")))?;
        let mut header = [0_u8; 16];
        reader.read_exact(&mut header)
            .map_err(|e| ManifestError(format!("cannot read RPF7 resource size header: {e}")))?;
        if &header[..4] != b"RSC7" {
            return Err(ManifestError("RPF7 resource size sentinel does not point to an RSC7 header".into()));
        }
        u64::from(header[7]) | (u64::from(header[14]) << 8)
            | (u64::from(header[5]) << 16) | (u64::from(header[2]) << 24)
    } else { stored_size };
    let end = start.checked_add(size)
        .ok_or_else(|| ManifestError("RPF7 resource end overflow".into()))?;
    if size < 16 || start < metadata_end || end > inventory.archive_len {
        return Err(ManifestError("RPF7 resource range is truncated, overlaps metadata, or exceeds archive bounds".into()));
    }
    decode_rsc7_resource_stream(reader, start, size, output, max_output_bytes)
}


/// Recursively inventory nested archives from a seekable file without loading
/// the parent archive payload into memory. Child TOC/name metadata is bounded.
pub fn scan_nested_rpf7_open_reader_recursive<R: Read + Seek>(
    reader: &mut R, root: &Rpf7FileInventory, max_depth: usize, max_archives: usize,
) -> Result<Vec<crate::NestedRpfReport>, ManifestError> {
    use crate::{NestedRpfReport, NestedRpfStatus, Rpf7EntryKind};
    fn walk<R: Read + Seek>(
        reader: &mut R, base: u64, archive_len: u64, inventory: &Rpf7Inventory,
        depth: usize, max_depth: usize, remaining: &mut usize, prefix: &str,
        out: &mut Vec<NestedRpfReport>,
    ) -> Result<(), ManifestError> {
        if depth >= max_depth || *remaining == 0 { return Ok(()); }
        let metadata_end = 16_u64.checked_add(inventory.header.toc_bytes)
            .and_then(|n| n.checked_add(u64::from(inventory.header.names_length)))
            .ok_or_else(|| ManifestError("nested RPF7 metadata boundary overflow".into()))?;
        for entry in &inventory.entries {
            if *remaining == 0 { break; }
            if !entry.name.to_ascii_lowercase().ends_with(".rpf") { continue; }
            let (stored, offset, unpacked, encrypted) = match &entry.kind {
                Rpf7EntryKind::BinaryFile { file_size, file_offset_blocks, uncompressed_size, encrypted } =>
                    (u64::from(*file_size), u64::from(*file_offset_blocks), u64::from(*uncompressed_size), *encrypted),
                _ => continue,
            };
            let path = if prefix.is_empty() { entry.name.clone() } else { format!("{prefix}/{}", entry.name) };
            *remaining -= 1;
            let mut status = if encrypted { NestedRpfStatus::SkippedEncryptedPayload }
                else if stored != unpacked { NestedRpfStatus::SkippedCompressedPayload }
                else { NestedRpfStatus::InvalidPayloadRange };
            let relative = offset.checked_mul(512);
            let start = relative.and_then(|v| base.checked_add(v));
            let end = start.and_then(|v| v.checked_add(stored));
            let mut child_inventory = None;
            if !encrypted && stored == unpacked {
                if let (Some(relative), Some(start), Some(end)) = (relative, start, end) {
                    if relative >= metadata_end && end <= base.saturating_add(archive_len) && stored >= 16 {
                        reader.seek(SeekFrom::Start(start)).map_err(|e| ManifestError(format!("cannot seek nested RPF7: {e}")))?;
                        let mut h = [0_u8; 16];
                        reader.read_exact(&mut h).map_err(|e| ManifestError(format!("cannot read nested RPF7 header: {e}")))?;
                        match crate::probe_rpf_header(&h)? {
                            RpfProbe::GtaVLegacy(header) => {
                                let metadata_len = 16_u64.checked_add(header.toc_bytes)
                                    .and_then(|v| v.checked_add(u64::from(header.names_length)))
                                    .ok_or_else(|| ManifestError("nested RPF7 metadata length overflow".into()))?;
                                if metadata_len <= stored {
                                    let len = usize::try_from(metadata_len).map_err(|_| ManifestError("nested RPF7 metadata too large".into()))?;
                                    let mut metadata = vec![0_u8; len];
                                    reader.seek(SeekFrom::Start(start)).map_err(|e| ManifestError(format!("cannot rewind nested RPF7: {e}")))?;
                                    reader.read_exact(&mut metadata).map_err(|e| ManifestError(format!("cannot read nested RPF7 metadata: {e}")))?;
                                    match crate::inventory_rpf7_open(&metadata) {
                                        Ok(value) => { status = NestedRpfStatus::InventoriedRpf7(value.clone()); child_inventory = Some(value); }
                                        Err(e) if e.0.contains("AES/NG keys are required") => status = NestedRpfStatus::EncryptedTocUnsupported,
                                        Err(_) => status = NestedRpfStatus::InvalidRpf7Toc,
                                    }
                                }
                            }
                            RpfProbe::GtaVEnhancedOrRdr2 { .. } => status = NestedRpfStatus::Rpf8Unsupported,
                            RpfProbe::UnsupportedOrUnknown { .. } => status = NestedRpfStatus::NotAnRpfArchive,
                        }
                    }
                }
            }
            out.push(NestedRpfReport { parent_entry_index: entry.index, name: path.clone(), status });
            if let (Some(child), Some(start)) = (child_inventory, start) {
                if depth + 1 < max_depth && *remaining > 0 {
                    walk(reader, start, stored, &child, depth + 1, max_depth, remaining, &path, out)?;
                }
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    let mut remaining = max_archives;
    walk(reader, 0, root.archive_len, &root.inventory, 0, max_depth, &mut remaining, "", &mut out)?;
    Ok(out)
}


/// Discover and register a root RPF7 archive plus eligible nested RPF7 DLC
/// archives in one case-insensitive virtual mount index.
///
/// The root archive is mounted beneath the supplied virtual prefix. Each
/// nested archive is mounted beneath a directory named after its RPF filename
/// stem (for example, dlc.rpf contributes assets under dlc/). All payload
/// offsets are absolute offsets in the same seekable backing file. Only
/// unencrypted, byte-for-byte stored RPF7 children are traversed; archive
/// depth and total child count are caller bounded.
pub fn mount_rpf7_open_reader_tree<R: Read + Seek>(
    reader: &mut R,
    root: &Rpf7FileInventory,
    root_archive_id: &str,
    virtual_prefix: &str,
    max_depth: usize,
    max_archives: usize,
    mounts: &mut crate::AssetMountIndex,
) -> Result<usize, ManifestError> {
    use crate::add_rpf7_inventory_to_mount_at;

    fn join_virtual_prefix(parent: &str, child: &str) -> String {
        if parent.is_empty() { child.to_owned() }
        else if child.is_empty() { parent.to_owned() }
        else { format!("{parent}/{child}") }
    }

    fn walk<R: Read + Seek>(
        reader: &mut R,
        base: u64,
        archive_len: u64,
        inventory: &Rpf7Inventory,
        archive_id: &str,
        virtual_prefix: &str,
        depth: usize,
        max_depth: usize,
        remaining: &mut usize,
        mounts: &mut crate::AssetMountIndex,
        added: &mut usize,
    ) -> Result<(), ManifestError> {
        use crate::{add_rpf7_inventory_to_mount_at, Rpf7EntryKind};

        if depth >= max_depth || *remaining == 0 { return Ok(()); }
        let metadata_end = 16_u64.checked_add(inventory.header.toc_bytes)
            .and_then(|n| n.checked_add(u64::from(inventory.header.names_length)))
            .ok_or_else(|| ManifestError("nested RPF7 metadata boundary overflow".into()))?;
        let archive_end = base.checked_add(archive_len)
            .ok_or_else(|| ManifestError("nested RPF7 archive end overflow".into()))?;

        for entry in &inventory.entries {
            if *remaining == 0 { break; }
            if !entry.name.to_ascii_lowercase().ends_with(".rpf") { continue; }

            let (stored, offset_blocks, unpacked, encrypted) = match &entry.kind {
                Rpf7EntryKind::BinaryFile {
                    file_size, file_offset_blocks, uncompressed_size, encrypted,
                } => (
                    u64::from(*file_size),
                    u64::from(*file_offset_blocks),
                    u64::from(*uncompressed_size),
                    *encrypted,
                ),
                _ => continue,
            };
            *remaining -= 1;
            if encrypted || stored == 0 || stored != unpacked { continue; }

            let relative = offset_blocks.checked_mul(512)
                .ok_or_else(|| ManifestError("nested RPF7 child offset overflow".into()))?;
            let child_base = base.checked_add(relative)
                .ok_or_else(|| ManifestError("nested RPF7 child base overflow".into()))?;
            let child_end = child_base.checked_add(stored)
                .ok_or_else(|| ManifestError("nested RPF7 child end overflow".into()))?;
            if relative < metadata_end || child_end > archive_end || stored < 16 { continue; }

            reader.seek(SeekFrom::Start(child_base))
                .map_err(|e| ManifestError(format!("cannot seek nested RPF7 header: {e}")))?;
            let mut header_bytes = [0_u8; 16];
            reader.read_exact(&mut header_bytes)
                .map_err(|e| ManifestError(format!("cannot read nested RPF7 header: {e}")))?;
            let header = match crate::probe_rpf_header(&header_bytes)? {
                RpfProbe::GtaVLegacy(header) => header,
                _ => continue,
            };
            let metadata_len = 16_u64.checked_add(header.toc_bytes)
                .and_then(|n| n.checked_add(u64::from(header.names_length)))
                .ok_or_else(|| ManifestError("nested RPF7 metadata length overflow".into()))?;
            if metadata_len > stored { continue; }
            let metadata_size = usize::try_from(metadata_len)
                .map_err(|_| ManifestError("nested RPF7 metadata does not fit this platform".into()))?;
            let mut metadata = Vec::new();
            metadata.try_reserve_exact(metadata_size)
                .map_err(|e| ManifestError(format!("cannot allocate nested RPF7 metadata: {e}")))?;
            metadata.resize(metadata_size, 0);
            reader.seek(SeekFrom::Start(child_base))
                .map_err(|e| ManifestError(format!("cannot rewind nested RPF7 metadata: {e}")))?;
            reader.read_exact(&mut metadata)
                .map_err(|e| ManifestError(format!("cannot read nested RPF7 metadata: {e}")))?;
            let child_inventory = match crate::inventory_rpf7_open(&metadata) {
                Ok(value) => value,
                Err(_) => continue,
            };

            let child_id = format!("{archive_id}/{}", entry.name);
            let stem = entry.name[..entry.name.len() - 4].to_owned();
            let child_prefix = join_virtual_prefix(virtual_prefix, &stem);
            *added += add_rpf7_inventory_to_mount_at(
                mounts,
                &child_id,
                &child_prefix,
                child_base,
                stored,
                &child_inventory,
            ).map_err(|e| ManifestError(format!("cannot mount nested archive {}: {}", entry.name, e)))?;

            if depth + 1 < max_depth && *remaining > 0 {
                walk(
                    reader,
                    child_base,
                    stored,
                    &child_inventory,
                    &child_id,
                    &child_prefix,
                    depth + 1,
                    max_depth,
                    remaining,
                    mounts,
                    added,
                )?;
            }
        }
        Ok(())
    }

    if root_archive_id.is_empty() {
        return Err(ManifestError("root archive identifier cannot be empty".into()));
    }
    let backing_len = reader.seek(SeekFrom::End(0))
        .map_err(|e| ManifestError(format!("cannot seek to backing file end: {e}")))?;
    if root.archive_len > backing_len {
        return Err(ManifestError("root RPF7 archive exceeds backing file bounds".into()));
    }

    let mut added = add_rpf7_inventory_to_mount_at(
        mounts,
        root_archive_id,
        virtual_prefix,
        0,
        root.archive_len,
        &root.inventory,
    ).map_err(|e| ManifestError(format!("cannot mount root archive: {e}")))?;

    let mut remaining = max_archives;
    walk(
        reader,
        0,
        root.archive_len,
        &root.inventory,
        root_archive_id,
        virtual_prefix,
        0,
        max_depth,
        &mut remaining,
        mounts,
        &mut added,
    )?;
    Ok(added)
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

    fn empty_rpf7() -> Vec<u8> {
        let mut bytes = vec![0_u8; 16];
        bytes[0..4].copy_from_slice(&0x5250_4637_u32.to_le_bytes());
        bytes[4..8].copy_from_slice(&0_u32.to_le_bytes());
        bytes[8..12].copy_from_slice(&0_u32.to_le_bytes());
        bytes[12..16].copy_from_slice(&0x4E45_504F_u32.to_le_bytes());
        bytes
    }

    fn archive_with_nested(name: &str, nested: &[u8]) -> Vec<u8> {
        let mut names = b"root\0".to_vec();
        names.extend_from_slice(name.as_bytes());
        names.push(0);
        let metadata_len = 16 + 32 + names.len();
        let mut bytes = vec![0_u8; 512 + nested.len()];
        bytes[0..4].copy_from_slice(&0x5250_4637_u32.to_le_bytes());
        bytes[4..8].copy_from_slice(&2_u32.to_le_bytes());
        bytes[8..12].copy_from_slice(&(names.len() as u32).to_le_bytes());
        bytes[12..16].copy_from_slice(&0x4E45_504F_u32.to_le_bytes());
        bytes[16..20].copy_from_slice(&0_u32.to_le_bytes());
        bytes[20..24].copy_from_slice(&0x7FFF_FF00_u32.to_le_bytes());
        bytes[24..28].copy_from_slice(&1_u32.to_le_bytes());
        bytes[28..32].copy_from_slice(&1_u32.to_le_bytes());
        bytes[32..34].copy_from_slice(&5_u16.to_le_bytes());
        let size = nested.len() as u32;
        bytes[34] = size as u8;
        bytes[35] = (size >> 8) as u8;
        bytes[36] = (size >> 16) as u8;
        bytes[37] = 1; // payload starts at archive-relative block 1
        bytes[40..44].copy_from_slice(&size.to_le_bytes());
        bytes[48..48 + names.len()].copy_from_slice(&names);
        assert!(metadata_len <= 512);
        bytes[512..].copy_from_slice(nested);
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
    fn reads_mounted_asset_after_revalidating_its_toc_descriptor() {
        let mut bytes = fixture();
        bytes[512..516].copy_from_slice(b"DATA");
        let asset = crate::MountedAsset {
            archive: "mods/test.rpf".into(),
            entry_index: 1,
            offset: 512,
            length: 4,
            kind: crate::MountedAssetKind::Binary,
        };
        let mut reader = Cursor::new(bytes.clone());
        let mut output = Vec::new();
        let copied = copy_mounted_rpf7_asset_from_reader(
            &mut reader, "mods/test.rpf", &asset, &mut output, 16,
        ).unwrap();
        assert_eq!(copied, 4);
        assert_eq!(output, b"DATA");
    }

    #[test]
    fn reads_mounted_asset_from_nested_archive_at_absolute_offset() {
        let child = fixture();
        let mut backing = vec![0_u8; 1024];
        backing.extend_from_slice(&child);
        backing[1024 + 512..1024 + 516].copy_from_slice(b"CHLD");
        let mut child_reader = Cursor::new(child.clone());
        let child_inventory = inventory_rpf7_open_reader(&mut child_reader).unwrap();
        let asset = crate::MountedAsset {
            archive: "mods/update.rpf".into(), entry_index: 1,
            offset: 1024 + 512, length: 4, kind: crate::MountedAssetKind::Binary,
        };
        let mut reader = Cursor::new(backing);
        let mut output = Vec::new();
        let copied = copy_mounted_rpf7_asset_from_inventory_at(
            &mut reader, "mods/update.rpf", 1024, child.len() as u64,
            &child_inventory.inventory, &asset, &mut output, 16,
        ).unwrap();
        assert_eq!(copied, 4);
        assert_eq!(output, b"CHLD");
    }

    #[test]
    fn rejects_forged_mounted_asset_range_and_wrong_archive() {
        let bytes = fixture();
        let mut asset = crate::MountedAsset {
            archive: "mods/test.rpf".into(),
            entry_index: 1,
            offset: 1024,
            length: 4,
            kind: crate::MountedAssetKind::Binary,
        };
        let mut reader = Cursor::new(bytes.clone());
        let mut output = Vec::new();
        assert!(copy_mounted_rpf7_asset_from_reader(
            &mut reader, "mods/test.rpf", &asset, &mut output, 16,
        ).unwrap_err().0.contains("does not match"));
        assert!(output.is_empty());

        asset.offset = 512;
        let mut reader = Cursor::new(bytes);
        assert!(copy_mounted_rpf7_asset_from_reader(
            &mut reader, "mods/other.rpf", &asset, &mut output, 16,
        ).unwrap_err().0.contains("identifier"));
        assert!(output.is_empty());
    }

    #[test]
    fn enforces_mounted_asset_copy_limit_before_writing() {
        let bytes = fixture();
        let asset = crate::MountedAsset {
            archive: "mods/test.rpf".into(),
            entry_index: 1,
            offset: 512,
            length: 4,
            kind: crate::MountedAssetKind::Binary,
        };
        let mut reader = Cursor::new(bytes);
        let mut output = Vec::new();
        assert!(copy_mounted_rpf7_asset_from_reader(
            &mut reader, "mods/test.rpf", &asset, &mut output, 3,
        ).unwrap_err().0.contains("exceeds extraction limit"));
        assert!(output.is_empty());
    }

    #[test]
    fn resolves_file_backed_resource_size_sentinel_before_decode() {
        let mut bytes = fixture();
        bytes[34..37].copy_from_slice(&[0xFF, 0xFF, 0xFF]);
        bytes[39] |= 0x80;
        bytes[512 + 7] = 16;
        bytes[512..516].copy_from_slice(b"RSC7");
        let mut reader = Cursor::new(bytes);
        let mut output = Vec::new();
        let error = extract_rpf7_rsc7_entry_from_reader(&mut reader, 1, &mut output, 64).unwrap_err();
        assert!(!error.0.contains("sentinel is not supported"), "sentinel was not resolved: {}", error.0);
        assert!(!error.0.contains("outside archive payload bounds"), "resolved range was rejected: {}", error.0);
    }

    #[test]
    fn rejects_file_backed_resource_sentinel_with_truncated_header() {
        let mut bytes = fixture();
        bytes.truncate(520);
        bytes[34..37].copy_from_slice(&[0xFF, 0xFF, 0xFF]);
        bytes[39] |= 0x80;
        let mut reader = Cursor::new(bytes);
        let mut output = Vec::new();
        let error = extract_rpf7_rsc7_entry_from_reader(&mut reader, 1, &mut output, 64).unwrap_err();
        assert!(error.0.contains("sentinel header is outside archive payload bounds"));
        assert!(output.is_empty());
    }

    #[test]
    fn rejects_file_backed_resource_sentinel_with_non_rsc7_header() {
        let mut bytes = fixture();
        bytes[34..37].copy_from_slice(&[0xFF, 0xFF, 0xFF]);
        bytes[39] |= 0x80;
        bytes[512..528].fill(0);
        let mut reader = Cursor::new(bytes);
        let mut output = Vec::new();
        let error = extract_rpf7_rsc7_entry_from_reader(&mut reader, 1, &mut output, 64).unwrap_err();
        assert!(error.0.contains("does not point to an RSC7 header"));
        assert!(output.is_empty());
    }

    #[test]
    fn rejects_file_backed_resource_sentinel_resolving_below_header_size() {
        let mut bytes = fixture();
        bytes[34..37].copy_from_slice(&[0xFF, 0xFF, 0xFF]);
        bytes[39] |= 0x80;
        bytes[512..516].copy_from_slice(b"RSC7");
        let mut reader = Cursor::new(bytes);
        let mut output = Vec::new();
        let error = extract_rpf7_rsc7_entry_from_reader(&mut reader, 1, &mut output, 64).unwrap_err();
        assert!(error.0.contains("resource range is truncated"));
        assert!(output.is_empty());
    }

    #[test]
    fn file_backed_recursive_scan_reports_nested_paths_and_respects_depth() {
        let leaf = empty_rpf7();
        let middle = archive_with_nested("leaf.rpf", &leaf);
        let outer = archive_with_nested("middle.rpf", &middle);
        let mut reader = Cursor::new(outer);
        let root = inventory_rpf7_open_reader(&mut reader).unwrap();

        let reports = scan_nested_rpf7_open_reader_recursive(&mut reader, &root, 2, 8).unwrap();
        assert_eq!(reports.len(), 2);
        assert_eq!(reports[0].name, "middle.rpf");
        assert_eq!(reports[1].name, "middle.rpf/leaf.rpf");

        let reports = scan_nested_rpf7_open_reader_recursive(&mut reader, &root, 1, 8).unwrap();
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].name, "middle.rpf");

        let reports = scan_nested_rpf7_open_reader_recursive(&mut reader, &root, 0, 8).unwrap();
        assert!(reports.is_empty());
    }

    #[test]
    fn file_backed_recursive_scan_enforces_archive_count_limit() {
        let leaf = empty_rpf7();
        let middle = archive_with_nested("leaf.rpf", &leaf);
        let outer = archive_with_nested("middle.rpf", &middle);
        let mut reader = Cursor::new(outer);
        let root = inventory_rpf7_open_reader(&mut reader).unwrap();

        let reports = scan_nested_rpf7_open_reader_recursive(&mut reader, &root, 8, 1).unwrap();
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].name, "middle.rpf");
    }

    #[test]
    fn file_backed_recursive_scan_reports_out_of_bounds_child_payload() {
        let nested = empty_rpf7();
        let mut outer = archive_with_nested("middle.rpf", &nested);
        outer[37] = 0; // points into the archive metadata instead of its payload
        let mut reader = Cursor::new(outer);
        let root = inventory_rpf7_open_reader(&mut reader).unwrap();

        let reports = scan_nested_rpf7_open_reader_recursive(&mut reader, &root, 2, 8).unwrap();
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].name, "middle.rpf");
        assert_eq!(reports[0].status, crate::NestedRpfStatus::InvalidPayloadRange);
    }

    #[test]
    fn automatically_mounts_nested_rpf7_assets_with_absolute_backing_offsets() {
        let mut child = fixture();
        child[512..516].copy_from_slice(b"CHLD");
        let outer = archive_with_nested("middle.rpf", &child);
        let mut reader = Cursor::new(outer);
        let root = inventory_rpf7_open_reader(&mut reader).unwrap();
        let mut mounts = crate::AssetMountIndex::new();

        let added = mount_rpf7_open_reader_tree(
            &mut reader, &root, "mods/root.rpf", "", 2, 8, &mut mounts,
        ).unwrap();
        assert_eq!(added, 2);

        let asset = mounts.resolve("middle/file.bin").unwrap().unwrap().clone();
        assert_eq!(asset.archive, "mods/root.rpf/middle.rpf");
        assert_eq!(asset.offset, 1024);
        assert_eq!(asset.length, 4);

        let child_inventory = inventory_rpf7_open_reader(&mut Cursor::new(child.clone())).unwrap();
        let mut output = Vec::new();
        let copied = copy_mounted_rpf7_asset_from_inventory_at(
            &mut reader,
            &asset.archive,
            512,
            child.len() as u64,
            &child_inventory.inventory,
            &asset,
            &mut output,
            16,
        ).unwrap();
        assert_eq!(copied, 4);
        assert_eq!(output, b"CHLD");
    }

    #[test]
    fn rejects_truncated_metadata_and_non_rpf7_archives() {
        let mut truncated = Cursor::new(fixture()[..48].to_vec());
        assert!(inventory_rpf7_open_reader(&mut truncated).unwrap_err().0.contains("truncated"));
        let mut unknown = Cursor::new(vec![0_u8; 32]);
        assert!(inventory_rpf7_open_reader(&mut unknown).unwrap_err().0.contains("unsupported archive magic"));
    }
}


/// Decode a mounted RSC7 resource using its absolute backing-file range.
/// The descriptor's critical range and kind are rechecked before reading.
pub fn decode_mounted_rsc7_asset_from_reader<R: Read + Seek, W: std::io::Write>(
    reader: &mut R,
    asset: &crate::MountedAsset,
    output: &mut W,
    max_output_bytes: usize,
) -> Result<crate::Rsc7StreamReport, ManifestError> {
    if asset.kind != crate::MountedAssetKind::Resource {
        return Err(ManifestError("mounted asset is not an RSC7 resource".into()));
    }
    if asset.length < 16 {
        return Err(ManifestError("mounted RSC7 resource is shorter than its header".into()));
    }
    let end = asset.offset.checked_add(asset.length)
        .ok_or_else(|| ManifestError("mounted RSC7 backing-file range overflows".into()))?;
    let backing_len = reader.seek(SeekFrom::End(0))
        .map_err(|e| ManifestError(format!("cannot seek to backing file end: {e}")))?;
    if end > backing_len {
        return Err(ManifestError("mounted RSC7 resource exceeds backing-file bounds".into()));
    }
    crate::decode_rsc7_resource_stream(reader, asset.offset, asset.length, output, max_output_bytes)
}


/// Decode a mounted RSC7 resource directly into separate system and graphics
/// page writers. The mounted absolute range is checked against the backing file
/// before streaming; no combined decompressed page buffer is retained.
pub fn decode_mounted_rsc7_asset_pages_from_reader<
    R: Read + Seek, WS: std::io::Write, WG: std::io::Write,
>(
    reader: &mut R,
    asset: &crate::MountedAsset,
    system_output: &mut WS,
    graphics_output: &mut WG,
    max_output_bytes: usize,
) -> Result<crate::Rsc7StreamReport, ManifestError> {
    if asset.kind != crate::MountedAssetKind::Resource {
        return Err(ManifestError("mounted asset is not an RSC7 resource".into()));
    }
    if asset.length < 16 {
        return Err(ManifestError("mounted RSC7 resource is shorter than its header".into()));
    }
    let end = asset.offset.checked_add(asset.length)
        .ok_or_else(|| ManifestError("mounted RSC7 backing-file range overflows".into()))?;
    let backing_len = reader.seek(SeekFrom::End(0))
        .map_err(|e| ManifestError(format!("cannot seek to backing file end: {e}")))?;
    if end > backing_len {
        return Err(ManifestError("mounted RSC7 resource exceeds backing-file bounds".into()));
    }
    crate::decode_rsc7_resource_pages_stream(
        reader, asset.offset, asset.length, system_output, graphics_output, max_output_bytes,
    )
}

#[cfg(test)]
mod mounted_rsc7_decode_tests {
    use super::*;
    use flate2::{write::DeflateEncoder, Compression};
    use std::io::{Cursor, Write};

    fn payload(body: &[u8]) -> Vec<u8> {
        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(body).unwrap();
        let compressed = encoder.finish().unwrap();
        let mut bytes = b"RSC7".to_vec();
        bytes.extend_from_slice(&1_u32.to_le_bytes());
        bytes.extend_from_slice(&(1_u32 << 27).to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(&compressed);
        bytes
    }

    fn descriptor(offset: u64, length: u64, kind: crate::MountedAssetKind) -> crate::MountedAsset {
        crate::MountedAsset { archive: "mods/test.rpf".into(), entry_index: 7, offset, length, kind }
    }

    #[test]
    fn decodes_mounted_resource_at_absolute_backing_offset() {
        let body = vec![0x5A; 512];
        let payload = payload(&body);
        let mut backing = vec![0xCC; 1024];
        backing[384..384 + payload.len()].copy_from_slice(&payload);
        let mut reader = Cursor::new(backing);
        let mut output = Vec::new();
        let report = decode_mounted_rsc7_asset_from_reader(
            &mut reader,
            &descriptor(384, payload.len() as u64, crate::MountedAssetKind::Resource),
            &mut output,
            1024,
        ).unwrap();
        assert_eq!(report.version, 1);
        assert_eq!(report.bytes_written, 512);
        assert_eq!(output, body);
    }

    #[test]
    fn rejects_non_resource_and_out_of_bounds_descriptors() {
        let mut reader = Cursor::new(vec![0_u8; 64]);
        let mut output = Vec::new();
        assert!(decode_mounted_rsc7_asset_from_reader(
            &mut reader, &descriptor(0, 32, crate::MountedAssetKind::Binary), &mut output, 1024,
        ).unwrap_err().0.contains("not an RSC7 resource"));
        assert!(decode_mounted_rsc7_asset_from_reader(
            &mut reader, &descriptor(48, 32, crate::MountedAssetKind::Resource), &mut output, 1024,
        ).unwrap_err().0.contains("backing-file bounds"));
    }

    #[test]
    fn enforces_decompressed_output_cap_for_mounted_resources() {
        let payload = payload(&vec![0x5A; 512]);
        let mut reader = Cursor::new(payload.clone());
        let mut output = Vec::new();
        assert!(decode_mounted_rsc7_asset_from_reader(
            &mut reader,
            &descriptor(0, payload.len() as u64, crate::MountedAssetKind::Resource),
            &mut output,
            128,
        ).unwrap_err().0.contains("exceeds configured limit"));
    }
}
