# Native shell integration contract (Phase 5)

## What exists now

`crates/glue5-native-shell` is a WASM-independent Rust crate. Its only direct
dependency is `egui`. It contains:

- a Home/Settings page model;
- conservative launch defaults (60 FPS, 1.0 render scale, audio enabled);
- launch-option validation (FPS 30–120; finite render scale 0.5–1.0) before
  options cross the engine adapter boundary;
- a `NativeEngine` lifecycle trait (`state`, `start`, `stop`);
- a UI that reports an unattached engine instead of pretending a game can launch.

The crate is intentionally not a JNI adapter and is not yet linked into the
existing `glue5` Android cdylib. The current `glue5` package still links the
WASM runtime, GPU, mods and streaming paths.

## XHYN-V 0.3 Preview 2 facts now verified

The public XHYN-V 0.3 project README describes its runtime path as Android/SDL
loading a native ARM64 game engine and explicitly says the 64-bit
`libgtav.so` is loaded through Android/SDL's native entry point. It also
describes DXVK translating the engine's D3D11/DXGI calls to Vulkan. This
establishes the broad architecture, but not the exact Java or JNI call
signature needed to host that engine from a different application.

The 0.3 Preview 2 `VERIFICATION.json` records:

- package `com.xhynph.xhynv.reborn174`, version `0.3-preview2`, code 301;
- ARM64 native library inventory count: 26;
- the exact Preview 2 APK SHA-256:
  `a681765d9aae1404c59e3570682f3078bf9062ff800abfc7c5e1972f2783309f`;
- host Application/launcher startup tests on Android APIs 30 and 35;
- no physical-device gameplay, Android UI execution, or physical GPU probe.

The uploaded Preview 2 APK hash previously supplied by the user matches this
published verification record. The exact-hash build driver in the same
modification source expects a different original input/base APK hash
(`93f7850751a6ca172ba3cc6c42ca9abe1557dea1bba7b9def9d0bef0c9cd894d)).
That driver creates the published modification layer from its exact base; it
does not compile or publish the original game engine.

## What is still not established

The XHYN-V modification source identifies `SDLActivity.nativeRunMain` as a
launcher call in its earlier source history, and its `RebornNative.java`
comment places a hook after SDL has loaded `libgtav` and before that call.
However, the public `GTAVActivity.java` files under `modifications/stubs`
are compile stubs, not the original Android Activity implementation. The
0.1 `verification/native-runtime.json` is explicitly static inspection of
the shipped 0.1 APK, not a 0.3 runtime trace.

Thus the public material does not yet provide a verified 0.3 Java host API,
JNI method signature, native lifecycle, Surface contract, or safe way to
create a second SDL Activity from Glue 5. The 0.1 static entry-point record
must not be treated as the verified 0.3 host contract. Do not copy guessed
JNI names, offsets, Activity classes, or companion-library lists.

## Adapter contract required before implementation

A verified Android adapter must establish all of the following from the exact
0.3 engine build:

1. exact ARM64 `libgtav.so` identity and exported/native entry contract;
2. matching SDL Java package, Activity class and library-loading order;
3. ownership and lifecycle of the SDL thread and Android Activity;
4. Android Surface/window hand-off and recreation behavior;
5. pause/resume, back, touch/controller input and audio-focus behavior;
6. all 26 native libraries, their dependencies, assets and external game-data
   directory requirements;
7. failure reporting and orderly shutdown.

Only then should an Android-specific adapter implement `NativeEngine`.
Until then the UI must remain in `NotAttached` state and must not offer a
fake successful launch.

## Build and data-safety boundary

- Keep the existing `local.glue5` WASM APK target and its output unchanged.
- Put a future native flavor in a distinct Gradle application ID/output path.
- Do not delete, migrate, or overwrite `Documents/Glue5` or any existing game
  data as part of building/installing the native flavor.
- Do not package proprietary game data or private signing material in CI.
- A no-WASM claim is valid only after inspecting the resolved native target
  dependency graph and proving it excludes `glue5-game`, `glue5-runtime`,
  `glue5-mods` and `glue5-gpu`.

## Validation status

The native-shell unit tests cover defaults, invalid launch settings and
lifecycle transition guards, but have not been executed in this environment.
The source/metadata CI checks are not a Rust compile, Android APK build,
native-engine launch, FPS measurement, or device crash test. Those remain
pending until the adapter inputs and build environment are present.

## 0.3 lifecycle touch points found in the published patch script

The 0.3 `apply_runtime.py` patch script names the host-side methods it wraps:
`GTAVActivity.loadLibraries()`, `onResume()`, `onPause()`, `onDestroy()`,
and the nested `GTAVActivity$GameSurface.surfaceCreated()` /
`surfaceChanged()` callbacks. It also patches menu lifecycle methods. This
is useful evidence that the engine is hosted by a specific SDL-backed Activity
and that surface recreation and pause/resume are part of its integration.

These are patch-script target names, not a complete Activity implementation or
a public API. They do not prove that a second Activity can be instantiated,
that a different SDL version is ABI-compatible, or that the engine can safely
run inside Glue 5's existing SDL3 process. The actual 0.3 Activity bytecode,
SDL version, native library loading list/order, and JNI/native entry contract
must still be inspected from the exact APK before writing an adapter.

