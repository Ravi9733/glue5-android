# Iron Man mod for Glue5 Android

This is a **starter visual mod**, not a complete Iron Man gameplay conversion.

## Install

Copy the `iron_man` folder into:

`Android/data/<Glue5 package>/files/game/mods/`

The folder name must remain lowercase: `iron_man`. Keep `main.luau` and `mod.json` inside it. Restart Glue5 after copying.

## Included

- Iron Man menu
- Visual-mode toggle using the same graphics/timecycle API pattern as the project's existing graphics mod
- Arc Reactor status notification
- Reset visual effects button

## Not included yet

Flight, suit replacement, repulsor projectiles, and combat are **not implemented** because the relevant Glue5/GTA native APIs have not been verified against this build. The menu reports this instead of pretending those features work.
