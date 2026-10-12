//! x86 port I/O.
use core::arch::asm;

/// # Safety
/// Writing to an I/O port can have arbitrary hardware side effects.
pub unsafe fn outb(port: u16, value: u8) {
    unsafe { asm!("out dx, al", in("dx") port, in("al") value, options(nomem, nostack, preserves_flags)) };
}

/// # Safety
/// Reading some I/O ports has side effects (e.g. popping a FIFO).
pub unsafe fn inb(port: u16) -> u8 {
    let value: u8;
    unsafe { asm!("in al, dx", out("al") value, in("dx") port, options(nomem, nostack, preserves_flags)) };
    value
}

/// # Safety
/// See [`outb`].
pub unsafe fn outw(port: u16, value: u16) {
    unsafe { asm!("out dx, ax", in("dx") port, in("ax") value, options(nomem, nostack, preserves_flags)) };
}

/// # Safety
/// See [`inb`].
pub unsafe fn inw(port: u16) -> u16 {
    let value: u16;
    unsafe { asm!("in ax, dx", out("ax") value, in("dx") port, options(nomem, nostack, preserves_flags)) };
    value
}

/// # Safety
/// See [`outb`].
pub unsafe fn outl(port: u16, value: u32) {
    unsafe { asm!("out dx, eax", in("dx") port, in("eax") value, options(nomem, nostack, preserves_flags)) };
}

/// # Safety
/// See [`inb`].
pub unsafe fn inl(port: u16) -> u32 {
    let value: u32;
    unsafe { asm!("in eax, dx", out("eax") value, in("dx") port, options(nomem, nostack, preserves_flags)) };
    value
}
