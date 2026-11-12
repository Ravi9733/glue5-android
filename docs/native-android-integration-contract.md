# Native shell integration contract (Phase 4)

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

## XHYN-V host facts verified from its public source

The XHYN-V modification source identifies the SDL launcher call as
`SDLActivity.nativeRunMain`. Its `RebornNative.java` comment places that
hook after SDL has loaded `libgtav` and before `nativeRunMain`. The public
`GTAVActivity.java` files under `modifications/stubs` are compile stubs,
not the original activity implementation.

Therefore, those files do not define a reusable Java host API, JNI method
signature, native lifecycle, surface contract, or a safe way to create a
second SDL activity from Glue 5. The exact matching host APK/source and
native-library contract are still required. Do not copy guessed JNI names or
call `SDL_main`/ `nativeRunMain` directly from this crate.

## Adapter contract required before implementation

A verified Android adapter must establish all of the following from the exact
engine build, not from an approximate release:

1. exact ARM64 `libgtav.so` identity and exported/native entry contract;
2. matching SDL Java package, Activity class and library-loading order;
3. ownership and lifecycle of the SDL thread and Android Activity;
4. Android Surface/window hand-off and recreation behavior;
5. pause/resume, back, touch/controller input and audio-focus behavior;
6. required companion libraries, assets and external game-data directory;
7. failure reporting and orderly shutdown.

Only then should an Android-specific adapter implement `NativeEngine`. Until
then the UI must remain in `NotAttached` state and must not offer a fake
successful launch.

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

The native-shell unit tests now cover defaults and invalid launch settings,
but they have not yet been executed in this environment. The source/metadata
CI checks are not a Rust compile, Android APK build,
native-engine launch, FPS measurement, or device crash test. Those remain
explicitly pending until the adapter inputs and build environment are present.

## Exact-build compatibility gate (confirmed against XHYN-V source)

The XHYN-V repository's `verification/native-runtime.json` explicitly labels
its evidence as static inspection of the shipped **0.1 APK**, not a runtime
trace or reconstruction of the original host source. It records
`SDLActivity.nativeRunMain` and an ARM64 engine library hash. The XHYN-V
`docs/SOURCE.md` separately states that its 0.1 and 1.7.4 modification layers
target different engine builds and their fixed native offsets are not
interchangeable.

The published `GTAVActivity.java` under `modifications/stubs` is only a
compile stub, not the original Activity. The published `RebornNative.java`
also says it must be called after SDL loads `libgtav` and before
`nativeRunMain`; its native language bridge is not a general-purpose engine
launcher API.

Therefore the 0.1 static entry-point record cannot be treated as the verified
host contract for the user's XHYN-V 0.3 Preview 2 APK. Do not copy its JNI
symbol, package, activity, native offsets, or companion-library list into a
new application until the exact Preview 2 host and matching base build are
identified. This is a hard compatibility gate, not a minor missing detail.
