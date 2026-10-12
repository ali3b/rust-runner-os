//! Backgrounds for the four zones. Everything is procedural: hashed column
//! heights, circles and lines, scrolled at different parallax rates.
use super::{hash, palette::*};
use crate::engine::graphics::{Canvas, HEIGHT, WIDTH, floor_i32};
use crate::game::{state::Game, tuning::GROUND_Y};

const W: i32 = WIDTH as i32;
const HORIZON: i32 = GROUND_Y as i32;

/// Draws sky, far layer, mid layer and ground for the zone being shown.
pub fn draw(c: &mut Canvas<'_>, game: &Game, beat: bool) {
    // During the palette fade the new zone's scenery is already shown.
    sky(c, game, beat);
    match game.zone {
        0 => {
            stars(c, game, 40, 190);
            far_steps(c, game.scroll * 0.1);
            city(c, game.scroll * 0.3, game.ticks);
            checker_ground(c, game.scroll, beat);
        }
        1 => {
            sun(c, 470, HORIZON - 40, 70, game.ticks);
            mesas(c, game.scroll * 0.1);
            dunes(c, game.scroll * 0.3);
            sand_ground(c, game.scroll, beat);
        }
        2 => {
            stars(c, game, 120, 380);
            planet(c, 460 - floor_i32(game.scroll * 0.02).rem_euclid(900) / 3, 150);
            nebula(c, game.scroll * 0.06);
            asteroids(c, game.scroll * 0.3, game.ticks);
            plate_ground(c, game.scroll, beat);
        }
        _ => {
            stars(c, game, 30, 150);
            sun(c, W / 2, HORIZON - 30, 90, game.ticks);
            wire_mountains(c, game.scroll * 0.1);
            grid_ground(c, game.scroll, beat);
        }
    }
}

fn sky(c: &mut Canvas<'_>, game: &Game, beat: bool) {
    let band = HORIZON / SKY_BANDS + 1;
    for i in 0..SKY_BANDS {
        // On the beat the horizon band flashes one step brighter.
        let color = if beat && i == SKY_BANDS - 2 { SKY + SKY_BANDS as u8 - 1 } else { SKY + i as u8 };
        c.fill_rect(0, i * band, W, band, color);
    }
    let _ = game;
}

fn stars(c: &mut Canvas<'_>, game: &Game, count: i32, max_y: u32) {
    for layer in 0..2 {
        let off = floor_i32(game.scroll * if layer == 0 { 0.02 } else { 0.06 });
        for i in 0..count / 2 {
            let h = hash(i * 7 + 1 + layer * 1000);
            let x = ((h % 1024) as i32 - off).rem_euclid(W + 16) - 8;
            let y = ((h >> 10) % max_y) as i32 + 6;
            let twinkle = (game.ticks / 8 + h as u64).is_multiple_of(6);
            let size = if layer == 1 && h & 0x800_0000 != 0 { 2 } else { 1 };
            c.fill_rect(x, y, size, size, if twinkle { STAR_DIM } else { STAR });
        }
    }
}

/// Columns of `bw` px starting left of the screen for a layer scrolled by `off`.
fn columns(off: f32, bw: i32) -> impl Iterator<Item = (i32, i32)> {
    let off = floor_i32(off);
    let first = off.div_euclid(bw);
    (0..=(W / bw + 1)).map(move |k| {
        let i = first + k;
        (i, i * bw - off)
    })
}

/// Stepped "voxel" mountains (city).
fn far_steps(c: &mut Canvas<'_>, off: f32) {
    for (i, x) in columns(off, 24) {
        let ridge = (hash(i / 3) % 90 + hash(i / 3 + 1) % 90 + hash(i) % 30) as i32;
        let h = 40 + ridge / 8 * 8;
        c.fill_rect(x, HORIZON - h, 24, h, FAR);
        c.fill_rect(x, HORIZON - h, 24, 3, FAR_EDGE);
    }
}

/// Blocky city with blinking windows.
fn city(c: &mut Canvas<'_>, off: f32, ticks: u64) {
    const BW: i32 = 56;
    for (i, x0) in columns(off, BW) {
        let h = hash(i.wrapping_mul(31) + 5);
        if h.is_multiple_of(5) {
            continue;
        }
        let x = x0 + (h % 8) as i32;
        let w = 36 + (h >> 4) as i32 % 16;
        let bh = 50 + ((h >> 8) % 12) as i32 * 10;
        let top = HORIZON - bh;
        c.fill_rect(x, top, w, bh, MID);
        c.fill_rect(x, top, w, 2, MID_EDGE);
        c.fill_rect(x, top, 2, bh, MID_EDGE);
        let mut wy = top + 8;
        let mut row = 0u32;
        while wy < HORIZON - 10 {
            let mut wx = x + 6;
            let mut col = 0u32;
            while wx + 4 < x + w - 4 {
                let wh = hash((i << 8) ^ (row << 4 | col) as i32);
                if !wh.is_multiple_of(3) {
                    let lit = !(ticks / 45 + wh as u64).is_multiple_of(7);
                    c.fill_rect(wx, wy, 4, 5, if lit { WINDOW } else { WINDOW_DIM });
                }
                wx += 9;
                col += 1;
            }
            wy += 12;
            row += 1;
        }
    }
}

