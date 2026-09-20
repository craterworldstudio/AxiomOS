[BITS 16]
[ORG 0x7C00]

start:
    cli                     ; Clear interrupts for safe segment setup
    xor ax, ax              
    mov ds, ax
    mov es, ax
    mov ss, ax
    mov sp, 0x7C00          ; Stack grows downward from bootloader
    sti

    ; BIOS leaves the boot drive number in DL. Save it.
    mov [BOOT_DRIVE], dl

    ; Breadcrumb 1
    mov si, stage1_msg
print_stage1:
    lodsb
    or al, al
    jz load_stage2
    mov ah, 0x0E
    int 0x10
    jmp print_stage1

load_stage2:
    ; Read Stage 2 from disk into memory at 0x7E00
    mov ah, 0x02            ; BIOS Read Sector function
    mov al, 8               ; Read 8 sectors (4096 bytes for Stage 2)
    mov ch, 0               ; Cylinder 0
    mov dh, 0               ; Head 0
    mov cl, 2               ; Sector 2 (1-indexed; Sector 1 is this bootloader)
    mov bx, 0x7E00          ; Destination address ES:BX (0x0000:0x7E00)
    int 0x13
    jc disk_error           ; Carry flag is set on failure

    ; Handoff: Jump into Stage 2
    jmp 0x7E00

disk_error:
    mov si, error_msg
print_error:
    lodsb
    or al, al
    jz halt
    mov ah, 0x0E
    int 0x10
    jmp print_error

halt:
    cli
    hlt
    jmp halt

stage1_msg db "Axiom Stage 1 Online", 13, 10, 0
error_msg db "Stage 2 Disk Error", 13, 10, 0
BOOT_DRIVE db 0

; Boot sector magic number
times 510-($-$$) db 0
dw 0xAA55