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
