//! Safe in-game installer for Luau mod folders and ZIP archives.
use std::fs;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

#[derive(Clone, Debug)]
pub struct Candidate { pub name: String, pub kind: &'static str }

pub fn staging_dir() -> Result<PathBuf, String> {
    let mods = crate::dir().ok_or("Mods directory is not ready")?;
    let path = mods.parent().ok_or("Cannot locate game directory")?.join("mod-import");
    fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    Ok(path)
}

pub fn candidates() -> Result<Vec<Candidate>, String> {
    let mut out = Vec::new();
    for e in fs::read_dir(staging_dir()?).map_err(|e| e.to_string())? {
        let e = e.map_err(|e| e.to_string())?;
        let p = e.path();
        let name = e.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') { continue; }
        if p.is_dir() && find_main(&p, 2).is_some() {
            out.push(Candidate { name, kind: "Folder" });
        } else if p.is_file() && p.extension().is_some_and(|x| x.eq_ignore_ascii_case("zip")) {
            out.push(Candidate { name, kind: "ZIP" });
        }
    }
    out.sort_by(|a,b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(out)
}

pub fn install_candidate(candidate: &str) -> Result<String, String> {
    if candidate.is_empty() || candidate.contains('/') || candidate.contains('\\') || candidate.starts_with('.') {
        return Err("Invalid import name".into());
    }
    let staging = staging_dir()?;
    let src = staging.join(candidate);
    if !src.exists() { return Err("Import no longer exists".into()); }
    let temp = staging.join(format!(".extract-{}", std::process::id()));
    if temp.exists() { fs::remove_dir_all(&temp).map_err(|e| e.to_string())?; }
    let result = if src.is_dir() {
        let root = find_main(&src, 2).ok_or("Folder has no main.luau at its root or one subfolder deep")?;
        let raw = if root == src { candidate.to_string() } else { root.file_name().unwrap_or_default().to_string_lossy().into_owned() };
        copy_install(&root, &safe_name(&raw, candidate)?)
    } else if src.extension().is_some_and(|x| x.eq_ignore_ascii_case("zip")) {
        fs::create_dir_all(&temp).map_err(|e| e.to_string())?;
        match extract_zip(&src, &temp) {
            Ok(()) => match find_main(&temp, 4) {
                Some(root) => {
                    let raw = if root == temp.as_path() { src.file_stem().unwrap_or_default().to_string_lossy().into_owned() } else { root.file_name().unwrap_or_default().to_string_lossy().into_owned() };
                    match safe_name(&raw, candidate) { Ok(name) => copy_install(&root, &name), Err(e) => Err(e) }
                }
                None => Err("ZIP has no main.luau (searched up to 4 folders deep)".into()),
            },
            Err(e) => Err(e),
        }
    } else { Err("Choose a ZIP file or a mod folder".into()) };
    let _ = fs::remove_dir_all(&temp);
    result
}

pub fn delete_mod(name: &str) -> Result<(), String> {
    if !crate::files::valid_name(name) { return Err("Invalid mod name".into()); }
    if crate::mods().iter().any(|m| m.name == name && m.state == crate::ModState::Running) {
        return Err("Stop this mod before deleting it".into());
    }
    let path = crate::dir().ok_or("Mods directory is not ready")?.join(name);
    if !path.is_dir() { return Err("Mod folder not found".into()); }
    fs::remove_dir_all(path).map_err(|e| e.to_string())?;
    crate::refresh();
    Ok(())
}

fn safe_name(raw: &str, fallback: &str) -> Result<String, String> {
    let clean = |s: &str| -> String { s.to_ascii_lowercase().chars().map(|c| if c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_' { c } else { '-' }).collect::<String>().trim_matches('-').to_string() };
    let name = clean(raw);
    let name = if crate::files::valid_name(&name) { name } else { clean(Path::new(fallback).file_stem().unwrap_or_default().to_string_lossy().as_ref()) };
    if crate::files::valid_name(&name) { Ok(name) } else { Err("Mod name must use lowercase letters, numbers, - or _ (max 40 characters)".into()) }
}

fn find_main(root: &Path, max: usize) -> Option<PathBuf> {
    fn walk(p: &Path, depth: usize, max: usize) -> Option<PathBuf> {
        if p.join("main.luau").is_file() { return Some(p.to_path_buf()); }
        if depth >= max { return None; }
        let mut entries: Vec<_> = fs::read_dir(p).ok()?.flatten().collect();
        entries.sort_by_key(|e| e.file_name());
        for e in entries {
            if e.file_type().ok()?.is_dir() && !e.file_name().to_string_lossy().starts_with('.') {
                if let Some(found) = walk(&e.path(), depth + 1, max) { return Some(found); }
            }
        }
        None
    }
    walk(root, 0, max)
}

fn copy_install(root: &Path, name: &str) -> Result<String, String> {
    let mods = crate::dir().ok_or("Mods directory is not ready")?;
    if !crate::files::valid_name(name) { return Err("Invalid mod name".into()); }
    let dest = mods.join(name);
    if dest.exists() { return Err(format!("{name} already exists; delete it first before replacing")); }
    let temp = mods.join(format!(".install-{name}-{}", std::process::id()));
    if temp.exists() { fs::remove_dir_all(&temp).map_err(|e| e.to_string())?; }
    fs::create_dir_all(&temp).map_err(|e| e.to_string())?;
    if let Err(e) = copy_tree(root, &temp) { let _ = fs::remove_dir_all(&temp); return Err(e); }
    if !temp.join("main.luau").is_file() { let _ = fs::remove_dir_all(&temp); return Err("main.luau must be at the mod root".into()); }
    if !temp.join("mod.json").exists() {
        fs::write(temp.join("mod.json"), "{\n  \"autostart\": false\n}\n").map_err(|e| e.to_string())?;
    }
    fs::rename(&temp, &dest).map_err(|e| e.to_string())?;
    crate::refresh();
    Ok(format!("Installed {name}. Enable autostart or start it from the Mods list."))
}

fn copy_tree(src: &Path, dst: &Path) -> Result<(), String> {
    for e in fs::read_dir(src).map_err(|e| e.to_string())? {
        let e = e.map_err(|e| e.to_string())?;
        let name = e.file_name();
        if name.to_string_lossy().starts_with('.') { continue; }
        let from = e.path(); let to = dst.join(name);
        let ty = e.file_type().map_err(|e| e.to_string())?;
        if ty.is_dir() { fs::create_dir_all(&to).map_err(|e| e.to_string())?; copy_tree(&from, &to)?; }
        else if ty.is_file() { fs::copy(from, to).map_err(|e| e.to_string())?; }
    }
    Ok(())
}

fn extract_zip(path: &Path, dest: &Path) -> Result<(), String> {
    let file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| format!("Invalid ZIP: {e}"))?;
    if zip.len() > 10_000 { return Err("ZIP has too many files (limit 10,000)".into()); }
    let mut total = 0u64;
    for i in 0..zip.len() {
        let mut f = zip.by_index(i).map_err(|e| e.to_string())?;
        let rel = f.enclosed_name().ok_or("ZIP contains an unsafe path")?.to_path_buf();
        if rel.components().any(|c| !matches!(c, Component::Normal(_))) { return Err("ZIP contains an unsafe path".into()); }
        let out = dest.join(rel);
        if f.is_dir() { fs::create_dir_all(&out).map_err(|e| e.to_string())?; continue; }
        total = total.saturating_add(f.size());
        if total > 500 * 1024 * 1024 { return Err("ZIP expands beyond 500 MB".into()); }
        if let Some(p) = out.parent() { fs::create_dir_all(p).map_err(|e| e.to_string())?; }
        let mut output = fs::File::create(out).map_err(|e| e.to_string())?;
        let mut buf = [0u8; 65536];
        loop { let n = f.read(&mut buf).map_err(|e| e.to_string())?; if n == 0 { break; } output.write_all(&buf[..n]).map_err(|e| e.to_string())?; }
    }
    Ok(())
}
