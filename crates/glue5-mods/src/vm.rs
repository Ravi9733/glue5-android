//! A mod's Luau VM: sandboxed, with the natives as namespace tables, `wait`
//! and `require` (the mod's own files); its main chunk runs as a coroutine
//! the engine resumes once per wait. The REPL's VM (`eval`) is the same
//! with a loop that runs the snippets it is given.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

use mlua::{Function, Lua, LuaOptions, MultiValue, StdLib, Table, Thread, Value, VmState};

use crate::world::Pool;
use crate::storage::Storage;
use crate::{EvalResult, json, menus, natives};

/// How long a mod may run before it yields to the game for this frame.
const SLICE: Duration = Duration::from_millis(2);
/// How long a mod may run without calling `wait` before it is stopped.
const RUNAWAY: Duration = Duration::from_secs(2);
/// Memory a mod's VM may use.
const MEMORY: usize = 64 << 20;

/// Only one vehicle package extraction may run at a time. This prevents two
/// rapid UI taps from racing on the same install directory or status file.
static VEHICLE_INSTALL_RUNNING: AtomicBool = AtomicBool::new(false);

struct VehicleInstallGuard;
impl Drop for VehicleInstallGuard {
    fn drop(&mut self) {
        VEHICLE_INSTALL_RUNNING.store(false, Ordering::Release);
    }
}

/// Snippets for the REPL: code, and where its result goes.
pub type Jobs = Rc<RefCell<VecDeque<(String, Sender<EvalResult>)>>>;

pub struct Vm {
    lua: Lua,
    module: String,
    /// The main chunk, until it ends, and when it is due (game time, ms).
    main: Option<Thread>,
    main_wake: u64,
    main_busy: Duration,
    /// Tasks: `task.spawn`/`task.delay`, menus' callbacks.
    sched: Sched,
    deadline: Rc<Cell<Instant>>,
    /// The mod's `storage` (none for the REPL).
    storage: Option<Rc<Storage>>,
}

/// What `task.*` and the menus share with the VM.
#[derive(Clone, Default)]
struct Sched {
    tasks: Rc<RefCell<Vec<Task>>>,
    /// The game time of the current turn, ms.
    now: Rc<Cell<u64>>,
    handlers: menus::Handlers,
}

struct Task {
    thread: Thread,
    wake: u64,
    /// What it starts with (a callback's arguments), the first time.
    args: Option<MultiValue>,
    busy: Duration,
}

/// What a turn of a mod came to.
pub enum Step {
    /// The next thing it does is this many milliseconds away (0: the next
    /// frame).
    Wait(u32),
    Finished,
    Failed(String),
}

/// What a coroutine's run came to.
enum Outcome {
    Wait(u32),
    Preempted,
    Finished,
    Failed(String),
}

