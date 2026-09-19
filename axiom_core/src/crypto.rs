use sha2::{Digest, Sha256};
use ed25519_dalek::{Signature, VerifyingKey, Verifier};

/// The special parent hash used by the Genesis capability.
pub const GENESIS_HASH: CapabilityHash = [0u8; 32];

/// Cryptographic identity of an Axiom capability.
pub type CapabilityHash = [u8; 32];

/// Ed25519 signature used by Axiom.
pub type Ed25519Signature = Signature;

/// Ed25519 public key used by Axiom identities.
pub type Ed25519PublicKey = VerifyingKey;

/// Compute SHA-256 over arbitrary bytes.
pub fn compute_hash(data: &[u8]) -> CapabilityHash {
    let mut hasher = Sha256::new();
    hasher.update(data);

    let result = hasher.finalize();

    let mut hash = [0u8; 32];
    hash.copy_from_slice(&result);

    hash
}

/// Verify an Ed25519 signature.
pub fn verify_signature(
    public_key: &Ed25519PublicKey,
    message: &[u8],
    signature: &Ed25519Signature,
) -> bool {
    public_key.verify(message, signature).is_ok()
}
