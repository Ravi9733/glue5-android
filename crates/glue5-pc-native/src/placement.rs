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

/// Adapter implemented by a future world/entity backend. The parser remains
/// independent of GTA engine APIs; this trait is the controlled hand-off point.
pub trait PlacementSink {
    /// Resolve the asset identifier and create one world placement.
    fn spawn_placement(&mut self, placement: &JsonPlacement) -> Result<(), String>;
}

/// Error from applying a parsed document. Earlier placements may already have
/// been created; applied reports how many succeeded before the error.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlacementApplyError {
    pub failed_index: usize,
    pub applied: usize,
    pub asset: String,
    pub reason: String,
}

/// Pass validated placements to an engine adapter in document order.
/// This does not itself load GTA assets or create game entities; the runtime
/// must provide a PlacementSink backed by its asset and world APIs.
pub fn apply_json_placements<S: PlacementSink>(
    document: &JsonPlacementDocument,
    sink: &mut S,
) -> Result<usize, PlacementApplyError> {
    let mut applied = 0;
    for (index, placement) in document.placements.iter().enumerate() {
        if let Err(reason) = sink.spawn_placement(placement) {
            return Err(PlacementApplyError {
                failed_index: index,
                applied,
                asset: placement.asset.clone(),
                reason,
            });
        }
        applied += 1;
    }
    Ok(applied)
}


/// Error returned when a placement references an asset absent from the mount index.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlacementAssetError {
    pub failed_index: usize,
    pub asset: String,
}

/// Resolve every placement asset before a world adapter is allowed to spawn
/// anything. This preflight prevents avoidable partial world updates caused by
/// a missing model later in the document.
pub fn validate_json_placement_assets(
    document: &JsonPlacementDocument,
    mounts: &crate::AssetMountIndex,
) -> Result<usize, PlacementAssetError> {
    for (index, placement) in document.placements.iter().enumerate() {
        match mounts.resolve(&placement.asset) {
            Ok(Some(_)) => {}
            Ok(None) | Err(_) => return Err(PlacementAssetError {
                failed_index: index,
                asset: placement.asset.clone(),
            }),
        }
    }
    Ok(document.placements.len())
}

/// Engine-facing adapter that receives both the validated transform and its
/// mounted source descriptor. A native world backend can use the descriptor to
/// read/decode the referenced YDR/YDD/YTD/RSC7 data before creating entities.
pub trait ResolvedPlacementSink {
    fn spawn_resolved_placement(
        &mut self,
        placement: &JsonPlacement,
        asset: &crate::MountedAsset,
    ) -> Result<(), String>;
}

/// Resolve every placement first, then hand the complete ordered set to a
/// renderer/world adapter. Missing assets therefore fail before the first
/// entity is spawned. This is an integration boundary, not a GTA model decoder.
pub fn apply_resolved_json_placements<S: ResolvedPlacementSink>(
    document: &JsonPlacementDocument,
    mounts: &crate::AssetMountIndex,
    sink: &mut S,
) -> Result<usize, PlacementApplyError> {
    let mut resolved = Vec::with_capacity(document.placements.len());
    for (index, placement) in document.placements.iter().enumerate() {
        let asset = mounts.resolve(&placement.asset)
            .map_err(|error| PlacementApplyError {
                failed_index: index,
                applied: 0,
                asset: placement.asset.clone(),
                reason: error.0,
            })?
            .ok_or_else(|| PlacementApplyError {
                failed_index: index,
                applied: 0,
                asset: placement.asset.clone(),
                reason: "asset is not present in the mounted archive index".into(),
            })?;
        resolved.push(asset);
    }

    for (index, (placement, asset)) in document.placements.iter().zip(resolved).enumerate() {
        if let Err(reason) = sink.spawn_resolved_placement(placement, asset) {
            return Err(PlacementApplyError {
                failed_index: index,
                applied: index,
                asset: placement.asset.clone(),
                reason,
            });
        }
    }
    Ok(document.placements.len())
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
        assert_eq!(doc.placements.len(), 2);
        assert_eq!(doc.placements[0].asset, "props/maze_bank_ramp.ydr");
        assert_eq!(doc.placements[0].position, [0.0, 0.0, 0.0]);
        assert_eq!(doc.placements[0].rotation, [0.0, 0.0, 0.0]);
        assert_eq!(doc.placements[1].asset, "props/maze_bank_barrier.ydr");
        assert_eq!(doc.placements[1].position, [4.0, 0.0, 0.0]);
        assert_eq!(doc.placements[1].rotation, [0.0, 0.0, 90.0]);
        assert_eq!(doc.placements[1].scale, [1.0, 1.0, 1.0]);
    }

    struct RecordingSink {
        assets: Vec<String>,
        fail_on: Option<usize>,
    }
    impl PlacementSink for RecordingSink {
        fn spawn_placement(&mut self, placement: &JsonPlacement) -> Result<(), String> {
            if self.fail_on == Some(self.assets.len()) {
                return Err("test asset resolver failure".into());
            }
            self.assets.push(placement.asset.clone());
            Ok(())
        }
    }

    #[test]
    fn applies_validated_placements_in_document_order() {
        let doc = parse_json_placements(VALID).unwrap();
        let mut sink = RecordingSink { assets: Vec::new(), fail_on: None };
        assert_eq!(apply_json_placements(&doc, &mut sink).unwrap(), 2);
        assert_eq!(sink.assets, vec![
            "props/maze_bank_ramp.ydr".to_string(),
            "props/maze_bank_barrier.ydr".to_string(),
        ]);
    }

    #[test]
    fn reports_partial_application_when_engine_sink_fails() {
        let doc = parse_json_placements(VALID).unwrap();
        let mut sink = RecordingSink { assets: Vec::new(), fail_on: Some(1) };
        let error = apply_json_placements(&doc, &mut sink).unwrap_err();
        assert_eq!(error.failed_index, 1);
        assert_eq!(error.applied, 1);
        assert_eq!(error.asset, "props/maze_bank_barrier.ydr");
        assert_eq!(sink.assets.len(), 1);
    }

    #[test]
    fn preflights_all_placement_assets_against_mount_index() {
        let doc = parse_json_placements(VALID).unwrap();
        let mut mounts = crate::AssetMountIndex::new();
        for (path, entry_index) in [
            ("props/maze_bank_ramp.ydr", 1),
            ("props/maze_bank_barrier.ydr", 2),
        ] {
            mounts.insert(path, crate::MountedAsset {
                archive: "mods/maze.rpf".into(),
                entry_index,
                offset: u64::from(entry_index) * 512,
                length: 128,
                kind: crate::MountedAssetKind::Binary,
            }).unwrap();
        }
        assert_eq!(validate_json_placement_assets(&doc, &mounts).unwrap(), 2);
    }

    #[test]
    fn missing_placement_asset_is_reported_before_any_spawn() {
        let doc = parse_json_placements(VALID).unwrap();
        let mut mounts = crate::AssetMountIndex::new();
        mounts.insert("props/maze_bank_ramp.ydr", crate::MountedAsset {
            archive: "mods/maze.rpf".into(),
            entry_index: 1,
            offset: 512,
            length: 128,
            kind: crate::MountedAssetKind::Binary,
        }).unwrap();
        let error = validate_json_placement_assets(&doc, &mounts).unwrap_err();
        assert_eq!(error.failed_index, 1);
        assert_eq!(error.asset, "props/maze_bank_barrier.ydr");
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
