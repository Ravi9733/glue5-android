# Armored Player Test — Glue5 Android

## Current status

This is a **built-in model compatibility test**, not a custom Iron Man suit. The menu script uses Glue5's documented `streaming.requestModel`, `streaming.hasModelLoaded`, `streaming.setModelAsNoLongerNeeded`, and three-argument menu-button API.

- **Use Juggernaut Armor** requests the built-in GTA V model `u_m_y_juggernaut_01`.
- **Restore Male Freemode** requests `mp_m_freemode_01`.
- Model loading has a frame-count timeout and reports an error instead of waiting forever.

This revision has **not been tested in a live Android game session**. A successful script commit is not proof that the model change works in your specific Glue5 build.

## Install

1. Back up the existing mod folder and save files.
2. Copy the entire `iron-man-powers` folder into the game's `mods` folder, or import a ZIP of the folder through Glue5 Studio.
3. Confirm `mod.json` is next to `main.luau`.
4. Enable mods, start the game, and open the Mods menu (F7 / the Mods button on touch screens).
5. Open **Armored Player Test** and try **Use Juggernaut Armor**. Use **Restore Male Freemode** to return to the default male model.
6. If it errors, send the exact error from Glue5 Studio's mod log. Do not delete your other mods or saves.

## Custom Iron Man still needs more work

Glue5's documented mod API loads models already present in the installed game; a GLB/FBX in a mod folder does not automatically become a GTA V ped. A custom suit still needs a compatible GTA V ped asset, a correct skeleton and skin weights, GTA material/drawable metadata, conversion to native game assets, and a verified way to install/load those assets in this Glue5 build. The Blender overlay script is an intermediate experiment, not a ready-to-install ped.

Do not treat this test as a completed or working custom Iron Man replacement until the custom asset is loaded and verified in-game.
