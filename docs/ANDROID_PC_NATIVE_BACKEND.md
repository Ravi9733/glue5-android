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

The glue5-pc-native crate provides a conservative component-presence readiness gate, a version-1 JSON manifest parser, and an on-disk integrity verifier. The parser validates required metadata, non-empty file lists, relative normalized paths, duplicate paths, unsigned file sizes, and 64-character SHA-256 hex strings.

The verify_data_files function checks each listed file's real size and SHA-256 using a bounded 1 MiB streaming buffer. It rejects missing files, non-regular files, size/hash mismatches, and resolved paths that escape the selected data root through symlinks. This verifies integrity only; it does not establish that PC archives are compatible with an Android game engine.

The crate also has an inventory-only RPF header probe. For GTA V Legacy it recognizes the RPF7 magic (0x52504637), reads the entry count and encryption tag, computes the 16-byte-per-entry TOC size with checked arithmetic, and separates the low 28-bit name-table length from the upper four header flags. It labels RPF8 separately and rejects truncated headers.

The next layer, inventory_rpf7_open, parses the table of contents and entry names for RPF7 archives marked NONE or OPEN. It identifies directory, binary-file, and resource-file records; validates the TOC/name-table bounds, entry-count and name-table safety limits, NUL-terminated UTF-8 names, binary encryption flags, and directory child ranges. It returns metadata only: no file payload is extracted, decompressed, decrypted, modified, or executed. AES/NG encrypted TOCs are explicitly rejected until a separately reviewed key/decryption implementation is available. This is important because a parser must never silently interpret encrypted bytes as filenames.

The `scan_nested_rpf7_open` pass checks direct `.rpf` binary children one level deep. It inspects only payloads that are unencrypted, have matching stored/uncompressed sizes, and whose 512-byte-block range is inside the parent archive. A nested OPEN/NONE RPF7 gets its own metadata inventory; RPF8 is labelled unsupported. Encrypted, compressed, malformed, and out-of-range payloads are skipped/reported rather than extracted. Encrypted nested TOCs and invalid nested RPF7 TOCs receive distinct statuses, so unsupported encryption is not confused with corrupt metadata. This pass is intentionally non-recursive and does not decode payloads. The `validate_rpf7_payload_ranges` helper checks binary and resource entries using checked 512-byte-block offsets. For binary entries, a stored size of zero correctly uses the uncompressed-size field. For resource entries, the 0xFFFFFF size sentinel is reported as unresolved rather than mistaken for a literal 16 MiB size; resolving it requires reading the resource payload's scattered size metadata. The all-ones 23-bit block offset is also reported as a sentinel, not treated as a real address. Ranges are checked against archive length and flagged when they overlap the header/TOC/name-table region. No payload is read or extracted by this helper. The nested-archive scanner refuses candidates that begin before the metadata region ends.

The RPF7 inventory path is Legacy-only. RPF8/Enhanced archives are rejected rather than treated as RPF7. Resource-size sentinel resolution, recursive nested archive traversal, asset decoding, and data conversion remain future work.

The readiness gate is intentionally conservative: even when configured component paths exist, its ReadyForRuntimeIntegration state means only that the three expected paths are present. It does not load or execute a game engine and does not mean the game is playable.

## Version-1 data manifest

Store glue5-pc-data.json at the root of a prepared, user-owned data directory:

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

- Verify the new parser against real user-owned OPEN test archives and fuzz malformed inputs.
- Add bounded nested-RPF inventory without extracting payloads.
- Obtain rebuildable native engine source/build scripts and confirm licensing permission.
- Define and verify the exact Android ABI and Vulkan interface.
- Implement a separate offline data-preparation tool only after the source format and target runtime format are known.
- Add a real Android build target only after source inputs and toolchain are available.
- Run device tests for boot, streaming, shader correctness, audio, saves, memory, and thermal stability.

No iOS binary is copied, no proprietary game data is committed, and no APK is produced by this change.
