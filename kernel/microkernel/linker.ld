ENTRY(_start)

SECTIONS
{
    . = 0x100000;

    KERNEL_START = .;

    .text : ALIGN(16)
    {
        *(.text._start)
        *(.text .text.*)
    }

    .rodata : ALIGN(16)
    {
        *(.rodata .rodata.*)
    }

    .data : ALIGN(16)
    {
        *(.data .data.*)
    }

    .bss : ALIGN(16)
    {
        BSS_START = .;
        *(COMMON)
        *(.bss .bss.*)
        BSS_END = .;
    }

    KERNEL_END = .;

    /DISCARD/ :
    {
        *(.eh_frame)
        *(.eh_frame_hdr)
        *(.dynamic)
        *(.comment)
        *(.note.*)
    }
}

