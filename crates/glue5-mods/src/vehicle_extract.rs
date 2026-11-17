//! Safe streaming ZIP extraction for vehicle packages.
//!
//! ZIP64, encrypted entries, symlinks, path traversal and unsupported
//! compression methods are rejected. Extraction is staged in a new directory
//! and published only after every entry, size and CRC has been verified.

use flate2::read::DeflateDecoder;
use std::{
    collections::HashSet,
    fs::{self, File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

const MAX_FILES: usize = 10_000;
const MAX_FILE_BYTES: u64 = 256 * 1024 * 1024;
const MAX_TOTAL_BYTES: u64 = 1024 * 1024 * 1024;
const MAX_CENTRAL_BYTES: usize = 64 * 1024 * 1024;
const MAX_NAME_BYTES: usize = 4096;
static NEXT_STAGE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedPackage {
    pub directory: PathBuf,
    pub files: usize,
    pub bytes: u64,
}

/// Extract a verified ZIP into `root/id/`. The destination must not exist.
/// On any error the staging directory is removed and no partial install remains.
pub fn extract_vehicle_zip(archive: &Path, root: &Path, id: &str) -> Result<ExtractedPackage, String> {
    if !valid_id(id) { return Err("invalid vehicle package ID".into()); }
    let mut file = File::open(archive).map_err(|e| format!("cannot open vehicle ZIP: {e}"))?;
    let archive_len = file.metadata().map_err(|e| format!("cannot inspect vehicle ZIP: {e}"))?.len();
    if archive_len < 22 { return Err("vehicle package is not a valid ZIP".into()); }
    fs::create_dir_all(root).map_err(|e| format!("cannot create vehicle install root: {e}"))?;
    let root = root.canonicalize().map_err(|e| format!("cannot resolve vehicle install root: {e}"))?;
    let destination = root.join(id);
    if destination.exists() { return Err("vehicle is already installed; refusing to overwrite".into()); }

    let entries = read_directory(&mut file, archive_len)?;
    if entries.len() > MAX_FILES { return Err(format!("ZIP contains more than {MAX_FILES} entries")); }

    let stage = loop {
        let n = NEXT_STAGE.fetch_add(1, Ordering::Relaxed);
        let p = root.join(format!(".{id}.{}.{}.staging", std::process::id(), n));
        match fs::create_dir(&p) {
            Ok(()) => break p,
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(format!("cannot create extraction staging folder: {e}")),
        }
    };

    let result = extract_entries(&mut file, &entries, &stage);
    match result {
        Ok((files, bytes)) => {
            if destination.exists() { let _ = fs::remove_dir_all(&stage); return Err("vehicle install destination appeared; refusing to overwrite".into()); }
            if let Err(e) = fs::rename(&stage, &destination) {
                let _ = fs::remove_dir_all(&stage);
                return Err(format!("cannot publish extracted vehicle package: {e}"));
            }
            Ok(ExtractedPackage { directory: destination, files, bytes })
        }
        Err(e) => {
            let _ = fs::remove_dir_all(&stage);
            Err(e)
        }
    }
}

#[derive(Debug)]
struct Entry {
    name: String,
    compressed: u64,
    uncompressed: u64,
    method: u16,
    flags: u16,
    crc32: u32,
    local_offset: u64,
    directory: bool,
}

fn read_directory(file: &mut File, length: u64) -> Result<Vec<Entry>, String> {
    let tail_len = length.min(65_557) as usize;
    file.seek(SeekFrom::End(-(tail_len as i64))).map_err(ioerr)?;
    let mut tail = vec![0; tail_len];
    file.read_exact(&mut tail).map_err(ioerr)?;
    let eocd = (0..=tail.len().saturating_sub(22)).rev()
        .find(|&i| tail.get(i..i + 4) == Some(&[0x50, 0x4b, 0x05, 0x06]))
        .ok_or("ZIP end directory record is missing")?;
    let disk = u16le(&tail, eocd + 4)?;
    let central_disk = u16le(&tail, eocd + 6)?;
    let disk_count = u16le(&tail, eocd + 8)?;
    let count = u16le(&tail, eocd + 10)? as usize;
    let central_size = u32le(&tail, eocd + 12)? as usize;
    let central_offset = u32le(&tail, eocd + 16)? as u64;
    if disk != 0 || central_disk != 0 || disk_count as usize != count {
        return Err("multi-disk ZIP packages are not supported".into());
    }
    if count == 0xffff || central_size == u32::MAX as usize || central_offset == u32::MAX as u64 {
        return Err("ZIP64 vehicle packages are not supported yet".into());
    }
    if count > MAX_FILES || central_size > MAX_CENTRAL_BYTES
        || central_offset.checked_add(central_size as u64).is_none_or(|end| end > length) {
        return Err("ZIP central directory exceeds safety limits".into());
    }
    file.seek(SeekFrom::Start(central_offset)).map_err(ioerr)?;
    let mut central = vec![0; central_size];
    file.read_exact(&mut central).map_err(ioerr)?;
    let mut pos = 0usize;
    let mut entries = Vec::with_capacity(count);
    let mut names = HashSet::new();
    let mut total = 0u64;
    for _ in 0..count {
        if u32le(&central, pos)? != 0x02014b50 { return Err("damaged ZIP central directory".into()); }
        let made_by = u16le(&central, pos + 4)?;
        let flags = u16le(&central, pos + 8)?;
        let method = u16le(&central, pos + 10)?;
        let crc32 = u32le(&central, pos + 16)?;
        let compressed = u32le(&central, pos + 20)? as u64;
        let uncompressed = u32le(&central, pos + 24)? as u64;
        let name_len = u16le(&central, pos + 28)? as usize;
        let extra_len = u16le(&central, pos + 30)? as usize;
        let comment_len = u16le(&central, pos + 32)? as usize;
        let disk_start = u16le(&central, pos + 34)?;
        let external = u32le(&central, pos + 38)?;
        let local_offset = u32le(&central, pos + 42)? as u64;
        let end = pos.checked_add(46 + name_len + extra_len + comment_len).ok_or("ZIP directory overflow")?;
        if end > central.len() || name_len == 0 || name_len > MAX_NAME_BYTES { return Err("invalid ZIP entry name or record length".into()); }
        let name = std::str::from_utf8(&central[pos + 46..pos + 46 + name_len])
            .map_err(|_| "ZIP entry name is not UTF-8")?.replace('\\', "/");
        pos = end;
        if disk_start != 0 { return Err("multi-disk ZIP entry is not supported".into()); }
        if flags & 1 != 0 { return Err(format!("encrypted ZIP entry is not supported: {name}")); }
        if flags & 0x40 != 0 { return Err(format!("strongly encrypted ZIP entry is not supported: {name}")); }
        if method != 0 && method != 8 { return Err(format!("unsupported ZIP compression method for {name}")); }
        if compressed == u32::MAX as u64 || uncompressed == u32::MAX as u64 || local_offset == u32::MAX as u64 {
            return Err("ZIP64 entry is not supported".into());
        }
        // Unix creator (3): reject symbolic links and other special file types.
        if made_by >> 8 == 3 {
            let mode = (external >> 16) as u16;
            let kind = mode & 0o170000;
            if kind != 0 && kind != 0o100000 && kind != 0o040000 {
                return Err(format!("special or symbolic-link ZIP entry rejected: {name}"));
            }
        }
        let directory = name.ends_with('/');
        let clean = name.trim_end_matches('/');
        if clean.is_empty() || clean.starts_with('/') || clean.contains(':')
            || clean.bytes().any(|b| b == 0 || b < 0x20)
            || clean.split('/').any(|part| part.is_empty() || part == "." || part == "..") {
            return Err(format!("unsafe ZIP path rejected: {name}"));
        }
        let relative = Path::new(clean);
        if relative.components().any(|c| !matches!(c, Component::Normal(_))) {
            return Err(format!("unsafe ZIP path rejected: {name}"));
        }
        let key = clean.to_ascii_lowercase();
        if !names.insert(key) { return Err(format!("duplicate ZIP path rejected: {name}")); }
        if !directory {
            if uncompressed > MAX_FILE_BYTES { return Err(format!("ZIP entry exceeds 256 MiB: {name}")); }
            total = total.checked_add(uncompressed).ok_or("ZIP expanded size overflow")?;
            if total > MAX_TOTAL_BYTES { return Err("ZIP expanded size exceeds 1 GiB safety limit".into()); }
        }
        entries.push(Entry { name: clean.to_string(), compressed, uncompressed, method, flags, crc32, local_offset, directory });
    }
    if pos != central.len() { return Err("unexpected trailing data in ZIP central directory".into()); }
    Ok(entries)
}

fn extract_entries(file: &mut File, entries: &[Entry], stage: &Path) -> Result<(usize, u64), String> {
    let mut total_written = 0u64;
    let mut files_written = 0usize;
    for entry in entries {
        let target = stage.join(&entry.name);
        if !target.starts_with(stage) { return Err("ZIP entry escapes staging folder".into()); }
        if entry.directory {
            fs::create_dir_all(&target).map_err(ioerr)?;
            continue;
        }
        if let Some(parent) = target.parent() { fs::create_dir_all(parent).map_err(ioerr)?; }
        // create_new prevents an entry from replacing a file created earlier.
        let mut output = OpenOptions::new().write(true).create_new(true).open(&target).map_err(ioerr)?;
        file.seek(SeekFrom::Start(entry.local_offset)).map_err(ioerr)?;
        let mut header = [0u8; 30];
        file.read_exact(&mut header).map_err(ioerr)?;
        if u32::from_le_bytes(header[0..4].try_into().unwrap()) != 0x04034b50 {
            return Err(format!("invalid local ZIP header: {}", entry.name));
        }
        let flags = u16::from_le_bytes(header[6..8].try_into().unwrap());
        let method = u16::from_le_bytes(header[8..10].try_into().unwrap());
        let name_len = u16::from_le_bytes(header[26..28].try_into().unwrap()) as u64;
        let extra_len = u16::from_le_bytes(header[28..30].try_into().unwrap()) as u64;
        if flags != entry.flags || method != entry.method { return Err(format!("ZIP header mismatch: {}", entry.name)); }
        let mut local_name = vec![0; name_len as usize];
        file.read_exact(&mut local_name).map_err(ioerr)?;
        if local_name != entry.name.as_bytes() { return Err(format!("ZIP entry name mismatch: {}", entry.name)); }
        file.seek(SeekFrom::Current(extra_len as i64)).map_err(ioerr)?;
        let limited = (&mut *file).take(entry.compressed);
        let mut crc = flate2::Crc::new();
        let mut written = 0u64;
        let mut buffer = [0u8; 64 * 1024];
        match entry.method {
            0 => {
                let mut reader = limited;
                loop {
                    let n = reader.read(&mut buffer).map_err(ioerr)?;
                    if n == 0 { break; }
                    account_chunk(&buffer[..n], &mut output, &mut crc, &mut written, entry)?;
                }
                if written != entry.compressed { return Err(format!("truncated stored ZIP entry: {}", entry.name)); }
            }
            8 => {
                let mut decoder = DeflateDecoder::new(limited);
                loop {
                    let n = decoder.read(&mut buffer).map_err(|e| format!("cannot decompress {}: {e}", entry.name))?;
                    if n == 0 { break; }
                    account_chunk(&buffer[..n], &mut output, &mut crc, &mut written, entry)?;
                }
                if decoder.total_in() != entry.compressed { return Err(format!("compressed-size mismatch: {}", entry.name)); }
            }
            _ => unreachable!(),
        }
        if written != entry.uncompressed || crc.sum() != entry.crc32 {
            return Err(format!("size or CRC verification failed: {}", entry.name));
        }
        output.flush().map_err(ioerr)?;
        output.sync_all().map_err(ioerr)?;
        total_written += written;
        files_written += 1;
    }
    Ok((files_written, total_written))
}

fn account_chunk(chunk: &[u8], out: &mut File, crc: &mut flate2::Crc, written: &mut u64, entry: &Entry) -> Result<(), String> {
    *written = written.checked_add(chunk.len() as u64).ok_or("expanded ZIP size overflow")?;
    if *written > entry.uncompressed || *written > MAX_FILE_BYTES { return Err(format!("ZIP decompression limit exceeded: {}", entry.name)); }
    crc.update(chunk);
    out.write_all(chunk).map_err(ioerr)
}

fn u16le(data: &[u8], at: usize) -> Result<u16, String> {
    data.get(at..at + 2).map(|b| u16::from_le_bytes([b[0], b[1]])).ok_or_else(|| "truncated ZIP record".into())
}
fn u32le(data: &[u8], at: usize) -> Result<u32, String> {
    data.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])).ok_or_else(|| "truncated ZIP record".into())
}
fn ioerr(e: impl std::fmt::Display) -> String { format!("vehicle ZIP I/O error: {e}") }
fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 48 && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root() -> PathBuf {
        let n = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let p = std::env::temp_dir().join(format!("glue5-vehicle-zip-{}-{n}", std::process::id()));
        fs::create_dir_all(&p).unwrap(); p
    }

    #[test]
    fn extracts_a_valid_zip_and_keeps_original_archive() {
        let root = temp_root();
        let zip = root.join("fixture.zip");
        // Minimal stored ZIP with one file, generated by the existing ZIP writer.
        let bytes = crate::zip::write(&[("cars/readme.txt".into(), b"vehicle data".to_vec())]);
        fs::write(&zip, bytes).unwrap();
        let installed = extract_vehicle_zip(&zip, &root.join("installed"), "test-car").unwrap();
        assert_eq!(fs::read(installed.directory.join("cars/readme.txt")).unwrap(), b"vehicle data");
        assert!(zip.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_traversal_and_does_not_leave_partial_install() {
        let root = temp_root();
        let zip = root.join("bad.zip");
        let bytes = crate::zip::write(&[("../escape.txt".into(), b"bad".to_vec())]);
        fs::write(&zip, bytes).unwrap();
        assert!(extract_vehicle_zip(&zip, &root.join("installed"), "bad-car").unwrap_err().contains("unsafe ZIP path"));
        assert!(!root.join("installed/bad-car").exists());
        fs::remove_dir_all(root).unwrap();
    }
}
