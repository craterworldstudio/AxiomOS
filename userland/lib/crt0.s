.global _start
.extern main

.section .text._start
_start:
    # System V AMD64 ABI:
    # RDI = argc
    # RSI = argv
    # RDX = keyring (CapKeyring*)
    
    # Align stack to 16 bytes
    pushq %rbp
    movq %rsp, %rbp
    
    # Call main(argc, argv, keyring)
    call main
    
    # Return code from main() is in %eax.
    # Invoke OP_TASK_EXIT (13) via hardware syscall
    movslq %eax, %rdx
    movq $0x100000003, %rdi
    movq $13, %rsi
    syscall

    # Safe pause loop if syscall returns
1:
    pause
    jmp 1b

