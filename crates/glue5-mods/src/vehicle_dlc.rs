//! Static validation of loose GTA V DLC metadata in imported vehicle packages.
//! This does not mount an RPF or modify the game's dlclist.

use roxmltree::Document;
use std::{fs, path::{Path, PathBuf}};

const MAX_XML_BYTES: u64 = 2 * 1024 * 1024;
const MAX_METADATA_FILES: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DlcMetadataReport {
    pub dlclist_entries: Vec<String>,
    pub setup_orders: Vec<(String, i32)>,
    pub content_xml_files: Vec<String>,
    pub warnings: Vec<String>,
}

/// Find and validate loose DLC XML files under an extracted package.
/// Paths are reported relative to the package root and never applied to GTA data.
pub fn inspect_dlc_metadata(root: &Path) -> Result<DlcMetadataReport, String> {
    let root = root.canonicalize().map_err(|e| format!("cannot resolve DLC package root: {e}"))?;
    if !root.is_dir() { return Err("DLC package root is not a directory".into()); }
    let mut files = Vec::new();
    collect_xml(&root, &root, 0, &mut files)?;
    if files.len() > MAX_METADATA_FILES {
        return Err("DLC package contains too many XML metadata files".into());
    }
    let mut report = DlcMetadataReport::default();
    let mut found_dlclist = false;
    let mut found_setup = false;
    let mut found_content = false;

    for (relative, path) in files {
        let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("").to_ascii_lowercase();
        if !matches!(name.as_str(), "dlclist.xml" | "setup2.xml" | "content.xml") { continue; }
        let meta = fs::metadata(&path).map_err(|e| format!("cannot inspect {}: {e}", relative))?;
        if meta.len() > MAX_XML_BYTES {
            report.warnings.push(format!("{relative}: XML exceeds 2 MiB and was skipped"));
            continue;
        }
        let bytes = fs::read(&path).map_err(|e| format!("cannot read {relative}: {e}"))?;
        let text = std::str::from_utf8(bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(&bytes))
            .map_err(|_| format!("{relative}: metadata is not UTF-8"))?;
        let doc = Document::parse(text).map_err(|e| format!("{relative}: invalid XML: {e}"))?;
        match name.as_str() {
            "dlclist.xml" => {
                found_dlclist = true;
                let root_node = doc.root_element();
                if root_node.tag_name().name() != "SMandatoryPacksData" {
                    report.warnings.push(format!("{relative}: unexpected dlclist root element"));
                    continue;
                }
                for item in doc.descendants().filter(|n| n.is_element() && n.tag_name().name() == "Item") {
                    let value = item.text().unwrap_or("").trim();
                    if let Some(id) = parse_dlcpacks_entry(value) {
                        report.dlclist_entries.push(id);
                    } else if value.to_ascii_lowercase().starts_with("dlcpacks:") {
                        report.warnings.push(format!("{relative}: unsafe DLC path entry ignored"));
                    }
                }
            }
            "setup2.xml" => {
                found_setup = true;
                let root_node = doc.root_element();
                if root_node.tag_name().name() != "SSetupData" {
                    report.warnings.push(format!("{relative}: unexpected setup2 root element"));
                    continue;
                }
                let order = root_node.children()
                    .find(|n| n.is_element() && n.tag_name().name() == "order")
                    .and_then(|n| n.attribute("value"))
                    .and_then(|v| v.trim().parse::<i32>().ok())
                    .unwrap_or(-1);
                report.setup_orders.push((relative, order));
            }
            "content.xml" => {
                found_content = true;
                report.content_xml_files.push(relative);
            }
            _ => {}
        }
    }
    if !found_dlclist {
        report.warnings.push("No loose dlclist.xml found; package cannot register itself in the game's DLC list.".into());
    }
    if !found_setup {
        report.warnings.push("No loose setup2.xml found; DLC load-order metadata was not found.".into());
    }
    if !found_content {
        report.warnings.push("No loose content.xml found; DLC content manifest was not found.".into());
    }
    report.dlclist_entries.sort();
    report.dlclist_entries.dedup();
    report.setup_orders.sort_by(|a, b| a.0.cmp(&b.0));
    report.content_xml_files.sort();
    Ok(report)
}


