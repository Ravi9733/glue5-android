# Android Adreno performance work

Target device: Snapdragon 8+ Gen 1 / Adreno 730 (iQOO Neo 7 Pro).

This work is deliberately staged. Do not enable speculative frame generation or
change image quality until the renderer has repeatable frame-time measurements.

## Findings at the foundation revision

- The renderer uses wgpu's Vulkan backend on Android.
- Presentation already supports bicubic scaling and frame pacing.
- The performance overlay can collect present intervals, render-thread time,
  optional GPU timestamps, draw/pass/submit counts and approximate GPU bytes.
- Frame generation in `Presentation` is currently implemented only for the
  Apple MetalFX temporal path; the Android Vulkan path does not advertise it.
- Texture decode work is bounded, but the current bound has a potential
  render-thread stall: when 16 decodes are in flight,
  `Gpu::make_decode_room` may call blocking `recv()` on the GPU command
  thread. The decode pool's `urgent_bounded` can also wait for queue space.
  This is a credible source of texture-streaming hitches and must be addressed
  without dropping texture regions or breaking per-texture upload ordering.

## Implementation order

1. **Remove avoidable render-thread waits in texture streaming.** Design a
   bounded deferred-work/backpressure path that keeps the GPU command thread
   draining commands and preserves ordering for regions belonging to the same
   texture. Keep memory bounded by bytes, not only job count.
2. **Measure before tuning.** Capture present/frame intervals, render-thread
   busy time, GPU time when timestamp queries are supported, decode queue age,
   decode duration, and upload bytes. Keep diagnostics low overhead and
   disabled or sampled during normal play.
3. **Adreno-specific tuning.** Tune decode worker count and queue byte budget
   from measurements on Adreno 730; do not assume more workers means faster
   gameplay because BC decoding can itself parallelize.
4. **Frame pacing and adaptive quality.** Prefer stable frame times and
   hysteresis. Do not silently lower render scale or alter the user's clear
   graphics preference. Any adaptive scale controller must be opt-in and
   expose its current scale.
5. **Frame generation research.** Treat Android Vulkan frame generation as a
   separate experimental project. It needs motion vectors, frame history,
   correct UI composition, latency measurements and device-specific
   validation. Do not advertise generated FPS as native game FPS.

## Acceptance checks

- No blocking wait for decode completion or decode-queue capacity on the GPU
  command thread during normal streaming.
- Per-texture mip/region upload order remains correct.
- In-flight decode memory has a byte budget and cannot grow without bound.
- Existing Bicubic presentation remains the default; no FSR or blur is enabled.
- Compare repeatable 10-minute runs in the same game area, reporting average
  frame time, 1% low, decode wait time, audio symptoms, temperature and memory.
- Build and run Android Vulkan tests before calling any change complete.


## Queue/backpressure design notes (follow-up audit)

The GPU command ring is consumed by a dedicated thread in `Gpu::run`; BCn
uploads are scheduled from `exec_texture_upload` and completed at flushes.
The current pool has two decode workers and a queue-capacity wait in
`Pool::urgent_bounded`. The decoder closures own both compressed input and
the decoded output until it is sent through the channel. A correct non-blocking
change therefore cannot simply replace `recv` with `try_recv` and keep
submitting jobs: that would move the unbounded backlog into `Gpu::decodes`.

A safe follow-up needs one admission-control owner for the complete lifecycle:

- Account compressed input bytes before accepting a decode job.
- Reserve a conservative decoded-output byte estimate before scheduling it
  (BC formats expand to uncompressed texels; ETC2 conversion has a separate
  output size).
- Keep deferred jobs in a bounded byte-budgeted queue, not in an unbounded
  vector of closures or receivers.
- Let the GPU thread poll completed jobs and schedule deferred work without
  waiting on a worker condition variable or channel receive.
- Preserve FIFO ordering for regions of the same texture while allowing
  independent textures to upload as soon as ready.
- Release reservations on success, cancellation, stale texture destruction,
  and worker disconnect; add unit tests for every release path.

The current `Decode` struct does not retain an explicit output-byte estimate,
and `Pool` exposes no non-blocking admission API or queue accounting. The
next implementation should introduce these together rather than make an
isolated change that trades a visible hitch for unbounded memory growth or
lost texture regions.

No renderer behavior was changed in this audit. This is an implementation
constraint, not a claim that the streaming hitch is fixed.


## Progress: checked decode-size primitives

- Added `bc::decoded_bytes` and `bc::etc2_bytes` helpers. They use checked
  arithmetic and match the allocation formulas used by the BC decode and
  ETC2 encode paths, so a future streaming admission controller can reserve
  bytes before scheduling work and reject overflow safely.
