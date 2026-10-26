# Android ZIP mod installer (draft)

This branch adds a first-pass ZIP installer for **Glue 5 Luau script mods**.

## ZIP format

Supported layouts:

- `weather_mod/main.luau` (optional `mod.json` beside it)
- `main.luau` at ZIP root (the installer derives the name from the archive filename, or accepts a requested name)

The archive is limited to 64 MiB unpacked and 2,000 entries. ZIP paths are checked, existing mods are not overwritten, and the install is staged before it is committed.

## How to try it

1. Build and install the APK from this branch.
2. Open Glue 5 Settings → Mods → Studio and turn Studio on.
3. In the Studio section, tap **Copy link** next to **Install Mods**.
4. Open the copied URL in the browser on the same device, select a ZIP, and install it.
5. Enable Mods and start/restart the game as prompted.

The Studio link contains a temporary pairing token. Do not share it with other people.

## Compatibility limits

This is not a general PC GTA V mod installer. It only installs folders containing Glue 5's `main.luau` script. Native `.asi` plugins, Script Hook V mods, iOS executable code, and OIV package installers are not made Android-compatible by extracting their archives. A mod can still fail if it calls game functions that Glue 5 does not expose or the game's build does not support.

## Status

The code has been added on a feature branch but needs a successful Android build and on-device testing before it should be treated as a working release.