/// Read DLC metadata embedded inside OPEN RPF archives. Encrypted archives are
/// deliberately skipped because this path does not obtain or persist GTA keys.
pub fn inspect_rpf_dlc_metadata(root: &Path) -> Result<DlcMetadataReport, String> {
    use rpf_archive::{RpfEncryption, RpfFile};
    let root = root.canonicalize().map_err(|e| format!("cannot resolve DLC package root: {e}"))?;
    let mut report = DlcMetadataReport::default();
    let mut archives = Vec::new();
    collect_rpf(&root, &root, 0, &mut archives)?;
    if archives.len() > 64 { return Err("vehicle package contains too many RPF archives".into()); }
    let mut metadata_count = 0usize;
    for (relative, path) in archives {
        let archive = match RpfFile::open(&path, None) {
            Ok(archive) => archive,
            Err(error) => {
                // Packages can contain optional, unsupported, or damaged RPFs.
                // Keep the staged package inspectable, but never treat a failed
                // archive parse as proof that its DLC metadata is valid.
                report.warnings.push(format!("{relative}: RPF parse failed and was skipped: {error}"));
                continue;
            }
        };
        if !matches!(archive.archive.encryption, RpfEncryption::Open | RpfEncryption::None) {
            report.warnings.push(format!("{relative}: encrypted RPF metadata skipped; keys are not configured"));
            continue;
        }
        archive.walk(None, &mut |inner_path, bytes| {
            let name = inner_path.rsplit('/').next().unwrap_or(inner_path).to_ascii_lowercase();
            if !matches!(name.as_str(), "dlclist.xml" | "setup2.xml" | "content.xml") { return; }
            metadata_count += 1;
            if metadata_count > MAX_METADATA_FILES { return; }
            if bytes.len() as u64 > MAX_XML_BYTES {
                report.warnings.push(format!("{relative}/{inner_path}: XML exceeds 2 MiB and was skipped"));
                return;
            }
            let text = match std::str::from_utf8(bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(&bytes)) {
                Ok(text) => text,
                Err(_) => { report.warnings.push(format!("{relative}/{inner_path}: metadata is not UTF-8")); return; }
            };
            let doc = match Document::parse(text) {
                Ok(doc) => doc,
                Err(e) => { report.warnings.push(format!("{relative}/{inner_path}: invalid XML: {e}")); return; }
            };
            match name.as_str() {
                "dlclist.xml" => {
                    if doc.root_element().tag_name().name() != "SMandatoryPacksData" {
                        report.warnings.push(format!("{relative}/{inner_path}: unexpected dlclist root"));
                    } else {
                        for item in doc.descendants().filter(|n| n.is_element() && n.tag_name().name() == "Item") {
                            let value = item.text().unwrap_or("").trim();
                            if let Some(id) = parse_dlcpacks_entry(value) {
                                report.dlclist_entries.push(id);
                            } else if value.to_ascii_lowercase().starts_with("dlcpacks:") {
                                report.warnings.push(format!("{relative}/{inner_path}: unsafe DLC path ignored"));
                            }
                        }
                    }
                }
                "setup2.xml" => {
                    if doc.root_element().tag_name().name() != "SSetupData" {
                        report.warnings.push(format!("{relative}/{inner_path}: unexpected setup2 root"));
                    } else {
                        let order = doc.root_element().children()
                            .find(|n| n.is_element() && n.tag_name().name() == "order")
                            .and_then(|n| n.attribute("value"))
                            .and_then(|v| v.trim().parse::<i32>().ok()).unwrap_or(-1);
                        report.setup_orders.push((format!("{relative}/{inner_path}"), order));
                    }
                }
                "content.xml" => report.content_xml_files.push(format!("{relative}/{inner_path}")),
                _ => {}
            }
        }).map_err(|e| format!("{relative}: RPF walk failed: {e}"))?;
    }
    if metadata_count > MAX_METADATA_FILES { return Err("RPF metadata file count exceeds safety limit".into()); }
    report.dlclist_entries.sort(); report.dlclist_entries.dedup();
    report.setup_orders.sort_by(|a,b| a.0.cmp(&b.0));
    report.content_xml_files.sort();
    Ok(report)
}

