//! Bounded, streaming staging for downloaded vehicle packages.
//!
//! Network transport is deliberately separate: callers stream response chunks
//! into this writer. A package becomes visible only after exact-size and SHA-256
//! verification; failed or abandoned downloads leave no installable file.

use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

const MAX_PACKAGE_BYTES: u64 = 1024 * 1024 * 1024;
static NEXT_TEMP: AtomicU64 = AtomicU64::new(1);

/// Progress of a package download before it is committed to the vehicle store.
pub struct PackageDownload {
    root: PathBuf,
    staging: PathBuf,
    final_path: PathBuf,
    file: File,
    hasher: Sha256,
    expected_bytes: u64,
    written_bytes: u64,
    expected_sha256: [u8; 32],
    committed: bool,
}

impl PackageDownload {
    /// Start a new staged package. `root` must be the dedicated vehicles folder;
    /// `id` is a catalog ID, never an arbitrary filesystem path.
    pub fn begin(root: &Path, id: &str, expected_bytes: u64, sha256: &str) -> Result<Self, String> {
        if !valid_id(id) {
            return Err("invalid vehicle package ID".into());
        }
        if expected_bytes == 0 || expected_bytes > MAX_PACKAGE_BYTES {
            return Err("vehicle package must be between 1 byte and 1 GiB".into());
        }
        let expected_sha256 = parse_sha256(sha256)?;
        fs::create_dir_all(root).map_err(|e| format!("cannot create vehicle store: {e}"))?;
        let root = root.canonicalize().map_err(|e| format!("cannot resolve vehicle store: {e}"))?;
        if !root.is_dir() {
            return Err("vehicle store is not a directory".into());
        }
        let final_path = root.join(format!("{id}.zip"));
        if final_path.exists() {
            return Err("vehicle package already exists; refusing to overwrite it".into());
        }

        let (staging, file) = loop {
            let nonce = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let candidate = root.join(format!(".{id}.{}.{}.part", std::process::id(), nonce));
            match OpenOptions::new().write(true).create_new(true).open(&candidate) {
                Ok(file) => break (candidate, file),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(format!("cannot create download staging file: {e}")),
            }
        };
        Ok(Self {
            root, staging, final_path, file, hasher: Sha256::new(),
            expected_bytes, written_bytes: 0, expected_sha256, committed: false,
        })
    }

    /// Append one network chunk. Memory use is bounded by the caller's chunk size.
    pub fn write_chunk(&mut self, chunk: &[u8]) -> Result<(), String> {
        let next = self.written_bytes.checked_add(chunk.len() as u64)
            .ok_or("download size overflow")?;
        if next > self.expected_bytes || next > MAX_PACKAGE_BYTES {
            return Err("download exceeded the catalog package size".into());
        }
        self.file.write_all(chunk).map_err(|e| format!("cannot write package chunk: {e}"))?;
        self.hasher.update(chunk);
        self.written_bytes = next;
        Ok(())
    }

    pub fn progress(&self) -> (u64, u64) {
        (self.written_bytes, self.expected_bytes)
    }

    /// Verify the complete stream and atomically publish it without replacing
    /// an existing package. Returns the committed ZIP path.
    pub fn finish(mut self) -> Result<PathBuf, String> {
        if self.written_bytes != self.expected_bytes {
            return Err(format!("incomplete download: received {} of {} bytes", self.written_bytes, self.expected_bytes));
        }
        self.file.flush().map_err(|e| format!("cannot flush package: {e}"))?;
        self.file.sync_all().map_err(|e| format!("cannot sync package: {e}"))?;
        let actual: [u8; 32] = self.hasher.clone().finalize().into();
        if actual != self.expected_sha256 {
            return Err("package SHA-256 mismatch; download rejected".into());
        }
        // Re-check immediately before same-directory atomic rename. This works
        // on Android's emulated Documents storage where hard links may be denied.
        if self.final_path.exists() {
            return Err("vehicle package appeared during download; refusing to overwrite it".into());
        }
        fs::rename(&self.staging, &self.final_path)
            .map_err(|e| format!("cannot atomically commit vehicle package: {e}"))?;
        self.committed = true;
        if let Ok(dir) = File::open(&self.root) { let _ = dir.sync_all(); }
        Ok(self.final_path.clone())
    }
}

impl Drop for PackageDownload {
    fn drop(&mut self) {
        if !self.committed {
            let _ = fs::remove_file(&self.staging);
        }
    }
}

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 48
        && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
}

fn parse_sha256(value: &str) -> Result<[u8; 32], String> {
    if value.len() != 64 || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("SHA-256 must contain exactly 64 hexadecimal characters".into());
    }
    let mut out = [0u8; 32];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[i * 2..i * 2 + 2], 16)
            .map_err(|_| "invalid SHA-256".to_string())?;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_root() -> PathBuf {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("glue5-vehicle-download-{}-{stamp}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    #[test]
    fn streams_chunks_and_commits_only_verified_package() {
        let root = test_root();
        let data = b"vehicle package fixture";
        let hash = hex(&Sha256::digest(data));
        let mut download = PackageDownload::begin(&root, "test-car", data.len() as u64, &hash).unwrap();
        download.write_chunk(&data[..7]).unwrap();
        download.write_chunk(&data[7..]).unwrap();
        assert_eq!(download.progress(), (data.len() as u64, data.len() as u64));
        let path = download.finish().unwrap();
        assert_eq!(fs::read(path).unwrap(), data);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_hash_mismatch_and_removes_partial_file() {
        let root = test_root();
        let mut download = PackageDownload::begin(&root, "test-car", 4, &"0".repeat(64)).unwrap();
        download.write_chunk(b"data").unwrap();
        assert!(download.finish().unwrap_err().contains("SHA-256 mismatch"));
        assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_traversal_and_incomplete_downloads() {
        let root = test_root();
        assert!(PackageDownload::begin(&root, "../escape", 4, &"0".repeat(64)).is_err());
        let download = PackageDownload::begin(&root, "valid-car", 4, &"0".repeat(64)).unwrap();
        assert!(download.finish().unwrap_err().contains("incomplete"));
        assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn refuses_to_overwrite_existing_package() {
        let root = test_root();
        fs::write(root.join("test-car.zip"), b"keep me").unwrap();
        assert!(PackageDownload::begin(&root, "test-car", 4, &"0".repeat(64)).is_err());
        assert_eq!(fs::read(root.join("test-car.zip")).unwrap(), b"keep me");
        fs::remove_dir_all(root).unwrap();
    }
}
