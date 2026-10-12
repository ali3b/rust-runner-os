//! One-bar patterns placed on the 16th-note grid. Each entry's step is when the
//! matching move should be pressed, so a well-played run lands every move on the
//! music. The rules the patterns must obey are checked by the tests below.
use super::hazards::HazardKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Thing {
    /// A block to jump; `arc` puts gold along the ideal jump.
    Jump { arc: bool },
    /// An overhead beam to slide under; `gold` puts gold under it.
    Slide { gold: bool },
    /// A wall to kick through.
    Kick,
    /// A row of gold at running height, one per step.
    Gold(u8),
}

impl Thing {
    pub fn hazard(self) -> Option<HazardKind> {
        match self {
            Thing::Jump { .. } => Some(HazardKind::Block),
            Thing::Slide { .. } => Some(HazardKind::Beam),
            Thing::Kick => Some(HazardKind::Wall),
            Thing::Gold(_) => None,
        }
    }
}

pub type Pattern = &'static [(u8, Thing)];

const J: Thing = Thing::Jump { arc: false };
const JA: Thing = Thing::Jump { arc: true };
const S: Thing = Thing::Slide { gold: false };
const SG: Thing = Thing::Slide { gold: true };
const K: Thing = Thing::Kick;
const fn g(n: u8) -> Thing {
    Thing::Gold(n)
}

/// Neon City: jumps and slides.
pub static ZONE1: &[Pattern] = &[
    &[(0, JA), (8, g(4))],
    &[(0, J), (8, JA)],
    &[(0, SG), (6, g(4))],
    &[(0, J), (8, SG)],
    &[(0, SG), (8, JA)],
    &[(4, JA), (10, g(4))],
    &[(0, g(4)), (8, J)],
];

/// Sunset Desert: kicks join in.
pub static ZONE2: &[Pattern] = &[
    &[(0, K), (4, K), (8, g(4))],
    &[(0, J), (8, K), (12, K)],
    &[(0, K), (4, JA)],
    &[(0, SG), (8, K), (12, K)],
    &[(0, JA), (8, SG)],
    &[(0, K), (4, SG), (12, g(3))],
];

/// Deep Space: denser, some off-beat.
pub static ZONE3: &[Pattern] = &[
    &[(0, K), (4, SG), (12, K)],
    &[(0, JA), (8, K), (12, K)],
    &[(0, K), (4, K), (8, JA)],
    &[(0, SG), (6, K), (10, K)],
    &[(0, J), (8, SG)],
    &[(0, S), (8, JA)],
];

/// Hyper Grid: everything, fast.
pub static ZONE4: &[Pattern] = &[
    &[(0, K), (4, K), (8, K), (12, K)],
    &[(0, SG), (6, K), (10, K)],
    &[(0, JA), (8, K), (12, K)],
    &[(0, K), (4, JA), (12, K)],
    &[(0, K), (4, SG), (10, K)],
];

pub static POOLS: [&[Pattern]; 4] = [ZONE1, ZONE2, ZONE3, ZONE4];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::tuning::STEPS_PER_BAR;

    fn all() -> impl Iterator<Item = Pattern> {
        POOLS.iter().flat_map(|p| p.iter().copied())
    }

    #[test]
    fn steps_are_sorted_and_in_the_bar() {
        for p in all() {
            assert!(p.windows(2).all(|w| w[0].0 < w[1].0), "{p:?}");
            assert!(p.iter().all(|&(s, _)| (s as u32) < STEPS_PER_BAR));
        }
    }

    /// The hero must be free again before each move, including across the bar line
    /// into any other pattern (which may start with a move on step 0).
    #[test]
    fn moves_never_overlap() {
        for p in all() {
            let moves: Vec<(u32, HazardKind)> =
                p.iter().filter_map(|&(s, t)| t.hazard().map(|k| (s as u32, k))).collect();
            for w in moves.windows(2) {
                assert!(w[1].0 - w[0].0 >= w[0].1.busy_steps(), "{p:?}");
            }
            if let Some(&(s, k)) = moves.last() {
                assert!(s + k.busy_steps() <= STEPS_PER_BAR, "{p:?} spills into the next bar");
            }
        }
    }

    /// Gold rows only where the hero is on the ground and nothing is in the way.
    #[test]
    fn gold_rows_are_reachable() {
        for p in all() {
            for &(gs, t) in p.iter() {
                let Thing::Gold(n) = t else { continue };
                for step in gs as u32..gs as u32 + n as u32 {
                    assert!(step < STEPS_PER_BAR, "{p:?}");
                    for &(hs, h) in p.iter() {
                        let hs = hs as u32;
                        let blocked = match h {
                            Thing::Jump { .. } => step > hs && step < hs + 6,
                            Thing::Slide { .. } => step >= hs && step <= hs + 4,
                            Thing::Kick => step > hs && step <= hs + 2,
                            Thing::Gold(_) => false,
                        };
                        assert!(!blocked, "{p:?}: gold at step {step} collides with move at {hs}");
                    }
                }
            }
        }
    }
}
