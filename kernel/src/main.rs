//! Rust Runner OS, 64-bit build: booted by the `bootloader` crate (BIOS) in QEMU.
//! Sets up the bootloader's framebuffer and the AC'97 sound card, then hands over
//! to the shared game loop in the `pc` crate. No processes, no scheduler, no
//! interrupts: one foreground loop polling the keyboard and a clock.
#![no_std]
#![no_main]

mod framebuffer;

use bootloader_api::{
    BootInfo, BootloaderConfig,
    config::Mapping,
    entry_point,
    info::{MemoryRegionKind, MemoryRegions},
};
use core::panic::PanicInfo;
use framebuffer::Display;
use pc::{AudioOut, ac97::Ac97, log, serial, timer};
use runner_core::{
    audio::Synth,
    engine::graphics::{HEIGHT, WIDTH},
};

// The framebuffer size is requested via `BootConfig` in xtask when building the image.
static BOOTLOADER_CONFIG: BootloaderConfig = {
    let mut config = BootloaderConfig::new_default();
    config.kernel_stack_size = 128 * 1024;
    config.mappings.framebuffer = Mapping::Dynamic;
    // Needed to reach the audio DMA buffers by physical address.
    config.mappings.physical_memory = Some(Mapping::Dynamic);
    config
};

entry_point!(kmain, config = &BOOTLOADER_CONFIG);

/// A page-aligned, 32-bit addressable usable block for the AC'97 DMA ring.
fn dma_region(regions: &MemoryRegions) -> Option<u64> {
    regions.iter().filter(|r| r.kind == MemoryRegionKind::Usable).find_map(|r| {
        let start = r.start.max(0x10_0000).next_multiple_of(4096);
        let end = r.end.min(0xFFFF_F000);
        (start + pc::ac97::DMA_BYTES as u64 <= end).then_some(start)
    })
}

fn kmain(boot_info: &'static mut BootInfo) -> ! {
    // SAFETY: we never set up an IDT, so interrupts must stay off.
    unsafe { core::arch::asm!("cli", options(nomem, nostack)) };
    serial::init();
    log!("Rust Runner OS started");

    let fb = boot_info
        .framebuffer
        .take()
        .unwrap_or_else(|| panic!("bootloader provided no framebuffer"));
    let info = fb.info();
    log!(
        "framebuffer: {}x{} stride={} bpp={} format={:?}",
        info.width,
        info.height,
        info.stride,
        info.bytes_per_pixel,
        info.pixel_format
    );
    let mut display = Display::new(fb).unwrap_or_else(|e| panic!("unusable framebuffer: {e}"));
    let (out_w, out_h) = display.output_size();
    log!("display: {}x{} logical -> {}x{} on screen", WIDTH, HEIGHT, out_w, out_h);

    let tsc_hz = timer::init();
    log!("timer: TSC {} MHz", tsc_hz / 1_000_000);

    let mut synth = Synth::new();
    let mut audio = boot_info
        .physical_memory_offset
        .into_option()
        .ok_or("bootloader did not map physical memory")
        .and_then(|offset| {
            let phys = dma_region(&boot_info.memory_regions).ok_or("no usable memory below 4 GiB for DMA")?;
            // SAFETY: a usable region from the boot memory map that nothing else
            // claims, reached through the bootloader's physical-memory mapping.
            unsafe { Ac97::init(phys, (phys + offset) as *mut u8, &mut |buf| synth.render(buf)) }
        })
        .inspect(|_| log!("audio: AC97 48 kHz stereo"))
        .inspect_err(|e| log!("audio: disabled ({e})"))
        .ok();

    pc::run(&mut display, audio.as_mut().map(|a| a as &mut dyn AudioOut), synth)
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    // Serial may not be initialised yet if we panic very early; init is idempotent.
    serial::init();
    log!("KERNEL PANIC: {info}");
    pc::halt_forever()
}
