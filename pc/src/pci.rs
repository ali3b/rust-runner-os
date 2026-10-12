//! Legacy PCI configuration access (ports 0xCF8/0xCFC), bus 0 only — enough for
//! the QEMU `pc` machine, where every device sits on bus 0.
use super::port::{inl, outl};

#[derive(Clone, Copy, Debug)]
pub struct PciDevice {
    pub dev: u8,
    pub vendor: u16,
    pub device: u16,
}

fn address(dev: u8, offset: u8) -> u32 {
    0x8000_0000 | (dev as u32) << 11 | (offset as u32 & 0xFC)
}

pub fn read32(dev: u8, offset: u8) -> u32 {
    // SAFETY: standard configuration mechanism #1.
    unsafe {
        outl(0xCF8, address(dev, offset));
        inl(0xCFC)
    }
}

pub fn write32(dev: u8, offset: u8, value: u32) {
    // SAFETY: as above.
    unsafe {
        outl(0xCF8, address(dev, offset));
        outl(0xCFC, value);
    }
}

/// Finds the first function-0 device on bus 0 matching `vendor:device`.
pub fn find(vendor: u16, device: u16) -> Option<PciDevice> {
    (0..32).find_map(|dev| {
        let id = read32(dev, 0);
        let found = PciDevice { dev, vendor: id as u16, device: (id >> 16) as u16 };
        (found.vendor == vendor && found.device == device).then_some(found)
    })
}

/// Finds the first function-0 device on bus 0 whose class code is `class`
/// (e.g. 0x03 for display controllers).
pub fn find_class(class: u8) -> Option<PciDevice> {
    (0..32).find_map(|dev| {
        let id = read32(dev, 0);
        if id as u16 == 0xFFFF || (read32(dev, 0x08) >> 24) as u8 != class {
            return None;
        }
        Some(PciDevice { dev, vendor: id as u16, device: (id >> 16) as u16 })
    })
}

impl PciDevice {
    /// Memory-space BAR `n` as a physical address, if it is a memory BAR.
    pub fn mem_bar(&self, n: u8) -> Option<u64> {
        let bar = read32(self.dev, 0x10 + n * 4);
        (bar & 1 == 0).then_some((bar & !0xF) as u64)
    }

    /// I/O-space BAR `n` as a port number, if it is an I/O BAR.
    pub fn io_bar(&self, n: u8) -> Option<u16> {
        let bar = read32(self.dev, 0x10 + n * 4);
        (bar & 1 == 1).then_some((bar & !0x3) as u16)
    }

    /// Enables I/O decoding and bus mastering (DMA).
    pub fn enable_io_and_dma(&self) {
        let cmd = read32(self.dev, 0x04) & 0xFFFF;
        write32(self.dev, 0x04, cmd | 0x1 | 0x4);
    }
}
