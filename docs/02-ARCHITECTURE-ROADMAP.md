# Axiom OS: Architecture & Roadmap

Axiom is a capability-based operating system where authority is not granted by user identity, but mathematically proven through a continuous, unforgeable cryptographic graph. Axiom does not manage resources; it manages cryptographic causality.

## 1. Core Architecture

### The Capability Model
Everything in Axiom is represented as an object accessed via unforgeable cryptographic capabilities. The system operates on a fundamental mathematical invariant: **Authority monotonically decreases along a delegation edge.** A child capability can never possess authority that its parent lacked.

### Cryptographic Provenance & The Silicon Root
Capabilities are not bearer tokens. They are bound to specific owner keys. The root of all authority is the **Silicon Root** (HardwareRootKey), established at boot. Every valid capability must prove its provenance by tracing a mathematically verifiable lineage of parental signatures directly back to this Genesis anchor. 

### Membranes
Capabilities can be attenuated not just by masking authority bits, but by applying Membranes. Membranes are strict constraints evaluated at invocation (e.g., temporal expiration, maximum invocation limits).

### The Validator
The Validator is a pure, deterministic engine. It evaluates invocations against the canonical `CapabilityStore`, verifying identity hashes, temporal monotonicity, capability lineage, parental issuance signatures, and finally, the caller's cryptographic proof of ownership. 

---

## 2. Kernel Strategy: The `std` → `no_std` Pipeline

Axiom is ultimately a bare-metal operating system. However, attempting to build a novel cryptographic security model simultaneously with a custom bootloader and memory manager is a recipe for failure. 

We are utilizing a **Laboratory-First Strategy**:
1. **The Laboratory (`std`):** Axiom 0 is built in standard Rust. This allows us to rapidly iterate, test, and mathematically prove the core capability physics, the Merkle-DAG validator, and the issuance logic using standard libraries and testing frameworks.
2. **The Bare-Metal Transition (`no_std`):** Once the cryptographic model is hardened and frozen, the core logic will be decoupled from the standard library. The proven capability validator will be dropped into a custom `no_std` kernel foundation.

---

## 3. Phased Roadmap

### Phase 1: Sovereign Boot & Architecture Substrate
*Status: Completed*
- [x] UEFI Bootloader (`axiom_boot`) with GOP high-resolution graphics and memory map passing.
- [x] Legacy BIOS Stage 1 & Stage 2 fallback bootloader.
- [x] GDT with 64-bit kernel and user descriptors, TSS with IST stacks.
- [x] IDT with exception handling, `#DF` on dedicated IST stack, `#PF` trap.
- [x] Physical memory bitmap allocator.

### Phase 2: Object Model & Capability Foundations
*Status: Completed*
- [x] Retype model: `Untyped` authority ledger partitioning physical memory.
- [x] Kernel object accounting: `TCB`, `Endpoint`, `CNode`, `VSpace`.
- [x] Generational capability slots and handle encoding `(slot, generation)`.
- [x] Intrusive FIFO wait queues for synchronous rendezvous IPC.

### Phase 3: Virtual Memory & Privilege Isolation (VSpace)
*Status: Completed*
- [x] Sovereign PML4 page table construction from physical memory.
- [x] Kernel supervisor huge page isolation (`0x0000_0000 - 0x001F_FFFF`, `U/S = 0`).
- [x] Dynamic CR3 address space switching per thread.
- [x] Ring 3 privilege boundary enforcement (`CPL = 3`).
- [x] `#PF` fault trap (Vector 14, error code `0x05`) with clean thread termination.

### Phase 4: Local APIC Timer, Preemption & Stress Hardening
*Status: Completed*
- [x] Local APIC detection via `IA32_APIC_BASE` MSR (`0x1B`) and legacy 8259 PIC masking.
- [x] PIT Channel 2 hardware calibration (10 ms countdown).
- [x] Periodic timer on Vector `0x20` with Spurious Vector `0xFF`.
- [x] Ring 3 preemption with safe Ring 0 deferral.
- [x] Dual-frame context switching (`FrameKind::Cooperative` vs `FrameKind::Interrupt`).
- [x] Dynamic `TSS.privilege_stack_table[0]` (`RSP0`) retargeting per thread.
- [x] 16-byte System V stack alignment and user segment selector restoration.
- [x] Stress suite: 50x syscalls under timer IRQ, 25+ preemption ticks, thread lifecycle/reuse, stack canaries.

### Phase 5: Capability System Call ABI (Next)
*Status: Upcoming*
- [ ] Hardware `syscall`/`sysret` MSR initialization (`STAR`, `LSTAR`, `SFMASK`).
- [ ] Fast-path capability invocation protocol.
- [ ] Capability Derivation Tree (CDT) and recursive revocation.
- [ ] Microkernel boundary freeze.

### Phase 6+: User-Space OS & Toolchain Execution
*Status: Planned*
- **Phase 6**: Root Task (`Init`), Embedded Ramdisk, & ELF64 Loader.
- **Phase 7**: Microkernel Services (GOP Terminal Server, PS/2 Keyboard, Storage/VFS).
- **Phase 8**: Authentication & Cryptographic Login Subsystem.
- **Phase 9**: The Sovereign Shell (AxSH), C/musl runtime, and Native C/ASM compilation.
