# Axiom 1: Bare-Metal Substrate

## 1. The Dual-Model Architecture
Axiom 1 introduces the physical reality of the operating system. We explicitly reject the approach of throwing out our laboratory model to start writing kernel code. Axiom maintains a strict separation between the definition of its physics and the execution on hardware.

* **`axiom_core` (The Oracle):** The current `std`-hosted reference implementation of Axiom 0 capability semantics.
* **`axiom_kernel` (The Substrate):** The current `#![no_std]` bare-metal x86_64 kernel environment.
* **Future `axiom_model`:** A planned `#![no_std]` extraction of the architecture-independent capability physics shared by the oracle and kernel.

## 2. Kernel Binary Contract
The Rust compiler natively produces ELF binaries designed for host operating systems (ASLR, PIE, dynamic linking). Axiom 1 rejects this. A bootloader requires a predictable, deterministic payload.

* **Format:** Raw, flat machine code (extracted via `llvm-objcopy` from the ELF).
* **Relocation:** Statically linked. Zero dynamic relocations.
* **Link Address:** `0x100000` (The 1MB mark). 
* **Entry Point:** The `_start` symbol is guaranteed to be placed at the absolute beginning of the `.text` segment, meaning `0x100000` is the exact jump target.
* **Stack:** The kernel expects the bootloader to establish a valid 64-bit stack prior to the handoff.

## 3. The Boot Architecture
Axiom boots via a legacy BIOS fallback process designed for maximum control and minimal magic. 

### Boot Image Layout (`disk.img`)
* **Sector 0:** Stage 1 Boot Sector (512 bytes).
* **Sector 1..N:** Stage 2 Bootloader.
* **Sector N+1..M:** Axiom Kernel payload.

### The Escalation Sequence
1. **16-bit Real Mode (Stage 1)**
   * BIOS loads Sector 0 to physical address `0x7C00`.
   * Stage 1 initializes the real-mode environment.
   * Stage 1 loads Stage 2 into low memory and transfers control to it.

2. **16-bit Real Mode (Stage 2)**
   * Stage 2 loads the kernel payload into a low-memory staging area.
   * Stage 2 prepares the CPU transition.

3. **32-bit Protected Mode**
   * Load the Global Descriptor Table (GDT).
   * Enable protected mode.

4. **64-bit Long Mode & Handoff**
   * Build the 4-level identity page tables.
   * Enable Physical Address Extension (PAE) and Long Mode (LME).
   * Enable paging.
   * Establish the 64-bit stack.
   * Copy the staged kernel to `0x100000`.
   * Execute a Long Jump to the kernel entry point (`0x100000`).

## 4. Axiom 1 Milestones

### Pre-Boot Substrate
- [x] Rust kernel compiles for `x86_64-unknown-none`.
- [x] Kernel linked deterministically at `0x100000`.
- [x] Kernel flat binary produced with zero dynamic relocations.
- [x] BIOS boot sector executes and prints confirmation.

### The Hardware Escalation
- [ ] Stage 1 reads Stage 2 from disk.
- [ ] Stage 2 reads kernel into a low-memory staging area.
- [ ] Enter 32-bit Protected Mode.
- [ ] Establish 4-level identity page tables.
- [ ] Enter 64-bit Long Mode.
- [ ] Relocate kernel to `0x100000` and establish the 64-bit stack.
- [ ] Execute Long Jump to Rust kernel `_start`.
- [ ] Rust kernel seizes the VGA buffer and prints confirmation.

### Kernel Foundations
- [ ] Establish physical memory model (Frame Allocator).
- [ ] Establish CPU exception handlers and Interrupt Descriptor Table (IDT).