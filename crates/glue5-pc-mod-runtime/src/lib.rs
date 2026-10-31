//! Static PC-plugin inspection only. This crate never executes native code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinaryFormat { Pe, Elf, MachO, Wasm, Unknown }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Architecture { X86, X86_64, Arm32, Arm64, Wasm32, Unknown }
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Compatibility {
    WindowsPeNeedsCompatibilityLayer { architecture: Architecture },
    NativeLibraryNeedsGlueApi { architecture: Architecture },
    WasmNeedsPluginAbi { architecture: Architecture },
    UnrecognizedBinary,
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
fn detect(b: &[u8]) -> (BinaryFormat, Architecture) {
    if b.len() >= 0x40 && b.starts_with(b"MZ") {
        let p = u32::from_le_bytes([b[0x3c],b[0x3d],b[0x3e],b[0x3f]]) as usize;
        if let Some(h) = b.get(p..p.saturating_add(6)) {
            if h.len()==6 && &h[..4]==b"PE\0\0" {
                let m=u16::from_le_bytes([h[4],h[5]]);
                return (BinaryFormat::Pe, match m {0x14c=>Architecture::X86,0x8664=>Architecture::X86_64,0x1c0|0x1c4=>Architecture::Arm32,0xaa64=>Architecture::Arm64,_=>Architecture::Unknown});
            }
        }
        return (BinaryFormat::Unknown,Architecture::Unknown);
    }
    if b.starts_with(b"\x7fELF") && b.len()>=20 {
        let m=u16::from_le_bytes([b[18],b[19]]);
        return (BinaryFormat::Elf,match (b[4],m) {(1,3)=>Architecture::X86,(2,62)=>Architecture::X86_64,(1,40)=>Architecture::Arm32,(2,183)=>Architecture::Arm64,_=>Architecture::Unknown});
    }
    if b.starts_with(b"\0asm") { return (BinaryFormat::Wasm,Architecture::Wasm32); }
    if b.len()>=4 {
        let m=u32::from_be_bytes([b[0],b[1],b[2],b[3]]);
        if matches!(m,0xfeedface|0xcefaedfe|0xfeedfacf|0xcffaedfe|0xfeedfece|0xcefeedfe) {
            return (BinaryFormat::MachO,if matches!(m,0xfeedfacf|0xcffaedfe){Architecture::X86_64}else{Architecture::Unknown});
        }
    }
    (BinaryFormat::Unknown,Architecture::Unknown)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn identifies_windows_x64_without_claiming_support() {
        let mut b=vec![0u8;0x40]; b[..2].copy_from_slice(b"MZ");
        b[0x3c..0x40].copy_from_slice(&0x40u32.to_le_bytes());
        b.extend_from_slice(b"PE\0\0"); b.extend_from_slice(&0x8664u16.to_le_bytes());
        let x=inspect(&b); assert_eq!(x.format,BinaryFormat::Pe);
        assert_eq!(x.architecture,Architecture::X86_64);
        assert!(matches!(x.compatibility,Compatibility::WindowsPeNeedsCompatibilityLayer{..}));
    }
    #[test] fn rejects_unknown_data() { assert_eq!(inspect(&[]).compatibility,Compatibility::UnrecognizedBinary); }
}