fn collect_rpf(root: &Path, dir: &Path, depth: usize, out: &mut Vec<(String, PathBuf)>) -> Result<(), String> {
    if depth > 24 { return Err("RPF folder nesting exceeds safety limit".into()); }
    for entry in fs::read_dir(dir).map_err(|e| format!("cannot read package folder: {e}"))? {
        let entry = entry.map_err(|e| format!("cannot read package entry: {e}"))?;
        let path = entry.path();
        let meta = fs::symlink_metadata(&path).map_err(|e| format!("cannot inspect package entry: {e}"))?;
        if meta.file_type().is_symlink() { return Err(format!("symbolic link rejected: {}", path.display())); }
        if meta.is_dir() { collect_rpf(root, &path, depth + 1, out)?; }
        else if meta.is_file() && path.extension().is_some_and(|e| e.eq_ignore_ascii_case("rpf")) {
            out.push((path.strip_prefix(root).unwrap_or(&path).to_string_lossy().replace('\\', "/"), path));
        } else if !meta.is_file() { return Err(format!("special file rejected: {}", path.display())); }
    }
    Ok(())
}

fn collect_xml(root: &Path, dir: &Path, depth: usize, out: &mut Vec<(String, PathBuf)>) -> Result<(), String> {
    if depth > 24 { return Err("DLC metadata folder nesting exceeds safety limit".into()); }
    for entry in fs::read_dir(dir).map_err(|e| format!("cannot read DLC package folder: {e}"))? {
        let entry = entry.map_err(|e| format!("cannot read DLC package entry: {e}"))?;
        let path = entry.path();
        let meta = fs::symlink_metadata(&path).map_err(|e| format!("cannot inspect DLC package entry: {e}"))?;
        if meta.file_type().is_symlink() { return Err(format!("symbolic link rejected: {}", path.display())); }
        if meta.is_dir() {
            collect_xml(root, &path, depth + 1, out)?;
        } else if meta.is_file() && path.extension().is_some_and(|e| e.eq_ignore_ascii_case("xml")) {
            let relative = path.strip_prefix(root).unwrap_or(&path).to_string_lossy().replace('\\', "/");
            out.push((relative, path));
            if out.len() > MAX_METADATA_FILES { return Err("DLC package contains too many XML files".into()); }
        } else if !meta.is_file() {
            return Err(format!("special file rejected: {}", path.display()));
        }
    }
    Ok(())
}

/// Accept GTA V's common slash and Windows-backslash DLC path spellings,
/// including the form used by dlclist entries in many PC add-on packages.
fn parse_dlcpacks_entry(value: &str) -> Option<String> {
    let normalized = value.trim().replace('\\', "/").to_ascii_lowercase();
    let rest = normalized.strip_prefix("dlcpacks:/")?;
    let id = rest.trim_matches('/');
    if valid_dlc_id(id) { Some(id.to_owned()) } else { None }
}

fn valid_dlc_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64
        && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp() -> PathBuf {
        let n = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let p = std::env::temp_dir().join(format!("glue5-dlc-meta-{}-{n}", std::process::id()));
        fs::create_dir_all(&p).unwrap(); p
    }

    #[test]
    fn parses_dlc_list_and_setup_order_without_modifying_it() {
        let root = temp();
        fs::write(root.join("dlclist.xml"), r#"<SMandatoryPacksData><Paths><Item>dlcpacks:/dyfurai/</Item><Item>dlcpacks:\\dyfurai\\</Item><Item>platform:/dlcPacks/mpBeach/</Item></Paths></SMandatoryPacksData>"#).unwrap();
        fs::write(root.join("setup2.xml"), "<SSetupData><order value=\"42\"/></SSetupData>").unwrap();
        fs::write(root.join("content.xml"), "<CContentChangeSets/>").unwrap();
        let report = inspect_dlc_metadata(&root).unwrap();
        assert_eq!(report.dlclist_entries, vec!["dyfurai"]); // slash and backslash spellings deduplicate
        assert_eq!(report.setup_orders[0].1, 42);
        assert_eq!(report.content_xml_files.len(), 1);
        assert!(root.join("dlclist.xml").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_invalid_xml_and_unsafe_dlc_ids() {
        let root = temp();
        fs::write(root.join("dlclist.xml"), "<SMandatoryPacksData><Paths><Item>dlcpacks:/../bad/</Item></Paths></SMandatoryPacksData>").unwrap();
        let report = inspect_dlc_metadata(&root).unwrap();
        assert!(report.dlclist_entries.is_empty());
        assert!(report.warnings.iter().any(|w| w.contains("unsafe")));
        fs::write(root.join("setup2.xml"), "<broken").unwrap();
        assert!(inspect_dlc_metadata(&root).unwrap_err().contains("invalid XML"));
        fs::remove_dir_all(root).unwrap();
    }
}
