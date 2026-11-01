# PC mod import status (Android)

This page describes the current feature-branch implementation. It is intentionally precise: importing a ZIP is not the same as registering its native assets with RAGE.

## What works in this branch

- Android document picker imports a ZIP into Glue5's private inbox.
- ZIP inspection rejects unsafe paths, duplicate paths, symlinks, excessive entry counts and oversized content.
- Packages are extracted into an isolated `mods/.pc-packages/<package>` directory. The original game data directory is not overwritten.
- Imported loose files are exposed as read-only `/game` overlays on the next game launch. A package file at the same relative path takes precedence over the base file.
- RPF7 archives can be inspected and, where the archive is readable, entries can be inventoried/extracted for metadata inspection.
- The DLC scanner searches imported packages for `dlcpacks/<name>/dlc.rpf`, reads accessible `setup2.xml` / `dlclist.xml`, and attempts YFT/YTD resource parsing.
- Supported JSON placement files are validated and converted into an opt-in Luau mod. It uses models that are already present in the game's loaded asset catalog.

## JSON placement format

Save a UTF-8 file such as `city_props.json`:

```json
{
  "objects": [
    {
      "model": "prop_roadcone02b",
      "x": 1.0,
      "y": 2.0,
      "z": 3.0,
      "heading": 90.0
    },
    {
      "model": "prop_crate_11",
      "x": 4.0,
      "y": 2.0,
      "z": 3.0
    }
  ]
}
```

Each item requires `model`, `x`, `y`, and `z`; `heading` is optional and defaults to zero. Model names are restricted to ASCII letters, digits and underscores. The importer accepts at most 5,000 placements and 4 MiB of JSON. Import creates a disabled-by-default script mod; enable/start it from the Mods page. Coordinates are world coordinates, not offsets from the player.

## Suggested ZIP layout for an add-on pack

```text
my_addon.zip
└── dlcpacks/
    └── my_addon/
        └── dlc.rpf
```

Keep the original relative paths inside the ZIP. The scanner reports pack discovery, setup order, imported dlclist registration and resource parse counts. It does not edit the base game's `update.rpf` or automatically add entries to the game's real DLC list.

## Not implemented / not verified yet

- **RPF runtime mounting:** the VFS can overlay loose files and supply an RPF file at its expected path. That does not itself mount an archive; the engine's packfile manager must successfully open/register it.
- **DLC activation:** discovering `dlcpacks`, parsing `setup2.xml`, or finding a `dlclist.xml` does not register a pack with RAGE.
- **Custom YFT/YTD loading:** parsing a resource header is not equivalent to registering its dictionary/archetype with the running game. Custom add-on vehicles are not yet guaranteed to appear in the F7 vehicle catalog.
- **YMAP streaming:** the JSON importer is a limited object-placement bridge. It is not a YMAP parser or a general PC map-mod loader.
- **ASI/DLL:** Windows native plugins are not loaded or executed on Android. Their features need a safe Luau/Rust port or a purpose-built Android-compatible implementation.

## Safe test sequence

1. Back up the game data before testing any experimental package.
2. Enable Mods and restart the game after importing a ZIP; overlays are initialized at startup.
3. Use **Scan DLC packs** to inspect imported DLC metadata. Treat encrypted/unverified resources as unknown, not as corrupt.
4. For a JSON smoke test, import a small file with one or two common base-game prop models. Enable the generated map mod and start it while in a safe open area.
5. Check the Mods log for the generated script's `PC map finished` count. A successful import only confirms validation and script creation; the in-game spawn still needs device testing.

## Build/test status

No Android APK or device runtime test is implied by this document. The feature branch must pass Rust checks and then be exercised on the Turnip Android build before compatibility claims are upgraded.
