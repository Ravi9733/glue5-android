# XHYN-V 0.3 launcher and SDL lifecycle audit

## Scope and evidence

This audit compares the public XHYN-V 0.3 Preview 2 modification source with
Glue 5's checked-in Android host. It is a source-level audit, not a decompilation
of the signed Preview 2 APK. The exact APK's DEX, binary manifest, SDL classes
and native dependency order still need local inspection.

Reviewed public XHYN-V files:
- `docs/SOURCE.md` and the 0.3 README;
- `modifications/v0.3/apply_runtime.py`;
- `modifications/v0.3/java/com/gtavsource/android/XhynV02Runtime.java`;
- `modifications/v0.3/java/com/gtavsource/android/XhynV02Native.java`;
- `modifications/v0.3/native/xhyn_v02.c`;
- `modifications/v0.3/stubs/com/gtavsource/android/GTAVActivity.java`;
- `modifications/v0.3/tests/check_launcher_structure.py` and `STATUS.md`.

## What is established

1. The XHYN documentation describes a native ARM64 game engine library
   `libgtav.so`, SDL's Android native entry path, and an exported `SDL_main`.
   This establishes the broad architecture, not the Java launch ABI.
2. The 0.3 runtime patch targets an existing `GTAVActivity` implementation:
   `loadLibraries()V`, `onResume()V`, `onPause()V`, `onDestroy()V`.
3. It also wraps `GTAVActivity$GameSurface.surfaceCreated(SurfaceHolder)V`
   and `surfaceChanged(SurfaceHolder;III)V`. The wrapper calls
   `XhynV02Runtime.surface(View)`; the same Activity lifecycle calls
   `resumeGame(Activity)`, `pauseGame(Activity)` and `gameStopped()`.
4. The runtime hook `librariesLoaded(Activity)` is injected after the original
   `DxvkManager.initialize(Context, RuntimeConfig)` call in
   `GTAVActivity.loadLibraries()`. Therefore the patch assumes the base
   Activity's library-loading sequence and state already exist.
5. The Java `XhynV02Native` methods belong to an auxiliary helper library
   `libxhyn_v02.so`. In the public C source, `install` expects `libgtav.so`
   to be already loaded, resolves `SDL_main`, then installs a build-specific
   controlled-stop patch. `identity` / `probe` are diagnostics. None is a
   general game-start, SDL Activity, surface ownership, or input API.
6. The published `GTAVActivity.java` under `stubs` only declares a method
   that throws `UnsupportedOperationException`. It is a compile stub, not
   the original host Activity.
7. XHYN's own status document says lifecycle/surface behavior and sustained
   gameplay remain unverified on physical devices. A host-side mock or patch
   script is not proof of device behavior.

## Glue 5 host

Glue 5's current `platform/android/java/local/glue5/Glue5Activity.java`
extends `org.libsdl.app.SDLActivity` and loads `SDL3`, then `glue5`.
Its manifest declares the launcher Activity as landscape/singleInstance and
requires Vulkan 1.1. Its Gradle application ID is `local.glue5`.

XHYN's exact SDL Java package/version, Activity superclass, native library
loading order, thread ownership and surface contract have not been proven
compatible with Glue 5's SDL3 host. Sharing a process or loading
`libgtav.so` into `Glue5Activity` would risk competing SDL runtimes and
conflicting lifecycle/surface owners.

## Lifecycle compatibility matrix

| Concern | XHYN 0.3 public evidence | Glue 5 | Adapter decision |
|---|---|---|---|
| Activity | Exact original class body not published; patch targets `GTAVActivity` | `Glue5Activity extends SDLActivity` | Do not subclass/copy a stub |
| SDL | SDL native entry / `SDL_main` described | SDL3 Java host | Exact version/ABI must be inspected |
| Libraries | `libgtav.so` and helper; complete exact ordered list not exposed by source | `SDL3`, `glue5` | Never append game library to current list |
| Startup | Existing Activity `loadLibraries()` patched | SDL calls Glue 5 native library main | No verified callable XHYN start method |
| Surface | Created/changed callbacks wrapped; destroy behavior not shown in this patch excerpt | SDL owns its surface callbacks | One owner required; no forwarding guess |
| Foreground | Existing Activity resume/pause wrapped | SDL Activity lifecycle | Exact ordering and thread requirements unknown |
| Shutdown | Activity destroy calls `gameStopped()` | SDL/native library lifecycle | No verified engine stop/join contract |
| Input/audio | No reusable public cross-host contract established | Glue 5 SDL3 input/audio | Not bridgeable yet |
| Graphics | DXVK/Vulkan native stack | Glue 5 wgpu/Vulkan | Engine must own its renderer; no wgpu command-ring bridge |

## Native adapter and launcher decision

A direct in-process adapter is **not safe to implement yet**. In particular,
do not:
- invoke `SDL_main` manually from a new thread;
- load `libgtav.so` into Glue 5's existing SDL3 process;
- call `XhynV02Native.install` as a launch method;
- reuse fixed offsets from the XHYN controlled-stop patch;
- forward a Glue 5 Surface to XHYN without a verified owner/lifecycle;
- claim that a successful Vulkan probe means the game can start.

The safe implementation direction is a separate native application/host
boundary, preserving `local.glue5` and its WASM target unchanged. Before
building that host, obtain the exact authorized Preview 2 APK evidence bundle
and establish: launcher Activity/superclass; exact SDL Java/native version;
all 26 ARM64 library hashes, dependencies and original load order; SDL entry
arguments; surface creation/destruction; pause/resume ordering; input/audio
ownership; and external data path. If XHYN cannot be hosted through a supported
public API, use its original app as a separately launched process only if it
exposes a documented, supported intent/deep-link contract; do not invent one.

## Next executable gate

Run `scripts/extract-native-host-evidence.py` on the exact APK whose SHA-256
matches the published Preview 2 verification record. Review the private JADX
output and ELF dependency report locally. Share only the hash, manifest
launcher declaration, relevant method signatures, SDL version and ordered
library names. Do not publish the proprietary libraries, full decompilation,
game assets or signing material.

## Status

- Public source lifecycle touchpoints: **identified**.
- Exact Preview 2 Activity/manifest/DEX and SDL ABI: **not yet verified**.
- Real native adapter: **blocked; not implemented**.
- Separate launcher: **architecture specified; no misleading launch stub added**.
- Android APK/gameplay/device validation: **not performed**.
