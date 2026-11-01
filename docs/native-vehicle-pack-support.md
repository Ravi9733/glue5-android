# Native GTA V vehicle-pack support

## Current status

The existing ZIP importer in `glue5-mods` installs Luau script mods. It does not install or register native GTA V vehicle assets. A script calling `streaming.requestModel` cannot make a missing model's geometry and textures appear.

The current game hook for `HasModelLoaded` resolves archetypes by hash; it is not an asset loader. Do not report a vehicle pack as installed/usable merely because a model name appears in a spawner list.

## Candidate format-support components

The feature branch now integrates `rpf-archive` for RPF7 inspection/extraction and `rage-formats` for YFT/YTD validation. The DLC scanner can inspect readable pack metadata and count validated or unverified resources. Encrypted RPF archives remain limited by required game keys.

These libraries are format readers, not the game's asset streaming system. Parsing YFT/YTD bytes does not register archetypes, construct drawable resources, upload textures, or connect custom assets to the RAGE streaming lifecycle. No custom vehicle should be shown as spawnable based on parser success alone.

## Remaining engine work

1. **Packfile registration** — discover and verify a real Glue5 WASM/RAGE packfile open/register path. Windows FiveM/ASI hook names or signatures must not be copied into this build without evidence.
2. **DLC activation** — integrate the pack into the game's DLC load order without corrupting the user's base files. The current scanner only reads imported `dlclist.xml` and `setup2.xml`; it does not change the game's active DLC list.
3. **Asset registration and streaming** — connect YFT/YTD resources, archetypes, drawable/model resources, textures, handling and vehicle metadata to the engine's resource lifecycle.
4. **Spawner integration** — expose only vehicles whose packfile and assets were registered and loaded successfully. Preserve all original vehicle entries.
5. **Validation** — test one real add-on pack on Android, confirm spawn, textures/materials, driving, reload, uninstall and base-game integrity before claiming compatibility.

## Compatibility gate

PC add-on packs are not automatically compatible with this Android build. Initial support should be explicitly limited to formats validated by tests; do not promise arbitrary ZIP/RPF packs.

## Acceptance criteria

- Invalid or unsupported archives fail safely with a useful reason.
- Failed installs leave no partial files and do not modify base game data.
- A successfully installed vehicle appears in the spawner only after asset registration succeeds.
- Original vehicles remain available.
- Uninstall removes only files owned by that installed pack.
- Turnip driver configuration remains unchanged.