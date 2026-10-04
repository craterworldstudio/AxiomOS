use crate::object::{ObjectKind, ObjectRef};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rights(u8);

impl Rights {
    pub const NONE: Rights  = Rights(0b0000);
    pub const READ: Rights  = Rights(0b0001);
    pub const WRITE: Rights = Rights(0b0010);
    pub const EXEC: Rights  = Rights(0b0100);
    pub const GRANT: Rights = Rights(0b1000);
    pub const ALL: Rights   = Rights(0b1111);

    pub fn contains(&self, other: Rights) -> bool {
        (self.0 & other.0) == other.0
    }
}

/// A fixed-size, kernel-resident record of authority.
/// Contains no provenance trees, no epochs, and no raw pointers.
#[derive(Debug, Clone, Copy)]
pub struct Capability {
    pub object: ObjectRef,
    pub kind: ObjectKind,
    pub rights: Rights,
}

/// A slot inside a CNode. Manages the temporal validity of a capability
/// to permanently eliminate stale reference vulnerabilities.
#[derive(Debug, Clone, Copy)]
pub struct CapabilitySlot {
    pub generation: u32,
    pub capability: Option<Capability>,
}

impl CapabilitySlot {
    pub const fn empty() -> Self {
        Self {
            // Generation starts at 1. 0 can be reserved for uninitialized/null handles.
            generation: 1, 
            capability: None,
        }
    }

    /// Inserts a capability into the slot.
    pub fn insert(&mut self, cap: Capability) {
        self.capability = Some(cap);
    }

    /// Deletes the capability and advances the generation counter.
    /// This strictly enforces the ADR-0006 invariant: a stale handle 
    /// pointing to this slot will immediately fail the generation check.
    pub fn delete(&mut self) {
        self.capability = None;
        self.generation = self.generation.wrapping_add(1);
    }
    
    /// Validates a userspace handle's generation against the slot's current generation.
    pub fn is_valid_generation(&self, expected_generation: u32) -> bool {
        self.generation == expected_generation && self.capability.is_some()
    }
}

pub const CNODE_SLOTS: usize = 256;

/// A Capability Node. A flat, O(1) lookup array of capability slots.
pub struct CNode {
    pub slots: [CapabilitySlot; CNODE_SLOTS],
}

impl CNode {
    pub const fn new() -> Self {
        Self {
            slots: [CapabilitySlot::empty(); CNODE_SLOTS],
        }
    }
}
