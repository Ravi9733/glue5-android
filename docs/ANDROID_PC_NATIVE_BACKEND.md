# Android native PC-game backend: implementation gate

## Scope

This is a separate experimental backend for a future native PC-game port. The existing Glue 5 WASM backend remains the default and is not modified by this work.

The supplied iOS IPA is a reference for architecture only. Its executable and dynamic libraries are Mach-O/iOS binaries linked against Apple frameworks and Metal; they are not Android ELF libraries and must not be packaged into the APK.

## Required runtime components

1. A rebuildable ARM64 game-engine source tree and Android NDK build.
2. An Android platform-compatibility layer for the engine's required OS APIs.
3. A D3D11/DXGI-to-Vulkan implementation that runs on Android and is validated against Turnip.
4. A reproducible offline preparation pipeline for user-owned PC game data, including archive/version checks and any required texture/audio conversion.
5. Android integration for storage, lifecycle, input, audio, saves, and diagnostics.

The PC game archives are data, not executable game code. A data directory alone cannot replace game.wasm.

## Current code in this branch

The new glue5-pc-native crate is a conservative readiness gate. It checks whether three required paths exist: an Android engine library, a Vulkan translation layer, and prepared game data. It does not load or execute any of them. The ReadyForRuntimeIntegration state means only that the paths exist; it does not mean the game is playable.

## Next engineering gates

- Obtain source/build scripts and licensing permission for the native engine port.
- Define and verify the exact Android ABI and Vulkan interface.
- Specify a versioned manifest for prepared PC data before implementing archive conversion.
- Add a real Android build target only after the source inputs and toolchain are available.
- Run device tests for boot, streaming, shader correctness, audio, saves, memory, and thermal stability.

No iOS binary is copied, no proprietary game data is committed, and no APK is produced by this change.
