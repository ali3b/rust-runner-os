//! Tiny integer-only chiptune synthesiser. The game is the clock: the kernel calls
//! [`Synth::on_step`] on every 16th note of the game's music clock, so the music
//! stays locked to the hazards (and slows down with slow-mo). Each zone has its
//! own song, layers build with the trail level, and the player's actions play
//! notes from the current chord.
//!
//! Everything is fixed-point (the kernel target has no hardware floats), and the
//! output is interleaved stereo `i16` at [`SAMPLE_RATE`].
mod song;
mod voice;

use crate::game::{Events, GameMode};
use song::*;
use voice::{Voice, Wave, decay_ms};

pub const SAMPLE_RATE: u32 = 48_000;

/// What the music should be doing, from the game state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Scene {
    pub mode: GameMode,
    pub zone: usize,
    /// Trail level 0..=4.
    pub level: u8,
}

/// A note the effect voice will play after `delay` samples.
#[derive(Clone, Copy)]
struct Pending {
    delay: u32,
    note: u8,
    wave: Wave,
    decay: u32,
    sweep: i32,
}

const HALF: u32 = 1 << 31;
const QUARTER: u32 = 1 << 30;
const EIGHTH: u32 = 1 << 29;

pub struct Synth {
    scene: Scene,
    /// Song bar of the last step, for chord-aware effects.
    bar: usize,
    kick: Voice,
    snare: Voice,
    hat: Voice,
    bass: Voice,
    arp: Voice,
    lead: Voice,
    harmony: Voice,
    sparkle: Voice,
    sfx: Voice,
    sfx_noise: Voice,
    pending: [Option<Pending>; 8],
}

impl Default for Synth {
    fn default() -> Self {
        Self::new()
    }
}

impl Synth {
    pub fn new() -> Self {
        Synth {
            scene: Scene { mode: GameMode::GameOver, zone: 0, level: 0 },
            bar: 0,
            kick: Voice::new(Wave::Triangle, 255, 128),
            snare: Voice::new(Wave::Noise, 110, 128),
            hat: Voice::new(Wave::Noise, 45, 160),
            bass: Voice::new(Wave::Triangle, 220, 128),
            arp: Voice::new(Wave::Pulse(QUARTER), 55, 90),
            lead: Voice::new(Wave::Pulse(HALF), 75, 150),
            harmony: Voice::new(Wave::Pulse(EIGHTH), 38, 190),
            sparkle: Voice::new(Wave::Pulse(QUARTER), 30, 60),
            sfx: Voice::new(Wave::Pulse(QUARTER), 105, 128),
            sfx_noise: Voice::new(Wave::Noise, 140, 128),
            pending: [None; 8],
        }
    }

