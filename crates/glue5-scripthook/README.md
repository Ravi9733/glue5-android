# Glue 5 Android ScriptHook host

This crate establishes the Android-native plugin boundary for the ScriptHook port. It discovers `.so` files and loads only libraries that export `glue5_scripthook_plugin_init`, passing a versioned host API.

## Current scope

- Deterministic discovery and loading of Android ARM64 shared libraries.
- Versioned C ABI and host logging callback.
- Loaded libraries remain resident so registered callbacks cannot point into unloaded code.
- Windows `.asi` / `.dll` and managed .NET assemblies are not treated as Android-native plugins.

## Not implemented yet

This is **not** a drop-in ScriptHookV / ScriptHookVDotNet implementation. The game-specific native registration table, script-thread lifecycle, Menyoo port, managed .NET runtime, and Android equivalents of the iOS PostHook hooks still need to be implemented and tested. The supplied ScriptHook pack contains mod assets and managed DLLs, but not the native iOS PostHook implementation needed to reproduce the iOS runtime.

Plugins must be rebuilt for Android `arm64-v8a` against this ABI. Existing Windows `.asi` binaries cannot run unchanged.
