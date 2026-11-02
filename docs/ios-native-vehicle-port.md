# iOS native vehicle-spawn port notes

## Source inspected

The supplied `Payload.zip` contains `Payload/PlayGTA5Native.app/NativeVehicleSpawn.inc`. This is C source included by the iOS app's engine host, not merely a UI mock-up. It targets game build `8b0b5899ed` and documents its expected `game.wasm` layout.

The supplied Menyoo archive contains `Menyoo-iOS.asi`, Menyoo data/catalog files and a manifest. The ASI starts with the Windows `MZ` executable signature and is a PE32+ x86-64 DLL. It imports the Windows ScriptHookV ABI. It is not an Android ARM64 shared library and cannot be loaded directly by Glue5 on Android. Its data files and feature behavior can still inform a native/Luau reimplementation.

## Verified iOS vehicle-spawn behavior

The iOS implementation provides a useful reference architecture:

1. It enumerates the engine's model registry incrementally, 512 model slots per frame, and keeps only entries whose model type is a vehicle. It skips trains and records model hash, class, internal name and localized label.
2. The menu thread queues a requested model; it does not call game natives from the controller/UI thread.
3. The game-thread tick waits until gameplay is active, enters the current script context, verifies that the owning script handler is still valid, and then requests the model.
4. It waits for model readiness with a bounded timeout, checks memory headroom, creates the vehicle, grounds it where appropriate and optionally seats the player.
5. It caps the number of vehicles owned by the menu and only removes an older unoccupied vehicle that the menu still owns.
6. It logs pending streaming objects when a spawn takes too long, so a missing asset can be distinguished from a model that is still streaming.

These lifecycle and threading rules are more important than copying the iOS UI.

## Android Glue5 comparison

The Android branch already has a touch-accessible native trainer tab and a Luau vehicle spawner. The trainer's current catalog uses `world.vehicleModels()`, filters with `streaming.isModelInCdimage` and `streaming.isModelAVehicle`, then calls `streaming.load` and `vehicle.createVehicle`.

The Android PC ZIP importer currently extracts/installs package files and inspects RPF/YFT/YTD metadata. It does not mount arbitrary RPF packs, register custom archetypes, or connect custom assets to the RAGE streaming lifecycle. Therefore, making an imported car appear in a list is not sufficient to make it spawnable.

## Android implementation plan

### A. Keep the existing safe Luau path

- Keep the current touch trainer and vanilla vehicle spawner working.
- Do not pass raw Windows ASI/DLL binaries to an Android loader.
- Show a clear import result when a ZIP contains Windows PE plugins; tell the user that those features need a native Android/Luau port.

### B. Port the vehicle catalog and spawn lifecycle

- First verify the exact Android `game.wasm` SHA-256 and compare it with the iOS payload's pinned build.
- Trace the Android runtime's existing WASM/native bridge and script-thread scheduling. Reuse those abstractions rather than calling iOS function addresses from an unrelated thread.
- If the exact engine layout is confirmed, implement incremental model-registry enumeration behind a Rust API and expose a safe catalog to Luau.
- Queue spawn requests and execute native calls only on the engine's script/game thread.
- Preserve bounded model-loading timeouts, script-owner checks, memory checks, entity ownership and cleanup.
- Expose a custom vehicle in the trainer only after the engine reports the model is registered and loaded.

### C. Add-on pack loading (separate prerequisite)

- Trace the real Android packfile mount and streaming registration callers, constructors and object lifetimes in the exact WASM build.
- Implement isolated pack mounting and DLC metadata activation without modifying base-game files.
- Connect YFT/YTD, archetypes, drawable resources, textures, handling and vehicle metadata to the engine's normal streaming lifecycle.
- Test one known pack end-to-end before claiming general PC add-on compatibility.

## Current status

- iOS payload and Menyoo ZIP inspected.
- iOS native vehicle-spawn implementation inspected.
- Android importer now detects Windows PE `.asi`/`.dll` plugins and reports that they cannot execute on Android.
- No Android add-on RPF mount or custom vehicle spawn has been verified yet.
- No claim of Menyoo compatibility or Mazda Furai runtime success is made by this document.


