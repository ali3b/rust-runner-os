//! Things that end a run: blocks (jump), overhead beams (slide) and walls (kick).
use super::tuning::*;
use crate::engine::collision::Aabb;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HazardKind {
    Block,
    Beam,
    Wall,
}

impl HazardKind {
    /// Updates between the ideal button press and the hazard reaching its target.
    pub const fn lead_ticks(self) -> u32 {
        match self {
            HazardKind::Block => JUMP_TICKS / 2,
            HazardKind::Beam => SLIDE_TICKS / 2,
            HazardKind::Wall => KICK_TICKS / 2,
        }
    }

    /// Steps the hero is busy after pressing this hazard's move, before the next
    /// hazard's move can be pressed.
    pub const fn busy_steps(self) -> u32 {
        match self {
            HazardKind::Block => 8,
            HazardKind::Beam => 6,
            HazardKind::Wall => 4,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hazard {
    pub kind: HazardKind,
    /// Left edge.
    pub x: f32,
    pub w: f32,
    /// Height above the ground (blocks and walls).
    pub h: f32,
    pub style: u8,
    /// World update at which pressing the matching move is perfect.
    pub press_tick: u32,
    pub broken: bool,
    pub cleared: bool,
    /// The player already got a timing judgement for this hazard.
    pub judged: bool,
}

impl Hazard {
    pub fn new(kind: HazardKind, w: f32, h: f32, style: u8, press_tick: u32) -> Self {
        Hazard { kind, x: 0.0, w, h, style, press_tick, broken: false, cleared: false, judged: false }
    }

    /// Left edge when the hazard is exactly "on target" (at `press_tick + lead`).
    pub fn target_x(&self) -> f32 {
        match self.kind {
            HazardKind::Block | HazardKind::Beam => player_center_x() - self.w / 2.0,
            HazardKind::Wall => PLAYER_X + PLAYER_W + KICK_REACH / 2.0,
        }
    }

    pub fn bounds(&self) -> Option<Aabb> {
        if self.broken {
            return None;
        }
        Some(match self.kind {
            HazardKind::Block | HazardKind::Wall => Aabb::new(self.x, GROUND_Y - self.h, self.w, self.h),
            HazardKind::Beam => Aabb::new(self.x, BEAM_TOP, self.w, BEAM_BOTTOM - BEAM_TOP),
        })
    }
}

#[derive(Clone, Debug)]
pub struct Hazards {
    pub slots: [Option<Hazard>; MAX_HAZARDS],
}

impl Hazards {
    pub fn new() -> Self {
        Self { slots: [None; MAX_HAZARDS] }
    }

    pub fn add(&mut self, h: Hazard) {
        if let Some(slot) = self.slots.iter_mut().find(|s| s.is_none()) {
            *slot = Some(h);
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &Hazard> {
        self.slots.iter().flatten()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Hazard> {
        self.slots.iter_mut().flatten()
    }

    /// Scrolls everything left by `travel` and drops what has left the screen.
    pub fn scroll(&mut self, travel: f32) {
        for slot in self.slots.iter_mut() {
            if let Some(h) = slot {
                h.x -= travel;
                if h.x + h.w < -40.0 {
                    *slot = None;
                }
            }
        }
    }
}

impl Default for Hazards {
    fn default() -> Self {
        Self::new()
    }
}
