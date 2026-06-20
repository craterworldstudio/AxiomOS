bits 16
org 0x7C00

cli
cld

xor ax, ax
mov ds, ax
mov es, ax
mov ss, ax
mov si, ax
mov di, ax
mov sp, 0x7C00

std





jmp $

times 510-($-$$) db 0
dw 0xAA55