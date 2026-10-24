# Iron Man Flight Controls for GLUE 5 Android

This example adds a touch-friendly **Iron Man Flight** menu:
- Start/stop continuous forward flight
- Ascend and descend
- Forward boost
- Land

It moves the game's current character using position natives. It is a script-based movement effect, not physics-based flight, and it has not yet been tested on the user's phone.

## Install

Copy this folder into the game's mods folder so the layout is:
`mods/iron-man-powers/main.luau`
and
`mods/iron-man-powers/mod.json`.

Restart Glue 5 with mods enabled. If the menu does not appear, check the mod's error/log entry in GLUE 5 Studio.

## Iron Man suit/model limitation

The script does **not** include an Iron Man 3D suit or textures. A real visual suit requires compatible character model assets and a verified way to load or replace the player model in this particular GLUE 5 game build. The modding API documents access to game natives and the models already present in the installed game; that does not by itself add a new custom model. No custom model has been bundled or tested here.
