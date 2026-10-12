//! The hero (with trail and shield), hazards, gold bars and power-up items.
use super::palette::*;
use crate::engine::graphics::{Canvas, Color, Sprite, floor_i32};
use crate::game::{
    hazards::{Hazard, HazardKind},
    pickups::PowerKind,
    player::Pose,
    state::{Game, GameMode},
    tuning::*,
};

// Hero sprites, 14 px wide at 2x = 28 px (PLAYER_W).
pub const HERO_SCALE: i32 = 2;
pub static RUN_A: Sprite = Sprite {
    rows: &[
        b"....######....",
        b"...########...",
        b"...######oo#..",
        b"...######oo#..",
        b"...#########..",
        b"....#######...",
        b".....#####....",
        b"...#########..",
        b"..##.#####.##.",
        b".##..#####..#.",
        b".....#####....",
        b".....#####....",
        b".....##.##....",
        b"....##...##...",
        b"...##.....##..",
        b"..##.......#..",
        b".##........##.",
        b".#..........#.",
        b"..............",
    ],
};
pub static RUN_B: Sprite = Sprite {
    rows: &[
        b"..............",
        b"....######....",
        b"...########...",
        b"...######oo#..",
        b"...######oo#..",
        b"...#########..",
        b"....#######...",
        b".....#####....",
        b"...#########..",
        b"...#.#####.#..",
        b"...#.#####.#..",
        b".....#####....",
        b".....#####....",
        b".....##.##....",
        b".....##.##....",
        b".....##.##....",
        b".....##..##...",
        b"....###..###..",
        b"..............",
    ],
};
pub static JUMP_POSE: Sprite = Sprite {
    rows: &[
        b"....######....",
        b"...########...",
        b"...######oo#..",
        b"...######oo#..",
        b"...#########..",
        b"..##.#####.##.",
        b".##..#####..##",
        b"##...#####...#",
        b".....#####....",
        b".....#####....",
        b".....#####....",
        b".....######...",
        b"....##...###..",
        b"...##......##.",
        b"..##.......##.",
        b"..#...........",
        b"..............",
        b"..............",
        b"..............",
    ],
};
/// 9 rows at 2x = 18 px (SLIDE_H): leaning back, feet first.
pub static SLIDE_POSE: Sprite = Sprite {
    rows: &[
        b"..........#####",
        b"...#.....##oo##",
        b"..##....#######",
        b".##############",
        b"###############",
        b"..#############",
        b".......####....",
        b"....#####......",
        b"##########.....",
    ],
};
/// 18 px wide: the leg shoots forward.
pub static KICK_POSE: Sprite = Sprite {
    rows: &[
        b"....######........",
        b"...########.......",
        b"...######oo#......",
        b"...######oo#......",
        b"...#########......",
        b"....#######.......",
        b"..#..#####........",
        b".##.#######.......",
        b"##..########......",
        b".....#####........",
        b".....############.",
        b".....#############",
        b".....##......####.",
        b".....##...........",
        b"....##............",
        b"....##............",
        b"...##.............",
        b"..###.............",
        b"..................",
    ],
};

fn hero_color(b: u8) -> Color {
    match b {
        b'#' => HERO,
        b'o' => HERO_EYE,
        _ => 0,
    }
}

