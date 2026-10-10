use crate::thread::{TCB, ThreadState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectKind {
    Null,
    Endpoint,
    TCB,
    VSpace,
    CNode,
    Untyped,
}

impl ObjectKind {
    /// Returns the (size, alignment) required for the kernel object.
    pub fn layout(&self) -> (usize, usize) {
        match self {
            ObjectKind::Endpoint => (128, 8),
            ObjectKind::TCB => (512, 8),
            ObjectKind::VSpace => (4096, 4096),
            ObjectKind::Untyped => (0, 1),
            _ => (0, 1),
        }
    }
}

/// An opaque reference to a kernel object.
/// Identity is strictly abstract; it does not encode the physical address.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct ObjectRef(u64);

impl ObjectRef {

    pub const fn new(index: u32, generation: u32) -> Self {
        Self(((generation as u64) << 32) | (index as u64))
    }

    pub const fn null() -> Self {
        Self(0)
    }

    pub fn index(&self) -> u32 {
        (self.0 & 0xFFFFFFFF) as u32
    }

    pub fn generation(&self) -> u32 {
        (self.0 >> 32) as u32
    }
}

/// The physical representation of an Untyped memory block.
/// The watermark is strictly an offset from physical_base.
/// Invariant: 0 <= watermark <= size.
#[derive(Debug, Clone, Copy)]
pub struct Untyped {
    pub physical_base: u64,
    pub size: u64,
    pub watermark: u64, 
}

#[derive(Debug, Clone, Copy)]
pub enum ObjectState {
    Empty { next_free_index: u32 },
    Active {
        kind: ObjectKind,
        physical_address: u64,
    },
}

#[derive(Debug, Clone, Copy)]
pub struct ObjectEntry {
    pub generation: u32,
    pub state: ObjectState,
}

pub const OBJECT_TABLE_SIZE: usize = 4096;

/// The central, kernel-private object registry.
/// Resolves an ObjectRef to its physical placement in bounded O(1) time.
pub struct ObjectTable {
    entries: [ObjectEntry; OBJECT_TABLE_SIZE],
    head_free_index: u32,
}

impl ObjectTable {
    pub const fn new() -> Self {
        let mut entries = [ObjectEntry {
            generation: 0,
            state: ObjectState::Empty { next_free_index: 0 },
        }; OBJECT_TABLE_SIZE];
        
        let mut i = 1;
        while i < OBJECT_TABLE_SIZE {
            entries[i].generation = 1; 
            entries[i].state = ObjectState::Empty { next_free_index: (i + 1) as u32 };
            i += 1;
        }
        
        entries[OBJECT_TABLE_SIZE - 1].state = ObjectState::Empty { next_free_index: 0 };
        
        Self {
            entries,
            head_free_index: 1,
        }
    }

    /// Checks if the table has capacity without mutating state.
    pub fn has_free_slot(&self) -> bool {
        self.head_free_index != 0
    }

    /// Claims a free slot, registers the object, and returns its opaque ObjectRef.
    pub fn register_object(&mut self, kind: ObjectKind, physical_address: u64) -> Option<ObjectRef> {
        if self.head_free_index == 0 {
            return None; 
        }

        let index = self.head_free_index as usize;
        let entry = &mut self.entries[index];

        if let ObjectState::Empty { next_free_index } = entry.state {
            self.head_free_index = next_free_index;
        } else {
            panic!("Fatal: ObjectTable free list corrupted");
        }

        entry.state = ObjectState::Active {
            kind,
            physical_address,
        };

        Some(ObjectRef::new(index as u32, entry.generation))
    }

    /// Resolves an ObjectRef in bounded O(1) time. 
    /// Validates generation matching and active state.
    pub fn resolve(&self, reference: ObjectRef) -> Option<(ObjectKind, u64)> {
        let index = reference.index() as usize;

        if index == 0 || index >= OBJECT_TABLE_SIZE {
            return None;
        }

        let entry = &self.entries[index];

        if entry.generation != reference.generation() {
            return None; 
        }

        if let ObjectState::Active { kind, physical_address } = entry.state {
            Some((kind, physical_address))
        } else {
            None
        }
    }
    
    /// Destroys the object identity, increments the slot generation, 
    /// and returns the slot to the free list.
    pub fn destroy_object(&mut self, reference: ObjectRef) {
        let index = reference.index() as usize;
        
        if index == 0 || index >= OBJECT_TABLE_SIZE {
            return;
        }

        let entry = &mut self.entries[index];
        
        if entry.generation == reference.generation() {
            if let Some(next_gen) = entry.generation.checked_add(1) {
                entry.generation = next_gen;
                entry.state = ObjectState::Empty { next_free_index: self.head_free_index };
                self.head_free_index = index as u32;
            } else {
                // Generation exhausted. Permanently retire this slot to prevent ABA alias.
                // By setting next_free_index to 0 and NOT updating head_free_index, 
                // this slot is permanently orphaned from the free list.
                entry.state = ObjectState::Empty { next_free_index: 0 };
            }
            
        }
    }
}


#[derive(Debug, Clone, Copy)]
pub struct WaitQueue {
    pub head: Option<ObjectRef>,
    pub tail: Option<ObjectRef>,
}

impl WaitQueue {
    pub const fn empty() -> Self {
        Self { head: None, tail: None }
    }

    pub fn push(&mut self, tcb_ref: ObjectRef, object_table: &ObjectTable) {
        let (_, phys_new) = object_table.resolve(tcb_ref).expect("Queue push: Invalid ObjectRef");
        let new_tcb_ptr = phys_new as *mut TCB;
        
        unsafe {
            core::ptr::write(&raw mut (*new_tcb_ptr).next_in_queue, None);
        }

        if let Some(tail_ref) = self.tail {
            let (_, phys_tail) = object_table.resolve(tail_ref).unwrap();
            let tail_tcb_ptr = phys_tail as *mut TCB;
            unsafe {
                core::ptr::write(&raw mut (*tail_tcb_ptr).next_in_queue, Some(tcb_ref));
            }
        } else {
            self.head = Some(tcb_ref);
        }
        
        self.tail = Some(tcb_ref);
    }

    pub fn pop(&mut self, object_table: &ObjectTable) -> Option<ObjectRef> {
        let head_ref = self.head?;
        let (_, phys_head) = object_table.resolve(head_ref).expect("Queue pop: Stale ObjectRef");
        let head_tcb_ptr = phys_head as *mut TCB;
        
        unsafe {
            let next_ptr = &raw mut (*head_tcb_ptr).next_in_queue;
            let next_ref = core::ptr::read(next_ptr);
            
            self.head = next_ref;
            if self.head.is_none() {
                self.tail = None;
            }
            core::ptr::write(next_ptr, None);
        }
        
        Some(head_ref)
    }
}

pub struct Endpoint {
    pub send_queue: WaitQueue,
    pub recv_queue: WaitQueue,
}

impl Endpoint {
    pub const fn new() -> Self {
        Self {
            send_queue: WaitQueue::empty(),
            recv_queue: WaitQueue::empty(),
        }
    }

    /// Attempts a synchronous SEND. Returns Some(woken_receiver) if rendezvous succeeded, None if blocked.
    pub fn endpoint_send(&mut self, sender_ref: ObjectRef, payload: u64, object_table: &ObjectTable) -> Option<ObjectRef> {
        if let Some(receiver_ref) = self.recv_queue.pop(object_table) {
            let (_, phys_recv) = object_table.resolve(receiver_ref).unwrap();
            let recv_ptr = phys_recv as *mut TCB;
            
            unsafe {
                core::ptr::write(&raw mut (*recv_ptr).ipc_payload, payload);
                core::ptr::write(&raw mut (*recv_ptr).state, ThreadState::Ready);
            }
            Some(receiver_ref) // Return the woken thread!
        } else {
            let (_, phys_send) = object_table.resolve(sender_ref).unwrap();
            let send_ptr = phys_send as *mut TCB;
            
            unsafe {
                core::ptr::write(&raw mut (*send_ptr).ipc_payload, payload);
                core::ptr::write(&raw mut (*send_ptr).state, ThreadState::BlockedSend);
            }
            self.send_queue.push(sender_ref, object_table);
            None
        }
    }

    /// Attempts a synchronous RECEIVE. Returns Some(woken_sender) if rendezvous succeeded, None if blocked.
    pub fn endpoint_receive(&mut self, receiver_ref: ObjectRef, object_table: &ObjectTable) -> Option<ObjectRef> {
        if let Some(sender_ref) = self.send_queue.pop(object_table) {
            let (_, phys_send) = object_table.resolve(sender_ref).unwrap();
            let send_ptr = phys_send as *mut TCB;
            
            let (_, phys_recv) = object_table.resolve(receiver_ref).unwrap();
            let recv_ptr = phys_recv as *mut TCB;
            
            unsafe {
                let payload = core::ptr::read(&raw const (*send_ptr).ipc_payload);
                core::ptr::write(&raw mut (*recv_ptr).ipc_payload, payload);
                core::ptr::write(&raw mut (*send_ptr).state, ThreadState::Ready);
            }
            Some(sender_ref) // Return the woken thread!
        } else {
            let (_, phys_recv) = object_table.resolve(receiver_ref).unwrap();
            let recv_ptr = phys_recv as *mut TCB;
            
            unsafe {
                core::ptr::write(&raw mut (*recv_ptr).state, ThreadState::BlockedReceive);
            }
            self.recv_queue.push(receiver_ref, object_table);
            None
        }
    }
}