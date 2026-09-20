cargo build -p axiom_kernel --target x86_64-unknown-none


cargo objcopy -p axiom_kernel --target x86_64-unknown-none -- \
    -O binary \
    target/x86_64-unknown-none/debug/axiom_kernel.bin
    
nasm -f bin bootloader/boot.asm -o bootloader/boot.bin
nasm -f bin bootloader/stage2.asm -o bootloader/stage2.bin

cat bootloader/boot.bin \
    bootloader/stage2.bin \
    target/x86_64-unknown-none/debug/axiom_kernel.bin \
    > target/disk.img
#cat bootloader/boot.bin target/x86_64-unknown-none/debug/axiom_kernel.bin > target/disk.img

#qemu-system-x86_64 -drive format=raw,file=bootloader/boot.bin
qemu-system-x86_64 -drive format=raw,file=target/disk.img
