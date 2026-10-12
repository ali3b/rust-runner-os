use super::{
    combo::{Combo, is_perfect},
    hazards::{Hazard, HazardKind, Hazards},
    patterns::Thing,
    pickups::{Coins, Item, PowerKind},
    player::Player,
    tuning::*,
    zones::{self, BAR_Q, ZONES},
};
use crate::engine::{
    input::InputState,
    particles::Particles,
    rng::Rng,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameMode {
    Title,
    Playing,
    GameOver,
}

impl GameMode {
    pub fn name(self) -> &'static str {
        match self {
            GameMode::Title => "Title",
            GameMode::Playing => "Playing",
            GameMode::GameOver => "GameOver",
        }
    }
}

/// Things that happened during an update, for audio, logging and effects. Bit flags.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Events(pub u32);

impl Events {
    pub const START: u32 = 1 << 0;
    pub const JUMP: u32 = 1 << 1;
    pub const COIN: u32 = 1 << 2;
    pub const LEVEL_UP: u32 = 1 << 3;
    pub const CRASH: u32 = 1 << 4;
    pub const TITLE: u32 = 1 << 5;
    /// The music clock moved to a new 16th note.
    pub const STEP: u32 = 1 << 6;
    pub const SLIDE: u32 = 1 << 7;
    pub const KICK: u32 = 1 << 8;
    /// A kick broke a wall.
    pub const BREAK: u32 = 1 << 9;
    pub const PERFECT: u32 = 1 << 10;
    pub const POWERUP: u32 = 1 << 11;
    pub const ZONE: u32 = 1 << 12;
    pub const SHIELD_HIT: u32 = 1 << 13;
    pub const COMBO_UP: u32 = 1 << 14;
    pub const COMBO_BREAK: u32 = 1 << 15;
    pub const LAND: u32 = 1 << 16;

    pub fn has(self, flag: u32) -> bool {
        self.0 & flag != 0
    }
}

/// Floating text near the hero.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PopupKind {
    Perfect,
    Points(u32),
    Combo(u32),
    Power(PowerKind),
    Shield,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Popup {
    pub kind: PopupKind,
    pub x: f32,
    pub y: f32,
    pub age: f32,
}

pub const POPUP_TIME: f32 = 0.8;
const MAX_POPUPS: usize = 8;

/// Rainbow stripes (offsets into the rainbow palette) the hero sheds as dust at
/// each trail level; level 4 is EXTRA.
pub const DUST_BY_LEVEL: [&[u8]; 5] = [&[], &[2], &[1, 2, 3], &[0, 1, 2, 3, 4, 5], &[0, 1, 2, 3, 4, 5]];
const RAINBOW_BASE: u8 = 21;

// Particle looks. Colors are palette indices owned by the renderer.
pub mod fx {
    use crate::engine::particles::Burst;
    pub const GOLD: [u8; 2] = [44, 46];
    pub const RAINBOW: [u8; 6] = [21, 22, 23, 24, 25, 26];
    pub const WALL: [u8; 3] = [47, 48, 49];
    pub const DUST: [u8; 2] = [18, 36];
    pub const SHIELD_COLORS: [u8; 2] = [50, 36];
    pub const COIN: Burst = Burst { count: 6, speed: 2, life: 18, size: 2, gravity: false, colors: &GOLD, push_x: 0 };
    pub const BREAK: Burst = Burst { count: 28, speed: 4, life: 40, size: 4, gravity: true, colors: &WALL, push_x: 3 };
    pub const CRASH: Burst = Burst { count: 60, speed: 5, life: 60, size: 3, gravity: true, colors: &RAINBOW, push_x: 0 };
    pub const LAND: Burst = Burst { count: 8, speed: 1, life: 14, size: 2, gravity: false, colors: &DUST, push_x: -1 };
    pub const SHIELD: Burst = Burst { count: 40, speed: 4, life: 36, size: 3, gravity: true, colors: &SHIELD_COLORS, push_x: 2 };
    pub const POWER: Burst = Burst { count: 24, speed: 3, life: 30, size: 2, gravity: false, colors: &RAINBOW, push_x: 0 };
}

