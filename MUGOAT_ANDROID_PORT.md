# Mugoat Android port — engineering notes

This work is isolated on the `mugoat-android` branch. The existing Glue5 `main` branch and its release workflow are intentionally unchanged.

## Source baseline

- Upstream: https://github.com/iOSVNNews/mugoat
- Pinned upstream commit: `6f061d12c58c23dbf99da7d4225996a6d586ab89`
- Upstream license: GPL-3.0; retain notices and provide corresponding source for redistributed derivatives.
- Upstream native host is an iOS/UIKit application and its renderer is implemented with Apple Metal.

## Android blockers found during inspection

The iOS source directly imports Apple-only frameworks and APIs, including Metal, MetalKit, UIKit, IOSurface, AVFAudio, Mach memory APIs, and iOS-specific build settings. Its graphics adapter calls `PGStartMetalRenderer`; its shader assets are compiled to a Metal library. These components cannot be built into an Android APK as-is.

Glue5 already has an Android/Vulkan stack, but it is a different runtime and renderer. Reusing the Glue5 renderer requires a deliberate adapter for Mugoat's `NativeGraphics.h` contract and verification that the engine's graphics calls, memory model, audio, input, file APIs, and generated C engine all work under Android. Merely adding an Android Gradle project or renaming an APK would not constitute a port.

## Planned implementation sequence

1. Keep the upstream source pinned as a submodule so it can be updated and reviewed independently.
2. Add an Android app shell and NDK build only after defining a renderer bridge; do not compile iOS-only sources in the Android target.
3. Implement an Android graphics host backed by Vulkan (preferably using the existing Glue5/wgpu infrastructure only where its API and license permit).
4. Port platform services (lifecycle, audio, touch/controller input, memory and game-data selection) and build an ARM64 debug APK.
5. Add CI checks for Android compilation, APK structure, and native library ABI. Do not publish a release APK until those checks pass; gameplay and performance still require device testing.

## Current status

**Porting investigation started; no Android Mugoat APK has been built yet.** A successful Glue5 APK build does not validate Mugoat. Do not claim Vulkan support, working gameplay, or FPS performance until verified.
