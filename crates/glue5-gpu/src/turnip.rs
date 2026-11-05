//! Android Vulkan / Mesa Turnip startup diagnostics.
//!
//! This does not load or install a Vulkan ICD. It reports which driver the
//! Vulkan loader actually selected so a build cannot silently claim Turnip
//! support merely because the Vulkan backend was compiled in.

pub(super) fn log_adapter_diagnostics(info: &wgpu::AdapterInfo) {
    let driver_identity = format!("{} {}", info.driver, info.driver_info);
    let turnip_identity = contains_turnip_identity(&driver_identity);
    let backend_is_vulkan = info.backend == wgpu::Backend::Vulkan;

    log::info!(
        "gpu: Vulkan/Turnip probe: backend={:?}, adapter={}, driver={}, driver_info={}, vendor={:#06x}, device={:#06x}",
        info.backend,
        info.name,
        info.driver,
        info.driver_info,
        info.vendor,
        info.device,
    );

    if !backend_is_vulkan {
        log::warn!("gpu: Turnip unavailable: active adapter is not Vulkan");
        return;
    }

    if turnip_identity {
        log::info!("gpu: Mesa/Turnip-family Vulkan driver identity detected");
    } else {
        log::warn!(
            "gpu: Vulkan is active, but Turnip identity was not confirmed; this is not a Turnip verification"
        );
    }

    for key in ["VK_DRIVER_FILES", "VK_ICD_FILENAMES", "VK_LAYER_PATH", "VK_INSTANCE_LAYERS"] {
        let configured = std::env::var_os(key).is_some_and(|value| !value.is_empty());
        log::info!("gpu: {key} configured={configured}");
    }
}

#[cfg(test)]
mod tests {
    use super::turnip_identity::contains_turnip_identity;

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
