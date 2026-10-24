# Armored Player Test — Glue5 Android

## What this revision does

The menu script uses Glue5's documented `streaming.load(model, nil, 10000)` API and the documented three-argument menu-button shape. It provides:

- **Use Juggernaut Armor** — switches the player to the built-in GTA V model `u_m_y_juggernaut_01`.
- **Restore Male Freemode** — switches back to `mp_m_freemode_01`.

This is a compatibility test using an existing game model. It is **not a custom Iron Man model**, and the source change has not yet been verified in a live Android game session.

## Install in Glue5

1. Back up the existing mod folder first.
2. Copy the entire `iron-man-powers` folder into the game's `mods` folder.
3. Confirm `mod.json` is present next to `main.luau`.
4. Start GTA V, enable mods, then open the Mods menu (F7 / the Mods menu button).
5. Open **Armored Player Test** and try **Use Juggernaut Armor**. Use **Restore Male Freemode** to return to the default male model.

If the menu stops with an error, open Glue5 Studio's mod log and share the exact error text. Do not delete your existing mods or saves.

## What is still required for custom Iron Man

Glue5's documented mod API loads models already present in the installed game; the mod folder cannot simply provide a GLB/FBX and have it become a GTA V ped. A custom suit still needs a compatible GTA V ped model, correctly weighted skeleton, GTA materials/drawable metadata, conversion to the game's native assets, and a verified loading/installation route for this Glue5 build. The Blender overlay script is an intermediate preparation experiment only; it is not a ready-to-install ped.

Do not call this a working custom Iron Man replacement until that complete asset pipeline has been built and tested in the target game.