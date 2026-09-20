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

### Phase 0: The Verifiable Object Machine (Current)
*Status: In Progress / Hardening*
- [x] Capability object hashing and DAG traversal.
- [x] Attenuation invariants and causal epoch ordering.
- [x] Silicon Root and Cryptographic Provenance.
- [x] Transactional `CapabilityStore` issuance API.
- [ ] Membrane limits (mutable state / invocation counters).

### Phase 1: Bare-Metal Transition
*Status: Planned*
- Custom Bootloader and hardware initialization.
- CPU setup, Interrupts, and Exceptions.
- Physical and Virtual Memory Management.
- Transitioning `axiom_core` to `no_std`.
- The Ring 0 Capability Kernel.

### Phase 2: System Foundations
*Status: Planned*
- **IPC:** Inter-process communication routed strictly through capability invocations.
- **VGOS (Verifiable Graph Object Store):** The Axiom filesystem. A persistent, versioned Merkle-DAG where file access is identical to capability access.
- **Network Domain (RCaps):** Treating network connections not as magical external memory, but as cryptographically secure bridges between capability domains.

### Phase 3: User Space & The Axiom Experience
*Status: Planned*
- **Observatory:** The native system debugger. A live visualizer for the capability graph, allowing real-time inspection of membranes, lineage, and authority propagation. 
- **AxSH:** The Axiom Shell.
- **Compositor & GUI:** A capability-secured display server where surface regions and input events are routed via membrane-constrained capabilities.
- **Package Manager:** Software distribution driven by capability signatures.

### Phase 4: Self-Hosting & Recovery
*Status: Planned*
- **State Collapse & Replay:** Treating corruption as an ontological paradox. The OS heals by collapsing corrupted state and replaying verifiable history from a known-good epoch.
- **Hot-Swapping:** Upgrading core components dynamically by shifting capability roots.
- Building Axiom OS on Axiom OS.