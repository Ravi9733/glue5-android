//! ReShade preset (.ini) parsing and safe effect-file discovery.
//!
//! A preset is configuration, not executable shader code. Callers must
//! compile and execute the referenced effects before reporting them active.

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Preset {
    pub sections: BTreeMap<String, BTreeMap<String, String>>,
    pub technique_sorting: Vec<String>,
    pub preprocessor_definitions: BTreeMap<String, String>,
    /// Enabled state keyed by effect filename (for example, "Bloom.fx").
    pub techniques: BTreeMap<String, bool>,
    pub diagnostics: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EffectFile {
    pub section: String,
    pub path: PathBuf,
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
            if section.eq_ignore_ascii_case("TECHNIQUES") {
                // ReShade keys are commonly TechniqueName@EffectFile.fx.
                for (name, value) in values {
                    if let (Some(effect), Some(enabled)) = (effect_from_technique(name), parse_bool(value)) {
                        preset.techniques.insert(effect.to_owned(), enabled);
                    }
                }
            } else if section.to_ascii_lowercase().ends_with(".fx") {
                if let Some(value) = find_value(values, "Enabled").and_then(parse_bool) {
                    preset.techniques.insert(section.clone(), value);
                }
            }
        }

        preset
    }

    /// Return referenced .fx sections in preset order, then remaining .fx
    /// sections. Both slash styles are accepted, but paths may not escape the
    /// preset directory or refer to absolute/drive-qualified locations.
    pub fn effect_files(&self, preset_dir: &Path) -> Vec<EffectFile> {
        let mut names = Vec::new();
        for item in &self.technique_sorting {
            let Some(name) = effect_from_technique(item) else { continue };
            if let Some(section) = self.sections.keys().find(|section| effect_name_eq(section, name)) {
                if !names.iter().any(|n: &String| effect_name_eq(n, section)) {
                    names.push(section.clone());
                }
            }
        }
        for section in self.sections.keys() {
            if section.to_ascii_lowercase().ends_with(".fx")
                && !names.iter().any(|n: &String| n.eq_ignore_ascii_case(section))
            {
                names.push(section.clone());
            }
        }

        names.into_iter().filter_map(|section| {
            let normalized = section.replace('\\', "/");
            if normalized.starts_with('/')
                || normalized.starts_with("//")
                || normalized.as_bytes().get(1) == Some(&b':')
            {
                return None;
            }
            let relative = Path::new(&normalized);
            if relative.components().any(|c| !matches!(c, Component::Normal(_))) {
                return None;
            }
            // Android storage is case-sensitive, while ReShade presets and
            // ZIP archives often differ only in path casing. Resolve each
            // component case-insensitively without following symlinks.
            let expected = preset_dir.join(relative);
            let path = resolve_case_insensitive(preset_dir, relative).unwrap_or(expected);
            let enabled = self.techniques.iter()
                .find(|(name, _)| effect_name_eq(name, &section))
                .map(|(_, enabled)| *enabled)
                .unwrap_or(true);
            Some(EffectFile { section, path, enabled })
        }).collect()
    }

    /// Read a parameter from an effect section without losing its original
    /// spelling or formatting.
    pub fn parameter(&self, effect: &str, name: &str) -> Option<&str> {
        let section = find_section(&self.sections, effect)?;
        find_value(section, name)
    }

    /// Translate a parsed preset into vkBasalt's ReShade-FX config format.
    /// This preserves the original preset/effect files; the generated config
    /// only maps enabled effects and their tunable uniform values.
    pub fn to_vkbasalt_config(&self, preset_dir: &Path, depth_capture: bool) -> String {
        let effects = self.effect_files(preset_dir);
        let mut used = std::collections::BTreeSet::new();
        let mut ordered_effects = Vec::new();
        let mut mappings = Vec::new();
        let mut effect_states = Vec::new();
        let mut parameters = Vec::new();

        for effect in effects {
            if !effect.path.is_file() {
                continue;
            }
            let stem = effect.path.file_stem().and_then(|s| s.to_str()).unwrap_or("effect");
            let mut alias: String = stem.chars()
                .map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' })
                .collect();
            if alias.is_empty() || alias.as_bytes()[0].is_ascii_digit() {
                alias.insert_str(0, "fx_");
            }
            let base = alias.clone();
            let mut suffix = 2;
            while !used.insert(alias.to_ascii_lowercase()) {
                alias = format!("{base}_{suffix}");
                suffix += 1;
            }
            ordered_effects.push(alias.clone());
            effect_states.push(format!("{alias}_enabled = {}", if effect.enabled { "True" } else { "False" }));
            mappings.push(format!("{alias} = \"{}\"", effect.path.display()));

            if let Some(section) = find_section(&self.sections, &effect.section) {
                for (name, value) in section {
                    if name.eq_ignore_ascii_case("Enabled") {
                        continue;
                    }
                    let parts: Vec<_> = value.split(',').map(str::trim).filter(|v| !v.is_empty()).collect();
                    if parts.len() > 1 {
                        for (index, part) in parts.iter().enumerate() {
                            parameters.push(format!("{alias}_{name}_{index} = {part}"));
                        }
                    } else if let Some(value) = parts.first() {
                        parameters.push(format!("{alias}_{name} = {value}"));
                    }
                }
            }
        }

        let shaders = if preset_dir.join("Shaders").is_dir() {
            preset_dir.join("Shaders")
        } else {
            preset_dir.to_path_buf()
        };
        let textures = if preset_dir.join("Textures").is_dir() {
            preset_dir.join("Textures")
        } else {
            preset_dir.to_path_buf()
        };
        let mut output = String::new();
        output.push_str("# Generated from the imported ReShade preset by Glue 5. Original files are preserved.\n");
        output.push_str(&format!("effects = {}\n", ordered_effects.join(":")));
        output.push_str(&format!("reshadeIncludePath = \"{}\"\n", shaders.display()));
        output.push_str(&format!("reshadeTexturePath = \"{}\"\n", textures.display()));
        let definitions = self.preprocessor_definitions.iter()
            .map(|(name, value)| format!("{name}={value}"))
            .collect::<Vec<_>>()
            .join(",");
        output.push_str(&format!("reshadePreprocessorDefinitions = \"{definitions}\"\n"));
        output.push_str(&format!("enableOnLaunch = True\ndepthCapture = {}\n", if depth_capture { "on" } else { "off" }));
        for line in mappings.into_iter().chain(effect_states).chain(parameters) {
            output.push_str(&line);
            output.push('\n');
        }
        output
    }
}