#[derive(Clone, Debug)]
pub struct Game {
    pub mode: GameMode,
    pub player: Player,
    pub hazards: Hazards,
    pub coins: Coins,
    pub item: Option<Item>,
    pub particles: Particles,
    pub popups: [Option<Popup>; MAX_POPUPS],
    pub combo: Combo,
    pub rng: Rng,
    /// Bars per zone (16 normally; the demo build uses fewer).
    pub zone_bars: u32,

    /// World time in 1/Q updates since the run (or title) started.
    pub world_q: u64,
    /// Current 16th-note step of the music clock.
    pub step: u32,
    next_spawn_bar: u32,
    pub zone: usize,
    pub lap: u32,
    /// Seconds since entering the current zone (drives banners and palette fades).
    pub zone_time: f32,
    pub prev_zone: usize,
    pub speed: f32,

    pub distance: f32,
    /// Free-running scroll position for the background (also moves on the title).
    pub scroll: f32,
    /// Bonus points from gold, clears and perfects (already multiplied).
    pub points: u32,
    pub score: u32,
    pub best: u32,
    pub new_best: bool,
    pub coin_count: u32,
    /// Trail level 0..=MAX_LEVEL (4 = EXTRA).
    pub level: u8,
    pub level_time: f32,

    pub shield: bool,
    pub shield_grace: f32,
    pub magnet: f32,
    pub slowmo: f32,
    pub last_power: Option<PowerKind>,

    /// Screen shake time left, seconds.
    pub shake: f32,
    pub crt: bool,
    /// Playing with on-screen touch buttons (changes the on-screen hints).
    pub touch: bool,
    /// Bumped whenever `palette()` would return something new.
    pub palette_version: u32,
    pub mode_time: f32,
    /// Real updates since boot; animation clock and RNG entropy.
    pub ticks: u64,
    events: Events,
}

const RUN_Y: f32 = GROUND_Y - PLAYER_H / 2.0;
const SLIDE_Y: f32 = GROUND_Y - SLIDE_H / 2.0;

impl Game {
    pub fn new(seed: u64) -> Self {
        Self {
            mode: GameMode::Title,
            player: Player::new(),
            hazards: Hazards::new(),
            coins: Coins::new(),
            item: None,
            particles: Particles::new(),
            popups: [None; MAX_POPUPS],
            combo: Combo::default(),
            rng: Rng::new(seed),
            zone_bars: ZONE_BARS,
            world_q: 0,
            step: 0,
            next_spawn_bar: 0,
            zone: 0,
            lap: 0,
            zone_time: 0.0,
            prev_zone: 0,
            speed: ZONES[0].speed,
            distance: 0.0,
            scroll: 0.0,
            points: 0,
            score: 0,
            best: 0,
            new_best: false,
            coin_count: 0,
            level: 0,
            level_time: f32::MAX,
            shield: false,
            shield_grace: 0.0,
            magnet: 0.0,
            slowmo: 0.0,
            last_power: None,
            shake: 0.0,
            crt: true,
            touch: false,
            palette_version: 0,
            mode_time: 0.0,
            ticks: 0,
            events: Events::default(),
        }
    }

    pub fn take_events(&mut self) -> Events {
        core::mem::take(&mut self.events)
    }

    fn emit(&mut self, flag: u32) {
        self.events.0 |= flag;
    }

    /// Current world update (integer).
    pub fn tick(&self) -> u32 {
        (self.world_q / Q) as u32
    }

    pub fn bar(&self) -> u32 {
        (self.world_q / BAR_Q) as u32
    }

    pub fn scroll_speed(&self) -> f32 {
        match self.mode {
            GameMode::Title => TITLE_SCROLL_SPEED,
            GameMode::Playing => self.speed * self.time_scale_q() as f32 / Q as f32,
            GameMode::GameOver => 0.0,
        }
    }

    /// World time per real update, in 1/Q units.
    pub fn time_scale_q(&self) -> u64 {
        if self.slowmo > 0.0 { SLOWMO_SCALE_Q } else { Q }
    }

    /// Combo multiplier, doubled in EXTRA mode.
    pub fn multiplier(&self) -> u32 {
        self.combo.multiplier() * if self.level >= MAX_LEVEL { 2 } else { 1 }
    }

    // ---- rainbow dust -----------------------------------------------------------

