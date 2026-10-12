//! PC hardware drivers and the game loop shared by the bootable builds: the
//! 64-bit `kernel` (bootloader crate, QEMU) and the 32-bit `kernel-v86`
//! (multiboot, runs in the v86 browser emulator). Everything here is plain port
//! I/O and memory, so it compiles for both `x86` and `x86_64`.
//!
//! No interrupts are used anywhere: the keyboard, the clock and the sound cards
//! are all polled from the main loop.
#![no_std]

pub mod ac97;
pub mod bochs_vga;
pub mod keyboard;
pub mod pci;
pub mod port;
pub mod sb16;
pub mod serial;
pub mod timer;
mod run;

pub use run::{AudioOut, Screen, run};

/// Stops the CPU for good (interrupts stay disabled, so `hlt` never returns).
pub fn halt_forever() -> ! {
    loop {
        // SAFETY: plain halt with interrupts masked; no memory effects.
        unsafe { core::arch::asm!("cli", "hlt", options(nomem, nostack)) };
    }
}
