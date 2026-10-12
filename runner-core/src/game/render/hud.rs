//! Score, combo, power-up timers, banners, popups and the title/game-over screens.
use super::{actors, palette::*};
use crate::engine::{
    font::{draw_text, format_u32, text_width},
    graphics::{Canvas, Color, WIDTH, floor_i32},
};
use crate::game::{
    state::{Game, GameMode, POPUP_TIME, PopupKind},
    tuning::*,
    zones::ZONES,
};

const W: i32 = WIDTH as i32;
const LEVEL_NAMES: [&str; 5] = ["", "BOOST!", "TURBO!", "RAINBOW!", "EXTRA!!"];
const LEVEL_COLORS: [Color; 4] = [TRAIL + 2, TRAIL + 3, TRAIL + 5, TRAIL];
const BANNER_TIME: f32 = 1.5;
const ZONE_BANNER_TIME: f32 = 2.2;

pub fn shadow_text(c: &mut Canvas<'_>, text: &str, x: i32, y: i32, scale: i32, color: Color) {
    draw_text(c, text, x + scale, y + scale, scale, SHADOW);
    draw_text(c, text, x, y, scale, color);
}

pub fn shadow_text_centered(c: &mut Canvas<'_>, text: &str, y: i32, scale: i32, color: Color) {
    shadow_text(c, text, (W - text_width(text, scale)) / 2, y, scale, color);
}

/// "LABEL value" centred, label in white and value in `color`.
fn labelled(c: &mut Canvas<'_>, label: &str, value: &str, y: i32, scale: i32, color: Color) {
    let lw = text_width(label, scale) + 8 * scale;
    let x = (W - lw - text_width(value, scale)) / 2;
    shadow_text(c, label, x, y, scale, TEXT);
    shadow_text(c, value, x + lw, y, scale, color);
}

fn blink(game: &Game, period: u64) -> bool {
    (game.ticks / period).is_multiple_of(2)
}

pub fn draw_hud(c: &mut Canvas<'_>, game: &Game) {
    let mut buf = [0; 10];
    shadow_text(c, "SCORE", 16, 14, 2, TEXT);
    shadow_text(c, format_u32(game.score, 7, &mut buf), 104, 14, 2, ACCENT);
    let best = game.best.max(game.score);
    let hi = format_u32(best, 7, &mut buf);
    let x = W - 16 - text_width(hi, 2);
    shadow_text(c, hi, x, 14, 2, ACCENT);
    shadow_text(c, "HI", x - 40, 14, 2, TEXT);

    // Gold counter.
    c.fill_rect(17, 41, 9, 15, GOLD_DARK);
    c.fill_rect(18, 42, 7, 13, GOLD);
    c.fill_rect(19, 43, 2, 10, GOLD_LIGHT);
    shadow_text(c, "x", 32, 42, 2, TEXT);
    shadow_text(c, format_u32(game.coin_count, 3, &mut buf), 50, 42, 2, ACCENT);

    // Combo multiplier.
    let mult = game.multiplier();
    if mult > 1 || game.combo.streak > 0 {
        let mut mbuf = [0; 10];
        let m = format_u32(mult, 1, &mut mbuf);
        let color = match mult {
            1 => TEXT,
            2 => TRAIL + 3,
            4 => TRAIL + 4,
            8 => TRAIL + 1,
            _ => TRAIL,
        };
        shadow_text(c, "x", 108, 40, 2, color);
        shadow_text(c, m, 124, 36, 3, color);
        let lx = 124 + text_width(m, 3) + 8;
        shadow_text(c, "COMBO", lx, 38, 1, TEXT);
        shadow_text(c, format_u32(game.combo.streak, 1, &mut buf), lx, 49, 1, color);
    }

    // Trail level pips (the fourth is EXTRA).
    for i in 0..MAX_LEVEL as i32 {
        let px = W - 16 - (MAX_LEVEL as i32 - i) * 22 + 4;
        c.fill_rect(px - 1, 41, 18, 12, SHADOW);
        let color = if i < game.level as i32 {
            if i == 3 { TRAIL + ((game.ticks / 4) % 6) as Color } else { LEVEL_COLORS[i as usize] }
        } else {
            PANEL
        };
        c.fill_rect(px, 42, 16, 10, color);
    }

    // Zone name and lap.
    let zone = ZONES[game.zone].name;
    let lw = text_width(zone, 1);
    shadow_text(c, zone, (W - lw) / 2, 18, 1, TEXT);
    if game.lap > 0 {
        let mut lbuf = [0; 10];
        labelled(c, "LAP", format_u32(game.lap + 1, 1, &mut lbuf), 30, 1, ACCENT);
    }

    // Active power-ups with time bars.
    let mut y = 66;
    if game.shield {
        power_row(c, game, crate::game::pickups::PowerKind::Shield, y, 1.0);
        y += 18;
    }
    if game.magnet > 0.0 {
        power_row(c, game, crate::game::pickups::PowerKind::Magnet, y, game.magnet / MAGNET_TIME);
        y += 18;
    }
    if game.slowmo > 0.0 {
        power_row(c, game, crate::game::pickups::PowerKind::SlowMo, y, game.slowmo / SLOWMO_TIME);
    }

    if game.mode == GameMode::Playing {
        if game.level > 0 && game.level_time < BANNER_TIME && !(game.ticks / 5).is_multiple_of(3) {
            let color = LEVEL_COLORS[(game.level as usize - 1).min(3)];
            shadow_text_centered(c, LEVEL_NAMES[game.level as usize], 84, 4, color);
        }
        if game.zone_time < ZONE_BANNER_TIME && (game.bar() > 0 || game.zone_time > 0.3) {
            zone_banner(c, game);
        }
    }
}

