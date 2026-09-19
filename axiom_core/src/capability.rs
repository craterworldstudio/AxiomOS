// axiom_core/src/capability.rs

use crypto::hash::Sha256Hash;
use crypto::signature::Ed25519Signature;

/// A globally unique identifier for an Axiom Object
pub type ObjectId = Sha256Hash;

/// The mathematical representation of Authority in Axiom.
/// This is a node in the Merkle-DAG.
#[derive(Debug, Clone)]
pub struct Capability {
    pub target_object: ObjectId,
    pub authority_mask: u64,       // Bitmask of allowed operations
    pub parent_hash: Sha256Hash,   // Link to the delegating capability
    pub membrane: Option<Membrane>,
    pub epoch_issued: u64,
    pub issuer_signature: Ed25519Signature, 
}

impl Capability {
    /// Cryptographic hash of this specific capability state
    pub fn identity_hash(&self) -> Sha256Hash {
        // Hashes the target, mask, parent, membrane, and epoch
        crypto::hash::compute(self)
    }
}

/// A constraint applied to a delegated capability.
#[derive(Debug, Clone)]
pub struct Membrane {
    pub max_invocations: Option<u32>,
    pub expires_at_epoch: Option<u64>,
    pub custom_bpf_filter: Option<Vec<u8>>, // Compiled eBPF constraint logic
}

/// The payload sent when attempting to exercise a capability.
pub struct Invocation {
    pub target_cap: Capability,
    pub operation: u64,
    pub nonce: u64,
    pub invocation_signature: Ed25519Signature,
}
