//! The game's files from a server ([`glue5_stream`]), through the engine's
//! own HTTP file device: the browser build's way of reading `/game`.
//!
//! When the game streams, `wasm_httpfs_manifest_js` hands the engine the
//! server's file list; its `fiDeviceLocal` then serves every `/game` file
//! from that list (lookups, sizes, times, directory listings) and reads
//! through `wasm_httpfs_pread`, hooked here: a read the copy holds is a
//! positioned read from it in the calling thread, as with a local folder; a
//! missing one waits for the server. The engine also announces the reads its
//! streamer has queued (and some it expects) in a ring, drained here into
//! prefetch requests. The browser build's request slots are not used.

use std::fs::File;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use glue5_game::Env;
use glue5_stream::Client;

use crate::memory::Mem;
use crate::rt;

fn client() -> Option<&'static Arc<Client>> {
    rt().config.stream.as_ref()
}

pub(crate) fn hooks(hooks: &mut glue5_game::Hooks) {
    if client().is_some() {
        hooks.httpfs_pread = Some(pread);
    }
}

/// The manifest wasm_httpfs_manifest_js gives the engine, when streaming.
/// Imported package paths are merged into this per-launch manifest. Existing
/// paths keep their file IDs; new loose files are appended and served locally.
pub(crate) fn manifest() -> Option<&'static [u8]> {
    let client = client()?;
    if client.engine_manifest().is_none() {
        return None;
    }
    Some(&overlay_state(client)?.manifest)
}

struct StreamOverlay {
    manifest: Vec<u8>,
    files: std::collections::HashMap<u32, File>,
}

fn overlay_state(client: &Client) -> Option<&'static StreamOverlay> {
    if client.engine_manifest().is_none() {
        return None;
    }
    static STATE: OnceLock<StreamOverlay> = OnceLock::new();
    Some(STATE.get_or_init(|| build_overlay_state(client)))
}

