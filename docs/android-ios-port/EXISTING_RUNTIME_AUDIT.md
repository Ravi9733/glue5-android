# Existing Android runtime reuse audit

This is a follow-up to `HOST_REUSE_MATRIX.md`, checked on `turnip-driver-integration`.

## Newly verified in source

- `crates/glue5-game/build/main.rs` has an explicit allowlist for build
  `8b0b5899ed` and the exact SHA-256 recorded in `PORT_STATUS.md`. It refuses
  an unknown wasm unless `GLUE5_ALLOW_UNKNOWN_BUILD` is set. Keep that guard.
- The build script runs patched `wasm2c`, generates `Imports` and `Hooks`,
  applies game-specific hooks, and compiles generated C into the game crate.
- `glue5-runtime/src/threads.rs` already runs the wasm main thread and
  Emscripten pthreads as native threads with separate wasm instances over
  shared memory.
- `glue5-runtime/src/vfs.rs` already maps `/game` read-only and
  `/userdata` to a persistent user directory, with an optional read-only
  PC-data fallback and a scratch overlay.
- `glue5-runtime/src/audio.rs` already reads the engine's 48 kHz stereo
  shared-memory audio ring. Android still needs an output callback/device
  integration test; the iOS AVAudioSession implementation is not portable.
- `glue5-runtime/src/input.rs` already provides a gamepad abstraction,
  button/axis/trigger injection, rumble state, and touch-play signals. The
  iOS context-specific touch UI still needs Android presentation/adaptation.
- `crates/glue5/src/settings.rs` already persists settings including
  `sandbox`, `mods`, `touch`, `touch_opacity`, `touch_size`,
  `touch_camera`, `touch_aim`, `max_fps`, `render_scale`,
  `upscaler`, and `vulkan_driver`.
- The settings default selects Bicubic only when configured; it also has
  Android Turnip selection and existing mobile performance migrations.
- `glue5-runtime/src/game_settings.rs` already handles GTA V's settings.xml
  and low-memory preset without replacing the user's save data.

## Consequence

The port is not a clean-room Android engine port. The existing Android runtime
already covers much of the engine host, settings persistence, input model,
filesystem, threads, and audio ring. The highest-value next work is a gap audit
and UI/installer adaptation—not replacing these services with copies of the
iOS C host.

## Not yet verified

- The complete import list of the exact wasm file versus Glue 5's generated
  bindings (the build script's success is the authoritative test).
- Whether Android SDL audio consumes the ring continuously on the target
  device and with acceptable latency.
- Whether the iOS archive installer supports formats/features absent from
  `glue5-mods`; do not assume all .ASI files are Android-compatible.
- Whether all iOS touch contexts and context hints are surfaced in the Android
  overlay.
- Actual runtime startup, Story/Sandbox, world streaming, and Turnip stability
  on the target device.

No APK build or device test is claimed by this audit.
