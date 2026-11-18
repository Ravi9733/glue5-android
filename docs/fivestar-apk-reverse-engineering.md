# Five Star Port APK inspection (user-supplied Android APK)

Inspected on 2026-11-18 from the supplied file `GTA V Mobile Native Port Offline-Online Mode.apk`.

## Binary facts
- APK size: 206,000,062 bytes.
- SHA-256: `45f7e7c43f59906b91d75a757d7d1b4bec07b7c1ab88f98bc630656b135a367d`.
- Contains `classes.dex`, `classes2.dex`, and `lib/arm64-v8a/libfivestar.so`.
- `libfivestar.so` is a stripped AArch64 Android shared library (Android API 29, NDK r30), approximately 116 MB, dynamically linked to Vulkan, Android, AAudio, zlib and system libraries.
- APK contains 5,060 ZIP entries. `assets/OfficialPatches.json` currently contains an empty patches list.
- DEX strings identify the app-side `ModsAndPatchesPage`, `ModsPage`, `PatchesPage`, and `ModStore` components, plus a mod warning/recovery flow and mod-key UI. The DEX explicitly labels C# ScriptHookVDotNet as not supported in this inspected build.

## Behavior confirmed by APK strings and official user documentation
- The Android app has a Mods & Patches screen and a per-mod store/list.
- The app describes car, skin and script mod installation, with mods switched on/off independently.
- It has a “start without mods” recovery path after a failed start and keeps installed mods.
- The official guide documents ZIP, RAR, OIV, ASI and loose-file input; replacement assets; add-on DLC packs; and Script Hook V ASI scripts.
- The official guide documents important limitations: nested archives are not supported, brand-new files cannot be inserted into existing archives, and RAGE Plugin Hook/LSPDFR are not supported. C# support is version-dependent and is explicitly not supported by this inspected Android APK's DEX strings.

## What this inspection does not prove
The APK is a compiled native application, not a source checkout. Strings and ELF metadata do not reveal a reusable source implementation or prove that its internal installer can be transplanted into Glue 5. The native library is built for a different engine and ABI contract. Copying `libfivestar.so` into Glue 5 would not port its mod system and could not be treated as a compatible plugin.

## Glue 5 porting boundary
Glue 5 already has a Luau runtime, Studio, a Mods UI, a safe ZIP extraction prototype, and a separate PC-plugin diagnostic module. The ZIP prototype currently stores files under `mods/pc/<id>/`; it does not mount RPF/DLC archives or execute Windows ASI binaries. A real port therefore requires Glue 5-specific work:
1. Finish Android document-picker to safe import pipeline and per-package manager.
2. Build a preview/planner that classifies replacement assets, DLC packs, OIV operations, data files and unsupported binaries.
3. Integrate a read-only mod overlay with Glue 5's actual game VFS. Do not report RPF/DLC installs as active until archive discovery and loading are implemented and tested.
4. Add DLC pack registration only after confirming the engine's archive loader and pack-list implementation.
5. Keep ASI/ScriptHookV marked unsupported until a compatible x86-64 interpreter plus a Glue 5 native/API bridge exists. Never load a Windows PE DLL as an Android .so.
6. Preserve existing Luau mods, game data and saves; install/uninstall only touches the isolated mod package directory.

This file records the actual binary inspection and engineering boundary. It does not claim that a Glue 5 APK has been built or that the full feature port is complete.
