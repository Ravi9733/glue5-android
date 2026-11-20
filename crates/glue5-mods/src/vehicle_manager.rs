//! End-to-end local vehicle package staging pipeline.
//!
//! A verified ZIP can be unpacked and inspected here, but this deliberately
//! does not mount RPF/DLC archives or register vehicles with the game engine.

use crate::{
    vehicle_dlc::{inspect_dlc_metadata, DlcMetadataReport},
    vehicle_hub::{PackageFormat, VehiclePackage},
    vehicle_extract::{extract_vehicle_zip, ExtractedPackage},
    vehicle_inspect::{inspect_vehicle_package, VehiclePackageReport},
    vehicle_compat::{assess_dlc_compatibility, DlcCompatibilityReport},
};
use sha2::{Digest, Sha256};
use std::{fs::File, io::Read, path::{Path, PathBuf}};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VehicleInstallResult {
    pub id: String,
    pub directory: PathBuf,
    pub extracted_files: usize,
    pub extracted_bytes: u64,
    pub report: VehiclePackageReport,
    pub dlc_metadata: DlcMetadataReport,
    pub compatibility: DlcCompatibilityReport,
    /// Always false until native mounting and in-game spawning are implemented.
    pub spawn_supported: bool,
}

/// Verify the downloaded archive against catalog metadata, extract it safely,
/// then run static compatibility inspection. The ZIP is retained as a backup;
/// no original game data is modified.
pub fn install_verified_package(
    archive: &Path,
    install_root: &Path,
    package: &VehiclePackage,
) -> Result<VehicleInstallResult, String> {
    if !valid_id(&package.id) {
        return Err("invalid vehicle package ID".into());
    }
    if package.spawn_supported {
        return Err("catalog cannot mark a vehicle spawnable before native loader verification".into());
    }
    let metadata = archive.metadata().map_err(|e| format!("cannot inspect downloaded package: {e}"))?;
    if !metadata.is_file() || metadata.len() != package.size_bytes {
        return Err("downloaded package size does not match catalog metadata".into());
    }
    let actual_hash = hash_file(archive)?;
    if !actual_hash.eq_ignore_ascii_case(&package.sha256) {
        return Err("downloaded package SHA-256 does not match catalog metadata".into());
    }

    let extracted: ExtractedPackage = extract_vehicle_zip(archive, install_root, &package.id)?;
    let report = match inspect_vehicle_package(&extracted.directory) {
        Ok(report) => report,
        Err(error) => {
            let _ = std::fs::remove_dir_all(&extracted.directory);
            return Err(format!("vehicle package inspection failed; staged install rolled back: {error}"));
        }
    };

    let mut dlc_metadata = match inspect_dlc_metadata(&extracted.directory) {
        Ok(report) => report,
        Err(error) => {
            let _ = std::fs::remove_dir_all(&extracted.directory);
            return Err(format!("DLC metadata validation failed; staged install rolled back: {error}"));
        }
    };
    let rpf_metadata = match crate::vehicle_dlc::inspect_rpf_dlc_metadata(&extracted.directory) {
        Ok(report) => report,
        Err(error) => {
            let _ = std::fs::remove_dir_all(&extracted.directory);
            return Err(format!("RPF metadata validation failed; staged install rolled back: {error}"));
        }
    };
    dlc_metadata.dlclist_entries.extend(rpf_metadata.dlclist_entries);
    dlc_metadata.dlclist_entries.sort();
    dlc_metadata.dlclist_entries.dedup();
    dlc_metadata.setup_orders.extend(rpf_metadata.setup_orders);
    dlc_metadata.content_xml_files.extend(rpf_metadata.content_xml_files);
    dlc_metadata.warnings.extend(rpf_metadata.warnings);

    match package.format {
        PackageFormat::RpfDlc if report.rpf_archives == 0 => {
            let _ = std::fs::remove_dir_all(&extracted.directory);
            return Err("catalog says RPF/DLC, but no .rpf archive was found; install rolled back".into());
        }
        PackageFormat::LooseFiles if report.vehicle_assets == 0 && report.rpf_archives == 0 => {
            let _ = std::fs::remove_dir_all(&extracted.directory);
            return Err("no recognized vehicle assets or RPF archive found; install rolled back".into());
        }
        _ => {}
    }

    let compatibility = assess_dlc_compatibility(&report, &dlc_metadata, &package.id);
    let mount_plan = build_native_mount_plan(&package.id, &report)?;

    // Persist a small manifest alongside the extracted files so the trainer
    // can show useful package details after an app restart. This is metadata
    // only: it does not register or mount the DLC with the game engine.
    let manifest = serde_json::json!({
        "schema": 1,
        "id": package.id,
        "name": package.name,
        "version": package.version,
        "author": package.author,
        "format": match package.format {
            PackageFormat::LooseFiles => "loose_files",
            PackageFormat::RpfDlc => "rpf_dlc",
        },
        "sha256": package.sha256,
        "size_bytes": package.size_bytes,
        "extracted_files": extracted.files,
        "extracted_bytes": extracted.bytes,
        "rpf_archives": report.rpf_archives,
        "vehicle_assets": report.vehicle_assets,
        "spawn_supported": false,
        "status": "staged_not_mounted",
        "native_mount_plan": mount_plan
    });
    let manifest_path = extracted.directory.join(".glue5-package.json");
    let manifest_tmp = extracted.directory.join(".glue5-package.json.tmp");
    let manifest_result = (|| -> Result<(), String> {
        let bytes = serde_json::to_vec_pretty(&manifest)
            .map_err(|e| format!("cannot encode vehicle package manifest: {e}"))?;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&manifest_tmp)
            .map_err(|e| format!("cannot create vehicle package manifest: {e}"))?;
        use std::io::Write as _;
        file.write_all(&bytes)
            .map_err(|e| format!("cannot write vehicle package manifest: {e}"))?;
        file.sync_all()
            .map_err(|e| format!("cannot sync vehicle package manifest: {e}"))?;
        std::fs::rename(&manifest_tmp, &manifest_path)
            .map_err(|e| format!("cannot finalize vehicle package manifest: {e}"))?;
        Ok(())
    })();
    if let Err(error) = manifest_result {
        let _ = std::fs::remove_file(&manifest_tmp);
        let _ = std::fs::remove_dir_all(&extracted.directory);
        return Err(format!("could not persist install metadata; staged install rolled back: {error}"));
    }

    Ok(VehicleInstallResult {
        id: package.id.clone(),
        directory: extracted.directory,
        extracted_files: extracted.files,
        extracted_bytes: extracted.bytes,
        report,
        dlc_metadata,
        compatibility,
        spawn_supported: false,
    })
}


