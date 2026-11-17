//! A conservative, measurement-driven performance controller for mobile GPUs.
//!
//! The controller recommends frame caps only. It never changes render scale,
//! texture quality, shaders, or the selected upscaler.
use std::collections::VecDeque;

/// Measurements from one presented frame. GPU timing may be unavailable.
#[derive(Clone, Copy, Debug, Default)]
pub struct FrameMetrics {
    pub frame_ms: f32,
    pub render_ms: Option<f32>,
    pub gpu_ms: Option<f32>,
}

/// A recommendation is advisory; it does not change the game's settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    Hold { fps_cap: u32 },
    RecommendCap { fps_cap: u32 },
}

/// Uses non-overlapping 60-frame windows and hysteresis to avoid oscillation.
pub struct Controller {
    samples: VecDeque<FrameMetrics>,
    current_cap: u32,
    preferred_cap: u32,
    overload_windows: u32,
    stable_windows: u32,
}

impl Controller {
    pub fn new(initial_cap: u32) -> Self {
        let cap = normalize_cap(initial_cap);
        Self {
            samples: VecDeque::with_capacity(60),
            current_cap: cap,
            preferred_cap: cap,
            overload_windows: 0,
            stable_windows: 0,
        }
    }

    pub fn fps_cap(&self) -> u32 {
        self.current_cap
    }

    /// Feed exactly one presented frame. The budget is the selected FPS target's frame-time budget.
    pub fn observe(&mut self, metrics: FrameMetrics, budget_ms: f32) -> Decision {
        if !metrics.frame_ms.is_finite()
            || metrics.frame_ms <= 0.0
            || !budget_ms.is_finite()
            || budget_ms <= 0.0
        {
            return Decision::Hold { fps_cap: self.current_cap };
        }

        self.samples.push_back(metrics);
        if self.samples.len() < 60 {
            return Decision::Hold { fps_cap: self.current_cap };
        }

        let overloaded = self.samples.iter().filter(|m| {
            m.frame_ms > budget_ms * 1.12
                || m.render_ms.is_some_and(|v| v.is_finite() && v > budget_ms * 0.90)
                || m.gpu_ms.is_some_and(|v| v.is_finite() && v > budget_ms * 0.95)
        }).count();
        self.samples.clear();

        if overloaded >= 45 {
            self.overload_windows = self.overload_windows.saturating_add(1);
            self.stable_windows = 0;
        } else if overloaded <= 6 {
            self.stable_windows = self.stable_windows.saturating_add(1);
            self.overload_windows = 0;
        } else {
            self.overload_windows = 0;
            self.stable_windows = 0;
        }

        if self.overload_windows >= 3 && self.current_cap > 30 {
            self.current_cap = if self.current_cap <= 45 { 30 } else { 45 };
            self.overload_windows = 0;
            return Decision::RecommendCap { fps_cap: self.current_cap };
        }

        if self.stable_windows >= 10 && self.current_cap < self.preferred_cap {
            self.current_cap = match self.current_cap {
                30 => 45.min(self.preferred_cap),
                45 => 60.min(self.preferred_cap),
                _ => self.preferred_cap,
            };
            self.stable_windows = 0;
            return Decision::RecommendCap { fps_cap: self.current_cap };
        }

        Decision::Hold { fps_cap: self.current_cap }
    }
}

fn normalize_cap(cap: u32) -> u32 {
    cap.clamp(30, 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed(c: &mut Controller, n: usize, frame: f32, gpu: Option<f32>) -> Decision {
        let mut decision = Decision::Hold { fps_cap: c.fps_cap() };
        for _ in 0..n {
            decision = c.observe(
                FrameMetrics { frame_ms: frame, render_ms: None, gpu_ms: gpu },
                16.67,
            );
        }
        decision
    }

    #[test]
    fn sustained_overload_recommends_lower_cap_after_three_windows() {
        let mut c = Controller::new(60);
        assert_eq!(feed(&mut c, 179, 24.0, Some(18.0)), Decision::Hold { fps_cap: 60 });
        assert_eq!(feed(&mut c, 1, 24.0, Some(18.0)), Decision::RecommendCap { fps_cap: 45 });
    }

    #[test]
    fn missing_gpu_timestamps_still_uses_frame_time() {
        let mut c = Controller::new(60);
        assert_eq!(feed(&mut c, 180, 24.0, None), Decision::RecommendCap { fps_cap: 45 });
    }

    #[test]
    fn recovery_is_slower_and_never_exceeds_original_target() {
        let mut c = Controller::new(45);
        let _ = feed(&mut c, 180, 24.0, None);
        assert_eq!(c.fps_cap(), 30);
        assert_eq!(feed(&mut c, 600, 10.0, Some(7.0)), Decision::RecommendCap { fps_cap: 45 });
    }

    #[test]
    fn invalid_measurements_do_not_change_the_cap() {
        let mut c = Controller::new(60);
        for _ in 0..500 {
            let _ = c.observe(FrameMetrics { frame_ms: f32::NAN, ..Default::default() }, 16.67);
        }
        assert_eq!(c.fps_cap(), 60);
    }

    #[test]
    fn cap_is_bounded_to_supported_steps() {
        assert_eq!(Controller::new(0).fps_cap(), 30);
        assert_eq!(Controller::new(44).fps_cap(), 44);
        assert_eq!(Controller::new(120).fps_cap(), 60);
    }
}
