//! Bounded RPF archive inspection. Parsing is not runtime mounting.
use rpf_archive::{RpfArchive, RpfEncryption, RpfEntryKind, RpfVersion};

pub const MAX_RPF_BYTES: usize = 512 * 1024 * 1024;
pub const MAX_RPF_ENTRIES: usize = 250_000;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RpfReport {
    pub name: String,
    pub version: String,
    pub encryption: String,
    pub entries: usize,
    pub directories: usize,
    pub binary_files: usize,
    pub resource_files: usize,
    pub yft_files: usize,
    pub ytd_files: usize,
    pub ymap_files: usize,
    pub nested_rpf_files: usize,
}

/// Parses the archive table of contents without extracting or mounting files.
/// AES/NG archives need valid game keys and are rejected until key handling is added.
pub fn inspect(name: &str, bytes: &[u8]) -> Result<RpfReport, String> {
    if bytes.is_empty() { return Err("RPF archive is empty".into()); }
    if bytes.len() > MAX_RPF_BYTES {
        return Err(format!("RPF archive exceeds {} MiB", MAX_RPF_BYTES / 1024 / 1024));
    }
    if name.is_empty() || name.len() > 255 || name.contains('/') || name.contains('\\') {
        return Err("invalid RPF archive name".into());
    }
    let archive = RpfArchive::parse(bytes, name, None)
        .map_err(|e| format!("RPF parse failed: {e}"))?;
    if archive.entries.len() > MAX_RPF_ENTRIES {
        return Err(format!("RPF contains more than {MAX_RPF_ENTRIES} entries"));
    }
    if matches!(archive.encryption, RpfEncryption::Aes | RpfEncryption::Ng) {
        return Err(format!("RPF TOC uses {:?} encryption; valid game keys are required", archive.encryption));
    }

    let version = match archive.version {
        RpfVersion::V7 => "RPF7", RpfVersion::V8 => "RPF8", RpfVersion::V6 => "RPF6",
        RpfVersion::V4 => "RPF4", RpfVersion::V3 => "RPF3", RpfVersion::V2 => "RPF2",
        RpfVersion::V0 => "RPF0", RpfVersion::Img1 => "IMG1", RpfVersion::Img2 => "IMG2",
        RpfVersion::Img3 => "IMG3",
    };
    let encryption = match archive.encryption {
        RpfEncryption::None => "none", RpfEncryption::Open => "open",
        RpfEncryption::Aes => "AES", RpfEncryption::Ng => "NG", RpfEncryption::Tfit => "TFIT",
    };
    let mut report = RpfReport {
        name: name.to_owned(), version: version.into(), encryption: encryption.into(),
        entries: archive.entries.len(), ..Default::default()
    };
    for entry in &archive.entries {
        if entry.is_directory() { report.directories += 1; continue; }
        match &entry.kind {
            RpfEntryKind::BinaryFile { .. } => report.binary_files += 1,
            RpfEntryKind::ResourceFile { .. } => report.resource_files += 1,
            RpfEntryKind::Directory { .. } => unreachable!(),
        }
        let n = entry.name_lower.as_str();
        if n.ends_with(".yft") { report.yft_files += 1; }
        if n.ends_with(".ytd") { report.ytd_files += 1; }
        if n.ends_with(".ymap") { report.ymap_files += 1; }
        if n.ends_with(".rpf") { report.nested_rpf_files += 1; }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_empty_input() { assert!(inspect("test.rpf", &[]).is_err()); }
    #[test]
    fn rejects_unknown_magic() { assert!(inspect("test.rpf", b"not an RPF archive").unwrap_err().contains("parse failed")); }
    #[test]
    fn rejects_path_like_names() { assert!(inspect("../bad.rpf", b"RPF7").unwrap_err().contains("name")); }
}
