use crate::object::ObjectRef;
use crate::thread::TCB;
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
            // 1. Queue rotation
            sched.enqueue(prev_ref);
            sched.current = Some(next_ref);

            // 2. Object resolution via the capability data plane
            let object_table = &mut KERNEL_OBJECT_TABLE;
            
            let (_, prev_phys) = object_table.resolve(prev_ref).unwrap();
            let prev_tcb = &mut *(prev_phys as *mut TCB);

            let (_, next_phys) = object_table.resolve(next_ref).unwrap();
            let next_tcb = &*(next_phys as *const TCB);

            // 3. Execution context switch
            crate::thread::switch_context(core::ptr::addr_of_mut!(prev_tcb.rsp), next_tcb.rsp);
        }
    }
}