/// Build a deterministic, host-path-free descriptor for the native VFS bridge.
/// It records only package-relative RPF paths; it does not mount or register
/// anything with the running game.
fn build_native_mount_plan(
    package_id: &str,
    report: &VehiclePackageReport,
) -> Result<serde_json::Value, String> {
    if !valid_id(package_id) {
        return Err("invalid package id in native mount plan".into());
    }
    let mut rpf_paths = Vec::with_capacity(report.rpf_files.len());
    for archive in &report.rpf_files {
        let normalized = archive.path.replace('\\', "/");
        if normalized.is_empty()
            || normalized.starts_with('/')
            || normalized.contains(':')
            || normalized.split('/').any(|part| part.is_empty() || part == "." || part == "..")
            || !normalized.to_ascii_lowercase().ends_with(".rpf")
        {
            return Err(format!("unsafe package-relative RPF path: {}", archive.path));
        }
        rpf_paths.push(normalized);
    }
    rpf_paths.sort();
    rpf_paths.dedup();
    let virtual_root = format!("dlcpacks:/{package_id}/");
    // ZIPs often wrap the actual DLC folder in vendor directories, e.g.
    // "Mazda Furai/Legacy/dyfurai/dlc.rpf". Strip through the matching
    // package-id directory so the virtual layout matches GTA's dlcpacks root.
    // If no such directory exists, retain the relative path rather than
    // silently guessing a different archive location.
    let rpf_mounts: Vec<serde_json::Value> = rpf_paths.iter().map(|relative| {
        let components: Vec<&str> = relative.split('/').collect();
        let package_component = components.iter().rposition(|part| part.eq_ignore_ascii_case(package_id));
        let virtual_relative = package_component
            .map(|index| components[index + 1..].join("/"))
            .filter(|tail| !tail.is_empty())
            .unwrap_or_else(|| relative.clone());
        let archive = report.rpf_files.iter().find(|archive| {
            archive.path.replace('\\\\', "/") == *relative
        });
        let encryption = archive.map(|archive| match archive.encryption {
            crate::vehicle_inspect::RpfEncryption::Open => "open",
            crate::vehicle_inspect::RpfEncryption::Aes => "aes",
            crate::vehicle_inspect::RpfEncryption::NgOrUnknown => "ng_or_unknown",
            crate::vehicle_inspect::RpfEncryption::NotRpf7 => "not_rpf7",
        }).unwrap_or("unknown");
        serde_json::json!({
            "package_relative_path": relative,
            "virtual_archive_path": format!("{virtual_root}{virtual_relative}"),
            "matched_package_directory": package_component.is_some(),
            "size_bytes": archive.map(|archive| archive.size_bytes).unwrap_or(0),
            "encryption": encryption
        })
    }).collect();
    Ok(serde_json::json!({
        "schema": 2,
        "package_id": package_id,
        "virtual_root": virtual_root,
        "rpf_paths": rpf_paths,
        "rpf_mounts": rpf_mounts,
        "path_kind": "package_relative",
        "state": "awaiting_native_vfs_mount"
    }))
}

