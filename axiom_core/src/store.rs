use std::collections::HashMap;
use crate::crypto::{CapabilityHash, Ed25519Signature};
use crate::capability::Capability;

#[derive(Debug)]
pub struct Tombstone {
    pub revoked_at_epoch: u64,
    pub issuer_signature: Ed25519Signature,
}

pub struct CapabilityStore {
    pub capabilities: HashMap<CapabilityHash, Capability>,
    pub tombstones: HashMap<CapabilityHash, Tombstone>,
}

impl CapabilityStore {
    pub fn new() -> Self {
        Self {
            capabilities: HashMap::new(),
            tombstones: HashMap::new(),
        }
    }

    /// In Axiom 0, we provide a raw insertion method for the test harness to construct reality.
    /// In production, this would only be written to by the kernel after a validated Epoch transaction.
    pub fn inject_for_test(&mut self, cap: Capability) -> CapabilityHash {
        let hash = cap.identity_hash();
        self.capabilities.insert(hash, cap);
        hash
    }

    pub fn mark_tombstone(&mut self, hash: CapabilityHash, tombstone: Tombstone) {
        self.tombstones.insert(hash, tombstone);
    }
}
