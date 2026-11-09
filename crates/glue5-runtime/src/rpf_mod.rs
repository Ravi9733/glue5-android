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

/// Applies add operations from one OpenIV archive element. Nested archive
/// elements are followed into nested RPF7 files. Only RPF7 Open/None/AES
/// archives are writable; NG-encrypted archives are rejected without changes.
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
            if replacements.contains_key(&target) || !deletions.insert(target.clone()) {
                bail!("duplicate or conflicting OIV archive target: {target}");
            }
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
            if deletions.contains(&target) || replacements.insert(target.clone(), data).is_some() {
                bail!("duplicate or conflicting OIV archive target: {target}");
            }
        } else {
            bail!("unsupported command inside OIV archive: {}", child.tag_name().name());
        }
    }
    Ok(())
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
        RpfEncryption::Open | RpfEncryption::None => Ok(None),
    }
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
    let archive = RpfArchive::parse(data, "oiv.rpf", keys).context("cannot parse RPF7 archive")?;
    if archive.version != RpfVersion::V7 {
        bail!("only GTA V RPF7 archives can be edited");
    }
    match archive.encryption {
        RpfEncryption::Ng => bail!("NG-encrypted nested RPF cannot be rebuilt"),
        RpfEncryption::Tfit => bail!("unsupported nested RPF encryption"),
        _ => {}
    }

    let tree = build_directory_tree(&archive.entries);
    let files = list_all_files(&tree);
    let mut builder = RpfBuilder::new(archive.encryption);
    let mut applied = 0usize;

    for file in files {
        let entry = &archive.entries[file.entry_index];
        match &entry.kind {
            rpf_archive::RpfEntryKind::BinaryFile {
                file_size, uncompressed_size, is_encrypted, ..
            } => {
                if *is_encrypted || (*file_size > 0 && file_size < uncompressed_size) {
                    bail!("cannot safely rebuild compressed or individually encrypted entry {}", file.path);
                }
            }
            rpf_archive::RpfEntryKind::ResourceFile { is_encrypted, .. } => {
                if *is_encrypted {
                    bail!("cannot safely rebuild individually encrypted resource {}", file.path);
                }
            }
            rpf_archive::RpfEntryKind::Directory { .. } => continue,
        }
        let mut bytes = archive.extract_entry(data, entry, keys)
            .with_context(|| format!("cannot extract {}", file.path))?;
        let full_path = join_archive_path(prefix, &file.path).to_ascii_lowercase();

        if deletions.remove(&full_path) {
            applied += 1;
            continue;
        } else if let Some(replacement) = replacements.remove(&full_path) {
            bytes = replacement;
            applied += 1;
        } else if file.name.to_ascii_lowercase().ends_with(".rpf") {
            let child_prefix = full_path;
            let child_prefix_with_slash = format!("{child_prefix}/");
            if replacements.keys().any(|candidate| candidate.starts_with(&child_prefix_with_slash))
                || deletions.iter().any(|candidate| candidate.starts_with(&child_prefix_with_slash)) {
                let (nested, count) = rewrite_archive(
                    &bytes,
                    &child_prefix,
                    replacements,
                    deletions,
                    keys,
                    depth + 1,
                )?;
                bytes = nested;
                applied += count;
            }
        }
        builder.add_file(&file.path, bytes);
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
            builder.add_file(&relative, bytes);
            applied += 1;
        }
    }

    let rebuilt = builder.build(keys).context("cannot rebuild modified RPF7 archive")?;
    Ok((rebuilt, applied))
}
