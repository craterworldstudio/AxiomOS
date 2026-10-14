use crate::object::{ObjectKind, ObjectRef, ObjectTable, Untyped};

pub const OP_SEND: u64    = 0;
pub const OP_RECEIVE: u64 = 1;
pub const OP_CALL: u64    = 2;
pub const OP_MINT: u64    = 3;
pub const OP_REVOKE: u64  = 4;
pub const OP_DELETE: u64  = 5;

pub const SYSCALL_OK: u64         = 0;
pub const ERR_INVALID_HANDLE: u64 = 1;
pub const ERR_STALE_HANDLE: u64   = 2;
pub const ERR_INVALID_RIGHTS: u64 = 3;
pub const ERR_INVALID_KIND: u64   = 4;
pub const ERR_UNSUPPORTED_OP: u64 = 5;
pub const ERR_CNODE_FULL: u64     = 6;
pub const ERR_SLOT_OCCUPIED: u64  = 7;
pub const ERR_NO_RECEIVER: u64    = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rights(pub u8);

impl Rights {
    pub const NONE: Rights  = Rights(0b0000);
    pub const READ: Rights  = Rights(0b0001);
    pub const WRITE: Rights = Rights(0b0010);
    pub const EXEC: Rights  = Rights(0b0100);
    pub const GRANT: Rights = Rights(0b1000);
    pub const ALL: Rights   = Rights(0b1111);

    #[inline(always)]
    pub fn contains(&self, other: Rights) -> bool {
        (self.0 & other.0) == other.0
    }

