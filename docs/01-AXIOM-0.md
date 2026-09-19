# Axiom 0: The Verifiable Object Machine

## Objective
Establish the minimal viable runtime capable of enforcing the Axiom capability physics. Axiom 0 has no user interface, no persistent filesystem, and no network stack. It exists purely in memory to prove that the Merkle-DAG capability model functions.

## Scope of Axiom 0
1. **Genesis:** Generate a throwaway `HardwareRootCap` on boot.
2. **Object Creation:** Allocate an isolated memory object.
3. **Delegation:** Generate a new capability derived from a parent, appending to the DAG.
4. **Attenuation (Membranes):** Wrap a capability in a mathematically strict constraint (e.g., Read-Only, Expiry-Tick).
5. **Invocation:** Request an operation against an object using a capability.
6. **Validation:** The core loop that traverses the DAG, verifies signatures, and applies Membrane constraints before allowing execution.
7. **Revocation:** Issue a tombstone and prove that subsequent invocations mathematically fail.

## Non-Goals
* Persistence (VGOS comes in Axiom 1).
* Multi-threading (Keep the validator deterministic for now).
* Human interaction (Tests will be driven by an automated test harness).

## The Mathematical Invariant of Authority
Axiom's security model relies on a single, mathematically verifiable invariant enforced during every capability invocation:
> *Authority monotonically decreases along a delegation edge, unless an explicitly defined, mathematically proven authority transformation exists.*

If `Capability A` delegates to `Capability B`, `B` may attenuate (reduce) the authority it grants, but it can never expand it. A read-only capability cannot spawn a write-capable child. The validator enforces this via bitwise subset verification (`child.mask & !parent.mask == 0`) and Merkle-DAG traversal. 

## ADR-0001: Capabilities Are References
In legacy systems like JWTs, the caller provides the token containing the claims. In Axiom, **Capabilities are References**. The invocation carries only the `CapabilityHash`. The kernel resolves this reference against its immutable `CapabilityStore`. 
* **Benefit:** Eliminates caller-side forgery of capability parameters. The capability store becomes part of the validated state rather than trusting caller-provided authority payloads.

## The Test Suite
Axiom 0 correctness is defined by its ability to yield a deterministic `ValidationResult` across the following cryptographic vectors:
* `TEST 01 - 02`: Genesis and Valid Delegation -> `VALID`
* `TEST 03`: Delegation granting more authority -> `REJECTED (AuthorityViolation)`
* `TEST 04 - 06`: Forged Parent/Signature/Unknown -> `REJECTED`
* `TEST 07`: Capability Cycle (A -> B -> A) -> `REJECTED (CycleDetected)`
* `TEST 08 - 09`: Revoked Capability or Ancestor -> `REJECTED (Revoked)`
* `TEST 10 - 11`: Membrane Violations -> `REJECTED (MembraneViolation)`
* `TEST 12`: Replayed Nonce -> `REJECTED (InvalidNonce)`
