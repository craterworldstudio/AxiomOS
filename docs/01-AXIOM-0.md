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