## Full supplied Payload.zip inspection (1.4.7)

The supplied archive was extracted and its complete app-bundle file inventory checked (106 files). The app is `PlayGTA5Native.app`, version 1.4.7 (build 17), Mach-O ARM64, minimum iOS 16. It contains no source project or separate Frameworks directory. The relevant source-like includes are:

- `NativeVehicleSpawn.inc`: the native model-catalog and spawn implementation described above.
- `NativeCheats.inc`: game-thread native cheat actions.
- `NativeDebugChannel.inc`: a debug command channel that queues native calls to the game thread, with bounded waits and memory-range checks.
- `asi-natives.inc`: the port's native-name/hash/handler mapping.
- `manifest.json`, `bootset_native.json`, `bootset_low.json`, shader reflection and Metal shader assets.

The payload's `NativeVehicleSpawn.inc` explicitly pins its engine memory layout to build `8b0b5899ed`. It also documents the important threading rule: model-pool enumeration and vehicle creation must run during the main script's game-thread turn; creating vehicles on the controller/UI thread raced the engine vehicle-pool walk and crashed after repeated spawns.

## Supplied Menyoo ZIP inspection

The archive contains 57 files, including `Menyoo-iOS.asi`, `Menyoo-iOS.json`, and about 30 MB of Menyoo data/catalog files. The data includes vehicle/ped lists, Spooner map data, timecycle modifiers, outfit/vehicle presets, animation and speech catalogs, and graphics assets.

Important binary facts from the actual file:

- `Menyoo-iOS.asi` is PE32+ x86-64, not an iOS ARM64 Mach-O or Android ARM64 ELF library.
- Its import table requests `ScriptHookV.dll` and Windows APIs from `KERNEL32.dll`.
- Its manifest says `gamePort: 1.4.1`, engine `8b0b5899ed`, `runtimeTestedOnDevice: false`.
- Its installation notes describe a built-in ASI/Blink loader and say the build's basic navigation was tested on Mac with mocked natives, while full iPhone-device testing was not complete.

Therefore, the user's observation that a Menyoo-labelled build works on an iPhone should not be dismissed, but this exact supplied archive does not prove that the x86-64 PE file itself was executed natively on an iPhone. The actual runtime path needs to be verified against the iOS loader. The same `game.wasm` build ID does not make the iOS and Android host/plugin ABIs identical.

## Android Glue5 runtime boundary

The Android branch already has a different, working extension mechanism:

- `glue5-mods/src/engine.rs` hooks the engine's script `WAIT` update and runs each mod in its own game script thread.
- `glue5-mods/src/natives.rs` invokes registered game natives through Glue5's WASM runtime bridge, with typed parameters and per-call error handling.
- `glue5-mods/src/vm.rs` hosts the Luau VM; `glue5-mods/src/menus.rs` maps touch UI input to mod callbacks.
- `glue5-mods/src/lib.rs` explicitly documents that imported PC packages are isolated data only: RPF archives are not mounted and ASI/DLL files are not executed.

This is a strong base for a native Menyoo-compatible port, but it is not yet an ASI ABI. Reusing the game build's native table and script-thread scheduler is safer than copying iOS absolute addresses or attempting to load the x86-64 PE into the Android ARM64 process.

## Porting decision

1. Preserve the Menyoo data/catalogs and map its feature modules to Glue5's typed native and touch-menu APIs.
2. Reimplement Menyoo feature logic in Android-native Rust/Luau (or obtain and port its matching source archive); do not treat the PE binary as an Android executable.
3. Port the iOS vehicle catalog/spawn lifecycle using the Android runtime's own build-verified native handlers and script thread.
4. Implement RPF/DLC mount and custom archetype streaming separately; the iOS vehicle spawn include alone does not provide add-on asset mounting.
5. Only claim original Menyoo binary compatibility if the actual iOS loader's execution/translation model is reproduced and an ARM64 Android test proves it. Otherwise label the result as a feature-complete native port, not direct ASI execution.
