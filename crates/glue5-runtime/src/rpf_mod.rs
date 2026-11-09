//! Safe OpenIV archive operations for GTA V RPF7 files.
//!
//! The original game data is only read. The caller writes the rebuilt archive
//! to its private staging directory and mounts it through the graphics overlay.

use anyhow::{bail, Context, Result};
use rpf_archive::{
    build_directory_tree, list_all_files, GtaKeys, RpfArchive, RpfBuilder,
    RpfEncryption, RpfVersion,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

const MAX_ARCHIVE_BYTES: u64 = 512 * 1024 * 1024;
const MAX_NESTING: usize = 8;

/// Applies add/delete operations from one OpenIV archive element. Nested archive
/// elements are followed into nested RPF7 files. Only RPF7 Open/None/AES
/// archives are writable; NG-encrypted archives are rejected without changes.
/// Existing compressed/encrypted payloads are copied in their stored form.
pub fn apply_oiv_rpf_archive(
    original_archive: &Path,
    archive_xml: &str,
    content_root: &Path,
    output_archive: &Path,
    keys_dir: Option<&Path>,
) -> Result<usize> {
    let document = roxmltree::Document::parse(archive_xml).context("invalid OIV archive XML")?;
    let root = document.root_element();
    if !root.has_tag_name("archive") {
        bail!("expected an OpenIV archive element");
    }
    if root.attribute("type").is_some_and(|kind| !kind.eq_ignore_ascii_case("rpf7")) {
        bail!("only OpenIV RPF7 archive commands are supported");
    }

    let source_root = content_root.canonicalize().context("cannot resolve OIV content folder")?;
    let mut replacements = BTreeMap::<String, Vec<u8>>::new();
    let mut deletions = BTreeSet::<String>::new();
    collect_oiv_operations(root, "", &source_root, &mut replacements, &mut deletions)?;
    if replacements.is_empty() && deletions.is_empty() {
        bail!("OIV archive contains no supported archive operations");
    }

    let metadata = fs::metadata(original_archive).context("cannot read original RPF archive")?;
    if !metadata.is_file() || metadata.len() > MAX_ARCHIVE_BYTES {
        bail!("RPF archive exceeds the 512 MiB on-device editing limit");
    }
    let original = fs::read(original_archive).context("cannot read original RPF archive")?;
    let keys = load_aes_keys_if_needed(&original, keys_dir)?;
    let (mut rebuilt, applied) = rewrite_archive(
        &original, "", &mut replacements, &mut deletions, keys.as_ref(), 0
    )?;
    if !replacements.is_empty() || !deletions.is_empty() {
        let remaining = replacements.keys().chain(deletions.iter()).cloned().collect::<Vec<_>>().join(", ");
        bail!("some OIV archive targets were not applied: {remaining}");
    }
    if rebuilt.len() > original.len() {
        bail!(
            "rebuilt RPF is {} bytes larger than the original {} bytes; refusing to replace it",
            rebuilt.len(),
            original.len()
        );
    }
    rebuilt.resize(original.len(), 0);
    if let Some(parent) = output_archive.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(output_archive, rebuilt).context("cannot write rebuilt RPF staging file")?;
    Ok(applied)
}

fn collect_oiv_operations(
    node: roxmltree::Node<'_, '_>,
    prefix: &str,
    source_root: &Path,
    replacements: &mut BTreeMap<String, Vec<u8>>,
    deletions: &mut BTreeSet<String>,
) -> Result<()> {
    for child in node.children().filter(roxmltree::Node::is_element) {
        if child.has_tag_name("archive") {
            if child.attribute("type").is_some_and(|kind| !kind.eq_ignore_ascii_case("rpf7")) {
                bail!("only nested RPF7 archives are supported");
            }
            let nested = safe_relative(child.attribute("path").context("nested archive has no path")?)?;
            let path = join_archive_path(prefix, &nested);
            collect_oiv_operations(child, &path, source_root, replacements, deletions)?;
        } else if child.has_tag_name("delete") {
            let target = join_archive_path(prefix, &safe_relative(child.text().unwrap_or("").trim())?)
                .to_ascii_lowercase();
            if replacements.keys().any(|existing| archive_paths_overlap(existing, &target))
                || deletions.iter().any(|existing| archive_paths_overlap(existing, &target))
                || !deletions.insert(target.clone()) {
                bail!("duplicate or conflicting OIV archive target: {target}");
            }
        } else if child.has_tag_name("defragmentation") {
            // Rebuilding the archive compacts its data and table of contents.
            continue;
        } else if child.has_tag_name("add") {
            let source = child.attribute("source").context("OIV add has no source")?;
            let target = safe_relative(child.text().unwrap_or("").trim())?;
            let source_path = safe_source_file(source_root, source)?;
            let data = fs::read(&source_path)
                .with_context(|| format!("cannot read OIV source {}", source_path.display()))?;
            if data.len() as u64 > MAX_ARCHIVE_BYTES {
                bail!("OIV source file is too large: {source}");
            }
            let target = join_archive_path(prefix, &target).to_ascii_lowercase();
            if deletions.iter().any(|existing| archive_paths_overlap(existing, &target))
                || replacements.keys().any(|existing| archive_paths_overlap(existing, &target))
                || replacements.insert(target.clone(), data).is_some() {
                bail!("duplicate or conflicting OIV archive target: {target}");
            }
        } else {
            bail!("unsupported command inside OIV archive: {}", child.tag_name().name());
        }
    }
    Ok(())
}


/// Two archive paths conflict when either operation replaces/deletes the
/// other path or one of its descendants.
fn archive_paths_overlap(left: &str, right: &str) -> bool {
    left == right || left.starts_with(&format!("{right}/")) || right.starts_with(&format!("{left}/"))
}

fn safe_relative(value: &str) -> Result<String> {
    let mut value = value.replace('\\', "/");
    while value.starts_with("./") { value.drain(..2); }
    if value.is_empty() || value.starts_with('/') || value.starts_with("//") {
        bail!("empty or absolute archive path");
    }
    if value.as_bytes().get(1) == Some(&b':') {
        bail!("drive-qualified archive path is not allowed");
    }
    let mut parts = Vec::new();
    for part in value.split('/') {
        if part.is_empty() || part == "." || part == ".." {
            bail!("unsafe archive path component in {value}");
        }
        parts.push(part);
    }
    Ok(parts.join("/"))
}

fn join_archive_path(prefix: &str, path: &str) -> String {
    if prefix.is_empty() { path.to_string() } else { format!("{prefix}/{path}") }
}

fn safe_source_file(root: &Path, relative: &str) -> Result<PathBuf> {
    let relative = safe_relative(relative)?;
    let mut current = root.to_path_buf();
    for component in relative.split('/') {
        let entries = fs::read_dir(&current)
            .with_context(|| format!("cannot read OIV source folder {}", current.display()))?;
        let mut exact = None;
        let mut folded = None;
        for entry in entries {
            let entry = entry?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name == component {
                exact = Some(entry.path());
                break;
            }
            if name.eq_ignore_ascii_case(component) {
                if folded.is_some() {
                    bail!("ambiguous case-insensitive OIV source path: {relative}");
                }
                folded = Some(entry.path());
            }
        }
        current = exact.or(folded).context("OIV source file is missing")?;
        if fs::symlink_metadata(&current)?.file_type().is_symlink() {
            bail!("OIV source path contains a symbolic link");
        }
    }
    let canonical = current.canonicalize().context("cannot resolve OIV source file")?;
    if !canonical.starts_with(root) || !canonical.is_file() {
        bail!("OIV source escapes content folder or is not a file");
    }
    Ok(canonical)
}

fn load_aes_keys_if_needed(data: &[u8], keys_dir: Option<&Path>) -> Result<Option<GtaKeys>> {
    if data.len() < 16 || u32::from_le_bytes(data[0..4].try_into().unwrap()) != 0x5250_4637 {
        bail!("target is not a GTA V RPF7 archive");
    }
    let encryption = RpfEncryption::from_u32(u32::from_le_bytes(data[12..16].try_into().unwrap()));
    match encryption {
        RpfEncryption::Ng => bail!("NG-encrypted RPF7 editing is not supported; original archive was left untouched"),
        RpfEncryption::Tfit => bail!("TFIT encryption is not supported for GTA V RPF7"),
        RpfEncryption::Aes => {
            let directory = keys_dir.context("AES-encrypted RPF needs imported GTA V AES keys")?;
            let key_path = directory.join("gtav_aes_key.dat");
            let bytes = fs::read(&key_path).with_context(|| {
                format!("missing AES key file {}", key_path.display())
            })?;
            let aes_key: [u8; 32] = bytes.try_into()
                .map_err(|_| anyhow::anyhow!("gtav_aes_key.dat must contain exactly 32 bytes"))?;
            Ok(Some(GtaKeys {
                aes_key,
                ng_keys: Vec::new(),
                ng_decrypt_tables: Box::new([[[0u32; 256]; 16]; 17]),
            }))
        }
        RpfEncryption::Open | RpfEncryption::None => {
            let key_path = keys_dir.map(|directory| directory.join("gtav_aes_key.dat"));
            if let Some(key_path) = key_path.filter(|path| path.is_file()) {
                let bytes = fs::read(&key_path)?;
                let aes_key: [u8; 32] = bytes.try_into()
                    .map_err(|_| anyhow::anyhow!("gtav_aes_key.dat must contain exactly 32 bytes"))?;
                Ok(Some(GtaKeys {
                    aes_key,
                    ng_keys: Vec::new(),
                    ng_decrypt_tables: Box::new([[[0u32; 256]; 16]; 17]),
                }))
            } else {
                Ok(None)
            }
        }
    }
}


/// RPF7 file offsets are measured in 512-byte blocks. Copying the stored
/// bytes preserves existing compression and per-entry encryption verbatim.
fn raw_entry_bytes(data: &[u8], block_offset: u32, size: u32) -> Result<Vec<u8>> {
    let start = (block_offset as usize).checked_mul(512)
        .context("RPF7 file offset overflow")?;
    let end = start.checked_add(size as usize).context("RPF7 file size overflow")?;
    Ok(data.get(start..end).context("RPF7 stored entry is out of bounds")?.to_vec())
}

fn rewrite_archive(
    data: &[u8],
    prefix: &str,
    replacements: &mut BTreeMap<String, Vec<u8>>,
    deletions: &mut BTreeSet<String>,
    keys: Option<&GtaKeys>,
    depth: usize,
) -> Result<(Vec<u8>, usize)> {
    if depth > MAX_NESTING {
        bail!("nested RPF depth exceeds {MAX_NESTING}");
    }
    if data.len() as u64 > MAX_ARCHIVE_BYTES {
        bail!("nested RPF exceeds the 512 MiB on-device editing limit");
    }
    if data.len() < 16 || u32::from_le_bytes(data[0..4].try_into().unwrap()) != 0x5250_4637 {
        bail!("nested target is not a GTA V RPF7 archive");
    }
    let encryption = RpfEncryption::from_u32(u32::from_le_bytes(data[12..16].try_into().unwrap()));
    match encryption {
        RpfEncryption::Ng => bail!("NG-encrypted nested RPF cannot be rebuilt"),
        RpfEncryption::Tfit => bail!("unsupported nested RPF encryption"),
        RpfEncryption::Aes if keys.is_none() => bail!("nested AES-encrypted RPF needs an AES key"),
        _ => {}
    }
    let archive = RpfArchive::parse(data, "oiv.rpf", keys).context("cannot parse RPF7 archive")?;
    if archive.version != RpfVersion::V7 {
        bail!("only GTA V RPF7 archives can be edited");
    }

    let tree = build_directory_tree(&archive.entries);
    let files = list_all_files(&tree);
    let mut builder = RpfBuilder::new(archive.encryption);
    let mut applied = 0usize;
    let mut applied_deletions = BTreeSet::<String>::new();

    for file in files {
        let entry = &archive.entries[file.entry_index];
        let full_path = join_archive_path(prefix, &file.path).to_ascii_lowercase();

        if let Some(target) = deletions.iter().find(|target| {
            full_path == target.as_str() || full_path.starts_with(&format!("{}/", target.as_str()))
        }) {
            applied_deletions.insert(target.clone());
            continue;
        }

        if let Some(replacement) = replacements.remove(&full_path) {
            let is_rsc7 = replacement.len() >= 16
                && u32::from_le_bytes(replacement[0..4].try_into().unwrap()) == rpf_archive::RSC7_MAGIC;
            match &entry.kind {
                rpf_archive::RpfEntryKind::BinaryFile { is_encrypted, .. } => {
                    if is_rsc7 {
                        bail!("replacement type does not match binary entry {}", file.path);
                    }
                    if *is_encrypted {
                        builder.add_encrypted_binary(&file.path, replacement, keys)?;
                    } else {
                        builder.add_file(&file.path, replacement);
                    }
                }
                rpf_archive::RpfEntryKind::ResourceFile { is_encrypted, .. } => {
                    if !is_rsc7 {
                        bail!("replacement for resource {} is not an RSC7 resource", file.path);
                    }
                    if *is_encrypted || file.name.to_ascii_lowercase().ends_with(".ysc") {
                        builder.add_encrypted_resource(&file.path, replacement, keys)?;
                    } else {
                        builder.add_file(&file.path, replacement);
                    }
                }
                rpf_archive::RpfEntryKind::Directory { .. } => continue,
            }
            applied += 1;
            continue;
        }

        if file.name.to_ascii_lowercase().ends_with(".rpf") {
            let child_prefix = full_path.clone();
            let child_prefix_with_slash = format!("{child_prefix}/");
            if replacements.keys().any(|candidate| candidate.starts_with(&child_prefix_with_slash))
                || deletions.iter().any(|candidate| candidate.starts_with(&child_prefix_with_slash)) {
                let nested_encrypted = matches!(
                    &entry.kind,
                    rpf_archive::RpfEntryKind::BinaryFile { is_encrypted: true, .. }
                );
                let nested_bytes = archive.extract_entry(data, entry, keys)
                    .with_context(|| format!("cannot extract nested archive {}", file.path))?;
                let (nested, count) = rewrite_archive(
                    &nested_bytes,
                    &child_prefix,
                    replacements,
                    deletions,
                    keys,
                    depth + 1,
                )?;
                if nested_encrypted {
                    builder.add_encrypted_binary(&file.path, nested, keys)?;
                } else {
                    builder.add_file(&file.path, nested);
                }
                applied += count;
                continue;
            }
        }

        match &entry.kind {
            rpf_archive::RpfEntryKind::BinaryFile {
                file_offset, file_size, uncompressed_size, is_encrypted
            } => {
                let stored_size = if *file_size > 0 { *file_size } else { *uncompressed_size };
                let stored = raw_entry_bytes(data, *file_offset, stored_size)
                    .with_context(|| format!("cannot preserve stored payload {}", file.path))?;
                builder.add_stored_binary(&file.path, stored, *uncompressed_size, *is_encrypted);
            }
            rpf_archive::RpfEntryKind::ResourceFile {
                file_offset, file_size, system_flags, graphics_flags, ..
            } => {
                let stored = raw_entry_bytes(data, *file_offset, *file_size)
                    .with_context(|| format!("cannot preserve resource payload {}", file.path))?;
                builder.add_stored_resource(&file.path, stored, *system_flags, *graphics_flags);
            }
            rpf_archive::RpfEntryKind::Directory { .. } => {}
        }
    }

    // Count each delete command once, even when it removes an entire directory.
    for target in applied_deletions {
        if deletions.remove(&target) {
            applied += 1;
        }
    }

    // Add new files belonging directly to this archive. Targets containing an
    // intermediate RPF path are consumed by the recursive archive branch.
    let archive_prefix = if prefix.is_empty() { String::new() } else { format!("{}/", prefix.to_ascii_lowercase()) };
    let pending = replacements.keys()
        .filter_map(|key| {
            let relative = if archive_prefix.is_empty() {
                key.as_str()
            } else {
                key.strip_prefix(&archive_prefix)?
            };
            let has_intermediate_rpf = relative.split('/').take(relative.split('/').count().saturating_sub(1))
                .any(|part| part.to_ascii_lowercase().ends_with(".rpf"));
            (!relative.is_empty() && !has_intermediate_rpf)
                .then(|| (key.clone(), relative.to_string()))
        })
        .collect::<Vec<_>>();
    for (key, relative) in pending {
        if let Some(bytes) = replacements.remove(&key) {
            if relative.to_ascii_lowercase().ends_with(".ysc") {
                builder.add_encrypted_resource(&relative, bytes, keys)?;
            } else {
                builder.add_file(&relative, bytes);
            }
            applied += 1;
        }
    }

    let rebuilt = builder.build(keys).context("cannot rebuild modified RPF7 archive")?;
    Ok((rebuilt, applied))
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir() -> PathBuf {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("glue5-rpf-test-{}-{nonce}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn applies_loose_file_replacement_inside_rpf7() {
        let root = temp_dir();
        let content = root.join("content");
        fs::create_dir_all(&content).unwrap();
        fs::write(content.join("replacement.bin"), b"new-data").unwrap();

        let mut builder = RpfBuilder::new(RpfEncryption::Open);
        builder.add_file("common/data/example.bin", b"old-data".to_vec());
        let original = builder.build(None).unwrap();
        let original_path = root.join("original.rpf");
        fs::write(&original_path, &original).unwrap();

        let output_path = root.join("rebuilt.rpf");
        let xml = r#"<archive path="update/update.rpf"><add source="replacement.bin">common/data/example.bin</add></archive>"#;
        let count = apply_oiv_rpf_archive(&original_path, xml, &content, &output_path, None).unwrap();
        assert_eq!(count, 1);

        let rebuilt = fs::read(&output_path).unwrap();
        assert_eq!(rebuilt.len(), original.len());
        let archive = RpfArchive::parse(&rebuilt, "rebuilt.rpf", None).unwrap();
        let entry = archive.entries.iter().find(|entry| entry.name_lower == "example.bin").unwrap();
        assert_eq!(archive.extract_entry(&rebuilt, entry, None).unwrap(), b"new-data".to_vec());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn deletes_an_entry_inside_rpf7() {
        let root = temp_dir();
        let content = root.join("content");
        fs::create_dir_all(&content).unwrap();

        let mut builder = RpfBuilder::new(RpfEncryption::Open);
        builder.add_file("common/data/remove.bin", b"remove".to_vec());
        builder.add_file("common/data/keep.bin", b"keep".to_vec());
        let original = builder.build(None).unwrap();
        let original_path = root.join("original.rpf");
        fs::write(&original_path, &original).unwrap();

        let output_path = root.join("rebuilt.rpf");
        let xml = r#"<archive path="update/update.rpf"><delete>common/data/remove.bin</delete></archive>"#;
        let count = apply_oiv_rpf_archive(&original_path, xml, &content, &output_path, None).unwrap();
        assert_eq!(count, 1);

        let rebuilt = fs::read(&output_path).unwrap();
        let archive = RpfArchive::parse(&rebuilt, "rebuilt.rpf", None).unwrap();
        assert!(!archive.entries.iter().any(|entry| entry.name_lower == "remove.bin"));
        assert!(archive.entries.iter().any(|entry| entry.name_lower == "keep.bin"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn deletes_all_files_beneath_a_directory_target() {
        let root = temp_dir();
        let content = root.join("content");
        fs::create_dir_all(&content).unwrap();

        let mut builder = RpfBuilder::new(RpfEncryption::Open);
        builder.add_file("common/data/remove/one.bin", b"one".to_vec());
        builder.add_file("common/data/remove/two.bin", b"two".to_vec());
        builder.add_file("common/data/keep.bin", b"keep".to_vec());
        let original = builder.build(None).unwrap();
        let original_path = root.join("original.rpf");
        fs::write(&original_path, &original).unwrap();

        let output_path = root.join("rebuilt.rpf");
        let xml = r#"<archive path="update/update.rpf"><delete>common/data/remove</delete></archive>"#;
        let count = apply_oiv_rpf_archive(&original_path, xml, &content, &output_path, None).unwrap();
        assert_eq!(count, 1);

        let rebuilt = fs::read(&output_path).unwrap();
        let archive = RpfArchive::parse(&rebuilt, "rebuilt.rpf", None).unwrap();
        assert!(!archive.entries.iter().any(|entry| entry.name_lower == "one.bin" || entry.name_lower == "two.bin"));
        assert!(archive.entries.iter().any(|entry| entry.name_lower == "keep.bin"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_overlapping_add_and_delete_paths() {
        assert!(archive_paths_overlap("common/data/folder", "common/data/folder/file.ytd"));
        assert!(archive_paths_overlap("common/data/folder/file.ytd", "common/data/folder"));
        assert!(!archive_paths_overlap("common/data/a.ytd", "common/data/b.ytd"));
    }

    #[test]
    fn preserves_compressed_entries_while_editing_another_file() {
        let root = temp_dir();
        let content = root.join("content");
        fs::create_dir_all(&content).unwrap();
        fs::write(content.join("replace.bin"), b"new-data").unwrap();

        let large_payload = vec![b'Q'; 32 * 1024];
        let mut builder = RpfBuilder::new(RpfEncryption::Open);
        builder.add_file("common/data/compressed.bin", large_payload.clone());
        builder.add_file("common/data/target.bin", b"old-data".to_vec());
        let original = builder.build(None).unwrap();
        let original_path = root.join("original.rpf");
        fs::write(&original_path, &original).unwrap();

        let output_path = root.join("rebuilt.rpf");
        let xml = r#"<archive path="update/update.rpf"><add source="replace.bin">common/data/target.bin</add></archive>"#;
        let count = apply_oiv_rpf_archive(&original_path, xml, &content, &output_path, None).unwrap();
        assert_eq!(count, 1);

        let rebuilt = fs::read(&output_path).unwrap();
        let archive = RpfArchive::parse(&rebuilt, "rebuilt.rpf", None).unwrap();
        let compressed = archive.entries.iter().find(|entry| entry.name_lower == "compressed.bin").unwrap();
        assert_eq!(archive.extract_entry(&rebuilt, compressed, None).unwrap(), large_payload);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn replaces_open_archive_ysc_resource_without_losing_resource_flags() {
        let root = temp_dir();
        let content = root.join("content");
        fs::create_dir_all(&content).unwrap();
        let mut replacement = Vec::new();
        replacement.extend_from_slice(&rpf_archive::RSC7_MAGIC.to_le_bytes());
        replacement.extend_from_slice(&0x12u32.to_le_bytes());
        replacement.extend_from_slice(&0x10000000u32.to_le_bytes());
        replacement.extend_from_slice(&0x20000000u32.to_le_bytes());
        replacement.extend_from_slice(b"new-script");
        fs::write(content.join("main.ysc"), &replacement).unwrap();

        let mut original_resource = replacement.clone();
        original_resource[16..].copy_from_slice(b"old-script");
        let mut builder = RpfBuilder::new(RpfEncryption::Open);
        builder.add_file("scripts/main.ysc", original_resource);
        let original = builder.build(None).unwrap();
        let original_path = root.join("original.rpf");
        fs::write(&original_path, &original).unwrap();

        let output_path = root.join("rebuilt.rpf");
        let xml = r#"<archive path="update/update.rpf"><add source="main.ysc">scripts/main.ysc</add></archive>"#;
        let count = apply_oiv_rpf_archive(&original_path, xml, &content, &output_path, None).unwrap();
        assert_eq!(count, 1);

        let rebuilt = fs::read(&output_path).unwrap();
        let archive = RpfArchive::parse(&rebuilt, "rebuilt.rpf", None).unwrap();
        let entry = archive.entries.iter().find(|entry| entry.name_lower == "main.ysc").unwrap();
        assert_eq!(archive.extract_entry(&rebuilt, entry, None).unwrap(), replacement);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn applies_file_replacement_inside_nested_rpf7() {
        let root = temp_dir();
        let content = root.join("content");
        fs::create_dir_all(&content).unwrap();
        fs::write(content.join("car.yft"), b"new-car").unwrap();

        let mut nested_builder = RpfBuilder::new(RpfEncryption::Open);
        nested_builder.add_file("car.yft", b"old-car".to_vec());
        let nested = nested_builder.build(None).unwrap();

        let mut root_builder = RpfBuilder::new(RpfEncryption::Open);
        root_builder.add_file("x64/vehicles.rpf", nested);
        root_builder.add_file("common/data/keep.dat", b"keep".to_vec());
        let original = root_builder.build(None).unwrap();
        let original_path = root.join("original.rpf");
        fs::write(&original_path, &original).unwrap();

        let output_path = root.join("rebuilt.rpf");
        let xml = r#"<archive path="update/update.rpf"><archive path="x64/vehicles.rpf"><add source="car.yft">car.yft</add></archive></archive>"#;
        let count = apply_oiv_rpf_archive(&original_path, xml, &content, &output_path, None).unwrap();
        assert_eq!(count, 1);

        let rebuilt = fs::read(&output_path).unwrap();
        assert_eq!(rebuilt.len(), original.len());
        let archive = RpfArchive::parse(&rebuilt, "rebuilt.rpf", None).unwrap();
        let nested_entry = archive.entries.iter().find(|entry| entry.name_lower == "vehicles.rpf").unwrap();
        let nested_bytes = archive.extract_entry(&rebuilt, nested_entry, None).unwrap();
        let nested_archive = RpfArchive::parse(&nested_bytes, "vehicles.rpf", None).unwrap();
        let car = nested_archive.entries.iter().find(|entry| entry.name_lower == "car.yft").unwrap();
        assert_eq!(nested_archive.extract_entry(&nested_bytes, car, None).unwrap(), b"new-car".to_vec());
        fs::remove_dir_all(root).unwrap();
    }

}
