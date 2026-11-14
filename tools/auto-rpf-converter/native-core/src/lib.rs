//! Bounded RPF7 inspection, extraction and OPEN RPF7 repacking primitives.
//!
//! The Android app's C++ ragekit layer performs the supported Gen9-to-Legacy
//! asset conversions. This crate validates RSC7 headers and builds the new
//! archive; neither layer alone proves compatibility with a particular Android
//! game runtime. Source archives are never modified and unsafe workloads fail closed.

use anyhow::{bail, Context, Result};
use rpf_archive::{build_directory_tree, list_all_files, RpfArchive};

pub mod android_jni;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub max_archive_bytes: u64,
    pub max_entries: usize,
    pub max_entry_bytes: u64,
    pub max_total_output_bytes: u64,
    /// Total input bytes accepted by the in-memory RPF7 writer.
    pub max_total_pack_input_bytes: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_archive_bytes: 1024 * 1024 * 1024,
            max_entries: 1_000_000,
            max_entry_bytes: 512 * 1024 * 1024,
            max_total_output_bytes: 8 * 1024 * 1024 * 1024,
            // rage-rpf's builder holds source buffers and a completed output buffer in memory.
            // Keep the aggregate pack input conservative for 8 GB Android devices.
            max_total_pack_input_bytes: 512 * 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ArchiveReport {
    pub version: String,
    pub encryption: String,
    pub entries: usize,
    pub files: usize,
    pub directories: usize,
    pub resources: usize,
}

#[derive(Debug, Clone)]
pub struct ExtractionReport {
    pub files_written: usize,
    pub bytes_written: u64,
    pub output_dir: PathBuf,
}

/// Parse archive metadata using the upstream RPF implementation.
/// Encrypted archives fail closed unless valid key support is added explicitly.
pub fn inspect_archive(path: impl AsRef<Path>, limits: Limits) -> Result<ArchiveReport> {
    let path = path.as_ref();
    let metadata = fs::metadata(path).with_context(|| format!("stat {}", path.display()))?;
    if metadata.len() > limits.max_archive_bytes {
        bail!("archive exceeds configured size limit");
    }
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    let archive = RpfArchive::parse(&bytes, &path.display().to_string(), None)
        .context("RPF parser rejected archive (encrypted archives need valid key material)")?;
    if archive.entries.len() > limits.max_entries {
        bail!("archive entry count exceeds configured limit");
    }
    let files = archive.entries.iter().filter(|e| e.is_file()).count();
    let directories = archive.entries.len() - files;
    let resources = archive.entries.iter().filter(|e| {
        matches!(e.kind, rpf_archive::RpfEntryKind::ResourceFile { .. })
    }).count();
    Ok(ArchiveReport {
        version: format!("{:?}", archive.version),
        encryption: format!("{:?}", archive.encryption),
        entries: archive.entries.len(),
        files,
        directories,
        resources,
    })
}