fn build_overlay_state(client: &Client) -> StreamOverlay {
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};
    use std::time::UNIX_EPOCH;
    use sha2::{Digest, Sha256};

    let Some(base_bytes) = client.engine_manifest() else {
        return StreamOverlay { manifest: Vec::new(), files: Default::default() };
    };
    let mut manifest: serde_json::Value = match serde_json::from_slice(base_bytes) {
        Ok(value) => value,
        Err(error) => {
            log::warn!("stream: cannot parse engine manifest for PC overlays: {error}");
            return StreamOverlay { manifest: base_bytes.to_vec(), files: Default::default() };
        }
    };
    let Some(entries) = manifest.get_mut("files").and_then(serde_json::Value::as_array_mut) else {
        log::warn!("stream: engine manifest has no files array; PC overlays disabled");
        return StreamOverlay { manifest: base_bytes.to_vec(), files: Default::default() };
    };
    let base_count = client.engine_file_count().unwrap_or(entries.len());
    if entries.len() != base_count {
        log::warn!("stream: engine manifest count does not match catalog; PC overlays disabled");
        return StreamOverlay { manifest: base_bytes.to_vec(), files: Default::default() };
    }

    // Lowercase keys make package priority case-insensitive. Later packages
    // replace earlier ones, matching the local VFS overlay's last-mount-wins rule.
    let mut candidates: BTreeMap<String, (String, PathBuf)> = BTreeMap::new();

    // Keep Glue5's own XML changes, but let an imported package override them.
    let scratch = rt().config.user_dir.join("fsroot");
    for name in crate::overlay::names() {
        let Some(id) = client.find(&format!("data/{name}")) else { continue };
        if id as usize >= base_count { continue; }
        let Some(real) = client.path(id) else { continue };
        if let Some(changed) = crate::overlay::game_file(name, &real, &scratch) {
            candidates.insert(name.to_ascii_lowercase(), (name.to_owned(), changed));
        }
    }

    const MAX_PC_OVERLAY_FILES: usize = 50_000;
    fn visit(
        root: &Path,
        dir: &Path,
        out: &mut BTreeMap<String, (String, PathBuf)>,
        visited: &mut usize,
    ) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        let mut entries = entries.flatten().collect::<Vec<_>>();
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            if *visited >= MAX_PC_OVERLAY_FILES { return; }
            let path = entry.path();
            let Ok(kind) = entry.file_type() else { continue };
            if kind.is_symlink() { continue; }
            if kind.is_dir() {
                if entry.file_name().to_string_lossy().starts_with('.') { continue; }
                visit(root, &path, out, visited);
                continue;
            }
            if !kind.is_file() { continue; }
            *visited += 1;
            let Ok(relative) = path.strip_prefix(root) else { continue };
            let relative = relative.to_string_lossy().replace('\\', "/");
            if relative.is_empty()
                || relative.split('/').any(|part| part.is_empty() || part == "." || part == ".." || part.starts_with('.'))
            {
                continue;
            }
            out.insert(relative.to_ascii_lowercase(), (relative, path));
        }
    }

    // Scan installed packages in stable order. A package in user_dir is
    // visited after game_dir and therefore has the higher overlay priority.
    let roots = [
        rt().config.game_dir.join("mods/.pc-packages"),
        rt().config.user_dir.join("mods/.pc-packages"),
    ];
    let mut visited = 0usize;
    for root in roots {
        let Ok(packages) = std::fs::read_dir(&root) else { continue };
        let mut packages = packages.flatten().map(|entry| entry.path())
            .filter(|path| {
                let Ok(metadata) = std::fs::symlink_metadata(path) else { return false };
                metadata.file_type().is_dir()
                    && !path.file_name().is_some_and(|name| name.to_string_lossy().starts_with('.'))
            })
            .collect::<Vec<PathBuf>>();
        packages.sort();
        for package in packages {
            visit(&package, &package, &mut candidates, &mut visited);
            if visited >= MAX_PC_OVERLAY_FILES { break; }
        }
        if visited >= MAX_PC_OVERLAY_FILES { break; }
    }

    let mut files = std::collections::HashMap::new();
    let mut digest = Sha256::new();
    let mut replaced = 0usize;
    let mut added = 0usize;
    for (_, (relative, path)) in candidates {
        let Ok(file) = File::open(&path) else { continue };
        let Ok(metadata) = file.metadata() else { continue };
        let size = metadata.len();
        let mtime = metadata.modified().ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |duration| duration.as_secs());

        let base_id = client.find(&format!("data/{relative}"));
        let Some((id, is_new)) = register_manifest_entry(
            entries, base_count, base_id, &relative, size, mtime
        ) else { continue };
        if is_new { added += 1; } else { replaced += 1; }
        digest.update(relative.as_bytes());
        digest.update(size.to_le_bytes());
        digest.update(mtime.to_le_bytes());
        files.insert(id, file);
    }

    if replaced + added > 0 {
        let version = manifest.get("version").and_then(serde_json::Value::as_str).unwrap_or("base");
        let hash = digest.finalize();
        let suffix = hash[..8].iter().map(|byte| format!("{byte:02x}")).collect::<String>();
        manifest["version"] = serde_json::Value::String(format!("{version}-pcmods-{suffix}"));
        log::info!(
            "stream: PC package manifest: {replaced} existing paths replaced, {added} new paths added"
        );
    }

    let bytes = serde_json::to_vec(&manifest).unwrap_or_else(|error| {
        log::warn!("stream: cannot serialize merged manifest: {error}");
        base_bytes.to_vec()
    });
    StreamOverlay { manifest: bytes, files }
}

/// Update an existing engine-manifest entry or append a package-only path.
/// Returns the file ID and whether a new manifest entry was created.
fn register_manifest_entry(
    entries: &mut Vec<serde_json::Value>,
    base_count: usize,
    base_id: Option<u32>,
    relative: &str,
    size: u64,
    mtime: u64,
) -> Option<(u32, bool)> {
    if let Some(id) = base_id.filter(|id| (*id as usize) < base_count) {
        let entry = entries.get_mut(id as usize)?;
        let canonical = entry.as_array().and_then(|array| array.first())
            .and_then(serde_json::Value::as_str).unwrap_or(relative).to_owned();
        *entry = serde_json::json!([canonical, size, mtime]);
        Some((id, false))
    } else {
        let id = u32::try_from(entries.len()).ok()?;
        entries.push(serde_json::json!([relative, size, mtime]));
        Some((id, true))
    }
}

#[cfg(test)]
mod manifest_tests {
    use super::*;

    #[test]
    fn replacement_updates_size_and_mtime_without_changing_id_or_path() {
        let mut entries = vec![serde_json::json!(["update/update.rpf", 100, 10])];
        let result = register_manifest_entry(&mut entries, 1, Some(0), "UPDATE/UPDATE.RPF", 250, 20);
        assert_eq!(result, Some((0, false)));
        assert_eq!(entries[0], serde_json::json!(["update/update.rpf", 250, 20]));
    }

