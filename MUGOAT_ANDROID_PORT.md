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

## Renderer protocol comparison — verified framing and opcode IDs

I inspected the pinned upstream `native-port/app/Source/MetalRenderer/MRRenderer.mm` command loop and Glue5's `crates/glue5-runtime/src/gpu.rs`.

**The basic ring framing and numeric opcode table match.** Both use a 64-word header/data offset, with header words for head, tail, state, executed-command count, errors and sleeping; each command begins with a 32-bit opcode and a 32-bit payload length (in 32-bit words), followed by that payload. Both use opcode `WRAP = 1` to return the consumer to `RING_DATA = 64`. The Metal loop advances by `8 + payload_words * 4` bytes, matching Glue5's reader. The listed command IDs from `NOP = 0` through `UPLOAD_BUFFER_NO_OVERWRITE = 47` also match, including query, readback and fence command IDs. This is a strong reuse opportunity: an Android host can potentially consume the same engine command ring without translating every command into a different wire format.

**Matching IDs do not prove full backend compatibility.** Each command's payload lengths and field meanings, engine-memory addresses, resource ownership/lifetime, and completion semantics still need a table-by-table audit. Glue5's host-specific `SCENE_DONE = 64` is outside the upstream renderer's engine opcode enum and must not be sent to the Mugoat decoder unless its host protocol explicitly handles it. We also need to confirm the actual Glue5 Vulkan backend implements all commands the engine issues, rather than relying on opcode constants alone.

The Mugoat renderer is a full backend, not just a ring reader: it implements Metal resource creation, shader/pipeline setup, texture format handling, draw/dispatch, copies, readbacks, fences, queries and presentation. Its renderer comments say game shaders are precompiled to a Metal library and D3D11 slots map to Metal argument indices. Android therefore still needs a Vulkan-compatible shader/pipeline path and a verified conversion or regeneration route for shader inputs; it cannot load the existing `.metallib` as Vulkan shaders. BC texture support/fallback behavior and render-target formats also need an Android GPU implementation.

## Engine preparation and host blockers

The upstream `native-port/app/Scripts/prepare.py` exits unless running on an Apple Silicon Mac with Xcode, validates the exact `game.wasm` SHA-256 (`11ca8d2c04c5e8439d18ff4aea4899d72c86973c6b031df334e67c446b2ae83e0`), generates C from the engine with a pinned WABT fork, and builds the Metal shader library. This pipeline cannot simply be invoked in Linux GitHub Actions; Android needs a separate preparation path that preserves expected engine imports and produces Android-compatible shader assets.

The engine host also contains iOS-specific headers and calls (for example `TargetConditionals.h`, `sys/sysctl.h`, Mach/process memory APIs). Audio is exposed via `PGFillAudio`, and controller/touch state is passed through the `PGUpdateController`/virtual-pad APIs. These need Android implementations and lifecycle testing, not just build-file changes.

## Planned implementation sequence

1. Keep the upstream source pinned and isolated from Glue5 `main`.
2. Audit payload lengths/fields and compare the actual Glue5 Vulkan backend's implementation of each command, including memory addressing and completion semantics.
3. Build an Android-specific engine preparation path (WASM validation/code generation and Vulkan shader compilation), rather than calling the Apple-only `prepare.py`.
4. Add an Android app shell and NDK build; exclude iOS-only sources and replace platform-specific host calls.
5. Implement Android graphics, lifecycle, audio, input, memory and game-data services; build an ARM64 debug APK.
6. Add CI checks for Android compilation, APK structure and native library ABI. Do not publish a release APK until those checks pass; gameplay and performance still require device testing.

## Automated guard added

Added `.github/scripts/check_mugoat_gpu_protocol.py` and `.github/workflows/mugoat-protocol-check.yml` on this branch. The workflow checks out the pinned submodule and compares the ring offset, `WRAP` ID and engine opcode IDs against Glue5. It deliberately excludes Glue5's host-only `SCENE_DONE` opcode. This is a static guard only; it does not validate payload fields, Vulkan behavior or gameplay. The workflow has been committed but its CI result has not yet been verified here.

## Current status

**The ring framing and engine opcode IDs match, but command semantics, Glue5 backend coverage, and the Vulkan renderer are not yet ported. No Android Mugoat APK has been built.** A successful Glue5 APK build does not validate Mugoat. Do not claim Vulkan support, working gameplay or FPS performance until verified.
