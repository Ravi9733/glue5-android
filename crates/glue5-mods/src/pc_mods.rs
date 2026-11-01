//! PC-style JSON world-placement data validation.
//!
//! This module deliberately parses and validates placement descriptions only.
//! It does not claim to mount YMAP/RPF assets or spawn engine entities; those
//! require a separate runtime integration and in-game verification.

use serde_json::Value;

pub const MAX_PLACEMENT_JSON_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_PLACEMENTS: usize = 5_000;
const MAX_COORDINATE: f64 = 20_000.0;
const MAX_HEADING: f64 = 36_000.0;

#[derive(Clone, Debug, PartialEq)]
pub struct Placement {
    pub model: String,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub heading: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlacementReport {
    pub placements: Vec<Placement>,
}

/// Parse a bounded JSON document in the simple PC-mod placement shape:
/// { "objects": [{ "model": "prop_name", "x": 1, "y": 2, "z": 3,
///                 "heading": 90 }] }.
///
/// Validation happens before any caller is allowed to spawn objects.
pub fn parse_placements(bytes: &[u8]) -> Result<PlacementReport, String> {
    if bytes.is_empty() {
        return Err("placement JSON is empty".into());
    }
    if bytes.len() > MAX_PLACEMENT_JSON_BYTES {
        return Err(format!("placement JSON exceeds {MAX_PLACEMENT_JSON_BYTES} bytes"));
    }
    let root: Value =
        serde_json::from_slice(bytes).map_err(|e| format!("invalid placement JSON: {e}"))?;
    let objects = root
        .get("objects")
        .and_then(Value::as_array)
        .ok_or("placement JSON must contain an 'objects' array")?;
    if objects.len() > MAX_PLACEMENTS {
        return Err(format!("placement JSON exceeds {MAX_PLACEMENTS} objects"));
    }

    let mut placements = Vec::with_capacity(objects.len());
    for (index, value) in objects.iter().enumerate() {
        let fail = |message: &str| format!("objects[{index}]: {message}");
        let object = value.as_object().ok_or_else(|| fail("expected an object"))?;
        let model = object
            .get("model")
            .and_then(Value::as_str)
            .ok_or_else(|| fail("'model' must be a string"))?;
        if model.is_empty()
            || model.len() > 64
            || !model
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_')
        {
            return Err(fail("'model' must be 1–64 ASCII letters, digits or underscores"));
        }

        let coordinate = |key: &str| -> Result<f64, String> {
            let value = object
                .get(key)
                .and_then(Value::as_f64)
                .ok_or_else(|| fail(&format!("'{key}' must be a number")))?;
            if !value.is_finite() || value.abs() > MAX_COORDINATE {
                return Err(fail(&format!("'{key}' is outside the supported coordinate range")));
            }
            Ok(value)
        };
        let heading = match object.get("heading") {
            None | Some(Value::Null) => 0.0,
            Some(value) => {
                let value = value
                    .as_f64()
                    .ok_or_else(|| fail("'heading' must be a number"))?;
                if !value.is_finite() || value.abs() > MAX_HEADING {
                    return Err(fail("'heading' is outside the supported range"));
                }
                value
            }
        };

        placements.push(Placement {
            model: model.to_owned(),
            x: coordinate("x")?,
            y: coordinate("y")?,
            z: coordinate("z")?,
            heading,
        });
    }

    Ok(PlacementReport { placements })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_placements_and_default_heading() {
        let report = parse_placements(
            br#"{"objects":[{"model":"prop_roadcone02b","x":100,"y":200,"z":20},{"model":"prop_crate_11","x":1.5,"y":2,"z":3,"heading":45}]}"#,
        )
        .unwrap();
        assert_eq!(report.placements.len(), 2);
        assert_eq!(report.placements[0].heading, 0.0);
        assert_eq!(report.placements[1].heading, 45.0);
    }

    #[test]
    fn rejects_invalid_model_names() {
        let input = br#"{"objects":[{"model":"../bad","x":0,"y":0,"z":0}]}"#;
        assert!(parse_placements(input).unwrap_err().contains("model"));
    }

    #[test]
    fn rejects_non_finite_and_out_of_range_coordinates() {
        for input in [
            br#"{"objects":[{"model":"prop_test","x":1e999,"y":0,"z":0}]}"#.as_slice(),
            br#"{"objects":[{"model":"prop_test","x":20001,"y":0,"z":0}]}"#.as_slice(),
        ] {
            assert!(parse_placements(input).is_err());
        }
    }

    #[test]
    fn rejects_wrong_root_and_oversized_documents() {
        assert!(parse_placements(br#"{"items":[]}"#).is_err());
        assert!(parse_placements(&vec![b' '; MAX_PLACEMENT_JSON_BYTES + 1]).is_err());
    }

    #[test]
    fn rejects_too_many_objects() {
        let objects = vec![serde_json::json!({
            "model": "prop_test", "x": 0, "y": 0, "z": 0
        }); MAX_PLACEMENTS + 1];
        let input = serde_json::json!({ "objects": objects }).to_string();
        assert!(parse_placements(input.as_bytes()).unwrap_err().contains("objects"));
    }
}


/// PC mod package classification. This is metadata inspection, not proof that
/// the game can load the contained engine assets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PackageKind {
    Script,
    VehiclePack,
    MapOrWorld,
    MixedContent,
    Unknown,
}

/// A bounded summary of a ZIP before an installer or runtime sees it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackageReport {
    pub kind: PackageKind,
    pub entries: usize,
    pub unpacked_bytes: u64,
    pub rpf_files: usize,
    pub yft_files: usize,
    pub ytd_files: usize,
    pub ymap_files: usize,
    pub meta_files: usize,
    pub asi_or_dll_files: usize,
    pub manifest_name: Option<String>,
}

const MAX_PACKAGE_ENTRIES: usize = 4096;
const MAX_PACKAGE_FILE_BYTES: u64 = 256 * 1024 * 1024;
const MAX_PACKAGE_TOTAL_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const MAX_MANIFEST_BYTES: u64 = 64 * 1024;

/// Inspect a PC-style ZIP without extracting it. Rejects unsafe paths,
/// duplicate normalized paths, symlinks and decompression-size bombs.
/// RPF/YFT/YTD recognition is only file classification; it does not mount
/// archives or register game assets.
pub fn inspect_package(bytes: &[u8]) -> Result<PackageReport, String> {
    use std::collections::HashSet;
    use std::io::{Cursor, Read};

    if bytes.is_empty() {
        return Err("package ZIP is empty".into());
    }
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|e| format!("invalid package ZIP: {e}"))?;
    if archive.len() == 0 || archive.len() > MAX_PACKAGE_ENTRIES {
        return Err(format!("package must contain 1–{MAX_PACKAGE_ENTRIES} entries"));
    }

