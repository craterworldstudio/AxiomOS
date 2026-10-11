#!/bin/bash
set -e

# ============================================================================
# Axiom Build System (UEFI & BIOS Dual-Mode)
#
#   ./build.sh        — Dev build  [UEFI + High-Res GOP Display] (default)
#   ./build.sh -prod  — Prod build [UEFI + Full Self-Tests]
#   ./build.sh -bios  — Legacy BIOS build (MBR + Stage 2)
# ============================================================================

PROD_MODE=0
BIOS_MODE=0

for arg in "$@"; do
    case "$arg" in
        -prod) PROD_MODE=1 ;;
        -bios) BIOS_MODE=1 ;;
        *) echo "[WARN] Unknown flag: $arg" ;;
    esac
done

if [ "$PROD_MODE" -eq 1 ]; then
    echo "[BUILD] Environment: PRODUCTION (main_prod.rs)"
    CARGO_FLAGS="--features axiom_kernel/prod-mode"
else
    echo "[BUILD] Environment: DEVELOPMENT (main_dev.rs)"
    CARGO_FLAGS=""
fi

echo "[BUILD] Compiling Axiom Kernel..."
cargo build -p axiom_kernel $CARGO_FLAGS

echo "[BUILD] Flattening Kernel Binary..."
objcopy -O binary target/x86_64-unknown-none/debug/axiom_kernel target/x86_64-unknown-none/debug/axiom_kernel.bin

KERNEL_BYTES=$(stat -c%s target/x86_64-unknown-none/debug/axiom_kernel.bin)
echo "[BUILD] Kernel size: $KERNEL_BYTES bytes"

# ============================================================================
# TARGET 1: PURE UEFI BOOTLOADER (DEFAULT)
# ============================================================================
if [ "$BIOS_MODE" -eq 0 ]; then
    echo "[BUILD] Compiling Sovereign UEFI Bootloader (axiom_boot)..."
    cargo build -p axiom_boot --target x86_64-unknown-uefi

    echo "[BUILD] Packaging FAT32 UEFI Disk Image..."
    mkdir -p target/esp/EFI/BOOT
    cp target/x86_64-unknown-uefi/debug/axiom_boot.efi target/esp/EFI/BOOT/BOOTX64.EFI
    cp target/x86_64-unknown-none/debug/axiom_kernel.bin target/esp/axiom_kernel.bin

    UEFI_IMG="target/uefi_disk.img"
    dd if=/dev/zero of="$UEFI_IMG" bs=1M count=64 status=none
    mkfs.vfat -F 32 "$UEFI_IMG" >/dev/null

    mmd -i "$UEFI_IMG" ::EFI 2>/dev/null || true
    mmd -i "$UEFI_IMG" ::EFI/BOOT 2>/dev/null || true
    mcopy -i "$UEFI_IMG" target/esp/EFI/BOOT/BOOTX64.EFI ::EFI/BOOT/BOOTX64.EFI
    mcopy -i "$UEFI_IMG" target/esp/axiom_kernel.bin ::axiom_kernel.bin

    # Auto-detect OVMF firmware
    OVMF_PATH=""
    for p in /usr/share/edk2/x64/OVMF.4m.fd /usr/share/ovmf/x64/OVMF.fd /usr/share/ovmf/OVMF.fd /usr/share/edk2-ovmf/x64/OVMF.fd; do
        if [ -f "$p" ]; then
            OVMF_PATH="$p"
            break
        fi
    done

    if [ -z "$OVMF_PATH" ]; then
        echo "[ERROR] OVMF firmware not found. Install edk2-ovmf or run with -bios."
        exit 1
    fi

    echo "[OK] Axiom UEFI Image forged successfully ($UEFI_IMG)."
    echo "[RUN] Starting AxiomOS under UEFI (Firmware: $OVMF_PATH)..."
    qemu-system-x86_64 \
        -bios "$OVMF_PATH" \
        -drive format=raw,file="$UEFI_IMG" \
        -m 512M

# ============================================================================
# TARGET 2: LEGACY BIOS BOOTLOADER (OPTIONAL FALLBACK)
# ============================================================================
else
    KERNEL_SECTORS=$(( (KERNEL_BYTES + 511) / 512 ))
    echo "[BUILD] Assembling Legacy BIOS Stage 1..."
    nasm -f bin bootloader/boot.asm -o bootloader/boot.bin

    echo "[BUILD] Assembling Legacy BIOS Stage 2..."
    nasm -f bin bootloader/stage2.asm -DKERNEL_SECTORS=$KERNEL_SECTORS -o bootloader/stage2.bin

    echo "[BUILD] Forging unified legacy disk image..."
    cat bootloader/boot.bin bootloader/stage2.bin target/x86_64-unknown-none/debug/axiom_kernel.bin > target/disk.img
    dd if=/dev/zero bs=512 count=64 >> target/disk.img 2>/dev/null

    echo "[OK] Legacy BIOS Image forged successfully."
    echo "[RUN] Starting AxiomOS under Legacy BIOS..."
    qemu-system-x86_64 -drive format=raw,file=target/disk.img
fi
