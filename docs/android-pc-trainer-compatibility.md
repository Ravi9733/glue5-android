# Android PC-trainer compatibility: feasibility and implementation plan

## Current status

This branch is a research/implementation branch only. It does **not** yet load Windows
`.asi` or `.dll` files, and it does not claim that PC trainers work on Android.

Glue 5 runs the supported GTA V WebAssembly build through wasm2c and a native host.
Its mod runtime in `crates/glue5-mods` runs Luau scripts in GTA script threads and
calls the game's generated native table. A PC trainer normally expects the Windows
GTA V executable, Windows DLL loading/ABI, and (for many trainers) ScriptHookV.
Those are different execution environments.

## Goal

Make trainer-like features comfortable to use on Android, while investigating
whether a specific trainer can be adapted. Do not execute arbitrary Windows DLLs
inside the Android process or silently label a Luau mod as an ASI loader.

## Staged plan

1. **Identify one target trainer.** Record its exact filename, version, and source
   or documentation. Inspect whether it is a ScriptHookV ASI, a ScriptHookVDotNet
   script, or a standalone DLL; these are different compatibility targets.
2. **Map its requested features** to the native functions available in this
   exact Glue 5 game build. Unsupported functions must be reported, not ignored.
3. **Prototype a Glue 5-side adapter** using the existing Luau mod runtime and
   native-call bridge. Keep Android touch-friendly menus and a default/off option.
4. **Validate in-game** with a small harmless feature first, then test each mapped
   feature and game stability. Build the APK only after the prototype compiles.
5. Consider a binary compatibility loader only if the target's dependencies and
   platform assumptions make it technically feasible. Generic compatibility for
   arbitrary Windows ASI/DLL trainers is not assumed.

## Acceptance criteria

- Existing game launch, saves, controls, and mods still work with the feature off.
- The adapter reports unsupported trainer functions clearly.
- No claim of PC binary compatibility until the actual target file is loaded and
  tested successfully on the Android device.
- APK build and device test results are recorded separately; a successful build
  alone is not proof that a trainer works.
