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
