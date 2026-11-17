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
