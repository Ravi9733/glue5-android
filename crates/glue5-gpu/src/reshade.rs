//! ReShade preset (.ini) parsing and safe effect-file discovery.
//!
//! This module deliberately parses the preset format independently of the
//! effect compiler. A preset is configuration, not executable shader code:
//! callers must compile the referenced .fx files before reporting an effect
//! as active.

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Preset {
    pub sections: BTreeMap<String, BTreeMap<String, String>>,
    pub technique_sorting: Vec<String>,
    pub preprocessor_definitions: BTreeMap<String, String>,
    pub techniques: BTreeMap<String, bool>,
    pub diagnostics: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EffectFile {
    /// Section name as written in the preset, normally a .fx filename.
    pub section: String,
    /// Resolved path relative to the preset directory.
    pub path: PathBuf,
    /// Whether the matching technique is enabled in the preset.
    pub enabled: bool,
}

impl Preset {
    /// Parse UTF-8 ReShade preset text. Unknown sections and keys are retained
    /// so newer preset versions remain inspectable rather than being dropped.
    pub fn parse(input: &str) -> Self {
        let mut preset = Self::default();
        let mut current = String::new();

        for (line_no, raw) in input.lines().enumerate() {
            let line = raw.trim().trim_start_matches('\u{feff}').trim();
            if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
                continue;
            }
            if line.starts_with('[') && line.ends_with(']') && line.len() >= 2 {
                current = line[1..line.len() - 1].trim().to_owned();
                if current.is_empty() {
                    preset.diagnostics.push(format!("Line {}: empty section name", line_no + 1));
                } else {
                    preset.sections.entry(current.clone()).or_default();
                }
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                preset.diagnostics.push(format!("Line {}: ignored entry without '='", line_no + 1));
                continue;
            };
            let key = key.trim();
            if key.is_empty() || current.is_empty() {
                preset.diagnostics.push(format!("Line {}: key outside a named section", line_no + 1));
                continue;
            }
            let value = value.trim().trim_matches('"').to_owned();
            preset.sections.entry(current.clone()).or_default().insert(key.to_owned(), value);
        }

        if let Some(general) = find_section(&preset.sections, "GENERAL") {
            if let Some(order) = find_value(general, "TechniqueSorting") {
                preset.technique_sorting = order.split(',')
                    .map(str::trim).filter(|s| !s.is_empty()).map(str::to_owned).collect();
            }
            if let Some(defs) = find_value(general, "PreprocessorDefinitions") {
                for item in defs.split(',').map(str::trim).filter(|s| !s.is_empty()) {
                    if let Some((key, value)) = item.split_once('=') {
                        preset.preprocessor_definitions.insert(key.trim().to_owned(), value.trim().to_owned());
                    } else {
                        preset.diagnostics.push(format!("Invalid preprocessor definition: {item}"));
                    }
                }
            }
        }

        for (section, values) in &preset.sections {
            if section.eq_ignore_ascii_case("GENERAL") || section.eq_ignore_ascii_case("TECHNIQUES") {
                for (name, value) in values {
                    if let Some(enabled) = parse_bool(value) {
                        preset.techniques.insert(name.clone(), enabled);
                    }
                }
            } else if section.to_ascii_lowercase().ends_with(".fx") {
                for (name, value) in values {
                    if name.eq_ignore_ascii_case("Enabled") {
                        if let Some(enabled) = parse_bool(value) {
                            preset.techniques.insert(section.clone(), enabled);
                        }
                    }
                }
            }
        }

        preset
    }

    /// Return referenced .fx sections in preset order, then remaining .fx
    /// sections. Paths are constrained to the preset folder (no ../ escape).
    pub fn effect_files(&self, preset_dir: &Path) -> Vec<EffectFile> {
        let mut names = Vec::new();
        for name in &self.technique_sorting {
            if name.to_ascii_lowercase().ends_with(".fx")
                && self.sections.keys().any(|s| s.eq_ignore_ascii_case(name))
                && !names.iter().any(|n: &String| n.eq_ignore_ascii_case(name))
            {
                names.push(name.clone());
            }
        }
        for section in self.sections.keys() {
            if section.to_ascii_lowercase().ends_with(".fx")
                && !names.iter().any(|n| n.eq_ignore_ascii_case(section))
            {
                names.push(section.clone());
            }
        }

        names.into_iter().filter_map(|section| {
            let relative = Path::new(&section);
            if relative.components().any(|c| !matches!(c, Component::Normal(_))) {
                return None;
            }
            let path = preset_dir.join(relative);
            Some(EffectFile {
                enabled: self.techniques.iter()
                    .find(|(name, _)| name.eq_ignore_ascii_case(&section))
                    .map(|(_, enabled)| *enabled)
                    .unwrap_or(true),
                section,
                path,
            })
        }).collect()
    }

    /// Read a parameter from an effect section without losing its original
    /// spelling or formatting.
    pub fn parameter(&self, effect: &str, name: &str) -> Option<&str> {
        let section = find_section(&self.sections, effect)?;
        find_value(section, name)
    }
}

