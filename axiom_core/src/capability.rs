use crate::crypto::{CapabilityHash, Ed25519Signature, Ed25519PublicKey, compute_hash};

pub const AUTH_READ: u64  = 1 << 0;
pub const AUTH_WRITE: u64 = 1 << 1;
pub const AUTH_DELEGATE: u64 = 1 << 2;

#[derive(Debug, Clone)]
pub struct Membrane {
    pub expires_at_epoch: Option<u64>,
    pub max_invocations: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CapabilityRef {
    pub hash: CapabilityHash,
}

#[derive(Debug, Clone)]
pub struct Capability {
    pub target_object: CapabilityHash,
    pub authority_mask: u64,
    pub parent_hash: CapabilityHash,
    pub membrane: Option<Membrane>,
    pub epoch_issued: u64,
    
    pub owner_key: Ed25519PublicKey,        // NEW: The cryptographic identity allowed to invoke this
    pub issuer_signature: Ed25519Signature, // The signature of the parent capability's owner
}

impl Capability {
    pub fn identity_hash(&self) -> CapabilityHash {
        // In a full implementation, this deterministically serializes the struct.
        // For Axiom 0's proof, we hash a unique concatenation of its core fields.
        let mut payload = Vec::new();
        payload.extend_from_slice(&self.target_object);
        payload.extend_from_slice(&self.authority_mask.to_le_bytes());
        payload.extend_from_slice(&self.parent_hash);
        payload.extend_from_slice(self.owner_key.as_bytes());
        payload.extend_from_slice(&self.epoch_issued.to_le_bytes())

        match &self.membrane {
            Some(membrane) => {
                payload.push(1);

                match membrane.expires_at_epoch {
                    Some(epoch) => {
                        payload.push(1);
                        payload.extend_from_slice(&epoch.to_le_bytes());
                    }
                    None => payload.push(0),
                }

                match membrane.max_invocations {
                    Some(max) => {
                        payload.push(1);
                        payload.extend_from_slice(&max.to_le_bytes());
                    }
                    None => payload.push(0),
                }
            }

            None => {
                payload.push(0);
            }
        }
        
        compute_hash(&payload)
    }
}

#[derive(Debug, Clone)]
pub struct Invocation {
    pub capability: CapabilityRef,
    pub operation: u64,
    pub nonce: u64,
    pub invocation_signature: Ed25519Signature,
}