    #[inline(always)]
    pub fn is_subset(&self, parent: Rights) -> bool {
        parent.contains(*self)
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlotIndex(pub u16);

#[derive(Debug, Clone, Copy)]
pub struct CdtLink {
    pub parent: Option<SlotIndex>,
    pub first_child: Option<SlotIndex>,
    pub next_sibling: Option<SlotIndex>,
    pub prev_sibling: Option<SlotIndex>,
}

impl CdtLink {
    pub const fn empty() -> Self {
        Self {
            parent: None,
            first_child: None,
            next_sibling: None,
            prev_sibling: None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CapabilitySlot {
    pub generation: u32,
    pub capability: Option<Capability>,
    pub cdt: CdtLink,
}

impl CapabilitySlot {
    pub const fn empty() -> Self {
        Self {
            generation: 1,
            capability: None,
            cdt: CdtLink::empty(),
        }
    }

    pub fn insert(&mut self, cap: Capability) {
        self.capability = Some(cap);
        self.cdt = CdtLink::empty();
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

    /// Derives a new capability into dest_idx with attenuated rights.
    /// Invariant: Authority monotonically decreases (child.rights.is_subset(parent.rights)).
    pub fn mint(&mut self, src_idx: usize, dest_idx: usize, new_rights: Rights) -> Result<CapHandle, u64> {
        if src_idx == 0 || src_idx >= CNODE_SLOTS || dest_idx == 0 || dest_idx >= CNODE_SLOTS {
            return Err(ERR_INVALID_HANDLE);
        }
        let src_cap = self.slots[src_idx].capability.ok_or(ERR_INVALID_HANDLE)?;
        if self.slots[dest_idx].capability.is_some() {
            return Err(ERR_SLOT_OCCUPIED);
        }
        // Rights monotonicity check: child rights must be a subset of parent rights
        if !new_rights.is_subset(src_cap.rights) {
            return Err(ERR_INVALID_RIGHTS);
        }

        let child_cap = Capability {
            object: src_cap.object,
            kind: src_cap.kind,
            rights: new_rights,
        };
        self.slots[dest_idx].capability = Some(child_cap);

        // Link into CDT:
        let src_first = self.slots[src_idx].cdt.first_child;
        self.slots[dest_idx].cdt.parent = Some(SlotIndex(src_idx as u16));
        self.slots[dest_idx].cdt.next_sibling = src_first;
        self.slots[dest_idx].cdt.prev_sibling = None;
        if let Some(old_head) = src_first {
            self.slots[old_head.0 as usize].cdt.prev_sibling = Some(SlotIndex(dest_idx as u16));
        }
        self.slots[src_idx].cdt.first_child = Some(SlotIndex(dest_idx as u16));

        Ok(CapHandle::new(dest_idx as u32, self.slots[dest_idx].generation))
    }

    /// Recursively revokes all descendant capabilities derived from slot_idx.
    /// Does NOT delete slot_idx itself.
    pub fn revoke(&mut self, slot_idx: usize) -> Result<(), u64> {
        if slot_idx == 0 || slot_idx >= CNODE_SLOTS {
            return Err(ERR_INVALID_HANDLE);
        }
        if self.slots[slot_idx].capability.is_none() {
            return Err(ERR_INVALID_HANDLE);
        }

        let mut child_opt = self.slots[slot_idx].cdt.first_child;
        self.slots[slot_idx].cdt.first_child = None;

        while let Some(child_idx) = child_opt {
            let next_sib = self.slots[child_idx.0 as usize].cdt.next_sibling;
            self.revoke_subtree(child_idx.0 as usize);
            child_opt = next_sib;
        }

        Ok(())
    }

    fn revoke_subtree(&mut self, idx: usize) {
        if idx == 0 || idx >= CNODE_SLOTS { return; }
        let mut child_opt = self.slots[idx].cdt.first_child;
        self.slots[idx].cdt.first_child = None;

        while let Some(child_idx) = child_opt {
            let next_sib = self.slots[child_idx.0 as usize].cdt.next_sibling;
            self.revoke_subtree(child_idx.0 as usize);
            child_opt = next_sib;
        }

        // Invalidate capability and advance generation counter
        self.slots[idx].capability = None;
        self.slots[idx].generation = self.slots[idx].generation.wrapping_add(1);
        self.slots[idx].cdt = CdtLink::empty();
    }

    /// Deletes the capability in slot_idx, unlinking it from CDT and revoking its children.
    pub fn delete(&mut self, slot_idx: usize) -> Result<(), u64> {
        if slot_idx == 0 || slot_idx >= CNODE_SLOTS {
            return Err(ERR_INVALID_HANDLE);
        }
        // 1. Recursively revoke all descendants
        let _ = self.revoke(slot_idx);

        // 2. Unlink from sibling / parent list
        let prev = self.slots[slot_idx].cdt.prev_sibling;
        let next = self.slots[slot_idx].cdt.next_sibling;

        if let Some(p) = prev {
            self.slots[p.0 as usize].cdt.next_sibling = next;
        } else if let Some(parent) = self.slots[slot_idx].cdt.parent {
            self.slots[parent.0 as usize].cdt.first_child = next;
        }

        if let Some(n) = next {
            self.slots[n.0 as usize].cdt.prev_sibling = prev;
        }

        self.slots[slot_idx].capability = None;
        self.slots[slot_idx].generation = self.slots[slot_idx].generation.wrapping_add(1);
        self.slots[slot_idx].cdt = CdtLink::empty();

        Ok(())
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
pub fn retype(
    untyped: &mut Untyped,
    kind: ObjectKind,
    object_table: &mut ObjectTable,
    cnode: &mut CNode,
) -> Result<(ObjectRef, usize), RetypeError> { 
    let (size, align) = kind.layout();
    let size = size as u64;
    let align = align as u64;
    
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
pub extern "C" fn sys_invoke_handler(
    handle_val: u64,
    operation: u64,
    arg1: u64,
    arg2: u64,
    _arg3: u64,
    _arg4: u64,
) -> u64 {
    let handle = CapHandle(handle_val);
    let slot_idx = handle.index() as usize;

    // Security: Bounds check slot index
    if slot_idx == 0 || slot_idx >= CNODE_SLOTS { return ERR_INVALID_HANDLE; }
    // Security: Spectre v1 mitigation
    unsafe { core::arch::asm!("lfence"); }

    let caller_ref = unsafe {
        crate::scheduler::SCHEDULER.current
            .expect("sys_invoke: no current thread")
    };
    let object_table = unsafe { &mut KERNEL_OBJECT_TABLE };

    // Resolve caller's private CNode or fallback to GENESIS_CNODE
    let (_, caller_phys) = object_table.resolve(caller_ref).unwrap();
    let caller_tcb = unsafe { &mut *(caller_phys as *mut crate::thread::TCB) };

    let cnode: &mut CNode = match caller_tcb.cspace {
        Some(cspace_ref) => {
            let (_, cnode_phys) = object_table.resolve(cspace_ref).unwrap();
            unsafe { &mut *(cnode_phys as *mut CNode) }
        },
        None => unsafe { &mut GENESIS_CNODE },
    };

    let slot = &mut cnode.slots[slot_idx];
    if !slot.is_valid_generation(handle.generation()) { return ERR_STALE_HANDLE; }

    let cap = slot.capability.as_ref().unwrap();

    // 1. CNode / Capability Management Operations (CDT)
    match operation {
        OP_MINT => {
            // arg1 = dest_slot, arg2 = new_rights_bits
            let dest_slot = arg1 as usize;
            let new_rights = Rights(arg2 as u8);
            return match cnode.mint(slot_idx, dest_slot, new_rights) {
                Ok(new_handle) => new_handle.0,
                Err(err) => err,
            };
        },
        OP_REVOKE => {
            return match cnode.revoke(slot_idx) {
                Ok(()) => SYSCALL_OK,
                Err(err) => err,
            };
        },
        OP_DELETE => {
            return match cnode.delete(slot_idx) {
                Ok(()) => SYSCALL_OK,
                Err(err) => err,
            };
        },
        _ => {}
    }

    // 2. Object Invocation Operations (Endpoint, etc.)
    let (kind, phys_addr) = object_table.resolve(cap.object).unwrap();

    match kind {
        ObjectKind::Endpoint => {
            let ep = unsafe { &mut *(phys_addr as *mut crate::object::Endpoint) };

            match operation {
                OP_SEND => {
                    if !cap.rights.contains(Rights::WRITE) { return ERR_INVALID_RIGHTS; }
                    let woken = ep.endpoint_send(caller_ref, arg1, object_table);
                    if let Some(woken_ref) = woken {
                        unsafe { crate::scheduler::SCHEDULER.enqueue(woken_ref); }
                    } else {
                        unsafe { crate::scheduler::yield_thread(); }
                    }
                    SYSCALL_OK
                },
                OP_RECEIVE => {
                    if !cap.rights.contains(Rights::READ) { return ERR_INVALID_RIGHTS; }
                    let woken = ep.endpoint_receive(caller_ref, object_table);
                    if let Some(woken_ref) = woken {
                        unsafe { crate::scheduler::SCHEDULER.enqueue(woken_ref); }
                    } else {
                        unsafe { crate::scheduler::yield_thread(); }
                    }
                    SYSCALL_OK
                },
                OP_CALL => {
                    // Atomic RPC primitive: Requires both WRITE (to send) and READ (to receive reply)
                    if !cap.rights.contains(Rights::WRITE) || !cap.rights.contains(Rights::READ) {
                        return ERR_INVALID_RIGHTS;
                    }
                    if let Some(receiver_ref) = ep.recv_queue.pop(object_table) {
                        let (_, phys_recv) = object_table.resolve(receiver_ref).unwrap();
                        let recv_ptr = phys_recv as *mut crate::thread::TCB;
                        unsafe {
                            core::ptr::write(&raw mut (*recv_ptr).ipc_payload, arg1);
                            core::ptr::write(&raw mut (*recv_ptr).state, crate::thread::ThreadState::Ready);
                            crate::scheduler::SCHEDULER.enqueue_front(receiver_ref);
                        }
                        // Atomically block caller waiting for reply on this endpoint
                        caller_tcb.state = crate::thread::ThreadState::BlockedReceive;
                        ep.recv_queue.push(caller_ref, object_table);
                        unsafe { crate::scheduler::yield_thread(); }
                        SYSCALL_OK
                    } else {
                        ERR_NO_RECEIVER
                    }
                },
                _ => ERR_UNSUPPORTED_OP,
            }
        },
        _ => ERR_INVALID_KIND,
    }
}
