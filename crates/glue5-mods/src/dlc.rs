//! DLC package discovery and load-order inspection for imported PC mod packages.
//!
//! This reads loose XML and XML entries inside readable RPF archives. It does
//! not patch the game's update.rpf or register packs with the RAGE packfile
//! manager; that requires an engine packfile integration.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use rpf_archive::{RpfArchive, RpfEncryption, build_directory_tree, list_all_files};

const MAX_RPF_SCAN_BYTES: u64 = 128 * 1024 * 1024;
const MAX_XML_BYTES: usize = 64 * 1024;
const MAX_PACKAGE_FILES: usize = 4096;
const MAX_RPF_FILES: usize = 512;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DlcPackInfo {
    pub package: String,
    pub name: String,
    pub archive: String,
    pub setup_order: Option<i32>,
    pub registered_in_imported_dlclist: bool,
    pub yft_files: usize,
    pub ytd_files: usize,
    pub valid_yft_files: usize,
    pub valid_ytd_files: usize,
    pub invalid_asset_files: usize,
    pub unverified_asset_files: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DlcScanReport {
    pub registered_names: Vec<String>,
    pub packs: Vec<DlcPackInfo>,
    pub warnings: Vec<String>,
}

/// Scan installed package directories for dlcpacks/<name>/dlc.rpf, parse
/// each readable archive, and read setup2.xml load-order metadata.
/// registered_names only reflects a dlclist.xml found inside the imported
/// packages; the game's base update.rpf is not modified or inferred here.
pub fn scan_installed() -> Result<DlcScanReport, String> {
    let root = crate::dir().ok_or("mods are off")?.join(".pc-packages");
    scan_root(&root)
}

/// Scan an isolated package root. Public for tests and tooling.
pub fn scan_root(root: &Path) -> Result<DlcScanReport, String> {
    if !root.is_dir() {
        return Ok(DlcScanReport::default());
    }

    let mut report = DlcScanReport::default();
    let mut package_dirs = std::fs::read_dir(root)
        .map_err(|e| format!("DLC package directory: {e}"))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir()
            && !path.file_name().is_some_and(|name| name.to_string_lossy().starts_with('.')))
        .collect::<Vec<_>>();
    package_dirs.sort();

    for package_dir in package_dirs {
        let package = package_dir.file_name().unwrap_or_default().to_string_lossy().into_owned();
        let files = collect_files(&package_dir, &mut report.warnings)?;
        let mut dlclist_found = false;

        for path in &files {
            if path.file_name().is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case("dlclist.xml")) {
                match read_small_xml(path) {
                    Ok(bytes) => match rpf_archive::parse_dlc_list(&bytes) {
                        Ok(names) => {
                            if !dlclist_found {
                                report.registered_names.extend(names);
                                dlclist_found = true;
                            }
                        }
                        Err(e) => report.warnings.push(format!("{}: {e}", path.display())),
                    },
                    Err(e) => report.warnings.push(e),
                }
            }
        }

        let mut rpf_paths = files.iter()
            .filter(|path| path.extension().is_some_and(|e| e.to_string_lossy().eq_ignore_ascii_case("rpf")))
            .cloned()
            .collect::<Vec<_>>();
        rpf_paths.sort();
        if rpf_paths.len() > MAX_RPF_FILES {
            report.warnings.push(format!("{package}: scanned only the first {MAX_RPF_FILES} RPF files"));
            rpf_paths.truncate(MAX_RPF_FILES);
        }

        for path in rpf_paths {
            let metadata = match std::fs::metadata(&path) {
                Ok(metadata) => metadata,
                Err(e) => {
                    report.warnings.push(format!("{}: {e}", path.display()));
                    continue;
                }
            };
            if metadata.len() > MAX_RPF_SCAN_BYTES {
                report.warnings.push(format!("{}: larger than {} MiB; skipped by scan limit",
                    path.display(), MAX_RPF_SCAN_BYTES / 1024 / 1024));
                continue;
            }
            let bytes = match std::fs::read(&path) {
                Ok(bytes) => bytes,
                Err(e) => {
                    report.warnings.push(format!("{}: {e}", path.display()));
                    continue;
                }
            };
            let archive_name = path.file_name().unwrap_or_default().to_string_lossy();
            let archive = match RpfArchive::parse(&bytes, &archive_name, None) {
                Ok(archive) => archive,
                Err(e) => {
                    report.warnings.push(format!("{}: invalid or unsupported RPF: {e}", path.display()));
                    continue;
                }
            };

            let tree = build_directory_tree(&archive.entries);
            let entries = list_all_files(&tree);
            let lower_path = path.to_string_lossy().replace('\\', "/").to_ascii_lowercase();

            if !dlclist_found
                && !matches!(archive.encryption, RpfEncryption::Aes | RpfEncryption::Ng)
            {
                if let Some(file) = entries.iter().find(|file| file.path.ends_with("dlclist.xml")) {
                    if let Some(entry) = archive.entries.get(file.entry_index) {
                        match archive.extract_entry(&bytes, entry, None) {
                            Ok(xml) if xml.len() <= MAX_XML_BYTES => {
                                match rpf_archive::parse_dlc_list(&xml) {
                                    Ok(names) => {
                                        report.registered_names.extend(names);
                                        dlclist_found = true;
                                    }
                                    Err(e) => report.warnings.push(format!("{}: dlclist.xml: {e}", path.display())),
                                }
                            }
                            Ok(_) => report.warnings.push(format!("{}: dlclist.xml exceeds size limit", path.display())),
                            Err(e) => report.warnings.push(format!("{}: cannot extract dlclist.xml: {e}", path.display())),
                        }
                    }
                }
            }

            let Some((_, pack_name)) = dlc_pack_name(&lower_path) else { continue };
            let mut setup_order = None;
            if !matches!(archive.encryption, RpfEncryption::Aes | RpfEncryption::Ng) {
                if let Some(file) = entries.iter().find(|file| file.path.rsplit('/').next() == Some("setup2.xml")) {
                    if let Some(entry) = archive.entries.get(file.entry_index) {
                        match archive.extract_entry(&bytes, entry, None) {
                            Ok(xml) if xml.len() <= MAX_XML_BYTES => {
                                match rpf_archive::parse_dlc_setup_order(&xml) {
                                    Ok(order) => setup_order = Some(order),
                                    Err(e) => report.warnings.push(format!("{}: setup2.xml: {e}", path.display())),
                                }
                            }
                            Ok(_) => report.warnings.push(format!("{}: setup2.xml exceeds size limit", path.display())),
                            Err(e) => report.warnings.push(format!("{}: cannot extract setup2.xml: {e}", path.display())),
                        }
                    }
                }
            } else {
                report.warnings.push(format!("{}: encrypted archive; setup2.xml needs game keys", path.display()));
            }

            let yft_files = entries.iter().filter(|file| file.path.ends_with(".yft")).count();
            let ytd_files = entries.iter().filter(|file| file.path.ends_with(".ytd")).count();
            let (mut valid_yft_files, mut valid_ytd_files, mut invalid_asset_files, mut unverified_asset_files) = (0, 0, 0, 0);
            if !matches!(archive.encryption, RpfEncryption::Aes | RpfEncryption::Ng) {
                for file in entries.iter().filter(|file| file.path.ends_with(".yft") || file.path.ends_with(".ytd")) {
                    if file.mem_size as u64 > 64 * 1024 * 1024 {
                        unverified_asset_files += 1;
                        report.warnings.push(format!("{}: {} exceeds the 64 MiB asset validation limit", path.display(), file.path));
                        continue;
                    }
                    let Some(entry) = archive.entries.get(file.entry_index) else {
                        unverified_asset_files += 1;
                        continue;
                    };
                    match archive.extract_entry(&bytes, entry, None) {
                        Ok(asset) if file.path.ends_with(".yft") => {
                            if rage_formats::parse_yft(&asset).is_ok() {
                                valid_yft_files += 1;
                            } else {
                                invalid_asset_files += 1;
                            }
                        }
                        Ok(asset) => {
                            if rage_formats::parse_ytd(&asset).is_ok() {
                                valid_ytd_files += 1;
                            } else {
                                invalid_asset_files += 1;
                            }
                        }
                        Err(e) => {
                            unverified_asset_files += 1;
                            report.warnings.push(format!("{}: cannot extract {}: {e}", path.display(), file.path));
                        }
                    }
                }
            } else {
                unverified_asset_files = yft_files + ytd_files;
            }
            let archive_path = path.strip_prefix(&package_dir).unwrap_or(&path).display().to_string();
            report.packs.push(DlcPackInfo {
                package: package.clone(),
                name: pack_name,
                archive: archive_path,
                setup_order,
                registered_in_imported_dlclist: false,
                yft_files,
                ytd_files,
                valid_yft_files,
                valid_ytd_files,
                invalid_asset_files,
                unverified_asset_files,
            });
        }
    }

    // Keep the original dlclist order: it is the stable tie-break when
    // multiple packs declare the same setup2.xml order value.
    let mut seen_names = HashSet::new();
    report.registered_names.retain(|name| seen_names.insert(name.to_ascii_lowercase()));
    for pack in &mut report.packs {
        pack.registered_in_imported_dlclist =
            report.registered_names.iter().any(|name| name.eq_ignore_ascii_case(&pack.name));
    }
    let registered_position = |name: &str| {
        report.registered_names.iter().position(|item| item.eq_ignore_ascii_case(name))
    };
    report.packs.sort_by(|a, b| {
        let a_pos = registered_position(&a.name);
        let b_pos = registered_position(&b.name);
        match (a_pos, b_pos) {
            (Some(_), Some(_)) => a.setup_order.unwrap_or(-1).cmp(&b.setup_order.unwrap_or(-1))
                .then_with(|| a_pos.cmp(&b_pos)),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => a.name.cmp(&b.name),
        }
    });
    Ok(report)
}

