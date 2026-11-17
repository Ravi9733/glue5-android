//! End-to-end local vehicle package staging pipeline.
//!
//! A verified ZIP can be unpacked and inspected here, but this deliberately
//! does not mount RPF/DLC archives or register vehicles with the game engine.

use crate::{
    vehicle_dlc::{inspect_dlc_metadata, DlcMetadataReport},
    vehicle_hub::{PackageFormat, VehiclePackage},
    vehicle_extract::{extract_vehicle_zip, ExtractedPackage},
    vehicle_inspect::{inspect_vehicle_package, VehiclePackageReport},
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

    let dlc_metadata = match inspect_dlc_metadata(&extracted.directory) {
        Ok(report) => report,
        Err(error) => {
            let _ = std::fs::remove_dir_all(&extracted.directory);
            return Err(format!("DLC metadata validation failed; staged install rolled back: {error}"));
        }
    };

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

    Ok(VehicleInstallResult {
        id: package.id.clone(),
        directory: extracted.directory,
        extracted_files: extracted.files,
        extracted_bytes: extracted.bytes,
        report,
        dlc_metadata,
        spawn_supported: false,
    })
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
