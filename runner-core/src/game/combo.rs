//! Streak, multiplier and timing judgements.
use super::tuning::{COMBO_STEPS, PERFECT_WINDOW};

#[derive(Clone, Debug, Default)]
pub struct Combo {
    pub streak: u32,
    pub best_streak: u32,
    pub perfects: u32,
}

impl Combo {
    pub fn multiplier(&self) -> u32 {
        match COMBO_STEPS.iter().filter(|&&s| self.streak >= s).count() {
            0 => 1,
            1 => 2,
            2 => 4,
            _ => 8,
        }
    }

    /// Extends the streak. Returns the new multiplier if it just went up.
    pub fn hit(&mut self) -> Option<u32> {
        let before = self.multiplier();
        self.streak += 1;
        self.best_streak = self.best_streak.max(self.streak);
        let after = self.multiplier();
        (after > before).then_some(after)
    }

    /// Breaks the streak. Returns true if there was one to lose.
    pub fn reset(&mut self) -> bool {
        core::mem::replace(&mut self.streak, 0) > 0
    }
}

/// True if a press at `now` is within the PERFECT window of `ideal`.
pub fn is_perfect(now: u32, ideal: u32) -> bool {
    now.abs_diff(ideal) <= PERFECT_WINDOW
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multiplier_thresholds() {
        let mut c = Combo::default();
        let mut ups = vec![];
        for _ in 0..60 {
            if let Some(m) = c.hit() {
                ups.push((c.streak, m));
            }
        }
        assert_eq!(ups, vec![(10, 2), (25, 4), (50, 8)]);
        assert_eq!(c.multiplier(), 8);
        assert!(c.reset());
        assert_eq!(c.multiplier(), 1);
        assert!(!c.reset());
        assert_eq!(c.best_streak, 60);
    }

    #[test]
    fn perfect_window_edges() {
        assert!(is_perfect(100, 100));
        assert!(is_perfect(98, 100) && is_perfect(102, 100));
        assert!(!is_perfect(97, 100) && !is_perfect(103, 100));
    }
}
