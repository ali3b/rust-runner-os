//! Sound Blaster 16 (v86 and QEMU `-device sb16`), 48 kHz 16-bit stereo.
//!
//! Like the AC'97 driver it never uses interrupts: the DSP runs one endless
//! auto-init DMA transfer (16-bit channel 5) over a ring buffer, and
//! [`AudioOut::poll`] reads the DMA controller's current address to see how far
//! playback has got, then renders new samples a fixed distance ahead of it.
//! The ring must sit below 16 MiB and must not cross a 128 KiB boundary (ISA DMA).
use crate::{
    AudioOut,
    port::{inb, outb},
};
use runner_core::audio::SAMPLE_RATE;

const BASE: u16 = 0x220;
const DSP_RESET: u16 = BASE + 0x6;
const DSP_READ: u16 = BASE + 0xA;
const DSP_WRITE: u16 = BASE + 0xC;
const DSP_READ_STATUS: u16 = BASE + 0xE;
const DSP_ACK_16: u16 = BASE + 0xF;

// 8237 controller #2 (16-bit channels); channel 5 is its channel 1.
const DMA_MASK: u16 = 0xD4;
const DMA_MODE: u16 = 0xD6;
const DMA_FLIPFLOP: u16 = 0xD8;
const DMA5_ADDR: u16 = 0xC4;
const DMA5_COUNT: u16 = 0xC6;
const DMA5_PAGE: u16 = 0x8B;

/// Ring size in bytes: 4096 stereo frames (~85 ms).
pub const RING_BYTES: usize = 16 * 1024;
/// Keep this much audio rendered ahead of the DMA position (~50 ms).
const LEAD_BYTES: usize = 9600;
const FRAME_BYTES: usize = 4;

pub struct Sb16 {
    ring: *mut i16,
    start_word: u16,
    /// Next byte offset in the ring to render into.
    write: usize,
    underruns: u32,
}

fn dsp_write(value: u8) -> Result<(), &'static str> {
    // SAFETY: SB16 DSP ports.
    unsafe {
        for _ in 0..100_000 {
            if inb(DSP_WRITE) & 0x80 == 0 {
                outb(DSP_WRITE, value);
                return Ok(());
            }
        }
    }
    Err("SB16 DSP never became ready")
}

impl Sb16 {
    /// Resets the card and starts playback. `phys`/`virt` locate a [`RING_BYTES`]
    /// buffer satisfying the ISA DMA rules; `fill` renders the first audio.
    ///
    /// # Safety
    /// `virt` must point to [`RING_BYTES`] of writable memory that nothing else
    /// uses, living for the rest of the program, whose physical address is `phys`.
    pub unsafe fn init(phys: u64, virt: *mut u8, fill: &mut dyn FnMut(&mut [i16])) -> Result<Self, &'static str> {
        if phys + RING_BYTES as u64 > 16 << 20 || phys / (128 << 10) != (phys + RING_BYTES as u64 - 1) / (128 << 10) {
            return Err("SB16 ring must be below 16 MiB and inside one 128 KiB block");
        }
        // SAFETY: SB16 DSP and 8237 DMA programming on their standard ports.
        unsafe {
            outb(DSP_RESET, 1);
            for _ in 0..1000 {
                core::hint::spin_loop();
            }
            outb(DSP_RESET, 0);
            let mut ok = false;
            for _ in 0..100_000 {
                if inb(DSP_READ_STATUS) & 0x80 != 0 && inb(DSP_READ) == 0xAA {
                    ok = true;
                    break;
                }
            }
            if !ok {
                return Err("no Sound Blaster DSP at 0x220");
            }
        }

        let mut card = Sb16 { ring: virt as *mut i16, start_word: (phys >> 1) as u16, write: 0, underruns: 0 };
        // Silence everywhere, then the first stretch of real audio.
        // SAFETY: the caller's ring buffer.
        unsafe { core::ptr::write_bytes(virt, 0, RING_BYTES) };
        card.render(LEAD_BYTES, fill);

        let words = (RING_BYTES / 2) as u16;
        // SAFETY: as above.
        unsafe {
            outb(DMA_MASK, 0x04 | 1); // mask channel 5
            outb(DMA_FLIPFLOP, 0);
            outb(DMA_MODE, 0x58 | 1); // single, auto-init, memory -> device, channel 5
            outb(DMA5_ADDR, card.start_word as u8);
            outb(DMA5_ADDR, (card.start_word >> 8) as u8);
            outb(DMA5_COUNT, (words - 1) as u8);
            outb(DMA5_COUNT, ((words - 1) >> 8) as u8);
            outb(DMA5_PAGE, ((phys >> 16) & 0xFE) as u8);
            outb(DMA_MASK, 1); // unmask channel 5
        }
        dsp_write(0xD1)?; // speaker on
        dsp_write(0x41)?; // output sample rate
        dsp_write((SAMPLE_RATE >> 8) as u8)?;
        dsp_write(SAMPLE_RATE as u8)?;
        dsp_write(0xB6)?; // 16-bit, D/A, auto-init, FIFO
        dsp_write(0x30)?; // stereo, signed
        dsp_write((words - 1) as u8)?;
        dsp_write(((words - 1) >> 8) as u8)?;
        Ok(card)
    }

    /// Byte offset in the ring the DMA engine will read next.
    fn play_pos(&self) -> usize {
        // SAFETY: reading the channel 5 current address (low byte, then high).
        let word = unsafe {
            outb(DMA_FLIPFLOP, 0);
            let lo = inb(DMA5_ADDR) as u16;
            let hi = inb(DMA5_ADDR) as u16;
            lo | hi << 8
        };
        (word.wrapping_sub(self.start_word) as usize * 2) % RING_BYTES
    }

    /// Renders `bytes` (a multiple of a frame) at the write position, wrapping.
    fn render(&mut self, bytes: usize, fill: &mut dyn FnMut(&mut [i16])) {
        let mut left = bytes - bytes % FRAME_BYTES;
        while left > 0 {
            let chunk = left.min(RING_BYTES - self.write);
            // SAFETY: `write..write+chunk` lies inside the ring.
            let buf = unsafe { core::slice::from_raw_parts_mut(self.ring.add(self.write / 2), chunk / 2) };
            fill(buf);
            self.write = (self.write + chunk) % RING_BYTES;
            left -= chunk;
        }
    }
}

impl AudioOut for Sb16 {
    fn poll(&mut self, fill: &mut dyn FnMut(&mut [i16])) -> bool {
        // Acknowledge the end-of-block interrupt the card raises (nobody handles it).
        // SAFETY: reading the 16-bit IRQ acknowledge port.
        unsafe { inb(DSP_ACK_16) };
        let play = self.play_pos();
        let ahead = (self.write + RING_BYTES - play) % RING_BYTES;
        if ahead > LEAD_BYTES + RING_BYTES / 4 {
            // Playback overtook us: restart just ahead of it.
            self.underruns += 1;
            self.write = (play + 4 * FRAME_BYTES) % RING_BYTES;
            self.render(LEAD_BYTES, fill);
            return true;
        }
        if ahead + 1024 >= LEAD_BYTES {
            return false;
        }
        self.render(LEAD_BYTES - ahead, fill);
        true
    }

    fn underruns(&self) -> u32 {
        self.underruns
    }
}
