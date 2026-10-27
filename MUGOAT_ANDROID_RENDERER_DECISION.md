# Mugoat Android renderer decision (2026-10-27)

## Important source finding

The upstream script `native-port/app/Scripts/build_metal_shaders.py` documents that the game data already contains shader source files as `<hash>.wgsl`, an `index.json`, and optional `.consts.json` files. That script translates those WGSL files into Metal only for the iOS app.

Glue5's existing `crates/glue5-gpu/src/shader.rs` also loads `<hash>.wgsl` and compiles it through wgpu. Glue5 selects Vulkan on non-Apple platforms, including Android.

**Implication:** Do not assume a Metal-to-SPIR-V converter is required. The shortest plausible route is to reuse the existing Glue5 wgpu/Vulkan renderer and validate Mugoat's WGSL index, shader hashes, reflection/bindings, and command payloads against it.

## What this does and does not prove

- Source inspected: upstream `build_metal_shaders.py`; Glue5 `shader.rs`, `gpu.rs`, and `exec.rs`.
- The engine command ring uses the same 64-word data offset and numeric engine opcode IDs as Glue5.
- This is a promising reuse path, **not** proof of shader ABI or payload compatibility.
- It does not produce an APK or prove that Mugoat gameplay renders on Android.

## Required next implementation steps

1. Add an automated compatibility test that opens the real game shader `index.json` and validates representative vertex, pixel, and compute WGSL programs with the same wgpu/Naga path used by Glue5.
2. Compare Mugoat command payload lengths and field meanings with Glue5's executor, not just opcode numbers.
3. Connect the Mugoat host/runtime to Glue5's Android Vulkan/wgpu path without sending Glue5-only host opcodes to the upstream engine.
4. Build the Android APK in GitHub Actions and verify APK/native ABI contents.
5. Test launch, shader compilation, first rendered frame, input, audio, memory use, and stability on an actual Android device.

Until steps 1–5 pass, no APK should be described as a working Mugoat port.
