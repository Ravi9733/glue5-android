# Android add-on vehicle DLC

Glue 5 can discover external GTA V RPF7 add-on packs in a separate `mods`
folder beside the selected game folder. The original `game/data` tree is
read-only and is never rewritten.

## Folder

For the common layout where the game is `/storage/emulated/0/GTA5/game`,
put packs here:

```text
/storage/emulated/0/GTA5/
├── game/
│   ├── data/
│   └── b/
└── mods/
    └── update/
        └── android/
            └── dlcpacks/
                └── pack_name/
                    └── dlc.rpf
```

The Android OpenIV layout above is preferred. The runtime also recognizes
`mods/update/x64/dlcpacks/<pack>/dlc.rpf` and
`mods/x64/dlcPacks/<pack>/dlc.rpf`.

Each pack folder name must contain only letters, digits, underscore or hyphen.
The archive must be a readable RPF7 file. Do not put the archive itself directly
inside `dlcpacks`; it must be inside its own named folder.

## Loading and spawning

1. Copy the complete add-on pack folder, including its `dlc.rpf`, into
   `mods/update/android/dlcpacks/`.
2. Fully close Glue 5 and start it again. DLC packs are discovered at game
   startup; adding or removing packs while the game is running does not hot-mount
   them.
3. Open Glue Trainer → **Add-on Cars**.
4. Use **Search installed cars** if the game registered the vehicle, or open
   **Spawn by model name** and enter the exact model name from the pack's
   `vehicles.meta` (the `modelName` field).

Glue 5 stages an augmented copy of the game's `common/data/dlcList.xml` in its
own user cache. The original XML and all original game files remain unchanged.
The game's native DLC loader remains responsible for parsing the RPF, loading
its `setup2.xml` and `content.xml`, and registering the vehicle. Unsupported,
encrypted, malformed, or incompatible packs may still be rejected by the game.

## Notes

- This is for local, fully downloaded game folders. Streamed sessions do not
  expose an external local DLC folder.
- Use add-on packs made for the same GTA V game build/platform. Desktop-only
  packs, scripts, trainers, and graphics injectors are not automatically
  compatible with this Android build.
- If a vehicle does not appear after restart, check the Android log for
  `DLC mods:` messages and verify that the pack's `setup2.xml`,
  `content.xml`, and `vehicles.meta` are valid for this game build.
