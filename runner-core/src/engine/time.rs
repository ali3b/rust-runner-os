/// Length of one simulation step: 1/60 s.
pub const STEP_NS: u64 = 16_666_667;
pub const STEP_SECONDS: f32 = 1.0 / 60.0;
/// Larger real-time gaps (debugger pause, host hiccup) are clamped to this.
pub const MAX_FRAME_DELTA_NS: u64 = 250_000_000;
/// At most this many updates run per rendered frame; the rest of the backlog is dropped.
pub const MAX_CATCHUP: u32 = 5;

/// Fixed-timestep accumulator driven by a monotonic nanosecond clock.
#[derive(Debug, Default)]
pub struct FixedStep {
    last_ns: Option<u64>,
    accumulator_ns: u64,
}

impl FixedStep {
    pub const fn new() -> Self {
        Self { last_ns: None, accumulator_ns: 0 }
    }

    /// Feeds the current time and returns how many fixed updates to run now.
    pub fn advance(&mut self, now_ns: u64) -> u32 {
        let elapsed = match self.last_ns {
            Some(last) => now_ns.saturating_sub(last).min(MAX_FRAME_DELTA_NS),
            None => 0,
        };
        self.last_ns = Some(now_ns);
        self.accumulator_ns += elapsed;

        let mut steps = 0;
        while self.accumulator_ns >= STEP_NS && steps < MAX_CATCHUP {
            self.accumulator_ns -= STEP_NS;
            steps += 1;
        }
        if steps == MAX_CATCHUP {
            // Too far behind: drop the backlog rather than spiralling.
            self.accumulator_ns %= STEP_NS;
        }
        steps
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_call_runs_nothing() {
        let mut f = FixedStep::new();
        assert_eq!(f.advance(123_456_789), 0);
    }

    #[test]
    fn steps_follow_real_time_not_call_count() {
        let mut f = FixedStep::new();
        f.advance(0);
        // Called very often: the number of steps still tracks elapsed time.
        let mut total = 0;
        let mut t = 0;
        while t < 1_000_000_000 {
            t += 1_000_000; // 1 ms per call
            total += f.advance(t);
        }
        assert!((59..=60).contains(&total), "got {total} steps in 1 s");
    }

    #[test]
    fn sub_step_time_accumulates() {
        let mut f = FixedStep::new();
        f.advance(0);
        assert_eq!(f.advance(10_000_000), 0);
        assert_eq!(f.advance(20_000_000), 1);
    }

    #[test]
    fn large_gaps_are_clamped_and_capped() {
        let mut f = FixedStep::new();
        f.advance(0);
        assert_eq!(f.advance(10_000_000_000), MAX_CATCHUP);
        // Backlog was dropped: the next tiny advance must not run more catch-up steps.
        assert_eq!(f.advance(10_000_000_001), 0);
    }

    #[test]
    fn clock_going_backwards_is_ignored() {
        let mut f = FixedStep::new();
        f.advance(1_000_000_000);
        assert_eq!(f.advance(500_000_000), 0);
    }
}