/// Resolve a relative shader path on case-sensitive filesystems while
/// accepting the case-insensitive paths commonly stored in Windows presets.
/// Symlinks are deliberately not followed.
fn resolve_case_insensitive(root: &Path, relative: &Path) -> Option<PathBuf> {
    let mut current = root.to_path_buf();
    let components: Vec<_> = relative.components().collect();
    for (index, component) in components.iter().enumerate() {
        let Component::Normal(wanted) = component else { return None };
        let wanted = wanted.to_string_lossy();
        let exact = current.join(wanted.as_ref());
        let is_last = index + 1 == components.len();
        let candidate = if exact.exists() {
            exact
        } else {
            std::fs::read_dir(&current).ok()?.flatten().find_map(|entry| {
                let kind = entry.file_type().ok()?;
                if kind.is_symlink() || !entry.file_name().to_string_lossy().eq_ignore_ascii_case(&wanted) {
                    return None;
                }
                if (is_last && kind.is_file()) || (!is_last && kind.is_dir()) {
                    Some(entry.path())
                } else {
                    None
                }
            })?
        };
        let kind = std::fs::symlink_metadata(&candidate).ok()?.file_type();
        if kind.is_symlink() || (is_last && !kind.is_file()) || (!is_last && !kind.is_dir()) {
            return None;
        }
        current = candidate;
    }
    current.is_file().then_some(current)
}

