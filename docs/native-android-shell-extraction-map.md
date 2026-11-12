# Glue 5 shell extraction map (Phase 1)

**Branch:** `native-android-engine-backend`  
**Status:** source map and dependency analysis; no application code changed in this phase.

This phase checks whether the existing Glue 5 launcher/settings/touch UI can be compiled independently from its WASM engine before introducing a native XHYN-V backend.

## Findings from the current source

### UI is not currently a standalone shell crate

- `crates/glue5/src/ui.rs` owns the start page, settings, credits, friends, servers, mods page, and touch-screen settings affordances.
- It imports `crate::settings::{PerfOverlay, Settings}` and `crate::touch::Touch`, plus SDL event/window types and egui.
- Its page modules include `mods`, `friends`, and `servers`; these are not merely static screens.
- `crates/glue5/src/app.rs` owns the SDL event loop and creates the wgpu presentation/surface before showing the start page. It also owns game-folder discovery, stream client/server state, the WASM runtime game object, input blocks, game audio, lifecycle events, and the UI.
- The app module directly imports `glue5_gpu`, `glue5_runtime`, `glue5_stream`, and `glue5_mods`. The running-game state contains `glue5_runtime::Game` and `InputBlock`.

**Conclusion:** compiling the existing `ui.rs` alone is not currently a valid native-shell solution. The UI and application loop are coupled to the current game runtime and its renderer. Simply setting Cargo features or hiding the Play button would still leave the native flavor linked to WASM dependencies.

### Android build pipeline

File: `.mise/tasks/android/lib.sh`

- `android_libs()` invokes `cargo ndk ... rustc --release -p glue5 --lib --crate-type cdylib`.
- It stages `libglue5.so`, `libSDL3.so`, and `libc++_shared.so` into the Android Gradle project's generated `target/android/jniLibs/arm64-v8a`.
- `android_apk()` resolves SDL's Java source from the locked Cargo dependency graph and invokes Gradle with `-p platform/android`.
- The launch task starts `local.glue5/.Glue5Activity`.

**Conclusion:** the present Android packaging target is a single `glue5` cdylib plus SDL Java. A separate native-engine APK flavor needs an explicit second packaging path and must not overwrite the existing WASM APK output.

## Recommended extraction boundary

Do not move the entire `app.rs` at once. First separate stable, engine-agnostic pieces from runtime-specific orchestration:

| Component | Current location | Native-shell suitability |
|---|---|---|
| Settings data model and JSON persistence | `crates/glue5/src/settings.rs` | Candidate, after removing platform/runtime assumptions |
| Touch-control drawing and layout | `crates/glue5/src/touch.rs`, `ui.rs` | Candidate, but input dispatch is currently tied to Glue 5 input types |
| Theme and generic egui widgets | `crates/glue5/src/ui/theme.rs`, `ui/widgets.rs`, `ui/chip.rs` | Best early extraction candidates |
| Start page / game source picker | `ui/pages.rs`, `app.rs` | WASM-specific today; native flavor needs its own launch/data selection |
| Friends / streaming / mods pages | `ui/friends.rs`, `ui/servers.rs`, `ui/mods.rs` | Keep out of the first native prototype until their runtime contracts are replaced |
| SDL window and event loop | `app.rs` | Shared platform host candidate only after the engine lifecycle is abstracted |
| wgpu presentation / GPU command ring | `app.rs`, `glue5-gpu` | Do not use for the XHYN-V first prototype; native engine owns DXVK/Vulkan |
| WASM game instance and audio/input blocks | `app.rs`, `glue5-runtime` | Must be excluded from the native target |

## Next safe code change

The first source-level extraction should be small and compile-verifiable: move only generic visual primitives/theme into a separate UI support module/crate, without changing behavior or introducing native-engine symbols. In parallel, define a separate Android packaging output directory and target name so a later native APK cannot replace the current WASM APK.

A backend trait or JNI symbol must not be invented until the matching XHYN-V Android host and native library exports have been inspected.

## Acceptance checks before claiming extraction

- Existing WASM Android build graph remains unchanged.
- Extracted UI support compiles on the existing targets.
- Native flavor dependency graph contains no `glue5-game`, `glue5-runtime`, `glue5-mods`, or `glue5-gpu` unless an explicitly documented adapter requires one.
- Native APK packaging uses a separate output path.
- No existing user game-data directory is deleted or migrated by the build.

This document is an implementation map, not a build artifact or APK.

## Phase 2 dependency check (file-level)

A second pass inspected the actual candidate modules rather than relying only on their filenames:

- `ui/theme.rs` depends on egui, PNG decoding, logging, and compile-time UI assets. Its layout/style helpers are generic, but `background()` embeds `assets/ui/home.png`; moving this file unchanged would also require moving the assets and preserving their relative include paths.
- `ui/widgets.rs` is **not yet engine-independent**. Its `text_field()` uses `crate::paths::TOUCH`, `TextKind`, and the parent UI's SDL clipboard helper. Other controls (buttons, sliders, segmented choices, label painting) are reusable once the text-field/clipboard boundary is split.
- `ui/chip.rs` is **not a generic widget**: it consumes the UI's `StreamAlert` type and its tests import `crate::touch::Touch` and `glue5_runtime::input`. It belongs with the streaming overlay until that feature is deliberately removed or adapted.
- `ui.rs` also exposes runtime-shaped page/status types: `Started`, `FoundServer`, `StreamAlert`, settings upscalers, benchmark results, sharing state, and mods/studio state. These must not leak into a native-only shell API.

### Revised extraction order

1. Extract the pure visual controls from `widgets.rs` (buttons, slider, segmented choice, choice selector, and label painter) behind a small UI-kit API; leave `text_field` and clipboard behavior in the host.
2. Extract theme colors, font/style installation, and responsive scale helpers separately from the home-background asset loader. Keep the current embedded background path unchanged until asset ownership is settled.
3. Keep `chip.rs`, streaming pages, friends, mods, and server pages outside the first native shell.
4. Only after the shell's page model no longer references Glue 5 runtime types should the app event loop/backend split begin.

This is a dependency audit only. No source files were moved and no compilation or device test is claimed. The first attempted extraction is intentionally narrowed to avoid dragging SDL clipboard, streaming, or WASM runtime types into a supposedly portable UI crate.
