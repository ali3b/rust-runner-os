//! Bit.Trip Runner–flavoured presentation: four zones of procedural scenery, a
//! dark hero with a rainbow trail, beat-pulsing hazards and a busy HUD.
mod actors;
mod hud;
mod palette;
mod scenery;

pub use palette::palette;

use super::state::{Game, GameMode};
use super::tuning::{CRASH_FLASH, STEP_TICKS};
use crate::engine::graphics::{Canvas, HEIGHT, WIDTH};
use palette::*;

/// Integer hash for procedural scenery.
pub(crate) fn hash(i: i32) -> u32 {
    let mut x = (i as u32) ^ 0x9E37_79B9;
    x = x.wrapping_mul(0x85EB_CA6B);
    x ^= x >> 13;
    x = x.wrapping_mul(0xC2B2_AE35);
    x ^= x >> 16;
    x
}

pub fn render_game(game: &Game, c: &mut Canvas<'_>) {
    if game.mode == GameMode::GameOver && game.mode_time < CRASH_FLASH {
        c.clear(FLASH);
        return;
    }
    // The world pulses for the first few updates of every beat.
    let beat = game.step.is_multiple_of(4) && game.tick() % STEP_TICKS < 3;

    // Screen shake moves the world, not the HUD.
    if game.shake > 0.0 {
        let amp = (game.shake * 20.0) as i32 + 1;
        let h = hash(game.ticks as i32);
        c.set_offset((h % (2 * amp as u32 + 1)) as i32 - amp, ((h >> 8) % (amp as u32 + 1)) as i32 - amp / 2);
    }
    scenery::draw(c, game, beat);
    speed_lines(c, game);
    for h in game.hazards.iter() {
        actors::draw_hazard(c, h, game, beat);
    }
    actors::draw_coins(c, game);
    actors::draw_item(c, game);
    // Dust and debris behind the hero.
    game.particles.draw(c);
    actors::draw_hero(c, game);
    c.set_offset(0, 0);

    if game.slowmo > 0.0 {
        // Tinted frame while time is slowed.
        let t = 6;
        c.fill_rect(0, 0, WIDTH as i32, t, SLOWMO);
        c.fill_rect(0, HEIGHT as i32 - t, WIDTH as i32, t, SLOWMO);
        c.fill_rect(0, 0, t, HEIGHT as i32, SLOWMO);
        c.fill_rect(WIDTH as i32 - t, 0, t, HEIGHT as i32, SLOWMO);
    }
    hud::draw_popups(c, game);
    match game.mode {
        GameMode::Title => hud::draw_title(c, game),
        GameMode::Playing => hud::draw_hud(c, game),
        GameMode::GameOver => {
            hud::draw_hud(c, game);
            hud::draw_game_over(c, game);
        }
    }
}

/// Streaks rushing past at high speed.
fn speed_lines(c: &mut Canvas<'_>, game: &Game) {
    let speed = game.scroll_speed();
    if speed < 260.0 {
        return;
    }
    let count = ((speed - 240.0) / 10.0) as i32;
    for i in 0..count.min(14) {
        let h = hash(i * 31 + 7);
        let y = 60 + (h % 330) as i32;
        let len = 40 + (h >> 9) as i32 % 80;
        let x = (WIDTH as i32 + 200) - ((game.ticks as i32 * (18 + (h % 7) as i32) + (h >> 3) as i32) % (WIDTH as i32 + 400));
        c.fill_rect(x, y, len, 1, SPEEDLINE);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{graphics::Rgb, input::InputState, time::STEP_SECONDS};
    use crate::game::{autopilot, tuning::*, update_game};

    #[test]
    fn hero_sprites_match_player_size() {
        for s in [&actors::RUN_A, &actors::RUN_B, &actors::JUMP_POSE, &actors::KICK_POSE, &actors::SLIDE_POSE] {
            assert!(s.rows.iter().all(|r| r.len() == s.width()), "ragged sprite");
        }
        for s in [&actors::RUN_A, &actors::RUN_B, &actors::JUMP_POSE] {
            assert_eq!(s.width() as i32 * actors::HERO_SCALE, PLAYER_W as i32);
            assert_eq!(s.height() as i32 * actors::HERO_SCALE, PLAYER_H as i32);
        }
        assert_eq!(actors::SLIDE_POSE.height() as i32 * actors::HERO_SCALE, SLIDE_H as i32);
    }

    /// Renders every zone, mode and power-up state; guards against panics in the
    /// procedural drawing code and checks something gets drawn.
    #[test]
    fn renders_everything_without_panicking() {
        let mut px = vec![0; WIDTH * HEIGHT];
        let mut g = Game::new(11);
        g.zone_bars = 6;
        update_game(&mut g, &InputState { jump_pressed: true, ..Default::default() }, STEP_SECONDS);
        let mut frame = 0;
        while g.bar() < 6 * 5 {
            let input = autopilot::wants(&g);
            update_game(&mut g, &input, STEP_SECONDS);
            if frame % 5 == 0 {
                render_game(&g, &mut Canvas::new(&mut px));
                let _ = palette(&g);
            }
            if frame == 600 {
                g.shield = true;
                g.magnet = 3.0;
                g.slowmo = 3.0;
                g.coin_count = 39;
            }
            frame += 1;
        }
        assert!(g.lap >= 1, "visited all zones");
        // Crash and render the game-over screen.
        g.shield = false;
        while g.mode == GameMode::Playing {
            update_game(&mut g, &InputState::default(), STEP_SECONDS);
        }
        for _ in 0..60 {
            update_game(&mut g, &InputState::default(), STEP_SECONDS);
            render_game(&g, &mut Canvas::new(&mut px));
        }
        assert!(px.iter().any(|&p| p != 0));
    }

    #[test]
    fn palette_slots_are_defined_for_every_zone() {
        let mut g = Game::new(1);
        for zone in 0..4 {
            g.zone = zone;
            g.prev_zone = zone;
            g.zone_time = 5.0;
            let p = palette(&g);
            for (i, &c) in p.iter().enumerate().take(LAST as usize + 1).skip(1) {
                assert!(c != Rgb::default() || i == HERO as usize, "zone {zone} slot {i} unset");
            }
        }
    }
}
