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

    ; Check if argc >= 2
    cmp rdi, 2
    jl .print_default

    ; argv is in rsi. argv[1] is at [rsi + 8]
    mov rax, [rsi + 8]
    test rax, rax
    jz .print_default

    ; Check if argv[1] is "-h"
    cmp byte [rax], '-'
    jne .print_default
    cmp byte [rax + 1], 'h'
    jne .check_dh
    cmp byte [rax + 2], 0
    jne .check_dh
    lea rbx, [msg_h]
    mov r12, msg_h_len
    jmp .loop

.check_dh:
    ; Check if argv[1] is "-dh"
    cmp byte [rax + 1], 'd'
    jne .print_default
    cmp byte [rax + 2], 'h'
    jne .print_default
    cmp byte [rax + 3], 0
    jne .print_default

    lea rbx, [msg_dh]
    mov r12, msg_dh_len
    jmp .loop

.print_default:
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

msg_dh: db "  whoami               - display active user identity and authority keyring", 10
msg_dh_len equ $ - msg_dh

msg_h: db "Usage: whoami [options]", 10, "Display active user identity and CSpace authority keyring.", 10, "Options:", 10, "  -h, --help    Show this command reference manual", 10
msg_h_len equ $ - msg_h
