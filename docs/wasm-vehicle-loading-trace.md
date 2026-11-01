# WASM vehicle-loading symbol trace

Target: `game/b/8b0b5899ed/game.wasm` from the PC-mod-loader CI artifact for commit `969497ae4ced287efa96897c80b61c605cc9fff2`.

## Confirmed symbols in the exact game binary

The generated symbol index and WAT contain these RAGE entry points:

| Function | WASM function index | What the symbol indicates |
|---|---:|---|
| `rage::fiPackfile::Init(char const*, bool, rage::fiPackfile::CacheMode, char*)` | 1881 | Initializes a packfile object from a path and options |
| `rage::fiPackfile::MountAs(char const*)` | 1883 | Mounts an initialized packfile at a virtual path |
| `rage::fiDevice::Mount(char const*, rage::fiDevice const&, bool)` | 1770 | Device mount operation |
| `rage::fiDevice::Unmount(char const*)` | 1806 | Device unmount operation |
| `rage::strPackfileStreamingModule::Register(char const*)` | 6048 | Registers a packfile with the packfile streaming module |
| `rage::strPackfileStreamingModule::Load(rage::strLocalIndex, void*, int)` | 6057 | Streaming-module load callback |
| `rage::fwArchetype::InitArchetypeFromDefinition(rage::strLocalIndex, rage::fwArchetypeDef*, bool)` | 4608 | Initializes an archetype from a definition |
| `rage::fwArchetypeManager::RegisterStreamedArchetype(rage::fwArchetype*, int)` | 4609 | Registers a streamed archetype |
| `rage::fwArchetypeManager::RegisterPermanentArchetype(rage::fwArchetype*, int, bool)` | 4610 | Registers a permanent archetype |
| `rage::fwArchetypeStreamingModule::Load(rage::strLocalIndex, void*, int)` | 4646 | Archetype streaming load callback |
| `CVehicleModelInfo::LoadVehicleMetaFile(char const*, bool, int)` | 38924 | Vehicle metadata loading routine |
| `CVehicleModelInfo::InitClass(unsigned int)` | 38909 | Vehicle model-info class initialization |

The `fiPackfile::Init` WAT body is present and calls `fiAssetManager::FullReadPath` and `fiDevice::GetDevice`; this is stronger evidence than a name-only symbol search that the packfile path is implemented in this binary.

## What is not established yet

A symbol name and function index are not a safe callable ABI. They do not establish object sizes/layouts, singleton addresses, calling convention at the native bridge, lifetime rules, load order, or whether these functions are reachable at a safe point from Glue5's Rust/WASM integration. The symbol list also does not prove that the game supports registering arbitrary add-on DLC at runtime.

Do not call these functions by guessed offsets or construct guessed RAGE objects. The next engineering step is to trace callers and object construction for the packfile streaming module and vehicle metadata loader in the WAT, then identify an existing in-binary registration path and validate it with the exact build. Only after that should a narrow native bridge be added.

## Runtime acceptance gate

1. Mount an isolated test pack without changing base game files.
2. Confirm the packfile streaming module discovers and opens the expected paths.
3. Confirm vehicle metadata is accepted by the game's existing metadata loader.
4. Confirm model/archetype registration and successful model streaming.
5. Expose the vehicle in the trainer only after the engine reports the model loaded.
6. Test spawn, textures/materials, driving, relaunch, uninstall, and unchanged vanilla vehicles.

This trace is investigation progress, not a claim that custom vehicles already load.