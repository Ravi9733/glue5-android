//! Static compatibility report for extracted vehicle packages.
//!
//! This is inspection only: it never executes ASI/DLL/Lua/native payloads and
//! never claims that an RPF/DLC can be mounted by the current game runtime.

use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};

const MAX_SCAN_FILES: usize = 20_000;
const MAX_SCAN_DEPTH: usize = 24;
const MAX_PACKAGE_SCAN_BYTES: u64 = 2 * 1024 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RpfEncryption {
    Open,
    Aes,
    NgOrUnknown,
    NotRpf7,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VehiclePackageReport {
    pub files_scanned: usize,
    pub total_bytes: u64,
    pub rpf_archives: usize,
    pub rpf7_open: usize,
    pub rpf7_aes: usize,
    pub rpf7_ng_or_unknown: usize,
    pub vehicle_assets: usize,
    pub metadata_files: Vec<String>,
    pub blocked_executables: Vec<String>,
    pub warnings: Vec<String>,
    /// Always false until the engine's native vehicle loader and spawner are
    /// integrated and verified on device.
    pub spawn_supported: bool,
}

/// Inspect a previously extracted package without executing or loading it.
pub fn inspect_vehicle_package(root: &Path) -> Result<VehiclePackageReport, String> {
    let root = root.canonicalize().map_err(|e| format!("cannot resolve vehicle package: {e}"))?;
    if !root.is_dir() { return Err("vehicle package path is not a directory".into()); }
    let mut report = VehiclePackageReport {
        files_scanned: 0, total_bytes: 0, rpf_archives: 0,
        rpf7_open: 0, rpf7_aes: 0, rpf7_ng_or_unknown: 0,
        vehicle_assets: 0, metadata_files: Vec::new(),
        blocked_executables: Vec::new(), warnings: Vec::new(),
        spawn_supported: false,
    };
    scan_dir(&root, &root, 0, &mut report)?;
    if report.rpf_archives > 0 {
        report.warnings.push("RPF archives found. Static detection is not proof that their DLC content can be mounted.".into());
    }
    if !report.blocked_executables.is_empty() {
        report.warnings.push("Executable plugin files were found and will not be loaded by Vehicle Hub.".into());
    }
    if report.vehicle_assets == 0 {
        report.warnings.push("No recognized vehicle asset files were found in the extracted folder.".into());
    }
    report.warnings.push("Native third-party vehicle registration/spawning is not implemented; this package is not spawn-ready.".into());
    Ok(report)
}

fn scan_dir(root: &Path, dir: &Path, depth: usize, report: &mut VehiclePackageReport) -> Result<(), String> {
    if depth > MAX_SCAN_DEPTH { return Err("vehicle package folder nesting exceeds safety limit".into()); }
    let entries = fs::read_dir(dir).map_err(|e| format!("cannot read vehicle package folder: {e}"))?;
    for item in entries {
        let item = item.map_err(|e| format!("cannot read vehicle package entry: {e}"))?;
        let path = item.path();
        let metadata = fs::symlink_metadata(&path).map_err(|e| format!("cannot inspect package entry: {e}"))?;
        if metadata.file_type().is_symlink() { return Err(format!("symbolic link rejected in vehicle package: {}", path.display())); }
        if metadata.is_dir() {
            scan_dir(root, &path, depth + 1, report)?;
            continue;
        }
        if !metadata.is_file() { return Err(format!("special file rejected in vehicle package: {}", path.display())); }
        report.files_scanned += 1;
        if report.files_scanned > MAX_SCAN_FILES { return Err("vehicle package contains too many files".into()); }
        report.total_bytes = report.total_bytes.checked_add(metadata.len()).ok_or("vehicle package size overflow")?;
        if report.total_bytes > MAX_PACKAGE_SCAN_BYTES { return Err("vehicle package expands beyond 2 GiB inspection limit".into()); }
        let relative = path.strip_prefix(root).unwrap_or(&path).to_string_lossy().replace('\\', "/");
        let lower = relative.to_ascii_lowercase();
        let ext = path.extension().and_then(|x| x.to_str()).unwrap_or("").to_ascii_lowercase();
        if matches!(ext.as_str(), "yft" | "ytd" | "ymt" | "ycd" | "ydr" | "ybn") {
            report.vehicle_assets += 1;
        }
        if matches!(ext.as_str(), "asi" | "dll" | "so" | "apk" | "dex" | "jar" | "exe" | "bat" | "sh") {
            if report.blocked_executables.len() < 64 { report.blocked_executables.push(relative.clone()); }
        }
        if matches!(lower.as_str(), "dlclist.xml" | "content.xml" | "setup2.xml" | "vehicles.meta" | "handling.meta" | "carcols.meta" | "carvariations.meta") {
            if report.metadata_files.len() < 128 { report.metadata_files.push(relative.clone()); }
        }
        if ext == "rpf" {
            report.rpf_archives += 1;
            let mut f = fs::File::open(&path).map_err(|e| format!("cannot open RPF header: {e}"))?;
            let mut header = [0u8; 16];
            let n = f.read(&mut header).map_err(|e| format!("cannot read RPF header: {e}"))?;
            if n < 16 || &header[0..4] != b"7FPR" {
                report.warnings.push(format!("{relative}: not a recognized GTA V RPF7 header"));
            } else {
                match u32::from_le_bytes(header[12..16].try_into().unwrap()) {
                    0x4E45504F => report.rpf7_open += 1,
                    0x0FFFFFF9 => report.rpf7_aes += 1,
                    _ => report.rpf7_ng_or_unknown += 1,
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp() -> PathBuf {
        let n = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let p = std::env::temp_dir().join(format!("glue5-vehicle-inspect-{}-{n}", std::process::id()));
        fs::create_dir_all(&p).unwrap(); p
    }

    #[test]
    fn identifies_assets_and_blocks_native_plugins() {
        let root = temp();
        fs::write(root.join("furai.yft"), b"asset").unwrap();
        fs::write(root.join("unsafe.asi"), b"plugin").unwrap();
        let report = inspect_vehicle_package(&root).unwrap();
        assert_eq!(report.vehicle_assets, 1);
        assert_eq!(report.blocked_executables.len(), 1);
        assert!(!report.spawn_supported);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_symbolic_links() {
        let root = temp();
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(root.join("missing"), root.join("link")).unwrap();
            assert!(inspect_vehicle_package(&root).unwrap_err().contains("symbolic link"));
        }
        fs::remove_dir_all(root).unwrap();
    }
}