fn dlc_pack_name(path: &str) -> Option<(usize, String)> {
    let parts = path.split('/').collect::<Vec<_>>();
    let index = parts.iter().position(|part| *part == "dlcpacks")?;
    let name = *parts.get(index + 1)?;
    if name.is_empty() || parts.get(index + 2).copied() != Some("dlc.rpf") {
        return None;
    }
    Some((index, name.to_string()))
}

fn collect_files(root: &Path, warnings: &mut Vec<String>) -> Result<Vec<PathBuf>, String> {
    let mut stack = vec![root.to_path_buf()];
    let mut files = Vec::new();
    while let Some(dir) = stack.pop() {
        let entries = std::fs::read_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        for entry in entries {
            let entry = entry.map_err(|e| format!("{}: {e}", dir.display()))?;
            let ty = entry.file_type().map_err(|e| format!("{}: {e}", entry.path().display()))?;
            if ty.is_symlink() {
                warnings.push(format!("{}: symbolic link skipped", entry.path().display()));
            } else if ty.is_dir() {
                stack.push(entry.path());
            } else if ty.is_file() {
                files.push(entry.path());
                if files.len() >= MAX_PACKAGE_FILES {
                    warnings.push(format!("{}: stopped at {MAX_PACKAGE_FILES} files", root.display()));
                    files.sort();
                    return Ok(files);
                }
            }
        }
    }
    files.sort();
    Ok(files)
}

