//! Limited GTA V YMAP-to-Luau bridge.
//!
//! Reads ordinary exterior HD entity placements from RSC7 YMAP files and
//! emits an opt-in script mod. This is not native YMAP streaming: it does not
//! register map extents, LOD graphs, MLO interiors, collision, navmeshes,
//! archetypes, or custom YFT/YTD assets. Every referenced model must already
//! be available to the running game.

use rage_formats::{parse_ymap_entities, YmapEntity};

const MAX_YMAP_BYTES: usize = 128 * 1024 * 1024;
const MAX_SPAWNED_ENTITIES: usize = 5_000;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct YmapImportReport {
    pub entities_in_file: usize,
    pub supported_entities: usize,
    pub skipped_mlo: usize,
    pub skipped_lod: usize,
    pub skipped_invalid: usize,
}

/// Heading from the inverse quaternion stored by GTA V map entities.
/// YMAP stores Quaternion.Invert(rotation), so negate the extracted yaw.
fn heading_degrees(rotation: [f32; 4]) -> Option<f64> {
    let [x, y, z, w] = rotation;
    if !rotation.iter().all(|v| v.is_finite()) {
        return None;
    }
    let norm = x*x + y*y + z*z + w*w;
    if norm < 1.0e-8 { return None; }
    let yaw = (2.0 * (w*z + x*y)).atan2(1.0 - 2.0 * (y*y + z*z));
    Some((-yaw.to_degrees()).rem_euclid(360.0) as f64)
}

fn supported(entity: &YmapEntity) -> Option<(u32, f64, f64, f64, f64)> {
    if entity.is_mlo_instance || entity.lod_level != YmapEntity::LOD_HD || entity.archetype_hash == 0 {
        return None;
    }
    let (x, y, z) = (entity.position.x, entity.position.y, entity.position.z);
    if !x.is_finite() || !y.is_finite() || !z.is_finite()
        || x.abs() > 20_000.0 || y.abs() > 20_000.0 || z.abs() > 20_000.0
    {
        return None;
    }
    Some((entity.archetype_hash, x as f64, y as f64, z as f64, heading_degrees(entity.rotation)?))
}

