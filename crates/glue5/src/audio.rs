//! The engine's mixer output to the default playback device (SDL).

use std::time::{Duration, Instant};

use anyhow::{Result, anyhow};
use glue5_runtime::audio::{AudioOut, CHANNELS, SAMPLE_RATE};
use sdl3::audio::{AudioCallback, AudioFormat, AudioSpec, AudioStream, AudioStreamWithCallback};

/// Playing while alive.
pub struct Audio {
    stream: AudioStreamWithCallback<Pull>,
}

impl Drop for Audio {
    fn drop(&mut self) {
        // sdl3 frees the callback's state before it destroys the stream, and
        // SDL's audio thread can call it in between: at exit, a memset into
        // the freed buffer. Pausing takes the device's lock, so it waits for
        // a callback in progress, and no other comes.
        let _ = self.stream.pause();
    }
}

pub fn start(subsystem: &sdl3::AudioSubsystem, out: AudioOut) -> Result<Audio> {
    let spec = AudioSpec {
        freq: Some(SAMPLE_RATE as i32),
        channels: Some(CHANNELS as i32),
        format: Some(AudioFormat::f32_sys()),
    };
    // Use a moderate callback period on Android: 256 frames can be starved
    // when the game is compiling pipelines or streaming world assets. 512
    // frames gives the audio thread more scheduling headroom without the
    // large latency of SDL's usual ~1024-frame period.
    sdl3::hint::set("SDL_AUDIO_DEVICE_SAMPLE_FRAMES", "512");
    let pull = Pull { out, buf: Vec::new(), frames: 0, mixed: 0, calls: 0, peak: 0.0, last_log: Instant::now() };
    let stream = subsystem.open_playback_stream(&spec, pull).map_err(|e| anyhow!("SDL audio: {e}"))?;
    stream.resume().map_err(|e| anyhow!("SDL audio: {e}"))?;
    log::info!("audio: {SAMPLE_RATE} Hz stereo ({})", subsystem.current_audio_driver());
    Ok(Audio { stream })
}

/// SDL's audio thread asks for samples; they come straight from the ring.
struct Pull {
    out: AudioOut,
    buf: Vec<f32>,
    frames: u64,
    mixed: u64,
    calls: u64,
    peak: f32,
    last_log: Instant,
}

impl AudioCallback<f32> for Pull {
    fn callback(&mut self, stream: &mut AudioStream, requested: i32) {
        let samples = (requested.max(0) as usize).next_multiple_of(CHANNELS);
        self.buf.resize(samples, 0.0);
        self.mixed += self.out.read(&mut self.buf) as u64;
        self.frames += (samples / CHANNELS) as u64;
        self.calls += 1;
        let _ = stream.put_data_f32(&self.buf);

        if log::log_enabled!(log::Level::Debug) {
            self.peak = self.buf.iter().fold(self.peak, |p, s| p.max(s.abs()));
            if self.last_log.elapsed() >= Duration::from_secs(10) {
                self.last_log = Instant::now();
                log::debug!(
                    "audio: {} frames played ({} per request), {} silent (underrun or no mixer yet), peak {:.3}",
                    self.frames,
                    self.frames / self.calls.max(1),
                    self.frames - self.mixed,
                    std::mem::take(&mut self.peak)
                );
            }
        }
    }
}
