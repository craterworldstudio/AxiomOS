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
    mov cl, 10               ; Start reading at Sector 6
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

    ; Print retroactive 16-bit status starting at Row 12 (safely below BIOS text)
    mov esi, msg_header
    mov edi, 1920           ; Row 12
    call vga_print

    mov esi, msg_kernel
    mov edi, 2080           ; Row 13
    call vga_print

    mov esi, msg_a20
    mov edi, 2240           ; Row 14
    call vga_print

    mov esi, msg_gdt
    mov edi, 2400           ; Row 15
    call vga_print

    mov esi, msg_pm
    mov edi, 2560           ; Row 16
    call vga_print





build_page_tables:
    ; 1. Clear 12 KiB of memory for PML4, PDPT, and PD (0x9000 - 0xBFFF)
    mov edi, 0x9000
    mov ecx, 3072           ; 12288 bytes / 4 bytes per DWORD = 3072
    xor eax, eax            ; We want to fill memory with zeros
    rep stosd               ; Store EAX (0) at [EDI], decrement ECX, repeat

    ; 2. Link PML4[0] to PDPT
    ; Address 0xA000 + Present (1) + Read/Write (2) = 0xA003
    mov dword [0x9000], 0xA003
    mov esi, msg_pml4
    mov edi, 2720            ; Row 17
    call vga_print

    ; 3. Link PDPT[0] to PD
    ; Address 0xB000 + Present (1) + Read/Write (2) = 0xB003
    mov dword [0xA000], 0xB003
    mov esi, msg_pdpt
    mov edi, 2880           ; Row 18
    call vga_print

    ; 4. Link PD[0] to the 2 MiB identity page (physical 0x0)
    ; Address 0x0 + Present (1) + Read/Write (2) + Page Size (0x80) = 0x0083
    mov dword [0xB000], 0x0083
    mov esi, msg_pd
    mov edi, 3040           ; Row 19
    call vga_print

    ; 5. Load CR3 with the physical address of the PML4
    mov eax, 0x9000
    mov cr3, eax
    mov esi, msg_cr3
    mov edi, 3200           ; Row 20
    call vga_print

    mov esi, msg_ready
    mov edi, 3520           ; Row 21 (Skip a line)
    call vga_print

pm_halt:
    cli
    hlt
    jmp pm_halt

; ------------------------------------------
; VGA telemetry helper
; ------------------------------------------
vga_print:
    push eax
    push ebx
    mov ah, 0x0F            ; Default color: White
.next:
    lodsb
    test al, al
    jz .done
    cmp al, 1               ; Is it the "Switch to Green" code?
    je .set_green
    cmp al, 2               ; Is it the "Switch to White" code?
    je .set_white
    ; Normal printable character
    mov [0xB8000 + edi], al
    mov [0xB8000 + edi + 1], ah
    add edi, 2
    jmp .next
.set_green:
    mov ah, 0x0A            ; Light Green
    jmp .next
.set_white:
    mov ah, 0x0F            ; White
    jmp .next
.done:
    pop ebx
    pop eax
    ret

; ------------------------------------------
; Telemetry Strings
; ------------------------------------------
msg_header db "AXIOM BOOT // STAGE 2", 0
msg_kernel db "[ ", 1, "OK", 2, " ] KERNEL LOADED @ 0x10000", 0
msg_a20    db "[ ", 1, "OK", 2, " ] A20 LINE ENABLED", 0
msg_gdt    db "[ ", 1, "OK", 2, " ] GDT LOADED", 0
msg_pm     db "[ ", 1, "OK", 2, " ] PROTECTED MODE", 0
msg_pml4   db "[ ", 1, "OK", 2, " ] PML4  @ 0x9000", 0
msg_pdpt   db "[ ", 1, "OK", 2, " ] PDPT  @ 0xA000", 0
msg_pd     db "[ ", 1, "OK", 2, " ] PD    @ 0xB000", 0
msg_cr3    db "[ ", 1, "OK", 2, " ] CR3   = 0x9000", 0
msg_ready  db 1, "PAGE TABLES READY", 2, 0

; Pad Stage 2 to exactly 8 sectors (4096 bytes)
times 4096-($-$$) db 0