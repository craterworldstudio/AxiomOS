use crate::object::{ObjectKind, ObjectRef, ObjectTable, Untyped};

pub const OP_SEND: u64    = 0;
pub const OP_RECEIVE: u64 = 1;
pub const OP_CALL: u64    = 2;
pub const OP_MINT: u64    = 3;
pub const OP_REVOKE: u64  = 4;
pub const OP_DELETE: u64  = 5;
pub const OP_CONSOLE_WRITE: u64  = 6;
pub const OP_CONSOLE_CLEAR: u64  = 7;
pub const OP_CONSOLE_BANNER: u64 = 8;
pub const OP_CONSOLE_COLOR: u64  = 9;
pub const OP_CONSOLE_GETCHAR: u64 = 10;
pub const OP_CONSOLE_SET_CURSOR: u64 = 11;
pub const OP_TASK_SPAWN: u64 = 12;
pub const OP_TASK_EXIT: u64  = 13;
pub const OP_TASK_WAIT: u64  = 14;
pub const OP_TASK_LIST: u64  = 15;
pub const OP_TASK_KILL: u64  = 16;
pub const OP_STORAGE_READ: u64   = 20;
pub const OP_STORAGE_WRITE: u64  = 21;
pub const OP_CHANNEL_CREATE: u64 = 22;
pub const OP_CHANNEL_READ: u64   = 23;
pub const OP_CHANNEL_WRITE: u64  = 24;
pub const OP_SYS_REBOOT: u64     = 30;
pub const OP_SYS_SHUTDOWN: u64   = 31;
pub const OP_KERNEL_PANIC: u64   = 99;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct ProcessInfo {
    pub pid: u32,
    pub ppid: u32,
    pub state: u32,
    pub _pad: u32,
    pub cr3: u64,
    pub name: [u8; 32],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct TaskSpawnParams {
    pub binary_ptr: u64,
    pub binary_len: u64,
    pub user_id: u32,
    pub rights: u32,
    pub storage_cap: u64,
    pub console_cap: u64,
    pub user_name: [u8; 32],
    pub map_ctx_ptr: u64,
    pub blob_ctx_ptr: u64,
    pub argc: u64,
    pub argv_ptrs: [u64; 8],
    pub stdin_cap: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct CapKeyring {
    pub user_id: u32,
    pub rights: u32,
    pub storage_cap: u64,
    pub console_cap: u64,
    pub user_name: [u8; 32],
    pub stdin_cap: u64,
}

pub static mut NEXT_PID: u32 = 2;


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

pub const CNODE_SLOTS: usize = 64;
pub struct CNode {
    pub slots: [CapabilitySlot; CNODE_SLOTS],
}

impl CNode {
    pub const fn new() -> Self {
        Self {
            slots: [CapabilitySlot::empty(); CNODE_SLOTS],
        }
    }

    pub unsafe fn init_in_place(ptr: *mut CNode) {
        let slots_ptr = core::ptr::addr_of_mut!((*ptr).slots);
        for i in 0..CNODE_SLOTS {
            core::ptr::write((*slots_ptr).as_mut_ptr().add(i), CapabilitySlot::empty());
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

    if kind == ObjectKind::TCB {
        unsafe {
            core::ptr::write_bytes(crate::memory::phys_to_virt_ptr::<u8>(aligned_phys), 0, core::mem::size_of::<crate::thread::TCB>());
        }
    } else if kind == ObjectKind::CNode {
        unsafe {
            CNode::init_in_place(crate::memory::phys_to_virt_ptr::<CNode>(aligned_phys));
        }
    }

    let cap = Capability { object: obj_ref, kind, rights: Rights::ALL };
    cnode.slots[cnode_slot_idx].insert(cap);

    Ok((obj_ref, cnode_slot_idx))
}

pub unsafe fn sys_task_spawn(
    caller_ref: ObjectRef,
    _caller_tcb: &mut crate::thread::TCB,
    cnode: &mut CNode,
    object_table: &mut ObjectTable,
    params_ptr: u64,
) -> u64 {
    crate::serial_print("[KERNEL] sys_task_spawn invoked with params_ptr=");
    crate::serial_print_hex(params_ptr);
    crate::serial_print("\n");

    let params_ptr = params_ptr as *const TaskSpawnParams;
    if params_ptr.is_null() || (params_ptr as u64) >= 0x0000_8000_0000_0000 {
        return ERR_INVALID_HANDLE;
    }
    let params = core::ptr::read(params_ptr);
    crate::serial_print("[KERNEL] binary_ptr=");
    crate::serial_print_hex(params.binary_ptr);
    crate::serial_print(" len=");
    crate::serial_print_hex(params.binary_len);
    crate::serial_print("\n");
    if params.binary_ptr == 0 || params.binary_len == 0 || params.binary_len > 0x100000 {
        return ERR_INVALID_HANDLE;
    }

    let pml4_frame = match crate::memory::allocate_frame() {
        Some(f) => f,
        None => return 0xFFFFFFFF_00000001,
    };

    let alloc = match crate::memory::KERNEL_FRAME_ALLOCATOR.as_mut() {
        Some(a) => a,
        None => return 0xFFFFFFFF_00000002,
    };

    let child_vspace = match crate::memory::VSpace::create(pml4_frame.start_address, alloc) {
        Ok(v) => v,
        Err(_) => {
            alloc.deallocate_frame(pml4_frame);
            return 0xFFFFFFFF_00000003;
        }
    };
    crate::serial_print("[KERNEL] sys_task_spawn: VSpace created, copying binary...\n");

    // 16 execution pages at 0x500000 (64 KiB)
    let binary_slice = core::slice::from_raw_parts(params.binary_ptr as *const u8, params.binary_len as usize);
    for i in 0..16 {
        let frame = match alloc.allocate_frame() {
            Some(f) => f,
            None => return 0xFFFFFFFF_00000004,
        };
        let dst = crate::memory::phys_to_virt(frame.start_address);
        let offset = i * 4096;
        if offset < params.binary_len as usize {
            let chunk = core::cmp::min(4096, params.binary_len as usize - offset);
            core::ptr::copy_nonoverlapping(binary_slice.as_ptr().add(offset), dst, chunk);
            if chunk < 4096 {
                core::ptr::write_bytes(dst.add(chunk), 0, 4096 - chunk);
            }
        } else {
            core::ptr::write_bytes(dst, 0, 4096);
        }

        let map_res = child_vspace.map_page(
            0x500000 + (i as u64) * 4096,
            frame.start_address,
            crate::memory::PageFlags::PRESENT | crate::memory::PageFlags::USER | crate::memory::PageFlags::WRITABLE,
            alloc,
        );
        if map_res.is_err() {
            return 0xFFFFFFFF_00000005;
        }
    }

    // 1 Data page at 0x600000
    let data_frame = match alloc.allocate_frame() {
        Some(f) => f,
        None => return 0xFFFFFFFF_00000006,
    };
    let data_ptr = crate::memory::phys_to_virt(data_frame.start_address);
    core::ptr::write_bytes(data_ptr, 0, 4096);

    let keyring_dst = data_ptr as *mut CapKeyring;
    let child_keyring = CapKeyring {
        user_id: params.user_id,
        rights: params.rights,
        storage_cap: 0x600040,
        console_cap: params.console_cap,
        user_name: params.user_name,
        stdin_cap: params.stdin_cap,
    };
    core::ptr::write(keyring_dst, child_keyring);

    if params.map_ctx_ptr != 0 && params.map_ctx_ptr < 0x0000_8000_0000_0000 {
        core::ptr::copy_nonoverlapping(params.map_ctx_ptr as *const u8, data_ptr.add(0x40), 904);
    } else if params.blob_ctx_ptr != 0 && params.blob_ctx_ptr < 0x0000_8000_0000_0000 {
        core::ptr::copy_nonoverlapping(params.blob_ctx_ptr as *const u8, data_ptr.add(0x40), 1032);
    }

    // Setup argv table (at 0x480) and argv strings (at 0x500)
    let argc = core::cmp::min(params.argc as usize, 8);
    let mut str_offset = 0x500;
    let argv_table = data_ptr.add(0x480) as *mut u64;

    for i in 0..argc {
        let arg_src = params.argv_ptrs[i] as *const u8;
        if !arg_src.is_null() && (arg_src as u64) < 0x0000_8000_0000_0000 {
            let mut slen = 0;
            while slen < 63 && *arg_src.add(slen) != 0 {
                slen += 1;
            }
            if str_offset + slen + 1 <= 4096 {
                core::ptr::copy_nonoverlapping(arg_src, data_ptr.add(str_offset), slen);
                *data_ptr.add(str_offset + slen) = 0;
                *argv_table.add(i) = 0x600000 + str_offset as u64;
                str_offset += slen + 1;
            } else {
                *argv_table.add(i) = 0;
            }
        } else {
            *argv_table.add(i) = 0;
        }
    }
    *argv_table.add(argc) = 0;

    let _ = child_vspace.map_page(
        0x600000,
        data_frame.start_address,
        crate::memory::PageFlags::PRESENT | crate::memory::PageFlags::USER | crate::memory::PageFlags::WRITABLE,
        alloc,
    );

    // 16 user stack pages at 0x7F0000..0x800000
    for s in 0..16 {
        let stack_frame = match alloc.allocate_frame() {
            Some(f) => f,
            None => return 0xFFFFFFFF_00000007,
        };
        let _ = child_vspace.map_page(
            0x7F0000 + (s as u64) * 4096,
            stack_frame.start_address,
            crate::memory::PageFlags::PRESENT | crate::memory::PageFlags::USER | crate::memory::PageFlags::WRITABLE,
            alloc,
        );
    }

    let kstack_phys = match alloc.allocate_contiguous(crate::memory::KERNEL_STACK_PAGES) {
        Some(addr) => addr,
        None => return 0xFFFFFFFF_00000008,
    };
    let kstack_virt_base = crate::memory::phys_to_virt(kstack_phys) as u64;
    let kstack_top = kstack_virt_base + crate::memory::KERNEL_STACK_SIZE;
    core::ptr::write_volatile(kstack_virt_base as *mut u64, 0xDEAD_BEEF_CAFE_0005);

    let child_vspace_ref = match object_table.register_object(ObjectKind::VSpace, child_vspace.pml4_phys) {
        Some(r) => r,
        None => return 0xFFFFFFFF_00000009,
    };

    let child_cnode_frame = match alloc.allocate_frame() {
        Some(f) => f,
        None => return 0xFFFFFFFF_0000000A,
    };
    let child_cnode_ptr = crate::memory::phys_to_virt_ptr::<CNode>(child_cnode_frame.start_address);
    CNode::init_in_place(child_cnode_ptr);
    let child_cnode = &mut *child_cnode_ptr;
    let child_cnode_ref = match object_table.register_object(ObjectKind::CNode, child_cnode_frame.start_address) {
        Some(r) => r,
        None => return 0xFFFFFFFF_0000000B,
    };

    let child_tcb_frame = match alloc.allocate_frame() {
        Some(f) => f,
        None => return 0xFFFFFFFF_0000000C,
    };
    let child_tcb_ptr = crate::memory::phys_to_virt_ptr::<crate::thread::TCB>(child_tcb_frame.start_address);
    core::ptr::write_bytes(child_tcb_ptr as *mut u8, 0, core::mem::size_of::<crate::thread::TCB>());
    let child_tcb = &mut *child_tcb_ptr;
    let child_tcb_ref = match object_table.register_object(ObjectKind::TCB, child_tcb_frame.start_address) {
        Some(r) => r,
        None => return 0xFFFFFFFF_0000000D,
    };

    let child_pid = NEXT_PID;
    NEXT_PID += 1;

    child_cnode.slots[1].insert(Capability {
        object: child_cnode_ref,
        kind: ObjectKind::CNode,
        rights: Rights::ALL,
    });
    child_cnode.slots[2].insert(Capability {
        object: child_vspace_ref,
        kind: ObjectKind::VSpace,
        rights: Rights::ALL,
    });
    child_cnode.slots[3].insert(Capability {
        object: child_tcb_ref,
        kind: ObjectKind::TCB,
        rights: Rights::ALL,
    });

    // Slot 5 (STDOUT / Output):
    if params.console_cap != 0 {
        let h5 = CapHandle(params.console_cap);
        let s5 = h5.index() as usize;
        if s5 < CNODE_SLOTS && cnode.slots[s5].is_valid_generation(h5.generation()) {
            if let Some(c5) = &cnode.slots[s5].capability {
                child_cnode.slots[5].insert(*c5);
            }
        }
    } else if let Some(c5) = &cnode.slots[5].capability {
        child_cnode.slots[5].insert(*c5);
    }

    // Slot 6 (STDIN / Input):
    if params.stdin_cap != 0 {
        let h6 = CapHandle(params.stdin_cap);
        let s6 = h6.index() as usize;
        if s6 < CNODE_SLOTS && cnode.slots[s6].is_valid_generation(h6.generation()) {
            if let Some(c6) = &cnode.slots[s6].capability {
                child_cnode.slots[6].insert(*c6);
            }
        }
    } else if let Some(c6) = &cnode.slots[6].capability {
        child_cnode.slots[6].insert(*c6);
    }

    if let Some(c7) = &cnode.slots[7].capability {
        child_cnode.slots[7].insert(*c7);
    }

    child_tcb.pid = child_pid;
    child_tcb.parent = Some(caller_ref);
    child_tcb.state = crate::thread::ThreadState::Ready;
    child_tcb.frame_kind = crate::thread::FrameKind::Interrupt;
    child_tcb.cspace = Some(child_cnode_ref);
    child_tcb.vspace = Some(child_vspace_ref);
    child_tcb.stack_base = kstack_virt_base;
    child_tcb.data_frame_phys = data_frame.start_address;
    child_tcb.exit_code = 0;

    let mut proc_name = [0u8; 32];
    let arg0_src = params.argv_ptrs[0] as *const u8;
    if !arg0_src.is_null() && (arg0_src as u64) < 0x0000_8000_0000_0000 {
        let mut slen = 0;
        while slen < 31 && *arg0_src.add(slen) != 0 {
            slen += 1;
        }
        core::ptr::copy_nonoverlapping(arg0_src, proc_name.as_mut_ptr(), slen);
    }
    child_tcb.name = proc_name;

    child_tcb.rsp = crate::thread::TCB::forge_user_stack_args(
        kstack_top,
        0x500000,
        0x800000,
        argc as u64,
        0x600480,
        0x600000,
    );

    crate::scheduler::SCHEDULER.enqueue_front(child_tcb_ref);

    crate::serial_print("[KERNEL] Spawned child task PID ");
    crate::serial_print_hex(child_pid as u64);
    crate::serial_print(" with isolated VSpace CR3=");
    crate::serial_print_hex(child_vspace.pml4_phys);
    crate::serial_print("\n");

    child_pid as u64
}

pub unsafe fn sys_task_exit(
    caller_tcb: &mut crate::thread::TCB,
    object_table: &mut ObjectTable,
    exit_code: u64,
) -> u64 {
    caller_tcb.state = crate::thread::ThreadState::Terminated;
    caller_tcb.exit_code = exit_code;

    crate::serial_print("[KERNEL] Task PID ");
    crate::serial_print_hex(caller_tcb.pid as u64);
    crate::serial_print(" exited with code ");
    crate::serial_print_hex(exit_code);
    crate::serial_print("\n");

    // Close any Channel write capabilities held by this exiting task
    if let Some(cspace_ref) = caller_tcb.cspace {
        if let Some((_, cnode_phys)) = object_table.resolve(cspace_ref) {
            let caller_cnode = &mut *(crate::memory::phys_to_virt_ptr::<CNode>(cnode_phys));
            for s in 0..CNODE_SLOTS {
                if let Some(cap) = &caller_cnode.slots[s].capability {
                    if cap.kind == ObjectKind::Channel && cap.rights.contains(Rights::WRITE) {
                        if let Some((_, chan_phys)) = object_table.resolve(cap.object) {
                            let chan = &mut *(crate::memory::phys_to_virt_ptr::<crate::object::Channel>(chan_phys));
                            chan.close_writer(object_table);
                        }
                    }
                }
            }
        }
    }

    if let Some(parent_ref) = caller_tcb.parent {
        if let Some((_, parent_phys)) = object_table.resolve(parent_ref) {
            let parent_tcb = &mut *(crate::memory::phys_to_virt_ptr::<crate::thread::TCB>(parent_phys));
            if parent_tcb.state == crate::thread::ThreadState::BlockedReceive {
                parent_tcb.state = crate::thread::ThreadState::Ready;
                crate::scheduler::SCHEDULER.enqueue_front(parent_ref);
            }
        }
    }

    crate::scheduler::yield_thread();
    SYSCALL_OK
}

pub unsafe fn sys_task_wait(
    caller_ref: ObjectRef,
    caller_tcb: &mut crate::thread::TCB,
    object_table: &mut ObjectTable,
    target_pid: u32,
    out_blob_ptr: *mut u8,
) -> u64 {
    loop {
        let (child_ref, child_phys) = match object_table.find_tcb_by_pid(target_pid) {
            Some(c) => c,
            None => return ERR_INVALID_HANDLE,
        };

        let child_tcb = &mut *(crate::memory::phys_to_virt_ptr::<crate::thread::TCB>(child_phys));
        if child_tcb.state == crate::thread::ThreadState::Terminated {
            let exit_code = child_tcb.exit_code;

            if !out_blob_ptr.is_null() && (out_blob_ptr as u64) < 0x0000_8000_0000_0000 && child_tcb.data_frame_phys != 0 {
                core::ptr::copy_nonoverlapping(
                    crate::memory::phys_to_virt(child_tcb.data_frame_phys + 0x40),
                    out_blob_ptr,
                    1032,
                );
            }

            crate::serial_print("[KERNEL] Reaped child task PID ");
            crate::serial_print_hex(target_pid as u64);
            crate::serial_print(" exit_code=");
            crate::serial_print_hex(exit_code);
            crate::serial_print("\n");

            if let Some(cspace_ref) = child_tcb.cspace {
                object_table.destroy_object(cspace_ref);
            }
            if let Some(vspace_ref) = child_tcb.vspace {
                object_table.destroy_object(vspace_ref);
            }
            object_table.destroy_object(child_ref);

            return exit_code;
        }

        caller_tcb.state = crate::thread::ThreadState::BlockedReceive;
        crate::scheduler::yield_thread();
    }
}

pub unsafe fn sys_task_list(
    _caller_tcb: &mut crate::thread::TCB,
    object_table: &ObjectTable,
    out_buf_ptr: u64,
    max_count: u64,
) -> u64 {
    if out_buf_ptr == 0 || out_buf_ptr >= 0x0000_8000_0000_0000 || max_count == 0 {
        return 0;
    }
    let limit = core::cmp::min(max_count as usize, 64);
    let out_slice = core::slice::from_raw_parts_mut(out_buf_ptr as *mut ProcessInfo, limit);
    let mut count = 0;

    for i in 1..crate::object::OBJECT_TABLE_SIZE {
        if let crate::object::ObjectState::Active { kind: ObjectKind::TCB, physical_address } = object_table.entries[i].state {
            if count >= limit {
                break;
            }
            let tcb = &*(crate::memory::phys_to_virt_ptr::<crate::thread::TCB>(physical_address));

            // Resolve parent PID
            let ppid = if let Some(parent_ref) = tcb.parent {
                if let Some((_, parent_phys)) = object_table.resolve(parent_ref) {
                    let parent_tcb = &*(crate::memory::phys_to_virt_ptr::<crate::thread::TCB>(parent_phys));
                    parent_tcb.pid
                } else {
                    0
                }
            } else {
                0
            };

            // Resolve CR3
            let cr3 = if let Some(vspace_ref) = tcb.vspace {
                if let Some((_, vspace_phys)) = object_table.resolve(vspace_ref) {
                    vspace_phys
                } else {
                    crate::memory::BOOT_CR3
                }
            } else {
                crate::memory::BOOT_CR3
            };

            out_slice[count] = ProcessInfo {
                pid: tcb.pid,
                ppid,
                state: tcb.state as u32,
                _pad: 0,
                cr3,
                name: tcb.name,
            };
            count += 1;
        }
    }

    count as u64
}

pub unsafe fn sys_task_kill(
    caller_tcb: &mut crate::thread::TCB,
    object_table: &mut ObjectTable,
    target_pid: u32,
    _signal: u64,
) -> u64 {
    // Security check: Protect Genesis PID 1
    if target_pid <= 1 {
        return ERR_INVALID_RIGHTS;
    }

    let (target_ref, target_phys) = match object_table.find_tcb_by_pid(target_pid) {
        Some(t) => t,
        None => return ERR_INVALID_HANDLE,
    };

    let target_tcb = &mut *(crate::memory::phys_to_virt_ptr::<crate::thread::TCB>(target_phys));
    if target_tcb.state == crate::thread::ThreadState::Terminated {
        // Reaping already-terminated zombie process
        if let Some(cspace_ref) = target_tcb.cspace {
            object_table.destroy_object(cspace_ref);
        }
        if let Some(vspace_ref) = target_tcb.vspace {
            object_table.destroy_object(vspace_ref);
        }
        object_table.destroy_object(target_ref);

        crate::serial_print("[KERNEL] Purged zombie process PID ");
        crate::serial_print_hex(target_pid as u64);
        crate::serial_print("\n");
        return SYSCALL_OK;
    }

    target_tcb.state = crate::thread::ThreadState::Terminated;
    target_tcb.exit_code = 137; // SIGKILL equivalent

    crate::serial_print("[KERNEL] Process PID ");
    crate::serial_print_hex(target_pid as u64);
    crate::serial_print(" terminated by PID ");
    crate::serial_print_hex(caller_tcb.pid as u64);
    crate::serial_print("\n");

    // Close any Channel write capabilities held by the killed task
    if let Some(cspace_ref) = target_tcb.cspace {
        if let Some((_, cnode_phys)) = object_table.resolve(cspace_ref) {
            let target_cnode = &mut *(crate::memory::phys_to_virt_ptr::<CNode>(cnode_phys));
            for s in 0..CNODE_SLOTS {
                if let Some(cap) = &target_cnode.slots[s].capability {
                    if cap.kind == ObjectKind::Channel && cap.rights.contains(Rights::WRITE) {
                        if let Some((_, chan_phys)) = object_table.resolve(cap.object) {
                            let chan = &mut *(crate::memory::phys_to_virt_ptr::<crate::object::Channel>(chan_phys));
                            chan.close_writer(object_table);
                        }
                    }
                }
            }
        }
    }

    // Unblock parent if waiting
    if let Some(parent_ref) = target_tcb.parent {
        if let Some((_, parent_phys)) = object_table.resolve(parent_ref) {
            let parent_tcb = &mut *(crate::memory::phys_to_virt_ptr::<crate::thread::TCB>(parent_phys));
            if parent_tcb.state == crate::thread::ThreadState::BlockedReceive {
                parent_tcb.state = crate::thread::ThreadState::Ready;
                crate::scheduler::SCHEDULER.enqueue_front(parent_ref);
            }
        }
    }

    SYSCALL_OK
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
    let caller_ref = unsafe {
        crate::scheduler::SCHEDULER.current
            .expect("sys_invoke: no current thread")
    };
    let object_table = unsafe { &mut KERNEL_OBJECT_TABLE };
    let (_, caller_phys) = object_table.resolve(caller_ref).unwrap();
    let caller_tcb = unsafe { &mut *(crate::memory::phys_to_virt_ptr::<crate::thread::TCB>(caller_phys)) };


    // Fast-path for OP_TASK_EXIT: a process can always exit itself regardless of handle!
    if operation == OP_TASK_EXIT {
        return unsafe { sys_task_exit(caller_tcb, object_table, arg1) };
    }

    let handle = CapHandle(handle_val);
    let slot_idx = handle.index() as usize;

    // Security: Bounds check slot index
    if slot_idx == 0 || slot_idx >= CNODE_SLOTS { return ERR_INVALID_HANDLE; }
    // Security: Spectre v1 mitigation
    unsafe { core::arch::asm!("lfence"); }


    // Resolve caller's private CNode or fallback to GENESIS_CNODE
    let cnode: &mut CNode = match caller_tcb.cspace {
        Some(cspace_ref) => {
            let (_, cnode_phys) = object_table.resolve(cspace_ref).unwrap();
            unsafe { &mut *(crate::memory::phys_to_virt_ptr::<CNode>(cnode_phys)) }
        },
        None => unsafe { &mut GENESIS_CNODE },
    };

    let slot = &mut cnode.slots[slot_idx];
    if !slot.is_valid_generation(handle.generation()) {
        unsafe { crate::serial_print("[SYSCALL] ERR_STALE_HANDLE!\n"); }
        return ERR_STALE_HANDLE;
    }

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
        OP_CHANNEL_CREATE => {
            if !cap.rights.contains(Rights::WRITE) && !cap.rights.contains(Rights::ALL) {
                return ERR_INVALID_RIGHTS;
            }

            let frame = match unsafe { crate::memory::allocate_frame() } {
                Some(f) => f,
                None => return 0xFFFFFFFF_00000001,
            };

            let channel_ptr = crate::memory::phys_to_virt_ptr::<crate::object::Channel>(frame.start_address);
            unsafe {
                core::ptr::write(channel_ptr, crate::object::Channel::new());
            }

            let channel_ref = match object_table.register_object(ObjectKind::Channel, frame.start_address) {
                Some(r) => r,
                None => {
                    if let Some(alloc) = unsafe { crate::memory::KERNEL_FRAME_ALLOCATOR.as_mut() } {
                        alloc.deallocate_frame(frame);
                    }
                    return ERR_CNODE_FULL;
                }
            };

            let read_slot = match cnode.find_free_slot() {
                Some(s) => s,
                None => return ERR_CNODE_FULL,
            };
            cnode.slots[read_slot].insert(Capability {
                object: channel_ref,
                kind: ObjectKind::Channel,
                rights: Rights::READ,
            });
            let read_handle = CapHandle::new(read_slot as u32, cnode.slots[read_slot].generation);

            let write_slot = match cnode.find_free_slot() {
                Some(s) => s,
                None => {
                    let _ = cnode.delete(read_slot);
                    return ERR_CNODE_FULL;
                }
            };
            cnode.slots[write_slot].insert(Capability {
                object: channel_ref,
                kind: ObjectKind::Channel,
                rights: Rights::WRITE,
            });
            let write_handle = CapHandle::new(write_slot as u32, cnode.slots[write_slot].generation);

            if arg1 != 0 && arg1 < 0x0000_8000_0000_0000 {
                let out_ptr = arg1 as *mut u64;
                unsafe {
                    core::ptr::write(out_ptr, read_handle.0);
                    core::ptr::write(out_ptr.add(1), write_handle.0);
                }
            }

            return SYSCALL_OK;
        },
        _ => {}
    }

    // 2. Object Invocation Operations (Endpoint, etc.)
    let (kind, phys_addr) = object_table.resolve(cap.object).unwrap();

    match kind {
        ObjectKind::Endpoint => {
            let ep = unsafe { &mut *(crate::memory::phys_to_virt_ptr::<crate::object::Endpoint>(phys_addr)) };

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
                        let recv_ptr = crate::memory::phys_to_virt_ptr::<crate::thread::TCB>(phys_recv);
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
                OP_CONSOLE_WRITE => {
                    if !cap.rights.contains(Rights::WRITE) { return ERR_INVALID_RIGHTS; }
                    let console = unsafe { &raw mut crate::console::CONSOLE };
                    let len = if arg2 != 0 { core::cmp::min(arg2 as usize, 8) } else { 0 };
                    if len > 0 {
                        let bytes = arg1.to_le_bytes();
                        for i in 0..len {
                            unsafe { (*console).write_char(bytes[i]); }
                        }
                    } else {
                        unsafe { (*console).write_char(arg1 as u8); }
                    }
                    SYSCALL_OK
                },
                OP_CONSOLE_CLEAR => {
                    if !cap.rights.contains(Rights::WRITE) { return ERR_INVALID_RIGHTS; }
                    let console = unsafe { &raw mut crate::console::CONSOLE };
                    unsafe { (*console).clear(); }
                    SYSCALL_OK
                },
                OP_CONSOLE_BANNER => {
                    unsafe { crate::serial_print("[KERNEL] OP_CONSOLE_BANNER invoked!\n"); }
                    if !cap.rights.contains(Rights::WRITE) { return ERR_INVALID_RIGHTS; }
                    let console = unsafe { &raw mut crate::console::CONSOLE };
                    unsafe { (*console).draw_banner(); }
                    SYSCALL_OK
                },
                OP_CONSOLE_COLOR => {
                    if !cap.rights.contains(Rights::WRITE) { return ERR_INVALID_RIGHTS; }
                    let console = unsafe { &raw mut crate::console::CONSOLE };
                    unsafe { (*console).set_color(arg1 as u32, arg2 as u32); }
                    SYSCALL_OK
                },
                OP_CONSOLE_GETCHAR => {
                    if !cap.rights.contains(Rights::READ) { return ERR_INVALID_RIGHTS; }
                    let console = unsafe { &raw mut crate::console::CONSOLE };
                    let ch = unsafe { (*console).read_char() };
                    ch as u64
                },
                OP_CONSOLE_SET_CURSOR => {
                    if !cap.rights.contains(Rights::WRITE) { return ERR_INVALID_RIGHTS; }
                    let console = unsafe { &raw mut crate::console::CONSOLE };
                    unsafe { (*console).set_cursor(arg1 as usize, arg2 as usize); }
                    SYSCALL_OK
                },
                OP_STORAGE_READ => {
                    if !cap.rights.contains(Rights::READ) { return ERR_INVALID_RIGHTS; }
                    let drive = if (arg1 >> 32) != 0 { ((arg1 >> 32) & 1) as u8 } else { 1u8 };
                    let lba = (arg1 & 0xFFFFFFFF) as u32;
                    let buf = arg2 as *mut u8;
                    unsafe {
                        match crate::ata::read_sector(drive, lba, buf) {
                            Ok(()) => SYSCALL_OK,
                            Err(_) => 0xFFFFFFFFFFFFFFFF,
                        }
                    }
                },
                OP_STORAGE_WRITE => {
                    if !cap.rights.contains(Rights::WRITE) { return ERR_INVALID_RIGHTS; }
                    let drive = if (arg1 >> 32) != 0 { ((arg1 >> 32) & 1) as u8 } else { 1u8 };
                    let lba = (arg1 & 0xFFFFFFFF) as u32;
                    let buf = arg2 as *const u8;
                    unsafe {
                        match crate::ata::write_sector(drive, lba, buf) {
                            Ok(()) => SYSCALL_OK,
                            Err(_) => 0xFFFFFFFFFFFFFFFF,
                        }
                    }
                },
                OP_KERNEL_PANIC => {
                    unsafe {
                        crate::serial_print("[KERNEL] OP_KERNEL_PANIC invoked! Scenario: ");
                        crate::serial_print_hex(arg1);
                        crate::serial_print("\n");

                        match arg1 {
                            1 => {
                                // Scenario 1: GPF (#13) via invalid segment selector
                                core::arch::asm!("mov ds, ax", in("ax") 0xFFFFu16);
                            },
                            2 => {
                                // Scenario 2: Kernel Page Fault (#14) via unmapped address
                                let bad_ptr = 0xFFFF_FFFF_DEAD_BEEF as *mut u64;
                                *bad_ptr = 0x1337;
                            },
                            3 => {
                                // Scenario 3: Double Fault (#8) via int 8
                                core::arch::asm!("int 8");
                            },
                            4 => {
                                // Scenario 4: Divide by Zero (#0)
                                let zero = 0u64;
                                let num = 42u64;
                                core::arch::asm!("div {0}", in(reg) zero, inout("rax") num => _, options(nomem, nostack));
                            },
                            _ => {
                                // Scenario 0 / default: Rust assertion failure panic
                                panic!("Substrate invariant test: manual kernel panic requested by Genesis");
                            }
                        }
                    }
                    SYSCALL_OK
                },
                OP_SYS_REBOOT => {
                    unsafe {
                        crate::serial_print("[KERNEL] OP_SYS_REBOOT invoked!\n");
                        core::arch::asm!("out dx, al", in("dx") 0x64u16, in("al") 0xFEu8, options(nomem, nostack));
                        loop {
                            core::arch::asm!("cli; hlt");
                        }
                    }
                },
                OP_SYS_SHUTDOWN => {
                    unsafe {
                        crate::serial_print("[KERNEL] OP_SYS_SHUTDOWN invoked! Powering off substrate...\n");
                        core::arch::asm!("out dx, ax", in("dx") 0x604u16, in("ax") 0x2000u16, options(nomem, nostack));
                        core::arch::asm!("out dx, ax", in("dx") 0xB004u16, in("ax") 0x2000u16, options(nomem, nostack));
                        core::arch::asm!("out dx, ax", in("dx") 0x4004u16, in("ax") 0x3400u16, options(nomem, nostack));
                        core::arch::asm!("out dx, ax", in("dx") 0x604u16, in("ax") 0x3400u16, options(nomem, nostack));
                        core::arch::asm!("out dx, ax", in("dx") 0x600u16, in("ax") 0x2000u16, options(nomem, nostack));
                        loop {
                            core::arch::asm!("cli; hlt");
                        }
                    }
                },
                OP_TASK_SPAWN => {
                    unsafe { sys_task_spawn(caller_ref, caller_tcb, cnode, object_table, arg1) }
                },
                OP_TASK_WAIT => {
                    unsafe { sys_task_wait(caller_ref, caller_tcb, object_table, arg1 as u32, arg2 as *mut u8) }
                },
                OP_TASK_EXIT => {
                    unsafe { sys_task_exit(caller_tcb, object_table, arg1) }
                },
                OP_TASK_LIST => {
                    unsafe { sys_task_list(caller_tcb, object_table, arg1, arg2) }
                },
                OP_TASK_KILL => {
                    unsafe { sys_task_kill(caller_tcb, object_table, arg1 as u32, arg2) }
                },
                _ => ERR_UNSUPPORTED_OP,
            }
        },
        ObjectKind::TCB => {
            match operation {
                OP_TASK_SPAWN => {
                    if !cap.rights.contains(Rights::EXEC) && !cap.rights.contains(Rights::ALL) {
                        return ERR_INVALID_RIGHTS;
                    }
                    unsafe { sys_task_spawn(caller_ref, caller_tcb, cnode, object_table, arg1) }
                },
                OP_TASK_WAIT => {
                    if !cap.rights.contains(Rights::READ) && !cap.rights.contains(Rights::ALL) {
                        return ERR_INVALID_RIGHTS;
                    }
                    unsafe { sys_task_wait(caller_ref, caller_tcb, object_table, arg1 as u32, arg2 as *mut u8) }
                },
                OP_TASK_EXIT => {
                    unsafe { sys_task_exit(caller_tcb, object_table, arg1) }
                },
                OP_TASK_LIST => {
                    unsafe { sys_task_list(caller_tcb, object_table, arg1, arg2) }
                },
                OP_TASK_KILL => {
                    unsafe { sys_task_kill(caller_tcb, object_table, arg1 as u32, arg2) }
                },
                _ => ERR_UNSUPPORTED_OP,
            }
        },
        ObjectKind::Channel => {
            let channel = unsafe { &mut *(crate::memory::phys_to_virt_ptr::<crate::object::Channel>(phys_addr)) };

            match operation {
                OP_CONSOLE_WRITE => {
                    if !cap.rights.contains(Rights::WRITE) { return ERR_INVALID_RIGHTS; }
                    let len = if arg2 != 0 { core::cmp::min(arg2 as usize, 8) } else { 0 };
                    if len > 0 {
                        let bytes = arg1.to_le_bytes();
                        for i in 0..len {
                            while !channel.write_byte(bytes[i]) {
                                while let Some(r) = channel.recv_queue.pop(object_table) {
                                    if let Some((_, r_phys)) = object_table.resolve(r) {
                                        let r_tcb = unsafe { &mut *(crate::memory::phys_to_virt_ptr::<crate::thread::TCB>(r_phys)) };
                                        r_tcb.state = crate::thread::ThreadState::Ready;
                                        unsafe { crate::scheduler::SCHEDULER.enqueue_front(r); }
                                    }
                                }
                                caller_tcb.state = crate::thread::ThreadState::BlockedSend;
                                channel.send_queue.push(caller_ref, object_table);
                                unsafe { crate::scheduler::yield_thread(); }
                            }
                        }
                    } else {
                        let byte = arg1 as u8;
                        while !channel.write_byte(byte) {
                            while let Some(r) = channel.recv_queue.pop(object_table) {
                                if let Some((_, r_phys)) = object_table.resolve(r) {
                                    let r_tcb = unsafe { &mut *(crate::memory::phys_to_virt_ptr::<crate::thread::TCB>(r_phys)) };
                                    r_tcb.state = crate::thread::ThreadState::Ready;
                                    unsafe { crate::scheduler::SCHEDULER.enqueue_front(r); }
                                }
                            }
                            caller_tcb.state = crate::thread::ThreadState::BlockedSend;
                            channel.send_queue.push(caller_ref, object_table);
                            unsafe { crate::scheduler::yield_thread(); }
                        }
                    }

                    while let Some(r) = channel.recv_queue.pop(object_table) {
                        if let Some((_, r_phys)) = object_table.resolve(r) {
                            let r_tcb = unsafe { &mut *(crate::memory::phys_to_virt_ptr::<crate::thread::TCB>(r_phys)) };
                            r_tcb.state = crate::thread::ThreadState::Ready;
                            unsafe { crate::scheduler::SCHEDULER.enqueue_front(r); }
                        }
                    }
                    SYSCALL_OK
                },
                OP_CONSOLE_COLOR | OP_CONSOLE_CLEAR | OP_CONSOLE_SET_CURSOR => {
                    SYSCALL_OK
                },
                OP_CHANNEL_WRITE => {
                    if !cap.rights.contains(Rights::WRITE) { return ERR_INVALID_RIGHTS; }
                    let user_buf = arg1 as *const u8;
                    let count = arg2 as usize;
                    if user_buf.is_null() || (user_buf as u64) >= 0x0000_8000_0000_0000 || count == 0 {
                        return 0;
                    }

                    let mut written = 0;
                    while written < count {
                        let byte = unsafe { *user_buf.add(written) };
                        while !channel.write_byte(byte) {
                            while let Some(r) = channel.recv_queue.pop(object_table) {
                                if let Some((_, r_phys)) = object_table.resolve(r) {
                                    let r_tcb = unsafe { &mut *(crate::memory::phys_to_virt_ptr::<crate::thread::TCB>(r_phys)) };
                                    r_tcb.state = crate::thread::ThreadState::Ready;
                                    unsafe { crate::scheduler::SCHEDULER.enqueue_front(r); }
                                }
                            }
                            caller_tcb.state = crate::thread::ThreadState::BlockedSend;
                            channel.send_queue.push(caller_ref, object_table);
                            unsafe { crate::scheduler::yield_thread(); }
                        }
                        written += 1;
                    }

                    while let Some(r) = channel.recv_queue.pop(object_table) {
                        if let Some((_, r_phys)) = object_table.resolve(r) {
                            let r_tcb = unsafe { &mut *(crate::memory::phys_to_virt_ptr::<crate::thread::TCB>(r_phys)) };
                            r_tcb.state = crate::thread::ThreadState::Ready;
                            unsafe { crate::scheduler::SCHEDULER.enqueue_front(r); }
                        }
                    }

                    written as u64
                },
                OP_CHANNEL_READ => {
                    if !cap.rights.contains(Rights::READ) { return ERR_INVALID_RIGHTS; }
                    let user_buf = arg1 as *mut u8;
                    let max_count = arg2 as usize;
                    if user_buf.is_null() || (user_buf as u64) >= 0x0000_8000_0000_0000 || max_count == 0 {
                        return 0;
                    }

                    loop {
                        if channel.count > 0 {
                            let to_read = core::cmp::min(channel.count, max_count);
                            let mut read_bytes = 0;
                            while read_bytes < to_read {
                                if let Some(b) = channel.read_byte() {
                                    unsafe { *user_buf.add(read_bytes) = b; }
                                    read_bytes += 1;
                                } else {
                                    break;
                                }
                            }

                            while let Some(w) = channel.send_queue.pop(object_table) {
                                if let Some((_, w_phys)) = object_table.resolve(w) {
                                    let w_tcb = unsafe { &mut *(crate::memory::phys_to_virt_ptr::<crate::thread::TCB>(w_phys)) };
                                    w_tcb.state = crate::thread::ThreadState::Ready;
                                    unsafe { crate::scheduler::SCHEDULER.enqueue_front(w); }
                                }
                            }

                            return read_bytes as u64;
                        }

                        if channel.writer_closed {
                            return 0;
                        }

                        caller_tcb.state = crate::thread::ThreadState::BlockedReceive;
                        channel.recv_queue.push(caller_ref, object_table);
                        unsafe { crate::scheduler::yield_thread(); }
                    }
                },
                _ => ERR_UNSUPPORTED_OP,
            }
        },
        _ => {
            ERR_INVALID_KIND
        },
    }
}
