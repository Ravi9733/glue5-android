//! GPU drivers the user adds, for Adreno GPUs (Android): Adreno Tools
//! packages, the zips emulators take (Mesa's Turnip, other builds of
//! Qualcomm's driver): a `meta.json` and the driver's libraries. Each is
//! unpacked into `drivers/<id>` in the user directory, which is the app's
//! internal storage (the linker loads nothing from shared storage); the
//! setting names the one the renderer opens at launch (glue5-gpu's
//! `adreno.rs`).
//!
//! A driver can crash the app before its start page shows, at every launch:
//! a mark written before the driver opens and removed once a frame is
//! shown makes the next launch go back to the phone's driver.
#![cfg_attr(not(target_os = "android"), allow(dead_code))]

use std::path::{Path, PathBuf};

use serde::Deserialize;

/// What a package may hold (Qualcomm's builds come with their libraries).
const MAX_FILES: usize = 64;
const MAX_SIZE: usize = 512 << 20;

/// The mark of a launch that opened a driver and has shown no frame yet.
const TRYING: &str = "trying";

/// A package added.
#[derive(Clone, Debug, PartialEq)]
pub struct Package {
    /// Its folder's name, the setting's value.
    pub id: String,
    /// What its `meta.json` calls it.
    pub name: String,
    /// The driver's library, in the folder.
    pub library: String,
}

/// The drivers, as the settings show them.
#[derive(Default)]
pub struct Drivers {
    pub packages: Vec<Package>,
    /// The package the renderer runs on (none: the phone's driver).
    pub in_use: Option<String>,
    /// The driver running, as it calls itself.
    pub running: String,
    /// A package is being added.
    pub adding: bool,
    /// Why the phone's driver runs although another was chosen (it went
    /// back to it at launch), for the start page.
    pub fallback: Option<String>,
}

/// `meta.json` (libadrenotools' ADPKG.md); the rest of its fields are
/// not needed.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Meta {
    name: String,
    library_name: String,
}

pub fn dir(user_dir: &Path) -> PathBuf {
    user_dir.join("drivers")
}

/// The packages added, by name.
pub fn list(user_dir: &Path) -> Vec<Package> {
    let mut packages: Vec<_> = std::fs::read_dir(dir(user_dir))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let id = e.file_name().into_string().ok().filter(|id| !id.starts_with('.'))?;
            let meta = std::fs::read(e.path().join("meta.json")).ok()?;
            let meta: Meta = serde_json::from_slice(&meta).ok()?;
            Some(Package { id, name: meta.name, library: meta.library_name })
        })
        .collect();
    packages.sort_by(|a, b| a.name.cmp(&b.name));
    packages
}

/// Adds the package in `zip` (one with the same name is replaced: a newer
/// build of it).
pub fn add(user_dir: &Path, zip: &[u8]) -> Result<Package, String> {
    let mut files = glue5_mods::zip::read_up_to(zip, MAX_FILES, MAX_SIZE, "a GPU driver package")?;
    // Zipped in a folder of its own, or not.
    if let Some((top, _)) = files.first().and_then(|(n, _)| n.split_once('/'))
        && files.iter().all(|(n, _)| n.starts_with(top) && n[top.len()..].starts_with('/'))
    {
        let cut = top.len() + 1;
        for (name, _) in &mut files {
            name.drain(..cut);
        }
    }
    if files.iter().any(|(n, _)| n.contains('/')) {
        return Err("not a GPU driver package: it has folders".into());
    }
    let meta = files.iter().find(|(n, _)| n == "meta.json").ok_or("not a GPU driver package: no meta.json")?;
    let meta: Meta = serde_json::from_slice(&meta.1).map_err(|e| format!("its meta.json: {e}"))?;
    let library = files.iter().find(|(n, _)| *n == meta.library_name);
    if !library.is_some_and(|(_, data)| data.starts_with(b"\x7fELF")) {
        return Err(format!("the package has no driver library ({})", meta.library_name));
    }
    let id = id(&meta.name);
    let (to, tmp) = (dir(user_dir).join(&id), dir(user_dir).join(format!(".{id}.tmp")));
    let written = (|| {
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp)?;
        for (name, data) in &files {
            std::fs::write(tmp.join(name), data)?;
        }
        let _ = std::fs::remove_dir_all(&to);
        std::fs::rename(&tmp, &to)
    })();
    if let Err(e) = written {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(format!("cannot unpack it: {e}"));
    }
    Ok(Package { id, name: meta.name, library: meta.library_name })
}

