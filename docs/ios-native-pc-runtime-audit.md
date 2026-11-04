# iOS native GTA V IPA audit and Android port plan

Date: 2026-11-04

## Executive finding

The supplied `Payload.zip` is **not a WASM/Glue 5 app**. Its `GTA5` entry point is a native 64-bit ARM Mach-O iOS executable. The bundle contains a D3D11/DXGI graphics stack and an iOS compatibility library. It does **not** contain the approximately 20 GB game-data installation: the expanded app bundle is about 91 MB, and there are no large RPF/data archives in the ZIP.

This is a materially different architecture from Glue 5's current WASM runtime. It is a useful architecture reference, but its compiled iOS binaries cannot be dropped into an Android APK.

## Supplied archive inspection

- Archive: `Payload.zip`
- SHA-256: `566281692c548bbd34b0e5218403e79875d0e76bd3eb8ecdc7eb6a62163abc45`
- App bundle: `Payload/GTAV.app` (about 91 MB expanded)
- Bundle identifier: `com.macport.gtav`
- App executable: `GTA5` (58,308,960 bytes)
- Executable SHA-256: `f9e15d0dbe4cca34100a51d2ffb1744c0d494ea989e7bd3813fc5b7f3a65d5e4`
- Mach-O: 64-bit ARM64, iOS platform, minimum OS 16.3, not encrypted (`cryptid=0`)
- Required device capability: ARM64 and Metal
- Build metadata: `MacportBuildDate=20261030`

### Bundled frameworks

| File | Size | What static inspection establishes |
|---|---:|---|
| `GTA5` | 58.3 MB | Native ARM64 Mach-O executable; links `libgtacompat.dylib` and Apple iOS frameworks |
| `libgtacompat.dylib` | 2.19 MB | Native ARM64 compatibility library; links UIKit, Foundation, Metal, audio/media and Apple system frameworks |
| `d3d11.dylib` | 3.31 MB | Native ARM64 D3D11 library; depends on DXGI and `winemetal.dylib` |
| `dxgi.dylib` | 347 KB | Native ARM64 DXGI library; depends on `winemetal.dylib` |
| `d3d10core.dylib` | 55 KB | Native ARM64 D3D10 core library |
| `winemetal.dylib` | 26.6 MB | Native ARM64 library linked directly to Metal and MetalFX |

The archive contains launcher images and an asset catalog, but no 20 GB PC game-data folder, no `game.wasm`, and no Android ELF libraries.

## What the public engineering notes describe

The public GTA V native-iOS engineering notes describe a native ARM64 build of the v2699 RAGE source, retaining the PC game and D3D11 renderer while replacing the platform layer. They describe:

1. Clang/Itanium/LP64 replacing the MSVC/LLP64 build ABI, with source/build-system adaptations.
2. An ARM64 compatibility layer reimplementing roughly 500 Win32 imports (threads, waits, virtual memory, files, TLS, window/message handling, input, audio and diagnostics).
3. DXMT translating D3D11 to Metal.
4. Offline processing of PC resources: RPF decryption, texture conversion (BC to ETC2/EAC), and data/audio recompression.
5. A phone-specific data set reported at approximately 21.6 GB after processing, rather than the untouched desktop install.

Reference: https://playgta5-1k8.pages.dev/technicalities

These notes are consistent with the IPA's ARM64 executable and bundled D3D11/DXGI/Metal libraries. They do not, by themselves, prove that every file in this particular IPA was built from the exact source revision described on that page.

## Consequence for Glue 5

Glue 5 currently hosts the WebAssembly build: WASM game code, Glue 5's Emscripten/WASI imports, and its own graphics command path. It does not contain the native RAGE v2699 source tree or the Windows-to-Android compatibility layer used by this iOS port.

Therefore:

- Replacing `game.wasm` with the iOS `GTA5` executable will not work. Mach-O and iOS frameworks are not Android ELF/NDK libraries.
- Copying `d3d11.dylib`, `dxgi.dylib`, `winemetal.dylib`, or `libgtacompat.dylib` into Glue 5 will not work. They depend on Apple frameworks and Metal.
- The PC archives are not self-running game code. They need a compatible engine build, correct structure/packing, and graphics/audio/platform services.
- Glue 5's existing `glue5-pc-mod-runtime` is a static PC-plugin/PE inspection crate. It is not a PC game runtime and must not be presented as one.

## Android implementation path

Keep the existing WASM build intact. Any PC-native route must be a separately selectable Android backend and must not change desktop or existing WASM behavior.

### Required source inputs

Before a real game-runtime port can be compiled, obtain the native port's source/build scripts and the permitted source revisions for:
- RAGE/game translation units and platform-specific patches;
- the Win32 compatibility layer;
- D3D11/DXGI implementation and shader compiler;
- the offline RPF/resource conversion pipeline;
- audio, input, file-streaming and launcher integration.

The IPA alone provides compiled ARM64 Mach-O files, not the source needed to rebuild them for Android. Do not try to turn the Mach-O binaries into Android ELF libraries.

### Android work packages

1. **Backend boundary:** define a separate `pc-native` backend and build target; keep the current WASM backend as the default and regression baseline.
2. **ARM64 engine build:** build the native game source for Android NDK/ARM64, resolving ABI, atomics, alignment, struct-layout and 16 KB page-size issues.
3. **Platform compatibility:** implement Android equivalents for the required Win32 services (threads/events/waits, virtual memory, file I/O, TLS, window lifecycle, input and audio).
4. **Graphics:** replace the iOS-only D3D11-to-Metal path with a D3D11-to-Vulkan path suitable for Android/Turnip. The iOS `winemetal` binary is not reusable.
5. **Asset preparation:** make a reproducible, verified offline pipeline for user-owned PC RPF archives, textures and audio; never silently modify the user's original data.
6. **Device validation:** first boot, streaming, shader correctness, audio, save/load, long-session memory and thermal tests on Android. A successful APK build alone is not runtime validation.

## Work completed in this branch

- Audited the supplied IPA's bundle metadata, executable format, linked libraries and archive contents.
- Corrected the PC-mod binary inspector's Mach-O architecture detection: Mach-O architecture is now read from the CPU type in the header, rather than guessed from the Mach-O magic. Added ARM64 and x86_64 fixtures. This is useful for correctly classifying native iOS/macOS binaries; it does not add executable loading support.

## Current blockers / next gate

The immediate blocker is the **native port source and its reproducible build pipeline**. Until those are available, the responsible next step is to harden the Android backend boundary and build/test the independent platform/asset tooling—not claim that the PC game is already runnable in Glue 5.

No game assets or third-party iOS binaries are copied into this repository.
