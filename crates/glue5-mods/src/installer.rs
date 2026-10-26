//! Safe installation of ZIP archives containing one Glue 5 Luau mod.
//!
//! Supported layout: `mod-name/main.luau` (optionally `mod-name/mod.json`)
//! or `main.luau` at the archive root. Native PC/iOS plugins are not supported.

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

const MAX_ENTRIES: usize = 2_000;
const MAX_UNPACKED_BYTES: u64 = 64 * 1024 * 1024;

/// Install a ZIP archive into the active mods directory.
///
/// Archives are unpacked to a temporary directory first and only committed
/// after a `main.luau` entry is found. Existing mods are never overwritten.
pub fn install_zip(archive_path: &Path, requested_name: Option<&str>) -> Result<String, String> {
    let mods_dir = crate::dir().ok_or("mods folder is not open")?.to_path_buf();
    let file = File::open(archive_path).map_err(|e| format!("cannot open archive: {e}"))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("invalid ZIP: {e}"))?;
    if archive.len() == 0 || archive.len() > MAX_ENTRIES {
        return Err(format!("ZIP must contain 1–{MAX_ENTRIES} entries"));
    }

    let temp = mods_dir.join(format!(".install-{}", std::process::id()));
    if temp.exists() {
        fs::remove_dir_all(&temp).map_err(|e| format!("cannot clear staging directory: {e}"))?;
    }
    fs::create_dir_all(&temp).map_err(|e| format!("cannot create staging directory: {e}"))?;

    let result = (|| {
        let mut total = 0u64;
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i).map_err(|e| format!("ZIP entry error: {e}"))?;
            let Some(relative) = entry.enclosed_name() else {
                return Err(format!("unsafe path in ZIP entry {}", entry.name()));
            };
            if relative.components().any(|c| !matches!(c, Component::Normal(_))) {
                return Err(format!("unsafe path in ZIP entry {}", entry.name()));
            }
            if entry.is_dir() {
                fs::create_dir_all(temp.join(&relative)).map_err(|e| e.to_string())?;
                continue;
            }
            total = total.checked_add(entry.size()).ok_or("ZIP size overflow")?;
            if total > MAX_UNPACKED_BYTES {
                return Err("mod archive exceeds the 64 MiB unpacked limit".into());
            }
            let target = temp.join(&relative);
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let mut output = File::create(&target).map_err(|e| e.to_string())?;
            let mut buffer = [0u8; 32 * 1024];
            loop {
                let n = entry.read(&mut buffer).map_err(|e| format!("cannot read ZIP entry: {e}"))?;
                if n == 0 { break; }
                output.write_all(&buffer[..n]).map_err(|e| e.to_string())?;
            }
        }

        // Accept either a single root main.luau or exactly one top-level
        // mod directory. Reject ambiguous archives instead of guessing.
        let root_main = temp.join("main.luau");
        let (source_dir, default_name) = if root_main.is_file() {
            let name = requested_name
                .map(str::to_owned)
                .or_else(|| archive_path.file_stem().map(|s| s.to_string_lossy().to_ascii_lowercase().replace(' ', "-")))
                .ok_or("provide a mod name for this archive")?;
            (temp.clone(), name)
        } else {
            let dirs: Vec<PathBuf> = fs::read_dir(&temp)
                .map_err(|e| e.to_string())?
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.is_dir())
                .collect();
            if dirs.len() != 1 {
                return Err("ZIP must contain main.luau at its root or in one top-level mod folder".into());
            }
            let source = dirs[0].clone();
            if !source.join("main.luau").is_file() {
                return Err("mod folder is missing main.luau".into());
            }
            let name = requested_name
                .map(str::to_owned)
                .or_else(|| source.file_name().map(|s| s.to_string_lossy().to_string()))
                .ok_or("cannot determine mod name")?;
            (source, name)
        };

        if !crate::files::valid_name(&default_name) {
            return Err("mod name must use lowercase letters, numbers, '-' or '_' (max 40 characters)".into());
        }
        let destination = mods_dir.join(&default_name);
        if destination.exists() {
            return Err(format!("mod '{default_name}' already exists; remove it first to avoid overwriting"));
        }
        fs::rename(&source_dir, &destination).map_err(|e| format!("cannot install mod: {e}"))?;
        crate::refresh();
        Ok(default_name)
    })();

    let _ = fs::remove_dir_all(&temp);
    result
}
