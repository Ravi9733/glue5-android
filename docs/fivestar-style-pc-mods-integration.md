# Five Star Port-style PC Mods for Glue 5 (Android/Turnip build)

## Scope confirmed
The goal is to bring the PC-mod installation and management experience from the supplied Five Star Port Android APK into the existing Glue 5 Android Turnip build. This is not a request to add Turnip to Five Star Port, and it is not a request to change the current renderer/driver work.

Keep Glue 5's existing Luau mods, GLUE 5 Studio, native trainer and user data. Add the PC-mod workflow alongside them.

## Existing Glue 5 foundations found on branch turnip-driver-integration
- crates/glue5-mods: Luau runtime, per-mod folders, state/logging, Studio and mod menus.
- crates/glue5/src/ui/mods.rs: existing in-app Mods page with Mods/Studio/Log tabs and enable switch.
- mods/glue-trainer: bundled Android-ready native trainer, including vehicle spawner/customization features.
- crates/glue5-mods/src/placements.rs: validated JSON placement manifest parser.
- crates/glue5-mods/src/pc_runtime.rs and glue5-pc-mod-runtime: currently classify PC plugin binaries and explicitly report that Windows PE/ASI execution is unsupported. This is diagnostic only, not an ASI loader.
- Android app user directory and persistent Mods folder are already part of the existing app architecture.

## Five Star Port workflow to reproduce
The reference app's documented PC-mod workflow accepts archives/loose files, unpacks them safely, previews where files will go, installs each mod into its own folder, and offers per-mod enable/disable/remove. It supports add-on DLC packs, OpenIV packages, replacement assets and data files, and a script/trainer path.

## Implementation phases

### Phase 1 — safe import and mod manager
1. Add Android document-picker import for ZIP first; add RAR/7z only when a reliable supported extractor is available.
2. Stream archive entries to disk; enforce archive/file count and expanded-size limits; reject absolute paths, '..', symlinks and path traversal.
3. Inspect the archive and produce a preview: detected mod name/type, target paths, conflicts, skipped/unsupported items.
4. Install under a separate persistent Mods/pc/<mod-id>/ directory. Never overwrite original game files during import.
5. Add enable/disable/remove controls and a per-mod log. Disabled mods must not be mounted into the game VFS.
6. Preserve existing Luau mods and Studio data; never delete or rewrite user game saves.

### Phase 2 — archive and asset integration
1. Inspect the current game's VFS/archive layer and define a read-only overlay/priority model for replacement files.
2. Add DLC pack discovery and pack-list integration only after confirming the game's archive loader can mount external RPFs.
3. Add OpenIV OIV parsing as a declarative install plan; reject executable installers and unsupported operations.
4. Support loose replacement assets/data only where the runtime has a verified path mapping. Show ambiguous mappings as skipped, not guessed.
5. Add rollback/uninstall by removing only that mod's overlay directory and metadata.

### Phase 3 — scripts/trainers and content discovery
1. Keep Luau as the supported safe script runtime.
2. Connect imported compatible script content to the existing Glue 5 native API only through a documented, sandboxed adapter.
3. Do not claim Windows .asi/ScriptHookV compatibility until a real compatible runtime and GTA native bridge are implemented and tested. A PC PE file must never be executed as if it were an Android library.
4. Add curated in-app car/mod catalog and download/install flow only after the local installer and validation are complete.

## Required safety and compatibility rules
- No global driver changes and no renderer changes as part of this work.
- No modification of the original game assets or saves during import.
- ZIP bombs, traversal paths, symlinks, duplicate targets and oversized files must be rejected or explicitly reported.
- Do not silently claim that a DLC/RPF or ASI mod works merely because it imported.
- Every format must show a compatibility status: supported, partial, unsupported, or needs runtime work.

## Current implementation status
This document records the confirmed scope and engineering plan only. The APK installer has not yet been ported, no PC mod installer UI has been added in this commit, and no APK has been built or tested. The next code target is Phase 1's safe ZIP import + preview + isolated per-mod enable/disable manager.