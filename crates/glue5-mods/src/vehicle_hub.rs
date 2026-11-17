//! Metadata and trust checks for the in-game Vehicle Hub catalog.
//!
//! This module deliberately does not install or load vehicle assets. A valid
//! catalog entry is only a discoverable package; RPF/DLC loading and native
//! vehicle registration must be implemented and tested before packages can
//! be advertised as spawnable.

use std::collections::HashSet;

const MAX_CATALOG_BYTES: usize = 2 * 1024 * 1024;
const MAX_ENTRIES: usize = 500;
const MAX_PACKAGE_BYTES: u64 = 1024 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PackageFormat {
    LooseFiles,
    RpfDlc,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VehiclePackage {
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub download_url: String,
    pub sha256: String,
    pub size_bytes: u64,
    pub format: PackageFormat,
    /// False until the native asset loader can mount this package format.
    pub spawn_supported: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Catalog {
    pub version: u32,
    pub vehicles: Vec<VehiclePackage>,
}

impl Catalog {
    /// Parse an untrusted remote catalog. Invalid entries reject the whole
    /// catalog so a partial/malformed feed is never silently presented.
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > MAX_CATALOG_BYTES {
            return Err("vehicle catalog exceeds 2 MiB".into());
        }
        let root: serde_json::Value =
            serde_json::from_slice(bytes).map_err(|e| format!("invalid vehicle catalog JSON: {e}"))?;
        let version = root.get("version").and_then(|v| v.as_u64())
            .ok_or("vehicle catalog has no numeric version")?;
        if version != 1 {
            return Err(format!("unsupported vehicle catalog version: {version}"));
        }
        let entries = root.get("vehicles").and_then(|v| v.as_array())
            .ok_or("vehicle catalog has no vehicles array")?;
        if entries.len() > MAX_ENTRIES {
            return Err(format!("vehicle catalog has more than {MAX_ENTRIES} entries"));
        }

        let mut ids = HashSet::new();
        let mut vehicles = Vec::with_capacity(entries.len());
        for (index, entry) in entries.iter().enumerate() {
            let field = |name: &str| entry.get(name).and_then(|v| v.as_str())
                .ok_or_else(|| format!("vehicle {index}: missing string field {name}"));
            let id = field("id")?.to_owned();
            if !valid_id(&id) || !ids.insert(id.clone()) {
                return Err(format!("vehicle {index}: invalid or duplicate id {id:?}"));
            }
            let name = bounded(field("name")?, 80, "name", index)?;
            let version = bounded(field("version")?, 32, "version", index)?;
            let author = bounded(field("author")?, 80, "author", index)?;
            let download_url = field("download_url")?.to_owned();
            if !download_url.starts_with("https://") || download_url.len() > 2048
                || download_url.chars().any(char::is_control) {
                return Err(format!("vehicle {index}: download_url must be a valid-length HTTPS URL"));
            }
            let sha256 = field("sha256")?.to_ascii_lowercase();
            if sha256.len() != 64 || !sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err(format!("vehicle {index}: sha256 must contain 64 hexadecimal characters"));
            }
            let size_bytes = entry.get("size_bytes").and_then(|v| v.as_u64())
                .ok_or_else(|| format!("vehicle {index}: missing numeric size_bytes"))?;
            if size_bytes == 0 || size_bytes > MAX_PACKAGE_BYTES {
                return Err(format!("vehicle {index}: package size must be between 1 byte and 1 GiB"));
            }
            let format = match field("format")? {
                "loose_files" => PackageFormat::LooseFiles,
                "rpf_dlc" => PackageFormat::RpfDlc,
                other => return Err(format!("vehicle {index}: unsupported package format {other:?}")),
            };
            // Current Glue 5 does not yet mount third-party vehicle assets.
            let spawn_supported = false;
            vehicles.push(VehiclePackage {
                id, name, version, author, download_url, sha256, size_bytes, format, spawn_supported,
            });
        }
        Ok(Self { version: version as u32, vehicles })
    }
}

fn bounded(value: &str, max: usize, field: &str, index: usize) -> Result<String, String> {
    if value.trim().is_empty() || value.len() > max || value.chars().any(char::is_control) {
        return Err(format!("vehicle {index}: invalid {field}"));
    }
    Ok(value.to_owned())
}

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 48
        && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog(format: &str) -> Vec<u8> {
        format!(r#"{{
            "version":1,
            "vehicles":[{{
                "id":"mazda-furai",
                "name":"Mazda Furai",
                "version":"1.0",
                "author":"Community",
                "download_url":"https://mods.example/cars/mazda-furai.zip",
                "sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "size_bytes":286535680,
                "format":"{format}"
            }}]
        }}"#).into_bytes()
    }

    #[test]
    fn parses_catalog_but_does_not_claim_assets_are_spawnable() {
        let parsed = Catalog::parse(&catalog("rpf_dlc")).unwrap();
        assert_eq!(parsed.vehicles.len(), 1);
        assert_eq!(parsed.vehicles[0].format, PackageFormat::RpfDlc);
        assert!(!parsed.vehicles[0].spawn_supported);
    }

    #[test]
    fn rejects_non_https_and_duplicate_ids() {
        let mut bytes = catalog("loose_files");
        let text = String::from_utf8(bytes.clone()).unwrap().replace("https://", "http://");
        bytes = text.into_bytes();
        assert!(Catalog::parse(&bytes).unwrap_err().contains("HTTPS"));

        let text = String::from_utf8(catalog("loose_files")).unwrap();
        let duplicate = text.replace(r#""vehicles":["#, r#""vehicles":[{"id":"mazda-furai","name":"Duplicate","version":"1","author":"x","download_url":"https://x.test/a.zip","sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","size_bytes":1,"format":"loose_files"} , "#);
        assert!(Catalog::parse(duplicate.as_bytes()).unwrap_err().contains("duplicate"));
    }

    #[test]
    fn rejects_bad_hash_and_oversized_catalog() {
        let text = String::from_utf8(catalog("loose_files")).unwrap().replace(&"a".repeat(64), "bad");
        assert!(Catalog::parse(text.as_bytes()).unwrap_err().contains("sha256"));
        assert!(Catalog::parse(&vec![b' '; MAX_CATALOG_BYTES + 1]).unwrap_err().contains("2 MiB"));
    }
}
