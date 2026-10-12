//! Gameplay constants. Everything rhythmic is expressed in updates (1/60 s) on a
//! 150 BPM grid, so hazards, moves and music all line up.

// ---- Rhythm grid -------------------------------------------------------------
pub const UPDATES_PER_SECOND: u32 = 60;
/// One 16th note at 150 BPM.
pub const STEP_TICKS: u32 = 6;
pub const STEPS_PER_BAR: u32 = 16;
pub const BEAT_TICKS: u32 = STEP_TICKS * 4;
pub const BAR_TICKS: u32 = STEP_TICKS * STEPS_PER_BAR;
/// World time is kept in 1/256 updates so slow-motion can scale it exactly.
pub const Q: u64 = 256;

// ---- Playfield & hero ----------------------------------------------------------
pub const GROUND_Y: f32 = 408.0;
pub const PLAYER_X: f32 = 112.0;
pub const PLAYER_W: f32 = 28.0;
pub const PLAYER_H: f32 = 38.0;
pub const SLIDE_H: f32 = 18.0;
/// Hitbox shrink per side, so grazing a corner doesn't feel unfair.
pub const PLAYER_HITBOX_INSET_X: f32 = 4.0;
pub const PLAYER_HITBOX_INSET_Y: f32 = 3.0;
pub const fn player_center_x() -> f32 {
    PLAYER_X + PLAYER_W / 2.0
}

// ---- Moves ---------------------------------------------------------------------
/// Jump air time is 1.5 beats (36 updates) with an apex of ~97 px.
pub const JUMP_TICKS: u32 = 36;
pub const GRAVITY: f32 = 2150.0;
pub const JUMP_VELOCITY: f32 = -645.0;
/// Pressing slide in the air slams down at this speed.
pub const FAST_FALL_VELOCITY: f32 = 900.0;
/// A slide lasts one beat.
pub const SLIDE_TICKS: u32 = BEAT_TICKS;
/// A kick is active for half a beat.
pub const KICK_TICKS: u32 = BEAT_TICKS / 2;
/// The kick hits anything this far in front of the hero's body.
pub const KICK_REACH: f32 = 40.0;

// ---- Hazards -------------------------------------------------------------------
pub const BLOCK_MIN_W: u32 = 26;
pub const BLOCK_MAX_W: u32 = 38;
pub const BLOCK_MIN_H: u32 = 28;
pub const BLOCK_MAX_H: u32 = 48;
pub const BEAM_W: f32 = 26.0;
/// Bottom edge of an overhead beam: standing heads hit it, sliding ones pass.
pub const BEAM_BOTTOM: f32 = GROUND_Y - 26.0;
pub const BEAM_TOP: f32 = 60.0;
pub const WALL_W: f32 = 22.0;
/// Walls are taller than the jump apex, so they must be kicked.
pub const WALL_H: f32 = 150.0;
pub const MAX_HAZARDS: usize = 16;

// ---- Pickups -------------------------------------------------------------------
pub const COIN_W: f32 = 10.0;
pub const COIN_H: f32 = 14.0;
pub const MAX_COINS: usize = 64;
pub const POWERUP_SIZE: f32 = 22.0;
/// Coins needed per trail level; level 4 is EXTRA.
pub const COINS_PER_LEVEL: u32 = 10;
pub const MAX_LEVEL: u8 = 4;
/// How long a collected coin's sparkle lingers.
pub const SPARKLE_TIME: f32 = 0.25;

// ---- Power-ups (seconds of real time) -----------------------------------------
pub const MAGNET_TIME: f32 = 8.0;
pub const MAGNET_RANGE: f32 = 140.0;
pub const MAGNET_PULL: f32 = 420.0;
pub const SLOWMO_TIME: f32 = 5.0;
/// World time runs at 154/256 ≈ 0.6x during slow-mo.
pub const SLOWMO_SCALE_Q: u64 = 154;
/// Invulnerability after the shield absorbs a hit.
pub const SHIELD_GRACE: f32 = 0.5;

// ---- Scoring -------------------------------------------------------------------
pub const DISTANCE_PER_POINT: f32 = 20.0;
pub const COIN_POINTS: u32 = 5;
pub const CLEAR_POINTS: u32 = 10;
pub const PERFECT_POINTS: u32 = 25;
/// A move within this many updates of the ideal one counts as PERFECT.
pub const PERFECT_WINDOW: u32 = 2;
/// Streak needed for each combo multiplier step (x2, x4, x8).
pub const COMBO_STEPS: [u32; 3] = [10, 25, 50];

// ---- Zones ---------------------------------------------------------------------
pub const ZONE_BARS: u32 = 16;
/// Empty bars at the start of a run and at the end of each zone.
pub const INTRO_BARS: u32 = 2;
pub const GATE_BARS: u32 = 2;
/// Bars spawned ahead of the one playing.
pub const LOOKAHEAD_BARS: u32 = 2;
/// Each lap through all zones is this much faster (percent).
pub const LAP_SPEEDUP_PERCENT: u32 = 10;

// ---- Presentation timing -------------------------------------------------------
pub const GAME_OVER_LOCKOUT: f32 = 0.4;
pub const CRASH_FLASH: f32 = 0.08;
pub const TITLE_SCROLL_SPEED: f32 = 100.0;

/// Seconds in the air for a full jump.
pub const fn airtime() -> f32 {
    JUMP_TICKS as f32 / UPDATES_PER_SECOND as f32
}

/// Height of the hero's feet above the ground `t` seconds into a jump.
pub fn jump_height(t: f32) -> f32 {
    -JUMP_VELOCITY * t - 0.5 * GRAVITY * t * t
}
