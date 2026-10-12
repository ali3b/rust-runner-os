//! The game compiled straight to WebAssembly: no emulator, no kernel. The same
//! `runner-core` as the bootable version, driven by `www/main.js`, which forwards
//! keys, calls [`frame`] every animation frame, blits [`pixels_ptr`] to a canvas
//! and pulls audio from [`audio_render`].
//!
//! The exports use plain C ABI types so no bindings generator is needed.
use runner_core::{
    audio::{SAMPLE_RATE, Scene, Synth},
    engine::{
        graphics::{Canvas, HEIGHT, WIDTH},
        input::{InputLatch, Key},
        time::{FixedStep, STEP_SECONDS},
    },
    game::{Events, Game, palette, render_game, update_game},
};
use std::cell::RefCell;

struct Web {
    game: Game,
    synth: Synth,
    latch: InputLatch,
    clock: FixedStep,
    indexed: Vec<u8>,
    /// RGBA8 frame for `ImageData`.
    rgba: Vec<u8>,
    /// Palette as little-endian RGBA words.
    lut: [u32; 256],
    palette_version: u32,
    audio: Vec<i16>,
}

thread_local! {
    static WEB: RefCell<Option<Web>> = const { RefCell::new(None) };
}

fn with<R>(f: impl FnOnce(&mut Web) -> R) -> R {
    WEB.with(|w| f(w.borrow_mut().as_mut().expect("init() not called")))
}

impl Web {
    fn refresh_palette(&mut self) {
        for (slot, c) in self.lut.iter_mut().zip(palette(&self.game).iter()) {
            *slot = u32::from_le_bytes([c.r, c.g, c.b, 255]);
        }
        self.palette_version = self.game.palette_version;
    }
}

/// Starts a fresh game. `seed` should be something like `Date.now()`.
#[unsafe(no_mangle)]
pub extern "C" fn init(seed: f64) {
    let mut w = Web {
        game: Game::new(seed as u64),
        synth: Synth::new(),
        latch: InputLatch::new(),
        clock: FixedStep::new(),
        indexed: vec![0; WIDTH * HEIGHT],
        rgba: vec![0; WIDTH * HEIGHT * 4],
        lut: [0; 256],
        palette_version: 0,
        audio: Vec::new(),
    };
    w.refresh_palette();
    WEB.with(|cell| *cell.borrow_mut() = Some(w));
}

/// Key ids: 0 jump, 1 restart, 2 escape, 3 slide, 4 kick, 5 CRT toggle.
#[unsafe(no_mangle)]
pub extern "C" fn key(id: u32, down: u32) {
    let key = match id {
        0 => Key::Jump,
        1 => Key::Restart,
        2 => Key::Escape,
        3 => Key::Slide,
        4 => Key::Kick,
        5 => Key::Crt,
        _ => return,
    };
    with(|w| w.latch.key_event(key, down != 0));
}

/// Advances the game to `now_ms` (e.g. `performance.now()`) in fixed 1/60 s steps
/// and renders. Returns the number of updates run (0 = frame unchanged).
#[unsafe(no_mangle)]
pub extern "C" fn frame(now_ms: f64) -> u32 {
    with(|w| {
        let steps = w.clock.advance((now_ms * 1_000_000.0) as u64);
        for _ in 0..steps {
            let input = w.latch.take();
            update_game(&mut w.game, &input, STEP_SECONDS);
            let ev = w.game.take_events();
            w.synth.handle(ev, w.game.combo.streak);
            if ev.has(Events::STEP) {
                let g = &w.game;
                w.synth.on_step(g.step, Scene { mode: g.mode, zone: g.zone, level: g.level });
            }
        }
        if steps > 0 {
            if w.game.palette_version != w.palette_version {
                w.refresh_palette();
            }
            render_game(&w.game, &mut Canvas::new(&mut w.indexed));
            for (dst, &c) in w.rgba.as_chunks_mut::<4>().0.iter_mut().zip(&w.indexed) {
                *dst = w.lut[c as usize].to_le_bytes();
            }
        }
        steps
    })
}

/// The RGBA frame, `width() * height() * 4` bytes.
#[unsafe(no_mangle)]
pub extern "C" fn pixels_ptr() -> *const u8 {
    with(|w| w.rgba.as_ptr())
}

#[unsafe(no_mangle)]
pub extern "C" fn width() -> u32 {
    WIDTH as u32
}

#[unsafe(no_mangle)]
pub extern "C" fn height() -> u32 {
    HEIGHT as u32
}

#[unsafe(no_mangle)]
pub extern "C" fn sample_rate() -> u32 {
    SAMPLE_RATE
}

/// Renders `frames` stereo frames and returns a pointer to `frames * 2`
/// interleaved `i16` samples (valid until the next call).
#[unsafe(no_mangle)]
pub extern "C" fn audio_render(frames: u32) -> *const i16 {
    with(|w| {
        w.audio.resize(frames as usize * 2, 0);
        w.synth.render(&mut w.audio);
        w.audio.as_ptr()
    })
}

/// Switches the game's on-screen hints to touch wording (1) or keyboard (0).
#[unsafe(no_mangle)]
pub extern "C" fn set_touch(on: u32) {
    with(|w| w.game.touch = on != 0);
}

/// Whether the CRT scanline overlay should be shown.
#[unsafe(no_mangle)]
pub extern "C" fn crt() -> u32 {
    with(|w| w.game.crt as u32)
}

/// 0 title, 1 playing, 2 game over (for tests and page chrome).
#[unsafe(no_mangle)]
pub extern "C" fn mode() -> u32 {
    with(|w| w.game.mode as u32)
}

#[unsafe(no_mangle)]
pub extern "C" fn score() -> u32 {
    with(|w| w.game.score)
}