/// Compile supported entities as numeric model hashes. Hashes are already
/// stored in YMAP; do not hash their hexadecimal display strings again.
fn compile(entities: &[(u32, f64, f64, f64, f64)]) -> String {
    use std::fmt::Write as _;
    let mut out = String::from(
        "-- Generated from a GTA V YMAP. Only existing game assets can spawn.\nlocal placements = {\n",
    );
    for (hash, x, y, z, heading) in entities {
        let _ = writeln!(out,
            "    {{ model = 0x{hash:08X}, x = {x:.6}, y = {y:.6}, z = {z:.6}, heading = {heading:.6} }},");
    }
    out.push_str(r#"}
local created = 0
local failed = 0
for index, placement in placements do
    local model = placement.model
    if streaming.load(model) then
        local position = vector.create(placement.x, placement.y, placement.z)
        local objectHandle = object.createObject(model, position, false, false, false)
        streaming.setModelAsNoLongerNeeded(model)
        if objectHandle then
            entity.setEntityHeading(objectHandle, placement.heading)
            created += 1
        else
            failed += 1
        end
    else
        failed += 1
    end
    if index % 4 == 0 then task.wait(0) end
end
print(`YMAP import finished: {created} objects created, {failed} failed`)
"#);
    out
}

/// Parse a retail RSC7 YMAP. Only plain HD exterior entities are translated
/// into object-spawn calls; unsupported map features are counted and skipped.
pub fn inspect(bytes: &[u8]) -> Result<(YmapImportReport, String), String> {
    if bytes.is_empty() { return Err("YMAP file is empty".into()); }
    if bytes.len() > MAX_YMAP_BYTES {
        return Err(format!("YMAP exceeds {} MiB", MAX_YMAP_BYTES / 1024 / 1024));
    }
    let entities = parse_ymap_entities(bytes).map_err(|e| format!("YMAP parse failed: {e}"))?;
    let mut report = YmapImportReport { entities_in_file: entities.len(), ..Default::default() };
    let mut placements = Vec::new();
    for entity in &entities {
        if entity.is_mlo_instance {
            report.skipped_mlo += 1;
            continue;
        }
        if entity.lod_level != YmapEntity::LOD_HD {
            report.skipped_lod += 1;
            continue;
        }
        match supported(entity) {
            Some(item) => {
                if placements.len() >= MAX_SPAWNED_ENTITIES {
                    return Err(format!("YMAP has more than {MAX_SPAWNED_ENTITIES} supported HD entities"));
                }
                placements.push(item);
            }
            None => report.skipped_invalid += 1,
        }
    }
    if placements.is_empty() {
        return Err("YMAP contains no supported plain HD object placements".into());
    }
    report.supported_entities = placements.len();
    Ok((report, compile(&placements)))
}

/// Install the converted YMAP as a disabled-by-default script mod.
pub fn install(name: &str, bytes: &[u8]) -> Result<YmapImportReport, String> {
    let (report, script) = inspect(bytes)?;
    install_script(name, &script)?;
    Ok(report)
}

/// Import ordinary exterior YMAP files contained in a readable RPF archive.
/// This is a conversion to script placements, not native archive mounting.
pub fn install_rpf_ymaps(name: &str, bytes: &[u8]) -> Result<RpfYmapImportReport, String> {
    use rpf_archive::{RpfArchive, RpfEncryption, build_directory_tree, list_all_files};

    const MAX_RPF_IMPORT_BYTES: usize = 128 * 1024 * 1024;
    const MAX_RPF_YMAPS: usize = 32;
    if bytes.is_empty() { return Err("RPF archive is empty".into()); }
    if bytes.len() > MAX_RPF_IMPORT_BYTES {
        return Err("RPF exceeds the 128 MiB Android import limit".into());
    }
    validate_mod_name(name)?;
    let archive = RpfArchive::parse(bytes, "import.rpf", None)
        .map_err(|e| format!("RPF parse failed: {e}"))?;
    if archive.entries.len() > crate::rpf_archive::MAX_RPF_ENTRIES {
        return Err("RPF contains too many entries".into());
    }
    if matches!(archive.encryption, RpfEncryption::Aes | RpfEncryption::Ng | RpfEncryption::Tfit) {
        return Err("Encrypted RPF cannot be converted without its game keys".into());
    }

    let tree = build_directory_tree(&archive.entries);
    let files = list_all_files(&tree);
    let mut report = RpfYmapImportReport::default();
    let mut script = String::new();
    for file in files.iter().filter(|file| file.path.to_ascii_lowercase().ends_with(".ymap")) {
        report.maps_found += 1;
        if report.maps_found > MAX_RPF_YMAPS {
            report.warnings.push(format!("Only the first {MAX_RPF_YMAPS} YMAP files are considered"));
            break;
        }
        let Some(entry) = archive.entries.get(file.entry_index) else {
            report.skipped_maps += 1;
            report.warnings.push(format!("{}: invalid RPF entry index", file.path));
            continue;
        };
        let map_bytes = match archive.extract_entry(bytes, entry, None) {
            Ok(data) if data.len() <= MAX_YMAP_BYTES => data,
            Ok(_) => {
                report.skipped_maps += 1;
                report.warnings.push(format!("{}: YMAP exceeds the 128 MiB limit", file.path));
                continue;
            }
            Err(error) => {
                report.skipped_maps += 1;
                report.warnings.push(format!("{}: extraction failed: {error}", file.path));
                continue;
            }
        };
        match inspect(&map_bytes) {
            Ok((map_report, map_script)) => {
                report.imported_maps += 1;
                report.entities += map_report.supported_entities;
                report.skipped_mlo += map_report.skipped_mlo;
                report.skipped_lod += map_report.skipped_lod;
                report.skipped_invalid += map_report.skipped_invalid;
                use std::fmt::Write as _;
                let _ = writeln!(script, "-- Converted from {}\ndo", file.path);
                script.push_str(&map_script);
                script.push_str("\nend\n");
            }
            Err(error) => {
                report.skipped_maps += 1;
                report.warnings.push(format!("{}: {error}", file.path));
            }
        }
    }
    if report.maps_found == 0 {
        return Err("RPF contains no YMAP files".into());
    }
    if report.imported_maps == 0 {
        return Err(format!("No supported YMAP placements found ({} maps skipped)", report.skipped_maps));
    }
    install_script(name, &script)?;
    Ok(report)
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RpfYmapImportReport {
    pub maps_found: usize,
    pub imported_maps: usize,
    pub skipped_maps: usize,
    pub entities: usize,
    pub skipped_mlo: usize,
    pub skipped_lod: usize,
    pub skipped_invalid: usize,
    pub warnings: Vec<String>,
}

fn validate_mod_name(name: &str) -> Result<(), String> {
    if name.is_empty() || name.len() > 40
        || !name.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
    {
        return Err("Map name must use lowercase letters, digits, - or _ (max 40)".into());
    }
    Ok(())
}

fn install_script(name: &str, script: &str) -> Result<(), String> {
    validate_mod_name(name)?;
    let dir = crate::dir().ok_or("mods are off")?;
    let target = dir.join(name);
    if target.exists() { return Err(format!("Mod {name} already exists")); }

    let mut random = [0u8; 6];
    getrandom::fill(&mut random).map_err(|e| format!("staging name: {e}"))?;
    let suffix = random.iter().map(|b| format!("{b:02x}")).collect::<String>();
    let staging = dir.join(format!(".stage-map-{name}-{suffix}"));
    std::fs::create_dir(&staging).map_err(|e| format!("create map staging directory: {e}"))?;
    let result = (|| {
        std::fs::write(staging.join("main.luau"), script)
            .map_err(|e| format!("write map script: {e}"))?;
        std::fs::write(staging.join("mod.json"), "{\n  \"autostart\": false\n}\n")
            .map_err(|e| format!("write map settings: {e}"))?;
        std::fs::rename(&staging, &target).map_err(|e| format!("install map mod: {e}"))?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inverse_yaw_quaternion_becomes_positive_heading() {
        let half = -90.0_f32.to_radians() / 2.0;
        let heading = heading_degrees([0.0, 0.0, half.sin(), half.cos()]).unwrap();
        assert!((heading - 90.0).abs() < 0.001);
    }

    #[test]
    fn rejects_empty_and_non_ymap_input() {
        assert!(inspect(&[]).unwrap_err().contains("empty"));
        assert!(inspect(b"not a YMAP").unwrap_err().contains("parse failed"));
    }

    #[test]
    fn compiled_ymap_uses_precomputed_numeric_hashes() {
        let script = compile(&[(0x1234ABCD, 1.0, 2.0, 3.0, 90.0)]);
        assert!(script.contains("model = 0x1234ABCD"));
        assert!(script.contains("streaming.load(model)"));
        assert!(script.contains("object.createObject(model, position, false, false, false)"));
        assert!(!script.contains("joaat("));
    }

    #[test]
    fn rejects_oversized_ymap_before_parsing() {
        assert!(inspect(&vec![0; MAX_YMAP_BYTES + 1]).unwrap_err().contains("MiB"));
    }
}
