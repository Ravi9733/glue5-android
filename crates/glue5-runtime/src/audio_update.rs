//! The game's frames no longer outrun its audio thread into a 250 ms stall.
//!
//! Each frame the main thread starts the game's audio update
//! (`audNorthAudioEngine::StartUpdate`), which runs on a thread of its own
//! and is waited for at the end of the frame. It fills buffer `count & 3`
//! of four for the audio thread (`rage::audEngine`), then counts the frame;
//! the audio thread, about every 10 ms, reads the newest buffer, `(count -
//! 1) & 3`.
//!
//! With the audio thread two frames behind, the next update would fill the
//! buffer it still reads, and `audController::UpdateEntityRef` waits for it
//! to move on, polling, from inside the update. Meanwhile the update's own
//! commands never reach the mixer, and the audio thread cannot finish a
//! frame without them: both stood still until the audio thread gave up on
//! the mixer, 250 ms later, the main thread waiting all along. Frames faster
//! than the audio thread's did it: the pause menu (150-200 fps on a desktop
//! GPU) stuttered several times a second, every menu opened or page turned.
//! The browser build never draws faster than the display.
//!
//! The frame now waits for the audio thread before the update starts, while
//! the audio thread can still finish its frame: a few milliseconds, only
//! when frames are that fast.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use glue5_game::{Env, orig};

use crate::memory::Mem;

/// The game's audio frames, counted by the update. Addresses in this build.
const FRAME_COUNT: u64 = 12034076;
/// The buffer the audio thread reads: `(count - 1) & 3` as of its last frame.
const READING: u64 = 12114820;
/// How long a frame waits at most: the audio thread runs every 10 ms.
const MAX_WAIT: Duration = Duration::from_millis(30);

// Aggregate only the rare frame waits; never log from the per-frame hot path.
static WAIT_TOTAL_US: AtomicU64 = AtomicU64::new(0);
static WAIT_MAX_US: AtomicU64 = AtomicU64::new(0);
static WAIT_EVENTS: AtomicU64 = AtomicU64::new(0);
static LAST_REPORT_MS: AtomicU64 = AtomicU64::new(0);
static REPORT_EPOCH: OnceLock<Instant> = OnceLock::new();

/// Hook for `audNorthAudioEngine::StartUpdate()`.
pub(crate) unsafe extern "C" fn start_update(env: *mut Env) {
    let mem = unsafe { Mem::of(env) };
    let two_behind = || {
        let count = mem.atomic_u32(FRAME_COUNT).load(Ordering::Acquire);
        let reading = mem.atomic_u32(READING).load(Ordering::Acquire);
        count.wrapping_sub(reading) & 3 == 3
    };
    let started = Instant::now();
    let deadline = started + MAX_WAIT;
    while two_behind() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_micros(500));
    }
    let waited_us = started.elapsed().as_micros().min(u64::MAX as u128) as u64;
    if waited_us >= 1_000 {
        WAIT_TOTAL_US.fetch_add(waited_us, Ordering::Relaxed);
        WAIT_MAX_US.fetch_max(waited_us, Ordering::Relaxed);
        WAIT_EVENTS.fetch_add(1, Ordering::Relaxed);
    }
    let elapsed_ms = REPORT_EPOCH.get_or_init(Instant::now).elapsed().as_millis() as u64;
    let last = LAST_REPORT_MS.load(Ordering::Relaxed);
    if elapsed_ms.saturating_sub(last) >= 10_000
        && LAST_REPORT_MS.compare_exchange(last, elapsed_ms, Ordering::Relaxed, Ordering::Relaxed).is_ok()
    {
        let events = WAIT_EVENTS.swap(0, Ordering::Relaxed);
        if events != 0 {
            let total_us = WAIT_TOTAL_US.swap(0, Ordering::Relaxed);
            let max_us = WAIT_MAX_US.swap(0, Ordering::Relaxed);
            log::warn!(
                "audio pacing: {events} frame waits in 10s, total {:.1} ms, max {:.1} ms",
                total_us as f64 / 1_000.0,
                max_us as f64 / 1_000.0,
            );
        }
    }
    unsafe { orig::audio_start_update(env) }
}

/// This module's hooks.
pub(crate) fn hooks(hooks: &mut glue5_game::Hooks) {
    hooks.audio_start_update = Some(start_update);
}
