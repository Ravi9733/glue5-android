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

Added `.github/scripts/check_mugoat_gpu_protocol.py` and `.github/workflows/mugoat-protocol-check.yml` on this branch. The workflow checks out the pinned submodule and compares ring framing and opcode IDs, excludes Glue5's host-only `SCENE_DONE`, and checks that each non-`WRAP` engine opcode has a reference in the Glue5 GPU executor. GitHub Actions runs for commits `cec0821` and `e89751f` completed successfully. This remains a static guard: opcode references do not validate payload fields, Vulkan behavior or gameplay.

## Current status

**Verified:** ring framing and engine opcode IDs match; the static GitHub Actions protocol check passes and the Glue5 executor references all non-`WRAP` engine opcodes. **Not yet implemented/verified:** complete payload semantics, a Mugoat-compatible Vulkan shader/pipeline path, Android host replacements, NDK app integration and an Android APK. No Mugoat Android APK has been built. A successful protocol check or Glue5 APK build does not validate Mugoat gameplay. Do not claim Vulkan support, working gameplay or FPS performance until verified.


## Payload and execution audit — first pass

Compared the Glue5 Vulkan executor dispatch in `crates/glue5-gpu/src/exec.rs` against the upstream Metal renderer's opcode table and Glue5 runtime's `Command` helpers. The executor has explicit dispatch references for all non-`WRAP` engine opcode IDs; `WRAP` is consumed by the shared ring reader before the executor sees it. This confirms dispatch coverage only, not equivalent rendering behavior.

| Area | Glue5 executor path | Port status / risk |
|---|---|---|
| Ring header and command framing | `glue5-runtime/src/gpu.rs` reader | Basic offsets and framing match the pinned upstream renderer. |
| Resource IDs / object lifetime | `exec.rs`, `objects.rs`, `submit.rs` | Different implementation models; lifetime and deferred destruction need runtime tests. |
| Textures and formats | `create_texture`, `format.rs`, `uploads.rs` | Vulkan/WebGPU format capabilities and BC fallback differ from Metal. Validate format mappings and large-texture memory behavior on target devices. |
| Shader / input layout | `create_shader`, `create_layout`, `shader.rs`, `pipeline.rs` | **Major blocker:** Mugoat currently uses precompiled Metal shader libraries. Android needs Vulkan-compatible SPIR-V/pipeline inputs and matching shader ABI. |
| State and draw commands | `SET_*`, `DRAW`, `dispatch` handlers | Handler presence is not proof of identical D3D semantics. Audit payload counts, slot ranges, dynamic state and shader bindings. |
| Readback and fences | `readback`, `FENCE`, `done.rs` | Glue5 uses asynchronous wgpu submissions and staging-map completion; must verify engine wait/wakeup ordering and failure behavior. |
| Queries | `CREATE_QUERY`, `BEGIN_QUERY`, `END_QUERY`, `ops.rs`, `done.rs` | GPU query results are asynchronous and may be approximated when readback fails; compare engine-visible result and sequence semantics. |
| Presentation / scene boundary | `PRESENT`, host-only `SCENE_DONE` | Host lifecycle differs. Never send Glue5-only opcode 64 to the upstream engine renderer decoder. |

### Current engineering decision

Do **not** wire Mugoat's engine directly to Glue5's Vulkan executor and call it a port yet. Matching command IDs and seeing executor arms is insufficient: the shader ABI and native host are incompatible, and an incorrect shortcut risks a black screen, corrupt resources or hangs on readback/fence waits. The next implementation milestone is an Android host/renderer adapter plus a shader-input pipeline; only after those exist does an Android NDK/APK workflow have meaningful build targets.



## Shader-tool feasibility check

Inspected the pinned upstream `native-port/app/ShaderTool/build_pgmsl.sh`. It builds an offline translator against Dawn/Tint with the Metal Shading Language writer enabled (`TINT_BUILD_MSL_WRITER=1`) and SPIR-V, GLSL and HLSL writers disabled. It also hardcodes macOS/Apple Silicon flags and links Apple frameworks. Therefore this existing tool cannot produce Android Vulkan shader artifacts in Linux CI; merely adding an Android Gradle/NDK target would still fail to create a runnable renderer.

A viable next graphics milestone must either (a) rebuild/configure a compatible Tint toolchain with a Vulkan-target shader output and validate the shader language/ABI expected by Glue5's pipeline, or (b) implement a dedicated translation stage from the engine's actual shader representation. The result must be tested with representative vertex, pixel and compute shaders before integrating it into the APK build.


## Automated Android blocker audit

Added `.github/scripts/check_mugoat_android_blockers.py` and `.github/workflows/mugoat-android-blocker-audit.yml`. This CI check confirms the pinned source still has the Apple Silicon/Xcode preparation guard, Metal-only shader writer, disabled SPIR-V writer and Metal-layer graphics API. It prevents those known blockers from being overlooked, but it is **not** a Vulkan implementation or APK build.

Commits:
- `0264750` — add the source blocker audit script.
- `9af46fb` — add the GitHub Actions workflow.

The next actual port work still requires implementing/replacing the graphics backend and platform host, then building and testing an ARM64 APK. No APK artifact is claimed by this audit.
