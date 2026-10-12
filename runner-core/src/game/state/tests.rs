use super::*;
use crate::engine::time::STEP_SECONDS;
use crate::game::{autopilot, update_game};

const NONE: InputState = InputState {
    jump_pressed: false,
    restart_pressed: false,
    escape_pressed: false,
    slide_pressed: false,
    kick_pressed: false,
    crt_pressed: false,
};
const JUMP: InputState = InputState { jump_pressed: true, ..NONE };
const RESTART: InputState = InputState { restart_pressed: true, ..NONE };
const ESC: InputState = InputState { escape_pressed: true, ..NONE };

fn step(g: &mut Game, i: InputState) {
    update_game(g, &i, STEP_SECONDS);
}

fn run_until_crash(g: &mut Game) {
    for _ in 0..60 * 60 {
        step(g, NONE);
        if g.mode == GameMode::GameOver {
            return;
        }
    }
    panic!("never crashed");
}

fn wait(g: &mut Game, seconds: f32) {
    for _ in 0..(seconds / STEP_SECONDS) as u32 {
        step(g, NONE);
    }
}

fn started(seed: u64) -> Game {
    started_with(seed, ZONE_BARS)
}

fn started_with(seed: u64, zone_bars: u32) -> Game {
    let mut g = Game::new(seed);
    g.zone_bars = zone_bars;
    step(&mut g, JUMP);
    assert_eq!(g.mode, GameMode::Playing);
    g
}

/// Plays with the bot (optionally jittered) for `bars`, asserting survival.
fn bot_run(g: &mut Game, bars: u32, jitter: impl Fn(u32) -> i32) {
    while g.bar() < bars {
        let input = autopilot::wants_with_jitter(g, &jitter);
        step(g, input);
        assert_eq!(g.mode, GameMode::Playing, "crashed in bar {} (zone {}, lap {})", g.bar(), g.zone, g.lap);
    }
}

#[test]
fn full_cycle_title_play_crash_restart() {
    let mut g = started(1);
    run_until_crash(&mut g);
    let first = g.score;
    assert!(first > 0);
    assert_eq!(g.best, first);
    assert!(g.new_best);
    wait(&mut g, GAME_OVER_LOCKOUT);
    step(&mut g, JUMP);
    assert_eq!(g.mode, GameMode::Playing);
    assert_eq!((g.score, g.coin_count, g.level), (0, 0, 0));
    assert_eq!(g.best, first);
    assert!(g.player.grounded, "restart press must not also jump");
}

#[test]
fn r_restarts_after_lockout_and_esc_goes_to_title() {
    let mut g = started(2);
    run_until_crash(&mut g);
    step(&mut g, RESTART);
    assert_eq!(g.mode, GameMode::GameOver);
    wait(&mut g, GAME_OVER_LOCKOUT);
    step(&mut g, RESTART);
    assert_eq!(g.mode, GameMode::Playing);
    step(&mut g, ESC);
    assert_eq!(g.mode, GameMode::Title);
}

#[test]
fn game_over_freezes_the_world() {
    let mut g = started(3);
    run_until_crash(&mut g);
    let xs: Vec<f32> = g.hazards.iter().map(|h| h.x).collect();
    let score = g.score;
    wait(&mut g, 2.0);
    assert_eq!(xs, g.hazards.iter().map(|h| h.x).collect::<Vec<_>>());
    assert_eq!(g.score, score);
}

