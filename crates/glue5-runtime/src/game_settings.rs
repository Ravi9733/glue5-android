//! The engine's graphics settings: `settings.xml` in the user directory,
//! which its pause menu edits and saves (Settings › Graphics). Changes apply
//! at once, or, for those that need it, at a restart the engine asks for
//! (see [`crate::exit`]).
//!
//! The host never overrides them on the command line, which would undo the
//! menu's choices at every start: it only writes the low-memory preset over
//! them, once, and puts them back to the engine's defaults when asked.

use std::io;
use std::path::{Path, PathBuf};

fn path(user_dir: &Path) -> PathBuf {
    user_dir.join("Documents/Rockstar Games/GTA V/settings.xml")
}

/// Low Memory must not write a different graphics preset. The player's
/// settings.xml remains authoritative so ON and OFF have identical visual
/// quality; resource management is handled separately by the runtime.
const LOW: &[(&str, &str)] = &[];

/// What the engine writes when it first starts without settings (its
/// defaults for the adapter it is given, the same everywhere).
const DEFAULTS: &str = include_str!("settings.xml");

/// Writes the low-memory preset over the current settings (or the engine's
/// defaults, before its first start).
pub fn apply_low_preset(user_dir: &Path) -> io::Result<()> {
    apply(user_dir, LOW)
}

/// A balanced explicit High-quality preset: sharp textures and normal
/// world density without enabling the expensive MSAA/ultra-shadow paths.
const HIGH: &[(&str, &str)] = &[
    ("TextureQuality", "2"),
    ("ShadowQuality", "1"),
    ("ReflectionQuality", "1"),
    ("ParticleQuality", "1"),
    ("GrassQuality", "1"),
    ("WaterQuality", "1"),
    ("ShaderQuality", "1"),
    ("AnisotropicFiltering", "4"),
    ("SSAO", "1"),
    ("PostFX", "1"),
    ("LodScale", "0.750000"),
    ("PedLodBias", "0.500000"),
    ("VehicleLodBias", "0.500000"),
    ("CityDensity", "0.750000"),
    ("PedVarietyMultiplier", "0.750000"),
    ("VehicleVarietyMultiplier", "0.750000"),
    ("MotionBlurStrength", "0.000000"),
];

/// Restores clear High graphics once on Android, replacing any old low preset.
pub fn apply_high_preset(user_dir: &Path) -> io::Result<()> {
    apply(user_dir, HIGH)
}

/// Reduce CPU-heavy world population while keeping texture, shader,
/// shadow and effects quality at the High preset. This specifically targets
/// frame-time spikes when many vehicles and pedestrians are visible.
const SMOOTH_WORLD: &[(&str, &str)] = &[
    ("CityDensity", "0.450000"),
    ("PedVarietyMultiplier", "0.500000"),
    ("VehicleVarietyMultiplier", "0.500000"),
    ("LodScale", "0.600000"),
    ("PedLodBias", "0.400000"),
    ("VehicleLodBias", "0.400000"),
];

/// Applies the one-time Android crowd/streaming workload reduction.
pub fn apply_smooth_world_preset(user_dir: &Path) -> io::Result<()> {
    apply(user_dir, SMOOTH_WORLD)
}

/// Writes `settings` (name, value) over the current settings (or the
/// engine's defaults, before its first start). Names the file lacks are
/// skipped, with a warning.
pub fn apply<N: AsRef<str>, V: AsRef<str>>(user_dir: &Path, settings: &[(N, V)]) -> io::Result<()> {
    let path = path(user_dir);
    let mut xml = std::fs::read_to_string(&path).unwrap_or_else(|_| DEFAULTS.to_string());
    for (name, value) in settings {
        let name = name.as_ref();
        if !xml.contains(&format!("<{name} value=\"")) {
            log::warn!("game settings: no {name} in settings.xml");
        }
        xml = set(&xml, name, value.as_ref());
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(&path, xml)
}

/// Puts the graphics settings back to the engine's defaults, keeping the
/// file as it was in `settings.xml.bak` (the rest of it, and the saves,
/// stay): the way out of a setting the engine cannot run with, which stops
/// it before its menu can change it back.
pub fn reset_graphics(user_dir: &Path) -> io::Result<()> {
    let path = path(user_dir);
    let Ok(xml) = std::fs::read_to_string(&path) else {
        return Ok(()); // none: the engine starts from its defaults
    };
    std::fs::write(path.with_extension("xml.bak"), &xml)?;
    std::fs::write(&path, with_graphics(&xml, DEFAULTS))
}

/// `xml` with the `<graphics>` section of `from` (all of `from` if either
/// lacks one).
fn with_graphics(xml: &str, from: &str) -> String {
    const END: &str = "</graphics>";
    let section = |s: &str| Some(s.find("<graphics>")?..s.find(END)? + END.len());
    match (section(xml), section(from)) {
        (Some(mine), Some(theirs)) => format!("{}{}{}", &xml[..mine.start], &from[theirs], &xml[mine.end..]),
        _ => from.to_string(),
    }
}

/// `xml` with `<name value="..." />` set to `value` (unchanged if absent).
fn set(xml: &str, name: &str, value: &str) -> String {
    let tag = format!("<{name} value=\"");
    let Some(start) = xml.find(&tag).map(|i| i + tag.len()) else { return xml.to_string() };
    let Some(len) = xml[start..].find('"') else { return xml.to_string() };
    format!("{}{value}{}", &xml[..start], &xml[start + len..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sets_values_by_exact_name() {
        let xml = "<LodScale value=\"1.000000\" />\n<MaxLodScale value=\"1.000000\" />";
        assert_eq!(
            set(xml, "LodScale", "0.000000"),
            "<LodScale value=\"0.000000\" />\n<MaxLodScale value=\"1.000000\" />"
        );
        assert_eq!(set(xml, "Missing", "0"), xml);
    }

    #[test]
    fn graphics_only_go_back_to_the_defaults() {
        let xml = "<Settings>\n  <graphics>\n    <MSAA value=\"2\" />\n  </graphics>\n  <video>\n    <VSync value=\"0\" />\n  </video>\n</Settings>";
        let reset = with_graphics(xml, DEFAULTS);
        assert!(reset.contains("<MSAA value=\"0\" />"));
        assert!(reset.contains("<VSync value=\"0\" />\n  </video>\n</Settings>"));
        assert_eq!(reset.matches("<graphics>").count(), 1);
        assert_eq!(with_graphics("broken", DEFAULTS), DEFAULTS);
    }

    #[test]
    fn smooth_world_values_are_in_the_defaults() {
        for (name, _) in SMOOTH_WORLD {
            assert!(DEFAULTS.contains(&format!(r#"<{name} value=""#)), "{name}");
        }
    }

    #[test]
    fn low_memory_does_not_override_player_graphics_settings() {
        assert!(LOW.is_empty(), "Low Memory must not write graphics settings");
    }

    #[test]
    fn preset_names_are_in_the_defaults() {
        for (name, _) in LOW {
            assert!(DEFAULTS.contains(&format!("<{name} value=\"")), "{name}");
        }
    }
}
