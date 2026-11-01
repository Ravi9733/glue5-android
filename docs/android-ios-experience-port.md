# Android iOS-Style Experience Port

Target: Android Turnip build only. iOS is a behavior/reference target; do not modify the iOS app.

## Acceptance goals

- Bring the iOS-style navigation, layout, and touch interaction to Android while retaining Android lifecycle, storage picker, and Vulkan/Turnip integration.
- Add a touch-accessible native trainer entry point. Do not require F4/F7 keyboard input.
- Trainer must expose only capabilities implemented by Glue5's shared engine and report unsupported actions clearly.
- PC package installer must validate ZIP contents, show import progress/result, support safe removal, and preserve user game data.
- Native add-on vehicle support is a separate engine milestone: packfile mount, DLC activation/load order, vehicle metadata/archetype registration, model/texture streaming, then trainer catalog integration.
- Script compatibility must distinguish Glue5 Luau/native hooks from PC ScriptHookV ASI/DLL compatibility; never label ASI support unless a compatible runtime exists.

## Required validation gates

1. Android build succeeds for the Turnip target.
2. Touch trainer opens/closes and each exposed action is exercised in-game.
3. ZIP import has clear success/failure reporting and does not leave partial installs.
4. A test add-on vehicle is registered by the game engine, appears in the trainer, spawns with visible geometry/textures, can be driven, and survives relaunch.
5. Uninstall removes only the selected mod and leaves base game data, existing mods, and Turnip configuration intact.

## Implementation order

1. Audit shared UI/navigation and Android lifecycle/storage bridges against iOS reference behavior.
2. Implement Android touch-first trainer entry and menu flow using existing shared trainer features.
3. Finish robust ZIP install/report/uninstall UX for supported package types.
4. Trace and implement RAGE packfile/DLC/asset registration using verified ABI and object lifetimes; do not guess offsets or call signatures.
5. Connect successfully registered vehicle models to trainer catalog.
6. Run Android CI and device runtime acceptance tests before publishing a final APK.

## Current known boundary

The current importer can copy/extract packages and inspect supported formats, but package installation alone does not mount DLC or register custom vehicle assets in the RAGE engine. Existing trainer spawning is catalog-driven and does not automatically discover installed PC add-on vehicles.
