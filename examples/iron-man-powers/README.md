# Armored Player + Flight Controls for GLUE 5 Android

This mod adds a touch-friendly menu to change the player into a **built-in GTA V Juggernaut model** and restore the male freemode model. It also includes an optional armored NPC spawn button.

## Install

Copy this folder into the game's mods folder so the layout is:
`mods/iron-man-powers/main.luau`
and
`mods/iron-man-powers/mod.json`.

Enable mods in Glue 5 and open the mod's menu. If a model is unavailable in your installed game build, the mod reports a failure instead of silently claiming success.

## Important limitation

This is **not a custom Iron Man suit**. It calls GTA V's `SET_PLAYER_MODEL` native with an existing in-game model; it does not load the GLB prototype or custom textures. A true custom Iron Man character still needs a game-compatible ped asset, GTA V skeleton rigging and texture/material conversion, plus a verified asset-loading path in Glue 5. The player replacement code has been committed but has not yet been built and tested on the user's phone.
