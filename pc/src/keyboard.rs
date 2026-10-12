//! Polled PS/2 keyboard on the i8042 controller, scan code set 1 (the BIOS leaves
//! translation on, which is QEMU's default).
use super::port::inb;
use runner_core::engine::input::{InputLatch, Key};

const DATA: u16 = 0x60;
const STATUS: u16 = 0x64;
const OUTPUT_FULL: u8 = 0x01;
const AUX_DATA: u8 = 0x20;

pub struct Keyboard {
    extended: bool,
}

impl Default for Keyboard {
    fn default() -> Self {
        Self::new()
    }
}

impl Keyboard {
    /// Drops any bytes left over from the BIOS/bootloader.
    pub fn new() -> Self {
        let mut kb = Keyboard { extended: false };
        kb.drain(|_| {});
        kb
    }

    fn drain(&mut self, mut f: impl FnMut(u8)) {
        // Bounded so a stuck controller can't freeze the game loop.
        for _ in 0..64 {
            // SAFETY: reading i8042 status/data ports.
            let status = unsafe { inb(STATUS) };
            if status & OUTPUT_FULL == 0 {
                break;
            }
            let byte = unsafe { inb(DATA) };
            if status & AUX_DATA == 0 {
                f(byte);
            }
        }
    }

    /// Reads all pending scan codes and feeds key transitions into `latch`.
    pub fn poll(&mut self, latch: &mut InputLatch) {
        let mut extended = self.extended;
        self.drain(|byte| {
            if byte == 0xE0 {
                extended = true;
                return;
            }
            let was_extended = core::mem::replace(&mut extended, false);
            let down = byte & 0x80 == 0;
            let key = match (was_extended, byte & 0x7F) {
                (false, 0x39) | (true, 0x48) | (false, 0x11) => Some(Key::Jump), // Space, Up, W
                (true, 0x50) | (false, 0x1F) => Some(Key::Slide),                // Down, S
                (true, 0x4D) | (false, 0x20) | (false, 0x25) => Some(Key::Kick), // Right, D, K
                (false, 0x13) => Some(Key::Restart),                             // R
                (false, 0x01) => Some(Key::Escape),                              // Esc
                (false, 0x2E) => Some(Key::Crt),                                 // C
                _ => None,
            };
            if let Some(key) = key {
                latch.key_event(key, down);
            }
        });
        self.extended = extended;
    }
}