- Added unit tests for BC1, BC4, BC6H, ETC2 sizing and arithmetic overflow.
- Commit: `dc54102` (`perf(gpu): add checked texture decode memory estimates`).
- These helpers and tests are committed but have not yet been run in CI or on
  Android. The blocking admission path is still present; no APK was built.

## Follow-up audit — layered ETC2 accounting

- Confirmed the ETC2 encoder receives array layers as vertically stacked block rows (width = cols * 4, height = block_rows * 4 * layers). Its allocation is therefore cols * block_rows * layers * 16 bytes, matching `etc2_bytes`.
- Added a regression test that encodes two stacked 8x8 layers and compares the actual output length with the estimate.
- Reconfirmed the remaining render-thread hitch: `make_decode_room()` still calls blocking `recv()` when all 16 decode slots are occupied. The later `finish_decodes()` path is nonblocking, but that does not remove this admission-time wait.
- Next implementation must replace the blocking slot reclamation with deferred, byte-budgeted admission. It must retain compressed input ownership, preserve per-texture region order, and release reservations on every completion/drop path. Do not remove the cap or silently discard uploads.
- Test status: the new regression test is committed but has not been executed; no Android APK or device validation has been performed.

## Follow-up test coverage — BC allocation sizes

- Added a multi-layer allocation test that invokes the real BC decoder for BC1, BC4, BC5, BC6H and BC7, then compares `Vec::len()` against `decoded_bytes`. This checks the helper against the decoder's actual allocation expression, not only a duplicated expected formula.
- The ETC2 stacked-layer test and this BC test are committed, but neither has been run in CI or on-device yet.
- Implementation gate before changing the streaming scheduler: deferred jobs must be bounded by both count and bytes; reservations must include compressed source plus worst-case decoded intermediate and final ETC2 output; per-texture FIFO must survive deferred admission; destroyed textures and disconnected workers must release all accounting. If the incoming upload cannot be admitted within the budget, backpressure must happen at a safe command-ring boundary rather than allocating an unbounded deferred list or dropping the upload.

## Peak decode working-set reservation primitive

- Added `decode_working_bytes`, a checked upper-bound estimate for one job's peak owned CPU bytes: copied compressed input + BC-decoded intermediate + ETC2 output (when ETC2 conversion is enabled). The intermediate and final ETC2 buffers coexist during encoding, so both are counted.
- Added tests for ETC2 and non-ETC2 reservation arithmetic and overflow rejection.
- This is the accounting primitive needed by the deferred scheduler; it is not yet wired into `Gpu` admission. The current `make_decode_room()` blocking receive and `urgent_bounded()` queue wait remain unchanged. No runtime performance improvement is claimed from this commit.
- Next: locate and update every `Gpu` construction site, add a bounded deferred-job queue with explicit byte reservations, then pump it nonblocking from flush completion. The integration must preserve FIFO ordering per texture and account for cancellation/disconnection.

## Decode budget tracker foundation

- Added `decode_budget.rs` with nonblocking byte reservations, checked addition/subtraction, capacity reporting, and unit tests for saturation, reuse, overflow, and invalid release.
- Registered the module in `glue5-gpu`.
- Important: the tracker is not yet connected to `Gpu::upload_texture` or `finish_decodes`. The current blocking `make_decode_room()` receive and `urgent_bounded()` wait remain in the live path. This commit establishes the accounting primitive only; it does not yet change runtime behavior.
- Integration is being staged deliberately: the upload command owns its source bytes in the GPU command ring, so a safe deferred queue must apply backpressure without dropping commands or retaining an unbounded copied backlog. Next inspect the ring reader/release boundary and all `Gpu` construction sites before wiring the queue.

## Command-ring preflight support

- Added `Reader::peek(head)` in `glue5-runtime`: it exposes the next command without advancing the ring tail or counting it as executed. Wrap markers are followed safely, while the command payload remains unconsumed for the subsequent `next()` call.
- This is the prerequisite for GPU-side backpressure: the renderer can inspect a texture upload and decide whether decode memory is available before consuming/releasing the ring slot. If admission is blocked, it can leave the upload command in the ring instead of copying its source into an unbounded deferred queue.
- `exec.rs` still uses the old blocking decode-room path; the peek API is groundwork only. Next integration must preflight BC upload size, reserve bytes, stop at the unconsumed command on saturation, poll completions, and only then consume it.

## GPU ring peek regression coverage

- Added runtime unit tests that construct an in-memory GPU command ring and verify `Reader::peek` returns the pending upload command without advancing the published tail or incrementing the executed-command count.
- Repeated peeks return the same pending command; `next` then consumes it exactly once and `release` publishes the new tail.
- These tests have been committed but not run in this environment. The renderer still needs the actual preflight/backpressure integration before the blocking decode wait can be removed.

## First live texture-upload backpressure integration

