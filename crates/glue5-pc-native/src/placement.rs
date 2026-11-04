//! Versioned, bounded JSON placement-mod parsing for the experimental PC-content layer.
//!
//! This defines Glue5's own interchange schema; it is not a claim of compatibility
//! with every third-party GTA V placement JSON format.
use serde_json::Value;
use std::path::{Component, Path};

const SCHEMA_VERSION: u64 = 1;
const MAX_DOCUMENT_BYTES: usize = 4 * 1024 * 1024;
const MAX_PLACEMENTS: usize = 20_000;
const MAX_ASSET_BYTES: usize = 240;
const MAX_ABS_TRANSFORM: f64 = 1_000_000.0;
const MAX_SCALE: f64 = 10_000.0;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlacementError(pub String);
impl std::fmt::Display for PlacementError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str(&self.0) }
}
impl std::error::Error for PlacementError {}

#[derive(Clone, Debug, PartialEq)]
pub struct JsonPlacement {
    pub asset: String,
    pub position: [f64; 3],
    /// Euler rotation in degrees, in X/Y/Z order.
    pub rotation: [f64; 3],
    pub scale: [f64; 3],
}
#[derive(Clone, Debug, PartialEq)]
pub struct JsonPlacementDocument {
    pub version: u64,
    pub placements: Vec<JsonPlacement>,
}

/// Parse Glue5's version-1 schema: root {version, placements}; each placement
/// has asset, position [x,y,z], rotation [x,y,z], and scale [x,y,z].
/// This is an explicit Glue5 interchange format, not a universal GTA JSON parser.
pub fn parse_json_placements(bytes: &[u8]) -> Result<JsonPlacementDocument, PlacementError> {
    if bytes.len() > MAX_DOCUMENT_BYTES {
        return Err(PlacementError(format!("placement JSON exceeds {} byte limit", MAX_DOCUMENT_BYTES)));
    }
    let root: Value = serde_json::from_slice(bytes)
        .map_err(|e| PlacementError(format!("invalid placement JSON: {e}")))?;
    let object = root.as_object().ok_or_else(|| PlacementError("document root must be an object".into()))?;
    let version = object.get("version").and_then(Value::as_u64)
        .ok_or_else(|| PlacementError("document requires an unsigned version".into()))?;
    if version != SCHEMA_VERSION {
        return Err(PlacementError(format!("unsupported placement schema version {version}")));
    }
    let values = object.get("placements").and_then(Value::as_array)
        .ok_or_else(|| PlacementError("document requires a placements array".into()))?;
    if values.len() > MAX_PLACEMENTS {
        return Err(PlacementError(format!("placement count {} exceeds limit {}", values.len(), MAX_PLACEMENTS)));
    }
    let mut placements = Vec::with_capacity(values.len());
    for (index, value) in values.iter().enumerate() {
        let item = value.as_object().ok_or_else(|| PlacementError(format!("placement {index} must be an object")))?;
        let asset = item.get("asset").and_then(Value::as_str)
            .ok_or_else(|| PlacementError(format!("placement {index} requires an asset string")))?;
        validate_asset_path(asset).map_err(|e| PlacementError(format!("placement {index}: {}", e.0)))?;
        let position = parse_vector(item.get("position"), "position", index)?;
        let rotation = parse_vector(item.get("rotation"), "rotation", index)?;
        let scale = parse_vector(item.get("scale"), "scale", index)?;
        if scale.iter().any(|v| *v <= 0.0 || *v > MAX_SCALE) {
            return Err(PlacementError(format!("placement {index} scale must be > 0 and <= {MAX_SCALE}")));
        }
        placements.push(JsonPlacement { asset: asset.to_owned(), position, rotation, scale });
    }
    Ok(JsonPlacementDocument { version, placements })
}

