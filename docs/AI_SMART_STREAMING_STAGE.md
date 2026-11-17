# Smart Streaming AI — implementation stage

## Scope

This stage targets delayed game-asset reads and texture pop-in without changing
the game files, the engine manifest, graphics quality, render scale, or the
Bicubic upscaler. It is an adaptive streaming scheduler, not a neural-network
model; the UI must call it AI Assist only while the controller is enabled.

## Existing mechanisms to build on

The stream client already has several useful signals and scheduling paths:

- Engine read announcements are delivered through
  `glue5_runtime::stream::drain` to `Client::hint`.
- `Client::read` checks the local sparse cache first and records reads that
  had to wait for missing blocks.
- The client tracks `received`, `waits`, `wait_us`, `cold`, `hints`,
  and current `waiting` counts internally.
- Demand reads, boot-set reads, engine hints, read-ahead and full-download
  fill already have distinct priorities.
- Requests are split into bounded chunks, so a demand read can be promoted
  ahead of lower-priority prefetch work.

## Work sequence

1. Expose a read-only, low-cost streaming telemetry snapshot: cache-hit/miss
   counts, waiting reads, cumulative wait time, late/missing hints, bytes
   received, and in-flight request count. Snapshot reads must use relaxed
   atomics or short existing locks; no filesystem scan on the render thread.
2. Add a rolling pressure estimator with hysteresis. It should distinguish
   local-cache hits from server waits and avoid treating a slow network as a
   GPU bottleneck.
3. Adapt only speculative read-ahead budgets and hint promotion based on
   measured demand pressure. Never delay or demote a demand read; never let
   speculative work starve the engine's own read requests.
4. Add tests for priority ordering, bounded queues, recovery after pressure
   falls, missing telemetry, and stable decisions around thresholds.
5. Surface the controller state and streaming wait counters in the existing
   Smart Performance HUD. Keep it opt-in and default OFF.

## Safety constraints

- Do not infer camera distance or visible texture importance unless the game
  exposes reliable asset/camera metadata.
- Do not claim that this fixes local-folder texture pop-in: the stream client
  only governs assets fetched through the streaming client. Local-file reads
  need separate engine-side profiling.
- Do not reduce texture quality, graphics preset, render scale, or switch away
  from Bicubic.
- Do not modify `game.wasm`, the user's game data, or the persistent game
  directory.
- Do not call a heuristic controller a trained neural network.

## Acceptance criteria

The feature is not complete until telemetry is wired to the real client,
scheduler decisions are exercised by tests, the Android build succeeds, and
the HUD shows actual (not fabricated) streaming state. Device-side testing is
still required to establish whether it reduces visible pop-in or stutter.
