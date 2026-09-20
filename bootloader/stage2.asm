[BITS 16]
[ORG 0x7E00]

start:
    ; Stage 1 jumped here. DL still contains the boot drive.
    mov [BOOT_DRIVE], dl
    
    mov si, stage2_msg
    jmp print_stage2

print_stage2:
    lodsb
    or al, al
    jz load_kernel
    mov ah, 0x0E
    int 0x10
    jmp print_stage2

load_kernel:

    ; 1. Load the Kernel payload into staging memory
    ; The kernel starts at Sector 6 (Stage 1 = 1 sector, Stage 2 = 4 sectors)

    mov dl, [BOOT_DRIVE]

    mov ah, 0x02
    mov al, 5               ; Read 5 sectors 
    mov ch, 0               ; Cylinder 0
    mov dh, 0               ; Head 0
    mov cl, 6               ; Start reading at Sector 6
    mov bx, 0x1000          ; Load to Segment 0x1000 (0x1000:0x0000 = physical 0x10000)
    mov es, bx
    xor bx, bx              ; Offset 0
    int 0x13
    jc disk_error

    ; Reset ES back to 0 for flat memory operations
    xor ax, ax
    mov es, ax

    ; 2. Enable the A20 line (allows accessing memory above 1MB later)
    in al, 0x92
    or al, 2
    out 0x92, al

    ; 3. Disable interrupts and load the GDT
    cli
    lgdt [gdt_descriptor]

    ; 4. Enter 32-bit Protected Mode
    mov eax, cr0
    or eax, 0x1             ; Flip the Protection Enable (PE) bit
    mov cr0, eax

    ; 5. Far jump to flush the CPU pipeline and lock in the 32-bit Code Segment
    jmp CODE_SEG:init_pm

disk_error:
    mov si, err_msg
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

err_msg db "Kernel Disk Error", 13, 10, 0
stage2_msg db "Axiom Stage 2 Online", 13, 10, 0
BOOT_DRIVE db 0

; ==========================================
; Global Descriptor Table (GDT)
; ==========================================
gdt_start:
    dq 0x0                  ; Null descriptor (required)

gdt_code:                   ; 32-bit Code Segment
    dw 0xFFFF               ; Limit (bits 0-15)
    dw 0x0                  ; Base (bits 0-15)
    db 0x0                  ; Base (bits 16-23)
    db 10011010b            ; Access byte (Present, Ring 0, Executable)
    db 11001111b            ; Flags (32-bit) + Limit (bits 16-19)
    db 0x0                  ; Base (bits 24-31)

gdt_data:                   ; 32-bit Data Segment
    dw 0xFFFF               ; Limit
    dw 0x0                  ; Base
    db 0x0                  ; Base
    db 10010010b            ; Access byte (Present, Ring 0, Writable)
    db 11001111b            ; Flags + Limit
    db 0x0                  ; Base

gdt_end:

gdt_descriptor:
    dw gdt_end - gdt_start - 1
    dd gdt_start

CODE_SEG equ gdt_code - gdt_start
DATA_SEG equ gdt_data - gdt_start

; ==========================================
; 32-Bit Protected Mode Execution
; ==========================================
[BITS 32]
init_pm:
    ; Point all data segment registers to the new 32-bit Data Segment
    mov ax, DATA_SEG
    mov ds, ax
    mov ss, ax
    mov es, ax
    mov fs, ax
    mov gs, ax

    ; Set a safe 32-bit stack below the 1MB mark
    mov ebp, 0x90000
    mov esp, ebp

    ; 6. Print diagnostic proof directly to the VGA hardware buffer
    ; We write to 0xB8140 (Row 3) to avoid overwriting the BIOS text
    mov ebx, pm_msg
    mov edx, 0xB8140
print_pm:
    mov al, [ebx]
    cmp al, 0
    je pm_halt
    mov [edx], al           ; Write the character
    mov byte [edx+1], 0x0A  ; Write the color (Light Green)
    inc ebx
    add edx, 2
    jmp print_pm

pm_halt:
    cli
    hlt
    jmp pm_halt

pm_msg db "32-BIT PROTECTED MODE ONLINE", 0

; Pad Stage 2 to exactly 4 sectors (2048 bytes)
times 2048-($-$$) db 0