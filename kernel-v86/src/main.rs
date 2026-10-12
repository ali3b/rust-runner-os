//! Rust Runner OS, 32-bit build: a multiboot kernel for the v86 browser emulator
//! (also boots with `qemu-system-i386 -kernel`). Paging stays off, so physical
//! addresses are used directly: the Bochs VGA framebuffer for the screen and a
//! static ring buffer below 16 MiB for Sound Blaster 16 DMA. Then it hands over to
//! the same game loop as the 64-bit build.
#![no_std]
#![no_main]

use core::panic::PanicInfo;
use pc::{AudioOut, bochs_vga::BochsVga, log, sb16::Sb16, serial, timer};
use runner_core::audio::Synth;

const MULTIBOOT_MAGIC: u32 = 0x2BAD_B002;

core::arch::global_asm!(
    r#"
    .section .multiboot, "a"
    .align 4
    .long 0x1BADB002            /* magic */
    .long 0x00000003            /* page-align modules, want memory info */
    .long -(0x1BADB002 + 3)     /* checksum */

    .section .text._start, "ax"
    .global _start
    _start:
        cli
        mov esp, offset runner_stack_top
        push ebx                /* multiboot info */
        push eax                /* magic */
        call {main}
    2:  cli
        hlt
        jmp 2b

    .section .bss.runner_stack, "aw", @nobits
    .align 16
        .skip 256 * 1024
    runner_stack_top:
    "#,
    main = sym kmain32,
);

/// The Sound Blaster DMA ring: below 16 MiB (we are loaded at 1 MiB), and aligned
/// so it can't cross a 128 KiB boundary.
#[repr(C, align(32768))]
struct DmaRing([u8; pc::sb16::RING_BYTES]);
static mut DMA_RING: DmaRing = DmaRing([0; pc::sb16::RING_BYTES]);

extern "C" fn kmain32(magic: u32, _info: u32) -> ! {
    serial::init();
    log!("Rust Runner OS started (32-bit, v86 build)");
    if magic != MULTIBOOT_MAGIC {
        log!("warning: not started by a multiboot loader (magic {magic:#x})");
    }
    let tsc_hz = timer::init();
    log!("timer: TSC {} MHz", tsc_hz / 1_000_000);

    // Paging is off: physical addresses are pointers.
    let mut screen = BochsVga::init(|phys| phys as usize as *mut u8).unwrap_or_else(|e| panic!("no display: {e}"));

    let mut synth = Synth::new();
    let ring = &raw mut DMA_RING as *mut u8;
    // SAFETY: DMA_RING is only ever used here; with paging off, its address is physical.
    let mut audio = unsafe { Sb16::init(ring as usize as u64, ring, &mut |buf| synth.render(buf)) }
        .inspect(|_| log!("audio: SB16 48 kHz stereo"))
        .inspect_err(|e| log!("audio: disabled ({e})"))
        .ok();

    pc::run(&mut screen, audio.as_mut().map(|a| a as &mut dyn AudioOut), synth)
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    serial::init();
    log!("KERNEL PANIC: {info}");
    pc::halt_forever()
}