    /// Trail level shown: the title shows off the full rainbow.
    pub fn dust_level(&self) -> u8 {
        if self.mode == GameMode::Title { MAX_LEVEL - 1 } else { self.level }
    }

    /// The hero sheds rainbow dust into the world: a few specks per update in
    /// rainbow bands down their back. The specks stay where they were dropped and
    /// scroll away with the scenery, so a jump leaves its arc hanging in the air.
    fn shed_dust(&mut self) {
        let level = self.dust_level();
        let stripes = DUST_BY_LEVEL[level as usize];
        if stripes.is_empty() {
            return;
        }
        let extra = level >= MAX_LEVEL;
        use crate::engine::particles::FP;
        let body = self.player.body();
        let band = 3 * FP;
        let top = (body.y + body.h / 2.0) as i32 * FP - stripes.len() as i32 * band / 2;
        for (i, &s) in stripes.iter().enumerate() {
            let jitter = |rng: &mut Rng, span: i32| rng.range(0, (2 * span) as u32) as i32 - span;
            let x = (PLAYER_X as i32 + 2) * FP + jitter(&mut self.rng, 3 * FP);
            let y = top + i as i32 * band + jitter(&mut self.rng, FP);
            // Barely drifting: a hint of backwards motion and float.
            let vx = -(self.rng.range(0, 6) as i32);
            let vy = jitter(&mut self.rng, 3) - 1;
            let life = 50 + self.rng.range(0, 30) as u16 + if extra { 30 } else { 0 };
            let size = if extra { 4 } else { 3 };
            self.particles.spawn(x, y, vx, vy, life, RAINBOW_BASE + s, size, false);
        }
        if extra && self.rng.range(0, 2) == 0 {
            // EXTRA throws off bright sparkles too.
            let x = (PLAYER_X as i32) * FP;
            let y = (body.y as i32 + self.rng.range(0, body.h as u32) as i32) * FP;
            let vx = -(self.rng.range(8, 30) as i32);
            let vy = self.rng.range(0, 40) as i32 - 20;
            self.particles.spawn(x, y, vx, vy, 40, 46, 2, false);
        }
    }

    // ---- mode changes ---------------------------------------------------------

    fn set_mode(&mut self, mode: GameMode) {
        self.mode = mode;
        self.mode_time = 0.0;
    }

    fn reset_run(&mut self) {
        self.player = Player::new();
        self.hazards = Hazards::new();
        self.coins = Coins::new();
        self.item = None;
        self.popups = [None; MAX_POPUPS];
        self.combo = Combo::default();
        self.world_q = 0;
        self.step = 0;
        self.next_spawn_bar = 0;
        self.zone = 0;
        self.prev_zone = 0;
        self.lap = 0;
        self.zone_time = 0.0;
        self.speed = zones::speed_of_bar(0, self.zone_bars);
        self.distance = 0.0;
        self.points = 0;
        self.score = 0;
        self.new_best = false;
        self.coin_count = 0;
        self.level = 0;
        self.level_time = f32::MAX;
        self.shield = false;
        self.shield_grace = 0.0;
        self.magnet = 0.0;
        self.slowmo = 0.0;
        self.palette_version = self.palette_version.wrapping_add(1);
    }

    fn start_run(&mut self) {
        self.reset_run();
        self.particles.clear();
        self.rng.mix(self.ticks);
        self.set_mode(GameMode::Playing);
        self.emit(Events::START | Events::STEP | Events::ZONE);
        while self.next_spawn_bar <= LOOKAHEAD_BARS {
            self.spawn_bar(self.next_spawn_bar);
            self.next_spawn_bar += 1;
        }
    }

    fn return_to_title(&mut self) {
        self.reset_run();
        self.set_mode(GameMode::Title);
        self.emit(Events::TITLE | Events::STEP);
    }

    fn crash(&mut self) {
        let score = self.score;
        if score > self.best {
            self.best = score;
            self.new_best = true;
        }
        let (x, y) = (player_center_x() as i32, (self.player.y + PLAYER_H / 2.0) as i32);
        self.particles.burst(x, y, &fx::CRASH, &mut self.rng);
        self.shake = 0.5;
        self.set_mode(GameMode::GameOver);
        self.emit(Events::CRASH);
    }

