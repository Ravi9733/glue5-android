//! Read-only discovery of add-on DLC packs in the sibling GTA5/mods folder.
//!
//! The game still owns RPF parsing and mounting. This module exposes each
//! external DLC archive at the path already used by the game's dlcList.xml,
//! and stages a merged list without changing the original game data.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const DLC_PACKS_ENGINE_DIR: &str = "x64/dlcPacks";

#[derive(Default)]
pub(crate) struct DlcMods {
    /// Engine-relative path (lowercase) -> external file or directory.
    pub(crate) paths: HashMap<String, PathBuf>,
    /// Engine-relative directory (lowercase) -> additional child entries.
    pub(crate) directories: HashMap<String, Vec<(String, u8)>>,
    /// Staged merged common/data/dlcList.xml, if add-on packs were found.
    pub(crate) dlc_list: Option<PathBuf>,
}

impl DlcMods {
    pub(crate) fn discover(
        game_data: Option<&Path>,
        game_index: &HashMap<String, String>,
        scratch: &Path,
    ) -> Self {
        let Some(game_data) = game_data else {
            return Self::default();
        };
        if !game_data.file_name().is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case("data")) {
            return Self::default();
        }
        // game_data is <GTA root>/game/data; mods is its sibling to game/.
        let Some(game_root) = game_data.parent().and_then(Path::parent) else {
            return Self::default();
        };
        let mods_root = game_root.join("mods");
        if !mods_root.is_dir() {
            return Self::default();
        }

        // Prefer the Android OpenIV layout, then accept the PC-style layout.
        let pack_roots = [
            mods_root.join("update/android/dlcpacks"),
            mods_root.join("update/x64/dlcpacks"),
            mods_root.join("x64/dlcPacks"),
        ];
        let mut found = Self::default();
        let mut pack_names = Vec::new();
        let mut seen = HashSet::new();

        for root in pack_roots {
            let Ok(entries) = fs::read_dir(&root) else { continue };
            for entry in entries.flatten() {
                let path = entry.path();
                let Ok(kind) = entry.file_type() else { continue };
                if !kind.is_dir() || kind.is_symlink() {
                    continue;
                }
                let name = entry.file_name().to_string_lossy().into_owned();
                if !valid_pack_name(&name) || !case_insensitive_child(&path, "dlc.rpf").is_some_and(|p| {
                    fs::symlink_metadata(p).is_ok_and(|meta| meta.file_type().is_file())
                }) {
                    continue;
                }
                let key = name.to_ascii_lowercase();
                if seen.contains(&key) {
                    log::warn!("DLC mod ignored (duplicate pack name): {name}");
                    continue;
                }

                let engine_rel = format!("{DLC_PACKS_ENGINE_DIR}/{name}");
                let mut pack_paths = HashMap::new();
                let mut pack_directories = HashMap::new();
                if let Err(error) = index_pack(&path, &engine_rel, &mut pack_paths, &mut pack_directories) {
                    log::warn!("DLC mod {name} could not be indexed: {error}");
                    continue;
                }
                seen.insert(key);
                found.paths.extend(pack_paths);
                found.directories.extend(pack_directories);
                let parent = DLC_PACKS_ENGINE_DIR.to_ascii_lowercase();
                let children = found.directories.entry(parent).or_default();
                if !children.iter().any(|(existing, _)| existing.eq_ignore_ascii_case(&name)) {
                    children.push((name.clone(), 4));
                }
                pack_names.push(name);
            }
        }

        if pack_names.is_empty() {
            return found;
        }
        pack_names.sort_by_key(|name| name.to_ascii_lowercase());

        let Some(original_rel) = game_index.get("common/data/dlclist.xml") else {
            log::warn!("DLC mods found, but common/data/dlcList.xml is missing");
            return found;
        };
        let original = game_data.join(original_rel);
        match merged_dlc_list(&original, &pack_names, scratch) {
            Ok(Some(path)) => {
                log::info!("DLC mods: {} add-on pack(s) ready from {}", pack_names.len(), mods_root.display());
                found.dlc_list = Some(path);
            }
            Ok(None) => {
                log::info!("DLC mods: all discovered packs are already listed");
            }
            Err(error) => {
                log::warn!("DLC mods were indexed but dlcList.xml could not be extended: {error:#}");
            }
        }
        found
    }
}

fn valid_pack_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

