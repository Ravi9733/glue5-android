# Android native PC-game backend: implementation gate

## Scope

This is a separate experimental backend for a future native PC-game port. The existing Glue 5 WASM backend remains the default and is not modified by this work.

The supplied iOS IPA is an architecture reference only. Its executable and dynamic libraries are Mach-O/iOS binaries linked against Apple frameworks and Metal; they are not Android ELF libraries and must not be packaged into the APK.

## Required runtime components

1. A rebuildable ARM64 game-engine source tree and Android NDK build.
2. An Android platform-compatibility layer for the engine's required OS APIs.
3. A D3D11/DXGI-to-Vulkan implementation that runs on Android and is validated against Turnip.
4. A reproducible offline preparation pipeline for user-owned PC game data, including archive/version checks and any required texture/audio conversion.
5. Android integration for storage, lifecycle, input, audio, saves, and diagnostics.

The PC game archives are data, not executable game code. A data directory alone cannot replace game.wasm.

## Current code

The `glue5-pc-native` crate provides a conservative component-presence readiness gate and a version-1 JSON manifest parser. The parser validates required metadata, non-empty file lists, relative normalized paths, duplicate paths, unsigned file sizes, and 64-character SHA-256 hex strings.

Manifest validation is metadata-only. It does not hash files on disk, extract archives, transform textures/audio, load an engine, or make the game playable. A ready layout means only that the configured component paths exist.

## Version-1 data manifest

Store `glue5-pc-data.json` at the root of a prepared, user-owned data directory:

```json
{
  "schema_version": 1,
  "game_build": "build-2699",
  "preparation_tool": "glue5-data-prep/0.1",
  "files": [
    {
      "path": "update/update.rpf",
      "size": 1234,
      "sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    }
  ]
}
```

Paths are relative to the data root and use forward slashes. The manifest is a contract for future offline preparation and verification tooling; it does not imply that a PC archive is already compatible with an Android engine.

## Next engineering gates

- Obtain rebuildable native engine source/build scripts and confirm licensing permission.
- Define and verify the exact Android ABI and Vulkan interface.
- Implement a separate offline data-preparation tool only after the source format and target runtime format are known.
- Add a real Android build target only after source inputs and toolchain are available.
- Run device tests for boot, streaming, shader correctness, audio, saves, memory, and thermal stability.

No iOS binary is copied, no proprietary game data is committed, and no APK is produced by this change.
