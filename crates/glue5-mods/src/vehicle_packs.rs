//! Read-only inspection of ZIPs that may contain native GTA V vehicle packs.
//!
//! This deliberately does not install or register assets. It identifies pack
//! contents so a later loader can make an explicit compatibility decision.
//! Script-mod ZIP installation remains handled by zip_import.

use std::io::{Cursor, Read};

const MAX_ENTRIES: usize = 2048;
const MAX_ENTRY_SIZE: u64 = 512 * 1024 * 1024;
const MAX_TOTAL_SIZE: u64 = 2 * 1024 * 1024 * 1024;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VehiclePackReport {
    pub entries: usize,
    pub unpacked_bytes: u64,
    pub yft_models: usize,
    pub ytd_textures: usize,
    pub vehicles_meta: usize,
    pub handling_meta: usize,
    pub content_xml: usize,
    pub setup_xml: usize,
    pub rpf_archives: usize,
    pub rpf7_archives: usize,
    pub other_files: usize,
    pub has_native_vehicle_assets: bool,
    pub likely_addon_pack: bool,
    pub notes: Vec<String>,
}

impl VehiclePackReport {
    /// A concise status suitable for a future import screen.
    pub fn summary(&self) -> String {
        if !self.has_native_vehicle_assets {
            return "No native GTA V vehicle assets found; this may be a script-mod ZIP.".into();
        }
        if self.likely_addon_pack {
            return format!(
                "Vehicle pack contents found: {} YFT, {} YTD, {} RPF archive(s). Metadata present.",
                self.yft_models, self.ytd_textures, self.rpf_archives
            );
        }
        format!(
            "Possible vehicle assets found: {} YFT, {} YTD, {} RPF archive(s). Pack layout needs validation.",
            self.yft_models, self.ytd_textures, self.rpf_archives
        )
    }
}

/// Inspect a ZIP without extracting files or changing the game directory.
///
/// Paths, symbolic links, entry counts and declared unpacked sizes are
/// validated. Only four bytes are read from each RPF candidate to identify
/// its archive magic; model and texture payloads are not loaded.
pub fn inspect_zip(bytes: &[u8]) -> Result<VehiclePackReport, String> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|e| format!("Invalid ZIP archive: {e}"))?;
    if archive.is_empty() || archive.len() > MAX_ENTRIES {
        return Err(format!("ZIP must contain 1–{MAX_ENTRIES} entries"));
    }

    let mut report = VehiclePackReport { entries: archive.len(), ..Default::default() };
    let mut total = 0u64;

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)
            .map_err(|e| format!("ZIP entry {index}: {e}"))?;
        let raw = entry.name().to_owned();
        if raw.starts_with('/') || raw.contains('\\') || raw.contains(':') {
            return Err(format!("Unsafe ZIP path: {raw}"));
        }
        let mut parts = raw.split('/').collect::<Vec<_>>();
        if entry.is_dir() && parts.last() == Some(&"") {
            parts.pop();
        }
        if parts.is_empty() || parts.iter().any(|p| p.is_empty() || *p == "." || *p == ".." || p.starts_with('.')) {
            return Err(format!("Unsafe ZIP path: {raw}"));
        }
        if entry.unix_mode().is_some_and(|mode| mode & 0o170000 == 0o120000) {
            return Err(format!("Symbolic links are not allowed: {raw}"));
        }
        if entry.is_dir() {
            continue;
        }
        let size = entry.size();
        if size > MAX_ENTRY_SIZE {
            return Err(format!("ZIP entry is too large: {raw}"));
        }
        total = total.checked_add(size).ok_or("ZIP unpacked size overflow")?;
        if total > MAX_TOTAL_SIZE {
            return Err("ZIP unpacked contents exceed the 2 GiB inspection limit".into());
        }
        report.unpacked_bytes = total;

        let leaf = parts.last().unwrap().to_ascii_lowercase();
        match leaf.rsplit_once('.').map(|(_, ext)| ext) {
            Some("yft") => report.yft_models += 1,
            Some("ytd") => report.ytd_textures += 1,
            Some("meta") if leaf == "vehicles.meta" => report.vehicles_meta += 1,
            Some("meta") if leaf == "handling.meta" => report.handling_meta += 1,
            Some("xml") if leaf == "content.xml" => report.content_xml += 1,
            Some("xml") if leaf == "setup2.xml" => report.setup_xml += 1,
            Some("rpf") => {
                report.rpf_archives += 1;
                let mut magic = [0u8; 4];
                let read = entry.read(&mut magic).map_err(|e| format!("Could not inspect {raw}: {e}"))?;
                if read == 4 && &magic == b"RPF7" {
                    report.rpf7_archives += 1;
                }
            }
            _ => report.other_files += 1,
        }
    }

    report.has_native_vehicle_assets =
        report.yft_models > 0 || report.ytd_textures > 0 || report.rpf_archives > 0;
    report.likely_addon_pack = report.has_native_vehicle_assets
        && report.rpf_archives > 0
        && report.content_xml > 0
        && (report.vehicles_meta > 0 || report.setup_xml > 0);

    if report.rpf_archives > report.rpf7_archives {
        report.notes.push("One or more RPF files did not have the RPF7 magic; format support is unconfirmed.".into());
    }
    if report.rpf7_archives > 0 {
        report.notes.push("RPF7 was detected, but archive contents have not been parsed.".into());
    }
    if report.yft_models > 0 || report.ytd_textures > 0 {
        report.notes.push("YFT/YTD filenames were detected; resource versions and compatibility are not verified.".into());
    }
    if !report.has_native_vehicle_assets {
        report.notes.push("No YFT, YTD or RPF vehicle asset files were detected.".into());
    }
    Ok(report)
}
