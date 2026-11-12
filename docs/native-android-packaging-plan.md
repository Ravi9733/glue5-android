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

## Required Gradle work (blocked on locating the actual Android project)

1. Identify the checked-in Android Gradle root, module names, manifest and
   launcher Activity from the repository tree.
2. Add a separate native application module or product flavor only after
   confirming the existing packaging layout.
3. Give the native variant its own application ID, label, version suffix and
   APK output name.
4. Package only the exact ABI-compatible engine and companion libraries.
5. Preserve the existing WASM application ID, build tasks and output paths.
6. Keep game data external; do not copy, delete, migrate or clear
   `Documents/Glue5` during install, update or uninstall.

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

The repository's public native-engine material is a modification layer, not
the original complete Android host project. Until the actual Gradle module and
matching host implementation are located, adding guessed Gradle paths,
Activity classes, JNI symbols or packaging rules would create a misleading or
non-buildable APK target. No APK build or device validation is claimed here.
