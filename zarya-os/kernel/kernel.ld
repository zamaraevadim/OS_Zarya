/* Zarya OS Kernel Linker Script */

ENTRY(kernel_main)

SECTIONS
{
    . = 1M;

    .boot :
    {
        KEEP(*(.multiboot_header))
    }

    .text : ALIGN(4K)
    {
        *(.text*)
    }

    .rodata : ALIGN(4K)
    {
        *(.rodata*)
    }

    .data : ALIGN(4K)
    {
        *(.data*)
    }

    .bss : ALIGN(4K)
    {
        *(COMMON)
        *(.bss*)
        _bss_end = .;
    }

    /DISCARD/ :
    {
        *(.eh_frame)
        *(.gnu.hash)
    }
}
