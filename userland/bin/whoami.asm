[bits 64]
default rel

section .text
global _start
_start:
    push rbp
    mov rbp, rsp
    push rbx
    push r12
    push r13

    lea rbx, [msg]
    mov r12, msg_len

.loop:
    cmp r12, 0
    jle .done

    mov r13, r12
    cmp r13, 8
    jle .small
    mov r13, 8
.small:
    mov rdx, [rbx]
    mov rdi, 0x100000005    ; Slot 5, Gen 1 (Console capability handle)
    mov rsi, 6              ; OP_CONSOLE_WRITE
    mov r10, r13            ; arg2 = chunk length
    syscall

    add rbx, r13
    sub r12, r13
    jmp .loop

.done:
    pop r13
    pop r12
    pop rbx
    mov rsp, rbp
    pop rbp
    xor eax, eax
    ret

section .rodata
msg: db "root (Authority Keyring #0: Genesis Sovereign)", 10
msg_len equ $ - msg
