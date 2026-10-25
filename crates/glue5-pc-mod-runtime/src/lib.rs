//! PC-mod runtime foundation for Glue 5.
//!
//! This crate deliberately does not execute native plugin code yet. It
//! identifies plugin binary formats and architectures before any loader is
//! attempted. GTA V WASM builds are not Windows GTA V processes: PE plugins
//! need a compatibility environment and a ScriptHookV-compatible API bridge.
//! Never treat successful file detection as plugin compatibility.

use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinaryFormat { Pe, Elf, MachO, Wasm, Unknown }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Architecture { X86, X86_64, Arm32, Arm64, Wasm32, Wasm64, Unknown }

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Compatibility {
    WindowsPeNeedsCompatibilityLayer { architecture: Architecture },
    NativeLibraryNeedsGlueApi { architecture: Architecture },
    WasmNeedsPluginAbi { architecture: Architecture },
    UnrecognizedBinary,
}

impl fmt::Display for Compatibility {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WindowsPeNeedsCompatibilityLayer { architecture } =>
                write!(f, "Windows PE ({architecture:?}) needs a Windows compatibility layer and the required GTA/ScriptHook API"),
            Self::NativeLibraryNeedsGlueApi { architecture } =>
                write!(f, "native library ({architecture:?}) has no registered Glue 5 plugin ABI"),
            Self::WasmNeedsPluginAbi { architecture } =>
                write!(f, "WASM ({architecture:?}) has no registered PC-mod plugin ABI"),
            Self::UnrecognizedBinary => write!(f, "unrecognized or truncated binary"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Inspection {
    pub format: BinaryFormat,
    pub architecture: Architecture,
    pub compatibility: Compatibility,
}

pub fn inspect(bytes: &[u8]) -> Inspection {
    let (format, architecture) = detect(bytes);
    let compatibility = match format {
        BinaryFormat::Pe => Compatibility::WindowsPeNeedsCompatibilityLayer { architecture },
        BinaryFormat::Elf | BinaryFormat::MachO => Compatibility::NativeLibraryNeedsGlueApi { architecture },
        BinaryFormat::Wasm => Compatibility::WasmNeedsPluginAbi { architecture },
        BinaryFormat::Unknown => Compatibility::UnrecognizedBinary,
    };
    Inspection { format, architecture, compatibility }
}

fn detect(bytes: &[u8]) -> (BinaryFormat, Architecture) {
    if bytes.len() >= 0x40 && bytes.starts_with(b"MZ") {
        let offset = u32::from_le_bytes(bytes[0x3c..0x40].try_into().unwrap()) as usize;
        if let Some(sig) = bytes.get(offset..offset.saturating_add(6)) {
            if sig.len() == 6 && &sig[..4] == b"PE\0\0" {
                let machine = u16::from_le_bytes([sig[4], sig[5]]);
                let arch = match machine {
                    0x014c => Architecture::X86,
                    0x8664 => Architecture::X86_64,
                    0x01c0 | 0x01c4 => Architecture::Arm32,
                    0xaa64 => Architecture::Arm64,
                    _ => Architecture::Unknown,
                };
                return (BinaryFormat::Pe, arch);
            }
        }
        return (BinaryFormat::Unknown, Architecture::Unknown);
    }
    if bytes.starts_with(b"\x7fELF") && bytes.len() >= 20 {
        let class = bytes[4];
        let endian = bytes[5];
        if endian != 1 { return (BinaryFormat::Elf, Architecture::Unknown); }
        let machine = u16::from_le_bytes([bytes[18], bytes[19]]);
        let arch = match (class, machine) {
            (1, 3) => Architecture::X86,
            (2, 62) => Architecture::X86_64,
            (1, 40) => Architecture::Arm32,
            (2, 183) => Architecture::Arm64,
            _ => Architecture::Unknown,
        };
        return (BinaryFormat::Elf, arch);
    }
    if bytes.starts_with(b"\0asm") {
        return (BinaryFormat::Wasm, if bytes.get(4) == Some(&1) { Architecture::Wasm32 } else { Architecture::Unknown });
    }
    if bytes.len() >= 4 {
        let magic = u32::from_be_bytes(bytes[..4].try_into().unwrap());
        let arch = match magic {
            0xfeedface | 0xcefaedfe => Architecture::X86,
            0xfeedfacf | 0xcffaedfe => Architecture::X86_64,
            0xfeedfece | 0xcefeedfe => Architecture::Arm32,
            _ => Architecture::Unknown,
        };
        if matches!(magic, 0xfeedface | 0xcefaedfe | 0xfeedfacf | 0xcffaedfe | 0xfeedfece | 0xcefeedfe) {
            return (BinaryFormat::MachO, arch);
        }
    }
    (BinaryFormat::Unknown, Architecture::Unknown)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn identifies_pe_x64_as_not_yet_loadable() {
        let mut bytes = vec![0u8; 0x40];
        bytes[0..2].copy_from_slice(b"MZ");
        bytes[0x3c..0x40].copy_from_slice(&(0x40u32).to_le_bytes());
        bytes.extend_from_slice(b"PE\0\0");
        bytes.extend_from_slice(&0x8664u16.to_le_bytes());
        let got = inspect(&bytes);
        assert_eq!(got.format, BinaryFormat::Pe);
        assert_eq!(got.architecture, Architecture::X86_64);
        assert!(matches!(got.compatibility, Compatibility::WindowsPeNeedsCompatibilityLayer { .. }));
    }
    #[test]
    fn rejects_empty_and_truncated_data() {
        assert_eq!(inspect(&[]).compatibility, Compatibility::UnrecognizedBinary);
        assert_eq!(inspect(b"MZ").format, BinaryFormat::Unknown);
    }
    #[test]
    fn recognizes_wasm_but_does_not_claim_pc_abi() {
        let got = inspect(b"\0asm\x01\0\0\0");
        assert_eq!(got.format, BinaryFormat::Wasm);
        assert_eq!(got.compatibility, Compatibility::WasmNeedsPluginAbi { architecture: Architecture::Wasm32 });
    }
}


/// Parsed import table for a Windows PE image. This is static analysis only:
/// imports are useful for deciding which compatibility APIs a plugin would
/// require, but parsing them does not make the plugin executable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PeImportReport {
    pub architecture: Architecture,
    pub is_pe32_plus: bool,
    pub imported_dlls: Vec<String>,
    pub uses_scripthookv: bool,
    pub uses_dotnet_bridge: bool,
    pub has_common_proxy_loader: bool,
}

/// Errors while safely reading a PE import table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PeParseError {
    NotPe,
    Truncated,
    UnsupportedOptionalHeader,
    InvalidSectionTable,
    InvalidRva,
    InvalidImportTable,
}

