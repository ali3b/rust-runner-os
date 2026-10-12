//! Palette layout and per-zone colour schemes. Zone colours fade into each other
//! on zone changes, and EXTRA mode cycles the sky through the rainbow.
use super::super::{state::Game, tuning::MAX_LEVEL, zones::ZONES};
use crate::engine::graphics::{Color, Palette, Rgb};

// Zone-dependent entries.
pub const SKY: Color = 1; // 1..=8, top to horizon
pub const SKY_BANDS: i32 = 8;
pub const FAR: Color = 9;
pub const FAR_EDGE: Color = 10;
pub const MID: Color = 11;
pub const MID_EDGE: Color = 12;
pub const WINDOW: Color = 13;
pub const WINDOW_DIM: Color = 14;
pub const GROUND_A: Color = 15;
pub const GROUND_B: Color = 16;
pub const GROUND_GLOW: Color = 17;
pub const SUN_A: Color = 57;
pub const SUN_B: Color = 58;
pub const PLANET: Color = 59;
pub const PLANET_DARK: Color = 60;
pub const RING: Color = 61;
pub const GRID: Color = 62;
// Fixed entries.
pub const GROUND_EDGE: Color = 18;
pub const HERO: Color = 19;
pub const HERO_EYE: Color = 20;
pub const TRAIL: Color = 21; // 21..=26 rainbow
pub const BLOCK: Color = 27; // 27..=30 fill per style
pub const BLOCK_SHADE: Color = 31; // 31..=34
pub const BLOCK_SHINE: Color = 35;
pub const TEXT: Color = 36;
pub const SHADOW: Color = 37;
pub const ACCENT: Color = 38;
pub const FLASH: Color = 39;
pub const STAR: Color = 40;
pub const STAR_DIM: Color = 41;
pub const PANEL: Color = 42;
pub const PANEL_EDGE: Color = 43;
pub const GOLD: Color = 44;
pub const GOLD_DARK: Color = 45;
pub const GOLD_LIGHT: Color = 46;
pub const WALL: Color = 47;
pub const WALL_DARK: Color = 48;
pub const WALL_LIGHT: Color = 49;
pub const SHIELD: Color = 50;
pub const MAGNET: Color = 51;
pub const SLOWMO: Color = 52;
pub const BEAM_METAL: Color = 53;
pub const BEAM_DARK: Color = 54;
pub const BEAM_GLOW: Color = 55;
pub const BEAM_HOT: Color = 56;
pub const SPEEDLINE: Color = 63;
#[cfg_attr(not(test), allow(dead_code))]
pub const LAST: Color = SPEEDLINE;

struct ZonePalette {
    sky: [u32; 8],
    far: u32,
    far_edge: u32,
    mid: u32,
    mid_edge: u32,
    window: u32,
    window_dim: u32,
    ground_a: u32,
    ground_b: u32,
    glow: u32,
    sun_a: u32,
    sun_b: u32,
    planet: u32,
    planet_dark: u32,
    ring: u32,
    grid: u32,
}

static ZONE_PALETTES: [ZonePalette; 4] = [
    // Neon City
    ZonePalette {
        sky: [0x120a2e, 0x1e0f45, 0x2e1560, 0x46207a, 0x642c8c, 0x8a3896, 0xb44890, 0xe0627f],
        far: 0x34195e,
        far_edge: 0x5a3190,
        mid: 0x1b1d4f,
        mid_edge: 0x3a49a8,
        window: 0x46f0ff,
        window_dim: 0x2a6a9a,
        ground_a: 0x2f1650,
        ground_b: 0x22103c,
        glow: 0xff4fd8,
        sun_a: 0xffe14d,
        sun_b: 0xff3f8f,
        planet: 0xe0607a,
        planet_dark: 0x8a2a5a,
        ring: 0xffc0e0,
        grid: 0x46f0ff,
    },
    // Sunset Desert
    ZonePalette {
        sky: [0x2a0f3d, 0x4a1650, 0x7a1f5c, 0xb32f5a, 0xe0504a, 0xf27a3a, 0xf8a23a, 0xfcd05a],
        far: 0x7a2f45,
        far_edge: 0xa8475a,
        mid: 0xc2603a,
        mid_edge: 0xeb9450,
        window: 0x2f7a3a,
        window_dim: 0x1f5a2a,
        ground_a: 0xd9944a,
        ground_b: 0xc4803c,
        glow: 0xffe24a,
        sun_a: 0xfff07a,
        sun_b: 0xff8a3a,
        planet: 0xe0607a,
        planet_dark: 0x8a2a5a,
        ring: 0xffc0e0,
        grid: 0xffe24a,
    },
    // Deep Space
    ZonePalette {
        sky: [0x020208, 0x04040f, 0x070718, 0x0a0a22, 0x0e0e2e, 0x14143c, 0x1a1a4a, 0x22225a],
        far: 0x2a1458,
        far_edge: 0x5a2a9a,
        mid: 0x3a3a52,
        mid_edge: 0x75759a,
        window: 0x24243a,
        window_dim: 0x55557a,
        ground_a: 0x2a2f45,
        ground_b: 0x1f2335,
        glow: 0x4af0ff,
        sun_a: 0xffffff,
        sun_b: 0xaaccff,
        planet: 0xe0607a,
        planet_dark: 0x7a2050,
        ring: 0xffc0e0,
        grid: 0x4af0ff,
    },
    // Hyper Grid
    ZonePalette {
        sky: [0x0a0020, 0x150033, 0x22004a, 0x330060, 0x4a0a75, 0x6a1585, 0x922090, 0xc02a95],
        far: 0x1a0045,
        far_edge: 0xff2fd0,
        mid: 0x0c0026,
        mid_edge: 0x00f0ff,
        window: 0xff2fd0,
        window_dim: 0x7a1070,
        ground_a: 0x0a0018,
        ground_b: 0x0c001e,
        glow: 0xff2fd0,
        sun_a: 0xffe14d,
        sun_b: 0xff3f8f,
        planet: 0xe0607a,
        planet_dark: 0x8a2a5a,
        ring: 0xffc0e0,
        grid: 0x00f0ff,
    },
];