impl Vm {
    /// Loads mod `name`'s `main.luau` from `dir`; `log` gets what it prints.
    pub fn new(name: &str, dir: &Path, log: Rc<dyn Fn(&str)>) -> mlua::Result<Vm> {
        let sched = Sched::default();
        let (lua, deadline) = base(name, log, &sched)?;
        lua.globals().set("require", require(&lua, name, dir)?)?;
        // Data-driven placement manifests are read from this mod's own
        // directory. The parser bounds file size, placement count, model
        // names and coordinates before any script can use the data.
        let world: Table = lua.globals().get("world")?;
        world.set(
            "requestStuntRaceChallengePage",
            lua.create_function(move |_, ()| {
                let app_files = std::env::var_os("GLUE5_APP_FILES_DIR")
                    .map(PathBuf::from)
                    .ok_or_else(|| mlua::Error::runtime("Android browser launch is unavailable on this platform"))?;
                std::fs::create_dir_all(&app_files)
                    .map_err(|e| mlua::Error::runtime(format!("cannot access app files: {e}")))?;
                std::fs::write(app_files.join("open-stunt-race-page-request"), b"open")
                    .map_err(|e| mlua::Error::runtime(format!("cannot request browser: {e}")))?;
                Ok(())
            })?,
        )?;
        let vehicle_app_files = std::env::var_os("GLUE5_APP_FILES_DIR").map(PathBuf::from);
        world.set(
            "requestVehicleCatalog",
            lua.create_function(move |_, ()| {
                let app_files = vehicle_app_files.as_ref()
                    .ok_or_else(|| mlua::Error::runtime("Android catalog service is unavailable on this platform"))?;
                std::fs::create_dir_all(app_files)
                    .map_err(|e| mlua::Error::runtime(format!("cannot access app files: {e}")))?;
                let _ = std::fs::remove_file(app_files.join("vehicle-catalog-status.json"));
                std::fs::write(app_files.join("vehicle-catalog-fetch-request"), b"fetch")
                    .map_err(|e| mlua::Error::runtime(format!("cannot request vehicle catalog: {e}")))?;
                Ok(())
            })?,
        )?;
        let vehicle_status_dir = std::env::var_os("GLUE5_APP_FILES_DIR").map(PathBuf::from);
        world.set(
            "vehicleCatalogStatus",
            lua.create_function(move |_, ()| {
                let Some(dir) = &vehicle_status_dir else { return Ok((false, false, "Catalog service unavailable".to_string())); };
                let path = dir.join("vehicle-catalog-status.json");
                let Ok(text) = std::fs::read_to_string(path) else { return Ok((false, false, "pending".to_string())); };
                let value: serde_json::Value = serde_json::from_str(&text)
                    .map_err(|e| mlua::Error::runtime(format!("invalid catalog status: {e}")))?;
                let ok = value.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
                let message = value.get("message").and_then(|v| v.as_str()).unwrap_or("Catalog request finished").to_string();
                Ok((true, ok, message))
            })?,
        )?;
        let vehicle_catalog_dir = std::env::var_os("GLUE5_APP_FILES_DIR").map(PathBuf::from);
        world.set(
            "vehicleCatalogJson",
            lua.create_function(move |_, ()| {
                let Some(dir) = &vehicle_catalog_dir else { return Ok(None::<String>); };
                match std::fs::read_to_string(dir.join("vehicle-catalog.json")) {
                    Ok(text) if text.len() <= 2 * 1024 * 1024 => Ok(Some(text)),
                    Ok(_) => Err(mlua::Error::runtime("cached vehicle catalog exceeds 2 MiB")),
                    Err(_) => Ok(None),
                }
            })?,
        )?;
        let vehicle_package_request_dir = std::env::var_os("GLUE5_APP_FILES_DIR").map(PathBuf::from);
        world.set(
            "requestVehiclePackageDownload",
            lua.create_function(move |_, id: String| {
                let app_files = vehicle_package_request_dir.as_ref()
                    .ok_or_else(|| mlua::Error::runtime("Android package downloader is unavailable on this platform"))?;
                let catalog_path = app_files.join("vehicle-catalog.json");
                let catalog_bytes = std::fs::read(&catalog_path)
                    .map_err(|e| mlua::Error::runtime(format!("refresh the online catalog first: {e}")))?;
                let catalog = crate::vehicle_hub::Catalog::parse(&catalog_bytes)
                    .map_err(|e| mlua::Error::runtime(format!("catalog validation failed: {e}")))?;
                let package = catalog.vehicles.iter().find(|p| p.id == id)
                    .ok_or_else(|| mlua::Error::runtime("vehicle package is not in the validated catalog"))?;
                let request = serde_json::json!({
                    "id": package.id,
                    "download_url": package.download_url,
                    "sha256": package.sha256,
                    "size_bytes": package.size_bytes
                });
                std::fs::create_dir_all(app_files)
                    .map_err(|e| mlua::Error::runtime(format!("cannot access app files: {e}")))?;
                std::fs::write(app_files.join("vehicle-package-status.json"), br#"{"state":"pending","message":"Waiting for downloader"}"#)
                    .map_err(|e| mlua::Error::runtime(format!("cannot reset download status: {e}")))?;
                std::fs::write(app_files.join("vehicle-package-download-request"), request.to_string())
                    .map_err(|e| mlua::Error::runtime(format!("cannot queue package download: {e}")))?;
                Ok(())
            })?,
        )?;
        let package_status_dir = std::env::var_os("GLUE5_APP_FILES_DIR").map(PathBuf::from);
        world.set(
            "vehiclePackageDownloadStatus",
            lua.create_function(move |_, ()| {
                let Some(dir) = &package_status_dir else { return Ok((false, "Downloader unavailable".to_string())); };
                let path = dir.join("vehicle-package-status.json");
                let Ok(text) = std::fs::read_to_string(path) else { return Ok((false, "pending".to_string())); };
                let value: serde_json::Value = serde_json::from_str(&text)
                    .map_err(|e| mlua::Error::runtime(format!("invalid package status: {e}")))?;
                let done = value.get("state").and_then(|v| v.as_str()) == Some("complete")
                    || value.get("state").and_then(|v| v.as_str()) == Some("error");
                let message = value.get("message").and_then(|v| v.as_str()).unwrap_or("Downloading").to_string();
                Ok((done, message))
            })?,
        )?;
        let vehicle_install_request_dir = std::env::var_os("GLUE5_APP_FILES_DIR").map(PathBuf::from);
        world.set(
            "requestVehiclePackageInstall",
            lua.create_function(move |_, id: String| {
                let app_files = vehicle_install_request_dir.as_ref()
                    .ok_or_else(|| mlua::Error::runtime("Android vehicle installer is unavailable on this platform"))?
                    .clone();
                let catalog_bytes = std::fs::read(app_files.join("vehicle-catalog.json"))
                    .map_err(|e| mlua::Error::runtime(format!("refresh catalog first: {e}")))?;
                let catalog = crate::vehicle_hub::Catalog::parse(&catalog_bytes)
                    .map_err(|e| mlua::Error::runtime(format!("catalog validation failed: {e}")))?;
                let package = catalog.vehicles.into_iter().find(|p| p.id == id)
                    .ok_or_else(|| mlua::Error::runtime("vehicle package is not in the validated catalog"))?;
                let archive = app_files.join("vehicles/downloads").join(format!("{}.zip", package.id));
                if !archive.is_file() {
                    return Err(mlua::Error::runtime("download this vehicle package first"));
                }
                VEHICLE_INSTALL_RUNNING.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                    .map_err(|_| mlua::Error::runtime("another vehicle package is already being installed; wait for it to finish"))?;
                let install_root = app_files.join("vehicles/installed");
                let status = app_files.join("vehicle-install-status.json");
                if let Err(error) = std::fs::write(&status, br#"{"state":"running","ok":false,"message":"Preparing safe install"}"#) {
                    VEHICLE_INSTALL_RUNNING.store(false, Ordering::Release);
                    return Err(mlua::Error::runtime(format!("cannot write install status: {error}")));
                }
                let spawn = std::thread::Builder::new().name("Glue5VehicleInstall".into()).spawn(move || {
                    let _guard = VehicleInstallGuard;
                    let outcome = crate::vehicle_manager::install_verified_package(&archive, &install_root, &package);
                    let (state, ok, message) = match outcome {
                        Ok(result) => ("complete", true, format!(
                            "Verified and extracted {}: {} files, {} bytes. Static checks passed; native spawning is not enabled yet.",
                            result.id, result.extracted_files, result.extracted_bytes
                        )),
                        Err(error) => ("error", false, error),
                    };
                    let value = serde_json::json!({"state":state,"ok":ok,"message":message});
                    let _ = std::fs::write(status, value.to_string());
                });
                if let Err(error) = spawn {
                    VEHICLE_INSTALL_RUNNING.store(false, Ordering::Release);
                    return Err(mlua::Error::runtime(format!("cannot start installer: {error}")));
                }
                Ok(())
            })?,
        )?;
        let vehicle_install_status_dir = std::env::var_os("GLUE5_APP_FILES_DIR").map(PathBuf::from);
        world.set(
            "vehiclePackageInstallStatus",
            lua.create_function(move |_, ()| {
                let Some(dir) = &vehicle_install_status_dir else {
                    return Ok((false, false, "Installer unavailable".to_string()));
                };
                let path = dir.join("vehicle-install-status.json");
                let Ok(text) = std::fs::read_to_string(path) else {
                    return Ok((false, false, "not started".to_string()));
                };
                let value: serde_json::Value = serde_json::from_str(&text)
                    .map_err(|e| mlua::Error::runtime(format!("invalid install status: {e}")))?;
                let state = value.get("state").and_then(|v| v.as_str()).unwrap_or("running");
                let done = state == "complete" || state == "error";
                let ok = value.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
                let message = value.get("message").and_then(|v| v.as_str()).unwrap_or("Installing").to_string();
                Ok((done, ok, message))
            })?,
        )?;
        let placement_dir = dir.to_path_buf();
        let map_dir = placement_dir.join("maps");
        let import_dir = map_dir.clone();
        world.set(
            "requestPlacementImport",
            lua.create_function(move |_, ()| {
                let app_files = std::env::var_os("GLUE5_APP_FILES_DIR")
                    .map(PathBuf::from)
                    .ok_or_else(|| mlua::Error::runtime("Android import picker is unavailable on this platform"))?;
                std::fs::create_dir_all(&import_dir)
                    .map_err(|e| mlua::Error::runtime(format!("cannot create stunt maps folder: {e}")))?;
                std::fs::create_dir_all(&app_files)
                    .map_err(|e| mlua::Error::runtime(format!("cannot access app files: {e}")))?;
                let request = app_files.join("stunt-race-import-request");
                std::fs::write(&request, import_dir.to_string_lossy().as_bytes())
                    .map_err(|e| mlua::Error::runtime(format!("cannot request Android file picker: {e}")))?;
                Ok(())
            })?,
        )?;
        let race_import_dir = map_dir.join("routes");
        world.set(
            "requestRaceImport",
            lua.create_function(move |_, ()| {
                let app_files = std::env::var_os("GLUE5_APP_FILES_DIR")
                    .map(PathBuf::from)
                    .ok_or_else(|| mlua::Error::runtime("Android import picker is unavailable on this platform"))?;
                std::fs::create_dir_all(&race_import_dir)
                    .map_err(|e| mlua::Error::runtime(format!("cannot create race routes folder: {e}")))?;
                std::fs::create_dir_all(&app_files)
                    .map_err(|e| mlua::Error::runtime(format!("cannot access app files: {e}")))?;
                let request = app_files.join("stunt-race-import-request");
                std::fs::write(&request, race_import_dir.to_string_lossy().as_bytes())
                    .map_err(|e| mlua::Error::runtime(format!("cannot request race file picker: {e}")))?;
                Ok(())
            })?,
        )?;
        world.set(
            "listPlacementFiles",
            lua.create_function(move |_, ()| {
                let _ = std::fs::create_dir_all(&map_dir);
                let mut names = std::fs::read_dir(&map_dir)
                    .ok()
                    .into_iter()
                    .flatten()
                    .filter_map(Result::ok)
                    .filter(|entry| entry.path().is_file())
                    .filter_map(|entry| {
                        let path = entry.path();
                        let ext = path.extension()?.to_str()?;
                        if !ext.eq_ignore_ascii_case("xml") {
                            return None;
                        }
                        let name = path.file_name()?.to_str()?.to_string();
                        if name.len() <= 80
                            && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-' || b == b'.')
                        {
                            Some(name)
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>();
                names.sort_by_key(|name| name.to_ascii_lowercase());
                names.truncate(40);
                Ok(names)
            })?,
        )?;
        let race_route_dir = dir.to_path_buf().join("maps").join("routes");
        world.set(
            "listRaceRoutes",
            lua.create_function(move |_, ()| {
                let _ = std::fs::create_dir_all(&race_route_dir);
                let mut routes = std::fs::read_dir(&race_route_dir)
                    .ok()
                    .into_iter()
                    .flatten()
                    .filter_map(Result::ok)
                    .filter(|entry| entry.path().is_file())
                    .filter_map(|entry| {
                        let path = entry.path();
                        let extension = path.extension()?.to_str()?;
                        let is_xml = extension.eq_ignore_ascii_case("xml");
                        let is_json = extension.eq_ignore_ascii_case("json");
                        if !is_xml && !is_json {
                            return None;
                        }
                        let metadata = std::fs::metadata(&path).ok()?;
                        let limit = if is_json { 8 * 1024 * 1024 } else { 4 * 1024 * 1024 };
                        if metadata.len() > limit {
                            return None;
                        }
                        let source = std::fs::read_to_string(&path).ok()?;
                        if is_json {
                            crate::placements::parse_ugc_race_checkpoints(&source).ok()?;
                        } else {
                            crate::placements::parse_xml_checkpoints(&source).ok()?;
                        }
                        let name = path.file_name()?.to_str()?.to_string();
                        (name.len() <= 80 && name.bytes().all(|b|
                            b.is_ascii_alphanumeric() || b == b'_' || b == b'-' || b == b'.'
                        )).then_some(name)
                    })
                    .collect::<Vec<_>>();
                routes.sort_by_key(|name| name.to_ascii_lowercase());
                routes.truncate(40);
                Ok(routes)
            })?,
        )?;
        let delete_map_dir = dir.to_path_buf().join("maps");
        world.set(
            "deletePlacementFile",
            lua.create_function(move |_, name: String| {
                if name.len() > 80
                    || !name.to_ascii_lowercase().ends_with(".xml")
                    || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-' || b == b'.')
                    || name.contains("..")
                {
                    return Err(mlua::Error::runtime("deletePlacementFile: invalid XML filename"));
                }
                let path = delete_map_dir.join(&name);
                if !path.is_file() {
                    return Err(mlua::Error::runtime(format!("map not found: {name}")));
                }
                std::fs::remove_file(&path)
                    .map_err(|e| mlua::Error::runtime(format!("cannot delete {name}: {e}")))?;
                Ok(())
            })?,
        )?;
        let route_dir = dir.to_path_buf().join("maps").join("routes");
        let delete_route_dir = route_dir.clone();
        let save_route_dir = route_dir.clone();
        world.set(
            "deleteRaceRoute",
            lua.create_function(move |_, name: String| {
                if name.len() > 80
                    || !(name.to_ascii_lowercase().ends_with(".xml") || name.to_ascii_lowercase().ends_with(".json"))
                    || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-' || b == b'.')
                    || name.contains("..")
                {
                    return Err(mlua::Error::runtime("deleteRaceRoute: invalid route filename"));
                }
                let path = delete_route_dir.join(&name);
                if !path.is_file() {
                    return Err(mlua::Error::runtime(format!("race route not found: {name}")));
                }
                std::fs::remove_file(&path)
                    .map_err(|e| mlua::Error::runtime(format!("cannot delete race route {name}: {e}")))?;
                Ok(())
            })?,
        )?;
        world.set(
            "saveRaceRoute",
            lua.create_function(move |_, (name, points): (String, Table)| {
                if name.is_empty() || name.len() > 64
                    || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
                {
                    return Err(mlua::Error::runtime("saveRaceRoute: name must use 1-64 letters, digits, _ or -"));
                }
                let points = points.sequence_values::<mlua::Vector>()
                    .collect::<mlua::Result<Vec<_>>>()?;
                if points.len() < 2 || points.len() > 128 {
                    return Err(mlua::Error::runtime("saveRaceRoute: route needs 2-128 checkpoints"));
                }
                let mut xml = String::from("<RaceRoute>\n");
                for point in points {
                    let (x, y, z) = (point.x(), point.y(), point.z());
                    if !x.is_finite() || !y.is_finite() || !z.is_finite()
                        || x.abs() > 20_000.0 || y.abs() > 20_000.0 || z.abs() > 20_000.0
                    {
                        return Err(mlua::Error::runtime("saveRaceRoute: checkpoint coordinates are outside allowed bounds"));
                    }
                    xml.push_str(&format!(
                        "  <Checkpoint><Position><X>{x:.3}</X><Y>{y:.3}</Y><Z>{z:.3}</Z></Position></Checkpoint>\n"
                    ));
                }
                xml.push_str("</RaceRoute>\n");
                std::fs::create_dir_all(&save_route_dir)
                    .map_err(|e| mlua::Error::runtime(format!("saveRaceRoute: {e}")))?;
                let path = save_route_dir.join(format!("{name}.xml"));
                use std::io::Write as _;
                let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(&path)
                    .map_err(|e| mlua::Error::runtime(format!("saveRaceRoute: {e} (existing route is never overwritten)")))?;
                file.write_all(xml.as_bytes())
                    .map_err(|e| mlua::Error::runtime(format!("saveRaceRoute: {e}")))?;
                Ok(format!("{name}.xml"))
            })?,
        )?;
        world.set(
            "loadRaceRoute",
            lua.create_function(move |lua, relative: String| {
                use std::path::Component;
                let path = Path::new(&relative);
                if relative.is_empty()
                    || relative.len() > 128
                    || path.is_absolute()
                    || !path.components().all(|c| matches!(c, Component::Normal(_)))
                    || !(relative.to_ascii_lowercase().ends_with(".xml")
                        || relative.to_ascii_lowercase().ends_with(".json"))
                {
                    return Err(mlua::Error::runtime("world.loadRaceRoute: expected a relative XML route or Rockstar UGC JSON inside maps/routes/"));
                }
                let file = route_dir.join(path.file_name().ok_or_else(|| mlua::Error::runtime("invalid route filename"))?);
                let metadata = std::fs::metadata(&file)
                    .map_err(|e| mlua::Error::runtime(format!("world.loadRaceRoute: {e}")))?;
                let is_json = file.extension().and_then(|ext| ext.to_str())
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("json"));
                let limit = if is_json { 8 * 1024 * 1024 } else { 4 * 1024 * 1024 };
                if metadata.len() > limit {
                    return Err(mlua::Error::runtime(format!("world.loadRaceRoute: route file exceeds {} MiB", limit / 1024 / 1024)));
                }
                let source = std::fs::read_to_string(&file)
                    .map_err(|e| mlua::Error::runtime(format!("world.loadRaceRoute: {e}")))?;
                let points = if is_json {
                    crate::placements::parse_ugc_race_checkpoints(&source)
                } else {
                    crate::placements::parse_xml_checkpoints(&source)
                }.map_err(|e| mlua::Error::runtime(format!("world.loadRaceRoute: {e}")))?;
                let result = lua.create_table()?;
                for (index, point) in points.iter().enumerate() {
                    result.set(index + 1, mlua::Vector::new(point[0], point[1], point[2]))?;
                }
                Ok(result)
            })?,
        )?;
        world.set(
            "loadPlacements",
            lua.create_function(move |lua, relative: String| {
                use std::path::Component;
                let path = Path::new(&relative);
                if relative.is_empty()
                    || relative.len() > 128
                    || path.is_absolute()
                    || !path.components().all(|c| matches!(c, Component::Normal(_)))
                    || !(relative.ends_with(".json") || relative.ends_with(".xml"))
                {
                    return Err(mlua::Error::runtime(
                        "world.loadPlacements: expected a relative .json or .xml file inside this mod",
                    ));
                }
                let file = placement_dir.join(path);
                let metadata = std::fs::metadata(&file).map_err(|e| {
                    mlua::Error::runtime(format!("world.loadPlacements: {e}"))
                })?;
                let limit = if relative.ends_with(".xml") { 4 * 1024 * 1024 }
                    else if relative.starts_with("maps/routes/") { 8 * 1024 * 1024 }
                    else { 256 * 1024 };
                if metadata.len() > limit {
                    return Err(mlua::Error::runtime(format!(
                        "world.loadPlacements: file exceeds {} KiB",
                        limit / 1024
                    )));
                }
                let source = std::fs::read_to_string(&file).map_err(|e| {
                    mlua::Error::runtime(format!("world.loadPlacements: {e}"))
                })?;
                let set = if relative.ends_with(".xml") {
                    crate::placements::parse_xml_placements_with_hashes(&source, crate::world::resolve_model_hash)
                } else {
                    match crate::placements::parse_manifest(&source) {
                        Ok(manifest) => Ok(manifest),
                        Err(_) => crate::placements::parse_ugc_race_placements(
                            &source, crate::world::resolve_model_hash
                        ),
                    }
                }.map_err(|e| mlua::Error::runtime(format!("world.loadPlacements: {e}")))?;
                let result = lua.create_table()?;
                for (index, placement) in set.placements.iter().enumerate() {
                    let item = lua.create_table()?;
                    item.set("model", placement.model.as_str())?;
                    item.set("frozen", placement.frozen)?;
                    item.set(
                        "position",
                        mlua::Vector::new(
                            placement.position[0],
                            placement.position[1],
                            placement.position[2],
                        ),
                    )?;
                    item.set(
                        "rotation",
                        mlua::Vector::new(
                            placement.rotation[0],
                            placement.rotation[1],
                            placement.rotation[2],
                        ),
                    )?;
                    result.set(index + 1, item)?;
                }
                Ok(result)
            })?,
        )?;
        // Spawn a validated placement set on the mod's own coroutine. The
        // model load may yield while streaming, so keep this in Luau rather
        // than calling yielding helpers from a Rust callback.
        let spawn: Function = lua
            .load(
                r#"
                local loadPlacements = world.loadPlacements
                local spawnedObjects = {}
                local spawnGeneration = 0
                world.clearSpawnedPlacements = function()
                    spawnGeneration += 1
                    local removed = 0
                    for i = #spawnedObjects, 1, -1 do
                        local handle = spawnedObjects[i]
                        if entity.doesEntityExist(handle) then
                            object.deleteObject(handle)
                            removed += 1
                        end
                        spawnedObjects[i] = nil
                    end
                    return removed
                end
                return function(relative)
                    assert(type(relative) == "string", "world.spawnPlacements: expected a relative .json or .xml path")
                    local placements = loadPlacements(relative)
                    assert(#spawnedObjects + #placements <= 2048, "world.spawnPlacements: clear some spawned track objects first (2048 object session limit)")
                    local generation = spawnGeneration
                    local taskThread = task.spawn(function()
                        local created, missing = 0, 0
                        for _, item in placements do
                            if generation ~= spawnGeneration then
                                break
                            end
                            if streaming.load(item.model) then
                                if generation ~= spawnGeneration then
                                    streaming.setModelAsNoLongerNeeded(item.model)
                                    break
                                end
                                local objectHandle = object.createObject(item.model, item.position, false, false, false)
                                streaming.setModelAsNoLongerNeeded(item.model)
                                if objectHandle then
                                    entity.setEntityRotation(objectHandle, item.rotation, 2, true)
                                    -- Keep map props alive until the player's explicit Clear action.
                                    pcall(function()
                                        entity.setEntityAsMissionEntity(objectHandle, true, true)
                                    end)
                                    -- Static Creator props are frozen; dprop keeps physics.
                                    if item.frozen ~= false then
                                        entity.freezeEntityPosition(objectHandle, true)
                                    end
                                    table.insert(spawnedObjects, objectHandle)
                                    created += 1
                                else
                                    missing += 1
                                end
                            else
                                missing += 1
                            end
                            wait()
                        end
                        ui.notify("Track objects: " .. tostring(created) .. " spawned, " .. tostring(missing) .. " unavailable")
                    end)
                    return taskThread
                end
                "#,
            )
            .eval()?;
        world.set("spawnPlacements", spawn)?;
        let storage = Storage::open(name);
        storage.install(&lua)?;
        // The mod's code starts once the player is in the game (not on
        // the loading screens, where the player's ped is a stand-in).
        let ready: Function = lua
            .load(
                "local yield = coroutine.yield\n\
                 return function(main) return function(...)\n\
                 while dlc.getIsLoadingScreenActive() or not camera.isScreenFadedIn() \
                 or not player.isPlayerPlaying(player.playerId()) do yield(0) end\n\
                 return main(...)\n\
                 end end",
            )
            .set_name("=ready")
            .eval()?;
        lua.sandbox(true)?;
        let path = dir.join("main.luau");
        let source = std::fs::read_to_string(&path)
            .map_err(|e| mlua::Error::runtime(format!("reading {}: {e}", path.display())))?;
        let chunk = lua.load(source).set_name(format!("@{name}/main.luau")).into_function()?;
        let main = lua.create_thread(ready.call::<Function>(chunk)?)?;
        Ok(Vm {
            lua,
            module: name.to_string(),
            main: Some(main),
            main_wake: 0,
            main_busy: Duration::ZERO,
            sched,
            deadline,
            storage: Some(storage),
        })
    }

    /// The REPL: runs `jobs`' snippets, as expressions when they are.
    pub fn repl(jobs: Jobs, log: Rc<dyn Fn(&str)>) -> mlua::Result<Vm> {
        let sched = Sched::default();
        let (lua, deadline) = base("eval", log, &sched)?;
        let waiting: Rc<RefCell<HashMap<i64, Sender<EvalResult>>>> = Rc::default();
        let next = Cell::new(0i64);
        let pending = waiting.clone();
        let take = lua.create_function(move |lua, ()| {
            let Some((code, reply)) = jobs.borrow_mut().pop_front() else { return Ok(MultiValue::new()) };
            let id = next.get() + 1;
            next.set(id);
            let compiled = lua
                .load(format!("return {code}"))
                .set_name("=eval")
                .into_function()
                .or_else(|_| lua.load(code.as_str()).set_name("=eval").into_function());
            Ok(match compiled {
                Ok(f) => {
                    pending.borrow_mut().insert(id, reply);
                    MultiValue::from_vec(vec![Value::Integer(id), Value::Function(f)])
                }
                Err(e) => {
                    let _ = reply.send(Err(error_text(&e)));
                    MultiValue::new()
                }
            })
        })?;
        let reply = lua.create_function(move |_, args: MultiValue| {
            let mut args = args.into_iter();
            let id = match args.next() {
                Some(Value::Integer(id)) => id,
                _ => return Ok(()),
            };
            let ok = matches!(args.next(), Some(Value::Boolean(true)));
            let values: Vec<Value> = args.collect();
            if let Some(sender) = waiting.borrow_mut().remove(&id) {
                let _ = sender.send(if ok {
                    Ok(values.iter().map(describe).collect::<Vec<_>>().join(", "))
                } else {
                    Err(values.first().map_or_else(|| "error".into(), describe_plain))
                });
            }
            Ok(())
        })?;
        let main: Function = lua
            .load(
                "local take, reply = ...\n\
                 return function()\n\
                 while true do\n\
                 local id, f = take()\n\
                 if id then reply(id, pcall(f)) else wait() end\n\
                 end end",
            )
            .set_name("=repl")
            .call((take, reply))?;
        lua.sandbox(true)?;
        let main = lua.create_thread(main)?;
        Ok(Vm { lua, module: "~eval".into(), main: Some(main), main_wake: 0, main_busy: Duration::ZERO, sched, deadline, storage: None })
    }

    /// A turn at game time `now` (ms): what the player did with the menus
    /// becomes tasks; the tasks and the main chunk that are due run, until
    /// they wait, end, fail or run out of time.
    pub fn step(&mut self, now: u64) -> Step {
        self.sched.now.set(now);
        if let Some(storage) = &self.storage {
            storage.flush(false);
        }
        for (item, input) in menus::take_inputs(&self.module) {
            match menus::apply(&self.lua, &self.sched.handlers, item, input) {
                Ok(Some((f, args))) => match self.lua.create_thread(f) {
                    Ok(thread) => self.sched.tasks.borrow_mut().push(Task { thread, wake: now, args: Some(args), busy: Duration::ZERO }),
                    Err(e) => crate::log_line(&self.module, crate::LogLevel::Error, &error_text(&e)),
                },
                Ok(None) => {}
                Err(e) => crate::log_line(&self.module, crate::LogLevel::Error, &error_text(&e)),
            }
        }
        // The tasks that are due; others may be added while they run.
        let due: Vec<Task> = {
            let mut tasks = self.sched.tasks.borrow_mut();
            let (due, later): (Vec<Task>, Vec<Task>) = tasks.drain(..).partition(|t| t.wake <= now);
            *tasks = later;
            due
        };
        for mut task in due {
            let args = task.args.take().unwrap_or_default();
            let (outcome, spent) = self.run(&task.thread, args);
            task.busy += spent;
            match outcome {
                Outcome::Wait(ms) => {
                    task.wake = now + u64::from(ms);
                    task.busy = Duration::ZERO;
                }
                Outcome::Preempted if task.busy >= RUNAWAY => {
                    crate::log_line(&self.module, crate::LogLevel::Error, &runaway());
                    continue;
                }
                Outcome::Preempted => task.wake = now,
                Outcome::Finished => continue,
                Outcome::Failed(e) => {
                    // A task's error is logged; the mod goes on.
                    crate::log_line(&self.module, crate::LogLevel::Error, &e);
                    continue;
                }
            }
            self.sched.tasks.borrow_mut().push(task);
        }
        if let Some(main) = self.main.clone()
            && self.main_wake <= now
        {
            let (outcome, spent) = self.run(&main, MultiValue::new());
            self.main_busy += spent;
            match outcome {
                Outcome::Wait(ms) => {
                    self.main_wake = now + u64::from(ms);
                    self.main_busy = Duration::ZERO;
                }
                Outcome::Preempted if self.main_busy >= RUNAWAY => return Step::Failed(runaway()),
                Outcome::Preempted => self.main_wake = now,
                Outcome::Finished => self.main = None,
                Outcome::Failed(e) => return Step::Failed(e),
            }
        }
        let menus = menus::has_menus(&self.module);
        let tasks = self.sched.tasks.borrow();
        if self.main.is_none() && tasks.is_empty() && !menus {
            return Step::Finished;
        }
        // A menu may be used any time: every frame then.
        if menus {
            return Step::Wait(0);
        }
        let next = tasks.iter().map(|t| t.wake).chain(self.main.as_ref().map(|_| self.main_wake)).min().unwrap_or(now);
        Step::Wait(next.saturating_sub(now).min(u64::from(u32::MAX)) as u32)
    }

    /// Runs a coroutine until it yields, ends or fails; and how long it ran.
    fn run(&self, thread: &Thread, args: MultiValue) -> (Outcome, Duration) {
        if !thread.is_resumable() {
            return (Outcome::Finished, Duration::ZERO);
        }
        let start = Instant::now();
        self.deadline.set(start + SLICE);
        let result = thread.resume::<MultiValue>(args);
        let spent = start.elapsed();
        let outcome = match result {
            Err(e) => Outcome::Failed(error_text(&e)),
            Ok(_) if !thread.is_resumable() => Outcome::Finished,
            // `wait(ms)`.
            Ok(values) => match values.into_iter().next() {
                Some(v) => Outcome::Wait(match v {
                    Value::Integer(i) => i.clamp(0, i32::MAX.into()) as u32,
                    Value::Number(n) if n.is_finite() => n.clamp(0.0, i32::MAX as f64) as u32,
                    _ => 0,
                }),
                // The interrupt: out of time.
                None => Outcome::Preempted,
            },
        };
        (outcome, spent)
    }

    /// Memory the VM uses, in bytes.
    pub fn memory(&self) -> usize {
        self.lua.used_memory()
    }
}

fn runaway() -> String {
    format!("ran for {} s without calling wait(): a loop needs wait() to let the game go on", RUNAWAY.as_secs())
}

/// A VM with the libraries, the natives and the mods' globals (`print`,
/// `wait`, `task`, `ui`, `joaat`, `mod`), and the interrupt that keeps a
/// slice short; not sandboxed yet.
fn base(name: &str, log: Rc<dyn Fn(&str)>, sched: &Sched) -> mlua::Result<(Lua, Rc<Cell<Instant>>)> {
    let lua = Lua::new_with(
        StdLib::COROUTINE
            | StdLib::TABLE
            | StdLib::STRING
            | StdLib::UTF8
            | StdLib::MATH
            | StdLib::BIT
            | StdLib::VECTOR
            | StdLib::BUFFER
            | StdLib::OS,
        LuaOptions::default(),
    )?;
    lua.set_memory_limit(MEMORY)?;
    let g = lua.globals();
    g.set(
        "print",
        lua.create_function(move |_, args: MultiValue| {
            log(&args.iter().map(describe_plain).collect::<Vec<_>>().join(" "));
            Ok(())
        })?,
    )?;
    // `wait(ms)`: back to the game until then; plain Lua, so it yields from
    // the running coroutine.
    let wait: Function = lua
        .load("local yield = coroutine.yield\nreturn function(ms) yield(ms or 0) end")
        .set_name("=wait")
        .eval()?;
    g.set("wait", wait.clone())?;
    // `task.spawn(f, ...)`, `task.delay(ms, f, ...)`, `task.wait(ms)`.
    let task = lua.create_table()?;
    task.set("wait", wait)?;
    let s = sched.clone();
    task.set(
        "spawn",
        lua.create_function(move |lua, (f, args): (Function, MultiValue)| {
            let thread = lua.create_thread(f)?;
            s.tasks.borrow_mut().push(Task { thread, wake: s.now.get(), args: Some(args), busy: Duration::ZERO });
            Ok(())
        })?,
    )?;
    let s = sched.clone();
    task.set(
        "delay",
        lua.create_function(move |lua, (ms, f, args): (f64, Function, MultiValue)| {
            let thread = lua.create_thread(f)?;
            let wake = s.now.get() + ms.clamp(0.0, 1e9) as u64;
            s.tasks.borrow_mut().push(Task { thread, wake, args: Some(args), busy: Duration::ZERO });
            Ok(())
        })?,
    )?;
    menus::install(&lua, name, sched.handlers.clone())?;
    json::install(&lua)?;
    // `world.vehicles()`..., the catalog's lists.
    let world = lua.create_table()?;
    for (key, pool) in [("vehicles", Pool::Vehicles), ("peds", Pool::Peds), ("objects", Pool::Objects)] {
        world.set(
            key,
            lua.create_function(move |_, max: Option<usize>| {
                let env = natives::env().ok_or_else(|| mlua::Error::runtime(format!("world.{key}: only in a mod's script")))?;
                Ok(unsafe { crate::world::handles(env, pool, max.unwrap_or(1024).min(4096)) })
            })?,
        )?;
    }
    world.set("vehicleModels", lua.create_function(|_, ()| Ok(crate::world::catalog().map(|c| c.vehicles).unwrap_or_default()))?)?;
    world.set("pedModels", lua.create_function(|_, ()| Ok(crate::world::catalog().map(|c| c.peds).unwrap_or_default()))?)?;
    world.set("weapons", lua.create_function(|_, ()| Ok(crate::world::catalog().map(|c| c.weapons).unwrap_or_default()))?)?;
    g.set("world", world)?;
    g.set("joaat", lua.create_function(|_, s: mlua::LuaString| Ok(natives::joaat(&s.to_str()?)))?)?;
    let m = lua.create_table()?;
    m.set("name", name)?;
    g.set("mod", m)?;

    // A table per namespace; a native's function is made the first time it
    // is looked up.
    for (ns, space) in natives::NAMESPACES.iter().enumerate() {
        let cache: RefCell<HashMap<String, Function>> = RefCell::default();
        let table = lua.create_table()?;
        let meta = lua.create_table()?;
        meta.set(
            "__index",
            lua.create_function(move |lua, (_, key): (Table, String)| {
                if let Some(f) = cache.borrow().get(&key) {
                    return Ok(Value::Function(f.clone()));
                }
                let Some(index) = natives::find(ns, &key) else { return Ok(Value::Nil) };
                let f = lua.create_function(move |lua, args: MultiValue| natives::call(lua, index, args))?;
                cache.borrow_mut().insert(key, f.clone());
                Ok(Value::Function(f))
            })?,
        )?;
        table.set_metatable(Some(meta))?;
        if *space == "task" {
            // The scheduler shares the table with the TASK_ natives.
            for pair in task.pairs::<Value, Value>() {
                let (k, v) = pair?;
                table.raw_set(k, v)?;
            }
        }
        g.set(*space, table)?;
    }

    let deadline = Rc::new(Cell::new(Instant::now() + SLICE));
    let interrupt_deadline = deadline.clone();
    lua.set_interrupt(move |_| {
        Ok(if Instant::now() >= interrupt_deadline.get() { VmState::Yield } else { VmState::Continue })
    });
    Ok((lua, deadline))
}

/// `require("./menu")`: the mod's `menu.luau`, run once.
fn require(lua: &Lua, name: &str, dir: &Path) -> mlua::Result<Function> {
    let loaded: RefCell<HashMap<PathBuf, Value>> = RefCell::default();
    let (name, dir) = (name.to_string(), dir.to_path_buf());
    lua.create_function(move |lua, path: String| {
        let rel = path
            .strip_prefix("./")
            .filter(|r| !r.split('/').any(|p| p.is_empty() || p.starts_with('.')))
            .ok_or_else(|| mlua::Error::runtime(format!("require(\"{path}\"): a file of the mod, as \"./name\"")))?;
        let file = dir.join(format!("{rel}.luau"));
        if let Some(v) = loaded.borrow().get(&file) {
            return Ok(v.clone());
        }
        let source = std::fs::read_to_string(&file)
            .map_err(|e| mlua::Error::runtime(format!("require(\"{path}\"): {e}")))?;
        let value: Value = lua.load(source).set_name(format!("@{name}/{rel}.luau")).call(())?;
        let value = if value.is_nil() { Value::Boolean(true) } else { value };
        loaded.borrow_mut().insert(file, value.clone());
        Ok(value)
    })
}

/// An error as people read it: its message, and where in the mod's files it
/// happened when the message does not say (natives' errors).
pub fn error_text(e: &mlua::Error) -> String {
    let (message, traceback) = match e {
        mlua::Error::CallbackError { traceback, cause } => (error_text(cause), Some(traceback.as_str())),
        // Without the traceback the message carries (its `file:line` is enough).
        mlua::Error::RuntimeError(m) => (m.split("\nstack traceback:").next().unwrap_or(m).to_string(), None),
        mlua::Error::SyntaxError { message, .. } => (message.clone(), None),
        mlua::Error::MemoryError(_) => ("out of memory (a mod may use 64 MB)".into(), None),
        other => (other.to_string(), None),
    };
    // `trainer/main.luau:12: in function ...`: the first frame in a mod's file.
    let at = traceback.and_then(|t| {
        t.lines().map(str::trim).find(|l| l.contains(".luau:")).and_then(|l| {
            let end = l.find(".luau:")? + 6;
            let digits = l[end..].chars().take_while(char::is_ascii_digit).count();
            Some(l[..end + digits].trim_start_matches('@').to_string())
        })
    });
    match at {
        Some(at) if !message.contains(".luau:") => format!("{at}: {message}"),
        _ => message,
    }
}

/// A value as the REPL shows it.
pub fn describe(v: &Value) -> String {
    match v {
        Value::String(s) => format!("{:?}", s.to_string_lossy()),
        other => describe_plain(other),
    }
}

/// A value as `print` shows it: strings as they are.
fn describe_plain(v: &Value) -> String {
    match v {
        Value::Nil => "nil".into(),
        Value::Boolean(b) => b.to_string(),
        Value::Integer(i) => i.to_string(),
        Value::Number(n) if n.fract() == 0.0 && n.abs() < 1e15 => format!("{n:.0}"),
        Value::Number(n) => format!("{n}"),
        Value::Vector(v) => format!("vector({}, {}, {})", short(v.x()), short(v.y()), short(v.z())),
        Value::String(s) => s.to_string_lossy(),
        Value::Table(t) => {
            let mut parts = Vec::new();
            for (i, pair) in t.pairs::<Value, Value>().enumerate() {
                if i == 16 {
                    parts.push("…".to_string());
                    break;
                }
                if let Ok((k, v)) = pair {
                    parts.push(match k {
                        Value::Integer(_) => describe(&v),
                        k => format!("{} = {}", describe_plain(&k), describe(&v)),
                    });
                }
            }
            format!("{{{}}}", parts.join(", "))
        }
        Value::Function(_) => "function".into(),
        Value::Error(e) => error_text(e),
        other => other.type_name().into(),
    }
}

fn short(x: f32) -> String {
    let s = format!("{x:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

impl Drop for Vm {
    /// The mod stops: its storage is written now.
    fn drop(&mut self) {
        if let Some(storage) = &self.storage {
            storage.flush(true);
        }
    }
}
