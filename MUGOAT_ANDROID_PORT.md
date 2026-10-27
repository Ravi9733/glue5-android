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

### Glue5 renderer comparison

Glue5's `crates/glue5-runtime/src/gpu.rs` documents its ring as a browser-build `wgpu_worker.js` / `wgpu_backend` protocol. It has a 64-word header area and commands encoded as an opcode, payload length in 32-bit words, and payload; its known opcode table includes texture/buffer creation, shader/layout/state setup, draws, dispatches, fences and readbacks. Its backend runs the queue on a dedicated thread and uses Glue5 runtime memory helpers.

That is useful reference material, but it does **not yet prove wire compatibility** with Mugoat's Metal renderer. The next implementation gate is to inspect the upstream renderer's command decoder and compare every consumed opcode, field width, address interpretation, ring wrap rule, synchronization point and shader format against Glue5's definitions. No renderer adapter should be wired until that comparison is complete; guessing at this boundary could corrupt engine memory or produce a black screen.

The upstream `native-port/app/Scripts/prepare.py` is also a hard platform gate: it exits unless running on an Apple Silicon Mac with Xcode, validates the exact `game.wasm` SHA-256 (`11ca8d2c04c5e843d18ff4aea4899d72c86973c6b031df334e67c446b2ae83e0`), generates C from the engine with a pinned WABT fork, and builds the Metal shader library. This preparation pipeline cannot simply be invoked in Linux GitHub Actions; Android needs a separate preparation path that preserves the expected engine imports and generates Android-compatible shader assets.

The engine host also contains iOS-specific headers and calls (for example `TargetConditionals.h`, `sys/sysctl.h`, Mach/process memory APIs). Audio is exposed via `PGFillAudio`, and controller/touch state is passed through the `PGUpdateController`/virtual-pad APIs. These need Android implementations and lifecycle testing, not just build-file changes.

## Planned implementation sequence

1. Pin and preserve the upstream source as a submodule so it can be reviewed independently.
2. Finish a source-level renderer protocol comparison: opcodes, payload layouts, ring semantics, shaders and resource synchronization.
3. Build an Android-specific engine preparation path (WASM validation/code generation and Vulkan shader compilation), rather than calling the Apple-only `prepare.py`.
4. Add an Android app shell and NDK build; exclude iOS-only sources and replace platform-specific host calls.
5. Implement Android graphics, lifecycle, audio, input, memory and game-data services; build an ARM64 debug APK.
6. Add CI checks for Android compilation, APK structure, and native library ABI. Do not publish a release APK until those checks pass; gameplay and performance still require device testing.

## Current status

**Source pin and platform investigation are committed; no Android Mugoat APK has been built yet.** A successful Glue5 APK build does not validate Mugoat. Do not claim Vulkan support, working gameplay, or FPS performance until verified.
