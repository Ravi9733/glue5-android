# Live PC placement renderer integration

## Current boundary

The PC placement pipeline currently resolves JSON placement references, inspects/decompresses supported resources, assembles CPU meshes and uploads bounded vertex/index buffers through the active Glue5 GPU device. Buffer upload is not rendering: the uploaded buffers are not yet connected to a frame draw list, material/shader binding, transforms, depth state, or world/entity lifetime.

## Required renderer integration

1. Keep all wgpu object creation, ownership and destruction on the renderer's GPU thread. Do not move raw wgpu handles into the game/WASM thread.
2. Introduce a renderer-owned external-placement resource registry with bounded mesh count, byte budget, stable IDs, and explicit unload/replacement semantics.
3. Queue placement commands from the game/runtime side; consume them on the GPU command thread at a defined frame boundary.
4. Define a documented vertex ABI and matching wgpu vertex layouts. Gate GTA-derived layouts on the retail fixture validation described in `PC_VERTEX_DECLARATION_FIXTURES.md`.
5. Resolve world transform, index format, material/texture, blend mode, culling and depth behavior. Missing or unsupported materials must produce a visible diagnostic/fallback, not undefined bindings.
6. Add an isolated render pass/draw path that cannot corrupt the existing translated GTA command stream or presentation/upscaler passes.
7. Verify with a visible, user-owned test mesh and capture a frame on Android Vulkan; then repeat on the supported Turnip driver.

## Acceptance gate

Do not mark placement rendering complete because buffers upload successfully. Completion requires a visible in-game placement, correct transform and index count, bounded resource unload, no validation errors, and a captured device test. Until then the feature remains preparation/upload only.