    fn song(&self) -> &'static Song {
        &SONGS[self.scene.zone % SONGS.len()]
    }

    fn chord(&self) -> [u8; 3] {
        self.song().chords[self.bar % 4]
    }

    /// Plays the notes for music step `step` (16th notes since the run or title began).
    pub fn on_step(&mut self, step: u32, scene: Scene) {
        self.scene = scene;
        let (drums, level) = match scene.mode {
            GameMode::GameOver => return,
            GameMode::Title => (false, 1),
            GameMode::Playing => (true, scene.level),
        };
        let song = self.song();
        let s = step % SONG_STEPS;
        let beat_step = s % 16;
        self.bar = (s / 16) as usize;
        let root = song.roots[self.bar];
        let chord = song.chords[self.bar];

        if let Some(up) = song.bass[beat_step as usize] {
            self.bass.trigger(note_inc(root + up), decay_ms(180), 0);
        }
        if drums {
            if beat_step.is_multiple_of(4) {
                self.kick.trigger(note_inc(52), decay_ms(160), -260);
            }
            if beat_step == 4 || beat_step == 12 {
                self.snare.trigger(note_inc(96), decay_ms(130), 0);
            }
            let hat = match level {
                0 => false,
                1 | 2 => beat_step % 4 == 2,
                3 => beat_step % 2 == 1 || beat_step % 4 == 2,
                _ => true,
            };
            if hat {
                self.hat.trigger(note_inc(118), decay_ms(30), 0);
            }
        }
        if level >= 1 && s.is_multiple_of(song.arp_every) {
            let octave = if (s / song.arp_every) % 6 >= 3 { 12 } else { 0 };
            let n = chord[((s / song.arp_every) % 3) as usize] + octave;
            self.arp.trigger(note_inc(n), decay_ms(80), 0);
        }
        if level >= 2 {
            let n = song.melody[s as usize];
            if n != 0 {
                self.lead.trigger(note_inc(n), decay_ms(240), 0);
                if level >= 3 {
                    self.harmony.trigger(note_inc(n + 12), decay_ms(180), 0);
                }
            }
        }
        if level >= 4 {
            // EXTRA: a glittering two-octaves-up arpeggio on every 16th.
            let n = chord[(s % 3) as usize] + 24;
            self.sparkle.trigger(note_inc(n), decay_ms(50), 0);
        }
    }

    /// Triggers sound effects for the events of one update. `streak` is the combo
    /// streak, which makes consecutive gold climb up the chord.
    pub fn handle(&mut self, ev: Events, streak: u32) {
        if ev.has(Events::CRASH) {
            self.pending = [None; 8];
            self.sfx_noise.trigger(note_inc(100), decay_ms(600), 0);
            self.play(0, 57, Wave::Pulse(HALF), 700, -40);
            self.play(9_000, 50, Wave::Pulse(HALF), 900, -30);
            return;
        }
        let chord = self.chord();
        if ev.has(Events::ZONE) && !ev.has(Events::START) {
            self.sfx_noise.trigger(note_inc(110), decay_ms(900), -8);
            for (i, n) in [chord[0], chord[1], chord[2], chord[0] + 12, chord[1] + 12, chord[2] + 12].into_iter().enumerate() {
                self.play(i as u32 * 1_800, n, Wave::Pulse(QUARTER), 220, 0);
            }
        }
        if ev.has(Events::START) {
            self.play(0, 72, Wave::Pulse(QUARTER), 90, 0);
            self.play(4_000, 79, Wave::Pulse(QUARTER), 90, 0);
            self.play(8_000, 84, Wave::Pulse(QUARTER), 160, 0);
        }
        if ev.has(Events::SHIELD_HIT) {
            self.sfx_noise.trigger(note_inc(104), decay_ms(300), 0);
            self.play(0, 96, Wave::Pulse(EIGHTH), 400, -60);
        }
        if ev.has(Events::POWERUP) {
            for (i, n) in [chord[0], chord[1], chord[2], chord[0] + 12].into_iter().enumerate() {
                self.play(i as u32 * 2_000, n + 12, Wave::Pulse(EIGHTH), 140, 0);
            }
        } else if ev.has(Events::LEVEL_UP) || ev.has(Events::COMBO_UP) {
            for (i, n) in [chord[0], chord[1], chord[2], chord[0] + 12, chord[1] + 12].into_iter().enumerate() {
                self.play(i as u32 * 2_000, n + 12, Wave::Pulse(QUARTER), 120, 0);
            }
        } else if ev.has(Events::PERFECT) {
            self.play(0, chord[2] + 24, Wave::Pulse(EIGHTH), 140, 0);
            self.play(1_500, chord[0] + 36, Wave::Pulse(EIGHTH), 200, 0);
        } else if ev.has(Events::COIN) {
            // Each gold in a streak climbs one chord tone.
            let k = (streak % 9) as usize;
            let n = chord[k % 3] + 12 * (1 + (k / 3) as u8);
            self.play(0, n, Wave::Pulse(QUARTER), 60, 0);
            self.play(2_000, n + 5, Wave::Pulse(QUARTER), 120, 0);
        } else if ev.has(Events::JUMP) {
            self.play(0, chord[0] + 12, Wave::Pulse(QUARTER), 150, 70);
        }
        if ev.has(Events::BREAK) {
            self.sfx_noise.trigger(note_inc(92), decay_ms(260), -20);
            self.kick.trigger(note_inc(48), decay_ms(140), -300);
        } else if ev.has(Events::KICK) {
            self.sfx_noise.trigger(note_inc(105), decay_ms(70), 0);
        }
        if ev.has(Events::SLIDE) {
            self.sfx_noise.trigger(note_inc(112), decay_ms(320), -30);
        }
    }

    fn play(&mut self, delay: u32, note: u8, wave: Wave, ms: u32, sweep: i32) {
        let p = Pending { delay, note, wave, decay: decay_ms(ms), sweep };
        if delay == 0 {
            self.start(p);
        } else if let Some(slot) = self.pending.iter_mut().find(|s| s.is_none()) {
            *slot = Some(p);
        }
    }

    fn start(&mut self, p: Pending) {
        self.sfx.wave = p.wave;
        self.sfx.trigger(note_inc(p.note), p.decay, p.sweep);
    }

    /// Fills `out` (interleaved stereo) with the next samples.
    pub fn render(&mut self, out: &mut [i16]) {
        let master: i32 = match self.scene.mode {
            GameMode::Title => 400,
            _ => 520,
        };
        for frame in out.as_chunks_mut::<2>().0 {
            for i in 0..self.pending.len() {
                if let Some(p) = &mut self.pending[i] {
                    p.delay -= 1;
                    if p.delay == 0 {
                        let p = *p;
                        self.pending[i] = None;
                        self.start(p);
                    }
                }
            }
            let (mut l, mut r) = (0i32, 0i32);
            for v in [
                &mut self.kick,
                &mut self.snare,
                &mut self.hat,
                &mut self.bass,
                &mut self.arp,
                &mut self.lead,
                &mut self.harmony,
                &mut self.sparkle,
                &mut self.sfx,
                &mut self.sfx_noise,
            ] {
                let (vl, vr) = v.next();
                l += vl;
                r += vr;
            }
            frame[0] = ((l * master) >> 8).clamp(i16::MIN as i32, i16::MAX as i32) as i16;
            frame[1] = ((r * master) >> 8).clamp(i16::MIN as i32, i16::MAX as i32) as i16;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Samples per 16th note at 150 BPM.
    const STEP_SAMPLES: usize = (SAMPLE_RATE / 10) as usize;

    fn scene(mode: GameMode, zone: usize, level: u8) -> Scene {
        Scene { mode, zone, level }
    }

    /// Plays `steps` music steps and returns (mean |sample|, peak).
    fn play_steps(s: &mut Synth, sc: Scene, steps: u32) -> (u64, i16) {
        let mut buf = vec![0i16; STEP_SAMPLES * 2];
        let (mut sum, mut peak, mut n) = (0u64, 0i16, 0u64);
        for step in 0..steps {
            s.on_step(step, sc);
            s.render(&mut buf);
            sum += buf.iter().map(|&x| (x as i64).unsigned_abs()).sum::<u64>();
            peak = peak.max(buf.iter().map(|x| x.saturating_abs()).max().unwrap());
            n += buf.len() as u64;
        }
        (sum / n, peak)
    }

    fn silence(s: &mut Synth, samples: usize) -> u64 {
        let mut buf = vec![0i16; samples * 2];
        s.render(&mut buf);
        buf.iter().map(|&x| (x as i64).unsigned_abs()).sum::<u64>()
    }

    #[test]
    fn silent_without_steps() {
        let mut s = Synth::new();
        assert_eq!(silence(&mut s, 48_000), 0);
    }

    #[test]
    fn every_zone_plays_and_layers_build_up() {
        for zone in 0..4 {
            let mut levels = vec![];
            for level in 0..=4 {
                let mut s = Synth::new();
                levels.push(play_steps(&mut s, scene(GameMode::Playing, zone, level), 64).0);
            }
            assert!(levels[0] > 0, "zone {zone}");
            for w in levels.windows(2) {
                assert!(w[1] > w[0], "zone {zone} levels {levels:?}");
            }
        }
    }

    #[test]
    fn game_over_goes_quiet_after_the_crash() {
        let mut s = Synth::new();
        play_steps(&mut s, scene(GameMode::Playing, 0, 2), 16);
        s.handle(Events(Events::CRASH), 0);
        s.on_step(16, scene(GameMode::GameOver, 0, 2));
        assert!(silence(&mut s, 24_000) > 0, "crash is audible");
        silence(&mut s, 48_000 * 3);
        assert_eq!(silence(&mut s, 48_000), 0);
    }

    #[test]
    fn every_effect_is_audible() {
        for ev in [
            Events::JUMP,
            Events::SLIDE,
            Events::KICK,
            Events::BREAK,
            Events::COIN,
            Events::LEVEL_UP,
            Events::START,
            Events::PERFECT,
            Events::POWERUP,
            Events::ZONE,
            Events::SHIELD_HIT,
            Events::COMBO_UP,
        ] {
            let mut s = Synth::new();
            s.handle(Events(ev), 3);
            assert!(silence(&mut s, 48_000) > 0, "event {ev:#x}");
        }
    }

    #[test]
    fn full_mix_with_effects_rarely_clips() {
        let mut s = Synth::new();
        let mut buf = vec![0i16; STEP_SAMPLES * 2];
        let (mut clipped, mut total) = (0, 0);
        for step in 0..640 {
            s.on_step(step, scene(GameMode::Playing, (step / 64) as usize % 4, 4));
            if step % 3 == 0 {
                s.handle(Events(Events::COIN | Events::JUMP | Events::KICK), step);
            }
            s.render(&mut buf);
            clipped += buf.iter().filter(|&&x| x == i16::MAX || x == i16::MIN).count();
            total += buf.len();
        }
        assert!(clipped * 1000 < total, "{clipped} of {total} samples clipped");
    }

    #[test]
    fn deterministic_for_the_same_steps() {
        let run = || {
            let mut s = Synth::new();
            s.handle(Events(Events::JUMP), 0);
            let mut out = vec![];
            for step in 0..8 {
                s.on_step(step, scene(GameMode::Playing, 1, 2));
                let mut buf = vec![0i16; STEP_SAMPLES * 2];
                s.render(&mut buf);
                out.extend(buf);
            }
            out
        };
        assert_eq!(run(), run());
    }
}
