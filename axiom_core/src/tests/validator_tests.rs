use ed25519_dalek::{Signer, SigningKey, VerifyingKey};
use rand_core::OsRng;
use std::collections::HashSet;

use crate::capability::{
    Capability,
    CapabilityRef,
    Invocation,
    AUTH_READ,
    AUTH_WRITE,
};
use crate::crypto::{compute_hash, GENESIS_HASH};
use crate::store::CapabilityStore;
use crate::validator::{RejectReason, ValidationResult, Validator};

struct TestEnvironment {
    pub store: CapabilityStore,
    pub consumed_nonces: HashSet<([u8; 32], u64)>,
    pub current_epoch: u64,
    pub genesis_key: SigningKey,
}

impl TestEnvironment {
    fn new() -> Self {
        let mut csprng = OsRng;
        let genesis_key = SigningKey::generate(&mut csprng);

        Self {
            store: CapabilityStore::new(),
            consumed_nonces: HashSet::new(),
            current_epoch: 1,
            genesis_key,
        }
    }

    fn run_validator(&mut self, invocation: &Invocation) -> ValidationResult {
        let mut validator = Validator::new(
            &self.store,
            self.current_epoch,
            &mut self.consumed_nonces,
        );

        validator.validate(invocation)
    }
}

fn make_capability(
    target: [u8; 32],
    authority_mask: u64,
    parent_hash: [u8; 32],
    epoch: u64,
    owner_key: VerifyingKey, // Bind it!
) -> Capability {
    Capability {
        target_object: target,
        authority_mask,
        parent_hash,
        membrane: None,
        epoch_issued: epoch,
        owner_key, 
        issuer_signature: {
            let mut csprng = OsRng;
            let random_signer = SigningKey::generate(&mut csprng);
            random_signer.sign(b"mocked-issuer-signature")
        },
    }
}

fn sign_invocation(
    signer: &SigningKey,
    cap_hash: &[u8; 32],
    operation: u64,
    nonce: u64,
) -> ed25519_dalek::Signature {
    let mut payload = Vec::new();
    payload.extend_from_slice(cap_hash);
    payload.extend_from_slice(&operation.to_le_bytes());
    payload.extend_from_slice(&nonce.to_le_bytes());
    signer.sign(&payload)
}

#[test]
fn test_01_valid_genesis_to_child_delegation() {
    let mut env = TestEnvironment::new();

    let child_signer = SigningKey::generate(&mut csprng);
    let object_id = compute_hash(b"axiom-test-object");

    // --------------------------------------------------
    // Genesis capability
    // --------------------------------------------------

    let genesis = make_capability(
        object_id,
        AUTH_READ | AUTH_WRITE,
        GENESIS_HASH,
        0,
        env.genesis_key.verifying_key(),
    );

    let genesis_hash = env.store.inject_for_test(genesis);

    // --------------------------------------------------
    // Child capability
    //
    // Parent: READ | WRITE
    // Child:  READ
    //
    // Therefore:
    //
    // READ ⊆ READ | WRITE
    // --------------------------------------------------

    let child = make_capability(
        object_id,
        AUTH_READ,
        genesis_hash,
        1,
        child_signer.verifying_key(),
    );

    let child_hash = env.store.inject_for_test(child);

    // --------------------------------------------------
    // Invocation
    // --------------------------------------------------

    let invocation = Invocation {
        capability: CapabilityRef {
            hash: child_hash,
        },
        operation: AUTH_READ,
        nonce: 1,
        invocation_signature: sign_invocation(
            &child_signer, &child_hash, AUTH_READ, 1
        ),
    };

    let result = env.run_validator(&invocation);

    assert_eq!(
        result,
        ValidationResult::Valid
    );
}

#[test]
fn test_03_authority_escalation_is_rejected() {
    let mut env = TestEnvironment::new();

    let child_signer = SigningKey::generate(&mut csprng);
    let object_id = compute_hash(b"axiom-test-object");

    // --------------------------------------------------
    // Genesis capability
    //
    // Authority = READ only
    // --------------------------------------------------

    let genesis = make_capability(
        object_id,
        AUTH_READ,
        GENESIS_HASH,
        0,
        env.genesis_key.verifying_key(),
    );

    let genesis_hash = env.store.inject_for_test(genesis);

    // --------------------------------------------------
    // Malicious child
    //
    // Parent = READ
    // Child  = READ | WRITE
    //
    // This violates monotonic attenuation.
    // --------------------------------------------------

    let malicious_child = make_capability(
        object_id,
        AUTH_READ | AUTH_WRITE,
        genesis_hash,
        1,
        child_signer.verifying_key(),
    );

    let child_hash = env.store.inject_for_test(malicious_child);

    let invocation = Invocation {
        capability: CapabilityRef {
            hash: child_hash,
        },
        operation: AUTH_WRITE,
        nonce: 3,
        invocation_signature: sign_invocation(
            &child_signer, &child_hash, AUTH_WRITE, 2
        ),
    };

    let result = env.run_validator(&invocation);

    assert_eq!(
        result,
        ValidationResult::Rejected(
            RejectReason::AuthorityViolation
        )
    );
}
