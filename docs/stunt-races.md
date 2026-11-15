# Glue 5 Stunt Races

This feature is an offline/Sandbox trainer workflow. It does not call Menyoo, ScriptHookV, or GTA Online race services.

## Importing a track

1. Download a compatible map/job archive to Android storage.
2. If the download is RAR or 7z, extract it first. ZIP, XML and Rockstar Creator JSON can be selected from the in-game Android picker.
3. Open **Mods → STUNT RACES**.
4. Use **Import XML / ZIP** for Menyoo Spooner or CodeWalker/YMAP object XML.
5. Use **Import Rockstar UGC JSON / ZIP** for a Rockstar Creator job JSON. The JSON must contain `mission.race.chl`; track props are read from `mission.prop` and `mission.dprop`.
6. Restart Glue 5 after importing or deleting so the trainer rebuilds its menu from the updated files.

Imports are copied into the trainer's own maps directory. ZIP extraction rejects path traversal and limits archive entries and expanded size. Map XML is limited to 4 MiB, Creator JSON to 8 MiB, and one ZIP import to 32 MiB total.

## Playing a race

- Choose **Start checkpoint race**, select a route, vehicle model name and 1–10 laps.
- The route needs at least two checkpoints.
- The trainer spawns the selected vehicle, counts down, draws route/checkpoint markers, tracks laps and elapsed time, and supports respawning at the last passed checkpoint.
- **Spawn selected Rockstar track props** separately to place the Creator track objects in the world. Use **Clear spawned track objects** when finished.
- The race and track-object spawning actions are guarded as Offline/Sandbox-only.

## Supported route formats

- Local route XML: `<Checkpoint><Position><X>…</X><Y>…</Y><Z>…</Z></Position></Checkpoint>`.
- Rockstar Creator JSON: `mission.race.chl` checkpoint coordinates; supported vector forms are XYZ objects, XYZ triples, flat XYZ numeric lists, and JSON-encoded coordinate arrays.
- Menyoo Spooner and CodeWalker/YMAP XML are object placement formats, not race-route formats. They do not inherently describe a checkpoint sequence. For these maps, use **Create checkpoint route** and capture checkpoints in driving order.

## Current limitations

- This is an offline trainer race, not a full reproduction of Rockstar's multiplayer race lobby, grid, catch-up, slipstream, vehicle-class rules or creator scoring.
- Creator static props are spawned frozen; `dprop` objects retain physics. Advanced fixture (`dhprop`), per-prop creator flags, collision overrides, prop tint, and specialized checkpoint variants are not implemented yet.
- Numeric model hashes must resolve against the active game's native database; unknown hashes are reported instead of guessed.
- RAR/7z must be extracted before import. Downloads are currently selected from Android storage rather than fetched by an in-game downloader.
- A successful parser CI run does not replace on-device gameplay verification. No APK is produced by the parser or stunt-race test workflows.
