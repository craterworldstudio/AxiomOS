use sha2::{Sha256, Digest};
use ed25519_dalek::{Signature, VerifyingKey, Signer, Verifier};

// A fixed array of 32 zero-bytes represents the Genesis Hash root.
pub const GENESIS_HASH: CapabilityHash = [0u8; 32];

pub type CapabilityHash = [u8; 32];
pub type Ed25519Signature = Signature;
pub type Ed25519PublicKey = VerifyingKey;

pub fn compute_hash(data: &[u8]) -> CapabilityHash {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&result);
    hash
}

pub fn verify_signature(
    public_key: &Ed25519PublicKey, 
    message: &[u8], 
    signature: &Ed25519Signature
) -> bool {
    public_key.verify(message, signature).is_ok()
}
