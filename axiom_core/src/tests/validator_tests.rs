use ed25519_dalek::{Signer, SigningKey, VerifyingKey};
use rand_core::OsRng;
use std::collections::HashSet;

use crate::capability::{
    Capability,
    CapabilityRef,
    Invocation,
    AUTH_READ,
    AUTH_WRITE,
    AUTH_DELEGATE,
};
use crate::crypto::{compute_hash, GENESIS_HASH};
use crate::store::{CapabilityStore, GenesisRoot};
use crate::validator::{RejectReason, ValidationResult, Validator};

struct TestEnvironment {
    pub store: CapabilityStore,
    pub consumed_nonces: HashSet<([u8; 32], u64)>,
    pub current_epoch: u64,
    pub hardware_root_key: SigningKey,
}

impl TestEnvironment {
    fn new(object_id: [u8; 32], genesis_authority: u64) -> Self {
        let mut csprng = OsRng;
        let hardware_root_key = SigningKey::generate(&mut csprng);

        // Construct the Silicon Root Capability
        let genesis_cap = Capability {
            target_object: object_id,
            authority_mask: genesis_authority,
            parent_hash: GENESIS_HASH,
            membrane: None,
            epoch_issued: 0,
            owner_key: hardware_root_key.verifying_key(),
            issuer_signature: ed25519_dalek::Signature::from_bytes(&[0u8; 64]), // placeholder for hash
        };

        // Root-signed, not self-signed. Domain separated.
        let mut root_payload = b"AXIOM/GENESIS/V1".to_vec();
        root_payload.extend_from_slice(&genesis_cap.identity_hash());
        let root_signature = hardware_root_key.sign(&root_payload);

        let genesis_root = GenesisRoot {
            capability: genesis_cap,
            root_signature,
        };

        Self {
            store: CapabilityStore::new(genesis_root),
            consumed_nonces: HashSet::new(),
            current_epoch: 1,
            hardware_root_key,
        }
    }

    fn run_validator(&mut self, invocation: &Invocation) -> ValidationResult {
        
        let hroot_key_verified = &self.hardware_root_key.verifying_key();
        
        let mut validator = Validator::new(
            &self.store,
            self.current_epoch,
            &mut self.consumed_nonces,
            &hroot_key_verified,
        );

        validator.validate(invocation)
    }
}

fn make_capability(
    target: [u8; 32],
    authority_mask: u64,
    parent_hash: [u8; 32],
    epoch: u64,
    owner_key: VerifyingKey,
    parent_signer: &SigningKey,

) -> Capability {
    let mut cap = Capability {
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
    };

    // Parent explicitly signs the issuance of this child. Domain separated.
    let mut payload = b"AXIOM/CAPABILITY-ISSUANCE/V1".to_vec();
    payload.extend_from_slice(&cap.identity_hash());
    cap.issuer_signature = parent_signer.sign(&payload);

    cap
}

fn sign_invocation(
    signer: &SigningKey,
    cap_hash: &[u8; 32],
    operation: u64,
    nonce: u64,
) -> ed25519_dalek::Signature {
    let mut payload = b"AXIOM/INVOCATION/V1".to_vec();
    payload.extend_from_slice(cap_hash);
    payload.extend_from_slice(&operation.to_le_bytes());
    payload.extend_from_slice(&nonce.to_le_bytes());
    signer.sign(&payload)
}