    #[test]
    fn new_package_path_is_appended_after_engine_files() {
        let mut entries = vec![serde_json::json!(["common.rpf", 100, 10])];
        let result = register_manifest_entry(&mut entries, 1, None, "dlcpacks/new/dlc.rpf", 300, 30);
        assert_eq!(result, Some((1, true)));
        assert_eq!(entries[1], serde_json::json!(["dlcpacks/new/dlc.rpf", 300, 30]));
    }

    #[test]
    fn non_engine_catalog_id_is_appended_as_a_new_game_path() {
        let mut entries = vec![serde_json::json!(["common.rpf", 100, 10])];
        let result = register_manifest_entry(&mut entries, 1, Some(99), "dlcpacks/new/dlc.rpf", 300, 30);
        assert_eq!(result, Some((1, true)));
    }
}

/// Hook for wasm_httpfs_pread(id, offset, dst, len): bytes read, or -1
/// (the engine tries again). Package-only IDs are served from local files.
unsafe extern "C" fn pread(env: *mut Env, id: u32, offset: u64, dst: u64, len: u32) -> u32 {
    let Some(client) = client() else { return u32::MAX };
    let buf = unsafe { Mem::of(env) }.bytes_mut(dst, len as usize);
    if let Some(file) = overlay_state(client).and_then(|state| state.files.get(&id)) {
        return read_at(file, buf, offset).map_or(u32::MAX, |n| n as u32);
    }
    client.read(id, offset, buf).map_or(u32::MAX, |n| n as u32)
}

/// Called by the engine (`wasm_httpfs_io_start_js`) with the address of its
/// I/O table: starts draining the read announcements. False if not streaming.
pub(crate) fn start_announcements(table: u64) -> bool {
    static STARTED: AtomicBool = AtomicBool::new(false);
    let Some(client) = client().cloned() else { return false };
    if STARTED.swap(true, Ordering::AcqRel) {
        return true;
    }
    log::info!("stream: the engine's read announcements at {table:#x}");
    let spawned = std::thread::Builder::new().name("stream hints".into()).spawn(move || {
        let m = rt().memory.mem();
        let bell = m.atomic_u32(table);
        loop {
            let seen = bell.load(Ordering::SeqCst);
            drain(m, table, &client);
            glue5_game::futex::wait32(bell, seen, Some(Duration::from_secs(1)));
        }
    });
    if let Err(e) = spawned {
        log::warn!("stream: no announcement thread: {e}");
    }
    true
}

/// The announcement ring (the engine's `httpfs_wasm.cpp` layout, 32-bit
/// words from `table`): [2] the ring's word offset, [3] its entries, [4]
/// entries claimed by the engine, [5] entries taken here. An entry is a file
/// id, the offset's low and high words, and the length, written last (0:
/// claimed, not yet written; bit 30: a guess rather than a queued read).
fn drain(m: Mem, table: u64, client: &Client) {
    let word = |i: u64| m.atomic_u32(table + 4 * i);
    let (ring, cap) = (word(2).load(Ordering::Acquire) as u64, word(3).load(Ordering::Acquire));
    if ring == 0 || cap == 0 {
        return;
    }
    let head = word(4).load(Ordering::Acquire);
    let mut tail = word(5).load(Ordering::Acquire);
    if head.wrapping_sub(tail) > cap {
        // The engine went round the ring: the oldest were overwritten.
        tail = head.wrapping_sub(cap);
    }
    while tail != head {
        let e = ring + 4 * (tail % cap) as u64;
        let raw = word(e + 3).load(Ordering::Acquire);
        if raw == 0 {
            break;
        }
        let off = word(e + 1).load(Ordering::Relaxed) as u64 | (word(e + 2).load(Ordering::Relaxed) as u64) << 32;
        client.hint(word(e).load(Ordering::Relaxed), off, (raw & 0x3fff_ffff) as u64, raw & 0x4000_0000 != 0);
        word(e + 3).store(0, Ordering::Release);
        tail = tail.wrapping_add(1);
    }
    word(5).store(tail, Ordering::Release);
}

fn read_at(f: &File, buf: &mut [u8], off: u64) -> std::io::Result<usize> {
    let mut done = 0;
    while done < buf.len() {
        #[cfg(unix)]
        let n = std::os::unix::fs::FileExt::read_at(f, &mut buf[done..], off + done as u64)?;
        #[cfg(windows)]
        let n = std::os::windows::fs::FileExt::seek_read(f, &mut buf[done..], off + done as u64)?;
        if n == 0 {
            break;
        }
        done += n;
    }
    Ok(done)
}
