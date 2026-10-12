/* Multiboot kernel loaded at 1 MiB, paging off, so addresses are physical. */
ENTRY(_start)

SECTIONS
{
    . = 1M;
    .multiboot : ALIGN(4) { KEEP(*(.multiboot)) }
    .text : ALIGN(16) { *(.text._start) *(.text .text.*) }
    .rodata : ALIGN(16) { *(.rodata .rodata.*) }
    .data : ALIGN(16) { *(.data .data.*) }
    .bss : ALIGN(4096) { *(.bss .bss.*) *(COMMON) }
    /DISCARD/ : { *(.eh_frame*) *(.comment) }
}
