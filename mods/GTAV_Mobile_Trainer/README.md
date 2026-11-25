# GTAV Mobile Trainer — GLUE 5 port

This is a native Luau reimplementation for GLUE 5. It does not load or inject
the original Fuzzy-specific Android trainer libraries.

## Included source controls

- Spawn a vehicle from GLUE 5's installed-game vehicle catalogue.
- Save and return to a position.
- Give a selected weapon.
- Repair the current vehicle and start/stop its engine.
- Restore health and apply full armour.
- Set or clear the wanted level.
- Toggle player god mode, never-wanted protection, and infinite ammo clip.

These are source-level integrations. Their native availability/signatures must
be checked against the exact `game.wasm` used by the build, then exercised
in-game before they can be called verified working.

## What was found in the supplied trainer APK

The supplied APK is a Fuzzy-targeted Android trainer, not a standalone GTA
mod package. Its DEX and native libraries contain hard-coded references to
`com.thegwchr.fuzzy`, `com.thegwchr.fuzzy:game`, and JNI entry points named
for `com.thegwchr.fuzzy.GTAVNativeActivity`. The native agent searches for
`libgtav.so` and hooks its GOT; the Java side also uses Fuzzy-specific
private/external storage paths and process-launch commands.

Therefore its `libopeniv.so`, `libshv.so`, and Java injection/service layer
cannot simply be copied into GLUE 5. The GLUE 5 branch is porting the
trainer's in-game actions to GLUE 5's own Luau/native bridge instead. The
current source adds a live installed-vehicle catalogue and player-protection
toggles in addition to the earlier vehicle, teleport, weapon, repair,
health/armour, and wanted-level controls.

The source has been updated on the `trainer-glue5-port` branch. This is not
yet a built or device-tested APK: the exact game-native definitions still
need to be generated/checked from the target `game.wasm`, then the Android
APK must be built and tested in-game.

## OpenIV / DLC compatibility boundary

GLUE 5 currently has a **loose-file overlay**, not a complete OpenIV package
or GTA V DLC archive mount system. The runtime overlay resolves replacement
files beneath the configured graphics-mod directory (or the legacy
`user/mods/files` directory), and only accepts a replacement when its byte
length exactly matches the original game file. This is deliberate: the engine
may size reads from the game's manifest.

Consequences:

- A loose replacement for an existing game-data path can be considered only
  when it passes the same-size check.
- A normal OpenIV `.oiv` installer is not executed by this module.
- Arbitrary `.rpf` archives, DLC pack registration, `dlclist.xml` expansion,
  and new vehicle assets are **not** mounted by this trainer yet.
- The original APK's `libopeniv.so` / Fuzzy integration is not a substitute
  for a GLUE 5 asset-mount bridge.

A full DLC target requires a separate runtime change: a safe archive/pack
mount layer integrated with GLUE 5's game file device, plus validation of
archive indexing, path precedence, streaming reads, and Android storage
permissions. Until that exists, this port must not advertise full DLC support.
