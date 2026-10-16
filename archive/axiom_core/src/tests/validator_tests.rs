use ed25519_dalek::{Signer, SigningKey, VerifyingKey};
use rand_core::OsRng;
use std::collections::HashSet;

use crate::capability::{
    Capability, CapabilityRef, Invocation, AUTH_DELEGATE, AUTH_READ, AUTH_WRITE, RejectReason, 
};
use crate::crypto::{compute_hash, GENESIS_HASH};
use crate::store::{CapabilityStore, GenesisRoot,};
use crate::validator::{ValidationResult, Validator};

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

        let genesis_cap = Capability {
            target_object: object_id,
            authority_mask: genesis_authority,
            parent_hash: GENESIS_HASH,
            membrane: None,
            epoch_issued: 0,
            owner_key: hardware_root_key.verifying_key(),
            issuer_signature: ed25519_dalek::Signature::from_bytes(&[0u8; 64]),
        };

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
        let hw_pubkey = self.hardware_root_key.verifying_key();
        let mut validator = Validator::new(
            &self.store,
            self.current_epoch,
            &mut self.consumed_nonces,
            &hw_pubkey,
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
        issuer_signature: ed25519_dalek::Signature::from_bytes(&[0u8; 64]),
    };

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
fn test_11_revoked_parent_cannot_issue() {
    let object_id = compute_hash(b"axiom-test-object");
    let mut env = TestEnvironment::new(object_id, AUTH_READ | AUTH_DELEGATE);
    let mut csprng = OsRng;
    
    let child_signer = SigningKey::generate(&mut csprng);
    let grandchild_signer = SigningKey::generate(&mut csprng);
    let genesis_hash = env.store.get_genesis().capability.identity_hash();

    // 1. Issue a valid child
    let child = make_capability(
        object_id,
        AUTH_READ | AUTH_DELEGATE,
        genesis_hash,
        1,
        child_signer.verifying_key(),
        &env.hardware_root_key,
    );
    let child_hash = env.store.issue(child).unwrap();

    // 2. Revoke the child
    let tombstone = crate::store::Tombstone {
        revoked_at_epoch: 2,
        issuer_signature: ed25519_dalek::Signature::from_bytes(&[0u8; 64]), // Mock for Axiom 0
    };
    env.store.mark_tombstone(child_hash, tombstone);

    // 3. Attempt to issue a grandchild from the revoked child
    let grandchild = make_capability(
        object_id,
        AUTH_READ,
        child_hash,
        3,
        grandchild_signer.verifying_key(),
        &child_signer,
    );

    // The issuance must fail at the store boundary, not during invocation
    assert_eq!(
        env.store.issue(grandchild),
        Err(RejectReason::Revoked)
    );
}

#[test]
fn test_03_authority_escalation_is_rejected_at_issuance() {
    let object_id = compute_hash(b"axiom-test-object");
    let mut env = TestEnvironment::new(object_id, AUTH_READ | AUTH_DELEGATE);
    let mut csprng = OsRng;

    let child_signer = SigningKey::generate(&mut csprng);
    let genesis_hash = env.store.get_genesis().capability.identity_hash();

    let malicious_child = make_capability(
        object_id,
        AUTH_READ | AUTH_WRITE,
        genesis_hash,
        1,
        child_signer.verifying_key(),
        &env.hardware_root_key,
    );

    // MIGRATED: The store itself prevents the bad capability from entering reality
    assert_eq!(
        env.store.issue(malicious_child),
        Err(RejectReason::AuthorityViolation)
    );
}

#[test]
fn test_04_invalid_signature_is_rejected_at_invocation() {
    let object_id = compute_hash(b"axiom-test-object");
    let mut env = TestEnvironment::new(object_id, AUTH_READ | AUTH_DELEGATE);
    let mut csprng = OsRng;
    
    let child_signer = SigningKey::generate(&mut csprng);
    let rogue_signer = SigningKey::generate(&mut csprng);
    let genesis_hash = env.store.get_genesis().capability.identity_hash();

    let child = make_capability(
        object_id,
        AUTH_READ,
        genesis_hash,
        1,
        child_signer.verifying_key(),
        &env.hardware_root_key,
    );
    let child_hash = env.store.issue(child).unwrap();

    let invocation = Invocation {
        capability: CapabilityRef { hash: child_hash },
        operation: AUTH_READ,
        nonce: 1,
        invocation_signature: sign_invocation(&rogue_signer, &child_hash, AUTH_READ, 1),
    };

    assert_eq!(
        env.run_validator(&invocation),
        ValidationResult::Rejected(RejectReason::InvalidSignature)
    );
}

#[test]
fn test_05_invalid_issuer_signature_is_rejected_at_issuance() {
    let object_id = compute_hash(b"axiom-test-object");
    let mut env = TestEnvironment::new(object_id, AUTH_READ | AUTH_DELEGATE);
    let mut csprng = OsRng;
    
    let child_signer = SigningKey::generate(&mut csprng);
    let attacker_signer = SigningKey::generate(&mut csprng); 
    let genesis_hash = env.store.get_genesis().capability.identity_hash();

    let child = make_capability(
        object_id,
        AUTH_READ,
        genesis_hash,
        1,
        child_signer.verifying_key(),
        &attacker_signer, 
    );

    assert_eq!(
        env.store.issue(child),
        Err(RejectReason::InvalidIssuerSignature)
    );
}

