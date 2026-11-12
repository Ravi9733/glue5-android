# Glue 5 Native — XHYN Preview 2 bootstrap APK

## Download

The latest verified CI artifact is named `Glue5-Native-XHYN-Preview2`:

- GitHub Actions run: https://github.com/Ravi9733/glue5-android/actions/runs/43244472959
- APK SHA-256: `de5ad9599877e41765abfe19c9a9f0087e0ed3efb0dc6e15e826214347da45d0`
- Artifact retention: 3 days from the build.

Open the run's **Artifacts** section and download the ZIP. Extract the APK and checksum file. Verify the APK SHA-256 before installing.

## Install and data safety

- Application ID: `local.glue5.nativeapp` (separate from `local.glue5` and the original XHYN package).
- Architecture: ARM64 only.
- Minimum Android API: 30; target API: 36.
- Development-signed build. It does not update or replace an official XHYN installation.
- Do not uninstall the existing Glue 5 app and do not clear its storage.
- The patch does not delete, copy, or migrate `Documents/Glue5`.
- XHYN's default external game root remains `/storage/emulated/0/Games/GTAV`. If the game files are elsewhere, use the existing storage/game-root selection flow.
- Compatible GTA V game files are not included in this APK; provide your own legally obtained compatible files.

## What this build contains

- A small green-accented Glue 5 Native launcher Activity.
- The original XHYN Preview 2 storage/game menu and native SDL3 game Activity.
- The original `SDL3 → gtav` native load order and `SDL_main` entry.
- All 169 original assets and all 27 ARM64 shared libraries, verified byte-identical to the source Preview 2 APK.
- No `game.wasm` or other WASM payload.

## Known limits

This is the first installable native host bootstrap, not the final Glue 5 Rust/egui UI port. The game screen and storage/configuration flow are still XHYN's. The APK has passed CI packaging, signing and structural checks, but it has **not** been installed or run on the iQOO Neo 7 Pro. Gameplay, frame rate, audio, pause/resume, surface recreation and crash stability are unverified. Treat it as a development test build, not a stable release.

The mod-spawn provider authority is namespaced to `local.glue5.nativeapp.modspawn`; a companion app hard-coded to the original XHYN authority may need a matching update.
