//! A conservative, measurement-driven performance controller for mobile GPUs.
//!
//! This is the decision core for the Android smart-performance feature. It
//! deliberately changes frame pacing recommendations only: it never changes
//! render scale, texture quality, shaders, or the selected upscaler. The UI /
//! runtime integration consumes the recommendation in a later stage.
use std::collections::VecDeque;

/// Measurements from one presented frame. Missing GPU timing is supported.
#[derive(Clone, Copy, Debug, Default)]
pub struct FrameMetrics {
    pub frame_ms: f32,
    pub render_ms: Option<f32>,
    pub gpu_ms: Option<f32>,
}

/// A conservative pacing recommendation; not a claim about measured FPS.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    Hold { fps_cap: u32 },
    RecommendCap { fps_cap: u32 },
}

/// A hysteretic controller: sustained overload lowers the suggested cap;
/// long, stable periods allow one step of recovery.
pub struct Controller {
    samples: VecDeque<FrameMetrics>,
    current_cap: u32,
    overload_windows: u32,
    stable_windows: u32,
}

impl Controller {
    pub fn new(initial_cap: u32) -> Self {
        Self { samples: VecDeque::with_capacity(60), current_cap: normalize_cap(initial_cap), overload_windows: 0, stable_windows: 0 }
    }

    pub fn fps_cap(&self) -> u32 { self.current_cap }

    /// Feed exactly one presented frame. budget_ms is the frame-time budget.
    pub fn observe(&mut self, metrics: FrameMetrics, budget_ms: f32) -> Decision {
        if !metrics.frame_ms.is_finite() || metrics.frame_ms <= 0.0 || !budget_ms.is_finite() || budget_ms <= 0.0 {
            return Decision::Hold { fps_cap: self.current_cap };
        }
        if self.samples.len() == 60 { self.samples.pop_front(); }
        self.samples.push_back(metrics);
        if self.samples.len() < 60 { return Decision::Hold { fps_cap: self.current_cap }; }

        let overloaded = self.samples.iter().filter(|m| {
            m.frame_ms > budget_ms * 1.12
                || m.render_ms.is_some_and(|v| v.is_finite() && v > budget_ms * 0.90)
                || m.gpu_ms.is_some_and(|v| v.is_finite() && v > budget_ms * 0.95)
        }).count();
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
            self.samples.clear();
            return Decision::RecommendCap { fps_cap: self.current_cap };
        }
        if self.stable_windows >= 10 && self.current_cap < 60 {
            self.current_cap = 60;
            self.stable_windows = 0;
            self.samples.clear();
            return Decision::RecommendCap { fps_cap: self.current_cap };
        }
        Decision::Hold { fps_cap: self.current_cap }
    }
}

fn normalize_cap(cap: u32) -> u32 { match cap { 0..=30 => 30, 31..=45 => 45, _ => 60 } }

#[cfg(test)]
mod tests {
    use super::*;
    fn feed(c: &mut Controller, n: usize, frame: f32, gpu: Option<f32>) -> Decision {
        let mut d = Decision::Hold { fps_cap: c.fps_cap() };
        for _ in 0..n { d = c.observe(FrameMetrics { frame_ms: frame, render_ms: None, gpu_ms: gpu }, 16.67); }
        d
    }
    #[test] fn sustained_overload_recommends_lower_cap() {
        let mut c = Controller::new(60);
        assert_eq!(feed(&mut c, 179, 24.0, Some(18.0)), Decision::Hold { fps_cap: 60 });
        assert_eq!(feed(&mut c, 1, 24.0, Some(18.0)), Decision::RecommendCap { fps_cap: 45 });
    }
    #[test] fn missing_gpu_timestamps_still_uses_frame_time() {
        let mut c = Controller::new(60);
        assert_eq!(feed(&mut c, 180, 24.0, None), Decision::RecommendCap { fps_cap: 45 });
    }
    #[test] fn recovery_is_slower_than_reduction() {
        let mut c = Controller::new(45);
        let _ = feed(&mut c, 180, 24.0, None);
        assert_eq!(c.fps_cap(), 30);
        assert_eq!(feed(&mut c, 600, 10.0, Some(7.0)), Decision::RecommendCap { fps_cap: 60 });
    }
    #[test] fn invalid_measurements_do_not_change_cap() {
        let mut c = Controller::new(60);
        for _ in 0..500 { let _ = c.observe(FrameMetrics { frame_ms: f32::NAN, ..Default::default() }, 16.67); }
        assert_eq!(c.fps_cap(), 60);
    }
    #[test] fn cap_is_bounded() {
        assert_eq!(Controller::new(0).fps_cap(), 30);
        assert_eq!(Controller::new(44).fps_cap(), 45);
        assert_eq!(Controller::new(120).fps_cap(), 60);
    }
}