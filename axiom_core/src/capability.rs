use crypto::hash::Sha256Hash;
use crypto::signature::Ed25519Signature;

/// A globally unique identifier for an Axiom object.
pub type ObjectId = Sha256Hash;

/// The canonical identifier of a capability.
///
/// A capability is referenced by its cryptographic identity rather than
/// being supplied directly by an untrusted caller.
pub type CapabilityHash = Sha256Hash;

/// Authority bits.
///
/// Axiom 0 deliberately keeps the authority model small.
/// More sophisticated typed authorities can come later.
pub const AUTH_READ: u64 = 1 << 0;
pub const AUTH_WRITE: u64 = 1 << 1;
pub const AUTH_EXECUTE: u64 = 1 << 2;

/// A capability is a cryptographically verifiable delegation.
///
/// Authority may be attenuated through delegation, but a child capability
/// must never gain authority that its parent did not possess.
#[derive(Debug, Clone)]
pub struct Capability {
    
    pub target_object: ObjectId, /// Object this capability grants authority over.
    pub authority_mask: u64, /// Operations permitted by this capability.

    /// Cryptographic identity of the parent capability.
    ///
    /// `GENESIS_HASH` is used for the root capability.
    pub parent_hash: CapabilityHash,
    pub membrane: Option<Membrane>, /// Optional restrictions applied to this capability.    
    pub epoch_issued: u64, /// Epoch at which this capability was created.
    pub issuer_signature: Ed25519Signature, /// Signature proving that the issuer authorized this capability.
}

impl Capability {
    /// Calculate the canonical cryptographic identity of this capability.
    ///
    /// The hash covers every field that determines the capability's authority.
    pub fn identity_hash(&self) -> CapabilityHash {
        crypto::hash::compute(self)
    }
}

/// Restrictions applied to a capability.
///
/// Axiom 0 intentionally uses only deterministic, built-in constraints.
/// Programmable filters belong to a later version of Axiom.
#[derive(Debug, Clone)]
pub enum Membrane {
    ReadOnly, /// Capability may only perform read operations.
    ExpiresAt(u64), /// Capability becomes invalid after the specified epoch.
    MaxInvocations(u32), /// Capability may be invoked at most this many times.
    AuthoritySubset(u64), /// Further restrict the authority mask.
}

/// Reference to a canonical capability in the capability store.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CapabilityRef {
    pub hash: CapabilityHash,
}

/// The payload submitted when attempting to exercise a capability.
///
/// The caller provides a reference to a capability, not the capability
/// itself. The kernel resolves the reference against its canonical store.
#[derive(Debug, Clone)]
pub struct Invocation {
    pub capability: CapabilityRef,

    
    pub operation: u64, /// Operation the caller wants to perform.
    pub nonce: u64, /// Unique value preventing replay of the same invocation.
    pub invocation_signature: Ed25519Signature, /// Signature authenticating this invocation.
}

/// A cryptographic tombstone representing revocation.
#[derive(Debug, Clone)]
pub struct Tombstone {
    pub capability: CapabilityHash,     /// Capability whose authority has been revoked.
    pub revoked_at_epoch: u64, /// Epoch at which revocation occurred.
    pub issuer_signature: Ed25519Signature, /// Signature authorizing the revocation.
}

/// Root of Axiom 0's authority hierarchy.
#[derive(Debug, Clone)]
pub struct Genesis {
    pub public_key: Ed25519PublicKey, /// Public key corresponding to the genesis authority.
    pub root_capability: CapabilityHash, /// Cryptographic identity of the root capability.
}
