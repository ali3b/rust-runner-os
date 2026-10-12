//! Bochs/QEMU "dispi" VGA (QEMU `-vga std`, and v86's VGA) in 640x480, 8 bits per
//! pixel with an 8-bit palette DAC. The game's frames are already palette indices,
//! so presenting is one 300 KiB copy and palette changes are 768 port writes.
use crate::{
    Screen, pci,
    port::{inw, outb, outw},
};
use runner_core::engine::graphics::{HEIGHT, Palette, WIDTH};

const INDEX: u16 = 0x1CE;
const DATA: u16 = 0x1CF;
const REG_ID: u16 = 0;
const REG_XRES: u16 = 1;
const REG_YRES: u16 = 2;
const REG_BPP: u16 = 3;
const REG_ENABLE: u16 = 4;
const REG_VIRT_WIDTH: u16 = 6;
const ENABLED: u16 = 0x01;
const DAC_8BIT: u16 = 0x20;
const LFB_ENABLED: u16 = 0x40;
/// Where QEMU and v86 put the framebuffer if PCI doesn't say otherwise.
const DEFAULT_LFB: u64 = 0xE000_0000;

fn write(reg: u16, value: u16) {
    // SAFETY: Bochs VBE index/data ports.
    unsafe {
        outw(INDEX, reg);
        outw(DATA, value);
    }
}

fn read(reg: u16) -> u16 {
    // SAFETY: as above.
    unsafe {
        outw(INDEX, reg);
        inw(DATA)
    }
}

pub struct BochsVga {
    lfb: *mut u8,
}

impl BochsVga {
    /// Switches to 640x480x8. `map` turns the framebuffer's physical address into a
    /// pointer (identity when paging is off).
    pub fn init(map: impl FnOnce(u64) -> *mut u8) -> Result<Self, &'static str> {
        let id = read(REG_ID);
        if !(0xB0C0..=0xB0CF).contains(&id) {
            return Err("no Bochs VBE display adapter");
        }
        let phys = pci::find_class(0x03).and_then(|d| d.mem_bar(0)).filter(|&a| a != 0).unwrap_or(DEFAULT_LFB);
        write(REG_ENABLE, 0);
        write(REG_XRES, WIDTH as u16);
        write(REG_YRES, HEIGHT as u16);
        write(REG_BPP, 8);
        write(REG_VIRT_WIDTH, WIDTH as u16);
        write(REG_ENABLE, ENABLED | LFB_ENABLED | DAC_8BIT);
        crate::log!("display: Bochs VBE {:#x}, 640x480x8 at {:#x}", id, phys);
        Ok(BochsVga { lfb: map(phys) })
    }
}

impl Screen for BochsVga {
    fn set_palette(&mut self, palette: &Palette) {
        // SAFETY: VGA DAC write index and data ports.
        unsafe {
            outb(0x3C8, 0);
            for c in palette.iter() {
                outb(0x3C9, c.r);
                outb(0x3C9, c.g);
                outb(0x3C9, c.b);
            }
        }
    }

    fn present(&mut self, frame: &[u8], _crt: bool) {
        // Scanlines are drawn by whatever shows this screen (the browser page).
        let n = (WIDTH * HEIGHT).min(frame.len());
        // SAFETY: the LFB holds at least 640x480 bytes in this mode.
        unsafe { core::ptr::copy_nonoverlapping(frame.as_ptr(), self.lfb, n) };
    }
}
