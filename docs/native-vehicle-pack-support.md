# Native GTA V vehicle-pack support

## Current status

The existing ZIP importer in `glue5-mods` installs Luau script mods. It does not install or register native GTA V vehicle assets. A script calling `streaming.requestModel` cannot make a missing model's geometry and textures appear.

The current game hook for `HasModelLoaded` resolves archetypes by hash; it is not an asset loader. Do not report a vehicle pack as installed/usable merely because a model name appears in a spawner list.

## Required implementation stages

1. **Archive inspection** — identify supported pack layouts and enumerate `.yft`, `.ytd`, `.meta`, and RPF contents without extracting unsafe paths. Keep script-mod ZIP import separate.
2. **Format support** — determine which RPF variants and GTA V resource versions are present, then implement/read a compatible parser. Reject unsupported or encrypted archives explicitly.
3. **Asset registration and streaming** — integrate vehicle archetypes, drawable/model resources, textures, handling and vehicle metadata with the game's resource/streaming lifecycle. A metadata-only registry is insufficient.
4. **Android storage** — install into a dedicated user-visible game-mod directory with staging, rollback, size limits and uninstall; never overwrite base game data.
5. **Spawner integration** — list only vehicles whose assets registered and loaded successfully. Preserve original vehicle entries.
6. **Validation** — test one replacement/add-on pack on Android, confirm spawn, texture/material rendering, driving, reload, uninstall and base-game integrity before claiming compatibility.

## Compatibility gate

A real sample vehicle archive is needed before selecting the supported archive layout and resource versions. PC add-on packs are not automatically compatible with this Android build. Initial support should be explicitly limited to the formats validated by tests; do not promise arbitrary ZIP/RPF packs.

## Acceptance criteria

- Invalid or unsupported archives fail safely with a useful reason.
- Failed installs leave no partial files and do not modify base game data.
- A successfully installed vehicle appears in the spawner only after asset registration succeeds.
- Original vehicles remain available.
- Uninstall removes only files owned by that installed pack.
- Turnip driver configuration remains unchanged.