impl fmt::Display for PeParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", match self {
            Self::NotPe => "not a Windows PE image",
            Self::Truncated => "truncated PE image",
            Self::UnsupportedOptionalHeader => "unsupported PE optional-header format",
            Self::InvalidSectionTable => "invalid PE section table",
            Self::InvalidRva => "PE data directory points outside mapped sections",
            Self::InvalidImportTable => "malformed PE import table",
        })
    }
}

fn pe_u16(bytes: &[u8], at: usize) -> Result<u16, PeParseError> {
    let b = bytes.get(at..at.checked_add(2).ok_or(PeParseError::Truncated)?)
        .ok_or(PeParseError::Truncated)?;
    Ok(u16::from_le_bytes([b[0], b[1]]))
}
fn pe_u32(bytes: &[u8], at: usize) -> Result<u32, PeParseError> {
    let b = bytes.get(at..at.checked_add(4).ok_or(PeParseError::Truncated)?)
        .ok_or(PeParseError::Truncated)?;
    Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}
fn pe_c_string(bytes: &[u8], at: usize) -> Result<String, PeParseError> {
    let tail = bytes.get(at..).ok_or(PeParseError::Truncated)?;
    let end = tail.iter().position(|b| *b == 0).ok_or(PeParseError::Truncated)?;
    if end == 0 { return Err(PeParseError::InvalidImportTable); }
    Ok(String::from_utf8_lossy(&tail[..end]).into_owned())
}