pub fn draw_hero(c: &mut Canvas<'_>, game: &Game) {
    let p = &game.player;
    let x = PLAYER_X as i32;
    let y = floor_i32(p.y);
    if game.mode == GameMode::GameOver {
        return; // the hero burst into particles
    }
    // Blink while invulnerable after a shield hit.
    if game.shield_grace > 0.0 && (game.ticks / 3).is_multiple_of(2) {
        return;
    }
    match p.pose() {
        Pose::Slide => c.blit_sprite(&SLIDE_POSE, x, (GROUND_Y - SLIDE_H) as i32, HERO_SCALE, hero_color),
        Pose::Kick => c.blit_sprite(&KICK_POSE, x, y, HERO_SCALE, hero_color),
        Pose::Jump => c.blit_sprite(&JUMP_POSE, x, y, HERO_SCALE, hero_color),
        Pose::Run => {
            let sprite = if (game.scroll / 28.0) as i32 % 2 == 0 { &RUN_A } else { &RUN_B };
            c.blit_sprite(sprite, x, y, HERO_SCALE, hero_color);
        }
    }
    if game.shield {
        let (cx, cy) = (x + PLAYER_W as i32 / 2, y + PLAYER_H as i32 / 2);
        ring(c, cx, cy, 30 + (game.ticks / 4 % 2) as i32, SHIELD);
    }
    if game.magnet > 0.0 {
        // Little sparks being pulled in.
        for k in 0..6u64 {
            let a = ((game.ticks * 3 + k * 40) % 120) as i32;
            let r = 60 - a / 2;
            let dir = [(1, 0), (1, -1), (0, -1), (1, 1), (-1, -1), (1, 0)][k as usize];
            c.fill_rect(x + 14 + dir.0 * r, y + 19 + dir.1 * r, 2, 2, MAGNET);
        }
    }
}

/// Circle outline built from per-row edge points.
pub fn ring(c: &mut Canvas<'_>, cx: i32, cy: i32, r: i32, color: Color) {
    for dy in -r..=r {
        let half = crate::engine::graphics::isqrt((r * r - dy * dy) as u32) as i32;
        c.fill_rect(cx - half, cy + dy, 2, 1, color);
        c.fill_rect(cx + half - 1, cy + dy, 2, 1, color);
    }
    c.fill_rect(cx - 3, cy - r, 6, 2, color);
    c.fill_rect(cx - 3, cy + r - 1, 6, 2, color);
}

pub fn draw_hazard(c: &mut Canvas<'_>, h: &Hazard, game: &Game, beat: bool) {
    if h.broken {
        return;
    }
    let x = floor_i32(h.x);
    match h.kind {
        HazardKind::Block => block(c, x, h.w as i32, h.h as i32, h.style, beat),
        HazardKind::Beam => beam(c, x, h.w as i32, game.ticks),
        HazardKind::Wall => wall(c, x, h.w as i32, h.h as i32, beat),
    }
}

fn block(c: &mut Canvas<'_>, x: i32, w: i32, h: i32, style: u8, beat: bool) {
    let style = style.min(3);
    let top = GROUND_Y as i32 - h;
    let (fill, shade) = (BLOCK + style, BLOCK_SHADE + style);
    let g = if beat { 3 } else { 2 };
    c.fill_rect(x - g, top - g, w + 2 * g, h + g, shade);
    c.fill_rect(x, top, w, h, fill);
    c.fill_rect(x, top, w, 3, BLOCK_SHINE);
    let mut gx = x + 6;
    while gx < x + w - 4 {
        c.fill_rect(gx, top + 7, 2, h - 10, shade);
        gx += 8;
    }
}

/// A hanging girder ending in a crackling energy emitter: slide under it.
fn beam(c: &mut Canvas<'_>, x: i32, w: i32, ticks: u64) {
    let top = BEAM_TOP as i32;
    let bottom = BEAM_BOTTOM as i32;
    // Cable to the top of the screen.
    c.fill_rect(x + w / 2 - 1, 0, 3, top, BEAM_DARK);
    // Girder with cross bracing.
    c.fill_rect(x, top, w, bottom - top - 12, BEAM_METAL);
    c.fill_rect(x, top, 3, bottom - top - 12, BEAM_DARK);
    c.fill_rect(x + w - 3, top, 3, bottom - top - 12, BEAM_DARK);
    let mut y = top + 6;
    while y < bottom - 24 {
        for d in 0..w - 6 {
            c.fill_rect(x + 3 + d, y + d * 12 / w, 1, 2, BEAM_DARK);
        }
        y += 16;
    }
    // Warning stripes and the emitter.
    let ey = bottom - 12;
    for k in 0..w / 6 + 1 {
        c.fill_rect(x + k * 6, ey, 3, 4, ACCENT);
    }
    let flicker = (ticks / 2) % 3;
    c.fill_rect(x - 2, ey + 4, w + 4, 8, BEAM_GLOW);
    c.fill_rect(x + 1, ey + 6 + flicker as i32 / 2, w - 2, 3, BEAM_HOT);
    // Sparks dripping off the bottom.
    let s = ((ticks / 3) % 6) as i32;
    c.fill_rect(x + (ticks as i32 * 7 % w.max(1)), bottom + s, 2, 2, BEAM_HOT);
}

