# Android native mod system — implementation stages

## Why a separate Android ABI is required

The iOS payload contains native engine-side includes and an iOS host bridge. The supplied Menyoo plugin is a Windows PE32+ x86-64 DLL that imports ScriptHookV and Win32 APIs. Neither binary is an Android ARM64 shared object. The same game WASM build does not make those host ABIs interchangeable.

## Stage 1: package contract and binary validation (implemented)

A native package declares `glue5-native-mod.json`:

```json
{
  "format_version": 1,
  "id": "sample-native",
  "name": "Sample Native Mod",
  "version": "1.0.0",
  "abi": "glue5-native-v1",
  "library": "libsample.so",
  "entry": "glue5_mod_init",
  "min_engine": "8b0b5899ed"
}
```

The validator rejects unknown fields, unsafe paths, unsupported ABI versions, non-ELF files, non-64-bit or big-endian ELF, and non-AArch64 machine types. Validation is not execution: the library is not loaded by this stage.

## Stage 2: host ABI and lifecycle

A draft C ABI v1 contract is defined in `crates/glue5-mods/include/glue5_native_mod.h`. It specifies:
- ABI/version and engine-build handshake;
- plugin init, per-frame tick, and shutdown callbacks;
- opaque host context rather than raw engine pointers;
- a bounded queued command channel (not direct arbitrary native invocation);
- bounded callback time and memory budgets;
- structured logging and fault isolation.

The header is a contract draft only; the host does not yet implement these function pointers or load plugins.

All engine/native access must be marshalled onto Glue5's existing game script thread. UI/controller threads must never call game natives directly. A plugin must not retain transient engine pointers across ticks.

## Stage 3: trust and loading

Native code is executable code, not passive mod data. Before loading is enabled:
- the app must show the plugin identity and request explicit user confirmation;
- libraries must remain in the app-private mod directory;
- manifest and library paths must be canonicalized beneath that directory;
- the loader must enforce the declared ABI and engine build;
- a crash/recovery record must identify the plugin that was active;
- disable/uninstall must call shutdown on the game thread before unloading;
- unsigned or incompatible plugins must remain disabled.

## Stage 4: content and compatibility

Native code loading does not mount GTA V RPF/DLC content. Vehicle packs still need verified packfile registration, DLC load order, archetype registration and streaming integration. Menyoo feature parity is a separate port of its feature logic and data catalogs.

## Current status

- Added `native_mods` manifest and Android ARM64 ELF validation module with tests.
- Exposed the module from `glue5-mods`.
- Added a draft, versioned C ABI v1 header for future Android plugins.
- No dynamic library loading, plugin callback execution, RPF mounting, or Menyoo binary compatibility is claimed yet.