    let mut seen = HashSet::with_capacity(archive.len());
    let mut total = 0u64;
    let (mut rpf, mut yft, mut ytd, mut ymap, mut meta, mut native) = (0, 0, 0, 0, 0, 0);
    let mut has_script = false;
    let mut manifest_name = None;

    for index in 0..archive.len() {
        let mut file = archive.by_index(index)
            .map_err(|e| format!("package entry {index}: {e}"))?;
        let raw = file.name().to_owned();
        if raw.starts_with('/') || raw.contains('\\') || raw.contains(':') {
            return Err(format!("unsafe package path: {raw}"));
        }
        let is_dir = file.is_dir();
        let mut parts: Vec<&str> = raw.split('/').collect();
        if is_dir && parts.last() == Some(&"") { parts.pop(); }
        if parts.is_empty() || parts.iter().any(|p| p.is_empty() || *p == "." || *p == ".." || p.starts_with('.')) {
            return Err(format!("unsafe package path: {raw}"));
        }
        if file.unix_mode().is_some_and(|mode| mode & 0o170000 == 0o120000) {
            return Err(format!("package symlink is not allowed: {raw}"));
        }
        let normalized = parts.join("/");
        if !seen.insert(normalized.to_ascii_lowercase()) {
            return Err(format!("duplicate package path: {raw}"));
        }
        if is_dir { continue; }

        let size = file.size();
        if size > MAX_PACKAGE_FILE_BYTES {
            return Err(format!("package file exceeds 256 MiB: {raw}"));
        }
        total = total.checked_add(size).ok_or("package size overflow")?;
        if total > MAX_PACKAGE_TOTAL_BYTES {
            return Err("package exceeds 2 GiB unpacked size".into());
        }

        let lower = raw.to_ascii_lowercase();
        if lower.ends_with(".rpf") { rpf += 1; }
        if lower.ends_with(".yft") { yft += 1; }
        if lower.ends_with(".ytd") { ytd += 1; }
        if lower.ends_with(".ymap") { ymap += 1; }
        if lower.ends_with(".meta") || lower.ends_with(".xml") { meta += 1; }
        if lower.ends_with(".asi") || lower.ends_with(".dll") { native += 1; }
        if lower.ends_with("main.luau") { has_script = true; }
        if lower == "glue5-mod.json" {
            if size > MAX_MANIFEST_BYTES {
                return Err("glue5-mod.json exceeds 64 KiB".into());
            }
            let mut text = String::new();
            file.take(MAX_MANIFEST_BYTES + 1).read_to_string(&mut text)
                .map_err(|e| format!("glue5-mod.json: {e}"))?;
            let value: serde_json::Value = serde_json::from_str(&text)
                .map_err(|e| format!("invalid glue5-mod.json: {e}"))?;
            let object = value.as_object().ok_or("glue5-mod.json must be a JSON object")?;
            if let Some(name) = object.get("name") {
                let name = name.as_str().ok_or("manifest name must be a string")?;
                if name.is_empty() || name.len() > 64 || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-') {
                    return Err("manifest name must be 1–64 ASCII letters, digits, _ or -".into());
                }
                manifest_name = Some(name.to_owned());
            }
        }
    }

    let has_assets = rpf + yft + ytd + ymap + meta > 0;
    let kind = match (has_script, has_assets, native > 0) {
        (true, false, false) => PackageKind::Script,
        (false, true, false) if yft + ytd > 0 => PackageKind::VehiclePack,
        (false, true, false) if ymap > 0 => PackageKind::MapOrWorld,
        (true, true, _) | (_, true, true) => PackageKind::MixedContent,
        _ => PackageKind::Unknown,
    };

    Ok(PackageReport {
        kind, entries: archive.len(), unpacked_bytes: total,
        rpf_files: rpf, yft_files: yft, ytd_files: ytd, ymap_files: ymap,
        meta_files: meta, asi_or_dll_files: native, manifest_name,
    })
}


