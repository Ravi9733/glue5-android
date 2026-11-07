# Android host reuse matrix (source audit)

Baseline: `android-ios-port` branch, based on `turnip-driver-integration`.

## Key implementation decision

Do **not** copy the iOS `EngineHost.c` wholesale into Android.

The iOS target builds a standalone C host around wasm2c and Apple platform services. Glue 5 already has a Rust host runtime around its generated wasm2c engine, and its Android target already starts that runtime through SDL. The port should adapt the iOS app behavior to the existing Glue 5 runtime rather than introduce a second engine host.

The public iOS project declares version 1.5.2 in `app/project.yml`. Glue 5's `crates/glue5-game/build/main.rs` explicitly supports game build `8b0b5899ed` with SHA-256 `11ca8d2c04c5e843d18ff4aea4899d72c86973c6b031df334e67c446b2ae83e0`. Its build script translates the wasm with patched wasm2c, generates imports/hooks, and compiles generated C to a static library. This is the build path to retain.

## Service mapping

| iOS port responsibility | Existing Glue 5 Android-side implementation | Port action |
|---|---|---|
| wasm2c engine + native imports | `glue5-game` build script and generated `Imports`/`Hooks` | Reuse; verify actual wasm import and hook coverage from the selected game.wasm |
| Engine threads / pthreads / futex | `glue5-runtime/src/threads.rs`, `futex.rs` | Reuse; test thread names, stack sizes, join/cleanup and shutdown |
| WASI file access / game and save paths | `glue5-runtime/src/vfs.rs`, `files.rs` | Reuse; add iOS-compatible path semantics only where required; preserve existing user data |
| GPU command ring | `glue5-runtime/src/gpu.rs` | Reuse existing ring reader and protocol; validate payload layouts against iOS source before claiming full compatibility |
| Vulkan / Turnip rendering | `glue5-gpu` + Android activity's pre-SDL Turnip environment setup | Reuse; do not transplant Metal renderer |
| Audio output | `glue5-runtime/src/audio.rs` exposes 48 kHz stereo ring reader | Wire/verify Android audio callback and latency; do not duplicate iOS AVAudioSession |
| Controller/touch input | `glue5-runtime/src/input.rs` and existing app UI | Adapt iOS touch-context/control semantics to Android overlay; test touch and gamepad separately |
| Memory safeguards | `glue5-runtime/src/memory.rs`, runtime configuration | Map iOS memory-cap settings to Android process/memory budget; do not call Apple Jetsam APIs |
| Settings / Story / Sandbox | `glue5-runtime/src/game_settings.rs`, runtime `Config.start_mode` | Reuse Story=1/Sandbox=2 semantics; Android UI still needs settings screen and persistence |
| Mod files / asset overlays | `glue5-runtime/src/overlay.rs`, `script_assets.rs`, VFS | Audit actual format support; port iOS archive installer only where Android lacks equivalent support |
| Android app lifecycle / storage picker | `platform/android/java/local/glue5/Glue5Activity.java` | Extend existing activity or native UI; avoid new package/activity stack |

## iOS-only pieces that must not be copied as-is

- `os_proc_available_memory`, Mach task info, QoS and other Apple process APIs.
- Metal / MetalKit / CAMetalLayer and MetalFX renderer paths.
- UIKit, GameController and AVFAudio/AudioToolbox UI/audio lifecycle.
- iOS security-scoped file URLs and app sandbox conventions.
- Swift settings, mod installer and touch-control views: their behavior can be reimplemented, not their UIKit implementation.

## First implementation slice

1. Keep the single Glue 5 engine/runtime path and confirm the game.wasm import list and generated bindings at build time.
2. Add an Android settings/configuration layer for start mode, frame limit, memory cap, local game folder and mods-off.
3. Add the iOS touch-context model to the Android overlay, feeding Glue 5's existing input API.
4. Test the existing audio ring through Android's audio output.
5. Only then fill gaps in archive/mod handling and add-on vehicle UI.

## Acceptance gates

- Build uses the supported game.wasm hash above; no unknown-build override.
- Story and Sandbox both reach gameplay.
- No data deletion or APK-uninstall-dependent save path changes.
- GPU commands render correctly on system Vulkan and Turnip.
- Audio is continuous; touch/gamepad input maps correctly.
- Mod archive traversal is rejected and existing game files remain read-only.
- Release APK builds and starts on the target Snapdragon/Adreno device.

This document records source-level mapping only. It does not claim that the Android port is implemented or that an APK has been built.
