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
        if model.is_empty() || model.len() > 64 ||
            !model.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-') {
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
            model: model.to_owned(),
            position: vector("position", [0.0; 3])?,
            rotation: vector("rotation", [0.0; 3])?,
        });
    }
    Ok(PlacementSet { version: 1, placements })
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
    fn rejects_invalid_models_and_unbounded_coordinates() {
        assert!(parse_manifest(r#"{"version":1,"placements":[{"model":"../bad","position":[0,0,0]}]}"#).is_err());
        assert!(parse_manifest(r#"{"version":1,"placements":[{"model":"prop","position":[99999,0,0]}]}"#).is_err());
    }

    #[test]
    fn rejects_unsupported_versions() {
        assert!(parse_manifest(r#"{"version":2,"placements":[]}"#).is_err());
    }
}
