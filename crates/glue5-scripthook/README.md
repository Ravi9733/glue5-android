# Glue 5 Android ScriptHook host

This crate establishes an Android-native plugin boundary for the ScriptHook port. It discovers .so files and loads only libraries that export glue5_scripthook_plugin_init, passing a versioned host API.

## Current implementation

- Deterministic discovery and loading of Android arm64-v8a shared libraries.
- Versioned C ABI (currently ABI v4), host logging, script-stack allocation/restore helpers, and hash-based GTA native dispatch to this game build's handler table.
- Plugins can register callbacks dispatched from the GTA script WAIT hook.
- Callback dispatch is chained through the existing Glue 5 mods hook when that feature is enabled, avoiding two systems overwriting the same game hook.
- A typed scalar native-call entrypoint marshals up to 64 integer, unsigned/hash, float-bit, or wasm-memory-offset arguments into the game's scrThread::Info layout and returns raw result bits.
- Loaded libraries remain resident so registered callbacks cannot point into unloaded code.
- Windows .asi / .dll and managed .NET assemblies are not treated as Android-native plugins.

The low-level native call still accepts a prepared game-memory scrThread::Info offset. The typed helper covers scalar slots only: strings, output/in-out parameters, vector reference handling, return-type decoding, entity helpers, and signature validation are not yet implemented. Plugin authors must verify each native's hash and signature against the bundled game build.

The public C header is in include/glue5_scripthook.h; a minimal plugin example is in examples/native-plugin/plugin.c.

## Plugin location

The app currently scans the user data folder at scripthook/plugins/.

Place Android ARM64 .so plugins there. They must export glue5_scripthook_plugin_init and be built against the header in this crate.

## Important limitations

This is not yet a drop-in ScriptHookV / ScriptHookVDotNet implementation. Menyoo has not been ported. A managed .NET runtime is not included. Existing Windows .asi binaries cannot run unchanged and must be ported/rebuilt for Android.

The supplied iOS ScriptHook pack contains Menyoo assets and managed DLLs, but not the native iOS PostHook implementation needed to reproduce the iOS runtime. This Android implementation therefore needs its own native bridge and Android-specific plugin builds.


## ABI v5 additions

- Native metadata lookup by hash: reports handler index, parameter count, and return-kind code for the exact generated database.
- Per-parameter introspection: reports each argument kind and direction (input, output, in/out) without exposing Rust internals.
- Entity enumeration from the game's live pools: peds, vehicles, and objects are returned as script handles, capped at 4096 per request.

These APIs provide discovery and inspection; they do not yet implement a complete safe typed marshaller for strings, vectors, or output parameters. Entity enumeration must be called from the game script callback context.