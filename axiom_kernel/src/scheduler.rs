use crate::object::ObjectRef;
use crate::thread::{TCB, ThreadState};
use crate::capability::KERNEL_OBJECT_TABLE;

pub const MAX_READY: usize = 16;

pub struct Scheduler {
    pub current: Option<ObjectRef>,
    queue: [Option<ObjectRef>; MAX_READY],
    head: usize,
    tail: usize,
}



impl Scheduler {
    pub const fn new() -> Self {
        Self {
            current: None,
            queue: [None; MAX_READY],
            head: 0,
            tail: 0,
        }
    }

    pub fn enqueue(&mut self, thread: ObjectRef) {
        self.queue[self.tail] = Some(thread);
        self.tail = (self.tail + 1) % MAX_READY;
    }

    pub fn dequeue(&mut self) -> Option<ObjectRef> {
        if self.head == self.tail {
            return None;
        }
        let thread = self.queue[self.head].take();
        self.head = (self.head + 1) % MAX_READY;
        thread
    }
}

pub static mut SCHEDULER: Scheduler = Scheduler::new();

/// The cooperative yield primitive. 
/// Rotates the current thread to the back of the queue and switches to the next.
pub unsafe fn yield_thread() {
    let sched = &mut SCHEDULER;
    
    if let Some(next_ref) = sched.dequeue() {
        if let Some(prev_ref) = sched.current {
            let object_table = &KERNEL_OBJECT_TABLE;
            
            // 1. Resolve outgoing TCB securely
            let (_, prev_phys) = object_table.resolve(prev_ref).unwrap();
            let prev_tcb_ptr = prev_phys as *mut TCB;
            
            // 2. State-Aware Rotation: Only enqueue if not blocked!
            let prev_state = core::ptr::read(&raw const (*prev_tcb_ptr).state);
            if prev_state == ThreadState::Ready || prev_state == ThreadState::Running {
                // Safely update state to Ready if it was Running
                core::ptr::write(&raw mut (*prev_tcb_ptr).state, ThreadState::Ready);
                sched.enqueue(prev_ref);
            }

            // 3. Resolve incoming TCB
            sched.current = Some(next_ref);
            let (_, next_phys) = object_table.resolve(next_ref).unwrap();
            let next_tcb_ptr = next_phys as *mut TCB;
            
            core::ptr::write(&raw mut (*next_tcb_ptr).state, ThreadState::Running);

            // 4. UB-Free Context Switch
            crate::thread::switch_context(
                &raw mut (*prev_tcb_ptr).rsp, 
                core::ptr::read(&raw const (*next_tcb_ptr).rsp)
            );
        }
    }
}