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
