use std::collections::HashMap;
use crate::crypto::{CapabilityHash, Ed25519Signature};
use crate::capability::{Capability, RejectReason, verify_delegation};

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
    genesis: GenesisRoot,
    capabilities: HashMap<CapabilityHash, Capability>,
    tombstones: HashMap<CapabilityHash, Tombstone>,
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
    pub fn get_capability(&self, hash: &CapabilityHash) -> Option<&Capability> {
        self.capabilities.get(hash)
    }

    pub fn get_genesis(&self) -> &GenesisRoot {
        &self.genesis
    }

    pub fn is_revoked(&self, hash: &CapabilityHash) -> bool {
        self.tombstones.contains_key(hash)
    }

    /// Transactional issuance primitive. Replaces the `inject_for_test` escape hatch.
    /// Enforces capability physics before allowing authority to enter the graph.
    pub fn issue(&mut self, child: Capability) -> Result<CapabilityHash, RejectReason> {
        let parent = self.capabilities.get(&child.parent_hash)
            .ok_or(RejectReason::UnknownParent)?;

        if self.is_revoked(&child.parent_hash) {
            return Err(RejectReason::Revoked);
        }

        verify_delegation(&child, parent)?;

        let hash = child.identity_hash();
        self.capabilities.insert(hash, child.clone());
        Ok(hash)
    }


    pub fn mark_tombstone(&mut self, hash: CapabilityHash, tombstone: Tombstone) {
        self.tombstones.insert(hash, tombstone);
    }








    /// DEPRECATED.
    /// In Axiom 0, we provide a raw insertion method for the test harness to construct reality.
    /// In production, this would only be written to by the kernel after a validated Epoch transaction.
    pub fn inject_for_test(&mut self, cap: Capability) -> CapabilityHash {
        let hash = cap.identity_hash();
        self.capabilities.insert(hash, cap);
        hash
    }

    #[cfg(test)]
    pub(crate) fn corrupt_capability_for_test(
        &mut self,
        hash: CapabilityHash,
        capability: Capability,
    ) {
        self.capabilities.insert(hash, capability);
    }
    
    #[cfg(test)]
    pub(crate) fn corrupt_genesis_for_test<F>(&mut self, mutate: F)
    where
        F: FnOnce(&mut Capability),
    {
        mutate(&mut self.genesis.capability);
    }

}