//! Physics-based smooth scroll animation.
//!
//! This simulates inertial scrolling with friction and air drag.
//! User actions add impulse (velocity) to the simulation,
//! and repeated presses accumulate momentum for faster scrolling. Each frame,
//! the simulation produces an integer row delta that the viewport uses for incremental rendering.
//! Inspired by https://github.com/yuttie/comfortable-motion.vim

/// Constant friction force opposing velocity direction.
const FRICTION: f64 = 80.0;

/// Velocity-proportional drag coefficient.
const AIR_DRAG: f64 = 2.0;

/// Minimum velocity below which the simulation stops.
const MIN_VELOCITY: f64 = 1.0;

/// Physics-based scroll animation state.
pub struct ScrollPhysics {
    velocity: f64,
    /// Fractional row accumulator (sub-row precision).
    delta: f64,
}

impl ScrollPhysics {
    pub fn new() -> Self {
        Self {
            velocity: 0.0,
            delta: 0.0,
        }
    }

    /// Add impulse to scroll approximately `target_rows` rows.
    /// The impulse is computed to achieve the target distance from rest,
    /// then added to the current velocity (allowing momentum accumulation).
    pub fn impulse(&mut self, target_rows: f64) {
        let impulse = impulse_for_distance(target_rows.abs());
        self.velocity += impulse * target_rows.signum();
    }

    /// Advance the simulation by `dt` seconds and return integer rows to scroll.
    pub fn tick(&mut self, dt: f64) -> isize {
        if !self.is_active() {
            return 0;
        }

        // Apply friction (constant, opposing velocity) and air drag (proportional to velocity)
        let sign = self.velocity.signum();
        let friction_force = -sign * FRICTION;
        let drag_force = -self.velocity * AIR_DRAG;
        let dv = (friction_force + drag_force) * dt;

        // Clamp: don't let deceleration reverse the velocity direction
        if dv.abs() > self.velocity.abs() {
            self.velocity = 0.0;
        } else {
            self.velocity += dv;
        }

        self.delta += self.velocity * dt;
        self.extract_rows()
    }

    /// Whether the simulation has active motion.
    pub fn is_active(&self) -> bool {
        self.velocity.abs() >= MIN_VELOCITY
    }

    /// Stop the simulation immediately.
    pub fn stop(&mut self) {
        self.velocity = 0.0;
        self.delta = 0.0;
    }

    /// Extract whole rows from the fractional accumulator.
    fn extract_rows(&mut self) -> isize {
        let rows = if self.delta >= 0.0 {
            self.delta.floor() as isize
        } else {
            self.delta.ceil() as isize
        };
        self.delta -= rows as f64;
        rows
    }
}

/// Compute the impulse (initial velocity) needed to scroll approximately
/// `target_distance` rows with the current friction and air drag parameters.
///
/// Uses the analytical total-distance formula for the physics model:
///   distance(v0) = v0/D - F/D^2 * ln(1 + v0*D/F)
/// where F = FRICTION, D = AIR_DRAG.
///
/// Binary search finds the v0 that produces the target distance.
fn impulse_for_distance(target_distance: f64) -> f64 {
    if target_distance <= 0.0 {
        return 0.0;
    }

    let mut lo = 0.0;
    // Upper bound: v0/D grows linearly, so target * D * 4 is a safe upper bound.
    let mut hi = target_distance * AIR_DRAG * 4.0;

    for _ in 0..64 {
        let mid = (lo + hi) / 2.0;
        if analytical_distance(mid) < target_distance {
            lo = mid;
        } else {
            hi = mid;
        }
    }

    (lo + hi) / 2.0
}

/// Analytical total scroll distance for a given initial velocity.
/// Derived from integrating v(t) = (v0 + F/D)*exp(-D*t) - F/D
/// from t=0 until v = MIN_VELOCITY (matching the simulation stop condition).
fn analytical_distance(v0: f64) -> f64 {
    if v0 <= MIN_VELOCITY {
        return 0.0;
    }
    let a = FRICTION / AIR_DRAG; // F/D
    // distance = (v0 - MIN_VELOCITY) / D - F/D^2 * ln((v0 + F/D) / (MIN_VELOCITY + F/D))
    (v0 - MIN_VELOCITY) / AIR_DRAG - a / AIR_DRAG * ((v0 + a) / (MIN_VELOCITY + a)).ln()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analytical_distance_sanity() {
        // Below MIN_VELOCITY, distance is 0
        assert!((analytical_distance(0.0) - 0.0).abs() < f64::EPSILON);
        assert!((analytical_distance(MIN_VELOCITY) - 0.0).abs() < f64::EPSILON);
        // Monotonically increasing above MIN_VELOCITY
        let mut prev = 0.0;
        for i in 2..=100 {
            let v0 = i as f64;
            let d = analytical_distance(v0);
            assert!(
                d > prev,
                "distance must increase: v0={v0}, d={d}, prev={prev}"
            );
            prev = d;
        }
    }

    #[test]
    fn impulse_for_known_distances() {
        for target in [5.0, 10.0, 20.0, 50.0, 100.0] {
            let v0 = impulse_for_distance(target);
            let actual = analytical_distance(v0);
            assert!(
                (actual - target).abs() < 0.01,
                "target={target}, v0={v0}, actual={actual}"
            );
        }
    }

    /// Run the simulation to completion and return total rows scrolled.
    fn resolve(physics: &mut ScrollPhysics) -> isize {
        let dt = 1.0 / 120.0;
        let mut total = 0isize;
        for _ in 0..10000 {
            if !physics.is_active() {
                break;
            }
            total = total.saturating_add(physics.tick(dt));
        }
        total
    }

    #[test]
    fn physics_basic_scroll_down() {
        let mut physics = ScrollPhysics::new();
        physics.impulse(20.0);
        assert!(physics.is_active());

        let total = resolve(&mut physics);
        // Physics approximates the target; allow some tolerance
        assert!((total - 20).abs() <= 3, "expected ~20 rows, got {total}");
        assert!(!physics.is_active());
    }

    #[test]
    fn physics_basic_scroll_up() {
        let mut physics = ScrollPhysics::new();
        physics.impulse(-15.0);
        assert!(physics.is_active());

        let total = resolve(&mut physics);
        assert!((total + 15).abs() <= 3, "expected ~-15 rows, got {total}");
    }

    #[test]
    fn physics_momentum_accumulation() {
        // Two impulses of 10 add to velocity, producing more total distance
        // than a single impulse of 20 (due to the nonlinear friction model).
        let mut single = ScrollPhysics::new();
        single.impulse(20.0);
        let single_total = resolve(&mut single);

        let mut double = ScrollPhysics::new();
        double.impulse(10.0);
        double.impulse(10.0);
        let double_total = resolve(&mut double);

        assert!(
            double_total > single_total,
            "momentum should accumulate: double={double_total}, single={single_total}"
        );
    }

    #[test]
    fn physics_stop() {
        let mut physics = ScrollPhysics::new();
        physics.impulse(20.0);
        assert!(physics.is_active());

        physics.stop();
        assert!(!physics.is_active());
        assert_eq!(physics.tick(0.008), 0);
    }

    #[test]
    fn physics_tick_returns_zero_when_idle() {
        let mut physics = ScrollPhysics::new();
        assert_eq!(physics.tick(0.008), 0);
    }
}
