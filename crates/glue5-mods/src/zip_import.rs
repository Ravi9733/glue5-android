//! Safe ZIP installation for script mods. Archives are staged and only
//! become visible after their complete contents have been validated.

use std::collections::HashSet;
use std::io::{Cursor, Read};
use std::path::{Component, Path, PathBuf};

const MAX_ENTRIES: usize = 256;
const MAX_FILE: u64 = 8 << 20;
const MAX_TOTAL: u64 = 32 << 20;

struct Entry {
    index: usize,
    path: PathBuf,
    directory: bool,
    size: u64,
}

pub fn install(name: &str, bytes: &[u8]) -> Result<(), String> {
    if !crate::files::valid_name(name) {
        return Err("Mod name must use lowercase letters, digits, - or _ (max 40 characters)".into());
    }
    let root = crate::dir().ok_or("mods are off")?;
    let target = root.join(name);
    if target.exists() {
        return Err(format!("{name} already exists; uninstall it before replacing"));
    }

    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|e| format!("Invalid ZIP archive: {e}"))?;
    if archive.is_empty() || archive.len() > MAX_ENTRIES {
        return Err(format!("ZIP must contain 1–{MAX_ENTRIES} entries"));
    }

    let mut entries = Vec::with_capacity(archive.len());
    let mut total = 0u64;
    let mut root_folder: Option<String> = None;
    let mut has_root_main = false;

    for index in 0..archive.len() {
        let file = archive.by_index(index).map_err(|e| format!("ZIP entry {index}: {e}"))?;
        let raw = file.name();
        if raw.starts_with('/') || raw.contains('\\') || raw.contains(':') {
            return Err(format!("Unsafe ZIP path: {raw}"));
        }
        let directory = file.is_dir();
        let mut parts: Vec<&str> = raw.split('/').collect();
        if directory && parts.last() == Some(&"") {
            parts.pop();
        }
        if parts.is_empty() || parts.iter().any(|p| p.is_empty() || *p == "." || *p == ".." || p.starts_with('.')) {
            return Err(format!("Unsafe ZIP path: {raw}"));
        }
        let path = parts.iter().collect::<PathBuf>();
        if path.components().any(|c| !matches!(c, Component::Normal(_))) {
            return Err(format!("Unsafe ZIP path: {raw}"));
        }
        if file.unix_mode().is_some_and(|mode| mode & 0o170000 == 0o120000) {
            return Err(format!("Symbolic links are not allowed: {raw}"));
        }
        if !directory {
            if file.size() > MAX_FILE {
                return Err(format!("File is too large: {raw}"));
            }
            total = total.checked_add(file.size()).ok_or("ZIP is too large")?;
            if total > MAX_TOTAL {
                return Err("Unpacked ZIP exceeds 32 MiB".into());
            }
            if parts.len() == 1 && parts[0] == "main.luau" {
                has_root_main = true;
            }
            if parts.len() == 2 && parts[1] == "main.luau" {
                match &root_folder {
                    Some(folder) if folder != parts[0] => return Err("ZIP has more than one mod folder".into()),
                    _ => root_folder = Some(parts[0].to_string()),
                }
            }
        }
        entries.push(Entry { index, path, directory, size: file.size() });
    }

    let strip = if has_root_main { None } else { root_folder.as_deref() };
    if strip.is_none() && !has_root_main {
        return Err("ZIP must contain main.luau at its root or inside one top-level folder".into());
    }
    if let Some(folder) = strip {
        if entries.iter().filter(|e| !e.directory).any(|e| e.path.components().next().and_then(|c| c.as_os_str().to_str()) != Some(folder)) {
            return Err("All mod files must be inside the same top-level folder".into());
        }
    }

    let mut random = [0u8; 6];
    getrandom::fill(&mut random).map_err(|e| format!("Could not create install staging folder: {e}"))?;
    let suffix = random.iter().map(|b| format!("{b:02x}")).collect::<String>();
    let staging = root.join(format!(".import-{name}-{suffix}"));
    std::fs::create_dir(&staging).map_err(|e| format!("Could not create staging folder: {e}"))?;

    let result = (|| {
        let mut written = HashSet::new();
        for entry in &entries {
            let relative = if let Some(folder) = strip {
                entry.path.strip_prefix(folder).map_err(|_| "Invalid mod folder layout")?
            } else {
                entry.path.as_path()
            };
            if relative.as_os_str().is_empty() {
                continue;
            }
            if relative.components().any(|c| !matches!(c, Component::Normal(_))) {
                return Err("Unsafe path in mod archive".to_string());
            }
            let destination = staging.join(relative);
            if !written.insert(relative.to_path_buf()) {
                return Err(format!("Duplicate ZIP path: {}", relative.display()));
            }
            if entry.directory {
                std::fs::create_dir_all(&destination).map_err(|e| format!("{}: {e}", relative.display()))?;
                continue;
            }
            if let Some(parent) = destination.parent() {
                std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", relative.display()))?;
            }
            let mut file = archive.by_index(entry.index).map_err(|e| format!("ZIP entry: {e}"))?;
            let mut output = Vec::with_capacity(entry.size.min(MAX_FILE) as usize);
            file.take(MAX_FILE + 1).read_to_end(&mut output).map_err(|e| format!("{}: {e}", relative.display()))?;
            if output.len() as u64 != entry.size || output.len() as u64 > MAX_FILE {
                return Err(format!("Invalid or oversized file: {}", relative.display()));
            }
            std::fs::write(&destination, output).map_err(|e| format!("{}: {e}", relative.display()))?;
        }
        let main = staging.join("main.luau");
        if !main.is_file() {
            return Err("ZIP does not contain main.luau".into());
        }
        let config = staging.join("mod.json");
        if config.exists() {
            let text = std::fs::read_to_string(&config).map_err(|e| format!("mod.json: {e}"))?;
            let value: serde_json::Value = serde_json::from_str(&text).map_err(|e| format!("Invalid mod.json: {e}"))?;
            if !value.is_object() || value.get("autostart").is_some_and(|v| !v.is_boolean()) {
                return Err("mod.json must be an object and autostart must be true or false".into());
            }
        } else {
            std::fs::write(config, "{\n  \"autostart\": false\n}\n").map_err(|e| format!("mod.json: {e}"))?;
        }
        std::fs::rename(&staging, &target).map_err(|e| format!("Could not finish installing {name}: {e}"))?;
        Ok(())
    })();

    if result.is_err() {
        let _ = std::fs::remove_dir_all(&staging);
    } else {
        crate::changed(&format!("{name}/"));
        crate::refresh();
    }
    result
}
