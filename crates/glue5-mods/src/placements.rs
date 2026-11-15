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

/// Parse named-model XML placements from common Menyoo-style Placement nodes
/// and CodeWalker/YMAP CEntityDef nodes. Numeric-only ModelHash entries are
/// intentionally rejected: they need a game-build-specific hash-to-name lookup.
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
        let index = placements.len();
        let named_model = child_text(node, &["ModelName", "Model", "archetypeName"])
            .or_else(|| node.attribute("model"));
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
        let position = xml_vec(node, &["Position", "position"], index)?
            .ok_or_else(|| PlacementError(format!("placement {index}: missing Position vector")))?;
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
            // Menyoo exports often store Euler angles as Pitch/Roll/Yaw.
            let pitch = child_text(node, &["Pitch"]).map(|s| number(s, "Pitch", index)).transpose()?.unwrap_or(0.0);
            let roll = child_text(node, &["Roll"]).map(|s| number(s, "Roll", index)).transpose()?.unwrap_or(0.0);
            let yaw = child_text(node, &["Yaw", "Heading"]).map(|s| number(s, "Yaw", index)).transpose()?.unwrap_or(0.0);
            [pitch, roll, yaw]
        };
        placements.push(Placement {
            model: model.to_owned(),
            position: valid_vector(position, "position", index)?,
            rotation: valid_vector(rotation, "rotation", index)?,
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
    fn parses_offline_race_checkpoint_route() {
        let xml = r#"<Race><Checkpoint><Position><X>1</X><Y>2</Y><Z>3</Z></Position></Checkpoint><Checkpoint><Position><X>4</X><Y>5</Y><Z>6</Z></Position></Checkpoint></Race>"#;
        assert_eq!(parse_xml_checkpoints(xml).unwrap(), vec![[1.0,2.0,3.0],[4.0,5.0,6.0]]);
    }

    #[test]
    fn rejects_unsupported_versions() {
        assert!(parse_manifest(r#"{"version":2,"placements":[]}"#).is_err());
    }
}
