use crate::object::{ObjectKind, ObjectRef, ObjectTable, Untyped};

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

#[derive(Debug, Clone, Copy)]
pub struct Capability {
    pub object: ObjectRef,
    pub kind: ObjectKind,
    pub rights: Rights,
}

#[derive(Debug, Clone, Copy)]
pub struct CapabilitySlot {
    pub generation: u32,
    pub capability: Option<Capability>,
}

impl CapabilitySlot {
    pub const fn empty() -> Self {
        Self {
            generation: 1, 
            capability: None,
        }
    }

    pub fn insert(&mut self, cap: Capability) {
        self.capability = Some(cap);
    }

    pub fn delete(&mut self) {
        self.capability = None;
        self.generation = self.generation.wrapping_add(1);
    }
    
    pub fn is_valid_generation(&self, expected_generation: u32) -> bool {
        self.generation == expected_generation && self.capability.is_some()
    }
}

pub const CNODE_SLOTS: usize = 256;

pub struct CNode {
    pub slots: [CapabilitySlot; CNODE_SLOTS],
}

impl CNode {
    pub const fn new() -> Self {
        Self {
            slots: [CapabilitySlot::empty(); CNODE_SLOTS],
        }
    }

    /// Checks for a free slot without mutating state.
    pub fn find_free_slot(&self) -> Option<usize> {
        for i in 1..CNODE_SLOTS {
            if self.slots[i].capability.is_none() {
                return Some(i);
            }
        }
        None
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum RetypeError {
    InvalidKind,
    InsufficientMemory,
    ObjectTableFull,
    CNodeFull,
}

/// Transactionally carves a kernel object out of an Untyped memory block.
/// Guaranteed to leave all structures unchanged if the allocation cannot complete.
pub fn retype(
    untyped: &mut Untyped,
    kind: ObjectKind,
    object_table: &mut ObjectTable,
    cnode: &mut CNode,
) -> Result<ObjectRef, RetypeError> {
    if kind == ObjectKind::Null || kind == ObjectKind::Untyped {
        return Err(RetypeError::InvalidKind);
    }

    // 1. Calculate alignment and new watermark
    let (size, align) = kind.layout();
    let current_phys = untyped.physical_base + untyped.watermark;
    let remainder = current_phys % align;
    let offset = if remainder == 0 { 0 } else { align - remainder };
    
    let aligned_phys = current_phys + offset;
    let new_watermark = untyped.watermark + offset + size;

    // 2. Verify physical capacity
    if new_watermark > untyped.size {
        return Err(RetypeError::InsufficientMemory);
    }

    // 3. Verify registry capacity
    if !object_table.has_free_slot() {
        return Err(RetypeError::ObjectTableFull);
    }

    // 4. Verify capability slot capacity
    let cnode_slot_idx = cnode.find_free_slot().ok_or(RetypeError::CNodeFull)?;

    // ==========================================
    // COMMIT PHASE
    // ==========================================
    
    untyped.watermark = new_watermark;
    
    let obj_ref = object_table.register_object(kind, aligned_phys)
        .expect("ObjectTable availability verified pre-commit");

    let cap = Capability {
        object: obj_ref,
        kind,
        rights: Rights::ALL,
    };
    
    cnode.slots[cnode_slot_idx].insert(cap);

    Ok(obj_ref)
}