pub fn remove(user_dir: &Path, id: &str) {
    if let Err(e) = std::fs::remove_dir_all(dir(user_dir).join(id)) {
        log::warn!("cannot remove the GPU driver {id}: {e}");
    }
}

/// A folder name for a package called `name`.
fn id(name: &str) -> String {
    let words: Vec<String> = name
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_ascii_lowercase)
        .collect();
    let mut id = words.join("-");
    id.truncate(48);
    if id.is_empty() { "driver".into() } else { id }
}

/// The wgpu instance, on the driver `chosen` names where there is one; the
/// setting goes back to the phone's driver, with a word for the user, when
/// it is gone, does not open or kept the last launch from showing a frame.
#[cfg(target_os = "android")]
pub fn instance(
    user_dir: &Path,
    chosen: &mut Option<String>,
    vulkan_driver: VulkanDriver,
) -> (wgpu::Instance, Option<String>) {
    // The Android ZIP importer installs the selected Turnip ICD in
    // getFilesDir()/turnip and records its library in driver_name. Use that
    // exact directory: it is deliberately separate from the legacy
    // user_dir/drivers/<id> package manager below.
    if vulkan_driver == VulkanDriver::Turnip {
        return turnip_instance();
    }
    let Some(id) = chosen.clone() else { return (glue5_gpu::instance(), None) };
    let mark = dir(user_dir).join(TRYING);
    let package = list(user_dir).into_iter().find(|p| p.id == id);
    let failed = |why: String| {
        log::warn!("gpu: the driver {id}: {why}");
        Some(why)
    };
    let why = match package {
        None => failed(format!("The GPU driver {id} is no longer there: the phone's is in use.")),
        Some(p) if mark.exists() => failed(format!(
            "The GPU driver {} stopped Glue 5 before its start page last time: the phone's is in use again.",
            p.name
        )),
        Some(p) => {
            let _ = std::fs::write(&mark, &p.id);
            match glue5_gpu::instance_with_driver(&dir(user_dir).join(&p.id), &p.library) {
                Ok(instance) => return (instance, None),
                Err(e) => failed(format!("The GPU driver {} did not open: {e:#}. The phone's is in use.", p.name)),
            }
        }
    };
    let _ = std::fs::remove_file(&mark);
    *chosen = None;
    (glue5_gpu::instance(), why)
}


/// The imported Android Turnip package, installed by Glue5Activity.
#[cfg(target_os = "android")]
fn turnip_instance() -> (wgpu::Instance, Option<String>) {
    let Some(app_files) = std::env::var_os("GLUE5_APP_FILES_DIR").map(PathBuf::from) else {
        let why = "Android app-private storage path is unavailable; using the system Vulkan driver.".to_owned();
        log::warn!("gpu: {why}");
        return (glue5_gpu::instance(), Some(why));
    };
    let dir = app_files.join("turnip");
    let status = app_files.join("turnip-status.txt");
    let mark = dir.join(TRYING);
    let report = |message: &str| {
        let _ = std::fs::write(&status, message);
    };
    let library = match std::fs::read_to_string(dir.join("driver_name")) {
        Ok(name) => name.trim().to_owned(),
        Err(e) => {
            let why = format!("Turnip is selected but no imported driver_name is available ({e}); using system Vulkan.");
            report(&why);
            log::warn!("gpu: {why}");
            return (glue5_gpu::instance(), Some(why));
        }
    };
    if library.is_empty() || Path::new(&library).file_name().and_then(|n| n.to_str()) != Some(library.as_str())
        || !library.ends_with(".so") || !dir.join(&library).is_file()
    {
        let why = format!("Turnip package is incomplete: invalid or missing driver library '{library}'; using system Vulkan.");
        report(&why);
        log::warn!("gpu: {why}");
        return (glue5_gpu::instance(), Some(why));
    }
    if mark.exists() {
        let why = "Turnip did not reach its first presented frame on the previous launch; using system Vulkan for recovery.".to_owned();
        let _ = std::fs::remove_file(&mark);
        report(&why);
        log::warn!("gpu: {why}");
        return (glue5_gpu::instance(), Some(why));
    }
    if let Err(e) = std::fs::write(&mark, b"turnip") {
        let why = format!("Cannot create Turnip crash-recovery marker ({e}); using system Vulkan.");
        report(&why);
        log::warn!("gpu: {why}");
        return (glue5_gpu::instance(), Some(why));
    }
    match glue5_gpu::instance_with_driver(&dir, &library) {
        Ok(instance) => {
            let message = format!("Turnip Vulkan driver loaded: {library}. Waiting for the first presented frame.");
            report(&message);
            log::info!("gpu: {message}");
            (instance, None)
        }
        Err(e) => {
            let _ = std::fs::remove_file(&mark);
            let why = format!("Turnip failed to load ({e:#}); using system Vulkan.");
            report(&why);
            log::warn!("gpu: {why}");
            (glue5_gpu::instance(), Some(why))
        }
    }
}