- `Gpu::execute_until` now peeks the next ring command before consuming it. For BC texture uploads it estimates peak CPU working-set bytes (compressed input + BC intermediate + ETC2 output when applicable), reserves that amount, and enforces a 16-entry in-flight cap.
- If the reservation cannot be admitted, the renderer polls completed decodes and leaves the upload command unconsumed in the ring. The source payload therefore remains protected by the ring's tail; the renderer no longer needs to copy a backlog into an unbounded deferred queue. Saturation polling sleeps for 1 ms rather than blocking on a decode receiver.
- Reservations are attached to decode entries and released on successful upload, stale texture destruction, or worker disconnection. The previous `make_decode_room()` blocking `recv()` path and its upload helper have been removed.
- The nominal working-set budget is 256 MiB. A single oversized job is allowed only when no other decode reservation is active, avoiding a permanent ring deadlock on a texture larger than the nominal cap while still preventing concurrent oversubscription.
- This is the first runtime integration, not yet a verified performance fix. Code has not been compiled or tested here; validate Rust borrow/type correctness, ring behavior under saturation, per-texture region ordering, and Android memory/latency before treating it as ready. Add stress tests for reservation release and blocked-command replay next.

### Follow-up safety checks for the first integration

- Added in-memory ring tests for repeated `peek`, exactly-once `next`, and tail/executed counter publication. The fixtures use explicitly aligned storage for atomic ring words.
- Decode-budget admission now permits one oversized upload only when no other reservation is active; without this exception a single texture larger than 256 MiB could otherwise leave the ring permanently blocked. Concurrent work remains constrained by the nominal budget.
- The tests are committed but still unexecuted. Before merging or distributing an APK, run the workspace formatter and the `glue5-runtime` and `glue5-gpu` test suites, then stress-test ring saturation and texture destruction while decode is pending.


## Nonblocking decode worker admission

The BC texture upload path now uses `Pool::try_urgent_bounded` instead of the blocking `urgent_bounded` call. When the urgent worker queue is full, ownership of the decode closure is retained in a FIFO deferred-job queue on the GPU backend. Completion polling retries admission without waiting for a worker condition variable. Deferred jobs remain covered by the command-ring preflight reservation and the existing 16-upload in-flight limit. This removes worker-queue-capacity waits from the GPU command thread; it does not guarantee that all texture decode work completes before a frame, and it still requires Rust tests, stress testing, and Android Vulkan validation.


## Smart performance controller — first implementation stage

- Added `glue5_gpu::adaptive::Controller`, a deterministic, measurement-driven
  decision core using presented frame time and optional renderer/GPU timings.
- It waits for a 60-frame sample window, requires sustained overload across
  three windows before recommending a 60→45→30 FPS pacing step, and requires
  ten stable windows before recommending recovery. Invalid measurements are
  ignored; missing GPU timestamp support does not disable frame-time feedback.
- It only recommends a frame cap. It does not silently lower render scale,
  change texture quality, enable FSR, add blur, or change the Bicubic upscaler.
- Added unit tests for sustained overload, missing GPU timestamps, slow
  recovery, invalid samples and bounded caps.
- Commits: `8ba8850` (controller core), `b97d841` (module export).
- Status: controller core is committed but is not yet wired to live frame
  telemetry, runtime settings or an in-game AI panel. Tests have not yet been
  executed in CI or on the phone. Do not describe this stage as a completed
  in-game AI feature.
- Next: connect the controller to each presented-frame sample, expose its
  state and recommendation in the existing performance HUD/settings, then
  add an explicit opt-in setting before applying pacing changes.


### Smart Performance runtime integration — current stage

- Added a persisted, opt-in `smart_performance` setting (off by default).
- The Video settings page now exposes **Smart Performance (AI Assist)**.
- While gameplay is active and the option is enabled, the renderer gathers frame intervals and available render/GPU timing, feeds the adaptive controller, and displays a compact AI status row in the performance HUD even if the normal HUD option was off.
- The analyser compares measurements with the player's selected FPS budget. It reports an advisory FPS target after sustained overload; it does not silently modify the saved FPS cap, render scale, graphics quality, Bicubic upscaler, or game files.
- The controller evaluates non-overlapping 60-frame windows, requires three overloaded windows before lowering its recommendation, and requires ten stable windows before stepping back toward the original target.
- This is a lightweight adaptive controller, not a neural-network model. It is intended to reduce oscillation and expose measured recommendations without adding a large ML runtime to the render thread.
- Current commits include `2c8ef8b` (windowing and recovery logic), `8f4a242` (telemetry integration), `3b23908` (persisted setting), `2f9dca7` (Video settings toggle), `369d76c` (runtime activation), `d10b6aa` (HUD status), and `b06b486` (consistent telemetry snapshot).
- CI is running for Rust tests and Android Adreno type-check. The resulting APK and device behavior are not yet verified.
