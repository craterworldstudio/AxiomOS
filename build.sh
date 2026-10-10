#!/bin/bash
set -e

# ============================================================================
# Axiom Build System
#
#   ./build.sh        — Dev build   (main_dev.rs)  [default]
#   ./build.sh -prod  — Prod build  (main_prod.rs)
# ============================================================================

PROD_MODE=0
for arg in "$@"; do
    case "$arg" in
        -prod) PROD_MODE=1 ;;
        *) echo "[WARN] Unknown flag: $arg"; ;;
    esac
done

if [ "$PROD_MODE" -eq 1 ]; then
    echo "[BUILD] Mode: PRODUCTION (main_prod.rs)"
    CARGO_FLAGS="--features axiom_kernel/prod-mode"
else
    echo "[BUILD] Mode: DEVELOPMENT (main_dev.rs)"
    CARGO_FLAGS=""
fi

echo "[BUILD] Compiling Rust Kernel..."
cargo build -p axiom_kernel $CARGO_FLAGS

echo "[BUILD] Flattening Kernel Binary..."
objcopy -O binary target/x86_64-unknown-none/debug/axiom_kernel target/x86_64-unknown-none/debug/axiom_kernel.bin

# --- AUTO-SCALING LOGIC ---
KERNEL_BYTES=$(stat -c%s target/x86_64-unknown-none/debug/axiom_kernel.bin)
KERNEL_SECTORS=$(( (KERNEL_BYTES + 511) / 512 ))
echo "[BUILD] Kernel size: $KERNEL_BYTES bytes ($KERNEL_SECTORS sectors)"

echo "[BUILD] Assembling Stage 1..."
nasm -f bin bootloader/boot.asm -o bootloader/boot.bin
STAGE1_SIZE=$(stat -c%s bootloader/boot.bin)
if [ "$STAGE1_SIZE" -ne 512 ]; then
    echo "[ERROR] Stage 1 is $STAGE1_SIZE bytes (must be exactly 512)"
    exit 1
fi

echo "[BUILD] Assembling Stage 2..."
# Inject the dynamic sector count into NASM using -D
nasm -f bin bootloader/stage2.asm -DKERNEL_SECTORS=$KERNEL_SECTORS -o bootloader/stage2.bin
STAGE2_SIZE=$(stat -c%s bootloader/stage2.bin)
if [ "$STAGE2_SIZE" -ne 4096 ]; then
    echo "[ERROR] Stage 2 is $STAGE2_SIZE bytes (must be exactly 4096)"
    exit 1
fi

echo "[BUILD] Forging unified disk image..."
cat bootloader/boot.bin bootloader/stage2.bin target/x86_64-unknown-none/debug/axiom_kernel.bin > target/disk.img

echo "[BUILD] Padding disk image to prevent EOF read errors..."
dd if=/dev/zero bs=512 count=64 >> target/disk.img 2>/dev/null

echo "[OK] Axiom Image forged successfully."
echo "[RUN] Starting AxiomOS."
qemu-system-x86_64 -drive format=raw,file=target/disk.img