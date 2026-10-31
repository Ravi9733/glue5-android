//! Diagnostics for PC plugin files; no native plugin is loaded or executed.
pub use glue5_pc_mod_runtime::{Architecture, BinaryFormat, Compatibility, Inspection};
pub fn inspect(bytes: &[u8]) -> Inspection { glue5_pc_mod_runtime::inspect(bytes) }
pub fn status_text(x: &Inspection) -> String {
    match x.compatibility {
        Compatibility::WindowsPeNeedsCompatibilityLayer { architecture } =>
            format!("Unsupported Windows PC plugin ({architecture:?}): PE compatibility runtime and GTA/ScriptHook API bridge are not available."),
        Compatibility::NativeLibraryNeedsGlueApi { architecture } =>
            format!("Native library ({architecture:?}) detected, but no Glue5 plugin ABI is registered."),
        Compatibility::WasmNeedsPluginAbi { architecture } =>
            format!("WASM ({architecture:?}) detected, but no PC plugin ABI is registered."),
        Compatibility::UnrecognizedBinary => "Unknown or truncated plugin binary; it will not be loaded.".to_owned(),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn labels_pe_as_unsupported() {
        let mut b=vec![0u8;0x40]; b[..2].copy_from_slice(b"MZ");
        b[0x3c..0x40].copy_from_slice(&0x40u32.to_le_bytes());
        b.extend_from_slice(b"PE\0\0"); b.extend_from_slice(&0x8664u16.to_le_bytes());
        assert!(status_text(&inspect(&b)).contains("Unsupported Windows PC plugin"));
    }
}
