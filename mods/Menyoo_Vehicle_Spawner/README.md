# Menyoo Vehicle Spawner

A first Android-native GLUE 5 vehicle-spawner mod.

- Reads the installed game's vehicle model catalog.
- Provides a paged touch menu (28 models per page).
- Requests and waits for the selected model before spawning.
- Spawns a few metres ahead of the player and seats the player in it.
- Uses GLUE 5's existing Luau/native bridge; it is not a Windows ScriptHookV ASI.

## Current scope

This first pass covers models already registered in the running game's catalog. Add-on DLC/RPF mounting and registering custom vehicle archives are separate work and are not claimed by this mod.
