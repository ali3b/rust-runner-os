//! Intel 82801AA AC'97 audio (QEMU `-device AC97`), 48 kHz 16-bit stereo output.
//!
//! No interrupts: the main loop calls [`Ac97::poll`] often, which tops up a ring of
//! small DMA buffers a fixed distance ahead of the one playing. The ring and its
//! buffer descriptor list live in a block of RAM the caller provides (below 4 GiB,
//! page aligned, [`DMA_BYTES`] long), given by both its physical and virtual address.
use super::{
    pci,
    port::{inb, inl, inw, outb, outl, outw},
};
use core::sync::atomic::{Ordering, compiler_fence};

const VENDOR_INTEL: u16 = 0x8086;
const DEVICE_AC97: u16 = 0x2415;

// Native audio mixer registers (BAR0).
const NAM_RESET: u16 = 0x00;
const NAM_MASTER_VOL: u16 = 0x02;
const NAM_PCM_VOL: u16 = 0x18;
const NAM_EXT_ID: u16 = 0x28;
const NAM_EXT_CTRL: u16 = 0x2A;
const NAM_FRONT_RATE: u16 = 0x2C;

// Native audio bus master registers (BAR1); PCM out box starts at 0x10.
const PO_BDBAR: u16 = 0x10;
const PO_CIV: u16 = 0x14;
const PO_LVI: u16 = 0x15;
const PO_SR: u16 = 0x16;
const PO_CR: u16 = 0x1B;
const GLOB_CNT: u16 = 0x2C;
const GLOB_STA: u16 = 0x30;

const CR_RUN: u8 = 0x01;
const CR_RESET: u8 = 0x02;
const SR_HALTED: u16 = 0x01;
const SR_CLEAR: u16 = 0x1C;

const RING: usize = 32;
/// Stereo frames per DMA buffer (~5.3 ms at 48 kHz).
pub const FRAMES_PER_BUFFER: usize = 256;
const SAMPLES_PER_BUFFER: usize = FRAMES_PER_BUFFER * 2;
const BUFFER_BYTES: usize = SAMPLES_PER_BUFFER * 2;
/// Buffers kept queued ahead of the playing one (~53 ms of latency).
const LEAD: usize = 10;
pub const DMA_BYTES: usize = RING * 8 + RING * BUFFER_BYTES;

#[repr(C)]
struct Descriptor {
    addr: u32,
    samples: u16,
    flags: u16,
}

pub struct Ac97 {
    nabm: u16,
    /// Virtual address of the descriptor list; buffers follow it.
    bdl: *mut Descriptor,
    buffers: *mut i16,
    next: usize,
    pub underruns: u32,
}

impl Ac97 {
    /// Finds and starts the sound card. `fill` renders the first buffers.
    ///
    /// # Safety
    /// `virt` must point to [`DMA_BYTES`] of writable memory that nothing else uses,
    /// living for the rest of the program, whose physical address is `phys`.
    pub unsafe fn init(phys: u64, virt: *mut u8, fill: &mut dyn FnMut(&mut [i16])) -> Result<Self, &'static str> {
        let dev = pci::find(VENDOR_INTEL, DEVICE_AC97).ok_or("no AC97 device on PCI bus 0")?;
        let nam = dev.io_bar(0).ok_or("AC97 BAR0 is not I/O")?;
        let nabm = dev.io_bar(1).ok_or("AC97 BAR1 is not I/O")?;
        dev.enable_io_and_dma();
        if phys + DMA_BYTES as u64 > 0xFFFF_F000 {
            return Err("DMA block must be below 4 GiB");
        }

        // SAFETY: AC'97 register programming on the ports read from the BARs.
        unsafe {
            outl(nabm + GLOB_CNT, 0x2); // leave cold reset
            let mut ready = false;
            for _ in 0..1_000_000 {
                if inl(nabm + GLOB_STA) & 0x100 != 0 {
                    ready = true;
                    break;
                }
            }
            if !ready {
                return Err("AC97 codec never became ready");
            }
            outw(nam + NAM_RESET, 0);
            outw(nam + NAM_MASTER_VOL, 0x0000); // 0 dB, unmuted
            outw(nam + NAM_PCM_VOL, 0x0808); // 0 dB, unmuted
            if inw(nam + NAM_EXT_ID) & 1 != 0 {
                // Variable-rate audio: pin it to our rate explicitly.
                outw(nam + NAM_EXT_CTRL, inw(nam + NAM_EXT_CTRL) | 1);
                outw(nam + NAM_FRONT_RATE, runner_core::audio::SAMPLE_RATE as u16);
            }
            let rate = inw(nam + NAM_FRONT_RATE);
            if rate as u32 != runner_core::audio::SAMPLE_RATE {
                return Err("AC97 codec is not running at 48 kHz");
            }

            outb(nabm + PO_CR, CR_RESET);
            for _ in 0..100_000 {
                if inb(nabm + PO_CR) & CR_RESET == 0 {
                    break;
                }
            }
        }

        let mut card = Ac97 {
            nabm,
            bdl: virt as *mut Descriptor,
            // SAFETY: inside the caller's DMA block.
            buffers: unsafe { virt.add(RING * 8) } as *mut i16,
            next: 0,
            underruns: 0,
        };
        for i in 0..RING {
            let addr = phys + (RING * 8 + i * BUFFER_BYTES) as u64;
            // SAFETY: `i < RING`, inside the DMA block we own.
            unsafe {
                card.bdl.add(i).write_volatile(Descriptor { addr: addr as u32, samples: SAMPLES_PER_BUFFER as u16, flags: 0 });
            }
        }
        for _ in 0..LEAD {
            card.fill_next(fill);
        }
        compiler_fence(Ordering::SeqCst);
        // SAFETY: start DMA on the descriptor list just written.
        unsafe {
            outl(nabm + PO_BDBAR, phys as u32);
            outb(nabm + PO_LVI, (LEAD - 1) as u8);
            outb(nabm + PO_CR, CR_RUN);
        }
        Ok(card)
    }

    fn fill_next(&mut self, fill: &mut dyn FnMut(&mut [i16])) {
        // SAFETY: `next < RING`, so the slice lies inside our DMA block, and the
        // controller isn't reading this buffer (it is behind LVI).
        let buf = unsafe { core::slice::from_raw_parts_mut(self.buffers.add(self.next * SAMPLES_PER_BUFFER), SAMPLES_PER_BUFFER) };
        fill(buf);
        self.next = (self.next + 1) % RING;
    }

}

impl crate::AudioOut for Ac97 {
    fn underruns(&self) -> u32 {
        self.underruns
    }

    /// Keeps `LEAD` buffers queued ahead of the playing one. Cheap when nothing is
    /// due. Returns whether any buffer was refilled.
    fn poll(&mut self, fill: &mut dyn FnMut(&mut [i16])) -> bool {
        // SAFETY: reading/writing the PCM-out box registers.
        unsafe {
            let civ = inb(self.nabm + PO_CIV) as usize % RING;
            let mut filled = false;
            while (self.next + RING - civ) % RING < LEAD {
                self.fill_next(fill);
                filled = true;
            }
            if !filled {
                return false;
            }
            compiler_fence(Ordering::SeqCst);
            outb(self.nabm + PO_LVI, ((self.next + RING - 1) % RING) as u8);
            let sr = inw(self.nabm + PO_SR);
            if sr & SR_HALTED != 0 {
                // We fell behind and the DMA engine stopped at LVI; kick it again.
                self.underruns += 1;
                outb(self.nabm + PO_CR, CR_RUN);
            }
            outw(self.nabm + PO_SR, SR_CLEAR);
        }
        true
    }
}
