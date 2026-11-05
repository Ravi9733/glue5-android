//! Pure driver identity classification, kept separate so CI can test it
//! without compiling the full renderer or requiring the proprietary game.wasm.

pub(super) fn contains_turnip_identity(value: &str) -> bool {
    let value = value.to_ascii_lowercase();
    value.contains("turnip") || value.contains("freedreno") || value.contains("mesa")
}

#[cfg(test)]
mod tests {
    use super::contains_turnip_identity;

    #[test]
    fn recognizes_turnip_and_freedreno_driver_strings() {
        assert!(contains_turnip_identity("Turnip (Mesa 24.3)"));
        assert!(contains_turnip_identity("freedreno"));
        assert!(contains_turnip_identity("Mesa RADV"));
    }

    #[test]
    fn does_not_mislabel_other_vulkan_drivers_as_turnip() {
        assert!(!contains_turnip_identity("Qualcomm Adreno Vulkan"));
        assert!(!contains_turnip_identity("SwiftShader"));
        assert!(!contains_turnip_identity("NVIDIA"));
    }
}
