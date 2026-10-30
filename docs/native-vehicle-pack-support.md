# Native GTA V vehicle-pack support

## Current status

The existing ZIP importer in `glue5-mods` installs Luau script mods. It does not install or register native GTA V vehicle assets. A script calling `streaming.requestModel` cannot make a missing model's geometry and textures appear.

The current game hook for `HasModelLoaded` resolves archetypes by hash; it is not an asset loader. Do not report a vehicle pack as installed/usable merely because a model name appears in a spawner list.

## Candidate format-support components

The source tree currently has no native GTA V RPF/YFT/YTD parser dependency. Two Rust libraries are promising candidates for a native support layer:

- `rage-rpf` — RPF7 archive parsing/decryption and extraction. Start with unencrypted/Open mod archives; encrypted retail archives must not be assumed supported without the required keys and a clear user-owned-game workflow.
- `rage-formats` — parsing GTA V resource formats including YFT fragments and YTD texture dictionaries. Parsing a resource is not the same as registering it with Glue5's renderer or game streaming system.

These are candidates, not integrated dependencies or verified Android builds. Before adding them to the app, check their current APIs, licenses, transitive dependencies, Android/arm64 compilation, binary size and performance. Pin versions and add Android CI coverage. Do not add a dependency or show an install-success state until those checks pass.

## Required implementation stages

1. **Archive inspection** — identify supported pack layouts and enumerate `.yft`, `.ytd`, `.meta`, and RPF contents without extracting unsafe paths. Keep script-mod ZIP import separate.
2. **Format support** — use a compatible RPF parser and resource parsers for explicitly supported archive/resource versions. Reject unsupported or encrypted archives explicitly.
3. **Asset registration and streaming** — integrate vehicle archetypes, drawable/model resources, textures, handling and vehicle metadata with the game's resource/streaming lifecycle. A metadata-only registry is insufficient.
4. **Android storage** — install into a dedicated user-visible game-mod directory with staging, rollback, size limits and uninstall; never overwrite base game data.
5. **Spawner integration** — list only vehicles whose assets registered and loaded successfully. Preserve original vehicle entries.
6. **Validation** — test one replacement/add-on pack on Android, confirm spawn, texture/material rendering, driving, reload, uninstall and base-game integrity before claiming compatibility.

## Compatibility gate

PC add-on packs are not automatically compatible with this Android build. Initial support should be explicitly limited to formats validated by tests; do not promise arbitrary ZIP/RPF packs.

## Acceptance criteria

- Invalid or unsupported archives fail safely with a useful reason.
- Failed installs leave no partial files and do not modify base game data.
- A successfully installed vehicle appears in the spawner only after asset registration succeeds.
- Original vehicles remain available.
- Uninstall removes only files owned by that installed pack.
- Turnip driver configuration remains unchanged.