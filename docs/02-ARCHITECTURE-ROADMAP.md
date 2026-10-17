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

### Phase 5: Hardware Fast-Path Syscall Gateway, CDT & Microkernel Freeze
*Status: Completed*
- [x] Hardware `syscall`/`sysretq` MSR programming (`IA32_EFER`, `STAR`, `LSTAR`, `SFMASK`, `KERNEL_GS_BASE`).
- [x] Atomic kernel stack isolation via `swapgs` and per-CPU `CpuLocal` state.
- [x] Full Capability Derivation Tree (CDT) with rights monotonicity enforcement ($R_{\text{child}} \subseteq R_{\text{parent}}$).
- [x] Recursive transitive revocation (`OP_REVOKE`) and slot deletion (`OP_DELETE`).
- [x] Synchronous atomic RPC rendezvous (`OP_CALL`) with direct timeslice donation (`enqueue_front`).
- [x] Microkernel security hardening: Spectre v1 speculation barrier (`lfence`), SYSRET non-canonical check (CVE-2012-0217 mitigation), user segment reload (`0x1B`), scratch register zero-leak wipe.
- [x] High-resolution 1280x720 16:9 widescreen GOP graphics integration.
- [x] Microkernel boundary formally frozen: Ring 0 locked; all subsequent OS services run in Ring 3.

### Phase 6: The Genesis Process (PID 1) & `libaxiom` Capability Runtime
*Status: Completed*
- [x] De-scoped legacy `axiom_core` to `archive/axiom_core`.
- [x] Engineered `libaxiom`: sovereign user-space capability runtime crate (typed caps, syscall gateway, BumpAllocator, sync IPC).
- [x] Built `axiom_genesis`: Ring 3 root user task independently linked at virtual address `0x400000`.
- [x] Multi-page dynamic ELF binary loader in `main_prod.rs` mapping arbitrary user binary sizes.
- [x] Genesis Root CSpace configured: `ROOT_SLOT_CNODE=1`, `VSPACE=2`, `TCB=3`, `CONSOLE=5`.
- [x] Verified full Ring 3 user capability cycle: 54/54 automated boot selftests passing in emerald green.

### Phase 7: Sovereign User-Space OS & Capability Subsystems
*Status: In Progress*
- **Phase 7A: Sovereign Screen Takeover & High-Resolution Console Canvas** *(Completed)*
  - [x] Framebuffer capability operation (`OP_CONSOLE_BANNER = 8`) on Slot 5.
  - [x] 1280x28 obsidian status bar (`#161B22`) with electric cyan accent line (`#00D2FF`) on 1280x720 16:9 canvas.
  - [x] User-space capability-backed print pipeline (`libaxiom::console::ConsoleWriter`).
  - [x] Clean screen takeover clearing boot selftest logs and presenting sovereign `root@axiom:~#` prompt.
- **Phase 7B: Sovereign Capability Filesystem ("VaultFS" / "CapFS"), PS/2 Keyboard Driver & Interactive Shell** *(Current Milestone)*
  - [ ] PS/2 Keyboard interrupt driver: IRQ 1 routed to capability endpoint for non-blocking stdin keystrokes.
  - [ ] Capability Filesystem ("VaultFS"): Zero global ambient root `/`, directories and files as object capability endpoints.
  - [ ] Per-process sovereign namespaces: processes only see directories delegated to them in CSpace.
  - [ ] User accounts as Authority Keyrings: cryptographic capability capsules instead of flat `UID 0` god-mode.
  - [ ] In-memory RamFS (`libaxiom/fs`) with standard operations (`LOOKUP`, `READ`, `WRITE`, `CREATE`, `LIST`).
  - [ ] Interactive Genesis root shell commands (`help`, `ls`, `cat`, `echo`, `clear`, `status`, `reboot`).
- **Phase 7C: Dynamic Program Spawner & Block Storage Driver** *(Planned)*
  - [ ] Untyped memory retyping from Ring 3 to spawn child worker processes.
  - [ ] AHCI / NVMe / FAT32 persistent storage driver mounted via capability endpoint.
  - [ ] Simple C / shell script command runner.
