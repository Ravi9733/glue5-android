# Native Android backend: source audit (Phase 0)

**Branch:** `native-android-engine-backend`  
**Status:** source inspection only; no native-engine code or APK has been built.

This audit records the exact Glue 5 source constraints found before changing the build graph. It supersedes assumptions that a backend can be switched by changing only the Android activity.

## Verified Glue 5 integration points

### 1. Android application entry point

File: `crates/glue5/src/lib.rs`

- The crate documents that Android's SDL Java activity loads SDL's library and `libglue5.so`, then calls `SDL_main`.
- The Android-exported `SDL_main` calls the common `main()`, then `glue5_runtime::qos::end()`.
- The common `main()` installs panic handling and enters `app::run()`.

Implication: replacing only the Java Activity or adding `libgtav.so` to the APK will not replace the game backend. The Rust application still enters the Glue 5 runtime.

### 2. WASM is a build-time hard dependency today

Files:
- `crates/glue5/Cargo.toml`
- `crates/glue5-runtime/Cargo.toml`
- `crates/glue5-game/Cargo.toml`
- `crates/glue5-game/build/main.rs`

Observed facts:

- `glue5` has unconditional dependencies on `glue5-runtime`, `glue5-gpu`, and `glue5-mods`.
- `glue5-runtime` has an unconditional dependency on `glue5-game`.
- `glue5-game` declares `build = "build/main.rs"`.
- The build script reads `GLUE5_GAME_DIR`, locates `game.wasm`, checks its build ID and SHA-256, runs `wasm2c`, generates bindings/native tables, and compiles the generated C sources.

Implication: a true no-WASM native flavor needs a Cargo dependency/build-script boundary that excludes `glue5-game` and the WASM runtime/mods path from the native target. Merely hiding the file picker or removing a runtime check would not satisfy the requirement.

### 3. Renderer boundary

The Glue 5 application currently depends on `glue5-gpu` and the Glue 5 GPU command-ring renderer. The XHYN-V modification-layer documentation describes a native engine library using its own SDL/native startup and DXVK-to-Vulkan graphics path.

Implication: the first native-engine prototype must let the XHYN engine own rendering. There is no verified adapter that converts its DXVK/Vulkan work into Glue 5's GPU command ring. The existing Glue 5 renderer must not be wired into the native path by assumption.

## Safe implementation sequence

1. Keep the existing WASM app target unchanged.
2. Split platform-independent launcher/settings/touch UI from the current game-running path only after mapping their actual dependencies.
3. Add a separate native Android executable/library target whose dependency graph does not include `glue5-game`, `glue5-runtime`, or `glue5-mods`.
4. Define a narrow native lifecycle contract (create, surface/window change, pause/resume, input, audio focus, destroy) only after inspecting the authorized XHYN native binary's exported symbols and Java/JNI contract.
5. Integrate the native target's own Vulkan/DXVK renderer; do not instantiate the Glue 5 wgpu renderer for this flavor.
6. Build and test both flavors independently. Do not claim device stability until tested on the iQOO Neo 7 Pro.

## Blocking evidence still required

- The complete, authorized native engine binary and its exact exported symbol/JNI/SDL contract. A library filename alone does not establish a callable entry point.
- The Java/Kotlin Android host contract from the matching XHYN-V source/build.
- Confirmation that the required engine/game files may be redistributed or supplied externally.
- A build environment and CI workflow capable of compiling the selected native target.

## Explicit non-claims

This audit is not an implementation, APK, or performance fix. It does not remove `game.wasm` from the existing Glue 5 target, and it does not establish that the XHYN-V engine can be embedded in Glue 5. The current WASM build remains untouched.
