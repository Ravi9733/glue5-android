# XHYN-V 0.3 Preview 2 launcher and SDL lifecycle audit

## Evidence basis

The exact published Preview 2 APK was downloaded in CI and its SHA-256
verified as:

`a681765d9aae1404c59e3570682f3078bf9062ff800abfc7c5e1972f2783309f`.

JADX 1.5.2 decoded the APK DEX with nine recoverable decode errors. The
relevant `GTAVActivity` class and SDL lifecycle declarations were present.
The CI job emits only selected host signatures and does not publish the full
decompilation or proprietary libraries.

## Verified launcher facts from the exact APK

1. Launcher implementation is `com.gtavsource.android.GTAVActivity`.
2. It extends `org.libsdl.app.SDLActivity` (not plain Android Activity).
3. Its `getLibraries()` method returns the ordered pair
   `["SDL3", "gtav"]`. Android/SDL loads `SDL3` before `gtav`.
4. `createSDLSurface(Context)` creates the nested
   `GTAVActivity.GameSurface`, which extends `org.libsdl.app.SDLSurface`.
5. The nested surface overrides `surfaceCreated`,
   `surfaceChanged`, and `surfaceDestroyed`. The wrappers preserve the
   superclass SDL callbacks; XHYN's DriverManager also tracks surface
   readiness and destruction.
6. The Activity overrides `onCreate`, `onResume`, `onPause`,
   `onDestroy`, `loadLibraries`, and `getMainFunction`. The exact method
   declarations and relevant SDL/surface call sites are captured in the
   short-lived `xhyn-v03-host-evidence` CI artifact.
7. The APK contains 27 ARM64 shared libraries in total. The published
   verification count of 26 refers to the original engine/companion set;
   the additional `libxhyn_v02.so` is the XHYN runtime helper. CI confirmed
   exactly 26 non-helper libraries plus that helper, and all 27 are ELF64,
   little-endian AArch64. The audit records every library SHA-256 and
   DT_NEEDED dependency list. This is static ELF validation, not yet a full
   Android runtime dependency-closure or symbol-compatibility proof.

## What is still not established

- The exact SDL3 Java source revision and native SDL ABI compatibility with
  the SDL3 Java source bundled by Glue 5.
- Whether the XHYN Activity's private lifecycle logic can be safely reused
  as a separate app host, and which parts depend on XHYN-specific services,
  preferences, overlay UI or managers.
- The full body/ordering of custom library setup around
  `DxvkManager.initialize`, exact SDL main function return, and all
  native entry arguments.
- Whether the 26 original native libraries and their DT_NEEDED dependencies
  can be packaged with Glue 5's selected NDK/runtime without symbol conflicts.
- Device-tested pause/resume, surface recreation, input, audio focus,
  shutdown, gameplay stability and performance.

The source patch `apply_runtime.py` targets the Activity's library-loading,
resume, pause, destroy and nested surface methods. Its helper
`XhynV02Native.install` expects `libgtav.so` already loaded and installs a
build-specific controlled-stop hook. It is not the game launcher.

## Glue 5 comparison

Glue 5's current `platform/android/java/local/glue5/Glue5Activity.java`
also extends `org.libsdl.app.SDLActivity` and loads `SDL3`, then
`glue5`. This is a meaningful compatibility lead: both hosts use the
SDL3 Java package and SDL3-first native load order. It is not yet proof that
their SDL3 Java revisions, JNI signatures, SDL native binaries, lifecycle
overrides or surface ownership are interchangeable.

## Integration decision

Do not load `gtav` into the existing `local.glue5` process or add it to
`Glue5Activity.getLibraries()`. The current app has its own SDL3 and Glue 5
native main; a second SDL engine main in the same process is not a safe
assumption.

Next implementation should be a distinct native Android app target with its
own package ID and data path, adapting the verified XHYN host rather than
inventing an SDL launcher. Reuse the existing Glue 5 UI shell only through a
well-defined boundary; do not start the game by manually invoking
`SDL_main` or by calling `XhynV02Native.install`.

Keep `local.glue5` and `Documents/Glue5` unchanged. Do not package or
overwrite any existing game data. A production APK remains blocked until
the native dependency audit and SDL host comparison pass, and the exact
XHYN host's dependencies on its app-specific managers/services are mapped.

## Current status

- Exact Preview 2 APK hash: **verified**.
- Actual Activity class and SDLActivity superclass: **verified**.
- Native SDL library order `SDL3 → gtav`: **verified**.
- SDL surface subclass and create/change/destroy overrides: **verified**.
- 27 ARM64 libraries (26 original + helper): **count discrepancy explained**.
- Per-library ELF/DT_NEEDED inventory: **passed** (27 AArch64 ELF64 libraries;
  26 original plus `libxhyn_v02.so`).
- Glue 5 SDL3 source/ABI equivalence: **not yet proven**.
- Native adapter and separate launcher implementation: **not yet implemented**.
- Installable APK and device gameplay/performance tests: **not yet available**.
