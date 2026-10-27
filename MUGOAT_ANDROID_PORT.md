# Mugoat Android port — engineering notes

This work is isolated on the `mugoat-android` branch. The existing Glue5 `main` branch and its release workflow are intentionally unchanged.

## Source baseline

- Upstream: https://github.com/iOSVNNews/mugoat
- Pinned submodule: `third_party/mugoat-upstream`
- Pinned upstream commit: `6f061d12c58c23dbf99da7d4225996a6d586ab89`
- Upstream license: GPL-3.0; retain notices and provide corresponding source for redistributed derivatives.
- Upstream native host is an iOS/UIKit application and its renderer is implemented with Apple Metal.

## Verified platform blockers

The iOS source directly imports Apple-only frameworks and APIs, including Metal, MetalKit, UIKit, IOSurface, AVFAudio, Mach memory APIs, and iOS-specific build settings. Its graphics adapter calls `PGStartMetalRenderer`; its shader assets are compiled to a Metal library. These components cannot be built into an Android APK as-is.

The upstream graphics contract in `native-port/app/Source/NativeGraphics.h` includes:
- `PGStartGraphics(memory, memorySize, ring, ringSize, debug)`
- `PGAttachMetalLayer(layer)`
- `PGRenderedFramesPerSecond()` and `PGSetPresentPacing(seconds)`
- `PGCaptureNativeFrame(name)` and `PGGPUReadsBC()`

Android needs a separate implementation of this contract. It cannot call `PGAttachMetalLayer` or rely on a Metal shader library. Before implementation, the GPU command/ring semantics and shader representation must be traced from the upstream engine and compared with Glue5's Vulkan/wgpu runtime.

The engine host also contains iOS-specific headers and calls (for example `TargetConditionals.h`, `sys/sysctl.h`, Mach/process memory APIs). Audio is exposed via `PGFillAudio`, and controller/touch state is passed through the `PGUpdateController`/virtual-pad APIs. These need Android implementations and lifecycle testing, not just build-file changes.

Glue5 already has an Android/Vulkan stack, but it is a different runtime and renderer. Reusing the Glue5 renderer requires a deliberate adapter for Mugoat's native engine graphics contract and verification that the engine's graphics calls, memory model, audio, input, file APIs, and generated C engine all work under Android. Merely adding an Android Gradle project or renaming an APK would not constitute a port.

## Planned implementation sequence

1. Pin and preserve the upstream source as a submodule so it can be reviewed independently.
2. Trace the native renderer's command interface and shader inputs; write an explicit mapping to the Vulkan/wgpu path before wiring renderer code.
3. Add an Android app shell and NDK build; exclude iOS-only sources and replace platform-specific host calls.
4. Implement Android graphics, lifecycle, audio, input, memory and game-data services; build an ARM64 debug APK.
5. Add CI checks for Android compilation, APK structure, and native library ABI. Do not publish a release APK until those checks pass; gameplay and performance still require device testing.

## Current status

**Initial investigation and source pin are committed; no Android Mugoat APK has been built yet.** A successful Glue5 APK build does not validate Mugoat. Do not claim Vulkan support, working gameplay, or FPS performance until verified.