/// A striped retro sun, half sunk below `cy`.
fn sun(c: &mut Canvas<'_>, cx: i32, cy: i32, r: i32, ticks: u64) {
    for dy in -r..=0 {
        let half = crate::engine::graphics::isqrt((r * r - dy * dy) as u32) as i32;
        let y = cy + dy;
        // Horizontal cut-outs that get thicker towards the bottom, drifting down.
        let depth = dy + r; // 0 at the top
        let gap = depth / 12;
        let phase = ((ticks / 4) as i32 + depth) % 12;
        if depth > r / 3 && phase < gap {
            continue;
        }
        let color = if depth < r / 2 { SUN_A } else { SUN_B };
        c.fill_rect(cx - half, y, 2 * half + 1, 1, color);
    }
}

/// Flat-topped plateaus (desert far layer).
fn mesas(c: &mut Canvas<'_>, off: f32) {
    for (i, x) in columns(off, 16) {
        let group = i.div_euclid(6);
        let g = hash(group * 13 + 3);
        if g.is_multiple_of(3) {
            continue;
        }
        let pos = i.rem_euclid(6);
        let h = 50 + (g % 60) as i32;
        // Sloped sides: the outer columns are lower.
        let h = match pos {
            0 | 5 => h - 30,
            1 | 4 => h - 8,
            _ => h,
        };
        if h > 0 {
            c.fill_rect(x, HORIZON - h, 16, h, FAR);
            c.fill_rect(x, HORIZON - h, 16, 3, FAR_EDGE);
        }
    }
}

/// Rolling dunes with the odd cactus (desert mid layer).
fn dunes(c: &mut Canvas<'_>, off: f32) {
    const BW: i32 = 8;
    for (i, x) in columns(off, BW) {
        // Smooth heights: blend two hashed control points with a quadratic ease.
        let seg = i.div_euclid(12);
        let t = i.rem_euclid(12);
        let a = (hash(seg * 7 + 1) % 50) as i32;
        let b = (hash(seg * 7 + 8) % 50) as i32;
        let ease = t * t * (36 - 2 * t) / 144; // smoothstep scaled to 0..=12
        let h = 14 + a + (b - a) * ease.clamp(0, 12) / 12;
        c.fill_rect(x, HORIZON - h, BW, h, MID);
        c.fill_rect(x, HORIZON - h, BW, 2, MID_EDGE);
        if hash(i * 3 + 11).is_multiple_of(37) {
            let top = HORIZON - h - 34;
            c.fill_rect(x + 2, top, 6, 34, WINDOW);
            c.fill_rect(x - 4, top + 10, 6, 4, WINDOW);
            c.fill_rect(x - 4, top + 4, 4, 8, WINDOW);
            c.fill_rect(x + 8, top + 14, 6, 4, WINDOW);
            c.fill_rect(x + 10, top + 6, 4, 10, WINDOW);
            c.fill_rect(x + 3, top, 2, 34, WINDOW_DIM);
        }
    }
}

/// A ringed planet.
fn planet(c: &mut Canvas<'_>, cx: i32, cy: i32) {
    let r = 64;
    c.fill_rect(cx - r - 46, cy + 4, 2 * r + 92, 5, RING);
    c.fill_circle(cx, cy, r, PLANET);
    // Shadowed crescent.
    for dy in -r..=r {
        let half = crate::engine::graphics::isqrt((r * r - dy * dy) as u32) as i32;
        let shade = half / 3;
        c.fill_rect(cx + half - shade, cy + dy, shade + 1, 1, PLANET_DARK);
    }
    // Bands.
    c.fill_rect(cx - r + 8, cy - 20, 2 * r - 16, 6, PLANET_DARK);
    c.fill_rect(cx - r + 4, cy + 18, 2 * r - 8, 4, PLANET_DARK);
    // Front half of the ring.
    c.fill_rect(cx - r - 46, cy + 4, 46, 5, RING);
    c.fill_rect(cx + r, cy + 4, 46, 5, RING);
    c.fill_rect(cx - r, cy + 6, 2 * r, 3, RING);
}

/// Soft nebula clouds (space far layer).
fn nebula(c: &mut Canvas<'_>, off: f32) {
    for (i, x) in columns(off, 90) {
        let h = hash(i * 17 + 4);
        if h.is_multiple_of(2) {
            continue;
        }
        let y = 230 + (h % 120) as i32;
        let r = 24 + (h >> 8) as i32 % 30;
        c.fill_circle(x, y, r, FAR);
        c.fill_circle(x + r / 2, y - r / 3, r * 2 / 3, FAR_EDGE);
        c.fill_circle(x - r / 2, y + r / 4, r / 2, FAR);
    }
}

