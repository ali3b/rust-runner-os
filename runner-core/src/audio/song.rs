//! One 4-bar loop per zone, all on the same 150 BPM 16th-note grid.
use super::SAMPLE_RATE;

pub const SONG_STEPS: u32 = 64;

/// Bass rhythm: per step, `None` = rest, `Some(semitones above the root)`.
pub type BassRhythm = [Option<u8>; 16];

const O: Option<u8> = None;
const R: Option<u8> = Some(0);
const U: Option<u8> = Some(12);

pub struct Song {
    /// Bass root per bar (MIDI).
    pub roots: [u8; 4],
    /// Chord tones per bar, used by arpeggios and by notes the player triggers.
    pub chords: [[u8; 3]; 4],
    /// Lead melody, one entry per step; 0 = rest.
    pub melody: [u8; 64],
    pub bass: BassRhythm,
    /// Arpeggio note every `arp_every` steps.
    pub arp_every: u32,
}

#[rustfmt::skip]
pub static SONGS: [Song; 4] = [
    // Neon City: A minor, Am - F - C - G.
    Song {
        roots: [45, 41, 48, 43],
        chords: [[69, 72, 76], [65, 69, 72], [64, 67, 72], [67, 71, 74]],
        melody: [
            76, 0, 0, 74,  72, 0, 69, 0,  72, 0, 74, 0,  76, 0, 0, 0,
            77, 0, 76, 0,  72, 0, 0, 0,   69, 0, 72, 0,  77, 0, 76, 0,
            79, 0, 0, 76,  72, 0, 76, 0,  79, 0, 81, 0,  79, 0, 0, 0,
            74, 0, 0, 71,  67, 0, 71, 0,  74, 0, 76, 0,  74, 0, 71, 0,
        ],
        bass: [R, O, R, O, U, O, R, O, R, O, R, O, U, O, R, O],
        arp_every: 1,
    },
    // Sunset Desert: D harmonic minor, Dm - Bb - A - Dm, tresillo bass.
    Song {
        roots: [38, 46, 45, 38],
        chords: [[62, 65, 69], [62, 65, 70], [61, 64, 69], [62, 65, 69]],
        melody: [
            74, 0, 73, 74,  77, 0, 76, 0,  74, 0, 0, 70,  69, 0, 0, 0,
            70, 0, 69, 70,  74, 0, 72, 0,  70, 0, 69, 0,  65, 0, 0, 0,
            69, 0, 0, 73,   76, 0, 73, 0,  69, 0, 73, 0,  76, 0, 77, 76,
            74, 0, 0, 0,    69, 0, 72, 0,  74, 0, 0, 0,   0, 0, 0, 0,
        ],
        bass: [R, O, O, R, O, O, U, O, R, O, O, R, O, O, U, O],
        arp_every: 2,
    },
    // Deep Space: E minor, Em - C - G - D, long floating notes.
    Song {
        roots: [40, 36, 43, 38],
        chords: [[64, 67, 71], [64, 67, 72], [62, 67, 71], [62, 66, 69]],
        melody: [
            79, 0, 0, 0,  0, 0, 78, 0,  76, 0, 0, 0,  71, 0, 0, 0,
            72, 0, 0, 0,  0, 0, 74, 0,  76, 0, 0, 0,  79, 0, 0, 0,
            78, 0, 0, 0,  74, 0, 0, 0,  71, 0, 74, 0,  79, 0, 0, 0,
            78, 0, 0, 0,  76, 0, 74, 0,  74, 0, 0, 0,  0, 0, 0, 0,
        ],
        bass: [R, O, O, O, O, O, U, O, R, O, O, O, O, O, U, O],
        arp_every: 1,
    },
    // Hyper Grid: C minor synthwave, Cm - Ab - Eb - Bb, driving 16th bass.
    Song {
        roots: [36, 44, 39, 46],
        chords: [[60, 63, 67], [60, 63, 68], [58, 63, 67], [58, 62, 65]],
        melody: [
            72, 0, 75, 0,  79, 0, 75, 0,  72, 0, 75, 0,  79, 0, 80, 79,
            75, 0, 72, 0,  68, 0, 72, 0,  75, 0, 72, 0,  68, 0, 0, 0,
            70, 0, 75, 0,  79, 0, 82, 0,  79, 0, 75, 0,  70, 0, 75, 0,
            77, 0, 74, 0,  70, 0, 74, 0,  77, 0, 79, 0,  82, 0, 0, 0,
        ],
        bass: [R, R, U, R, R, R, U, R, R, R, U, R, R, R, U, R],
        arp_every: 1,
    },
];

/// Phase increment per sample (2^32 = one cycle) for each MIDI note.
const NOTE_INC: [u32; 128] = {
    let mut table = [0u32; 128];
    let semitone = 1.059_463_094_359_295_3_f64;
    let mut n = 0;
    while n < 128 {
        let mut f = 440.0f64;
        let mut k = 69;
        while k < n {
            f *= semitone;
            k += 1;
        }
        while k > n {
            f /= semitone;
            k -= 1;
        }
        table[n] = (f * 4_294_967_296.0 / SAMPLE_RATE as f64) as u32;
        n += 1;
    }
    table
};

pub fn note_inc(note: u8) -> u32 {
    NOTE_INC[note as usize & 127]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a4_is_440_hz() {
        let hz = (note_inc(69) as u64 * SAMPLE_RATE as u64) >> 32;
        assert_eq!(hz, 439); // floor of 440 after fixed-point truncation
        assert!(note_inc(81) / 2 - note_inc(69) < 2, "octave doubles");
    }

    #[test]
    fn melodies_stay_in_a_sane_range() {
        for s in SONGS.iter() {
            assert!(s.melody.iter().all(|&n| n == 0 || (55..=96).contains(&n)));
            assert!(s.chords.iter().flatten().all(|&n| (55..=84).contains(&n)));
        }
    }
}
