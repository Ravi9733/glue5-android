# Android mods and trainer

Glue 5 mods are **Luau scripts** that call the game's native functions through
Glue 5's mod runtime. Regular PC trainer DLLs, ScriptHookV files, ASI loaders,
and Xbox 360 mod files are not compatible just by copying them over.

## Install the example trainer

1. Open the exact game folder already used by Glue 5. It contains
   `b/<build-id>/game.wasm`, `b/<build-id>/shaders/`, and `data/`.
2. Create a `mods` folder beside `b` and `data`.
3. Copy the complete `examples/android-trainer` folder into `game/mods/`
   and rename it to `android-trainer`, so the final files are:
   `game/mods/android-trainer/main.luau` and
   `game/mods/android-trainer/mod.json`.
4. In Glue 5 Settings, turn **Mods** on. Fully close and reopen the app if
   you created `game/mods` while Glue 5 was already open.
5. Start the game. The example has `autostart: true`; open the in-game Mods
   menu with the touch menu button beside the settings button, or press F7
   when using a keyboard. Choose the **MENUS** tab if the Mods page opens.

The example trainer provides invincibility, wanted-level control, heal, clear
wanted level, and an Adder spawn button. The trainer menu is rendered by Glue 5
and its actions execute through the existing Luau/native bridge.

## Mod folder selection

Glue 5 prefers `game/mods/` when it exists. If it does not exist, Glue 5 uses
the older app-private mods folder, preserving existing installations. Mods
must be enabled in Settings before they run.

Only install scripts you trust: a mod can change gameplay by calling the game's
native functions. If a mod causes a problem, turn Mods off or remove that
mod's folder and restart.
