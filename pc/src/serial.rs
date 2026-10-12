//! COM1 (16550 UART) debug output. QEMU forwards it with `-serial stdio`/`-serial file:…`.
use super::port::{inb, outb};
use core::fmt;

const COM1: u16 = 0x3F8;

pub fn init() {
    // SAFETY: standard 16550 init sequence on the COM1 ports.
    unsafe {
        outb(COM1 + 1, 0x00); // no interrupts
        outb(COM1 + 3, 0x80); // DLAB on
        outb(COM1, 0x01); // divisor 1 = 115200 baud
        outb(COM1 + 1, 0x00);
        outb(COM1 + 3, 0x03); // 8N1, DLAB off
        outb(COM1 + 2, 0xC7); // FIFO on, cleared, 14-byte threshold
        outb(COM1 + 4, 0x03); // DTR + RTS
    }
}

fn write_byte(b: u8) {
    // SAFETY: polling the line status register, then writing the data register.
    unsafe {
        // Bounded wait so a missing UART can't hang the game.
        for _ in 0..100_000 {
            if inb(COM1 + 5) & 0x20 != 0 {
                break;
            }
        }
        outb(COM1, b);
    }
}

pub struct Serial;

impl fmt::Write for Serial {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for b in s.bytes() {
            if b == b'\n' {
                write_byte(b'\r');
            }
            write_byte(b);
        }
        Ok(())
    }
}

#[macro_export]
macro_rules! log {
    ($($arg:tt)*) => {{
        use core::fmt::Write as _;
        let _ = writeln!($crate::serial::Serial, $($arg)*);
    }};
}
