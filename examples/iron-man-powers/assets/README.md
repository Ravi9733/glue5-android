# Iron Man-style armor prototype assets

This folder contains two experimental preparation paths:

- `build_armor_prototype.py` creates a low-poly red/gold GLB preview and texture atlas. The resulting GLB is not a GTA V ped asset.
- `armor_overlay.py` is a Blender script that takes an existing rigged ped FBX template, adds rigid red/gold armor plates parented to recognizable GTA-style skeleton bones, and exports an intermediate FBX.

## Blender overlay usage

1. Obtain a legally sourced/exported rigged ped FBX compatible with the target GTA V skeleton and back it up.
2. Install Blender with FBX import/export support.
3. Run from a terminal:
   `blender -b rigged_ped_template.fbx --python armor_overlay.py -- --out iron_man_overlay.fbx`
4. Inspect the result in Blender. The plate positions and dimensions are only a first pass and may need adjustment to the chosen template.

## Critical limits

The exported FBX is an **intermediate**, not a GTA V-ready `.ydd/.yft` character. This script does not automatically create proper skinned armor weights, GTA V shader/material definitions, collision, drawable/fragment metadata, or a DLC/resource pack. A final asset must be converted and checked with a compatible GTA V toolchain (for example, Sollumz in Blender) and tested in the exact game build.

Glue5's documented mod API can load models already present in the installed game. The current mod script uses GTA's player-model native for built-in models; it cannot directly load a GLB/FBX from a mod folder. No custom Iron Man ped has yet been successfully loaded in Glue5. Do not treat these prototype files as a completed replacement.
