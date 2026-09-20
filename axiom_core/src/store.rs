use std::collections::HashMap;
use crate::crypto::{CapabilityHash, Ed25519Signature};
use crate::capability::Capability;
use crate::validator::RejectReason;

#[derive(Debug, Clone)]
pub struct GenesisRoot {
    pub capability: Capability,
    pub root_signature: Ed25519Signature,
}

#[derive(Debug)]
pub struct Tombstone {
    pub revoked_at_epoch: u64,
    pub issuer_signature: Ed25519Signature,
}

pub struct CapabilityStore {
    pub genesis: GenesisRoot,
    pub capabilities: HashMap<CapabilityHash, Capability>,
    pub tombstones: HashMap<CapabilityHash, Tombstone>,
}

impl CapabilityStore {
    pub fn new(genesis: GenesisRoot) -> Self {
        let mut capabilities = HashMap::new();
        capabilities.insert(genesis.capability.identity_hash(), genesis.capability.clone());
        Self {
            genesis,
            capabilities,
            tombstones: HashMap::new(),
        }
    }

    /// DEPRECATED.
    /// In Axiom 0, we provide a raw insertion method for the test harness to construct reality.
    /// In production, this would only be written to by the kernel after a validated Epoch transaction.
    pub fn inject_for_test(&mut self, cap: Capability) -> CapabilityHash {
        let hash = cap.identity_hash();
        self.capabilities.insert(hash, cap);
        hash
    }
    /// Transactional issuance primitive. Replaces the `inject_for_test` escape hatch.
    /// Enforces capability physics before allowing authority to enter the graph.
    pub fn issue(&mut self, child: Capability) -> Result<CapabilityHash, RejectReason> {
        let parent = self.capabilities.get(&child.parent_hash)
            .ok_or(RejectReason::UnknownParent)?;

        if child.target_object != parent.target_object {
            return Err(RejectReason::AuthorityViolation);
        }

        if child.epoch_issued < parent.epoch_issued {
            return Err(RejectReason::EpochInvalid);
        }

        if (parent.authority_mask & AUTH_DELEGATE) == 0 {
            return Err(RejectReason::DelegationNotPermitted);
        }

        if (child.authority_mask & !parent.authority_mask) != 0 {
            return Err(RejectReason::AuthorityViolation);
        }

        let mut payload = b"AXIOM/CAPABILITY-ISSUANCE/V1".to_vec();
        payload.extend_from_slice(&child.identity_hash());

        if !verify_signature(&parent.owner_key, &payload, &child.issuer_signature) {
            return Err(RejectReason::InvalidIssuerSignature);
        }

        let hash = child.identity_hash();
        self.capabilities.insert(hash, child.clone());
        Ok(hash)
    }


    pub fn mark_tombstone(&mut self, hash: CapabilityHash, tombstone: Tombstone) {
        self.tombstones.insert(hash, tombstone);
    }
}