/// Extract a supported archive to a separate destination. Source archive is never modified.
/// Writes are bounded and path components are checked to prevent traversal.
pub fn extract_archive(
    source: impl AsRef<Path>,
    destination: impl AsRef<Path>,
    limits: Limits,
) -> Result<ExtractionReport> {
    let source = source.as_ref();
    let destination = destination.as_ref();
    let metadata = fs::metadata(source).with_context(|| format!("stat {}", source.display()))?;
    if metadata.len() > limits.max_archive_bytes {
        bail!("archive exceeds configured size limit");
    }
    let bytes = fs::read(source).with_context(|| format!("read {}", source.display()))?;
    let archive = RpfArchive::parse(&bytes, &source.display().to_string(), None)
        .context("RPF parser rejected archive (encrypted archives need valid key material)")?;
    if archive.encryption.is_encrypted() {
        bail!("encrypted archive: supply legitimate matching key material");
    }
    if archive.entries.len() > limits.max_entries {
        bail!("archive entry count exceeds configured limit");
    }

    let tree = build_directory_tree(&archive.entries);
    let refs = list_all_files(&tree);
    fs::create_dir_all(destination)?;
    let root = fs::canonicalize(destination)?;
    let mut written = 0usize;
    let mut total = 0u64;

    for file_ref in refs {
        let relative = safe_relative_path(&file_ref.path)?;
        let target = root.join(relative);
        if !target.starts_with(&root) {
            bail!("unsafe archive path rejected");
        }
        let entry = archive.entries.get(file_ref.entry_index)
            .context("directory tree returned invalid entry index")?;
        let payload = archive.extract_entry(&bytes, entry, None)
            .with_context(|| format!("extract {}", file_ref.path))?;
        let size = payload.len() as u64;
        if size > limits.max_entry_bytes {
            bail!("entry exceeds configured extraction limit: {}", file_ref.path);
        }
        total = total.checked_add(size).context("output size overflow")?;
        if total > limits.max_total_output_bytes {
            bail!("total extracted output exceeds configured limit");
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut output = File::create(&target)
            .with_context(|| format!("create {}", target.display()))?;
        output.write_all(&payload)
            .with_context(|| format!("write {}", target.display()))?;
        output.sync_all()?;
        written += 1;
    }

    Ok(ExtractionReport {
        files_written: written,
        bytes_written: total,
        output_dir: root,
    })
}

fn safe_relative_path(raw: &str) -> Result<PathBuf> {
    if raw.contains(':') {
        bail!("drive-qualified archive path rejected: {raw}");
    }
    let normalized = raw.replace('\\', "/");
    let path = Path::new(&normalized);
    let mut clean = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => clean.push(part),
            Component::CurDir => {}
            _ => bail!("unsafe archive path rejected: {raw}"),
        }
    }
    if clean.as_os_str().is_empty() {
        bail!("empty archive path rejected");
    }
    Ok(clean)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_valid_open_rpf7_from_asset_directory() {
        let temp = tempfile::tempdir().unwrap();
        let input = temp.path().join("input");
        std::fs::create_dir_all(input.join("vehicles/democar")).unwrap();
        std::fs::write(input.join("vehicles/democar/readme.txt"), b"test asset").unwrap();
        let output = temp.path().join("converted.rpf");
        let (files, _bytes, converted) =
            build_android_rpf7(&input, &output, Limits::default()).unwrap();
        assert_eq!(files, 1);
        assert_eq!(converted, 0);
        let archive_bytes = std::fs::read(&output).unwrap();
        let archive = rage_rpf::RpfArchive::parse(&archive_bytes, "converted.rpf", None).unwrap();
        assert_eq!(archive.version, rage_rpf::RpfVersion::V7);
        assert!(!archive.encryption.is_encrypted());
        assert!(archive.entries.iter().any(|entry| entry.name_lower == "readme.txt"));
        let app_parser = rpf_archive::RpfArchive::parse(&archive_bytes, "converted.rpf", None).unwrap();
        assert_eq!(app_parser.entries.len(), archive.entries.len());
        assert!(!app_parser.encryption.is_encrypted());
    }

    #[test]
    fn packages_a_small_nested_rpf_as_a_binary_entry_without_truncation() {
        use rage_rpf::{RpfArchive, RpfBuilder, RpfEncryption, RpfVersion};

        let temp = tempfile::tempdir().unwrap();
        let input = temp.path().join("input");
        std::fs::create_dir_all(&input).unwrap();

        let mut nested_builder = RpfBuilder::new(RpfEncryption::Open);
        nested_builder.add_file("nested/readme.txt", b"nested archive payload".to_vec());
        let nested_bytes = nested_builder.build(None).unwrap();
        assert!(nested_bytes.len() < 0xFFFFFF);
        std::fs::write(input.join("nested.rpf"), &nested_bytes).unwrap();

        let output = temp.path().join("outer_android.rpf");
        let (files, _, _) = build_android_rpf7(&input, &output, Limits::default()).unwrap();
        assert_eq!(files, 1);

        let outer_bytes = std::fs::read(&output).unwrap();
        let outer = RpfArchive::parse(&outer_bytes, "outer_android.rpf", None).unwrap();
        let nested_entry = outer.entries.iter()
            .find(|entry| entry.name_lower == "nested.rpf")
            .expect("outer archive must retain nested.rpf");
        let nested_payload = outer.extract_entry(&outer_bytes, nested_entry, None).unwrap();
        let nested = RpfArchive::parse(&nested_payload, "nested.rpf", None).unwrap();
        assert_eq!(nested.version, RpfVersion::V7);
        assert!(nested.entries.iter().any(|entry| entry.name_lower == "readme.txt"));
    }

    #[test]
    fn repacks_rsc7_resources_without_losing_the_header_or_page_flags() {
        let temp = tempfile::tempdir().unwrap();
        let input = temp.path().join("input");
        std::fs::create_dir_all(&input).unwrap();

        // RPF7 stores the resource body plus page flags; extraction must reconstruct
        // the complete 16-byte standalone RSC7 header exactly.
        let mut resource = vec![0u8; 24];
        resource[0..4].copy_from_slice(&0x37435352u32.to_le_bytes());
        resource[4..8].copy_from_slice(&165u32.to_le_bytes());
        resource[8..12].copy_from_slice(&0xA0000002u32.to_le_bytes());
        resource[12..16].copy_from_slice(&0x50000000u32.to_le_bytes());
        resource[16..24].copy_from_slice(b"asset123");
        std::fs::write(input.join("prop.ydr"), &resource).unwrap();

        let output = temp.path().join("resources_android.rpf");
        let (files, _, _) = build_android_rpf7(&input, &output, Limits::default()).unwrap();
        assert_eq!(files, 1);

        let archive_bytes = std::fs::read(&output).unwrap();
        let archive = rage_rpf::RpfArchive::parse(&archive_bytes, "resources_android.rpf", None).unwrap();
        let entry = archive.entries.iter()
            .find(|entry| entry.name_lower == "prop.ydr")
            .expect("resource entry must be present");
        assert!(matches!(entry.kind, rage_rpf::RpfEntryKind::ResourceFile { .. }));
        assert_eq!(archive.extract_entry(&archive_bytes, entry, None).unwrap(), resource);

        let second_parser = rpf_archive::RpfArchive::parse(&archive_bytes, "resources_android.rpf", None).unwrap();
        let second_entry = second_parser.entries.iter()
            .find(|entry| entry.name_lower == "prop.ydr")
            .expect("second parser must find the resource");
        assert!(matches!(second_entry.kind, rpf_archive::RpfEntryKind::ResourceFile { .. }));
        assert_eq!(second_parser.extract_entry(&archive_bytes, second_entry, None).unwrap(), resource);
    }

    #[test]
    fn enforces_the_in_memory_rpf7_pack_budget_before_building() {
        let temp = tempfile::tempdir().unwrap();
        let input = temp.path().join("input");
        std::fs::create_dir_all(&input).unwrap();
        std::fs::write(input.join("file.bin"), b"123456").unwrap();

        let output = temp.path().join("over_budget.rpf");
        let mut limits = Limits::default();
        limits.max_total_pack_input_bytes = 5;
        assert!(build_android_rpf7(&input, &output, limits).is_err());
        assert!(!output.exists());
    }

    #[test]
    fn rejects_rsc7_files_with_inconsistent_header_version_and_page_flags() {
        let temp = tempfile::tempdir().unwrap();
        let input = temp.path().join("input");
        std::fs::create_dir_all(&input).unwrap();

        let mut resource = vec![0u8; 20];
        resource[0..4].copy_from_slice(&0x37435352u32.to_le_bytes());
        resource[4..8].copy_from_slice(&13u32.to_le_bytes());
        resource[8..12].copy_from_slice(&0xA0000002u32.to_le_bytes());
        resource[12..16].copy_from_slice(&0x50000000u32.to_le_bytes());
        std::fs::write(input.join("bad.ytd"), resource).unwrap();

        let output = temp.path().join("bad_android.rpf");
        assert!(build_android_rpf7(&input, &output, Limits::default()).is_err());
        assert!(!output.exists());
    }

    #[test]
    fn rejects_parent_traversal() {
        assert!(safe_relative_path("../outside.txt").is_err());
        assert!(safe_relative_path("folder/../../outside.txt").is_err());
    }

    #[test]
    fn accepts_nested_relative_path() {
        let path = safe_relative_path("vehicles/dyfurai/vehicle.yft").unwrap();
        assert_eq!(path, PathBuf::from("vehicles/dyfurai/vehicle.yft"));
    }

    #[test]
    fn rejects_absolute_paths() {
        assert!(safe_relative_path("/sdcard/outside.txt").is_err());
        assert!(safe_relative_path("C:\\Windows\\file.txt").is_err());
    }
}


