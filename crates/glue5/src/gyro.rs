//! Device-motion filtering shared by the Android sensor bridge.
//!
//! SDL supplies angular velocity in radians/second and gravity in device
//! coordinates. Samples are integrated before polling so short sensor updates
//! are not lost when the game renders at a lower frame rate.

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct MotionRate {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Default)]
pub(crate) struct GyroMotion {
    accumulated: MotionRate,
    last_timestamp: Option<f64>,
    gravity: Option<[f32; 3]>,
    smoothed: MotionRate,
}

impl GyroMotion {
    /// Integrate one sensor sample. landscape_right selects the screen's
    /// horizontal pitch axis; yaw is projected onto real-world vertical.
    pub(crate) fn sample(
        &mut self,
        timestamp: f64,
        angular_rate: [f32; 3],
        gravity: [f32; 3],
        landscape_right: bool,
    ) {
        self.gravity = Some(gravity);
        let Some(last) = self.last_timestamp.replace(timestamp) else {
            return;
        };
        let dt = (timestamp - last).clamp(0.0, 0.05) as f32;
        let g = (gravity[0] * gravity[0]
            + gravity[1] * gravity[1]
            + gravity[2] * gravity[2])
            .sqrt();
        let yaw = if g > 0.1 {
            (angular_rate[0] * gravity[0]
                + angular_rate[1] * gravity[1]
                + angular_rate[2] * gravity[2])
                / g
        } else {
            0.0
        };
        let pitch = if landscape_right {
            angular_rate[1]
        } else {
            -angular_rate[1]
        };
        self.accumulated.x += yaw * dt;
        self.accumulated.y += pitch * dt;
    }

    /// Average integrated motion since the previous poll. Inactive polls
    /// discard pending motion so enabling gyro cannot cause a camera jump.
    pub(crate) fn rate(&mut self, active: bool, elapsed: f32) -> MotionRate {
        let turned = std::mem::take(&mut self.accumulated);
        if !active || elapsed <= 0.0 {
            self.smoothed = MotionRate::default();
            return MotionRate::default();
        }

        let mut rate = MotionRate {
            x: turned.x / elapsed,
            y: turned.y / elapsed,
        };
        let speed = rate.x.hypot(rate.y);
        let blend = ((speed - 0.035) / (0.14 - 0.035)).clamp(0.0, 1.0);
        self.smoothed.x = self.smoothed.x * 0.5 + rate.x * 0.5;
        self.smoothed.y = self.smoothed.y * 0.5 + rate.y * 0.5;
        rate.x = rate.x * blend + self.smoothed.x * (1.0 - blend);
        rate.y = rate.y * blend + self.smoothed.y * (1.0 - blend);

        fn tighten(value: f32) -> f32 {
            const DEAD_ZONE: f32 = 0.026;
            if value.abs() < DEAD_ZONE {
                value * value.abs() / DEAD_ZONE
            } else {
                value
            }
        }

        MotionRate {
            x: tighten(rate.x),
            y: tighten(rate.y),
        }
    }

    /// Phone steering-wheel angle in radians, positive when turned right.
    pub(crate) fn steering_angle(&self, landscape_right: bool) -> Option<f32> {
        let gravity = self.gravity?;
        let (screen_x, screen_y) = if landscape_right {
            (-gravity[1], gravity[0])
        } else {
            (gravity[1], -gravity[0])
        };
        if screen_x.hypot(screen_y) <= 0.3 {
            return None;
        }
        Some(screen_x.atan2(-screen_y))
    }

    pub(crate) fn reset(&mut self) {
        self.accumulated = MotionRate::default();
        self.last_timestamp = None;
        self.gravity = None;
        self.smoothed = MotionRate::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_sample_establishes_timestamp_without_camera_motion() {
        let mut motion = GyroMotion::default();
        motion.sample(1.0, [0.0, 1.0, 0.0], [0.0, 1.0, 0.0], true);
        assert_eq!(motion.rate(true, 1.0), MotionRate::default());
    }

    #[test]
    fn samples_accumulate_until_the_next_frame_poll() {
        let mut motion = GyroMotion::default();
        motion.sample(1.0, [0.0, 1.0, 0.0], [0.0, 1.0, 0.0], true);
        motion.sample(1.01, [0.0, 1.0, 0.0], [0.0, 1.0, 0.0], true);
        motion.sample(1.02, [0.0, 1.0, 0.0], [0.0, 1.0, 0.0], true);
        let rate = motion.rate(true, 0.02);
        assert!(rate.y > 0.9);
        assert_eq!(motion.rate(true, 0.02), MotionRate::default());
    }

    #[test]
    fn inactive_poll_discards_motion() {
        let mut motion = GyroMotion::default();
        motion.sample(1.0, [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], true);
        motion.sample(1.02, [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], true);
        assert_eq!(motion.rate(false, 0.02), MotionRate::default());
        assert_eq!(motion.rate(true, 0.02), MotionRate::default());
    }

    #[test]
    fn steering_is_unavailable_when_phone_is_flat() {
        let mut motion = GyroMotion::default();
        motion.sample(1.0, [0.0; 3], [0.0, 0.0, 0.0], true);
        assert_eq!(motion.steering_angle(true), None);
    }
}
