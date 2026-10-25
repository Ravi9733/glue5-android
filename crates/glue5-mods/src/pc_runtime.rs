//! PC plugin inspection exposed to Glue 5's mod manager.
//!
//! This is diagnostics only. It deliberately does not load Windows DLL/ASI
//! code. A file must not be shown as supported solely because its format is
//! recognized.

pub use glue5_pc_mod_runtime::{Architecture, BinaryFormat, Compatibility, Inspection};

/// Inspect a plugin's binary header before installation or execution.
///
/// This does not execute the file. Windows PC plugins remain unsupported until
/// a compatible PE/ABI runtime and the required GTA/ScriptHook interfaces exist.
pub fn inspect(bytes: &[u8]) -> Inspection {
    glue5_pc_mod_runtime::inspect(bytes)
}

/// A short UI-ready status label.
pub fn status_text(inspection: &Inspection) -> String {
    match &inspection.compatibility {
        Compatibility::WindowsPeNeedsCompatibilityLayer { architecture } =>
            format!("Unsupported Windows PC plugin ({architecture:?}): compatibility runtime/API bridge is not available."),
        Compatibility::NativeLibraryNeedsGlueApi { architecture } =>
            format!("Native library ({architecture:?}) detected, but it is not a registered Glue 5 plugin."),
        Compatibility::WasmNeedsPluginAbi { architecture } =>
            format!("WASM ({architecture:?}) detected, but no PC-mod plugin ABI is registered."),
        Compatibility::UnrecognizedBinary =>
            "Unknown or truncated plugin binary; it will not be loaded.".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_windows_asi_as_unsupported_without_running_it() {
        let mut bytes = vec![0u8; 0x40];
        bytes[0..2].copy_from_slice(b"MZ");
        bytes[0x3c..0x40].copy_from_slice(&(0x40u32).to_le_bytes());
        bytes.extend_from_slice(b"PE\0\0");
        bytes.extend_from_slice(&0x8664u16.to_le_bytes());
        let result = inspect(&bytes);
        assert_eq!(result.format, BinaryFormat::Pe);
        assert!(status_text(&result).contains("Unsupported Windows PC plugin"));
    }
}


/// Inspect a Windows plugin's declared DLL dependencies without loading it.
/// This helps plan the required API shims; it is not an executable loader.
pub fn inspect_pe_dependencies(bytes: &[u8]) -> Result<glue5_pc_mod_runtime::PeImportReport, glue5_pc_mod_runtime::PeParseError> {
    glue5_pc_mod_runtime::inspect_pe_imports(bytes)
}
