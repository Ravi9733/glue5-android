# Android mod compatibility (experimental)

This branch provides limited PC-style loose asset replacement and Luau runtime mods. It is **not** a general PC GTA V mod loader.

## 1. Runtime object placements

A Luau mod can put a `placements.json` file beside its `main.luau` and call:

```lua
world.spawnPlacements("placements.json")
```

The JSON format is:

```json
{
  "version": 1,
  "placements": [
    {
      "model": "prop_barrel_02a",
      "position": [10.0, 20.0, 30.0],
      "rotation": [0.0, 0.0, 90.0]
    }
  ]
}
```

The loader validates the schema, model-name characters, coordinate bounds, manifest size, and placement count before spawning. Models must already exist in the installed game. This does not install new vehicle, map, or texture assets.

## 2. Same-size loose replacements

An optional replacement can be placed at:

```
<Glue5 user data>/mods/files/<path relative to game data>/
```

For example, a replacement for `data/common/example.xml` goes at:

```
<Glue5 user data>/mods/files/common/example.xml
```

Only files already present in the installed game's data tree are considered. A replacement is mounted only when its byte size exactly matches the original file, because the engine's immutable manifest still supplies the original size. Built-in compatibility overlays take precedence. Canonical path checks reject symlink/path escapes. The replacement is read directly from the mods folder; it is not copied into the game data.

This restriction is deliberate: the engine receives an immutable file manifest with original file sizes. Different-size replacements need manifest and archive/VFS integration, which is not implemented yet.

## Not supported yet

- RPF archive mounting or DLC pack installation
- Adding new game assets or changing file sizes
- PC `.asi` / ScriptHookV plugins
- Windows, D3D11, or .NET compatibility layers

Use only mods and game files you are authorized to use. Back up game data before testing replacements.
