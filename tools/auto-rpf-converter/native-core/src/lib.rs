//! Read-only RPF archive inspection and bounded extraction primitives.
//!
//! This crate deliberately does not claim to convert PC assets to an Android
//! game format. It extracts entries from archives the upstream parser can
//! read, preserves originals, and rejects unsafe paths / oversized workloads.

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
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_archive_bytes: 1024 * 1024 * 1024,
            max_entries: 1_000_000,
            max_entry_bytes: 512 * 1024 * 1024,
            max_total_output_bytes: 8 * 1024 * 1024 * 1024,
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
            if total > limits.max_total_output_bytes { bail!("conversion input exceeds total size limit"); }

            let relative = path.strip_prefix(&root).context("file escaped input root")?;
            let archive_path = relative.to_string_lossy().replace('\\', "/");
            let mut data = fs::read(&path)?;
            if path.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("ytd")) {
                if let Ok(converted) = rage_formats::convert_ytd_to_legacy(&data) {
                    if converted != data {
                        data = converted;
                        converted_ytd += 1;
                    }
                }
            }
            builder.add_file(&archive_path, data);
        }
    }
    if count == 0 { bail!("no files found to package"); }
    let archive_bytes = builder.build(None).context("build RPF7 archive")?;
    if archive_bytes.len() as u64 > limits.max_total_output_bytes {
        bail!("generated RPF exceeds output size limit");
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
