bits 16
org 0x7C00

start:

  mov [boot_drive], dl

  cli
  cld

  xor ax, ax
  mov ds, ax
  mov es, ax
  mov ss, ax
  mov si, ax
  mov di, ax
  mov sp, 0x7C00


  mov ah, 0x42
  mov si, dap

  int 13h



dap:
  db 10h
  db 0

  dw 4
  dw 0x8000
  dw 0x0000

  dq 1


boot_drive db 0

jmp $

times 510-($-$$) db 0
dw 0xAA55
