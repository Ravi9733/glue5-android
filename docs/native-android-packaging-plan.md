# Separate native Android packaging plan

## Goal

Produce a native-engine Android application using the reusable Glue 5 shell,
without changing the existing `local.glue5` WASM application or its data.

This is a packaging contract and implementation checklist, not a claim that an
APK target currently exists.

## Isolation requirements

| Area | Existing Glue 5 | Future native flavor |
| --- | --- | --- |
| Application ID | `local.glue5` | Must be a distinct ID, e.g. `local.glue5.native` |
| Launcher | Existing Glue 5 SDL activity | Verified XHYN-V host Activity/SDL entry |
| Native library | `libglue5.so` plus SDL3 | Exact matching XHYN-V `libgtav.so` and required companions |
| Engine runtime | WASM runtime and generated game C | Native ARM64 engine; no WASM runtime |
| User data | Existing Documents/Glue5 | Separate native data directory; never overwrite Glue 5 data |
| Output | Existing release APK path | Separate Gradle variant and output filename |

The example native application ID is a proposal only; it must be checked
against any installed package before release.

## Verified Gradle layout and required work

The checked-in Android Gradle project is `platform/android` (root project
`Glue5`). Its application module uses `platform/android/build.gradle.kts`,
`platform/android/settings.gradle.kts`, `platform/android/AndroidManifest.xml`
and `platform/android/java/local/glue5/Glue5Activity.java`. The existing
module currently has one application ID, `local.glue5`, and one launcher
Activity that extends SDL3's `SDLActivity` and loads `SDL3` plus `glue5`.

The safe packaging change is therefore a second Android application module
(or a carefully isolated variant) with its own manifest, Activity, application
ID, assets and native-library staging directory. Before choosing a variant,
confirm that the XHYN host can be represented in this project without loading
the existing SDL3/Glue5 native entrypoint. Do not simply append `libgtav.so`
to the current `getLibraries()` list: the XHYN 0.3 Activity and lifecycle
contract have not been verified as a reusable host API.

Required work:

1. Keep `local.glue5`, its existing SDL3 Activity, build tasks and output paths
   unchanged.
2. Add a separate native application target only after the exact XHYN 0.3
   Activity/DEX/manifest and native library loading order are verified.
3. Give that target a distinct application ID (proposed: `local.glue5.native`),
   label, version suffix, APK output name and JNI library staging directory.
4. Package only the exact ABI-compatible engine and companion libraries.
5. Keep game data external in a native-specific directory; do not copy, delete,
   migrate or clear `Documents/Glue5` during install, update or uninstall.

## Native runtime requirements

The native module must not instantiate the current `glue5` application crate,
because that crate directly depends on `glue5-runtime`, `glue5-gpu` and
`glue5-mods`. The native shell crate is currently separate and has only a
direct `egui` dependency. A future Android host must implement
`NativeEngine` only after the exact XHYN-V Activity, SDL lifecycle, JNI and
surface contract is verified.

## Release gates

- [ ] Actual Android Gradle project paths and manifest verified.
- [ ] Exact XHYN-V host contract and matching base build verified.
- [ ] Native module dependency graph excludes `glue5-game`,
      `glue5-runtime`, `glue5-mods` and `glue5-gpu`.
- [ ] APK package ID and output path are distinct from existing Glue 5.
- [ ] APK contents contain required ARM64 libraries and no WASM game runtime.
- [ ] Install/update/uninstall data-safety test passes without touching
      existing Glue 5 data.
- [ ] Launch, pause/resume, surface recreation, input, audio and shutdown
      verified on a real device.
- [ ] iQOO Neo 7 Pro performance and texture-streaming checks completed.

## Current limitation

The Glue 5 Android Gradle project is now located and its current launcher
contract is verified. The missing piece is the exact XHYN-V 0.3 Android host:
its original Activity/DEX, SDL version and native library load order are not
published in the modification layer. Adding guessed Activity classes, JNI
symbols or a second launcher before those are known would create a misleading
or non-buildable APK target. No APK build or device validation is claimed here.
