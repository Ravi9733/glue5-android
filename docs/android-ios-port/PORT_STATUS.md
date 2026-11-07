# Android port of the public iOS GTA V port

This work is isolated on `android-ios-port`, based on `turnip-driver-integration`.
The original `main` branch and the existing Turnip integration branch are not modified.

## Confirmed compatibility

- The public iOS project is version 1.5.2 (from `app/project.yml`).
- The iOS project and Glue 5 build script target the same game build ID:
  `8b0b5899ed`.
- The expected game.wasm SHA-256 is:
  `11ca8d2c04c5e843d18ff4aea4899d72c86973c6b031df334e67c446b2ae83e0`.
- Glue 5's `glue5-game` build script already translates this wasm with its patched
  `wasm2c`, generates Rust bindings, and compiles the generated C into a native
  static library. This is the engine build path to reuse on Android.
- Glue 5's Android platform already targets arm64 and Vulkan. The existing GPU
  backend remains the intended renderer; the iOS Metal renderer is not being
  transplanted.

## Port boundaries

1. **Engine ABI:** retain the matching generated game module and generated
   `Imports` / `Hooks` boundary from `glue5-game`.
2. **Host services:** port the iOS host behavior to Android equivalents:
   filesystem/WASI syscalls, thread creation and futexes, logging, memory
   budgeting, asset and mod path resolution, audio, input/controller state,
   and lifecycle.
3. **Graphics:** feed the same game GPU command stream into Glue 5's existing
   Vulkan/`wgpu` renderer. Keep this behind a small adapter and verify opcode,
   buffer-layout, synchronization, and resource-lifetime assumptions.
4. **Application layer:** Android-native UI for Story/Sandbox, settings,
   touch-control overlays, mod installation, and asset selection.
5. **Validation:** build arm64 APK, verify startup and world loading, then test
   audio, input, saves, asset streaming, and mods before performance tuning.

## Important source constraints

- The iOS host uses Apple-only APIs in several areas (for example process memory
  reporting, QoS, Metal, UIKit/AVFAudio and Apple filesystem structure). These
  must be replaced, not compiled unchanged for Android.
- The iOS source uses native libraries and generated artifacts (including
  wasm2c/WABT, Opus, archive handling and Blink). Android build dependencies
  and ABI availability must be verified individually.
- A shared game build ID does not by itself prove that every host import,
  structure layout, shader/resource contract, or runtime behavior is identical.

## Current status

- [x] Isolated working branch created from `turnip-driver-integration`.
- [x] Matching game build ID and wasm hash verified in both projects.
- [x] Reuse of Glue 5's native wasm2c build path identified.
- [ ] Complete import/host-service compatibility matrix.
- [ ] Android host implementation.
- [ ] Renderer adapter and GPU command validation.
- [ ] Android UI and mod/asset integration.
- [ ] Successful APK build and device validation.

Do not describe this port as built or working until the final validation items pass.
