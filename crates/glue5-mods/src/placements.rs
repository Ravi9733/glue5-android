//! Data-driven world placements used by content mods.
//!
//! This module deliberately only parses and validates placement manifests.
//! Spawning is performed later on the engine script thread through the game's
//! own registered natives; untrusted files never write directly into game memory.

use serde_json::Value;

pub const MAX_PLACEMENTS: usize = 512;
const MAX_COORDINATE: f32 = 20_000.0;

#[derive(Clone, Debug, PartialEq)]
pub struct Placement {
    pub model: String,
    pub position: [f32; 3],
    pub rotation: [f32; 3],
    /// Static Creator props are frozen; mission.dprop objects retain physics.
    pub frozen: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlacementSet {
    pub version: u32,
    pub placements: Vec<Placement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlacementError(pub String);

impl std::fmt::Display for PlacementError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for PlacementError {}

fn valid_model(model: &str) -> bool {
    !model.is_empty() && model.len() <= 64 &&
        model.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

fn valid_vector(v: [f32; 3], what: &str, index: usize) -> Result<[f32; 3], PlacementError> {
    if v.iter().any(|n| !n.is_finite() || n.abs() > MAX_COORDINATE) {
        return Err(PlacementError(format!("placement {index}: {what} is outside the allowed range")));
    }
    Ok(v)
}

/// Parse a version-1 placement manifest:
/// { "version": 1, "placements": [
///   { "model": "prop_name", "position": [x,y,z], "rotation": [rx,ry,rz] }
/// ] }
pub fn parse_manifest(text: &str) -> Result<PlacementSet, PlacementError> {
    let root: Value = serde_json::from_str(text)
        .map_err(|e| PlacementError(format!("invalid placement JSON: {e}")))?;
    let version = root.get("version").and_then(Value::as_u64)
        .ok_or_else(|| PlacementError("missing integer version".into()))?;
    if version != 1 {
        return Err(PlacementError(format!("unsupported placement version: {version}")));
    }
    let entries = root.get("placements").and_then(Value::as_array)
        .ok_or_else(|| PlacementError("placements must be an array".into()))?;
    if entries.len() > MAX_PLACEMENTS {
        return Err(PlacementError(format!("too many placements: {} (limit {MAX_PLACEMENTS})", entries.len())));
    }

    let mut placements = Vec::with_capacity(entries.len());
    for (index, entry) in entries.iter().enumerate() {
        let fail = |message: &str| PlacementError(format!("placement {index}: {message}"));
        let model = entry.get("model").and_then(Value::as_str)
            .ok_or_else(|| fail("model must be a string"))?;
        if !valid_model(model) {
            return Err(fail("model name must be 1-64 ASCII letters, digits, '_' or '-'"));
        }
        let vector = |key: &str, default: [f32; 3]| -> Result<[f32; 3], PlacementError> {
            let Some(value) = entry.get(key) else { return Ok(default) };
            let values = value.as_array().ok_or_else(|| fail(&format!("{key} must be a 3-number array")))?;
            if values.len() != 3 { return Err(fail(&format!("{key} must contain exactly 3 numbers"))); }
            let mut out = [0.0; 3];
            for (i, value) in values.iter().enumerate() {
                let n = value.as_f64().ok_or_else(|| fail(&format!("{key}[{i}] must be a number")))? as f32;
                if !n.is_finite() || n.abs() > MAX_COORDINATE {
                    return Err(fail(&format!("{key}[{i}] is outside the allowed range")));
                }
                out[i] = n;
            }
            Ok(out)
        };
        placements.push(Placement {
            model,
            position: vector("position", [0.0; 3])?,
            rotation: vector("rotation", [0.0; 3])?,
            frozen: entry.get("frozen").and_then(Value::as_bool).unwrap_or(true),
        });
    }
    Ok(PlacementSet { version: 1, placements })
}

fn child_text<'a, 'input>(node: roxmltree::Node<'a, 'input>, names: &[&str]) -> Option<&'a str> {
    node.children().find(|n| n.is_element() && names.iter().any(|name| n.tag_name().name().eq_ignore_ascii_case(name)))
        .and_then(|n| n.text()).map(str::trim).filter(|s| !s.is_empty())
}

fn number(text: &str, what: &str, index: usize) -> Result<f32, PlacementError> {
    let n: f32 = text.parse().map_err(|_| PlacementError(format!("placement {index}: invalid {what}")))?;
    if !n.is_finite() || n.abs() > MAX_COORDINATE {
        return Err(PlacementError(format!("placement {index}: {what} is outside the allowed range")));
    }
    Ok(n)
}

fn xml_vec(node: roxmltree::Node<'_, '_>, names: &[&str], index: usize) -> Result<Option<[f32; 3]>, PlacementError> {
    let Some(v) = node.children().find(|n| n.is_element() && names.iter().any(|name| n.tag_name().name().eq_ignore_ascii_case(name))) else {
        return Ok(None);
    };
    let component = |axis: &str| -> Result<f32, PlacementError> {
        let text = v.attribute(axis).or_else(|| child_text(v, &[axis]))
            .ok_or_else(|| PlacementError(format!("placement {index}: missing {axis} in {}", names[0])))?;
        number(text, axis, index)
    };
    Ok(Some([component("x")?, component("y")?, component("z")?]))
}

/// Parse Menyoo Spooner placements and CodeWalker/YMAP CEntityDef nodes.

pub fn parse_xml_placements(text: &str) -> Result<PlacementSet, PlacementError> {
    parse_xml_placements_with_hashes(text, |_| None)
}

/// Parse XML placements and resolve numeric model hashes against the active game build.
pub fn parse_xml_placements_with_hashes(
    text: &str,
    resolve_hash: impl Fn(u32) -> Option<String>,
) -> Result<PlacementSet, PlacementError> {
    const MAX_XML_BYTES: usize = 4 * 1024 * 1024;
    if text.len() > MAX_XML_BYTES {
        return Err(PlacementError("XML exceeds 4 MiB limit".into()));
    }
    let doc = roxmltree::Document::parse(text)
        .map_err(|e| PlacementError(format!("invalid placement XML: {e}")))?;
    let mut placements = Vec::new();

    for node in doc.descendants().filter(|n| {
        if !n.is_element() {
            return false;
        }
        let tag = n.tag_name().name();
        tag.eq_ignore_ascii_case("Placement")
            || tag.eq_ignore_ascii_case("CEntityDef")
            // CodeWalker/YMAP XML commonly represents entities as
            // <Item type="CEntityDef"> rather than a literal CEntityDef tag.
            || (tag.eq_ignore_ascii_case("Item")
                && n.attribute("type").is_some_and(|kind| kind.eq_ignore_ascii_case("CEntityDef")))
    }) {
        // Menyoo's entity type is 1=ped, 2=vehicle, 3=prop. This importer
        // builds static track geometry only; don't accidentally try to
        // create a ped or vehicle through CREATE_OBJECT.
        if let Some(raw_type) = child_text(node, &["Type"]) {
            let entity_type: i32 = raw_type.parse().map_err(|_| {
                PlacementError("placement has an invalid Menyoo Type value".into())
            })?;
            if entity_type != 3 {
                continue;
            }
        }
        let index = placements.len();
        let hash_name = child_text(node, &["HashName"])
            .filter(|name| valid_model(name) && !name.starts_with("0x") && !name.starts_with("0X"));
        let named_model = child_text(node, &["ModelName", "Model", "archetypeName"])
            .or_else(|| node.attribute("model"))
            .or(hash_name);
        let resolved_model = if let Some(model) = named_model {
            Some(model.to_owned())
        } else if let Some(raw_hash) = child_text(node, &["ModelHash", "modelHash"])
            .or_else(|| node.attribute("modelHash"))
            .or_else(|| node.attribute("hash"))
        {
            let raw = raw_hash.trim();
            let parsed = if let Some(hex) = raw.strip_prefix("0x").or_else(|| raw.strip_prefix("0X")) {
                u32::from_str_radix(hex, 16).ok()
            } else if let Ok(value) = raw.parse::<u32>() {
                Some(value)
            } else {
                raw.parse::<i32>().ok().map(|value| value as u32)
            }.ok_or_else(|| PlacementError(format!("placement {index}: invalid ModelHash")))?;
            Some(resolve_hash(parsed).ok_or_else(|| PlacementError(format!(
                "placement {index}: ModelHash {raw} is not registered in this game build"
            )))?)
        } else {
            None
        };
        let Some(model) = resolved_model else { continue };
        if !valid_model(&model) {
            return Err(PlacementError(format!("placement {index}: resolved model name is invalid")));
        }
        // Menyoo Spooner files store XYZ and Euler angles together in
        // <PositionRotation>; CodeWalker/YMAP uses a separate <position>.
        let position_rotation = node.children().find(|n| n.is_element()
            && n.tag_name().name().eq_ignore_ascii_case("PositionRotation"));
        let position = xml_vec(node, &["Position", "position", "PositionRotation"], index)?
            .ok_or_else(|| PlacementError(format!("placement {index}: missing Position/PositionRotation vector")))?;
        let rotation_node = node.children().find(|n| n.is_element() &&
            ["Rotation", "rotation"].iter().any(|name| n.tag_name().name().eq_ignore_ascii_case(name)));
        let rotation = if let Some(rn) = rotation_node {
            if let Some(w_text) = rn.attribute("w") {
                let x = rn.attribute("x").ok_or_else(|| PlacementError(format!("placement {index}: missing quaternion x")))?;
                let y = rn.attribute("y").ok_or_else(|| PlacementError(format!("placement {index}: missing quaternion y")))?;
                let z = rn.attribute("z").ok_or_else(|| PlacementError(format!("placement {index}: missing quaternion z")))?;
                let (x, y, z, w) = (
                    number(x, "rotation x", index)?, number(y, "rotation y", index)?,
                    number(z, "rotation z", index)?, number(w_text, "rotation w", index)?,
                );
                let roll = (2.0 * (w * x + y * z)).atan2(1.0 - 2.0 * (x * x + y * y));
                let pitch = (2.0 * (w * y - z * x)).clamp(-1.0, 1.0).asin();
                let yaw = (2.0 * (w * z + x * y)).atan2(1.0 - 2.0 * (y * y + z * z));
                [roll.to_degrees(), pitch.to_degrees(), yaw.to_degrees()]
            } else {
                xml_vec(node, &["Rotation", "rotation"], index)?.unwrap_or([0.0; 3])
            }
        } else {
            // Menyoo exports store Euler angles inside PositionRotation;
            // older/alternate exports may put them directly on Placement.
            let angle_source = position_rotation.unwrap_or(node);
            let pitch = child_text(angle_source, &["Pitch"]).map(|s| number(s, "Pitch", index)).transpose()?.unwrap_or(0.0);
            let roll = child_text(angle_source, &["Roll"]).map(|s| number(s, "Roll", index)).transpose()?.unwrap_or(0.0);
            let yaw = child_text(angle_source, &["Yaw", "Heading"]).map(|s| number(s, "Yaw", index)).transpose()?.unwrap_or(0.0);
            [pitch, roll, yaw]
        };
        placements.push(Placement {
            model: model.to_owned(),
            position: valid_vector(position, "position", index)?,
            rotation: valid_vector(rotation, "rotation", index)?,
            frozen: true,
        });
        if placements.len() > MAX_PLACEMENTS {
            return Err(PlacementError(format!("too many XML placements (limit {MAX_PLACEMENTS})")));
        }
    }

    if placements.is_empty() {
        return Err(PlacementError("no supported named-model placements found; numeric ModelHash-only Menyoo files need hash resolution".into()));
    }
    Ok(PlacementSet { version: 1, placements })
}

/// Extract Rockstar Creator static race props (mission.prop) into the same
/// validated placement representation used by XML maps.
pub fn parse_ugc_race_placements(
    text: &str,
    resolve_hash: impl Fn(u32) -> Option<String>,
) -> Result<PlacementSet, PlacementError> {
    const MAX_JSON_BYTES: usize = 8 * 1024 * 1024;
    if text.len() > MAX_JSON_BYTES {
        return Err(PlacementError("Rockstar Creator JSON exceeds 8 MiB limit".into()));
    }
    let root: Value = serde_json::from_str(text)
        .map_err(|e| PlacementError(format!("invalid Rockstar Creator JSON: {e}")))?;
    let mission = root.get("mission")
        .or_else(|| root.pointer("/content/data/mission"))
        .ok_or_else(|| PlacementError("Creator file has no mission section".into()))?;
    let prop = mission.get("prop")
        .ok_or_else(|| PlacementError("Creator file has no mission.prop section".into()))?;
    let models = prop.get("model").and_then(Value::as_array)
        .ok_or_else(|| PlacementError("mission.prop.model must be an array".into()))?;
    let positions = prop.get("loc").and_then(Value::as_array)
        .ok_or_else(|| PlacementError("mission.prop.loc must be an array".into()))?;
    let rotations = prop.get("vRot").and_then(Value::as_array)
        .ok_or_else(|| PlacementError("mission.prop.vRot must be an array".into()))?;
    let count = prop.get("no").and_then(Value::as_u64).unwrap_or(models.len() as u64) as usize;
    if count > MAX_PLACEMENTS {
        return Err(PlacementError(format!("Rockstar race has too many static props (limit {MAX_PLACEMENTS})")));
    }
    if models.len() < count || positions.len() < count || rotations.len() < count {
        return Err(PlacementError("mission.prop arrays are shorter than mission.prop.no".into()));
    }
    let vector = |value: &Value, what: &str, index: usize| -> Result<[f32; 3], PlacementError> {
        let fail = || PlacementError(format!("mission.prop.{what}[{index}] has an unsupported XYZ vector"));
        let axes = if let Some(a) = value.as_array() {
            if a.len() != 3 { return Err(fail()); }
            [
                a[0].as_f64().ok_or_else(fail)?,
                a[1].as_f64().ok_or_else(fail)?,
                a[2].as_f64().ok_or_else(fail)?,
            ]
        } else if let Some(o) = value.as_object() {
            let axis = |name: &str| o.iter()
                .find(|(key, _)| key.eq_ignore_ascii_case(name))
                .and_then(|(_, value)| value.as_f64()).ok_or_else(fail);
            [axis("x")?, axis("y")?, axis("z")?]
        } else {
            return Err(fail());
        };
        valid_vector([axes[0] as f32, axes[1] as f32, axes[2] as f32], what, index)
    };
    let mut placements = Vec::with_capacity(count);
    for index in 0..count {
        let raw_model = &models[index];
        let model = if let Some(name) = raw_model.as_str() {
            let raw = name.trim();
            let hash = raw.strip_prefix("0x").or_else(|| raw.strip_prefix("0X"))
                .and_then(|hex| u32::from_str_radix(hex, 16).ok())
                .or_else(|| raw.parse::<u32>().ok())
                .or_else(|| raw.parse::<i32>().ok().map(|n| n as u32));
            if let Some(hash) = hash {
                resolve_hash(hash).ok_or_else(|| PlacementError(format!(
                    "mission.prop.model[{index}] hash {hash:#010x} is not registered in this game build"
                )))?
            } else {
                raw.to_owned()
            }
        } else {
            let hash = raw_model.as_u64()
                .map(|n| n as u32)
                .or_else(|| raw_model.as_i64().map(|n| n as u32))
                .ok_or_else(|| PlacementError(format!("mission.prop.model[{index}] must be a model name or hash")))?;
            resolve_hash(hash).ok_or_else(|| PlacementError(format!(
                "mission.prop.model[{index}] hash {hash:#010x} is not registered in this game build"
            )))?
        };
        if !valid_model(&model) {
            return Err(PlacementError(format!("mission.prop.model[{index}] resolved to an invalid model name")));
        }
        placements.push(Placement {
            model,
            position: vector(&positions[index], "loc", index)?,
            rotation: vector(&rotations[index], "vRot", index)?,
            frozen: true,
        });
    }

    // Creator dynamic props (mission.dprop) are spawned with physics instead
    // of being frozen like the main track geometry.
    if let Some(dynamic) = mission.get("dprop") {
        let dynamic_models = dynamic.get("model").and_then(Value::as_array).cloned().unwrap_or_default();
        let dynamic_count = dynamic.get("no").and_then(Value::as_u64).unwrap_or(dynamic_models.len() as u64) as usize;
        if placements.len().saturating_add(dynamic_count) > MAX_PLACEMENTS {
            return Err(PlacementError(format!("Rockstar race has too many props (limit {MAX_PLACEMENTS})")));
        }
        let dynamic_positions = dynamic.get("loc").and_then(Value::as_array)
            .ok_or_else(|| PlacementError("mission.dprop.loc must be an array".into()))?;
        let dynamic_rotations = dynamic.get("vRot").and_then(Value::as_array)
            .ok_or_else(|| PlacementError("mission.dprop.vRot must be an array".into()))?;
        if dynamic_models.len() < dynamic_count || dynamic_positions.len() < dynamic_count
            || dynamic_rotations.len() < dynamic_count {
            return Err(PlacementError("mission.dprop arrays are shorter than mission.dprop.no".into()));
        }
        for index in 0..dynamic_count {
            let raw_model = &dynamic_models[index];
            let model = if let Some(name) = raw_model.as_str() {
                let raw = name.trim();
                let hash = raw.strip_prefix("0x").or_else(|| raw.strip_prefix("0X"))
                    .and_then(|hex| u32::from_str_radix(hex, 16).ok())
                    .or_else(|| raw.parse::<u32>().ok())
                    .or_else(|| raw.parse::<i32>().ok().map(|n| n as u32));
                if let Some(hash) = hash {
                    resolve_hash(hash).ok_or_else(|| PlacementError(format!(
                        "mission.dprop.model[{index}] hash {hash:#010x} is not registered in this game build"
                    )))?
                } else { raw.to_owned() }
            } else {
                let hash = raw_model.as_u64().map(|n| n as u32)
                    .or_else(|| raw_model.as_i64().map(|n| n as u32))
                    .ok_or_else(|| PlacementError(format!("mission.dprop.model[{index}] must be a model name or hash")))?;
                resolve_hash(hash).ok_or_else(|| PlacementError(format!(
                    "mission.dprop.model[{index}] hash {hash:#010x} is not registered in this game build"
                )))?
            };
            if !valid_model(&model) {
                return Err(PlacementError(format!("mission.dprop.model[{index}] resolved to an invalid model name")));
            }
            placements.push(Placement {
                model,
                position: vector(&dynamic_positions[index], "dprop.loc", index)?,
                rotation: vector(&dynamic_rotations[index], "dprop.vRot", index)?,
                frozen: false,
            });
        }
    }
    Ok(PlacementSet { version: 1, placements })
}

/// Parse the public Rockstar Creator job JSON and extract its race checkpoint
/// route. Supports the Creator's common vector-array representations while
/// rejecting unknown encodings rather than guessing coordinates.
pub fn parse_ugc_race_checkpoints(text: &str) -> Result<Vec<[f32; 3]>, PlacementError> {
    const MAX_JSON_BYTES: usize = 8 * 1024 * 1024;
    const MAX_CHECKPOINTS: usize = 128;
    if text.len() > MAX_JSON_BYTES {
        return Err(PlacementError("Rockstar Creator JSON exceeds 8 MiB limit".into()));
    }
    let root: Value = serde_json::from_str(text)
        .map_err(|e| PlacementError(format!("invalid Rockstar Creator JSON: {e}")))?;
    let mission = root.get("mission")
        .or_else(|| root.pointer("/content/data/mission"))
        .ok_or_else(|| PlacementError("Creator file has no mission section".into()))?;
    let race = mission.get("race")
        .ok_or_else(|| PlacementError("Creator file has no mission.race section".into()))?;
    let raw = race.get("chl")
        .ok_or_else(|| PlacementError("mission.race has no checkpoint coordinate field (chl)".into()))?;

    fn coord(value: &Value, index: usize) -> Result<[f32; 3], PlacementError> {
        let fail = || PlacementError(format!("mission.race.chl checkpoint {index} has an unsupported coordinate shape"));
        let values: [f64; 3] = if let Some(a) = value.as_array() {
            if a.len() != 3 { return Err(fail()); }
            [
                a[0].as_f64().ok_or_else(fail)?,
                a[1].as_f64().ok_or_else(fail)?,
                a[2].as_f64().ok_or_else(fail)?,
            ]
        } else if let Some(o) = value.as_object() {
            let axis = |name: &str| o.iter()
                .find(|(key, _)| key.eq_ignore_ascii_case(name))
                .and_then(|(_, value)| value.as_f64()).ok_or_else(fail);
            [axis("x")?, axis("y")?, axis("z")?]
        } else if let Some(s) = value.as_str() {
            let parts = s.split(|ch: char| ch == ',' || ch.is_ascii_whitespace())
                .filter(|part| !part.is_empty()).collect::<Vec<_>>();
            if parts.len() != 3 { return Err(fail()); }
            [
                parts[0].parse::<f64>().map_err(|_| fail())?,
                parts[1].parse::<f64>().map_err(|_| fail())?,
                parts[2].parse::<f64>().map_err(|_| fail())?,
            ]
        } else {
            return Err(fail());
        };
        let point = [values[0] as f32, values[1] as f32, values[2] as f32];
        valid_vector(point, "Rockstar checkpoint position", index)
    }

    let mut points = Vec::new();
    if let Some(entries) = raw.as_array() {
        if entries.iter().all(Value::is_number) {
            if entries.len() % 3 != 0 {
                return Err(PlacementError("mission.race.chl flat coordinate list must contain X/Y/Z triples".into()));
            }
            for (i, triple) in entries.chunks_exact(3).enumerate() {
                points.push(coord(&Value::Array(triple.to_vec()), i)?);
            }
        } else {
            for (i, entry) in entries.iter().enumerate() {
                points.push(coord(entry, i)?);
            }
        }
    } else if let Some(encoded) = raw.as_str() {
        let decoded: Value = serde_json::from_str(encoded)
            .map_err(|_| PlacementError("mission.race.chl string is not a JSON coordinate array".into()))?;
        let Some(entries) = decoded.as_array() else {
            return Err(PlacementError("mission.race.chl string must decode to an array".into()));
        };
        for (i, entry) in entries.iter().enumerate() {
            points.push(coord(entry, i)?);
        }
    } else {
        return Err(PlacementError("mission.race.chl must be a coordinate array".into()));
    }
    if points.len() < 2 {
        return Err(PlacementError("Rockstar race needs at least two checkpoints".into()));
    }
    if points.len() > MAX_CHECKPOINTS {
        return Err(PlacementError(format!("Rockstar race has too many checkpoints (limit {MAX_CHECKPOINTS})")));
    }
    if let Some(expected) = race.get("chp").and_then(Value::as_u64) {
        if expected as usize != points.len() {
            return Err(PlacementError(format!(
                "mission.race.chp says {expected} checkpoints but chl contains {}", points.len()
            )));
        }
    }
    Ok(points)
}

/// Parse an offline race route from <Checkpoint><Position><X/Y/Z>...</Position></Checkpoint> nodes.
pub fn parse_xml_checkpoints(text: &str) -> Result<Vec<[f32; 3]>, PlacementError> {
    const MAX_XML_BYTES: usize = 4 * 1024 * 1024;
    const MAX_CHECKPOINTS: usize = 128;
    if text.len() > MAX_XML_BYTES {
        return Err(PlacementError("race route XML exceeds 4 MiB limit".into()));
    }
    let doc = roxmltree::Document::parse(text)
        .map_err(|e| PlacementError(format!("invalid race route XML: {e}")))?;
    let mut points = Vec::new();
    for node in doc.descendants().filter(|n| n.is_element()
        && (n.tag_name().name().eq_ignore_ascii_case("Checkpoint")
            || n.tag_name().name().eq_ignore_ascii_case("RaceCheckpoint"))) {
        let index = points.len();
        let position = xml_vec(node, &["Position", "position"], index)?
            .ok_or_else(|| PlacementError(format!("checkpoint {index}: missing Position vector")))?;
        points.push(valid_vector(position, "checkpoint position", index)?);
        if points.len() > MAX_CHECKPOINTS {
            return Err(PlacementError(format!("too many race checkpoints (limit {MAX_CHECKPOINTS})")));
        }
    }
    if points.len() < 2 {
        return Err(PlacementError("race route needs at least 2 <Checkpoint> entries".into()));
    }
    Ok(points)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_runtime_prop_placements() {
        let set = parse_manifest(r#"{"version":1,"placements":[{"model":"prop_roadcone02a","position":[1,2,3],"rotation":[0,0,90]}]}"#).unwrap();
        assert_eq!(set.placements.len(), 1);
        assert_eq!(set.placements[0].model, "prop_roadcone02a");
        assert_eq!(set.placements[0].position, [1.0, 2.0, 3.0]);
    }

    #[test]
    fn parses_named_menyoo_style_xml() {
        let xml = r#"<SpoonerPlacements><Placement><ModelName>stt_prop_ramp_adj_flip_m</ModelName><Position><X>1</X><Y>2</Y><Z>3</Z></Position><Rotation><X>0</X><Y>0</Y><Z>90</Z></Rotation></Placement></SpoonerPlacements>"#;
        let set = parse_xml_placements(xml).unwrap();
        assert_eq!(set.placements.len(), 1);
        assert_eq!(set.placements[0].model, "stt_prop_ramp_adj_flip_m");
        assert_eq!(set.placements[0].position, [1.0, 2.0, 3.0]);
    }

    #[test]
    fn uses_valid_menyoo_hash_name_when_available() {
        let xml = r#"<SpoonerPlacements><Placement><ModelHash>0xDEADBEEF</ModelHash><HashName>stt_prop_ramp_adj_flip_m</HashName><Type>3</Type><PositionRotation><X>1</X><Y>2</Y><Z>3</Z></PositionRotation></Placement></SpoonerPlacements>"#;
        let set = parse_xml_placements_with_hashes(xml, |_| None).unwrap();
        assert_eq!(set.placements[0].model, "stt_prop_ramp_adj_flip_m");
    }

    #[test]
    fn skips_menyoo_peds_and_vehicles_but_keeps_props() {
        let xml = r#"<SpoonerPlacements>
            <Placement><ModelHash>0x11111111</ModelHash><Type>1</Type><PositionRotation><X>0</X><Y>0</Y><Z>0</Z></PositionRotation></Placement>
            <Placement><ModelHash>0x22222222</ModelHash><Type>2</Type><PositionRotation><X>1</X><Y>0</Y><Z>0</Z></PositionRotation></Placement>
            <Placement><ModelHash>0x33333333</ModelHash><Type>3</Type><PositionRotation><X>2</X><Y>0</Y><Z>0</Z></PositionRotation></Placement>
        </SpoonerPlacements>"#;
        let set = parse_xml_placements_with_hashes(xml, |hash| {
            Some(format!("prop_{hash:08x}"))
        }).unwrap();
        assert_eq!(set.placements.len(), 1);
        assert_eq!(set.placements[0].position, [2.0, 0.0, 0.0]);
    }

    #[test]
    fn parses_real_menyoo_position_rotation_layout() {
        let xml = r#"<SpoonerPlacements><Placement><ModelHash>0x203011a1</ModelHash><PositionRotation><X>-1143.47192</X><Y>-1700.6554</Y><Z>3.6227603</Z><Pitch>-66.8104858</Pitch><Roll>-116.963776</Roll><Yaw>74.5379562</Yaw></PositionRotation></Placement></SpoonerPlacements>"#;
        let set = parse_xml_placements_with_hashes(xml, |hash| {
            (hash == 0x203011a1).then(|| "prop_roadcone02a".to_owned())
        }).unwrap();
        assert_eq!(set.placements.len(), 1);
        assert_eq!(set.placements[0].model, "prop_roadcone02a");
        assert_eq!(set.placements[0].position, [-1143.47192, -1700.6554, 3.6227603]);
        assert!((set.placements[0].rotation[0] - -66.8104858).abs() < 0.001);
        assert!((set.placements[0].rotation[1] - -116.963776).abs() < 0.001);
        assert!((set.placements[0].rotation[2] - 74.5379562).abs() < 0.001);
    }

    #[test]
    fn parses_ymap_entity_def_xml() {
        let xml = r#"<CMapData><entities><Item type="CEntityDef"><archetypeName>prop_roadcone02a</archetypeName><position x="10" y="20" z="30"/><rotation x="0" y="0" z="0" w="1"/></Item></entities></CMapData>"#;
        let set = parse_xml_placements(xml).unwrap();
        assert_eq!(set.placements[0].model, "prop_roadcone02a");
        assert_eq!(set.placements[0].position, [10.0, 20.0, 30.0]);
    }

    #[test]
    fn parses_codewalker_item_typed_entity_xml() {
        let xml = r#"<CMapData><entities><Item type="CEntityDef"><archetypeName>stt_prop_ramp_adj_flip_m</archetypeName><flags value="0"/><position x="12.5" y="-8" z="30"/><rotation x="0" y="0" z="0" w="1"/><scaleXY value="1"/><scaleZ value="1"/></Item></entities></CMapData>"#;
        let set = parse_xml_placements(xml).unwrap();
        assert_eq!(set.placements.len(), 1);
        assert_eq!(set.placements[0].model, "stt_prop_ramp_adj_flip_m");
        assert_eq!(set.placements[0].position, [12.5, -8.0, 30.0]);
    }

    #[test]
    fn resolves_numeric_hash_only_menyoo_xml() {
        let xml = r#"<SpoonerPlacements><Placement><ModelHash>123456</ModelHash><Position><X>1</X><Y>2</Y><Z>3</Z></Position></Placement></SpoonerPlacements>"#;
        let set = parse_xml_placements_with_hashes(xml, |hash| {
            (hash == 123456).then(|| "stt_prop_ramp_adj_flip_m".to_owned())
        }).unwrap();
        assert_eq!(set.placements[0].model, "stt_prop_ramp_adj_flip_m");
        assert_eq!(set.placements[0].position, [1.0, 2.0, 3.0]);
    }

    #[test]
    fn rejects_unregistered_numeric_model_hash() {
        let xml = r#"<SpoonerPlacements><Placement><ModelHash>0xDEADBEEF</ModelHash><Position><X>0</X><Y>0</Y><Z>0</Z></Position></Placement></SpoonerPlacements>"#;
        assert!(parse_xml_placements_with_hashes(xml, |_| None)
            .unwrap_err().0.contains("not registered in this game build"));
    }

    #[test]
    fn rejects_invalid_models_and_unbounded_coordinates() {
        assert!(parse_manifest(r#"{"version":1,"placements":[{"model":"../bad","position":[0,0,0]}]}"#).is_err());
        assert!(parse_manifest(r#"{"version":1,"placements":[{"model":"prop","position":[99999,0,0]}]}"#).is_err());
    }

    #[test]
    fn parses_rockstar_ugc_static_stunt_props() {
        let json = r#"{
            "mission": {
                "prop": {
                    "no": 2,
                    "model": [123, -456],
                    "loc": [{"x": 10, "y": 20, "z": 30}, {"x": -1, "y": -2, "z": -3}],
                    "vRot": [{"x": 0, "y": 0, "z": 90}, {"x": 10, "y": 20, "z": 30}],
                    "prpclr": [0, 1],
                    "head": [0, 0]
                }
            }
        }"#;
        let set = parse_ugc_race_placements(json, |hash| Some(format!("stt_prop_{hash}"))).unwrap();
        assert_eq!(set.placements.len(), 2);
        assert_eq!(set.placements[0].model, "stt_prop_123");
        assert_eq!(set.placements[0].position, [10.0, 20.0, 30.0]);
        assert_eq!(set.placements[0].rotation, [0.0, 0.0, 90.0]);
    }

    #[test]
    fn parses_ugc_dynamic_props_without_freezing_them() {
        let json = r#"{
            "mission": {
                "prop": {"no":1,"model":[123],"loc":[{"x":1,"y":2,"z":3}],"vRot":[{"x":0,"y":0,"z":0}]},
                "dprop": {"no":1,"model":[456],"loc":[{"x":4,"y":5,"z":6}],"vRot":[{"x":0,"y":0,"z":90}]}
            }
        }"#;
        let set = parse_ugc_race_placements(json, |hash| Some(format!("prop_{hash}"))).unwrap();
        assert_eq!(set.placements.len(), 2);
        assert!(set.placements[0].frozen);
        assert!(!set.placements[1].frozen);
        assert_eq!(set.placements[1].position, [4.0, 5.0, 6.0]);
    }

    #[test]
    fn resolves_string_encoded_ugc_prop_hashes() {
        let json = r#"{"mission":{"prop":{"no":1,"model":["0x7B"],"loc":[{"x":1,"y":2,"z":3}],"vRot":[{"x":0,"y":0,"z":90}]}}}"#;
        let set = parse_ugc_race_placements(json, |hash| {
            (hash == 123).then(|| "stt_prop_ramp_adj_flip_m".to_owned())
        }).unwrap();
        assert_eq!(set.placements[0].model, "stt_prop_ramp_adj_flip_m");
    }

    #[test]
    fn parses_complete_ugc_route_and_track_fixture() {
        let json = r#"{
            "content":{"data":{"mission":{
                "gen":{"type":2},
                "race":{"chp":2,"chl":[{"x":10,"y":20,"z":30},{"x":40,"y":50,"z":60}],"chs":[2,2]},
                "prop":{"no":1,"model":[123],"loc":[{"x":11,"y":22,"z":33}],"vRot":[{"x":0,"y":0,"z":45}]}
            }}}
        }"#;
        let route = parse_ugc_race_checkpoints(json).unwrap();
        let props = parse_ugc_race_placements(json, |_| Some("stt_prop_ramp_adj_flip_m".into())).unwrap();
        assert_eq!(route.len(), 2);
        assert_eq!(props.placements.len(), 1);
        assert_eq!(props.placements[0].rotation, [0.0, 0.0, 45.0]);
    }

    #[test]
    fn rejects_ugc_prop_array_count_mismatch() {
        let json = r#"{"mission":{"prop":{"no":2,"model":[1],"loc":[{"x":0,"y":0,"z":0}],"vRot":[{"x":0,"y":0,"z":0}]}}}"#;
        assert!(parse_ugc_race_placements(json, |_| Some("prop_test".into()))
            .unwrap_err().0.contains("shorter than mission.prop.no"));
    }

    #[test]
    fn parses_rockstar_ugc_race_checkpoint_vectors() {
        let json = r#"{"mission":{"gen":{"type":2},"race":{"chp":3,"lap":1,"chl":[[1,2,3],[4,5,6],[7,8,9]]}}}"#;
        assert_eq!(parse_ugc_race_checkpoints(json).unwrap(), vec![[1.0,2.0,3.0],[4.0,5.0,6.0],[7.0,8.0,9.0]]);
    }

    #[test]
    fn parses_rockstar_ugc_flat_and_object_coordinates() {
        let flat = r#"{"mission":{"race":{"chl":[1,2,3,4,5,6]}}}"#;
        assert_eq!(parse_ugc_race_checkpoints(flat).unwrap(), vec![[1.0,2.0,3.0],[4.0,5.0,6.0]]);
        let objects = r#"{"content":{"data":{"mission":{"race":{"chl":[{"x":1,"y":2,"z":3},{"X":4,"Y":5,"Z":6}]}}}}}"#;
        assert_eq!(parse_ugc_race_checkpoints(objects).unwrap(), vec![[1.0,2.0,3.0],[4.0,5.0,6.0]]);
    }

    #[test]
    fn rejects_mismatched_ugc_checkpoint_count() {
        let json = r#"{"mission":{"race":{"chp":3,"chl":[[1,2,3],[4,5,6]]}}}"#;
        assert!(parse_ugc_race_checkpoints(json).unwrap_err().0.contains("chp says 3"));
    }

    #[test]
    fn parses_offline_race_checkpoint_route() {
        let xml = r#"<Race><Checkpoint><Position><X>1</X><Y>2</Y><Z>3</Z></Position></Checkpoint><Checkpoint><Position><X>4</X><Y>5</Y><Z>6</Z></Position></Checkpoint></Race>"#;
        assert_eq!(parse_xml_checkpoints(xml).unwrap(), vec![[1.0,2.0,3.0],[4.0,5.0,6.0]]);
    }

    #[test]
    fn rejects_unsupported_versions() {
        assert!(parse_manifest(r#"{"version":2,"placements":[]}"#).is_err());
    }
}