fn power_row(c: &mut Canvas<'_>, game: &Game, kind: crate::game::pickups::PowerKind, y: i32, left: f32) {
    let color = actors::power_color(kind);
    c.fill_circle(24, y + 6, 7, color);
    draw_text(c, actors::power_letter(kind), 21, y + 3, 1, SHADOW);
    shadow_text(c, kind.name(), 38, y + 2, 1, color);
    let w = (60.0 * left) as i32;
    c.fill_rect(104, y + 4, 62, 6, SHADOW);
    if w > 0 && (left > 0.25 || blink(game, 6)) {
        c.fill_rect(105, y + 5, w, 4, color);
    }
}

fn zone_banner(c: &mut Canvas<'_>, game: &Game) {
    let t = game.zone_time;
    // Slide in from the right, hold, slide out to the left.
    let x_shift = if t < 0.3 {
        ((0.3 - t) / 0.3 * W as f32) as i32
    } else if t > ZONE_BANNER_TIME - 0.3 {
        -((t - (ZONE_BANNER_TIME - 0.3)) / 0.3 * W as f32) as i32
    } else {
        0
    };
    let mut nbuf = [0; 10];
    let number = format_u32(game.zone as u32 + 1, 1, &mut nbuf);
    let y = 130;
    c.fill_rect(x_shift, y - 10, W, 84, SHADOW);
    c.fill_rect(x_shift, y - 10, W, 3, GROUND_GLOW);
    c.fill_rect(x_shift, y + 71, W, 3, GROUND_GLOW);
    let label_w = text_width("ZONE ", 2) + text_width(number, 2);
    shadow_text(c, "ZONE", x_shift + (W - label_w) / 2, y, 2, ACCENT);
    shadow_text(c, number, x_shift + (W - label_w) / 2 + text_width("ZONE ", 2), y, 2, ACCENT);
    let name = ZONES[game.zone].name;
    shadow_text(c, name, x_shift + (W - text_width(name, 4)) / 2, y + 26, 4, TEXT);
}

