# GTAV Mobile Trainer — GLUE 5 port

This is a native Luau reimplementation for GLUE 5. It does not load or inject
the original Fuzzy-specific Android trainer libraries.

## Included source controls

- Spawn a selected vehicle.
- Save and return to a position.
- Give a selected weapon.
- Repair the current vehicle and start/stop its engine.
- Restore health and apply full armour.
- Set or clear the wanted level.

These are source-level integrations. Their native availability/signatures must
be checked against the exact `game.wasm` used by the build, then exercised
in-game before they can be called verified working.

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
