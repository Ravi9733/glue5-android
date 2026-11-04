# Glue5-native trainer example

This is a **source-based Lua/Luau adaptation** for Glue 5's GTA V WASM build. It is not a Windows PE loader and does not make arbitrary PC `.asi` or `.dll` plugins executable.

## Features

- Touch-friendly Glue 5 menu
- Weather presets via the generated native database
- Clock-hour slider and apply action
- Runtime error notifications instead of silently pretending an action succeeded

## Install

Copy this folder's `main.lua` into a mod folder in Glue 5's mods directory, for example:

```text
mods/
└── Glue5Trainer/
    └── main.lua
```

Enable/start the mod in Glue 5's mod manager. If the mod manager expects a different mods directory for your app build, use the directory already used by your working Lua mods.

## Compatibility boundary

The script uses Glue 5's checked native bridge and its aliases for `SET_WEATHER_TYPE_NOW_PERSIST` and `SET_CLOCK_TIME`. If either native is missing from the generated database for the supplied game build, the menu reports the error.

This is a first concrete trainer port, not full ScriptHookV compatibility. It does not run Windows x64 machine code, load `.asi`/`.dll` files, emulate Windows APIs, or implement the complete ScriptHookV ABI. Source-based trainers can be adapted feature by feature when their behavior can be expressed using Glue 5's exposed natives.

## Verification

The source has been added to the repository, but it has **not** been compiled or tested in-game in this change. Do not treat it as a verified working APK.