/// Read imported DLL names from a PE32/PE32+ file without loading or executing it.
/// The parser rejects malformed offsets and never follows arbitrary pointers.
pub fn inspect_pe_imports(bytes: &[u8]) -> Result<PeImportReport, PeParseError> {
    if bytes.len() < 0x40 || !bytes.starts_with(b"MZ") { return Err(PeParseError::NotPe); }
    let pe_at = pe_u32(bytes, 0x3c)? as usize;
    if bytes.get(pe_at..pe_at.saturating_add(4)) != Some(b"PE\0\0".as_slice()) {
        return Err(PeParseError::NotPe);
    }
    let coff = pe_at.checked_add(4).ok_or(PeParseError::Truncated)?;
    let machine = pe_u16(bytes, coff)?;
    let sections = pe_u16(bytes, coff + 2)? as usize;
    let optional_size = pe_u16(bytes, coff + 16)? as usize;
    let optional = coff.checked_add(20).ok_or(PeParseError::Truncated)?;
    let magic = pe_u16(bytes, optional)?;
    let (is_pe32_plus, directory_offset, number_directories_offset) = match magic {
        0x10b => (false, 96usize, 92usize),
        0x20b => (true, 112usize, 108usize),
        _ => return Err(PeParseError::UnsupportedOptionalHeader),
    };
    if optional_size < directory_offset + 16 { return Err(PeParseError::Truncated); }
    let directory_count = pe_u32(bytes, optional + number_directories_offset)?;
    let (import_rva, import_size) = if directory_count > 1 {
        (pe_u32(bytes, optional + directory_offset + 8)?, pe_u32(bytes, optional + directory_offset + 12)?)
    } else { (0, 0) };
    let section_table = optional.checked_add(optional_size).ok_or(PeParseError::Truncated)?;
    let section_bytes = sections.checked_mul(40).ok_or(PeParseError::InvalidSectionTable)?;
    if section_table.checked_add(section_bytes).filter(|end| *end <= bytes.len()).is_none() {
        return Err(PeParseError::InvalidSectionTable);
    }

    // Map an RVA only through a section's raw file data, not virtual padding.
    let rva_offset = |rva: u32| -> Result<usize, PeParseError> {
        for i in 0..sections {
            let s = section_table + i * 40;
            let virtual_size = pe_u32(bytes, s + 8)? as u64;
            let virtual_address = pe_u32(bytes, s + 12)? as u64;
            let raw_size = pe_u32(bytes, s + 16)? as u64;
            let raw_pointer = pe_u32(bytes, s + 20)? as u64;
            let delta = (rva as u64).checked_sub(virtual_address);
            if let Some(delta) = delta {
                if delta < virtual_size.max(raw_size) && delta < raw_size {
                    let pos = raw_pointer.checked_add(delta).ok_or(PeParseError::InvalidRva)?;
                    let pos = usize::try_from(pos).map_err(|_| PeParseError::InvalidRva)?;
                    if pos < bytes.len() { return Ok(pos); }
                }
            }
        }
        Err(PeParseError::InvalidRva)
    };

    let mut imported_dlls = Vec::new();
    if import_rva != 0 && import_size != 0 {
        let mut descriptor = rva_offset(import_rva)?;
        let table_end = descriptor.checked_add(import_size as usize).ok_or(PeParseError::InvalidImportTable)?;
        let mut terminated = false;
        while descriptor.checked_add(20).filter(|end| *end <= table_end && *end <= bytes.len()).is_some() {
            let original_first_thunk = pe_u32(bytes, descriptor)?;
            let timestamp = pe_u32(bytes, descriptor + 4)?;
            let forwarder_chain = pe_u32(bytes, descriptor + 8)?;
            let name_rva = pe_u32(bytes, descriptor + 12)?;
            let first_thunk = pe_u32(bytes, descriptor + 16)?;
            if original_first_thunk == 0 && timestamp == 0 && forwarder_chain == 0 && name_rva == 0 && first_thunk == 0 {
                terminated = true;
                break;
            }
            if name_rva == 0 { return Err(PeParseError::InvalidImportTable); }
            imported_dlls.push(pe_c_string(bytes, rva_offset(name_rva)?)?);
            descriptor = descriptor.checked_add(20).ok_or(PeParseError::InvalidImportTable)?;
        }
        if !terminated { return Err(PeParseError::InvalidImportTable); }
    }

    let lower: Vec<String> = imported_dlls.iter().map(|s| s.to_ascii_lowercase()).collect();
    let uses_scripthookv = lower.iter().any(|s| s == "scripthookv.dll" || s == "scripthookv.lib");
    let uses_dotnet_bridge = lower.iter().any(|s| s.contains("scriptshvdn") || s == "clr.dll" || s == "mscoree.dll");
    let has_common_proxy_loader = lower.iter().any(|s| matches!(s.as_str(), "dinput8.dll" | "version.dll" | "dsound.dll" | "winmm.dll"));
    let architecture = match machine {
        0x014c => Architecture::X86,
        0x8664 => Architecture::X86_64,
        0x01c0 | 0x01c4 => Architecture::Arm32,
        0xaa64 => Architecture::Arm64,
        _ => Architecture::Unknown,
    };
    Ok(PeImportReport { architecture, is_pe32_plus, imported_dlls, uses_scripthookv, uses_dotnet_bridge, has_common_proxy_loader })
}