/// Convert an extracted PC asset tree into a new, unencrypted GTA V RPF7 archive.
/// Gen9 YTD resources are converted to Legacy YTD; other asset types are preserved
/// byte-for-byte and are NOT claimed to be platform-converted.
pub fn build_android_rpf7(
    source_dir: impl AsRef<Path>,
    output_file: impl AsRef<Path>,
    limits: Limits,
) -> Result<(usize, u64, usize)> {
    use rage_rpf::{RpfBuilder, RpfEncryption, RpfArchive};

    let source_dir = source_dir.as_ref();
    let output_file = output_file.as_ref();
    let root = fs::canonicalize(source_dir).context("open source asset folder")?;
    let mut builder = RpfBuilder::new(RpfEncryption::Open);
    let mut stack = vec![root.clone()];
    let mut count = 0usize;
    let mut total = 0u64;
    let mut converted_ytd = 0usize;
    let mut names_bytes = 0usize;

    while let Some(dir) = stack.pop() {
        for item in fs::read_dir(&dir)? {
            let item = item?;
            let path = item.path();
            let meta = fs::symlink_metadata(&path)?;
            if meta.file_type().is_symlink() {
                bail!("symbolic links are not allowed in conversion input");
            }
            if meta.is_dir() {
                stack.push(path);
                continue;
            }
            if !meta.is_file() { continue; }
            count += 1;
            if count > limits.max_entries { bail!("file count exceeds conversion limit"); }
            if meta.len() > limits.max_entry_bytes { bail!("file exceeds conversion size limit: {}", path.display()); }
            total = total.checked_add(meta.len()).context("conversion size overflow")?;
            if total > limits.max_total_pack_input_bytes {
                bail!("RPF7 in-memory pack input exceeds configured limit ({} bytes)", limits.max_total_pack_input_bytes);
            }

            let relative = path.strip_prefix(&root).context("file escaped input root")?;
            let archive_path = relative.to_string_lossy().replace('\\', "/");
            names_bytes = names_bytes.checked_add(archive_path.len() + 1).context("RPF name table overflow")?;
            if names_bytes > u16::MAX as usize { bail!("RPF7 name table exceeds 64 KiB writer limit"); }
            let mut data = fs::read(&path)?;
            let has_rsc7_magic = data.len() >= 4
                && u32::from_le_bytes(data[0..4].try_into().unwrap()) == 0x37435352;
            if has_rsc7_magic {
                if data.len() < 16 {
                    bail!("truncated RSC7 header: {}", archive_path);
                }
                let header_version = u32::from_le_bytes(data[4..8].try_into().unwrap());
                let system_flags = u32::from_le_bytes(data[8..12].try_into().unwrap());
                let graphics_flags = u32::from_le_bytes(data[12..16].try_into().unwrap());
                let flags_version = rage_rpf::resource_version_from_flags(system_flags, graphics_flags);
                if header_version != flags_version {
                    bail!(
                        "RSC7 version/header flags disagree for {} (header {}, flags {})",
                        archive_path,
                        header_version,
                        flags_version
                    );
                }
            }
            let is_ytd = path.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("ytd"));
            if is_ytd {
                match rage_formats::Edition::of_file("ytd", &data).context("invalid YTD resource header")? {
                    Some(rage_formats::Edition::Gen9) => {
                        data = rage_formats::convert_ytd_to_legacy(&data).context("Gen9 YTD conversion failed")?;
                        converted_ytd += 1;
                    }
                    Some(rage_formats::Edition::Legacy) | None => {}
                }
            }
            let is_resource = has_rsc7_magic;
            if data.len() > 0xFFFFFF && !is_resource {
                bail!("binary file exceeds RPF7 24-bit size limit: {}", archive_path);
            }
            builder.add_file(&archive_path, data);
        }
    }
    if count == 0 { bail!("no files found to package"); }
    let archive_bytes = builder.build(None).context("build RPF7 archive")?;
    if archive_bytes.len() as u64 > limits.max_archive_bytes {
        bail!("generated RPF exceeds the configured archive size limit");
    }
    let parsed = RpfArchive::parse(&archive_bytes, "android-converted.rpf", None)
        .context("generated RPF failed validation")?;
    if parsed.version != rage_rpf::RpfVersion::V7 || parsed.entries.len() < count {
        bail!("generated RPF validation mismatch");
    }
    if let Some(parent) = output_file.parent() { fs::create_dir_all(parent)?; }
    fs::write(output_file, &archive_bytes).context("write generated RPF")?;
    Ok((count, archive_bytes.len() as u64, converted_ytd))
}
