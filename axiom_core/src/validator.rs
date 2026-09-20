use std::collections::HashSet;
use crate::capability::{Capability, Invocation, AUTH_DELEGATE};
use crate::crypto::{Ed25519PublicKey, GENESIS_HASH};
use crate::store::CapabilityStore;

#[derive(Debug, PartialEq)]
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
    DelegationNotPermitted,
    AuthorityViolation,
    Revoked,
    EpochInvalid,
    MembraneViolation,
    InvalidNonce,
    InvalidIssuerSignature,
    GenesisMismatch,
}

pub struct Validator<'a> {
    store: &'a CapabilityStore,
    current_epoch: u64,
    consumed_nonces: &'a mut HashSet<([u8; 32], u64)>, // Ephemeral for Axiom 0
    hardware_root_key: &'a Ed25519PublicKey,
}

impl<'a> Validator<'a> {
    pub fn new( 
        store: &'a CapabilityStore, 
        current_epoch: u64,
        consumed_nonces: &'a mut HashSet<([u8; 32], u64)>, 
        hardware_root_key: &'a Ed25519PublicKey,
    ) -> Self {
        Self { store, current_epoch, consumed_nonces, hardware_root_key, }
    }
    
    pub fn validate(&mut self, invocation: &Invocation) -> ValidationResult {
        // 1. Resolve capability reference

        let capability_hash = invocation.capability.hash;
        let cap = match self.store.capabilities.get(&capability_hash) {
            Some(cap) => cap,
            None => return ValidationResult::Rejected(RejectReason::CapabilityNotFound),
                };

        // 2. Anti-replay check
        let nonce_tuple = (capability_hash, invocation.nonce);
        if self.consumed_nonces.contains(&nonce_tuple) {
            return ValidationResult::Rejected(RejectReason::InvalidNonce);
        }

        // 3. Cryptographic identity check
        if cap.identity_hash() != capability_hash {
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
                    return ValidationResult::Rejected(RejectReason::MembraneViolation, );
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

        let _ = self.consumed_nonces.insert(nonce_tuple);

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
                return self.verify_genesis(current);
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
        
        if child.epoch_issued < parent.epoch_issued {
            return Err(RejectReason::EpochInvalid);
        }

        if (parent.authority_mask & AUTH_DELEGATE) == 0 {
            return Err(RejectReason::DelegationNotPermitted);
        }    
        // Bitwise check: Child cannot possess bits that the Parent lacks.
        if (child.authority_mask & !parent.authority_mask) != 0 {
            return Err(RejectReason::AuthorityViolation);
        }

        let mut payload = b"AXIOM/CAPABILITY-ISSUANCE/V1".to_vec();
        payload.extend_from_slice(&child.identity_hash());

        if !crate::crypto::verify_signature(&parent.owner_key, &payload, &child.issuer_signature) {
            return Err(RejectReason::InvalidIssuerSignature);
        }

        Ok(())
    }


    fn verify_genesis(&self, genesis: &Capability) -> Result<(), RejectReason> {
        if genesis.identity_hash() != self.store.genesis.capability.identity_hash() {
            return Err(RejectReason::GenesisMismatch);
        }

        if genesis.parent_hash != GENESIS_HASH {
            return Err(RejectReason::GenesisMismatch);
        }

        if genesis.owner_key.as_bytes() != self.hardware_root_key.as_bytes() {
            return Err(RejectReason::GenesisMismatch);
        }

        let mut payload = b"AXIOM/GENESIS/V1".to_vec();
        payload.extend_from_slice(&genesis.identity_hash());

        if !crate::crypto::verify_signature(
            self.hardware_root_key,
            &payload,
            &self.store.genesis.root_signature,
        ) {
            return Err(RejectReason::InvalidIssuerSignature);
        }

        Ok(())
    }

    fn verify_invocation_signature(&self, invocation: &Invocation, cap: &Capability) -> bool {
        // Reconstruct the exact byte payload the caller was required to sign:
        // [ capability_hash (32 bytes) | operation (8 bytes) | nonce (8 bytes) ]
        let mut payload = b"AXIOM/INVOCATION/V1".to_vec();
        payload.extend_from_slice(&invocation.capability.hash);
        payload.extend_from_slice(&invocation.operation.to_le_bytes());
        payload.extend_from_slice(&invocation.nonce.to_le_bytes());

        // Verify using the real crypto backend against the Capability's authorized owner
        crate::crypto::verify_signature(&cap.owner_key, &payload, &invocation.invocation_signature)
    }
}
