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
