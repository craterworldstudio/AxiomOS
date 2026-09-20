ENTRY(_start)

SECTIONS
{
    . = 0x100000;

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
        *(COMMON)
        *(.bss .bss.*)
    }

    /DISCARD/ :
    {
        *(.eh_frame)
        *(.eh_frame_hdr)
        *(.dynamic)
        *(.comment)
        *(.note.*)
    }
}