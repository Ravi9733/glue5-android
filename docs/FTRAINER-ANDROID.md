# FTrainer Android adaptation for GLUE 5

This is a native GLUE 5 Luau mod, not the Windows trainer binary and not a
separate Android app. GLUE 5 draws its menus using its existing touch and
controller UI. During play, open the menus with F7 or the on-screen mods-menu
button beside Settings; the Mods page also has a MENUS tab while mods are
running.

## Included menus

- **Vehicles**: pages generated from `world.vehicleModels()`, the catalog read
  from model archetypes loaded by the installed game.
- **Vehicle options**: repair/clean, flip upright, maximum available upgrades,
  vehicle invincibility.
- **Vehicle appearance**: primary/secondary paint IDs and neon lights.
- **Player options**: restore health, full armour, god mode.
- **Weapons**: pages generated from `world.weapons()`, give selected weapon,
  give all catalogued weapons, remove all weapons, infinite ammo clip.
- **World options**: weather selection and time of day.
- **Teleport locations**: airport, Legion Square, Mount Chiliad, Sandy Shores,
  Vinewood Sign, Fort Zancudo and Maze Bank Tower.

## Install and startup behavior

On first opening GLUE 5's mods directory, the app copies this bundled mod to
its private `mods/ftrainer_android` folder only when those files do not exist.
Existing user edits are deliberately not overwritten. The bundled
`mod.json` enables autostart when Mods are enabled.

Source review confirms the app opens/installs the mods directory when Mods are
enabled, refreshes the mod list after the bundled files are copied, and starts
autostart mods with the game. The mod VM waits until the loading screen is gone,
the screen is faded in, and the player is playing before executing the trainer.
The game-world catalog is read after the engine reports that the player is
ready. These are source-level findings, not a device/runtime test.

## DLC and add-on vehicle boundary

The spawner lists vehicle models present in GLUE 5's live game catalog. A DLC
or add-on vehicle can appear only if the game has already loaded its model
archetype into that catalog. This trainer does **not** mount RPF/DLC packs,
install add-on files, or force an unloaded model into the game. If a vehicle
does not appear in the list, the game-side DLC mounting/loading path must be
fixed first; the trainer cannot make an unavailable model spawn by itself.

## Compatibility and validation

The trainer uses GLUE 5's native Luau API and live model catalogs. The actual
native set is generated from the installed game's `game.wasm`; the API
available can therefore vary by game build. Individual optional actions are
guarded so an unavailable native reports an in-game message instead of
silently claiming success. Exact native availability and parameter
compatibility still need checking against the target game build.

The supplied upstream repository
(`FTrainer-GTAV/Trainer-GTA-V`) currently contains no usable trainer source
tree; its release asset is `GTA_V_NS_Trainer_Skin.apk`. This GLUE 5 work is
therefore an independent Android-native adaptation using GLUE 5's own mod
system, not a binary port of that APK.

## Source audit completed so far

- [x] Confirmed menu method signatures and callback values against
      `menus.rs`: choice callbacks receive a 1-based index, matching the
      trainer's page selection logic; each menu stays below the 64-item cap.
- [x] Confirmed the mods lifecycle: bundled files are copied before
      `refresh()`, autostart mods are queued before runtime start, and the VM
      waits for the loading screen to end and the player to be playing.
- [x] Corrected strict Luau handle inference: the protected vehicle is typed
      as `Vehicle?`, and teleport's target is typed as the common `Entity`
      handle so either a Ped or Vehicle can be moved.
- [x] Confirmed the vehicle/weapon catalogs are generated from the installed
      game's live archetypes and are not hard-coded lists.

## Remaining validation before the one final APK

- [ ] Check every native name and signature against the target game's generated
      native definitions.
- [ ] Run Luau diagnostics for `mods/ftrainer_android/main.luau`.
- [ ] Test all menus in Story Mode, including spawn/model-load timeout,
      vehicle upgrades/appearance, weapons, weather/time and teleport.
- [ ] Test DLC vehicles that are actually mounted and loaded by the game.
- [ ] Test mod stop/restart, user edits, and low-memory behavior.
- [ ] Review the complete source diff and preserve existing graphics,
      Turnip/Vulkan, performance settings and touch controls.

No APK has been built and no device/runtime test has been performed. Do not
start the final build until the source checks above are complete.
