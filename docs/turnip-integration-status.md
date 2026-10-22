# Turnip driver integration status

This branch targets optional Turnip support on Android Adreno GPUs. The working renderer currently creates a standard wgpu Vulkan instance; selecting a driver in the UI alone would not switch Vulkan implementations.

## Integration requirements

- Keep the system Vulkan path as the default and fallback.
- Package AdrenoTools and its hook libraries for `arm64-v8a`, with Android legacy JNI library packaging enabled.
- Let the user import a compatible Turnip driver archive into app-private internal storage. Never attempt to load a driver library directly from shared/external storage.
- Validate archive paths and expected library names before extracting; reject path traversal and unsupported ABIs.
- Retain the native driver handle for the lifetime of Vulkan objects.
- Route wgpu-hal's Vulkan entry creation through the custom `vkGetInstanceProcAddr` only when the user has selected a successfully installed driver; otherwise preserve `ash::Entry::load()`.
- Expose status and an explicit system-driver fallback in Settings. Do not promise FPS gains: results depend on driver version, device firmware, and the game workload.

## Current status

The app already uses `useLegacyPackaging = true`, which AdrenoTools requires for its hook library path. The actual driver loader, archive importer, Settings selector, and device validation are not yet implemented. This note deliberately does not label Turnip as working until the custom Vulkan entry path is wired and tested on an Adreno device.
