//! Safe importer for PC mod ZIP packages.
//!
//! This stores packages in an isolated `mods/pc/<id>/` tree. It deliberately
//! does not mount RPF/DLC archives or execute PE/ASI binaries: those require
//! a game VFS integration and a compatible plugin ABI, respectively.

use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

const MAX_ENTRIES: usize = 20_000;
const MAX_FILE_BYTES: u64 = 256 * 1024 * 1024;
const MAX_TOTAL_BYTES: u64 = 512 * 1024 * 1024;
const MAX_PATH_DEPTH: usize = 16;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ImportReport {
    pub id: String,
    pub files: usize,
    pub bytes: u64,
    pub luau_scripts: usize,
    pub rpf_archives: usize,
    pub windows_plugins: usize,
    pub native_libraries: usize,
    pub other_files: usize,
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 48
        && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
}

fn safe_relative(name: &str) -> Result<PathBuf, String> {
    if name.is_empty() || name.contains('\0') || name.starts_with('/') || name.starts_with('\\') {
        return Err("archive contains an absolute or empty path".into());
    }
    let normalized = name.replace('\\', "/");
    let path = Path::new(&normalized);
    let mut depth = 0usize;
    for component in path.components() {
        match component {
            Component::Normal(part) => {
                let s = part.to_string_lossy();
                if s.is_empty() || s == "." || s == ".." || s.contains(':') {
                    return Err(format!("unsafe archive path: {name}"));
                }
                depth += 1;
            }
            _ => return Err(format!("unsafe archive path: {name}")),
        }
    }
    if depth == 0 || depth > MAX_PATH_DEPTH {
        return Err(format!("archive path depth is invalid: {name}"));
    }
    Ok(path.to_path_buf())
}

/// Extract a user-selected ZIP into an isolated folder under the existing
/// mods directory. Nothing is written to game/data or the user's saves.
pub fn import_zip(mods_root: &Path, id: &str, archive: &[u8]) -> Result<ImportReport, String> {
    if !valid_id(id) {
        return Err("mod ID must use lowercase letters, digits, '-' or '_' (max 48)".into());
    }
    let pc_root = mods_root.join("pc");
    let final_dir = pc_root.join(id);
    if final_dir.exists() {
        return Err(format!("PC mod '{id}' already exists"));
    }
    std::fs::create_dir_all(&pc_root).map_err(|e| e.to_string())?;
    let staging = pc_root.join(format!(".{id}.installing"));
    if staging.exists() {
        std::fs::remove_dir_all(&staging).map_err(|e| e.to_string())?;
    }
    std::fs::create_dir(&staging).map_err(|e| e.to_string())?;

    let result = (|| {
        let cursor = std::io::Cursor::new(archive);
        let mut zip = zip::ZipArchive::new(cursor).map_err(|e| format!("invalid ZIP: {e}"))?;
        if zip.len() > MAX_ENTRIES {
            return Err(format!("ZIP has too many entries (limit {MAX_ENTRIES})"));
        }
        let mut report = ImportReport { id: id.to_owned(), ..Default::default() };
        for index in 0..zip.len() {
            let mut entry = zip.by_index(index).map_err(|e| format!("ZIP entry {index}: {e}"))?;
            if entry.is_dir() { continue; }
            let name = entry.name().to_owned();
            let relative = safe_relative(&name)?;
            if entry.size() > MAX_FILE_BYTES {
                return Err(format!("{name}: file exceeds 256 MiB limit"));
            }
            report.bytes = report.bytes.checked_add(entry.size()).ok_or("ZIP size overflow")?;
            if report.bytes > MAX_TOTAL_BYTES {
                return Err("ZIP expands beyond the 512 MiB import limit".into());
            }
            // Unix symlinks can escape the extraction directory.
            if entry.unix_mode().is_some_and(|mode| (mode & 0o170000) == 0o120000) {
                return Err(format!("{name}: symbolic links are not allowed"));
            }
            let target = staging.join(&relative);
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent).map_err(|e| format!("{name}: {e}"))?;
            }
            let mut output = std::fs::OpenOptions::new()
                .write(true).create_new(true).open(&target)
                .map_err(|e| format!("{name}: {e}"))?;
            let mut copied = 0u64;
            let mut buffer = [0u8; 64 * 1024];
            loop {
                let count = entry.read(&mut buffer).map_err(|e| format!("{name}: {e}"))?;
                if count == 0 { break; }
                copied += count as u64;
                if copied > MAX_FILE_BYTES || copied > entry.size() {
                    return Err(format!("{name}: invalid expanded file size"));
                }
                output.write_all(&buffer[..count]).map_err(|e| format!("{name}: {e}"))?;
            }
            if copied != entry.size() {
                return Err(format!("{name}: truncated ZIP entry"));
            }
            report.files += 1;
            let lower = name.to_ascii_lowercase();
            if lower.ends_with(".luau") || lower.ends_with(".lua") {
                report.luau_scripts += 1;
            } else if lower.ends_with(".rpf") {
                report.rpf_archives += 1;
            } else if lower.ends_with(".asi") || lower.ends_with(".dll") || lower.ends_with(".exe") {
                report.windows_plugins += 1;
            } else if lower.ends_with(".so") {
                report.native_libraries += 1;
            } else {
                report.other_files += 1;
            }
        }
        if report.files == 0 {
            return Err("ZIP contains no files".into());
        }
        let metadata = serde_json::json!({
            "id": id,
            "source": "pc-zip",
            "files": report.files,
            "expanded_bytes": report.bytes,
            "luau_scripts": report.luau_scripts,
            "rpf_archives": report.rpf_archives,
            "windows_plugins": report.windows_plugins,
            "native_libraries": report.native_libraries,
            "other_files": report.other_files,
            "runtime": "stored-only; archive mounting and PC plugin execution are not enabled"
        });
        std::fs::write(staging.join("glue5-pc-mod.json"),
            serde_json::to_vec_pretty(&metadata).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        std::fs::rename(&staging, &final_dir).map_err(|e| e.to_string())?;
        Ok(report)
    })();

    if result.is_err() {
        let _ = std::fs::remove_dir_all(&staging);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn zip_with(name: &str, data: &[u8]) -> Vec<u8> {
        let mut out = std::io::Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut out);
            zip.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
            zip.write_all(data).unwrap();
            zip.finish().unwrap();
        }
        out.into_inner()
    }

    #[test]
    fn rejects_traversal_and_absolute_paths() {
        for name in ["../escape.txt", "/absolute.txt", "folder/../../escape"] {
            let temp = std::env::temp_dir().join(format!("glue5-import-test-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&temp);
            assert!(import_zip(&temp, "sample", &zip_with(name, b"x")).is_err());
            let _ = std::fs::remove_dir_all(temp);
        }
    }

    #[test]
    fn extracts_to_isolated_pc_folder_and_reports_types() {
        let temp = std::env::temp_dir().join(format!("glue5-import-ok-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp);
        let mut out = std::io::Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut out);
            for (name, data) in [("car/dlc.rpf", b"RPF".as_slice()), ("scripts/readme.lua", b"print(1)".as_slice())] {
                zip.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
                zip.write_all(data).unwrap();
            }
            zip.finish().unwrap();
        }
        let report = import_zip(&temp, "my-car", &out.into_inner()).unwrap();
        assert_eq!(report.files, 2);
        assert_eq!(report.rpf_archives, 1);
        assert_eq!(report.luau_scripts, 1);
        assert!(temp.join("pc/my-car/car/dlc.rpf").is_file());
        assert!(!temp.join("my-car").exists());
        let _ = std::fs::remove_dir_all(temp);
    }
}