/// The core rhythm guarantee: every hazard reaches its target exactly
/// `lead_ticks` after its press tick, and press ticks sit on the 16th grid.
#[test]
fn hazards_arrive_on_the_beat() {
    let mut g = started_with(4, 6); // cross zone (speed) boundaries quickly
    let mut checked = 0;
    while g.bar() < 30 {
        let input = autopilot::wants(&g);
        step(&mut g, input);
        let now = g.tick();
        for h in g.hazards.iter().filter(|h| !h.broken) {
            assert_eq!(h.press_tick % STEP_TICKS, 0);
            let arrival = h.press_tick + h.kind.lead_ticks();
            if now == arrival {
                // In slow-mo the clock can be part-way past the tick; allow for that.
                let past = zones::travel_between(arrival as u64 * Q, g.world_q, g.zone_bars);
                let err = h.x + past - h.target_x();
                assert!(err.abs() < 0.5, "{:?} off by {err}", h.kind);
                checked += 1;
            }
        }
    }
    assert!(checked > 15, "only {checked} arrivals checked");
}

#[test]
fn perfect_bot_survives_every_zone_and_a_second_lap() {
    for seed in 1..12 {
        let mut g = started(seed);
        bot_run(&mut g, ZONE_BARS * 4 * 2 + 4, |_| 0);
        assert!(g.lap >= 2);
        assert!(g.combo.perfects > 50);
    }
}

/// Timing within ±2 updates of the beat must always be survivable.
#[test]
fn sloppy_bot_survives_too() {
    for seed in 1..12 {
        for bias in [-2, 2] {
            let mut g = started(seed);
            // Mix early, late and on-time presses.
            bot_run(&mut g, ZONE_BARS * 4 + 4, |t| match (t / STEP_TICKS) % 3 {
                0 => bias,
                1 => -bias,
                _ => 0,
            });
        }
    }
}

#[test]
fn zones_advance_with_banner_events() {
    let mut g = started_with(5, 6);
    g.take_events();
    let mut zones_seen = vec![g.zone];
    while g.bar() < 6 * 4 + 1 {
        let input = autopilot::wants(&g);
        step(&mut g, input);
        if g.take_events().has(Events::ZONE) {
            zones_seen.push(g.zone);
        }
    }
    assert_eq!(zones_seen, vec![0, 1, 2, 3, 0]);
    assert_eq!(g.lap, 1);
}

#[test]
fn walls_need_a_kick() {
    let mut g = started(6);
    // Skip the intro and force a wall straight ahead.
    let mut wall = Hazard::new(HazardKind::Wall, WALL_W, WALL_H, 0, g.tick() + 10);
    wall.x = PLAYER_X + PLAYER_W + 30.0;
    g.hazards = Hazards::new();
    g.hazards.add(wall);
    let mut kicked = g.clone();
    step(&mut kicked, InputState { kick_pressed: true, ..NONE });
    assert!(kicked.take_events().has(Events::BREAK));
    for _ in 0..60 {
        step(&mut kicked, NONE);
    }
    assert_eq!(kicked.mode, GameMode::Playing);
    // Jumping doesn't help.
    step(&mut g, JUMP);
    for _ in 0..60 {
        step(&mut g, NONE);
    }
    assert_eq!(g.mode, GameMode::GameOver);
}

#[test]
fn beams_need_a_slide() {
    let mut g = started(7);
    let mut beam = Hazard::new(HazardKind::Beam, BEAM_W, 0.0, 0, 0);
    beam.x = PLAYER_X + PLAYER_W + 10.0;
    g.hazards = Hazards::new();
    g.hazards.add(beam);
    let mut slid = g.clone();
    step(&mut slid, InputState { slide_pressed: true, ..NONE });
    for _ in 0..40 {
        step(&mut slid, NONE);
    }
    assert_eq!(slid.mode, GameMode::Playing);
    step(&mut g, JUMP);
    for _ in 0..40 {
        step(&mut g, NONE);
    }
    assert_eq!(g.mode, GameMode::GameOver);
}

