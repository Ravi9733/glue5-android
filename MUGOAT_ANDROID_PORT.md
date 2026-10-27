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

The Metal renderer header confirms that the renderer consumes the engine's graphics command ring directly. Android needs a separate implementation of that contract, with Vulkan rendering and a compatible shader pipeline; replacing only the window/layer call is insufficient. The exact command encoding, memory layout, shader inputs, synchronization and resource lifetime must be traced before an adapter can safely submit Vulkan commands.

The upstream `native-port/app/Scripts/prepare.py` is also a hard platform gate: it exits unless running on an Apple Silicon Mac with Xcode, validates the exact `game.wasm` SHA-256 (`11ca8d2c04c5e843d18ff4aea4899d72c86973c6b031df334e67c446b2ae83e0`), generates C from the engine with a pinned WABT fork, and builds the Metal shader library. This preparation pipeline cannot simply be invoked in Linux GitHub Actions; Android needs a separate preparation path that preserves the expected engine imports and generates Android-compatible shader assets.

The engine host also contains iOS-specific headers and calls (for example `TargetConditionals.h`, `sys/sysctl.h`, Mach/process memory APIs). Audio is exposed via `PGFillAudio`, and controller/touch state is passed through the `PGUpdateController`/virtual-pad APIs. These need Android implementations and lifecycle testing, not just build-file changes.

Glue5 already has an Android/Vulkan stack, but it is a different runtime and renderer. Reusing the Glue5 renderer requires a deliberate adapter for Mugoat's native engine graphics contract and verification that the engine's graphics calls, memory model, audio, input, file APIs, and generated C engine all work under Android. Merely adding an Android Gradle project or renaming an APK would not constitute a port.

## Planned implementation sequence

1. Pin and preserve the upstream source as a submodule so it can be reviewed independently.
2. Trace the native renderer's command interface, memory layout, shader inputs, synchronization and resource lifetime; document the mapping to Vulkan before wiring renderer code.
3. Build an Android-specific engine preparation path (WASM validation/code generation and Vulkan shader compilation), rather than calling the Apple-only `prepare.py`.
4. Add an Android app shell and NDK build; exclude iOS-only sources and replace platform-specific host calls.
5. Implement Android graphics, lifecycle, audio, input, memory and game-data services; build an ARM64 debug APK.
6. Add CI checks for Android compilation, APK structure, and native library ABI. Do not publish a release APK until those checks pass; gameplay and performance still require device testing.

## Current status

**Source pin and platform investigation are committed; no Android Mugoat APK has been built yet.** A successful Glue5 APK build does not validate Mugoat. Do not claim Vulkan support, working gameplay, or FPS performance until verified.
