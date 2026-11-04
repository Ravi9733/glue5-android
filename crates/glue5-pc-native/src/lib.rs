//! Capability gate for a future native PC-game backend.
//!
//! This crate intentionally does not load iOS Mach-O binaries, Windows PE
//! executables, or proprietary game archives. It provides a conservative
//! readiness report so the Android app cannot mistake copied game data or an
//! iOS executable for a runnable Android game runtime.

use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackendState {
    MissingAndroidEngine,
    MissingVulkanTranslationLayer,
    MissingPreparedGameData,
    ReadyForRuntimeIntegration,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadinessReport {
    pub state: BackendState,
    pub android_engine_present: bool,
    pub vulkan_translation_present: bool,
    pub prepared_data_present: bool,
    pub blockers: Vec<&'static str>,
}

/// Evaluate required component paths. This is not a game launch check.
pub fn inspect_layout(
    root: &Path,
    android_engine_relative: &Path,
    vulkan_layer_relative: &Path,
    prepared_data_relative: &Path,
) -> ReadinessReport {
    let engine = root.join(android_engine_relative).is_file();
    let vulkan = root.join(vulkan_layer_relative).is_file();
    let data = root.join(prepared_data_relative).is_dir();
    let mut blockers = Vec::new();
    if !engine { blockers.push("Android ARM64 native game engine is missing"); }
    if !vulkan { blockers.push("Android Vulkan graphics translation layer is missing"); }
    if !data { blockers.push("Prepared and version-checked PC game data is missing"); }
    let state = if !engine { BackendState::MissingAndroidEngine }
        else if !vulkan { BackendState::MissingVulkanTranslationLayer }
        else if !data { BackendState::MissingPreparedGameData }
        else { BackendState::ReadyForRuntimeIntegration };
    ReadinessReport {
        state,
        android_engine_present: engine,
        vulkan_translation_present: vulkan,
        prepared_data_present: data,
        blockers,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, time::{SystemTime, UNIX_EPOCH}};
    fn temp_root() -> std::path::PathBuf {
        let n = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        std::env::temp_dir().join(format!("glue5-pc-native-{}", n))
    }
    #[test]
    fn empty_layout_reports_all_blockers() {
        let root = temp_root(); fs::create_dir_all(&root).unwrap();
        let r = inspect_layout(&root, Path::new("bin/engine.so"), Path::new("lib/vulkan.so"), Path::new("data"));
        assert_eq!(r.state, BackendState::MissingAndroidEngine);
        assert_eq!(r.blockers.len(), 3);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn reports_missing_data_when_runtime_files_exist() {
        let root = temp_root();
        fs::create_dir_all(root.join("bin")).unwrap(); fs::create_dir_all(root.join("lib")).unwrap();
        fs::write(root.join("bin/engine.so"), b"placeholder").unwrap();
        fs::write(root.join("lib/vulkan.so"), b"placeholder").unwrap();
        let r = inspect_layout(&root, Path::new("bin/engine.so"), Path::new("lib/vulkan.so"), Path::new("data"));
        assert_eq!(r.state, BackendState::MissingPreparedGameData);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn ready_means_paths_exist_not_that_game_is_playable() {
        let root = temp_root();
        fs::create_dir_all(root.join("bin")).unwrap(); fs::create_dir_all(root.join("lib")).unwrap(); fs::create_dir_all(root.join("data")).unwrap();
        fs::write(root.join("bin/engine.so"), b"placeholder").unwrap();
        fs::write(root.join("lib/vulkan.so"), b"placeholder").unwrap();
        let r = inspect_layout(&root, Path::new("bin/engine.so"), Path::new("lib/vulkan.so"), Path::new("data"));
        assert_eq!(r.state, BackendState::ReadyForRuntimeIntegration);
        assert!(r.blockers.is_empty());
        fs::remove_dir_all(root).unwrap();
    }
}
