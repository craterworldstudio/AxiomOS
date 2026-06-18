bits 16
org 0x7C00

; Setup SECTOR --------------------------------------------
cli
cld

xor ax, ax      ; AX = 0
mov ds, ax      ; for memory DS:offset
mov ss, ax      ; for stack base/segment SS:SP
mov si, ax      ; reseting si for memory na lo  instructions
mov sp, 0x7C00  ; for stack offset/pointer

sti
jmp main

; Data SECTOR ----------------------------------------------

msgb db "Booted!", 0
msgh db "Halted-", 0
msgcl db "EoC. Current length: ", 0

section .text

read_char:

    mov ah, 0x00
    int 0x16

    ret

print_char:
    mov ah, 0x0E
    int 0x10

    ret

print_str:
    mov ah, 0x0E
    mov si, 0
    .loop:
        mov al, [bx+si]
        cmp al, 0
        je .done
        
        int 0x10

        add si, 1

        jmp .loop

    .done:
        ret

print_num:

    push bx
    mov bx, 10
    push CX
    mov cx, 0

    .divide_loop:
        xor dx, dx

        div bx ;Division happens at DX:AX / BX and Q = AX and R = DX
        push dx

        add cx, 1

        cmp ax, 0
        jne .divide_loop
        je .print_loop
        

    .print_loop:
        pop ax
        mov ah, 0x0E
        add al, '0'
        int 0x10

        sub cx, 1
        cmp cx, 0
        je .done

        jmp .print_loop

    .done:
        pop cx
        pop bx
        ret

mov_nextline:
    mov ah, 0x0E
    mov al, 0x0D                        ; CR
    int 0x10 
    mov al, 0x0A                        ; LF
    int 0x10

    ret




_shell:
    mov al, '>'
    call print_char
    
    jmp .read_loop


    .read_loop:
        call read_char

        cmp al, 0x0D
        je .enter

        cmp al, 0x1B
        je .exit

        call print_char
        jmp .read_loop

    .enter:
        call mov_nextline
        jmp _shell

    .exit:
        call mov_nextline
        ret

; ENTRY POINT ------------------------------------------------------------------ 

main:
    mov bx, msgb
    call print_str
    call mov_nextline
    call mov_nextline

    
    call _shell



    


    jmp halt

halt:


    mov bx, msgh
    call print_str
    call mov_nextline
    call mov_nextline














    mov bx, msgcl
    call print_str

    mov ax, (p_end - $$)
    call print_num

    jmp .end

    .end:        
        hlt
        jmp .end

p_end: ;

times 510-($-$$) db 0
dw 0xAA55