#[test]
fn shield_absorbs_exactly_one_hit() {
    let mut g = started(8);
    g.shield = true;
    let block = |x| {
        let mut h = Hazard::new(HazardKind::Block, 30.0, 40.0, 0, 0);
        h.x = x;
        h
    };
    g.hazards = Hazards::new();
    g.hazards.add(block(PLAYER_X + 10.0));
    step(&mut g, NONE);
    assert!(g.take_events().has(Events::SHIELD_HIT));
    assert!(!g.shield);
    assert_eq!(g.mode, GameMode::Playing);
    wait(&mut g, SHIELD_GRACE + 0.1);
    g.hazards.add(block(PLAYER_X + 10.0));
    step(&mut g, NONE);
    assert_eq!(g.mode, GameMode::GameOver);
}

#[test]
fn slowmo_slows_world_and_music_clock_together() {
    let mut a = started(9);
    let mut b = a.clone();
    b.slowmo = 3.0;
    for _ in 0..120 {
        step(&mut a, NONE);
        step(&mut b, NONE);
    }
    let ratio = b.distance / a.distance;
    assert!((ratio - SLOWMO_SCALE_Q as f32 / Q as f32).abs() < 0.02, "distance ratio {ratio}");
    let steps_ratio = b.step as f32 / a.step as f32;
    assert!((steps_ratio - ratio).abs() < 0.1, "music clock ratio {steps_ratio}");
}

#[test]
fn perfect_timing_is_rewarded() {
    let mut g = started(10);
    bot_run(&mut g, 8, |_| 0);
    assert!(g.combo.perfects > 0);
    let mut late = started(10);
    bot_run(&mut late, 8, |_| 2);
    assert_eq!(late.combo.perfects, late.combo.perfects.min(g.combo.perfects));
    let mut sloppy = started(10);
    // Outside the ±2 window but still survivable for blocks/walls is not guaranteed,
    // so only check that on-time play earns more points than late play.
    bot_run(&mut sloppy, 8, |_| 2);
    assert!(g.points >= sloppy.points);
}

#[test]
fn coins_power_the_trail_up_to_extra() {
    let mut g = started(11);
    g.collect_coins(COINS_PER_LEVEL * 4);
    assert_eq!(g.level, MAX_LEVEL);
    assert_eq!(g.multiplier(), g.combo.multiplier() * 2, "EXTRA doubles points");
}

#[test]
fn missing_gold_breaks_the_combo() {
    let mut g = started(12);
    for _ in 0..12 {
        g.combo_hit();
    }
    assert_eq!(g.combo.multiplier(), 2);
    g.coins.add(PLAYER_X - 2.0, 50.0); // just behind the hero, out of reach
    step(&mut g, NONE);
    assert_eq!(g.combo.streak, 0);
    assert!(g.take_events().has(Events::COMBO_BREAK));
}

#[test]
fn bot_collects_power_ups() {
    let mut g = started_with(13, 6);
    let mut kinds = vec![];
    while g.bar() < 6 * 4 {
        let input = autopilot::wants(&g);
        step(&mut g, input);
        if g.take_events().has(Events::POWERUP) {
            kinds.push(g.last_power.unwrap());
        }
    }
    assert_eq!(kinds.len(), 4, "{kinds:?}");
}

#[test]
fn rainbow_dust_is_left_behind_in_the_world() {
    let mut g = started(14);
    assert_eq!(g.particles.alive(), 0, "no dust before any gold");
    g.collect_coins(COINS_PER_LEVEL * 3);
    step(&mut g, JUMP);
    for _ in 0..30 {
        step(&mut g, NONE);
    }
    let alive = g.particles.alive();
    assert!(alive > 30 * 6 / 2, "dust is shed every update ({alive})");
    // Crashing stops the shedding; the dust already dropped fades away.
    run_until_crash(&mut g);
    wait(&mut g, 3.0);
    assert_eq!(g.particles.alive(), 0);
}

#[test]
fn title_runs_the_music_clock() {
    let mut g = Game::new(15);
    let mut steps = 0;
    for _ in 0..60 {
        step(&mut g, NONE);
        if g.take_events().has(Events::STEP) {
            steps += 1;
        }
    }
    assert_eq!(steps, 10, "150 BPM 16ths = 10 per second");
}

