use ed25519_dalek::SigningKey;
use rand_core::OsRng;
use crate::crypto::{GENESIS_HASH, verify_signature};
use crate::capability::{Capability, Invocation, Membrane};
use crate::store::CapabilityStore;
use crate::validator::{Validator, ValidationResult, RejectReason};
use std::collections::HashSet;

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
        let mut validator = Validator::new(&self.store, self.current_epoch, &mut self.consumed_nonces);
        validator.validate(invocation)
    }
}

// ==========================================
// THE AXIOM 0 TEST SUITE
// ==========================================

#[test]
fn test_00_genesis_math_is_real() {
    let mut env = TestEnvironment::new();
    let message = b"Axiom Runtime Initialization";
    let signature = env.genesis_key.sign(message);
    
    assert!(verify_signature(&env.genesis_key.verifying_key(), message, &signature));
}