/// The imported Turnip driver opened, but wgpu could not create its surface/device.
#[cfg(target_os = "android")]
pub fn turnip_device_failed(user_dir: &Path, e: &anyhow::Error) -> String {
    let app_files = std::env::var_os("GLUE5_APP_FILES_DIR").map(PathBuf::from);
    if let Some(dir) = app_files {
        let turnip = dir.join("turnip");
        let _ = std::fs::remove_file(turnip.join(TRYING));
        let why = format!("Turnip loaded but could not open the GPU/surface ({e:#}); using system Vulkan.");
        let _ = std::fs::write(dir.join("turnip-status.txt"), &why);
        log::warn!("gpu: {why}");
        return why;
    }
    let why = format!("Turnip could not open the GPU/surface ({e:#}); using system Vulkan.");
    log::warn!("gpu: {why}");
    let _ = user_dir;
    why
}

/// The driver `chosen` names opened, but not its GPU (`e`): the setting goes
/// back to the phone's driver, for this launch too; the word for the user.
#[cfg(target_os = "android")]
pub fn device_failed(user_dir: &Path, chosen: &mut Option<String>, e: &anyhow::Error) -> String {
    let id = chosen.take().unwrap_or_default();
    let name = list(user_dir).into_iter().find(|p| p.id == id).map_or(id, |p| p.name);
    shown(user_dir);
    let why = format!("The GPU driver {name} could not open the GPU: {e:#}. The phone's is in use.");
    log::warn!("gpu: {why}");
    why
}

/// A frame was shown: the driver in use got the app going.
pub fn shown(user_dir: &Path) {
    let _ = std::fs::remove_file(dir(user_dir).join(TRYING));
    #[cfg(target_os = "android")]
    if let Some(app_files) = std::env::var_os("GLUE5_APP_FILES_DIR").map(PathBuf::from) {
        let mark = app_files.join("turnip").join(TRYING);
        if mark.exists() {
            let _ = std::fs::remove_file(&mark);
            let _ = std::fs::write(
                app_files.join("turnip-status.txt"),
                "Turnip active: first frame successfully presented.",
            );
            log::info!("gpu: Turnip reached its first presented frame");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{add, id, list, remove};

    fn zip(files: &[(&str, &[u8])]) -> Vec<u8> {
        glue5_mods::zip::write(&files.iter().map(|(n, d)| (n.to_string(), d.to_vec())).collect::<Vec<_>>())
    }

    /// A package (in a folder of its own or not) is unpacked flat, by its
    /// name; what is not one is refused.
    #[test]
    fn packages_are_added_listed_and_removed() {
        let user = std::env::temp_dir().join(format!("glue5-drivers-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&user);
        let meta = br#"{"schemaVersion":1,"name":"Turnip v26.0.0 R8","vendor":"Mesa","libraryName":"vulkan.ad07XX.so"}"#;
        let elf: &[u8] = b"\x7fELF\x02\x01\x01";
        let p = add(&user, &zip(&[("turnip/meta.json", meta), ("turnip/vulkan.ad07XX.so", elf)])).unwrap();
        assert_eq!((p.id.as_str(), p.library.as_str()), ("turnip-v26-0-0-r8", "vulkan.ad07XX.so"));
        assert_eq!(std::fs::read(user.join("drivers/turnip-v26-0-0-r8/vulkan.ad07XX.so")).unwrap(), elf);
        // The same name again replaces it.
        add(&user, &zip(&[("meta.json", meta), ("vulkan.ad07XX.so", elf)])).unwrap();
        assert_eq!(list(&user), vec![p.clone()]);
        assert!(add(&user, &zip(&[("meta.json", meta)])).is_err(), "no library");
        assert!(add(&user, &zip(&[("meta.json", meta), ("vulkan.ad07XX.so", b"text")])).is_err(), "not ELF");
        assert!(add(&user, &zip(&[("a/meta.json", meta), ("b/vulkan.ad07XX.so", elf)])).is_err(), "folders");
        assert!(add(&user, b"not a zip").is_err());
        remove(&user, &p.id);
        assert!(list(&user).is_empty());
        let _ = std::fs::remove_dir_all(&user);
    }

    #[test]
    fn ids_are_folder_names() {
        assert_eq!(id("Qualcomm Driver v840"), "qualcomm-driver-v840");
        assert_eq!(id("../.."), "driver");
    }
}
