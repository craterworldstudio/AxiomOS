use std::collections::HashSet;
use crate::capability::{CapabilityStore, CapabilityHash, Capability, Invocation};
use crate::crypto::{GENESIS_HASH, Ed25519Signature};

pub enum ValidationResult {
    Valid,
    Rejected(RejectReason),
}

#[derive(Debug, PartialEq)]
pub enum RejectReason {
    CapabilityNotFound,
    CapabilityHashMismatch,
    InvalidSignature,
    UnknownParent,
    CycleDetected,
    AuthorityViolation,
    Revoked,
    EpochInvalid,
    MembraneViolation,
    InvalidNonce,
}

pub struct Validator<'a> {
    store: &'a CapabilityStore,
    current_epoch: u64,
    consumed_nonces: &'a mut HashSet<(CapabilityHash, u64)>, // Ephemeral for Axiom 0
}

impl<'a> Validator<'a> {
    pub fn validate(&mut self, invocation: &Invocation) -> ValidationResult {
        // 1. Resolve capability reference
        let cap = match self.store.capabilities.get(&invocation.capability) {
            Some(cap) => cap,
            None => return ValidationResult::Rejected(RejectReason::CapabilityNotFound),
        };

        // 2. Anti-replay check
        let nonce_tuple = (invocation.capability, invocation.nonce);
        if !self.consumed_nonces.insert(nonce_tuple) {
            return ValidationResult::Rejected(RejectReason::InvalidNonce);
        }

        // 3. Cryptographic identity check
        if cap.identity_hash() != invocation.capability {
            return ValidationResult::Rejected(RejectReason::CapabilityHashMismatch);
        }

        // 4. Temporal constraints
        if cap.epoch_issued > self.current_epoch {
            return ValidationResult::Rejected(RejectReason::EpochInvalid);
        }

        // 5. Evaluate basic membranes
        if let Some(membrane) = &cap.membrane {
            if let Some(expiry) = membrane.expires_at_epoch {
                if self.current_epoch > expiry {
                    return ValidationResult::Rejected(RejectReason::MembraneViolation);
                }
            }
            // In Axiom 0, MaxInvocations would be tracked in the Store's mutable state
            // outside this pure validation loop, updated upon ValidationResult::Valid.
        }

        // 6. Verify Lineage and Revocation
        if let Err(reason) = self.verify_lineage(cap) {
            return ValidationResult::Rejected(reason);
        }

        // 7. Verify Invocation Signature (Caller actually owns the capability)
        if !self.verify_invocation_signature(invocation, cap) {
            return ValidationResult::Rejected(RejectReason::InvalidSignature);
        }

        ValidationResult::Valid
    }

    fn verify_lineage(&self, starting: &Capability) -> Result<(), RejectReason> {
        let mut current = starting;
        let mut visited = HashSet::new();

        loop {
            let hash = current.identity_hash();

            if !visited.insert(hash) {
                return Err(RejectReason::CycleDetected);
            }

            // Check if current or any ancestor is tombstoned
            if self.store.tombstones.contains_key(&hash) {
                return Err(RejectReason::Revoked);
            }

            if current.parent_hash == GENESIS_HASH {
                // In a complete implementation, verify Genesis signature against HardwareRootKey
                return Ok(());
            }

            let parent = self.store.capabilities
                .get(&current.parent_hash)
                .ok_or(RejectReason::UnknownParent)?;

            self.verify_delegation(current, parent)?;

            current = parent;
        }
    }

    fn verify_delegation(&self, child: &Capability, parent: &Capability) -> Result<(), RejectReason> {
        if child.target_object != parent.target_object {
            return Err(RejectReason::AuthorityViolation);
        }

        // Bitwise check: Child cannot possess bits that the Parent lacks.
        if (child.authority_mask & !parent.authority_mask) != 0 {
            return Err(RejectReason::AuthorityViolation);
        }

        Ok(())
    }

    fn verify_invocation_signature(&self, _invocation: &Invocation, _cap: &Capability) -> bool {
        // Assume Ed25519 verification succeeds for Axiom 0 scaffolding
        true 
    }
}
