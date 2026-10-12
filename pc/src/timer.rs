//! Monotonic clock: the CPU timestamp counter, calibrated once against PIT channel 2.
//!
//! Channel 2 is polled through port 0x61, so no interrupts (and no IDT/PIC setup)
//! are needed. Under QEMU both the PIT and the TSC follow host real time.
use super::port::{inb, outb};
use core::sync::atomic::{AtomicU64, Ordering};

const PIT_HZ: u64 = 1_193_182;
/// ~50 ms calibration window (max 16-bit count is ~55 ms).
const CALIBRATION_COUNT: u16 = 59_659;

static TSC_HZ: AtomicU64 = AtomicU64::new(0);
static TSC_BASE: AtomicU64 = AtomicU64::new(0);

fn rdtsc() -> u64 {
    // SAFETY: rdtsc is available on every CPU we target and has no side effects.
    #[cfg(target_arch = "x86_64")]
    unsafe {
        core::arch::x86_64::_rdtsc()
    }
    #[cfg(target_arch = "x86")]
    unsafe {
        core::arch::x86::_rdtsc()
    }
}

/// Measures TSC ticks during one PIT channel-2 one-shot.
fn measure_once() -> Option<u64> {
    // SAFETY: programs PIT channel 2 (speaker gate, speaker output disabled).
    unsafe {
        let ctrl = inb(0x61) & !0x03; // gate low, speaker off
        outb(0x61, ctrl);
        outb(0x43, 0b1011_0000); // channel 2, lo/hi byte, mode 0, binary
        outb(0x42, CALIBRATION_COUNT as u8);
        outb(0x42, (CALIBRATION_COUNT >> 8) as u8);
        let start = rdtsc();
        outb(0x61, ctrl | 0x01); // gate high: start counting
        // OUT2 (bit 5) goes high at terminal count. Bound the loop in case there is no PIT.
        let mut spins: u64 = 0;
        while inb(0x61) & 0x20 == 0 {
            spins += 1;
            if spins > 50_000_000 {
                return None;
            }
        }
        let end = rdtsc();
        outb(0x61, ctrl);
        Some(end - start)
    }
}

/// Calibrates the TSC. Returns the measured frequency in Hz.
pub fn init() -> u64 {
    let mut samples = [0u64; 5];
    for s in samples.iter_mut() {
        *s = measure_once().unwrap_or_else(|| panic!("PIT channel 2 did not respond; cannot calibrate timer"));
    }
    samples.sort_unstable();
    let ticks = samples[samples.len() / 2];
    let hz = ticks * PIT_HZ / CALIBRATION_COUNT as u64;
    assert!(hz > 1_000_000, "implausible TSC frequency {hz} Hz");
    TSC_HZ.store(hz, Ordering::Relaxed);
    TSC_BASE.store(rdtsc(), Ordering::Relaxed);
    hz
}

/// Nanoseconds since [`init`].
pub fn now_ns() -> u64 {
    let hz = TSC_HZ.load(Ordering::Relaxed);
    let delta = rdtsc().saturating_sub(TSC_BASE.load(Ordering::Relaxed));
    (delta as u128 * 1_000_000_000 / hz as u128) as u64
}

/// Raw counter, used only as a seed.
pub fn entropy() -> u64 {
    rdtsc()
}
