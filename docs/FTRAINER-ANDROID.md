# FTrainer Android adaptation for GLUE 5

This is a native GLUE 5 Luau mod, not the Windows trainer binary and not a
separate Android app. GLUE 5 draws its menus using its existing touch and
controller UI (F7 / Mods > MENUS).

## Included menus

- **Vehicles**: pages generated from `world.vehicleModels()`, the catalog read
  from models loaded by the installed game. This can include loaded DLC/add-on
  vehicle models; the mod does not hardcode a short vanilla-only list.
- **Vehicle options**: repair/clean, flip upright, maximum available upgrades,
  vehicle invincibility.
- **Vehicle appearance**: primary/secondary paint IDs and neon lights.
- **Player options**: restore health, full armour, god mode.
- **Weapons**: pages generated from `world.weapons()`, give selected weapon,
  give all catalogued weapons, remove all weapons, infinite ammo clip.
- **World options**: weather selection and time of day.
- **Teleport locations**: airport, Legion Square, Mount Chiliad, Sandy Shores,
  Vinewood Sign, Fort Zancudo and Maze Bank Tower.

## Install behavior

On first opening GLUE 5's mods directory, the app copies this bundled mod to
its private `mods/ftrainer_android` folder only when those files do not exist.
Existing user edits are deliberately not overwritten. The bundled
`mod.json` enables autostart when Mods are enabled.

## Compatibility and validation

The trainer uses GLUE 5's native Luau API and live model catalogs. The actual
native set is generated from the installed game's `game.wasm`; the API
available can therefore vary by game build. Individual optional actions are
guarded so an unavailable native reports an in-game message instead of
silently claiming success.

The supplied upstream repository
(`FTrainer-GTAV/Trainer-GTA-V`) currently contains no usable trainer source
tree; its release asset is `GTA_V_NS_Trainer_Skin.apk`. This GLUE 5 work is
therefore an independent Android-native adaptation using GLUE 5's own mod
system, not a binary port of that APK.

No APK has been built and no device/runtime test has been performed. Before
a final release build, validate the generated native definitions against the
target game build, run the Luau diagnostics, and test every menu in Story Mode
including DLC vehicle spawn, model-load timeout, mod removal/restart and
low-memory behavior.
