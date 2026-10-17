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
    
    movq %rbp, %rsp
    popq %rbp
    ret
