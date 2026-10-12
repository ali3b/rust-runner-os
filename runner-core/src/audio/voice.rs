//! One oscillator with a decaying envelope, an optional pitch sweep and a pan.
use super::SAMPLE_RATE;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Wave {
    /// Square wave; the value is the duty cycle as a fraction of 2^32.
    Pulse(u32),
    Triangle,
    /// 15-bit LFSR noise, clocked at the voice frequency.
    Noise,
}

/// Envelope decrement per sample to fade from full to silent in `ms`.
pub const fn decay_ms(ms: u32) -> u32 {
    let samples = ms * (SAMPLE_RATE / 1000);
    let d = (u16::MAX as u32) / if samples == 0 { 1 } else { samples };
    if d == 0 { 1 } else { d }
}

pub struct Voice {
    pub wave: Wave,
    phase: u32,
    inc: u32,
    /// Envelope, 0..=65535.
    env: u32,
    decay: u32,
    /// Pitch change per sample, in millionths (2^-20) of the current pitch.
    sweep: i32,
    lfsr: u16,
    /// 0..=255.
    gain: i32,
    /// 0 = left, 128 = centre, 255 = right.
    pan: i32,
}

impl Voice {
    pub const fn new(wave: Wave, gain: i32, pan: i32) -> Self {
        Voice { wave, phase: 0, inc: 0, env: 0, decay: 0, sweep: 0, lfsr: 1, gain, pan }
    }

    pub fn trigger(&mut self, inc: u32, decay: u32, sweep: i32) {
        self.inc = inc;
        self.env = u16::MAX as u32;
        self.decay = decay;
        self.sweep = sweep;
        if self.wave != Wave::Noise {
            self.phase = 0;
        }
    }

    /// Next (left, right) sample.
    #[inline]
    pub fn next(&mut self) -> (i32, i32) {
        if self.env == 0 {
            return (0, 0);
        }
        let raw: i32 = match self.wave {
            Wave::Pulse(duty) => {
                if self.phase < duty {
                    4096
                } else {
                    -4096
                }
            }
            Wave::Triangle => {
                let p = (self.phase >> 20) as i32; // 0..4096
                if p < 2048 { p * 4 - 4096 } else { (4095 - p) * 4 - 4096 }
            }
            Wave::Noise => {
                if self.lfsr & 1 != 0 { 4096 } else { -4096 }
            }
        };
        let (next, wrapped) = self.phase.overflowing_add(self.inc);
        self.phase = next;
        if wrapped && self.wave == Wave::Noise {
            let bit = (self.lfsr ^ (self.lfsr >> 1)) & 1;
            self.lfsr = (self.lfsr >> 1) | (bit << 14);
        }
        if self.sweep != 0 {
            let delta = (self.inc as i64 * self.sweep as i64) >> 20;
            self.inc = (self.inc as i64 + delta).clamp(1 << 16, u32::MAX as i64 >> 1) as u32;
        }
        self.env = self.env.saturating_sub(self.decay);

        let s = (raw * (self.env >> 4) as i32) >> 12; // ±4096
        let s = (s * self.gain) >> 8;
        ((s * (256 - self.pan)) >> 7, (s * self.pan) >> 7)
    }
}