fn hash_file(path: &Path) -> Result<String, String> {
    let mut file = File::open(path).map_err(|e| format!("cannot open downloaded package: {e}"))?;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 256 * 1024];
    loop {
        let count = file.read(&mut buffer).map_err(|e| format!("cannot hash downloaded package: {e}"))?;
        if count == 0 { break; }
        hash.update(&buffer[..count]);
    }
    let digest = hash.finalize();
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        write!(&mut hex, "{byte:02x}").expect("writing to String cannot fail");
    }
    Ok(hex)
}

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 48
        && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vehicle_hub::VehiclePackage;


    #[test]
    fn native_mount_plan_contains_only_safe_relative_rpf_paths() {
        use crate::vehicle_inspect::{RpfArchiveInfo, RpfEncryption};
        let report = VehiclePackageReport {
            files_scanned: 2, total_bytes: 512, rpf_archives: 1,
            rpf7_open: 1, rpf7_aes: 0, rpf7_ng_or_unknown: 0,
            rpf_files: vec![RpfArchiveInfo {
                path: "Mazda Furai/dlc/dlc.rpf".into(),
                size_bytes: 512, encryption: RpfEncryption::Open,
            }],
            vehicle_assets: 0, metadata_files: vec![],
            blocked_executables: vec![], warnings: vec![], spawn_supported: false,
        };
        let plan = build_native_mount_plan("dyfurai", &report).unwrap();
        assert_eq!(plan["virtual_root"], "dlcpacks:/dyfurai/");
        assert_eq!(plan["rpf_paths"][0], "Mazda Furai/dlc/dlc.rpf");
        assert_eq!(plan["rpf_mounts"][0]["package_relative_path"], "Mazda Furai/dlc/dlc.rpf");
        assert_eq!(plan["rpf_mounts"][0]["virtual_archive_path"], "dlcpacks:/dyfurai/Mazda Furai/dlc/dlc.rpf");
        assert_eq!(plan["rpf_mounts"][0]["matched_package_directory"], false);
        assert_eq!(plan["rpf_mounts"][0]["size_bytes"], 512);
        assert_eq!(plan["rpf_mounts"][0]["encryption"], "open");

        let mut wrapped_report = report.clone();
        wrapped_report.rpf_files[0].path = "Mazda Furai/Legacy/dyfurai/dlc.rpf".into();
        let wrapped = build_native_mount_plan("dyfurai", &wrapped_report).unwrap();
        assert_eq!(wrapped["rpf_mounts"][0]["virtual_archive_path"], "dlcpacks:/dyfurai/dlc.rpf");
        assert_eq!(wrapped["rpf_mounts"][0]["matched_package_directory"], true);
        assert_eq!(plan["schema"], 2);
        assert_eq!(plan["state"], "awaiting_native_vfs_mount");
        let mut unsafe_report = report;
        unsafe_report.rpf_files[0].path = "../escape/dlc.rpf".into();
        assert!(build_native_mount_plan("dyfurai", &unsafe_report).is_err());
    }

    #[test]
    fn hashes_file_as_lowercase_sha256_hex() {
        let path = std::env::temp_dir().join(format!("glue5-hash-test-{}", std::process::id()));
        std::fs::write(&path, b"abc").unwrap();
        assert_eq!(hash_file(&path).unwrap(), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn rejects_catalog_that_claims_native_spawn_support() {
        let package = VehiclePackage {
            id: "furai".into(), name: "Furai".into(), version: "1".into(),
            author: "test".into(), download_url: "https://example.test/f.zip".into(),
            sha256: "a".repeat(64), size_bytes: 1, format: PackageFormat::RpfDlc,
            tags: vec![], spawn_supported: true,
        };
        assert!(install_verified_package(Path::new("missing.zip"), Path::new("install"), &package)
            .unwrap_err().contains("cannot mark"));
    }

    #[test]
    fn rejects_wrong_package_hash_before_extracting() {
        let root = std::env::temp_dir().join(format!("glue5-vehicle-manager-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let archive = root.join("package.zip");
        std::fs::write(&archive, b"not a zip").unwrap();
        let package = VehiclePackage {
            id: "furai".into(), name: "Furai".into(), version: "1".into(),
            author: "test".into(), download_url: "https://example.test/f.zip".into(),
            sha256: "a".repeat(64), size_bytes: 9, format: PackageFormat::RpfDlc,
            tags: vec![], spawn_supported: false,
        };
        assert!(install_verified_package(&archive, &root.join("install"), &package)
            .unwrap_err().contains("SHA-256"));
        assert!(!root.join("install/furai").exists());
        std::fs::remove_dir_all(root).unwrap();
    }
}