const RAINBOW: [u32; 6] = [0xff3355, 0xff9933, 0xffee33, 0x44ff66, 0x33bbff, 0xaa55ff];

fn base_palette() -> Palette {
    let mut p = [Rgb::default(); 256];
    let mut set = |i: Color, v: u32| p[i as usize] = Rgb::hex(v);
    set(GROUND_EDGE, 0xffffff);
    set(HERO, 0x0b0710);
    set(HERO_EYE, 0xffffff);
    for (i, &c) in RAINBOW.iter().enumerate() {
        set(TRAIL + i as Color, c);
    }
    let fill = [0xffcc1a, 0xff4f7b, 0x2cf5c8, 0xff8a1f];
    let shade = [0xb36b00, 0xa3204a, 0x138f7a, 0xa84a00];
    for i in 0..4 {
        set(BLOCK + i as Color, fill[i]);
        set(BLOCK_SHADE + i as Color, shade[i]);
    }
    set(BLOCK_SHINE, 0xfff6d8);
    set(TEXT, 0xffffff);
    set(SHADOW, 0x1a0a33);
    set(ACCENT, 0xffe14d);
    set(FLASH, 0xffffff);
    set(STAR, 0xffffff);
    set(STAR_DIM, 0x8877cc);
    set(PANEL, 0x140a2a);
    set(PANEL_EDGE, 0xff4fd8);
    set(GOLD, 0xffcf33);
    set(GOLD_DARK, 0xa8700a);
    set(GOLD_LIGHT, 0xfff6b8);
    set(WALL, 0x3ad6ff);
    set(WALL_DARK, 0x1a5f9a);
    set(WALL_LIGHT, 0xd8f8ff);
    set(SHIELD, 0x5ff8ff);
    set(MAGNET, 0xff4a5a);
    set(SLOWMO, 0x8a7aff);
    set(BEAM_METAL, 0x4a4a62);
    set(BEAM_DARK, 0x24243a);
    set(BEAM_GLOW, 0xff2a5a);
    set(BEAM_HOT, 0xffd0e0);
    set(SPEEDLINE, 0xe8e8ff);
    p
}

fn lerp(a: u32, b: u32, t: u32) -> Rgb {
    let (a, b) = (Rgb::hex(a), Rgb::hex(b));
    let mix = |x: u8, y: u8| ((x as u32 * (256 - t) + y as u32 * t) >> 8) as u8;
    Rgb { r: mix(a.r, b.r), g: mix(a.g, b.g), b: mix(a.b, b.b) }
}

/// The palette for the current frame.
pub fn palette(game: &Game) -> Palette {
    let mut p = base_palette();
    let (from, to) = (&ZONE_PALETTES[game.prev_zone % ZONES.len()], &ZONE_PALETTES[game.zone % ZONES.len()]);
    // Fade over the first second of a zone.
    let t = if game.zone_time >= 1.0 { 256 } else { (game.zone_time * 256.0) as u32 };
    let mut set = |i: Color, a: u32, b: u32| p[i as usize] = lerp(a, b, t);
    for k in 0..8 {
        set(SKY + k as Color, from.sky[k], to.sky[k]);
    }
    set(FAR, from.far, to.far);
    set(FAR_EDGE, from.far_edge, to.far_edge);
    set(MID, from.mid, to.mid);
    set(MID_EDGE, from.mid_edge, to.mid_edge);
    set(WINDOW, from.window, to.window);
    set(WINDOW_DIM, from.window_dim, to.window_dim);
    set(GROUND_A, from.ground_a, to.ground_a);
    set(GROUND_B, from.ground_b, to.ground_b);
    set(GROUND_GLOW, from.glow, to.glow);
    set(SUN_A, from.sun_a, to.sun_a);
    set(SUN_B, from.sun_b, to.sun_b);
    set(PLANET, from.planet, to.planet);
    set(PLANET_DARK, from.planet_dark, to.planet_dark);
    set(RING, from.ring, to.ring);
    set(GRID, from.grid, to.grid);

    if game.level >= MAX_LEVEL && game.mode != super::super::GameMode::Title {
        // EXTRA: the sky becomes a rolling rainbow, half-blended with the zone sky.
        let phase = (game.ticks / 3) as usize;
        for k in 0..8 {
            let zone_sky = p[SKY as usize + k];
            let zone = (zone_sky.r as u32) << 16 | (zone_sky.g as u32) << 8 | zone_sky.b as u32;
            p[SKY as usize + k] = lerp(zone, RAINBOW[(k + phase) % RAINBOW.len()], 110);
        }
    }
    p
}