#[test]
fn test_06_tampered_capability_is_rejected_by_validator() {
    let object_id = compute_hash(b"axiom-test-object");
    let mut env = TestEnvironment::new(object_id, AUTH_READ | AUTH_DELEGATE);
    let mut csprng = OsRng;
    
    let child_signer = SigningKey::generate(&mut csprng);
    let genesis_hash = env.store.get_genesis().capability.identity_hash();

    let mut child = make_capability(
        object_id,
        AUTH_READ,
        genesis_hash,
        1,
        child_signer.verifying_key(),
        &env.hardware_root_key,
    );
    
    let original_hash = child.identity_hash();
    
    // MIGRATED: Simulate raw memory corruption bypassing `issue()`
    child.authority_mask = AUTH_READ | AUTH_WRITE; 
    env.store.corrupt_capability_for_test(original_hash, child);

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
fn test_07_tampered_genesis_is_rejected_by_validator() {
    let object_id = compute_hash(b"axiom-test-object");
    let mut env = TestEnvironment::new(object_id, AUTH_READ | AUTH_DELEGATE);
    let mut csprng = OsRng;
    
    let child_signer = SigningKey::generate(&mut csprng);
    let original_genesis_hash = env.store.get_genesis().capability.identity_hash();

    let child = make_capability(
        object_id,
        AUTH_READ,
        original_genesis_hash,
        1,
        child_signer.verifying_key(),
        &env.hardware_root_key,
    );
    let child_hash = env.store.issue(child).unwrap();

    // MIGRATED: Mutate the Genesis Root directly to simulate corruption
    env.store.corrupt_genesis_for_test(|genesis| {
        genesis.authority_mask |= AUTH_WRITE;
    });

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
fn test_08_wrong_hardware_root_is_rejected_by_validator() {
    let object_id = compute_hash(b"axiom-test-object");
    let mut env = TestEnvironment::new(object_id, AUTH_READ | AUTH_DELEGATE);
    let mut csprng = OsRng;
    
    let child_signer = SigningKey::generate(&mut csprng);
    let genesis_hash = env.store.get_genesis().capability.identity_hash();

    let child = make_capability(
        object_id,
        AUTH_READ,
        genesis_hash,
        1,
        child_signer.verifying_key(),
        &env.hardware_root_key,
    );
    let child_hash = env.store.issue(child).unwrap();

    let invocation = Invocation {
        capability: CapabilityRef { hash: child_hash },
        operation: AUTH_READ,
        nonce: 1,
        invocation_signature: sign_invocation(&child_signer, &child_hash, AUTH_READ, 1),
    };

    let fake_hardware_root = SigningKey::generate(&mut csprng);
    let fake_hw_pubkey = fake_hardware_root.verifying_key();

    let mut validator = Validator::new(
        &env.store,
        env.current_epoch,
        &mut env.consumed_nonces,
        &fake_hw_pubkey, 
    );

    assert_eq!(
        validator.validate(&invocation),
        ValidationResult::Rejected(RejectReason::GenesisMismatch)
    );
}

#[test]
fn test_09_unknown_parent_is_rejected_at_issuance() {
    let object_id = compute_hash(b"axiom-test-object");
    let mut env = TestEnvironment::new(object_id, AUTH_READ | AUTH_DELEGATE);
    let mut csprng = OsRng;
    
    let child_signer = SigningKey::generate(&mut csprng);
    let fake_parent_hash = compute_hash(b"axiom-void-parent");

    let child = make_capability(
        object_id,
        AUTH_READ,
        fake_parent_hash,
        1,
        child_signer.verifying_key(),
        &env.hardware_root_key,
    );

    assert_eq!(
        env.store.issue(child),
        Err(RejectReason::UnknownParent)
    );
}

#[test]
fn test_10_child_cannot_predate_parent_at_issuance() {
    let object_id = compute_hash(b"axiom-test-object");
    // Genesis is created at Epoch 0 
    let mut env = TestEnvironment::new(object_id, AUTH_READ | AUTH_DELEGATE);
    let mut csprng = OsRng;
    
    let child_signer = SigningKey::generate(&mut csprng);
    let grandchild_signer = SigningKey::generate(&mut csprng);
    let genesis_hash = env.store.get_genesis().capability.identity_hash();

    // Child is issued at Epoch 2 (Valid: 2 >= 0)
    let child = make_capability(
        object_id,
        AUTH_READ | AUTH_DELEGATE,
        genesis_hash,
        2, 
        child_signer.verifying_key(),
        &env.hardware_root_key,
    );
    let child_hash = env.store.issue(child).unwrap();

    // Grandchild is issued at Epoch 1 (Invalid: 1 < 2)
    let grandchild = make_capability(
        object_id,
        AUTH_READ,
        child_hash,
        1, // Temporal Paradox
        grandchild_signer.verifying_key(),
        &child_signer,
    );

    assert_eq!(
        env.store.issue(grandchild),
        Err(RejectReason::EpochInvalid)
    );
}