    // ---- spawning -------------------------------------------------------------

    /// x (left edge or centre, as the caller defines) at which something that must
    /// reach `target_x` at world update `tick` has to be placed now.
    fn x_for(&self, target_x: f32, tick: u32) -> f32 {
        target_x + zones::travel_between(self.world_q, tick as u64 * Q, self.zone_bars)
    }

    fn spawn_bar(&mut self, bar: u32) {
        let zb = self.zone_bars;
        if zones::is_empty_bar(bar, zb) {
            return;
        }
        let info = zones::bar_info(bar, zb);
        let pool = zones::pool(info.zone);
        let pattern = pool[self.rng.range(0, pool.len() as u32 - 1) as usize];
        let bar_tick = bar * BAR_TICKS;
        for &(step, thing) in pattern {
            let press = bar_tick + step as u32 * STEP_TICKS;
            match thing {
                Thing::Gold(n) => {
                    for k in 0..n as u32 {
                        let x = self.x_for(player_center_x(), press + k * STEP_TICKS);
                        self.coins.add(x, RUN_Y);
                    }
                }
                _ => {
                    let kind = thing.hazard().unwrap_or(HazardKind::Block);
                    let (w, h) = match kind {
                        HazardKind::Block => (
                            self.rng.range(BLOCK_MIN_W, BLOCK_MAX_W) as f32,
                            self.rng.range(BLOCK_MIN_H, BLOCK_MAX_H) as f32,
                        ),
                        HazardKind::Beam => (BEAM_W, BEAM_BOTTOM - BEAM_TOP),
                        HazardKind::Wall => (WALL_W, WALL_H),
                    };
                    let style = self.rng.range(0, 3) as u8;
                    let mut hz = Hazard::new(kind, w, h, style, press);
                    hz.x = self.x_for(hz.target_x(), press + kind.lead_ticks());
                    self.hazards.add(hz);
                    match thing {
                        Thing::Jump { arc: true } => {
                            for f in [0.2, 0.35, 0.5, 0.65, 0.8] {
                                let dt = (JUMP_TICKS as f32 * f) as u32;
                                let t = dt as f32 / UPDATES_PER_SECOND as f32;
                                let x = self.x_for(player_center_x(), press + dt);
                                self.coins.add(x, RUN_Y - jump_height(t));
                            }
                        }
                        Thing::Slide { gold: true } => {
                            for k in 1..=3 {
                                let x = self.x_for(player_center_x(), press + k * STEP_TICKS);
                                self.coins.add(x, SLIDE_Y);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        if let Some(kind) = zones::power_for_bar(bar, zb) {
            let x = self.x_for(player_center_x(), bar_tick + 15 * STEP_TICKS);
            self.item = Some(Item { kind, x, y: RUN_Y - 4.0 });
        }
    }

    // ---- scoring helpers --------------------------------------------------------

    fn add_points(&mut self, base: u32) {
        self.points += base * self.multiplier();
    }

    fn combo_hit(&mut self) {
        if let Some(m) = self.combo.hit() {
            self.emit(Events::COMBO_UP);
            self.popup(PopupKind::Combo(m));
        }
    }

    fn combo_break(&mut self) {
        if self.combo.reset() {
            self.emit(Events::COMBO_BREAK);
        }
    }

    fn popup(&mut self, kind: PopupKind) {
        // Stack on top of popups that are still fresh, so they don't overlap.
        let fresh = self.popups.iter().flatten().filter(|p| p.age < POPUP_TIME / 2.0).count();
        let p = Popup { kind, x: player_center_x(), y: self.player.y - 14.0 - 22.0 * fresh as f32, age: 0.0 };
        let slot = self.popups.iter().position(|p| p.is_none()).unwrap_or(0);
        self.popups[slot] = Some(p);
    }

    /// Judges the timing of a move against the nearest hazard of the matching kind.
    fn judge(&mut self, kind: HazardKind) {
        let now = self.tick();
        let target = self
            .hazards
            .iter_mut()
            .filter(|h| h.kind == kind && !h.judged && !h.broken)
            .min_by_key(|h| h.press_tick.abs_diff(now));
        if let Some(h) = target
            && h.press_tick.abs_diff(now) <= BEAT_TICKS / 2 {
                h.judged = true;
                if is_perfect(now, h.press_tick) {
                    self.combo.perfects += 1;
                    self.add_points(PERFECT_POINTS);
                    self.popup(PopupKind::Perfect);
                    self.emit(Events::PERFECT);
                }
            }
    }

    fn collect_coins(&mut self, n: u32) {
        for _ in 0..n {
            self.coin_count += 1;
            self.add_points(COIN_POINTS);
            self.combo_hit();
        }
        if n > 0 {
            self.emit(Events::COIN);
            let level = (self.coin_count / COINS_PER_LEVEL).min(MAX_LEVEL as u32) as u8;
            if level > self.level {
                self.level = level;
                self.level_time = 0.0;
                self.emit(Events::LEVEL_UP);
                self.palette_version = self.palette_version.wrapping_add(1);
            }
        }
    }

    fn activate(&mut self, kind: PowerKind) {
        match kind {
            PowerKind::Shield => self.shield = true,
            PowerKind::Magnet => self.magnet = MAGNET_TIME,
            PowerKind::SlowMo => self.slowmo = SLOWMO_TIME,
        }
        self.last_power = Some(kind);
        self.popup(PopupKind::Power(kind));
        self.emit(Events::POWERUP);
        let (x, y) = (player_center_x() as i32, self.player.y as i32);
        self.particles.burst(x, y, &fx::POWER, &mut self.rng);
    }

    // ---- the run --------------------------------------------------------------

    fn update_playing(&mut self, input: &InputState, dt: f32) {
        if input.escape_pressed {
            self.return_to_title();
            return;
        }
        self.magnet = (self.magnet - dt).max(0.0);
        self.slowmo = (self.slowmo - dt).max(0.0);
        self.shield_grace = (self.shield_grace - dt).max(0.0);
        self.level_time = (self.level_time + dt).min(1e6);
        let scale_q = self.time_scale_q();
        let dt_world = dt * scale_q as f32 / Q as f32;

        // Moves.
        if input.jump_pressed && self.player.jump() {
            self.emit(Events::JUMP);
            self.judge(HazardKind::Block);
        }
        if input.slide_pressed && self.player.slide() {
            self.emit(Events::SLIDE);
            self.judge(HazardKind::Beam);
        }
        if input.kick_pressed && self.player.kick() {
            self.emit(Events::KICK);
            self.judge(HazardKind::Wall);
        }
        self.player.update(dt_world, scale_q);
        if self.player.just_landed {
            self.emit(Events::LAND);
            self.particles.burst(player_center_x() as i32, GROUND_Y as i32, &fx::LAND, &mut self.rng);
        }

        // World clock: scroll exactly as far as the speed schedule says.
        let old_q = self.world_q;
        self.world_q += scale_q;
        let travel = zones::travel_between(old_q, self.world_q, self.zone_bars);
        self.distance += travel;
        self.scroll += travel;
        self.hazards.scroll(travel);
        if let Some(item) = &mut self.item {
            item.x -= travel;
        }
        let step = self.tick() / STEP_TICKS;
        if step != self.step {
            self.step = step;
            self.emit(Events::STEP);
        }
        let bar = self.bar();
        self.speed = zones::speed_of_bar(bar, self.zone_bars);
        while self.next_spawn_bar <= bar + LOOKAHEAD_BARS {
            self.spawn_bar(self.next_spawn_bar);
            self.next_spawn_bar += 1;
        }
        let info = zones::bar_info(bar, self.zone_bars);
        if info.zone != self.zone || info.lap != self.lap {
            self.prev_zone = self.zone;
            self.zone = info.zone;
            self.lap = info.lap;
            self.zone_time = 0.0;
            self.emit(Events::ZONE);
        }
        self.zone_time += dt;

        // Kicks break walls in reach.
        if let Some(zone) = self.player.kick_zone() {
            let mut broke = 0;
            for h in self.hazards.iter_mut() {
                if h.kind == HazardKind::Wall && h.bounds().is_some_and(|b| b.intersects(&zone)) {
                    h.broken = true;
                    h.cleared = true;
                    broke += 1;
                    let (x, y) = ((h.x + h.w / 2.0) as i32, (GROUND_Y - h.h / 2.0) as i32);
                    self.particles.burst(x, y, &fx::BREAK, &mut self.rng);
                }
            }
            if broke > 0 {
                self.emit(Events::BREAK);
                self.shake = self.shake.max(0.12);
                for _ in 0..broke {
                    self.add_points(CLEAR_POINTS);
                    self.combo_hit();
                }
            }
        }

        // Collisions.
        let hitbox = self.player.hitbox();
        let hit = self.hazards.iter().position(|h| h.bounds().is_some_and(|b| b.intersects(&hitbox)));
        if let Some(i) = hit {
            if self.shield_grace > 0.0 {
                // Still invulnerable from the last shield hit.
            } else if self.shield {
                self.shield = false;
                self.shield_grace = SHIELD_GRACE;
                if let Some(h) = self.hazards.slots.iter_mut().flatten().nth(i) {
                    h.broken = true;
                    let (x, y) = ((h.x + h.w / 2.0) as i32, (GROUND_Y - 30.0) as i32);
                    self.particles.burst(x, y, &fx::SHIELD, &mut self.rng);
                }
                self.shake = 0.25;
                self.combo_break();
                self.popup(PopupKind::Shield);
                self.emit(Events::SHIELD_HIT);
            } else {
                self.score = (self.distance / DISTANCE_PER_POINT) as u32 + self.points;
                self.crash();
                return;
            }
        }

        // Hazards safely passed.
        let mut cleared = 0;
        for h in self.hazards.iter_mut() {
            if !h.cleared && !h.broken && h.x + h.w < hitbox.x {
                h.cleared = true;
                cleared += 1;
            }
        }
        for _ in 0..cleared {
            self.add_points(CLEAR_POINTS);
            self.combo_hit();
        }

        // Gold and power-ups.
        let body = self.player.body();
        let r = self.coins.update(dt, travel, &body, self.magnet > 0.0);
        if r.collected > 0 {
            let (x, y) = (player_center_x() as i32, body.y as i32);
            self.particles.burst(x, y, &fx::COIN, &mut self.rng);
        }
        self.collect_coins(r.collected);
        if r.missed > 0 {
            self.combo_break();
        }
        if let Some(item) = self.item {
            if item.bounds().intersects(&body) {
                self.item = None;
                self.activate(item.kind);
            } else if item.x < -40.0 {
                self.item = None;
            }
        }

        self.score = (self.distance / DISTANCE_PER_POINT) as u32 + self.points;
    }
}

/// Advances the game by one fixed step of `dt` seconds (real time).
pub fn update_game(game: &mut Game, input: &InputState, dt: f32) {
    game.ticks = game.ticks.wrapping_add(1);
    game.mode_time += dt;
    game.shake = (game.shake - dt).max(0.0);
    if input.crt_pressed {
        game.crt = !game.crt;
    }
    // Particles live in the world: they scroll with it (fixed point, no drift).
    let scroll_fp = (game.scroll_speed() * dt * crate::engine::particles::FP as f32) as i32;
    game.particles.update(scroll_fp);
    for slot in game.popups.iter_mut() {
        if let Some(p) = slot {
            p.age += dt;
            p.y -= 30.0 * dt;
            if p.age > POPUP_TIME {
                *slot = None;
            }
        }
    }

    match game.mode {
        GameMode::Title => {
            game.scroll += TITLE_SCROLL_SPEED * dt;
            game.zone_time += dt;
            game.world_q += Q;
            let step = game.tick() / STEP_TICKS;
            if step != game.step {
                game.step = step;
                game.emit(Events::STEP);
            }
            if input.jump_pressed {
                game.start_run();
            }
        }
        GameMode::Playing => game.update_playing(input, dt),
        GameMode::GameOver => {
            if input.escape_pressed {
                game.return_to_title();
            } else if (input.jump_pressed || input.restart_pressed) && game.mode_time >= GAME_OVER_LOCKOUT {
                game.start_run();
            }
        }
    }
    if game.mode != GameMode::GameOver {
        game.shed_dust();
    }
    // Palette animates while fading between zones and in EXTRA mode.
    if game.zone_time < 1.0 || game.level >= MAX_LEVEL {
        game.palette_version = game.palette_version.wrapping_add(1);
    }
}

#[cfg(test)]
mod tests;
