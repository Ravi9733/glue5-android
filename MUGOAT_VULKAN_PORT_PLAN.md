# Mugoat Vulkan renderer port — implementation boundary

This file defines the concrete boundary for replacing the current Metal renderer. It is intentionally separate from the upstream iOS submodule so the pinned upstream source remains reproducible.

## Required integration contract

The upstream engine currently calls `PGStartGraphics(memory, memorySize, ring, ringSize, debug)` and expects the native renderer to consume the D3D11-style GPU command ring. The ring framing and opcode numbers overlap with Glue5's implementation, but that alone does not make the renderers interchangeable: payload layouts, shader ABI, resource formats, synchronization and host lifecycle must also match.

### Android implementation modules

1. **`MugoatVulkanHost` (C ABI)** — Android implementation of the graphics-host exports currently declared in `NativeGraphics.h`. It must preserve the engine's expected ABI and lifecycle; it must not export a Metal-layer function on Android.
2. **Ring bridge** — bounds-checked reader over the engine's shared memory and ring offsets. It must preserve WRAP semantics, validate payload word counts and update producer-visible head/tail/error counters safely.
3. **Vulkan device/swapchain** — create instance/device/queues from Android's `ANativeWindow`, choose surface formats/present mode, handle `VK_ERROR_OUT_OF_DATE_KHR`, resize and teardown.
4. **D3D11-command translation** — implement every engine opcode used by this pinned build. Start with resource creation/uploads, shader creation, render-target setup, clears, draws, copies, compute, queries and readback. Unsupported opcodes must increment the protocol error counter rather than silently pretending to render.
5. **Shader ABI** — replace the Metal shader-tool output with SPIR-V (or another Vulkan-compatible path) while preserving binding/register mappings, reflection and specialization constants expected by the command translator.
6. **Host/runtime integration** — Android lifecycle, input, audio, asset paths and threads must be implemented independently of UIKit/Metal/Mach APIs.

## Compatibility gates

- Ring parser unit tests cover empty ring, wrap, truncated command, oversized payload and all opcode IDs.
- Shader fixtures compile to SPIR-V and reflection is compared with the original shader metadata.
- Vulkan validation-layer run reports no descriptor/layout/synchronization errors.
- Android ARM64 CI produces an installable APK.
- Device smoke test verifies startup, first presented frame, resize/pause/resume, audio and a short gameplay session.

## Current state

The pinned upstream still uses `MetalGraphicsHost.mm`, `MetalRenderer` and a Metal-only shader build. The Glue5 GPU crate is a useful protocol reference, not a drop-in C ABI renderer. The repository's audit workflows detect these boundaries; they do not claim the renderer replacement is complete.

Do not publish an APK as a working Mugoat port until all compatibility gates above pass.


## First code slice added

The branch now contains `native-port/android/vulkan/VulkanHost.h/.cpp` and a CMake target. This code creates an Android Vulkan instance/surface, selects a physical device with graphics + presentation support, creates a logical device and swapchain, makes swapchain image views/semaphores, and handles swapchain recreation/cleanup. It is a real platform-layer starting point, **not yet the Mugoat renderer replacement**: it does not consume the engine's command ring, translate draw/compute commands, compile the game's shaders, or present rendered game frames.

A GitHub Actions workflow, `.github/workflows/mugoat-vulkan-host.yml`, compiles this static library for `arm64-v8a` with Android NDK API 29. The next integration work is to add a frame acquisition/submission API, connect it to the engine's ring bridge, implement opcode/resource translation, and replace the Metal shader output with SPIR-V.
