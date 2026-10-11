use crate::object::{ObjectKind, ObjectRef, ObjectTable, Untyped};

pub const OP_SEND: u64    = 0;
pub const OP_RECEIVE: u64 = 1;
pub const OP_CALL: u64    = 2;

pub const SYSCALL_OK: u64         = 0;
pub const ERR_INVALID_HANDLE: u64 = 1;
pub const ERR_STALE_HANDLE: u64   = 2;
pub const ERR_INVALID_RIGHTS: u64 = 3;
pub const ERR_INVALID_KIND: u64   = 4;
pub const ERR_UNSUPPORTED_OP: u64 = 5;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct CapHandle(pub u64);

impl CapHandle {
    pub const fn new(index: u32, generation: u32) -> Self {
        Self(((generation as u64) << 32) | (index as u64))
    }
    pub fn index(&self) -> u32 { (self.0 & 0xFFFFFFFF) as u32 }
    pub fn generation(&self) -> u32 { (self.0 >> 32) as u32 }
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
        Self { generation: 1,  capability: None, }
    }

    pub fn insert(&mut self, cap: Capability) { self.capability = Some(cap); }

    pub fn delete(&mut self) {
        self.capability = None;
        self.generation = self.generation.wrapping_add(1);
    }
    
    pub fn is_valid_generation(&self, expected_generation: u32) -> bool {
        self.generation == expected_generation && self.capability.is_some()
    }
}

pub const CNODE_SLOTS: usize = 256;
pub struct CNode { pub slots: [CapabilitySlot; CNODE_SLOTS] }

impl CNode {
    pub const fn new() -> Self {
        Self { slots: [CapabilitySlot::empty(); CNODE_SLOTS], }
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

// Global Kernel State 
pub static mut KERNEL_OBJECT_TABLE: ObjectTable = ObjectTable::new();
pub static mut GENESIS_CNODE: CNode = CNode::new();

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
) -> Result<(ObjectRef, usize), RetypeError> { 
    let (size, align) = kind.layout();
    let size = size as u64;   // Cast to u64 for physical math
    let align = align as u64; // Cast to u64 for physical math
    
    let current_phys = untyped.physical_base + untyped.watermark;
    let remainder = current_phys % align;
    let offset = if remainder == 0 { 0 } else { align - remainder };
    
    let aligned_phys = current_phys + offset;
    let new_watermark = untyped.watermark + offset + size;

    if new_watermark > untyped.size { return Err(RetypeError::InsufficientMemory); }
    if !object_table.has_free_slot() { return Err(RetypeError::ObjectTableFull); }
    
    let cnode_slot_idx = cnode.find_free_slot().ok_or(RetypeError::CNodeFull)?;

    untyped.watermark = new_watermark;
    let obj_ref = object_table.register_object(kind, aligned_phys).unwrap();

    let cap = Capability { object: obj_ref, kind, rights: Rights::ALL };
    cnode.slots[cnode_slot_idx].insert(cap);

    Ok((obj_ref, cnode_slot_idx))
}

#[no_mangle]
pub extern "C" fn sys_invoke_handler(handle_val: u64, operation: u64, arg1: u64, _arg2: u64) -> u64 {
    let handle = CapHandle(handle_val);
    let slot_idx = handle.index() as usize;

    let cnode = unsafe { &GENESIS_CNODE };
    if slot_idx == 0 || slot_idx >= CNODE_SLOTS { return ERR_INVALID_HANDLE; }

    let slot = &cnode.slots[slot_idx];
    if !slot.is_valid_generation(handle.generation()) { return ERR_STALE_HANDLE; }
    
    let cap = slot.capability.as_ref().unwrap();

    // Rights Validation
    if operation == OP_SEND && !cap.rights.contains(Rights::WRITE) { return ERR_INVALID_RIGHTS; }
    if operation == OP_RECEIVE && !cap.rights.contains(Rights::READ) { return ERR_INVALID_RIGHTS; }

    let object_table = unsafe { &KERNEL_OBJECT_TABLE };
    let (kind, phys_addr) = object_table.resolve(cap.object).unwrap();

    // Endpoint Dispatch
    match kind {
        ObjectKind::Endpoint => {
            // SAFETY: phys_addr is the physical address of an Endpoint object registered
            // by retype(). It is valid for the lifetime of the kernel and exclusively
            // managed through the object table. No aliasing occurs because the object
            // table grants a single physical address per registered object.
            let ep = unsafe { &mut *(phys_addr as *mut crate::object::Endpoint) };

            let caller_ref = unsafe {
                crate::scheduler::SCHEDULER.current
                    .expect("sys_invoke: no current thread")
            };

            match operation {
                OP_SEND => {
                    let woken = ep.endpoint_send(caller_ref, arg1, object_table);
                    if let Some(woken_ref) = woken {
                        // Synchronous rendezvous: a receiver was already waiting.
                        // Mark it Ready (done inside endpoint_send) and put it back
                        // on the run queue so the scheduler will pick it up.
                        unsafe { crate::scheduler::SCHEDULER.enqueue(woken_ref); }
                    } else {
                        // No receiver was waiting — the sender is now BlockedSend.
                        // Yield so another thread can run (and eventually receive).
                        // When this thread is eventually unblocked and rescheduled,
                        // switch_context will return here and we fall through to SYSCALL_OK.
                        unsafe { crate::scheduler::yield_thread(); }
                    }
                    SYSCALL_OK
                },
                OP_RECEIVE => {
                    let woken = ep.endpoint_receive(caller_ref, object_table);
                    if let Some(woken_ref) = woken {
                        // Synchronous rendezvous: a sender was already waiting.
                        // Mark it Ready (done inside endpoint_receive) and re-enqueue it.
                        unsafe { crate::scheduler::SCHEDULER.enqueue(woken_ref); }
                    } else {
                        // No sender was waiting — the receiver is now BlockedReceive.
                        // Yield so another thread can run (and eventually send).
                        unsafe { crate::scheduler::yield_thread(); }
                    }
                    SYSCALL_OK
                },
                OP_CALL => ERR_UNSUPPORTED_OP,
                _ => ERR_UNSUPPORTED_OP,
            }
        },
        _ => ERR_INVALID_KIND,
    }
}