//! Smooth scroll animation.
//!
//! App keeps a `rendered_offset` (f64) representing the current visual scroll position.
//! ScrollAnimation interpolates between a start and target offset using cubic ease-out.
//! Each frame, App computes the integer delta from the last rendered position and
//! passes it to Viewport's scroll methods, which always work in whole rows.
//! This separation of float interpolation from integer row tracking is what makes
//! the animation visually smooth without complicating Viewport.

use std::time::{Duration, Instant};

/// Easing function: cubic ease-out (fast start, slow end).
fn ease_out_cubic(t: f64) -> f64 {
    let t1 = 1.0 - t;
    1.0 - t1 * t1 * t1
}

/// Tracks the state of a smooth scroll animation.
pub struct ScrollAnimation {
    start: f64,
    target: f64,
    start_time: Instant,
    duration: Duration,
}

impl ScrollAnimation {
    pub fn new(start: f64, target: f64, duration: Duration) -> Self {
        Self {
            start,
            target,
            start_time: Instant::now(),
            duration,
        }
    }

    /// Current interpolated offset.
    pub fn current_offset(&self, now: Instant) -> f64 {
        let t = self.progress(now);
        let eased = ease_out_cubic(t);
        self.start + (self.target - self.start) * eased
    }

    /// Whether the animation has completed.
    pub fn is_done(&self, now: Instant) -> bool {
        self.progress(now) >= 1.0
    }

    /// Target offset (final position).
    pub fn target(&self) -> f64 {
        self.target
    }

    fn progress(&self, now: Instant) -> f64 {
        let elapsed = now.duration_since(self.start_time).as_secs_f64();
        let total = self.duration.as_secs_f64();
        (elapsed / total).min(1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn easing_boundaries() {
        assert!((ease_out_cubic(0.0) - 0.0).abs() < f64::EPSILON);
        assert!((ease_out_cubic(1.0) - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn easing_monotonic() {
        let mut prev = 0.0;
        for i in 1..=100 {
            let t = i as f64 / 100.0;
            let val = ease_out_cubic(t);
            assert!(val >= prev, "easing must be monotonically increasing");
            prev = val;
        }
    }

    #[test]
    fn easing_fast_start() {
        // At t=0.5, ease-out-cubic should be past 0.5 (fast start)
        let mid = ease_out_cubic(0.5);
        assert!(
            mid > 0.5,
            "ease-out should progress quickly at first: {mid}"
        );
    }

    #[test]
    fn animation_at_start() {
        let anim = ScrollAnimation::new(0.0, 10.0, Duration::from_millis(200));
        let offset = anim.current_offset(anim.start_time);
        assert!((offset - 0.0).abs() < 0.01);
        assert!(!anim.is_done(anim.start_time));
    }

    #[test]
    fn animation_at_end() {
        let anim = ScrollAnimation::new(0.0, 10.0, Duration::from_millis(200));
        let end = anim.start_time + Duration::from_millis(200);
        let offset = anim.current_offset(end);
        assert!((offset - 10.0).abs() < 0.01);
        assert!(anim.is_done(end));
    }

    #[test]
    fn animation_past_end() {
        let anim = ScrollAnimation::new(0.0, 10.0, Duration::from_millis(200));
        let past = anim.start_time + Duration::from_millis(500);
        let offset = anim.current_offset(past);
        assert!((offset - 10.0).abs() < 0.01);
        assert!(anim.is_done(past));
    }

    #[test]
    fn animation_midpoint() {
        let anim = ScrollAnimation::new(0.0, 100.0, Duration::from_millis(200));
        let mid = anim.start_time + Duration::from_millis(100);
        let offset = anim.current_offset(mid);
        // At t=0.5, ease_out_cubic gives 0.875, so offset ~ 87.5
        assert!(offset > 50.0, "should be past halfway: {offset}");
        assert!(offset < 100.0, "should not be at target yet: {offset}");
    }
}