/// A glowing brick wall: kick it.
fn wall(c: &mut Canvas<'_>, x: i32, w: i32, h: i32, beat: bool) {
    let top = GROUND_Y as i32 - h;
    let g = if beat { 3 } else { 2 };
    c.fill_rect(x - g, top - g, w + 2 * g, h + g, WALL_DARK);
    c.fill_rect(x, top, w, h, WALL);
    let mut row = 0;
    let mut y = top + 9;
    while y < GROUND_Y as i32 {
        c.fill_rect(x, y, w, 2, WALL_DARK);
        let off = if row % 2 == 0 { 7 } else { 15 };
        if off < w {
            c.fill_rect(x + off, y - 9, 2, 9, WALL_DARK);
        }
        y += 11;
        row += 1;
    }
    c.fill_rect(x, top, w, 3, WALL_LIGHT);
    c.fill_rect(x, top, 3, h, WALL_LIGHT);
}

/// Spinning gold bars; collected ones burst into a brief sparkle.
pub fn draw_coins(c: &mut Canvas<'_>, game: &Game) {
    const SPIN: [i32; 4] = [10, 7, 2, 7];
    for coin in game.coins.iter() {
        let cx = floor_i32(coin.x);
        let cy = floor_i32(coin.y);
        match coin.taken {
            None => {
                let frame = ((game.ticks / 6) as i32 + cx / 40).rem_euclid(4);
                let w = SPIN[frame as usize];
                let h = COIN_H as i32;
                c.fill_rect(cx - w / 2 - 1, cy - h / 2 - 1, w + 2, h + 2, GOLD_DARK);
                c.fill_rect(cx - w / 2, cy - h / 2, w, h, GOLD);
                if w > 4 {
                    c.fill_rect(cx - w / 2 + 1, cy - h / 2 + 1, 2, h - 4, GOLD_LIGHT);
                }
            }
            Some(age) => {
                let r = 4 + (age / SPARKLE_TIME * 14.0) as i32;
                for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    c.fill_rect(cx + dx * r - 1, cy + dy * r - 1, 3, 3, GOLD_LIGHT);
                }
                c.fill_rect(cx - 1, cy - 1, 3, 3, TEXT);
            }
        }
    }
}

pub fn power_color(kind: PowerKind) -> Color {
    match kind {
        PowerKind::Shield => SHIELD,
        PowerKind::Magnet => MAGNET,
        PowerKind::SlowMo => SLOWMO,
    }
}

pub fn power_letter(kind: PowerKind) -> &'static str {
    match kind {
        PowerKind::Shield => "S",
        PowerKind::Magnet => "M",
        PowerKind::SlowMo => "T",
    }
}

/// A pulsing orb with the power-up's letter.
pub fn draw_item(c: &mut Canvas<'_>, game: &Game) {
    let Some(item) = game.item else { return };
    let (x, y) = (floor_i32(item.x), floor_i32(item.y));
    let pulse = ((game.ticks / 4) % 4) as i32;
    let color = power_color(item.kind);
    ring(c, x, y, 15 + pulse, color);
    c.fill_circle(x, y, 11, color);
    c.fill_circle(x - 3, y - 3, 3, TEXT);
    crate::engine::font::draw_text(c, power_letter(item.kind), x - 4, y - 4, 1, SHADOW);
}