fn parse_vector(value: Option<&Value>, field: &str, index: usize) -> Result<[f64; 3], PlacementError> {
    let values = value.and_then(Value::as_array)
        .ok_or_else(|| PlacementError(format!("placement {index} requires {field} array")))?;
    if values.len() != 3 {
        return Err(PlacementError(format!("placement {index} {field} must have exactly three numbers")));
    }
    let mut result = [0.0; 3];
    for (axis, value) in values.iter().enumerate() {
        let number = value.as_f64().ok_or_else(|| PlacementError(format!("placement {index} {field}[{axis}] must be a number")))?;
        if !number.is_finite() || number.abs() > MAX_ABS_TRANSFORM {
            return Err(PlacementError(format!("placement {index} {field}[{axis}] is non-finite or out of range")));
        }
        result[axis] = number;
    }
    Ok(result)
}

fn validate_asset_path(asset: &str) -> Result<(), PlacementError> {
    if asset.is_empty() || asset.len() > MAX_ASSET_BYTES {
        return Err(PlacementError("asset path is empty or too long".into()));
    }
    if asset.contains('\\') || asset.contains(':') || asset.starts_with('/') {
        return Err(PlacementError("asset path must be relative and slash-separated".into()));
    }
    if Path::new(asset).components().any(|part| !matches!(part, Component::Normal(_))) {
        return Err(PlacementError("asset path contains traversal or non-normal component".into()));
    }
    if asset.split('/').any(|part| part.is_empty()) {
        return Err(PlacementError("asset path contains empty component".into()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    const VALID: &[u8] = include_bytes!("../examples/maze_bank_ramp.placements.json");

    #[test]
    fn parses_versioned_placement_and_transform() {
        let doc = parse_json_placements(VALID).unwrap();
        assert_eq!(doc.version, 1);
        assert_eq!(doc.placements.len(), 1);
        assert_eq!(doc.placements[0].asset, "props/maze_bank_ramp.ydr");
        assert_eq!(doc.placements[0].position, [1.0, 2.0, 3.0]);
        assert_eq!(doc.placements[0].rotation, [0.0, 0.0, 90.0]);
        assert_eq!(doc.placements[0].scale, [1.0, 1.0, 1.0]);
    }

    #[test]
    fn rejects_unknown_versions_and_missing_fields() {
        assert!(parse_json_placements(br#"{"version":2,"placements":[]}"#).unwrap_err().0.contains("unsupported"));
        assert!(parse_json_placements(br#"{"version":1}"#).is_err());
    }

    #[test]
    fn rejects_traversal_and_absolute_asset_paths() {
        for asset in ["../secret.ydr", "props/../../secret.ydr", "/absolute.ydr", "C:/secret.ydr", "props\\secret.ydr", "props//item.ydr"] {
            let json = format!(r#"{{"version":1,"placements":[{{"asset":"{asset}","position":[0,0,0],"rotation":[0,0,0],"scale":[1,1,1]}}]}}"#);
            assert!(parse_json_placements(json.as_bytes()).is_err(), "asset={asset}");
        }
    }

    #[test]
    fn rejects_bad_vectors_non_finite_values_and_invalid_scale() {
        assert!(parse_json_placements(br#"{"version":1,"placements":[{"asset":"a.ydr","position":[0,1],"rotation":[0,0,0],"scale":[1,1,1]}]}"#).is_err());
        assert!(parse_json_placements(br#"{"version":1,"placements":[{"asset":"a.ydr","position":[1e999,0,0],"rotation":[0,0,0],"scale":[1,1,1]}]}"#).is_err());
        assert!(parse_json_placements(br#"{"version":1,"placements":[{"asset":"a.ydr","position":[0,0,0],"rotation":[0,0,0],"scale":[1,0,1]}]}"#).unwrap_err().0.contains("scale"));
    }

    #[test]
    fn rejects_oversized_documents_and_placement_counts() {
        assert!(parse_json_placements(&vec![b' '; MAX_DOCUMENT_BYTES + 1]).is_err());
        let mut many = String::from(r#"{"version":1,"placements":["#);
        for index in 0..=MAX_PLACEMENTS {
            if index != 0 { many.push(','); }
            many.push_str(r#"{"asset":"a.ydr","position":[0,0,0],"rotation":[0,0,0],"scale":[1,1,1]}"#);
        }
        many.push_str("]}");
        assert!(parse_json_placements(many.as_bytes()).unwrap_err().0.contains("placement count"));
    }
}
