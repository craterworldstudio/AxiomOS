bits 16
org 0x7C00

; Setup SECTOR --------------------------------------------
cli
cld

xor ax, ax      ; AX = 0
mov ds, ax      ; for memory DS:offset
mov ss, ax      ; for stack base/segment SS:SP
mov si, ax      ; reseting si for memory na lo  instructions
mov es, ax      ; for memory ES:DI
mov di, ax      ; for memory ES:DI
mov sp, 0x7C00  ; for stack offset/pointer

sti
jmp main

; Data SECTOR ----------------------------------------------

msgb db "Booted!", 0
msgh db "Halted-", 0
msgcl db "EoC. Current length: ", 0

comhelp db "help",0
comhelpout db "Helped ya there fam!", 0


txt_buff times 64 db 0

section .text

read_char:

    mov ah, 0x00
    int 0x16

    ret

read_line:    
    .read_loop:
        call read_char
        cmp al, 0x0D
        je .enter

        cmp al, 0x1B
        je .exit

        call print_char
        stosb
        jmp .read_loop


    .enter:
        call mov_nextline
        mov al, 0
        stosb   
        ret

    .exit:
        ret

print_char:
    mov ah, 0x0E
    int 0x10

    ret

print_str:
    mov ah, 0x0E
    
    .loop:
        lodsb

        cmp al, 0
        je .done
        
        int 0x10

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





_help:
    mov si, comhelpout
    call print_str
    call mov_nextline

    jmp _shell




_shell:
    mov al, '>'
    call print_char
    
    .input_loop:
        mov di, txt_buff
        call read_line

        cmp al, 0x1B
        je .exit

        


        mov si, txt_buff
        mov di, comhelp
        mov cx, 5

        repe cmpsb
        je _help




        
        
        jmp _shell
    
    .exit:
        call mov_nextline
        ret

; ENTRY POINT ------------------------------------------------------------------ 

main:
    mov si, msgb
    call print_str
    call mov_nextline
    call mov_nextline

    
    call _shell



    


    jmp halt

halt:


    mov si, msgh
    call print_str
    call mov_nextline
    call mov_nextline














    mov si, msgcl
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