#[cfg(test)]
mod pe_import_tests {
    use super::*;

    #[test]
    fn rejects_non_pe_and_truncated_images_without_panicking() {
        assert_eq!(inspect_pe_imports(b"").unwrap_err(), PeParseError::NotPe);
        let mut b = vec![0u8; 0x40];
        b[..2].copy_from_slice(b"MZ");
        b[0x3c..0x40].copy_from_slice(&0x1000u32.to_le_bytes());
        assert_eq!(inspect_pe_imports(&b).unwrap_err(), PeParseError::NotPe);
    }

    #[test]
    fn recognizes_pe64_without_claiming_it_can_run() {
        let mut b = vec![0u8; 0x40];
        b[..2].copy_from_slice(b"MZ");
        b[0x3c..0x40].copy_from_slice(&0x40u32.to_le_bytes());
        b.extend_from_slice(b"PE\0\0");
        b.extend_from_slice(&0x8664u16.to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes()); // sections
        b.extend_from_slice(&[0u8; 12]);
        b.extend_from_slice(&0xf0u16.to_le_bytes()); // optional header size
        b.extend_from_slice(&[0u8; 2]); // characteristics
        let optional = b.len();
        b.extend_from_slice(&0x20bu16.to_le_bytes());
        b.resize(optional + 0xf0, 0);
        // One empty section header; no imports.
        b.extend_from_slice(&[0u8; 40]);
        let report = inspect_pe_imports(&b).unwrap();
        assert_eq!(report.architecture, Architecture::X86_64);
        assert!(report.is_pe32_plus);
        assert!(report.imported_dlls.is_empty());
        assert!(!report.uses_scripthookv);
    }
}


/// Coarse dependency groups inferred from PE import names. These groups help
/// identify missing compatibility work; they are not an API-call trace.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DependencyGroup {
    ScriptHookV,
    DotNetRuntime,
    WindowsKernel,
    WindowsUi,
    CRunTime,
    Graphics,
    Audio,
    Input,
    UnknownWindowsDll,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompatibilityPlan {
    pub architecture: Architecture,
    pub groups: Vec<DependencyGroup>,
    pub blockers: Vec<String>,
    /// Always false until an actual PE execution engine and GTA API bridge exist.
    pub executable_here: bool,
}

