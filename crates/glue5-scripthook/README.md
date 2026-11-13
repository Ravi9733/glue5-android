# Glue 5 Android ScriptHook host

This crate establishes an Android-native plugin boundary for the ScriptHook port. It discovers `.so` files and loads only libraries that export `glue5_scripthook_plugin_init`, passing a versioned host API.

## Current implementation

- Deterministic discovery and loading of Android `arm64-v8a` shared libraries.
- Versioned C ABI, host logging, script-stack allocation/restore helpers, and hash-based GTA native dispatch to this game build's handler table.
- Plugins can register callbacks dispatched from the GTA script `WAIT` hook.
- Callback dispatch is chained through the existing Glue 5 mods hook when that feature is enabled, avoiding two systems overwriting the same game hook.
- Loaded libraries remain resident so registered callbacks cannot point into unloaded code.
- Windows `.asi` / `.dll` and managed .NET assemblies are not treated as Android-native plugins.

The native call accepts a prepared game-memory `rage::scrThread::Info` offset; plugins must save/restore the script stack and construct the game's argument/result buffers correctly. It is a low-level ABI, not yet the typed ScriptHookV native wrapper or a compatibility guarantee for Windows plugins.

The public C header is in `include/glue5_scripthook.h`; a minimal plugin example is in `examples/native-plugin/plugin.c`.

## Plugin location

The app currently scans the user data folder at `scripthook/plugins/`.

Place Android ARM64 `.so` plugins there. They must export `glue5_scripthook_plugin_init` and be built against the header in this crate.

## Important limitations

This is **not** yet a drop-in ScriptHookV / ScriptHookVDotNet implementation. The callbacks run in the game's script WAIT context, but the complete ScriptHook native API (native hash invocation, parameter marshalling, entity helpers, keyboard/input APIs, and lifecycle parity) is not implemented yet. Menyoo has not been ported. A managed .NET runtime is not included. Existing Windows `.asi` binaries cannot run unchanged and must be ported/rebuilt for Android.

The supplied iOS ScriptHook pack contains Menyoo assets and managed DLLs, but not the native iOS PostHook implementation needed to reproduce the iOS runtime. This Android implementation therefore needs its own native bridge and Android-specific plugin builds.
