# PC Trainer V — Android adapter prototype

This folder documents the Android-side adaptation target for the PC Trainer V
(Official) feature set. It is **not** the original Windows ASI and is not a
binary loader. The upstream PC trainer expects ScriptHookV and the Windows
GTA V executable; Glue 5 uses its own WASM-backed runtime.

## Prototype scope

Build an Android-friendly trainer menu using Glue 5's existing Luau menu/native
bridge. Work in small increments and retain a safe default state:

- First milestone: time, weather, and basic player/vehicle status actions only
  after checking exact native names/signatures in the generated database.
- Later milestones: health/armour, wanted-level controls, teleport, vehicle
  repair/customisation, and vehicle spawning, each separately validated.
- Features that depend on PC memory layouts, ScriptHookV APIs, Windows DLL
  loading, external texture archives, or unavailable game-build natives are
  explicitly unsupported until proven otherwise.
- Use touch-friendly menu items and show a clear notification when an action
  cannot run. Avoid auto-running destructive actions at game start.

## Test gate

Do not ship this as a working APK until it compiles and the initial supported
actions have been tested in-game on Android. This README is a scoped prototype
plan, not a claim that the trainer or its features already work.
