# Glue 5 Android ScriptHook host

This crate establishes an Android-native ScriptHook-style plugin boundary. It discovers Android shared libraries exporting `glue5_scripthook_plugin_init` and passes a versioned host API.

## Current implementation

- Deterministic discovery and loading of Android ARM64 `.so` plugins.
- Versioned C ABI (v9), host logging, script-stack allocation/restore helpers, and hash-based GTA native dispatch against the generated metadata for this exact game build.
- Plugins can register callbacks dispatched from the GTA script `WAIT` hook; callback dispatch is chained through Glue 5's existing mods hook when present.
- Native metadata introspection exposes handler index, parameter count, return kind, and per-parameter kind/direction; plugins can also resolve a native by its exact GTA name (for example `CREATE_VEHICLE`).
- Entity handles can be validated against the live script-guid pool and checked as any entity, ped, vehicle, or object before use.
- Model-name resolution checks the installed game's live vehicle/ped archetype catalog and returns the correct JOAAT model hash; it rejects unknown model names instead of blindly requesting them.
- Model streaming lifecycle helpers: `request_model(env, hash)` invokes the generated `REQUEST_MODEL` signature and checks `HAS_MODEL_LOADED` on each script tick (1=loaded, 0=pending); `release_model(env, hash)` calls `SET_MODEL_AS_NO_LONGER_NEEDED`. They reject unexpected generated parameter kinds/directions rather than guessing.
- Live ped, vehicle, and object enumeration returns script handles from the game's entity pools (maximum 4096).
- `call_native_typed` remains a scalar-only helper.
- `call_native_marshaled` validates each argument against the generated native signature and supports integer/hash/float/bool scalars, NUL-terminated string input, Vec3 input, and OUT/INOUT buffers. OUT/INOUT arguments provide a wasm-memory destination offset; Vec3 destinations are 12 contiguous bytes, string destinations are an 8-byte wasm pointer slot, and scalar destinations are 4 bytes.
- Native calls reject mismatched argument counts/types, invalid wasm-memory ranges, unsupported kinds, and creation calls when the relevant entity pool is nearly full.
- Loaded libraries remain resident so registered callbacks cannot point into unloaded code.

## C ABI argument kinds

- `0`: signed 32-bit integer or entity handle
- `1`: unsigned 32-bit value / hash
- `2`: IEEE-754 f32 bit pattern
- `3`: wasm-memory offset (string/vector input, or OUT destination)
- `4`: boolean (0 or 1)

For INOUT scalar arguments, `value` is the wasm-memory destination offset containing the initial value. For INOUT strings it points to an 8-byte wasm pointer slot; for INOUT Vec3 it points to 12 contiguous bytes containing three f32 values. For OUT arguments, use kind 3 and point `value` at the output destination.

## Plugin location

The app creates the user-data folder `scripthook/plugins/` on first Android game launch, then scans it for Android ARM64 `.so` plugins. Place plugins there; they must export `glue5_scripthook_plugin_init` and be built against `include/glue5_scripthook.h`. A minimal C example is in `examples/native-plugin/plugin.c`: it resolves `GET_GAME_TIMER` by name, verifies its generated zero-argument signature, registers a WAIT callback, and invokes the native without hard-coding its hash.

## Important limitations

This is not yet a drop-in ScriptHookV / ScriptHookVDotNet implementation. Menyoo has not been ported, managed .NET is not included, and existing Windows `.asi` / `.dll` binaries cannot run unchanged; they must be ported/rebuilt for Android. Native return values are still exposed as raw 64-bit bits, so plugin code must decode them according to the metadata. Device testing and a verified APK build remain outstanding.

The supplied iOS ScriptHook pack contains Menyoo assets and managed DLLs, but not the native iOS PostHook implementation needed to reproduce that runtime.

### Model streaming usage

Resolve a vehicle or ped model name with `resolve_model_name`, then call `request_model` once per script WAIT callback until it returns 1. Only then call the relevant creation native (for example `CREATE_VEHICLE` or `CREATE_PED`) through `call_native_marshaled`. After the creation call succeeds, call `release_model` to drop the streaming reference. A return value of 0 means the model is still loading; do not spin-wait or block the game thread.