fn case_insensitive_child(dir: &Path, wanted: &str) -> Option<PathBuf> {
    fs::read_dir(dir).ok()?.flatten()
        .find(|entry| entry.file_name().to_string_lossy().eq_ignore_ascii_case(wanted))
        .map(|entry| entry.path())
}

fn index_pack(
    root: &Path,
    engine_rel: &str,
    paths: &mut HashMap<String, PathBuf>,
    directories: &mut HashMap<String, Vec<(String, u8)>>,
) -> io::Result<()> {
    paths.entry(engine_rel.to_ascii_lowercase()).or_insert_with(|| root.to_path_buf());
    let mut children = Vec::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        let rel = format!("{engine_rel}/{name}");
        let child_path = entry.path();
        let is_dir = kind.is_dir();
        paths.entry(rel.to_ascii_lowercase()).or_insert_with(|| child_path.clone());
        children.push((name, if is_dir { 4 } else { 8 }));
        if is_dir {
            index_pack(&child_path, &rel, paths, directories)?;
        }
    }
    directories.entry(engine_rel.to_ascii_lowercase()).or_insert(children);
    Ok(())
}

fn merged_dlc_list(original: &Path, packs: &[String], scratch: &Path) -> anyhow::Result<Option<PathBuf>> {
    let source = fs::read_to_string(original)?;
    let document = roxmltree::Document::parse(&source)?;
    let paths = document.descendants().find(|node| node.has_tag_name("Paths"))
        .ok_or_else(|| anyhow::anyhow!("dlcList.xml has no Paths element"))?;
    let existing: HashSet<String> = paths.children()
        .filter(|node| node.has_tag_name("Item"))
        .filter_map(|node| node.text())
        .map(|value| value.trim().replace('/', "\\").to_ascii_lowercase())
        .collect();

    let additions: Vec<String> = packs.iter()
        .filter(|name| !existing.contains(&format!("platform:\\dlcpacks\\{}\\", name.to_ascii_lowercase())))
        .map(|name| format!("    <Item>platform:\\dlcPacks\\{name}\\</Item>"))
        .collect();
    if additions.is_empty() {
        return Ok(None);
    }

    let close = source.rfind("</Paths>").ok_or_else(|| anyhow::anyhow!("dlcList.xml has no closing Paths tag"))?;
    let mut merged = source;
    let insertion = format!("{}{}{}",
        if merged[..close].ends_with('\n') { "" } else { "\n" },
        additions.join("\n"),
        "\n  ",
    );
    merged.insert_str(close, &insertion);
    let output = scratch.join("dlc-mods").join("dlcList.xml");
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&output, merged)?;
    Ok(Some(output))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovers_android_dlc_and_stages_merged_list_without_touching_game_data() {
        let root = std::env::temp_dir().join(format!("glue5-dlc-mods-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let game_data = root.join("game/data");
        let pack = root.join("mods/update/android/dlcpacks/rmodsian");
        fs::create_dir_all(game_data.join("common/data")).unwrap();
        fs::create_dir_all(game_data.join("x64/dlcPacks")).unwrap();
        fs::create_dir_all(&pack).unwrap();
        fs::write(game_data.join("common/data/dlcList.xml"),
            "<?xml version=\"1.0\"?><SMandatoryPacksData><Paths><Item>platform:\\dlcPacks\\mpBeach\\</Item></Paths></SMandatoryPacksData>").unwrap();
        fs::write(pack.join("dlc.rpf"), b"RPF7").unwrap();
        let mut index = HashMap::new();
        index.insert("common/data/dlclist.xml".into(), "common/data/dlcList.xml".into());

        let mods = DlcMods::discover(Some(&game_data), &index, &root.join("user/fsroot"));
        assert_eq!(mods.paths.get("x64/dlcpacks/rmodsian/dlc.rpf"), Some(&pack.join("dlc.rpf")));
        assert!(mods.directories.get("x64/dlcpacks").unwrap()
            .iter().any(|(name, _)| name == "rmodsian"));
        let staged = fs::read_to_string(mods.dlc_list.unwrap()).unwrap();
        assert!(staged.contains("platform:\\dlcPacks\\rmodsian\\"));
        assert!(!fs::read_to_string(game_data.join("common/data/dlcList.xml")).unwrap()
            .contains("rmodsian"));
        let _ = fs::remove_dir_all(root);
    }
}
