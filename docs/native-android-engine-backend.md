# Native Android Engine Backend (Design Draft)

## Goal

Add an **optional native-engine Android flavor** to Glue 5 that keeps the Glue 5 application shell (launcher UI, settings, touch controls, storage selection and lifecycle surface) while launching a user-supplied native ARM64 Android game engine. This flavor must not require `game.wasm`, `wasm2c`, or generated WASM C.

The existing WASM build remains supported and unchanged. This document describes an integration boundary; it does not claim that the XHYN-V engine is already compatible or that a working game build exists.

## Current contracts

### Glue 5

The current Glue 5 game path is coupled to the WASM build:
- `crates/glue5-game/build.rs` validates `game.wasm`, runs `wasm2c`, applies hooks and compiles generated C.
- `crates/glue5-runtime` supplies Emscripten/WASI imports, threads, VFS, input, audio and a D3D11-style GPU command ring.
- `crates/glue5-gpu` consumes that command ring and renders through wgpu/Vulkan on Android.
- `crates/glue5-mods` derives native/script metadata from the WASM game and the natives database.

Consequently, removing a file check alone is not enough: the WASM-generated game code is the producer of the runtime and GPU contracts.

### XHYN-V native Android port

The XHYN-V public source documentation describes a native ARM64 Android engine loaded as `libgtav.so` through SDL's native entry point. Its graphics path includes DXVK/Vulkan. The public repository is a modification layer and explicitly does not contain the complete original game engine / Android-base source or all exact renderer patches.

This means a Glue 5 integration must treat the native engine as an external, user-provided runtime component and verify its actual exported entry points, JNI/SDL ownership, dependencies and license/redistribution permissions. Do not assume that a similarly named library or undocumented symbol is a stable API.

## Proposed architecture

Keep the application shell separate from the game execution backend:

```text
Glue 5 Android application
  ├── Launcher / UI / settings
  ├── Touch input mapping
  ├── User-selected game data directory
  └── EngineBackend (new boundary)
        ├── WasmBackend (existing, default)
        │     └── wasm2c + Glue 5 runtime + GPU command ring
        └── NativeAndroidBackend (new optional flavor)
              ├── Native engine lifecycle adapter
              ├── Native SDL/JNI entry point (only if verified)
              ├── Native audio/input bridge (only if supported)
              └── Engine-owned graphics path (e.g. its DXVK/Vulkan stack)
```

**Important renderer boundary:** Glue 5's wgpu renderer consumes Glue 5's GPU command ring. A native engine using DXVK does not automatically produce that command ring. The first native prototype should let the native engine own its graphics path; do not try to route native D3D/DXVK calls into the WASM renderer.

## Work plan

### Phase 0 — Interface inventory (no binary assumptions)
1. Record Glue 5 Android Activity, SDL lifecycle, storage, settings, touch input and audio entry points.
2. Record native package ABI, shared-library dependencies, exported symbols and SDL/JNI startup contract from a legally obtained local package.
3. Confirm whether the native engine can be initialized and shut down from the Glue 5 Activity process without competing SDL owners.
4. Document game-data path and permissions. Keep game assets user-supplied and outside the APK.

### Phase 1 — Backend boundary
1. Introduce a small app-level backend selection contract, e.g. `WasmBackend` and `NativeAndroidBackend`, behind Android build flavors/features.
2. Keep WASM-only build tasks and dependencies scoped to the WASM flavor.
3. Make native flavor configuration fail closed with a clear diagnostic if required engine libraries or runtime metadata are missing.
4. Keep the default build on the existing WASM path until native launch passes tests.

### Phase 2 — Native lifecycle prototype
1. Implement only verified engine initialization, foreground/background, surface changes and shutdown hooks.
2. Ensure one owner for SDL, Activity callbacks, audio focus and surface lifetime.
3. Add crash-safe logging and a diagnostic export path.
4. Do not invent native function offsets or patch binary code without exact-build verification.

### Phase 3 — Glue 5 shell integration
1. Map Glue 5 touch controls to the verified native input API.
2. Connect settings only where the native engine exposes supported settings.
3. Preserve Glue 5 launcher navigation and user-selected external data directory.
4. Mark unsupported WASM-derived features (e.g. WASM-native database/Luau hooks) as unavailable until a native equivalent is designed.

### Phase 4 — Device validation
1. Build a separate package/flavor; never overwrite the user's existing app or game data.
2. Verify installation, launch, resume, pause, rotation/surface recreation, audio focus and clean shutdown.
3. Test on physical Adreno 730 / Snapdragon 8+ Gen 1 hardware before claiming device compatibility.
4. Measure frame pacing, memory, thermal behavior and texture streaming with real captures.

## Acceptance criteria

- Native Android flavor configures and builds without `game.wasm`, wasm2c or generated game C.
- Existing WASM flavor still builds and behaves as before.
- Glue 5 launcher and touch-control shell remains usable.
- Native engine lifecycle and graphics are owned by a single verified backend.
- User game assets remain external and are not committed or redistributed.
- No gameplay, stability or FPS claim is made before physical-device testing.

## Known blockers / constraints

- The XHYN-V repository does not publish the complete proprietary engine or Android-base source. A full source rebuild of its native engine cannot be derived from that repository alone.
- The native library's exact exported startup/input/audio/lifecycle interfaces must be inspected from an authorized build; this draft does not presume them.
- Native engine and game assets must not be copied into Glue 5's repository or APK without clear redistribution rights.
- Glue 5's current WASM-derived mods and GPU command ring cannot be assumed to work with a separate native engine.
- This design is not an APK, not a completed integration and not proof of gameplay compatibility.
