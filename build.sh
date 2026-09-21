cargo build -p axiom_kernel --target x86_64-unknown-none


cargo objcopy -p axiom_kernel --target x86_64-unknown-none -- \
    -O binary \
    target/x86_64-unknown-none/debug/axiom_kernel.bin
    

echo "[BUILD] Assembling Stage 1..."
nasm -f bin bootloader/boot.asm -o bootloader/boot.bin
STAGE1_SIZE=$(stat -c%s bootloader/boot.bin)
if [ "$STAGE1_SIZE" -ne 512 ]; then
    echo "[ERROR] Stage 1 is $STAGE1_SIZE bytes (must be exactly 512)"
    exit 1
fi

echo "[BUILD] Assembling Stage 2..."
nasm -f bin bootloader/stage2.asm -o bootloader/stage2.bin
STAGE2_SIZE=$(stat -c%s bootloader/stage2.bin)
if [ "$STAGE2_SIZE" -ne 4096 ]; then
    echo "[ERROR] Stage 2 is $STAGE2_SIZE bytes (must be exactly 4096)"
    exit 1
fi

cat bootloader/boot.bin \
    bootloader/stage2.bin \
    target/x86_64-unknown-none/debug/axiom_kernel.bin \
    > target/disk.img
#cat bootloader/boot.bin target/x86_64-unknown-none/debug/axiom_kernel.bin > target/disk.img

echo "[OK] Axiom Image forged successfully."

echo "[RUN] Starting AxiomOS."
#qemu-system-x86_64 -drive format=raw,file=bootloader/boot.bin
qemu-system-x86_64 -drive format=raw,file=target/disk.img