#[test]
fn test_01_valid_genesis_to_child_delegation() {
    let object_id = compute_hash(b"axiom-test-object");
    let mut env = TestEnvironment::new(object_id, AUTH_READ | AUTH_WRITE | AUTH_DELEGATE);
    let mut csprng = OsRng;

    let child_signer = SigningKey::generate(&mut csprng);
    let genesis_hash = env.store.genesis.capability.identity_hash();

    let child = make_capability(
        object_id,
        AUTH_READ,
        genesis_hash,
        1,
        child_signer.verifying_key(),
        &env.hardware_root_key,
    );

    let child_hash = env.store.inject_for_test(child);

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
    let object_id = compute_hash(b"axiom-test-object");
    let mut env = TestEnvironment::new(object_id, AUTH_READ | AUTH_DELEGATE);
    let mut csprng = OsRng;

    let child_signer = SigningKey::generate(&mut csprng);
    let genesis_hash = env.store.genesis.capability.identity_hash();

    let malicious_child = make_capability(
        object_id,
        AUTH_READ | AUTH_WRITE,
        genesis_hash,
        1,
        child_signer.verifying_key(),
        &env.hardware_root_key,
    );

    let child_hash = env.store.inject_for_test(malicious_child);

    let invocation = Invocation {
        capability: CapabilityRef {
            hash: child_hash,
        },
        operation: AUTH_WRITE,
        nonce: 3,
        invocation_signature: sign_invocation(
            &child_signer, &child_hash, AUTH_WRITE, 3
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

#[test]
fn test_04_invalid_signature_is_rejected() {
    let object_id = compute_hash(b"axiom-test-object");
    let mut env = TestEnvironment::new(object_id, AUTH_READ | AUTH_DELEGATE);
    let mut csprng = OsRng;
    
    let child_signer = SigningKey::generate(&mut csprng);
    let rogue_signer = SigningKey::generate(&mut csprng);
    let genesis_hash = env.store.genesis.capability.identity_hash();

    // Child capability (Owned by child_signer)
    let child = make_capability(
        object_id,
        AUTH_READ,
        genesis_hash,
        1,
        child_signer.verifying_key(),
        &env.hardware_root_key,
    );
    let child_hash = env.store.inject_for_test(child);

    // Invocation perfectly formed, BUT signed by rogue_signer instead of child_signer
    let invocation = Invocation {
        capability: CapabilityRef { hash: child_hash },
        operation: AUTH_READ,
        nonce: 1,
        invocation_signature: sign_invocation(&rogue_signer, &child_hash, AUTH_READ, 1),
    };

    let result = env.run_validator(&invocation);
    
    assert_eq!(
        result, 
        ValidationResult::Rejected(RejectReason::InvalidSignature)
    );
}

#[test]
fn test_05_invalid_issuer_signature_is_rejected() {
    let object_id = compute_hash(b"axiom-test-object");
    let mut env = TestEnvironment::new(object_id, AUTH_READ | AUTH_DELEGATE);
    let mut csprng = OsRng;
    
    let child_signer = SigningKey::generate(&mut csprng);
    let attacker_signer = SigningKey::generate(&mut csprng); 
    let genesis_hash = env.store.genesis.capability.identity_hash();

    // Attacker constructs a child graph but signs the issuance themselves 
    // instead of obtaining parent authorization
    let child = make_capability(
        object_id,
        AUTH_READ,
        genesis_hash,
        1,
        child_signer.verifying_key(),
        &attacker_signer, 
    );
    let child_hash = env.store.inject_for_test(child);

    let invocation = Invocation {
        capability: CapabilityRef { hash: child_hash },
        operation: AUTH_READ,
        nonce: 1,
        invocation_signature: sign_invocation(&child_signer, &child_hash, AUTH_READ, 1),
    };

    assert_eq!(
        env.run_validator(&invocation),
        ValidationResult::Rejected(RejectReason::InvalidIssuerSignature)
    );
}

#[test]
fn test_06_tampered_capability_is_rejected() {
    let object_id = compute_hash(b"axiom-test-object");
    let mut env = TestEnvironment::new(object_id, AUTH_READ | AUTH_DELEGATE);
    let mut csprng = OsRng;
    
    let child_signer = SigningKey::generate(&mut csprng);
    let genesis_hash = env.store.genesis.capability.identity_hash();

    let mut child = make_capability(
        object_id,
        AUTH_READ,
        genesis_hash,
        1,
        child_signer.verifying_key(),
        &env.hardware_root_key,
    );
    
    let original_hash = child.identity_hash();
    
    // Attacker modifies an existing capability in the store
    child.authority_mask = AUTH_READ | AUTH_WRITE; 
    env.store.capabilities.insert(original_hash, child); // Bypass injection to simulate raw memory corruption

    let invocation = Invocation {
        capability: CapabilityRef { hash: original_hash },
        operation: AUTH_READ,
        nonce: 1,
        invocation_signature: sign_invocation(&child_signer, &original_hash, AUTH_READ, 1),
    };

    assert_eq!(
        env.run_validator(&invocation),
        ValidationResult::Rejected(RejectReason::CapabilityHashMismatch)
    );
}

#[test]
fn test_07_tampered_genesis_is_rejected() {
    let object_id = compute_hash(b"axiom-test-object");
    let mut env = TestEnvironment::new(object_id, AUTH_READ | AUTH_DELEGATE);
    let mut csprng = OsRng;
    
    let child_signer = SigningKey::generate(&mut csprng);
    let original_genesis_hash = env.store.genesis.capability.identity_hash();

    let child = make_capability(
        object_id,
        AUTH_READ,
        original_genesis_hash,
        1,
        child_signer.verifying_key(),
        &env.hardware_root_key,
    );
    let child_hash = env.store.inject_for_test(child);

    // Attacker modifies the Genesis Root in place
    env.store.genesis.capability.authority_mask |= AUTH_WRITE;

    let invocation = Invocation {
        capability: CapabilityRef { hash: child_hash },
        operation: AUTH_READ,
        nonce: 1,
        invocation_signature: sign_invocation(&child_signer, &child_hash, AUTH_READ, 1),
    };

    assert_eq!(
        env.run_validator(&invocation),
        ValidationResult::Rejected(RejectReason::GenesisMismatch)
    );
}

#[test]
fn test_08_wrong_hardware_root_is_rejected() {
    let object_id = compute_hash(b"axiom-test-object");
    let mut env = TestEnvironment::new(object_id, AUTH_READ | AUTH_DELEGATE);
    let mut csprng = OsRng;
    
    let child_signer = SigningKey::generate(&mut csprng);
    let genesis_hash = env.store.genesis.capability.identity_hash();

    let child = make_capability(
        object_id,
        AUTH_READ,
        genesis_hash,
        1,
        child_signer.verifying_key(),
        &env.hardware_root_key,
    );
    let child_hash = env.store.inject_for_test(child);

    let invocation = Invocation {
        capability: CapabilityRef { hash: child_hash },
        operation: AUTH_READ,
        nonce: 1,
        invocation_signature: sign_invocation(&child_signer, &child_hash, AUTH_READ, 1),
    };

    // The validator boots with a different Hardware Root than the one that signed Genesis
    let fake_hardware_root = SigningKey::generate(&mut csprng);
    let fake_hw_pubkey = fake_hardware_root.verifying_key();

    let mut validator = Validator::new(
        &env.store,
        env.current_epoch,
        &mut env.consumed_nonces,
        &fake_hw_pubkey, // DIFFERENT ROOT
    );

    assert_eq!(
        validator.validate(&invocation),
        ValidationResult::Rejected(RejectReason::GenesisMismatch)
    );
}

#[test]
fn test_09_unknown_parent_is_rejected() {
    let object_id = compute_hash(b"axiom-test-object");
    let mut env = TestEnvironment::new(object_id, AUTH_READ | AUTH_DELEGATE);
    let mut csprng = OsRng;
    
    let child_signer = SigningKey::generate(&mut csprng);
    let fake_parent_hash = compute_hash(b"axiom-void-parent");

    // Child claims to descend from a capability that does not exist in the store
    let child = make_capability(
        object_id,
        AUTH_READ,
        fake_parent_hash,
        1,
        child_signer.verifying_key(),
        &env.hardware_root_key,
    );
    let child_hash = env.store.inject_for_test(child);

    let invocation = Invocation {
        capability: CapabilityRef { hash: child_hash },
        operation: AUTH_READ,
        nonce: 1,
        invocation_signature: sign_invocation(&child_signer, &child_hash, AUTH_READ, 1),
    };

    assert_eq!(
        env.run_validator(&invocation),
        ValidationResult::Rejected(RejectReason::UnknownParent)
    );
}