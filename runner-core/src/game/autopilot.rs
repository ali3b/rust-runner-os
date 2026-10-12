//! Demo bot used by the `autoplay` kernel build (for recordings) and by the
//! fairness tests: presses the right move for each hazard on its beat, optionally
//! off by a few updates to prove the timing windows are forgiving enough.
use super::{
    hazards::HazardKind,
    state::{Game, GameMode},
};
use crate::engine::input::InputState;

/// Perfect play.
pub fn wants(game: &Game) -> InputState {
    wants_with_jitter(game, |_| 0)
}

/// Like [`wants`], but each hazard's press is shifted by `jitter(press_tick)` updates.
pub fn wants_with_jitter(game: &Game, jitter: impl Fn(u32) -> i32) -> InputState {
    let mut input = InputState::default();
    if game.mode != GameMode::Playing {
        return input;
    }
    let now = game.tick() as i64;
    for h in game.hazards.iter().filter(|h| !h.broken && !h.cleared) {
        if now != h.press_tick as i64 + jitter(h.press_tick) as i64 {
            continue;
        }
        match h.kind {
            HazardKind::Block => input.jump_pressed |= game.player.grounded,
            HazardKind::Beam => input.slide_pressed |= game.player.slide_left == 0,
            HazardKind::Wall => input.kick_pressed |= game.player.kick_left == 0,
        }
    }
    input
}
