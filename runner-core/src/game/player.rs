use super::tuning::*;
use crate::engine::collision::Aabb;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pose {
    Run,
    Jump,
    Slide,
    Kick,
}

#[derive(Clone, Debug)]
pub struct Player {
    /// Top edge of the standing sprite.
    pub y: f32,
    pub vy: f32,
    pub grounded: bool,
    /// World time of slide (or kick) left, in 1/Q updates; 0 when not sliding
    /// (kicking). World time, so slow-motion stretches moves along with the world.
    pub slide_left: u64,
    pub kick_left: u64,
    /// Set for one update when the hero touches down (for dust).
    pub just_landed: bool,
}

impl Player {
    pub fn new() -> Self {
        Self { y: GROUND_Y - PLAYER_H, vy: 0.0, grounded: true, slide_left: 0, kick_left: 0, just_landed: false }
    }

    pub fn pose(&self) -> Pose {
        if self.kick_left > 0 {
            Pose::Kick
        } else if self.slide_left > 0 {
            Pose::Slide
        } else if !self.grounded {
            Pose::Jump
        } else {
            Pose::Run
        }
    }

    /// Starts a jump (also cancels a slide). Returns false while airborne.
    pub fn jump(&mut self) -> bool {
        if !self.grounded {
            return false;
        }
        self.vy = JUMP_VELOCITY;
        self.grounded = false;
        self.slide_left = 0;
        true
    }

    /// On the ground: slide. In the air: slam down. Returns true if a slide started.
    pub fn slide(&mut self) -> bool {
        if self.grounded {
            self.slide_left = SLIDE_TICKS as u64 * Q;
            self.kick_left = 0;
            true
        } else {
            self.vy = self.vy.max(FAST_FALL_VELOCITY);
            false
        }
    }

    /// Starts a kick (ground or air). Not while sliding.
    pub fn kick(&mut self) -> bool {
        if self.slide_left > 0 || self.kick_left > 0 {
            return false;
        }
        self.kick_left = KICK_TICKS as u64 * Q;
        true
    }

    /// One update covering `dt` seconds / `dq` 1/Q-updates of world time.
    pub fn update(&mut self, dt: f32, dq: u64) {
        self.just_landed = false;
        self.slide_left = self.slide_left.saturating_sub(dq);
        self.kick_left = self.kick_left.saturating_sub(dq);
        if self.grounded {
            return;
        }
        self.vy += GRAVITY * dt;
        self.y += self.vy * dt;
        let floor = GROUND_Y - PLAYER_H;
        if self.y >= floor {
            self.y = floor;
            self.vy = 0.0;
            self.grounded = true;
            self.just_landed = true;
        }
    }

    /// The full sprite box (used for picking things up).
    pub fn body(&self) -> Aabb {
        if self.slide_left > 0 {
            Aabb::new(PLAYER_X, GROUND_Y - SLIDE_H, PLAYER_W, SLIDE_H)
        } else {
            Aabb::new(PLAYER_X, self.y, PLAYER_W, PLAYER_H)
        }
    }

    /// The box that collides with hazards.
    pub fn hitbox(&self) -> Aabb {
        self.body().inset(PLAYER_HITBOX_INSET_X, PLAYER_HITBOX_INSET_Y)
    }

    /// Area the kick breaks things in, while a kick is active.
    pub fn kick_zone(&self) -> Option<Aabb> {
        (self.kick_left > 0).then(|| Aabb::new(PLAYER_X + PLAYER_W, self.y, KICK_REACH, PLAYER_H))
    }
}

impl Default for Player {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::time::STEP_SECONDS;

    #[test]
    fn jump_lasts_one_and_a_half_beats() {
        let mut p = Player::new();
        let start = p.y;
        assert!(p.jump());
        assert!(!p.jump(), "no double jump");
        let (mut apex, mut ticks) = (start, 0u32);
        while !p.grounded {
            p.update(STEP_SECONDS, Q);
            apex = apex.min(p.y);
            ticks += 1;
            assert!(ticks < 200);
        }
        assert!(ticks.abs_diff(JUMP_TICKS) <= 1, "air time {ticks} updates");
        let height = start - apex;
        assert!((90.0..105.0).contains(&height), "apex {height}");
        assert!(height > BLOCK_MAX_H as f32 + 30.0);
        assert!(p.just_landed);
    }

    #[test]
    fn slide_shrinks_hitbox_for_one_beat() {
        let mut p = Player::new();
        assert!(p.slide());
        assert!(p.hitbox().y > BEAM_BOTTOM, "sliding head is below beams");
        for _ in 0..SLIDE_TICKS {
            p.update(STEP_SECONDS, Q);
        }
        assert_eq!(p.pose(), Pose::Run);
        assert!(p.hitbox().y < BEAM_BOTTOM, "standing head hits beams");
    }

    #[test]
    fn slide_in_air_fast_falls_and_jump_cancels_slide() {
        let mut p = Player::new();
        p.jump();
        p.update(STEP_SECONDS, Q);
        assert!(!p.slide());
        assert!(p.vy >= FAST_FALL_VELOCITY);
        let mut q = Player::new();
        q.slide();
        assert!(q.jump());
        assert_eq!(q.slide_left, 0);
    }

    #[test]
    fn kick_has_a_window_and_reach() {
        let mut p = Player::new();
        assert!(p.kick_zone().is_none());
        assert!(p.kick());
        assert!(!p.kick(), "no re-kick while active");
        let z = p.kick_zone().unwrap();
        assert_eq!(z.x, PLAYER_X + PLAYER_W);
        for _ in 0..KICK_TICKS {
            p.update(STEP_SECONDS, Q);
        }
        assert!(p.kick_zone().is_none());
    }

    #[test]
    fn jumping_cannot_clear_a_wall_or_beam() {
        let mut p = Player::new();
        p.jump();
        let mut highest_top = f32::MAX;
        while !p.grounded {
            p.update(STEP_SECONDS, Q);
            highest_top = highest_top.min(p.hitbox().y);
        }
        let feet_at_apex = highest_top + PLAYER_H - 2.0 * PLAYER_HITBOX_INSET_Y;
        assert!(feet_at_apex > GROUND_Y - WALL_H, "a wall is taller than the jump");
        assert!(highest_top > BEAM_TOP, "a beam reaches above the jump");
    }
}