/// Build a conservative checklist from a static import report. It intentionally
/// reports prerequisites instead of guessing that the DLL is compatible.
pub fn plan_compatibility(report: &PeImportReport) -> CompatibilityPlan {
    let mut groups = Vec::new();
    let mut blockers = Vec::new();
    let mut add = |group, blocker: &str| {
        if !groups.contains(&group) { groups.push(group); }
        if !blockers.iter().any(|b| b == blocker) { blockers.push(blocker.to_owned()); }
    };
    for dll in &report.imported_dlls {
        let name = dll.to_ascii_lowercase();
        match name.as_str() {
            "scripthookv.dll" | "scripthookv.lib" => add(
                DependencyGroup::ScriptHookV,
                "ScriptHookV exports and lifecycle are not implemented for Glue5 WASM",
            ),
            "clr.dll" | "mscoree.dll" | "scriptshvdn.dll" | "scriptshvdn2.dll" => add(
                DependencyGroup::DotNetRuntime,
                ".NET/SHVDN runtime and managed plugin bridge are not available",
            ),
            "kernel32.dll" | "kernelbase.dll" | "ntdll.dll" => add(
                DependencyGroup::WindowsKernel,
                "Windows process, memory, thread, and filesystem APIs need compatible shims",
            ),
            "user32.dll" | "comdlg32.dll" | "gdi32.dll" => add(
                DependencyGroup::WindowsUi,
                "Windows UI APIs have no Android trainer-window equivalent; controls must be ported",
            ),
            "msvcrt.dll" | "ucrtbase.dll" | "vcruntime140.dll" | "msvcp140.dll" => add(
                DependencyGroup::CRunTime,
                "Windows C/C++ runtime ABI and exception handling are not provided",
            ),
            "d3d11.dll" | "d3d12.dll" | "dxgi.dll" | "d3dcompiler_47.dll" => add(
                DependencyGroup::Graphics,
                "Direct3D/DXGI calls cannot target Glue5's Vulkan/WASM renderer directly",
            ),
            "dsound.dll" | "xaudio2_7.dll" | "xinput1_3.dll" | "xinput1_4.dll" => add(
                if name.starts_with("xinput") { DependencyGroup::Input } else { DependencyGroup::Audio },
                "Windows audio/input API compatibility and device mapping are not implemented",
            ),
            "dinput8.dll" | "winmm.dll" | "version.dll" => add(
                DependencyGroup::Input,
                "Proxy-DLL presence does not prove the plugin is self-contained; proxy loading is not implemented",
            ),
            other if other.ends_with(".dll") => add(
                DependencyGroup::UnknownWindowsDll,
                "one or more imported Windows/game DLLs need individual ABI and behavior analysis",
            ),
            _ => {}
        }
    }
    if report.architecture != Architecture::X86_64 {
        blockers.push("most modern GTA V PC plugins target Windows x64; this file is not reported as x64".to_owned());
    }
    if blockers.is_empty() {
        blockers.push("the import table alone is insufficient: exports, delay-loads, runtime behavior, and GTA API use still need analysis".to_owned());
    }
    CompatibilityPlan { architecture: report.architecture, groups, blockers, executable_here: false }
}

#[cfg(test)]
mod compatibility_plan_tests {
    use super::*;

    fn report(imported_dlls: &[&str]) -> PeImportReport {
        PeImportReport {
            architecture: Architecture::X86_64,
            is_pe32_plus: true,
            imported_dlls: imported_dlls.iter().map(|s| (*s).to_owned()).collect(),
            uses_scripthookv: imported_dlls.iter().any(|s| s.eq_ignore_ascii_case("ScriptHookV.dll")),
            uses_dotnet_bridge: imported_dlls.iter().any(|s| s.eq_ignore_ascii_case("mscoree.dll")),
            has_common_proxy_loader: false,
        }
    }

    #[test]
    fn plan_calls_out_scripthook_and_windows_api_blockers() {
        let plan = plan_compatibility(&report(&["ScriptHookV.dll", "kernel32.dll", "user32.dll"]));
        assert!(plan.groups.contains(&DependencyGroup::ScriptHookV));
        assert!(plan.groups.contains(&DependencyGroup::WindowsKernel));
        assert!(plan.groups.contains(&DependencyGroup::WindowsUi));
        assert!(!plan.executable_here);
        assert!(plan.blockers.iter().any(|b| b.contains("ScriptHookV")));
    }

    #[test]
    fn empty_imports_do_not_mean_compatible() {
        let plan = plan_compatibility(&report(&[]));
        assert!(!plan.executable_here);
        assert!(!plan.blockers.is_empty());
    }
}