fn read_small_xml(path: &Path) -> Result<Vec<u8>, String> {
    let metadata = std::fs::metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if metadata.len() > MAX_XML_BYTES as u64 {
        return Err(format!("{}: XML exceeds {} KiB", path.display(), MAX_XML_BYTES / 1024));
    }
    std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rpf_archive::{RpfBuilder, RpfEncryption};
    use std::io::Write;

    #[test]
    fn discovers_dlc_pack_and_reads_setup_order_from_rpf() {
        let root = std::env::temp_dir().join(format!("glue5-dlc-scan-{}", std::process::id()));
        let pack = root.join("pc_pack/dlcpacks/testcar");
        std::fs::create_dir_all(&pack).unwrap();
        let mut builder = RpfBuilder::new(RpfEncryption::Open);
        builder.add_file("setup2.xml", br#"<SSetupData><order value="7"/></SSetupData>"#.to_vec());
        builder.add_file("vehicles/testcar.yft", b"dummy-yft".to_vec());
        builder.add_file("vehicles/testcar.ytd", b"dummy-ytd".to_vec());
        let bytes = builder.build(None).unwrap();
        let mut file = std::fs::File::create(pack.join("dlc.rpf")).unwrap();
        file.write_all(&bytes).unwrap();

        let report = scan_root(&root).unwrap();
        assert_eq!(report.packs.len(), 1);
        assert_eq!(report.packs[0].name, "testcar");
        assert_eq!(report.packs[0].setup_order, Some(7));
        assert_eq!(report.packs[0].yft_files, 1);
        assert_eq!(report.packs[0].ytd_files, 1);
        assert!(!report.packs[0].registered_in_imported_dlclist);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn reads_dlc_registration_from_loose_xml() {
        let root = std::env::temp_dir().join(format!("glue5-dlc-list-{}", std::process::id()));
        let pack = root.join("pc_pack/dlcpacks/testcar");
        std::fs::create_dir_all(&pack).unwrap();
        std::fs::write(root.join("pc_pack/dlclist.xml"),
            br#"<SMandatoryPacksData><Paths><Item>dlcpacks:/testcar/</Item></Paths></SMandatoryPacksData>"#).unwrap();
        let mut builder = RpfBuilder::new(RpfEncryption::Open);
        builder.add_file("setup2.xml", br#"<SSetupData><order value="2"/></SSetupData>"#.to_vec());
        let bytes = builder.build(None).unwrap();
        std::fs::write(pack.join("dlc.rpf"), bytes).unwrap();

        let report = scan_root(&root).unwrap();
        assert_eq!(report.registered_names, vec!["testcar".to_string()]);
        assert!(report.packs[0].registered_in_imported_dlclist);
        let _ = std::fs::remove_dir_all(root);
    }
}
