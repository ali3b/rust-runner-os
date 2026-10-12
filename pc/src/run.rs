//! The game loop shared by every machine build: poll audio and keyboard, run
//! fixed 60 Hz updates, render, present. Each kernel supplies a [`Screen`] and
//! (optionally) an [`AudioOut`].
use crate::{keyboard::Keyboard, log, timer};
use runner_core::{
    audio::{Scene, Synth},
    engine::{
        graphics::{Canvas, HEIGHT, Palette, WIDTH},
        input::InputLatch,
        time::{FixedStep, STEP_SECONDS},
    },
    game::{Events, Game, GameMode, palette, render_game, update_game, zones::ZONES},
};

/// Where frames go.
pub trait Screen {
    fn set_palette(&mut self, palette: &Palette);
    /// Shows a `WIDTH * HEIGHT` frame of palette indices.
    fn present(&mut self, frame: &[u8], crt: bool);
}

/// A sound card fed by polling.
pub trait AudioOut {
    /// Tops up the card's buffers with `fill`. Returns whether anything was filled.
    fn poll(&mut self, fill: &mut dyn FnMut(&mut [i16])) -> bool;
    fn underruns(&self) -> u32;
}

/// The logical frame, 300 KiB; kept in .bss rather than on the stack.
static mut BACK_BUFFER: [u8; WIDTH * HEIGHT] = [0; WIDTH * HEIGHT];

/// Runs the game forever. `synth` is the one the audio card was primed with.
pub fn run(screen: &mut dyn Screen, mut audio: Option<&mut dyn AudioOut>, mut synth: Synth) -> ! {
    let mut keyboard = Keyboard::new();
    let mut latch = InputLatch::new();
    #[allow(unused_mut)]
    let mut game = Game::new(timer::entropy());
    let mut clock = FixedStep::new();
    #[cfg(feature = "autoplay")]
    let mut demo = demo::Demo::new(&mut game);
    screen.set_palette(&palette(&game));
    let mut palette_version = game.palette_version;
    let mut crt = game.crt;

    // SAFETY: the only reference ever created to BACK_BUFFER; we're single-threaded.
    #[allow(clippy::deref_addrof)] // `&raw` avoids the 2024 `static_mut_refs` error
    let back = unsafe { &mut *(&raw mut BACK_BUFFER) };
    let mut canvas = Canvas::new(back);
    log!("state: {}", game.mode.name());

    let mut frames: u64 = 0;
    let mut fps_window_start = timer::now_ns();
    // Time spent per stage in the current fps window, for the serial perf line.
    let mut spent = [0u64; 4]; // audio, update, render, present
    loop {
        let t0 = timer::now_ns();
        if let Some(a) = audio.as_deref_mut()
            && a.poll(&mut |buf| synth.render(buf))
        {
            spent[0] += timer::now_ns() - t0; // only real refills, not idle polls
        }
        keyboard.poll(&mut latch);
        let steps = clock.advance(timer::now_ns());
        if steps == 0 {
            core::hint::spin_loop();
            continue;
        }
        let t1 = timer::now_ns();
        for _ in 0..steps {
            let before = game.mode;
            #[allow(unused_mut)]
            let mut input = latch.take();
            #[cfg(feature = "autoplay")]
            demo.drive(&game, &mut input);
            update_game(&mut game, &input, STEP_SECONDS);
            if game.mode != before {
                log!("state: {} score={} best={} gold={}", game.mode.name(), game.score, game.best, game.coin_count);
            }
            let events = game.take_events();
            log_events(&game, events);
            synth.handle(events, game.combo.streak);
            if events.has(Events::STEP) {
                synth.on_step(game.step, Scene { mode: game.mode, zone: game.zone, level: game.level });
            }
        }
        if game.palette_version != palette_version {
            palette_version = game.palette_version;
            screen.set_palette(&palette(&game));
        }
        if game.crt != crt {
            crt = game.crt;
            // The browser page draws its scanlines itself and listens for this.
            log!("crt: {}", crt as u8);
        }
        let t2 = timer::now_ns();
        render_game(&game, &mut canvas);
        let t3 = timer::now_ns();
        screen.present(canvas.pixels(), game.crt);
        let t4 = timer::now_ns();
        spent[1] += t2 - t1;
        spent[2] += t3 - t2;
        spent[3] += t4 - t3;

        frames += 1;
        let now = timer::now_ns();
        if now - fps_window_start >= 5_000_000_000 {
            let underruns = audio.as_deref().map_or(0, |a| a.underruns());
            let us = |ns: u64| ns / frames / 1000;
            log!(
                "fps: {} audio_underruns: {} us/frame: audio {} update {} render {} present {}",
                frames * 1_000_000_000 / (now - fps_window_start),
                underruns,
                us(spent[0]),
                us(spent[1]),
                us(spent[2]),
                us(spent[3])
            );
            frames = 0;
            spent = [0; 4];
            fps_window_start = now;
        }
    }
}

/// Serial lines for the smoke test and the video captions.
fn log_events(game: &Game, ev: Events) {
    if ev.has(Events::LEVEL_UP) {
        log!("level: {} gold={}", game.level, game.coin_count);
    }
    if ev.has(Events::ZONE) && game.mode == GameMode::Playing && game.bar() > 0 {
        log!("zone: {} {} lap={}", game.zone + 1, ZONES[game.zone].name, game.lap + 1);
    }
    if ev.has(Events::POWERUP)
        && let Some(p) = game.last_power {
            log!("powerup: {}", p.name());
        }
    if ev.has(Events::SHIELD_HIT) {
        log!("shield: saved");
    }
    if ev.has(Events::COMBO_UP) {
        log!("combo: x{} streak={}", game.combo.multiplier(), game.combo.streak);
    }
    if ev.has(Events::SLIDE) {
        log!("action: slide");
    }
    if ev.has(Events::BREAK) {
        log!("action: kick wall");
    } else if ev.has(Events::KICK) {
        log!("action: kick");
    }
}

#[cfg(feature = "autoplay")]
mod demo {
    use runner_core::{
        engine::input::InputState,
        game::{Game, GameMode, autopilot},
    };

    /// Short zones so one recording shows all four.
    const DEMO_ZONE_BARS: u32 = 6;
    /// Run 1 plays perfectly until this long into the last zone, then stops
    /// pressing; later runs give up after this many bars. So the recording shows
    /// every zone, then a crash, a retry and another crash.
    const LAST_ZONE_SECONDS: f32 = 5.0;
    const LATER_RUNS_BARS: u32 = 5;

    pub struct Demo {
        runs: usize,
        was_playing: bool,
    }

    impl Demo {
        pub fn new(game: &mut Game) -> Self {
            game.zone_bars = DEMO_ZONE_BARS;
            crate::log!("autoplay: bot plays all moves, {DEMO_ZONE_BARS}-bar zones");
            Demo { runs: 0, was_playing: false }
        }

        pub fn drive(&mut self, game: &Game, input: &mut InputState) {
            let playing = game.mode == GameMode::Playing;
            if playing && !self.was_playing {
                self.runs += 1;
            }
            self.was_playing = playing;
            let keep_going = if self.runs <= 1 {
                !(game.zone == 3 && game.zone_time > LAST_ZONE_SECONDS)
            } else {
                game.bar() < LATER_RUNS_BARS
            };
            if playing && keep_going {
                let bot = autopilot::wants(game);
                input.jump_pressed |= bot.jump_pressed;
                input.slide_pressed |= bot.slide_pressed;
                input.kick_pressed |= bot.kick_pressed;
            }
        }
    }
}