#[cfg(test)]
mod package_tests {
    use super::*;
    use std::io::{Cursor, Write};

    fn zip_with(path: &str, data: &[u8]) -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        writer.start_file(path, zip::write::SimpleFileOptions::default()).unwrap();
        writer.write_all(data).unwrap();
        writer.finish().unwrap().into_inner()
    }

    #[test]
    fn identifies_vehicle_asset_package_without_claiming_runtime_support() {
        let bytes = zip_with("vehicles/adder.yft", b"not-a-real-model");
        let report = inspect_package(&bytes).unwrap();
        assert_eq!(report.kind, PackageKind::VehiclePack);
        assert_eq!(report.yft_files, 1);
    }

    #[test]
    fn identifies_native_plugins_as_mixed_or_unsupported_content() {
        let bytes = zip_with("plugins/example.asi", b"not-a-real-plugin");
        let report = inspect_package(&bytes).unwrap();
        assert_eq!(report.kind, PackageKind::Unknown);
        assert_eq!(report.asi_or_dll_files, 1);
    }

    #[test]
    fn rejects_traversal_paths_in_pc_packages() {
        let bytes = zip_with("../outside.ymap", b"bad");
        assert!(inspect_package(&bytes).unwrap_err().contains("unsafe"));
    }

    #[test]
    fn rejects_case_insensitive_duplicate_paths() {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default();
        writer.start_file("data/test.ytd", options).unwrap();
        writer.write_all(b"a").unwrap();
        writer.start_file("DATA/TEST.YTD", options).unwrap();
        writer.write_all(b"b").unwrap();
        let bytes = writer.finish().unwrap().into_inner();
        assert!(inspect_package(&bytes).unwrap_err().contains("duplicate"));
    }
}