fn find_section<'a>(
    sections: &'a BTreeMap<String, BTreeMap<String, String>>,
    wanted: &str,
) -> Option<&'a BTreeMap<String, String>> {
    sections.iter().find(|(name, _)| name.eq_ignore_ascii_case(wanted)).map(|(_, values)| values)
}

fn find_value<'a>(values: &'a BTreeMap<String, String>, wanted: &str) -> Option<&'a str> {
    values.iter().find(|(name, _)| name.eq_ignore_ascii_case(wanted)).map(|(_, value)| value.as_str())
}

fn parse_bool(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
; ReShade preset
[GENERAL]
PreprocessorDefinitions=USE_LUT=1,QUALITY=2
TechniqueSorting=Bloom.fx, Tonemap.fx
[TECHNIQUES]
Bloom@Bloom.fx=1
Tonemap@Tonemap.fx=0
[Bloom.fx]
Enabled=1
Threshold=1.25
Tint=1.0, 0.8, 0.6
[Tonemap.fx]
Enabled=0
Exposure=0.5
"#;

    #[test]
    fn parses_general_order_definitions_and_parameters() {
        let p = Preset::parse(SAMPLE);
        assert_eq!(p.technique_sorting, ["Bloom.fx", "Tonemap.fx"]);
        assert_eq!(p.preprocessor_definitions.get("USE_LUT").map(String::as_str), Some("1"));
        assert_eq!(p.preprocessor_definitions.get("QUALITY").map(String::as_str), Some("2"));
        assert_eq!(p.parameter("Bloom.fx", "threshold"), Some("1.25"));
        assert_eq!(p.parameter("Bloom.fx", "Tint"), Some("1.0, 0.8, 0.6"));
    }

    #[test]
    fn discovers_effects_in_sorting_order_and_keeps_disabled_state() {
        let p = Preset::parse(SAMPLE);
        let effects = p.effect_files(Path::new("/presets/example"));
        assert_eq!(effects.iter().map(|e| e.section.as_str()).collect::<Vec<_>>(), ["Bloom.fx", "Tonemap.fx"]);
        assert!(effects[0].enabled);
        assert!(!effects[1].enabled);
        assert_eq!(effects[0].path, PathBuf::from("/presets/example/Bloom.fx"));
    }

    #[test]
    fn rejects_parent_traversal_effect_paths() {
        let p = Preset::parse("[../outside.fx]\nEnabled=1\n");
        assert!(p.effect_files(Path::new("/presets")).is_empty());
    }

    #[test]
    fn preserves_unknown_sections_and_reports_malformed_lines() {
        let p = Preset::parse("bad line\n[NewFeature]\nMode=fast\n");
        assert_eq!(p.parameter("NewFeature", "Mode"), Some("fast"));
        assert_eq!(p.diagnostics.len(), 1);
    }
}