/// Extract the effect filename from either Effect.fx or Technique@Effect.fx.
fn effect_from_technique(value: &str) -> Option<&str> {
    let effect = value.rsplit_once('@').map_or(value, |(_, effect)| effect).trim();
    effect.to_ascii_lowercase().ends_with(".fx").then_some(effect)
}

fn effect_name_eq(left: &str, right: &str) -> bool {
    left.replace('\\', "/").eq_ignore_ascii_case(&right.replace('\\', "/"))
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
TechniqueSorting=Bloom@Bloom.fx, Tonemap@Tonemap.fx
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
        assert_eq!(p.technique_sorting, ["Bloom@Bloom.fx", "Tonemap@Tonemap.fx"]);
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
    fn accepts_windows_relative_effect_paths() {
        let p = Preset::parse("[GENERAL]\nTechniqueSorting=Color\u{5c}Grade@Shaders\u{5c}ColorGrade.fx\n[Shaders/ColorGrade.fx]\nEnabled=1\n");
        let effects = p.effect_files(Path::new("/presets"));
        assert_eq!(effects.len(), 1);
        assert_eq!(effects[0].path, PathBuf::from("/presets/Shaders/ColorGrade.fx"));
    }

    #[test]
    fn resolves_effect_paths_case_insensitively_on_android_filesystems() {
        let root = std::env::temp_dir().join(format!("glue5-reshade-case-{}", std::process::id()));
        let shaders = root.join("shaders");
        std::fs::create_dir_all(&shaders).unwrap();
        let actual = shaders.join("Bloom.FX");
        std::fs::write(&actual, "// fixture").unwrap();
        let preset = Preset::parse(
            "[GENERAL]\\nTechniqueSorting=Bloom@Shaders/Bloom.fx\\n[Shaders/Bloom.fx]\\nEnabled=1\\n"
        );
        let effects = preset.effect_files(&root);
        assert_eq!(effects.len(), 1);
        assert_eq!(effects[0].path, actual);
        assert!(effects[0].path.is_file());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_parent_and_absolute_effect_paths() {
        let p = Preset::parse("[../outside.fx]\nEnabled=1\n[C:/outside.fx]\nEnabled=1\n[/root.fx]\nEnabled=1\n");
        assert!(p.effect_files(Path::new("/presets")).is_empty());
    }

    #[test]
    fn preserves_unknown_sections_and_reports_malformed_lines() {
        let p = Preset::parse("bad line\n[NewFeature]\nMode=fast\n");
        assert_eq!(p.parameter("NewFeature", "Mode"), Some("fast"));
        assert_eq!(p.diagnostics.len(), 1);
    }

    #[test]
    fn converts_enabled_effects_and_uniforms_to_vkbasalt_config() {
        let root = std::env::temp_dir().join(format!("glue5-reshade-test-{}", std::process::id()));
        let shaders = root.join("Shaders");
        std::fs::create_dir_all(&shaders).unwrap();
        std::fs::write(shaders.join("Bloom.fx"), "// fixture").unwrap();
        let preset = Preset::parse("[GENERAL]\nTechniqueSorting=Bloom@Shaders/Bloom.fx\nPreprocessorDefinitions=USE_LUT=1,QUALITY=2\n[TECHNIQUES]\nBloom@Shaders/Bloom.fx=1\n[Shaders/Bloom.fx]\nEnabled=1\nThreshold=1.25\nTint=1.0, 0.8, 0.6\n");
        let config = preset.to_vkbasalt_config(&root, false);
        assert!(config.contains("effects = Bloom"));
        assert!(config.contains("Bloom = "));
        assert!(config.contains("Bloom_Threshold = 1.25"));
        assert!(config.contains("Bloom_Tint_0 = 1.0"));
        assert!(config.contains("Bloom_Tint_2 = 0.6"));
        assert!(config.contains("reshadePreprocessorDefinitions = \"QUALITY=2,USE_LUT=1\""));
        assert!(config.contains("depthCapture = off"));
        let depth_config = preset.to_vkbasalt_config(&root, true);
        assert!(depth_config.contains("depthCapture = on"));
        let _ = std::fs::remove_dir_all(root);
    }
}