pub fn draw_popups(c: &mut Canvas<'_>, game: &Game) {
    let mut buf = [0; 10];
    for p in game.popups.iter().flatten() {
        let (x, y) = (floor_i32(p.x), floor_i32(p.y));
        // Fade out by blinking in the last third.
        if p.age > POPUP_TIME * 0.66 && blink(game, 2) {
            continue;
        }
        match p.kind {
            PopupKind::Perfect => shadow_text(c, "PERFECT", x - 28, y - 20, 1, ACCENT),
            PopupKind::Points(n) => shadow_text(c, format_u32(n, 1, &mut buf), x, y - 30, 1, TEXT),
            PopupKind::Combo(m) => {
                let s = format_u32(m, 1, &mut buf);
                shadow_text(c, "x", x + 16, y - 48, 2, TRAIL + 1);
                shadow_text(c, s, x + 32, y - 52, 3, TRAIL + 1);
                shadow_text(c, "COMBO!", x + 16, y - 26, 1, TRAIL + 1);
            }
            PopupKind::Power(kind) => {
                let name = kind.name();
                shadow_text(c, name, x - text_width(name, 2) / 2, y - 44, 2, actors::power_color(kind));
            }
            PopupKind::Shield => shadow_text(c, "SAVED!", x - 24, y - 44, 2, SHIELD),
        }
    }
}

pub fn draw_title(c: &mut Canvas<'_>, game: &Game) {
    // Title wobbles on the beat.
    let bump = if game.step.is_multiple_of(4) { -3 } else { 0 };
    shadow_text_centered(c, "RUST RUNNER", 80 + bump, 6, TEXT);
    shadow_text_centered(c, "- O S -", 138, 2, ACCENT);
    if blink(game, 24) {
        let start = if game.touch { "TAP JUMP TO START" } else { "PRESS SPACE TO START" };
        shadow_text_centered(c, start, 200, 2, TEXT);
    }
    // Keep the small print readable over the skyline.
    c.fill_rect(140, 232, W - 280, 64, SHADOW);
    c.rect_outline(140, 232, W - 280, 64, PANEL_EDGE);
    let moves = if game.touch { "BUTTONS: JUMP   SLIDE   KICK" } else { "SPACE JUMP   DOWN SLIDE   RIGHT KICK" };
    shadow_text_centered(c, moves, 240, 1, TEXT);
    shadow_text_centered(c, "JUMP BLOCKS - SLIDE UNDER BEAMS - KICK WALLS", 256, 1, TRAIL + 4);
    shadow_text_centered(c, "GRAB GOLD - CHAIN COMBOS - HIT THE BEAT", 270, 1, GOLD);
    if !game.touch {
        shadow_text_centered(c, "C CRT   ESC TITLE", 286, 1, STAR_DIM);
    }
    if game.best > 0 {
        let mut buf = [0; 10];
        labelled(c, "BEST", format_u32(game.best, 7, &mut buf), 312, 2, ACCENT);
    }
}

pub fn draw_game_over(c: &mut Canvas<'_>, game: &Game) {
    let (w, h) = (540, 230);
    let (x, y) = ((W - w) / 2, 100);
    c.fill_rect(x, y, w, h, PANEL);
    c.rect_outline(x, y, w, h, PANEL_EDGE);
    c.rect_outline(x + 3, y + 3, w - 6, h - 6, PANEL_EDGE);
    shadow_text_centered(c, "GAME OVER", y + 20, 5, TEXT);
    let mut buf = [0; 10];
    labelled(c, "SCORE", format_u32(game.score, 7, &mut buf), y + 74, 2, ACCENT);
    if game.new_best {
        if blink(game, 15) {
            shadow_text_centered(c, "NEW BEST!", y + 100, 2, ACCENT);
        }
    } else {
        labelled(c, "BEST", format_u32(game.best, 7, &mut buf), y + 100, 2, ACCENT);
    }
    // Run stats.
    let stats: [(&str, u32); 4] = [
        ("ZONE", game.zone as u32 + 1 + game.lap * ZONES.len() as u32),
        ("GOLD", game.coin_count),
        ("PERFECT", game.combo.perfects),
        ("COMBO", game.combo.best_streak),
    ];
    for (i, (label, value)) in stats.iter().enumerate() {
        let cx = x + 30 + i as i32 * 125;
        shadow_text(c, label, cx, y + 136, 1, STAR_DIM);
        shadow_text(c, format_u32(*value, 1, &mut buf), cx, y + 148, 2, TEXT);
    }
    if game.mode_time >= GAME_OVER_LOCKOUT {
        let retry = if game.touch { "JUMP RETRY   MENU TITLE" } else { "SPACE/R RETRY   ESC TITLE" };
        shadow_text_centered(c, retry, y + 194, 2, TEXT);
    }
}