/// Tumbling asteroids (space mid layer).
fn asteroids(c: &mut Canvas<'_>, off: f32, ticks: u64) {
    for (i, x) in columns(off, 110) {
        let h = hash(i * 29 + 9);
        if h.is_multiple_of(3) {
            continue;
        }
        let bob = ((ticks / 10 + h as u64) % 8) as i32;
        let bob = if bob > 4 { 8 - bob } else { bob };
        let y = 250 + (h % 110) as i32 + bob;
        let r = 8 + (h >> 6) as i32 % 14;
        c.fill_circle(x, y, r, MID);
        c.fill_circle(x - r / 3, y - r / 3, r / 3, MID_EDGE);
        c.fill_circle(x + r / 3, y + r / 4, r / 4, WINDOW);
    }
}

/// Wireframe mountain range (grid far layer): filled triangles with neon edges.
fn wire_mountains(c: &mut Canvas<'_>, off: f32) {
    let off = floor_i32(off);
    let period = 160;
    let first = off.div_euclid(period) - 1;
    for k in 0..(W / period + 3) {
        let i = first + k;
        let h = hash(i * 5 + 2);
        let peak_x = i * period - off + (h % 60) as i32;
        let peak_h = 70 + (h >> 8) as i32 % 90;
        let half_w = peak_h + 40;
        for dx in -half_w..=half_w {
            let col_h = peak_h - (dx.abs() * peak_h / half_w);
            if col_h <= 0 {
                continue;
            }
            let x = peak_x + dx;
            c.fill_rect(x, HORIZON - col_h, 1, col_h, FAR);
            c.fill_rect(x, HORIZON - col_h, 1, 2, FAR_EDGE);
        }
    }
}

fn ground_glow(c: &mut Canvas<'_>, beat: bool) {
    let glow = if beat { 6 } else { 3 };
    c.fill_rect(0, HORIZON, W, glow, GROUND_GLOW);
    c.fill_rect(0, HORIZON, W, 1, GROUND_EDGE);
}

fn checker_ground(c: &mut Canvas<'_>, scroll: f32, beat: bool) {
    const TW: i32 = 32;
    const TH: i32 = 18;
    for row in 0..((HEIGHT as i32 - HORIZON) / TH + 1) {
        for (i, x) in columns(scroll, TW) {
            let color = if (i + row) % 2 == 0 { GROUND_A } else { GROUND_B };
            c.fill_rect(x, HORIZON + row * TH, TW, TH, color);
        }
    }
    ground_glow(c, beat);
}

fn sand_ground(c: &mut Canvas<'_>, scroll: f32, beat: bool) {
    c.fill_rect(0, HORIZON, W, HEIGHT as i32 - HORIZON, GROUND_A);
    for row in 0..6 {
        let y = HORIZON + 10 + row * 12;
        let off = floor_i32(scroll * (1.0 + row as f32 * 0.1));
        for (i, x) in columns(off as f32, 40) {
            if !hash(i * 3 + row).is_multiple_of(3) {
                c.fill_rect(x, y, 22, 3, GROUND_B);
            }
        }
    }
    ground_glow(c, beat);
}

fn plate_ground(c: &mut Canvas<'_>, scroll: f32, beat: bool) {
    const TW: i32 = 64;
    c.fill_rect(0, HORIZON, W, HEIGHT as i32 - HORIZON, GROUND_B);
    for (_, x) in columns(scroll, TW) {
        c.fill_rect(x + 2, HORIZON + 6, TW - 4, 30, GROUND_A);
        c.fill_rect(x + 2, HORIZON + 40, TW - 4, 30, GROUND_A);
        for (rx, ry) in [(6, 10), (TW - 8, 10), (6, 44), (TW - 8, 44)] {
            c.fill_rect(x + rx, HORIZON + ry, 2, 2, MID_EDGE);
        }
    }
    ground_glow(c, beat);
}

/// Synthwave perspective grid.
fn grid_ground(c: &mut Canvas<'_>, scroll: f32, beat: bool) {
    let depth = HEIGHT as i32 - HORIZON;
    c.fill_rect(0, HORIZON, W, depth, GROUND_A);
    // Horizontal lines bunch up towards the horizon and stream towards the viewer.
    const LINES: i32 = 8;
    let phase = (floor_i32(scroll) & 63) * 256 / 64; // 0..256 per 64 px of travel
    for k in 0..LINES {
        let z = k * 256 + phase; // 0 .. LINES*256
        let y = HORIZON + depth * z / (LINES * 256) * z / (LINES * 256);
        c.fill_rect(0, y, W, 1, GRID);
    }
    // Lines converging on the vanishing point.
    let vx = W / 2;
    let shift = floor_i32(scroll).rem_euclid(64);
    let mut xb = -W - shift;
    while xb < 2 * W {
        for dy in 0..depth {
            let x = vx + (xb - vx) * dy / depth;
            c.fill_rect(x, HORIZON + dy, 1, 1, GRID);
        }
        xb += 64;
    }
    ground_glow(c, beat);
}
