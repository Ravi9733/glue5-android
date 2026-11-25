# GTAV Mobile Trainer port to Glue 5

## Scope

Port the useful trainer features into Glue 5's existing in-game Luau mod system. Do not load the trainer APK's native libraries into Glue 5 and do not attach to the Fuzzy process.

## Findings from GTAV MOBILE TRAINER.apk

- The DEX contains hard-coded references to `com.thegwchr.fuzzy` and `com.thegwchr.fuzzy:game`.
- Its auto-inject daemon polls that Fuzzy game process and uses `am attach-agent`.
- Its OpenIV paths are hard-coded under `Android/data/com.thegwchr.fuzzy/files/game/mods`, including `common.rpf` and `update/update.rpf`.
- It depends on Shizuku/Root process operations and stores an injected `libopeniv.so` under Fuzzy's private files directory.
- The APK's `libopeniv.so` and `libshv.so` are AArch64 ELF libraries, but architecture compatibility alone does not make their process/runtime interfaces compatible with Glue 5.

## Glue 5 integration points confirmed in source

- `crates/glue5-mods` already runs Luau scripts in the game's script threads and exposes game natives through generated namespace tables.
- `ui.menu(...)` supports touch/controller-friendly toggles, sliders, choices and buttons. The app displays mod menus from its Mods page.
- `vehicle.createVehicle`, `player.playerPedId`, and other natives are called through the game's own script context; entity handles are type-checked.
- `world.vehicleModels()`, `world.weapons()` and related catalog functions expose names from the installed game build.
- Mods live in a folder with `main.luau`; `mod.json` can enable autostart.

## Port plan

1. Build a Glue 5-native trainer mod with a menu for vehicle spawn, teleport presets, weapons and player/vehicle options.
2. Verify every native signature against the generated natives database before calling it; use only native calls supported by the installed game build.
3. Add safe model streaming, entity/pool checks, cleanup and bounded per-frame work.
4. Treat OpenIV/RPF loading as a separate compatibility project. Do not claim it works by copying the APK's Fuzzy-specific file paths. First establish whether Glue 5's prepared WASM game data and VFS can safely mount additional archives.
5. Test on Android with the actual Glue 5 build and inspect `game.log`; do not distribute an APK until the mod runs in-game.

## Current status

Source inspection and port branch are prepared. No APK has been patched and no feature is claimed working yet. The original trainer APK is retained unchanged.
