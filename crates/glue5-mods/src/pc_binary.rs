//! Safe, non-executing inspection of Windows PE plugin files.
//!
//! This module does not load or execute PE files. It reports basic format
//! metadata so a future, separately implemented compatibility runtime can
//! make explicit support decisions instead of treating every DLL as runnable.

use std::fs;
use std::path::Path;

const MAX_FILE_SIZE: u64 = 128 * 1024 * 1024;
const MAX_IMPORTS: usize = 4096;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Architecture {
    X86,
    X86_64,
    Arm64,
    Other(u16),
}

impl Architecture {
    pub fn label(&self) -> &'static str {
        match self {
            Self::X86 => "x86",
            Self::X86_64 => "x86-64",
            Self::Arm64 => "ARM64",
            Self::Other(_) => "unknown",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PeReport {
    pub architecture: Architecture,
    pub is_dll: bool,
    pub pe32_plus: bool,
    pub imports: Vec<String>,
}

impl PeReport {
    /// A descriptive diagnostic only; it is deliberately not an execution claim.
    pub fn summary(&self) -> String {
        let kind = if self.is_dll { "DLL/plugin" } else { "executable image" };
        format!(
            "PE {kind}; architecture {}; format {}; {} imported libraries; execution is not provided by this inspector",
            self.architecture.label(),
            if self.pe32_plus { "PE32+" } else { "PE32" },
            self.imports.len(),
        )
    }
}

/// Read and inspect a PE file without mapping or executing it.
pub fn inspect_file(path: impl AsRef<Path>) -> Result<PeReport, String> {
    let path = path.as_ref();
    let metadata = fs::metadata(path).map_err(|e| format!("cannot read file metadata: {e}"))?;
    if !metadata.is_file() {
        return Err("not a regular file".into());
    }
    if metadata.len() > MAX_FILE_SIZE {
        return Err(format!("file is larger than the {} MiB inspection limit", MAX_FILE_SIZE / (1024 * 1024)));
    }
    let bytes = fs::read(path).map_err(|e| format!("cannot read file: {e}"))?;
    inspect_bytes(&bytes)
}

/// Inspect an in-memory PE image. This function only reads headers and import names.
pub fn inspect_bytes(bytes: &[u8]) -> Result<PeReport, String> {
    if bytes.len() < 0x40 || bytes.get(0..2) != Some(&b"MZ"[..]) {
        return Err("missing DOS MZ header (not a Windows PE file)".into());
    }
    let pe_offset = read_u32(bytes, 0x3c)? as usize;
    let coff = pe_offset.checked_add(4).ok_or("PE header offset overflow")?;
    if bytes.get(pe_offset..coff) != Some(b"PE\0\0") {
        return Err("missing PE signature".into());
    }
    let machine = read_u16(bytes, coff)?;
    let section_count = read_u16(bytes, coff + 2)? as usize;
    let optional_size = read_u16(bytes, coff + 16)? as usize;
    let characteristics = read_u16(bytes, coff + 18)?;
    let optional = coff.checked_add(20).ok_or("optional header offset overflow")?;
    let optional_end = optional.checked_add(optional_size).ok_or("optional header size overflow")?;
    if optional_end > bytes.len() {
        return Err("truncated optional header".into());
    }

    let magic = read_u16(bytes, optional)?;
    let (pe32_plus, directory_offset, number_of_directories_offset) = match magic {
        0x10b => (false, 96usize, 92usize),
        0x20b => (true, 112usize, 108usize),
        _ => return Err(format!("unsupported PE optional-header magic 0x{magic:04x}")),
    };
    let architecture = match machine {
        0x014c => Architecture::X86,
        0x8664 => Architecture::X86_64,
        0xaa64 => Architecture::Arm64,
        other => Architecture::Other(other),
    };

    let mut imports = Vec::new();
    let count_pos = optional.checked_add(number_of_directories_offset).ok_or("directory offset overflow")?;
    if count_pos + 4 <= optional_end {
        let count = read_u32(bytes, count_pos)?;
        let import_dir_pos = optional.checked_add(directory_offset).ok_or("import directory offset overflow")?;
        if count > 1 && import_dir_pos + 8 <= optional_end {
            let import_rva = read_u32(bytes, import_dir_pos)?;
            let import_size = read_u32(bytes, import_dir_pos + 4)?;
            if import_rva != 0 && import_size != 0 {
                let section_table = optional_end;
                let sections_bytes = section_count.checked_mul(40).ok_or("section table size overflow")?;
                if section_table.checked_add(sections_bytes).ok_or("section table offset overflow")? > bytes.len() {
                    return Err("truncated section table".into());
                }
                let import_offset = rva_to_offset(bytes, section_table, section_count, import_rva)?;
                let descriptor_limit = (import_size as usize / 20).min(MAX_IMPORTS);
                for i in 0..descriptor_limit {
                    let d = import_offset.checked_add(i * 20).ok_or("import descriptor offset overflow")?;
                    if d + 20 > bytes.len() {
                        return Err("truncated import descriptor table".into());
                    }
                    let original_thunk = read_u32(bytes, d)?;
                    let time_date_stamp = read_u32(bytes, d + 4)?;
                    let forwarder_chain = read_u32(bytes, d + 8)?;
                    let name_rva = read_u32(bytes, d + 12)?;
                    let first_thunk = read_u32(bytes, d + 16)?;
                    if original_thunk == 0 && time_date_stamp == 0 && forwarder_chain == 0 && name_rva == 0 && first_thunk == 0 {
                        break;
                    }
                    if name_rva == 0 {
                        continue;
                    }
                    let name_offset = rva_to_offset(bytes, section_table, section_count, name_rva)?;
                    let name = read_c_string(bytes, name_offset, 512)?;
                    if !name.is_empty() && !imports.iter().any(|s| s.eq_ignore_ascii_case(&name)) {
                        imports.push(name);
                    }
                }
            }
        }
    }

    Ok(PeReport {
        architecture,
        is_dll: characteristics & 0x2000 != 0,
        pe32_plus,
        imports,
    })
}

fn read_u16(bytes: &[u8], at: usize) -> Result<u16, String> {
    let s = bytes.get(at..at.checked_add(2).ok_or("offset overflow")?).ok_or("truncated PE header")?;
    Ok(u16::from_le_bytes([s[0], s[1]]))
}

fn read_u32(bytes: &[u8], at: usize) -> Result<u32, String> {
    let s = bytes.get(at..at.checked_add(4).ok_or("offset overflow")?).ok_or("truncated PE header")?;
    Ok(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

fn rva_to_offset(bytes: &[u8], section_table: usize, section_count: usize, rva: u32) -> Result<usize, String> {
    for index in 0..section_count {
        let section = section_table.checked_add(index * 40).ok_or("section offset overflow")?;
        let virtual_size = read_u32(bytes, section + 8)? as u64;
        let virtual_address = read_u32(bytes, section + 12)? as u64;
        let raw_size = read_u32(bytes, section + 16)? as u64;
        let raw_pointer = read_u32(bytes, section + 20)? as u64;
        let span = virtual_size.max(raw_size);
        let value = rva as u64;
        if value >= virtual_address && value < virtual_address.saturating_add(span) {
            let delta = value - virtual_address;
            if delta >= raw_size {
                return Err("RVA points into an unbacked section region".into());
            }
            let offset = raw_pointer.checked_add(delta).ok_or("RVA mapping overflow")?;
            let offset = usize::try_from(offset).map_err(|_| "RVA offset is too large")?;
            if offset >= bytes.len() {
                return Err("RVA maps outside the file".into());
            }
            return Ok(offset);
        }
    }
    Err(format!("RVA 0x{rva:x} is not mapped by a section"))
}

fn read_c_string(bytes: &[u8], offset: usize, max_len: usize) -> Result<String, String> {
    let tail = bytes.get(offset..).ok_or("string offset outside file")?;
    let limit = tail.len().min(max_len);
    let end = tail[..limit].iter().position(|b| *b == 0).ok_or("unterminated import library name")?;
    let raw = &tail[..end];
    if !raw.is_ascii() {
        return Err("import library name is not ASCII".into());
    }
    Ok(String::from_utf8_lossy(raw).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_pe(machine: u16, magic: u16, is_dll: bool) -> Vec<u8> {
        let mut b = vec![0u8; 0x40 + 4 + 20 + 240];
        b[0..2].copy_from_slice(b"MZ");
        b[0x3c..0x40].copy_from_slice(&(0x40u32).to_le_bytes());
        b[0x40..0x44].copy_from_slice(b"PE\0\0");
        let c = 0x44;
        b[c..c + 2].copy_from_slice(&machine.to_le_bytes());
        b[c + 2..c + 4].copy_from_slice(&0u16.to_le_bytes());
        b[c + 16..c + 18].copy_from_slice(&240u16.to_le_bytes());
        b[c + 18..c + 20].copy_from_slice(&(if is_dll { 0x2000u16 } else { 0 }).to_le_bytes());
        b[c + 20..c + 22].copy_from_slice(&magic.to_le_bytes());
        b
    }

    #[test]
    fn identifies_x64_dll_without_executing_it() {
        let bytes = minimal_pe(0x8664, 0x20b, true);
        let report = inspect_bytes(&bytes).unwrap();
        assert_eq!(report.architecture, Architecture::X86_64);
        assert!(report.is_dll);
        assert!(report.pe32_plus);
        assert!(report.summary().contains("execution is not provided"));
    }

    #[test]
    fn identifies_arm64_pe() {
        let bytes = minimal_pe(0xaa64, 0x20b, false);
        let report = inspect_bytes(&bytes).unwrap();
        assert_eq!(report.architecture, Architecture::Arm64);
        assert!(!report.is_dll);
    }

    #[test]
    fn rejects_non_pe_and_truncated_headers() {
        assert!(inspect_bytes(b"not a PE file").is_err());
        let mut bytes = minimal_pe(0x8664, 0x20b, true);
        bytes.truncate(0x50);
        assert!(inspect_bytes(&bytes).is_err());
    }
}
