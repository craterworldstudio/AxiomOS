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

    /// Fast-path timeslice donation / direct switch for synchronous RPC rendezvous
    pub fn enqueue_front(&mut self, thread: ObjectRef) {
        self.head = (self.head + MAX_READY - 1) % MAX_READY;
        self.queue[self.head] = Some(thread);
    }

    pub fn clear(&mut self) {
        self.queue = [None; MAX_READY];
        self.head = 0;
        self.tail = 0;
    }

    pub fn dequeue(&mut self) -> Option<ObjectRef> {
        if self.head == self.tail {
            return None;
        }
        let thread = self.queue[self.head].take();
        self.head = (self.head + 1) % MAX_READY;
        thread
    }

    pub fn dequeue_runnable(&mut self, object_table: &crate::object::ObjectTable) -> Option<(ObjectRef, u64)> {
        while self.head != self.tail {
            if let Some(thread_ref) = self.queue[self.head].take() {
                self.head = (self.head + 1) % MAX_READY;
                if let Some((_, phys)) = object_table.resolve(thread_ref) {
                    let tcb = crate::memory::phys_to_virt_ptr::<TCB>(phys);
                    let state = unsafe { core::ptr::read(&raw const (*tcb).state) };
                    if state == ThreadState::Ready || state == ThreadState::Running {
                        return Some((thread_ref, phys));
                    }
                }
            } else {
                self.head = (self.head + 1) % MAX_READY;
            }
        }
        None
    }
}

pub static mut SCHEDULER: Scheduler = Scheduler::new();

/// The cooperative yield primitive. 
/// Rotates the current thread to the back of the queue and switches to the next.
pub unsafe fn yield_thread() {
    let sched = &mut SCHEDULER;
    let object_table = &KERNEL_OBJECT_TABLE;
    
    // 1. State-Aware Rotation of outgoing thread: Only enqueue if not blocked/terminated!
    if let Some(prev_ref) = sched.current {
        if let Some((_, prev_phys)) = object_table.resolve(prev_ref) {
            let prev_tcb_ptr = crate::memory::phys_to_virt_ptr::<TCB>(prev_phys);
            let prev_state = core::ptr::read(&raw const (*prev_tcb_ptr).state);
            if prev_state == ThreadState::Ready || prev_state == ThreadState::Running {
                core::ptr::write(&raw mut (*prev_tcb_ptr).state, ThreadState::Ready);
                sched.enqueue(prev_ref);
            }
            core::ptr::write(&raw mut (*prev_tcb_ptr).frame_kind, crate::thread::FrameKind::Cooperative);
        }
    }

    // 2. Select next strictly runnable thread
    if let Some((next_ref, next_phys)) = sched.dequeue_runnable(object_table) {
        if let Some(prev_ref) = sched.current {
            if prev_ref == next_ref {
                // Same thread already selected, continue execution
                return;
            }
            let (_, prev_phys) = object_table.resolve(prev_ref).unwrap();
            let prev_tcb_ptr = crate::memory::phys_to_virt_ptr::<TCB>(prev_phys);

            sched.current = Some(next_ref);
            let next_tcb_ptr = crate::memory::phys_to_virt_ptr::<TCB>(next_phys);
            core::ptr::write(&raw mut (*next_tcb_ptr).state, ThreadState::Running);

            // Read all required incoming TCB fields before updating CR3
            let next_vspace = core::ptr::read(&raw const (*next_tcb_ptr).vspace);
            let next_stack_base = core::ptr::read(&raw const (*next_tcb_ptr).stack_base);
            let next_frame_kind = core::ptr::read(&raw const (*next_tcb_ptr).frame_kind);
            let next_rsp = core::ptr::read(&raw const (*next_tcb_ptr).rsp);

            // Synchronize CR3 to incoming thread's VSpace (keep BOOT_CR3 for kernel threads)
            let target_cr3 = match next_vspace {
                Some(vspace_ref) => {
                    let (_, pml4_phys) = object_table.resolve(vspace_ref).expect("Failed to resolve VSpace");
                    pml4_phys
                },
                None => crate::memory::BOOT_CR3,
            };

            if target_cr3 != 0 {
                let current_cr3: u64;
                core::arch::asm!("mov {}, cr3", out(reg) current_cr3, options(nomem, nostack));
                if (current_cr3 & !0xFFF) != (target_cr3 & !0xFFF) {
                    core::arch::asm!("mov cr3, {}", in(reg) target_cr3, options(nomem, nostack));
                }
            }

            // Synchronize TSS RSP0 and CpuLocal.kernel_rsp for user mode transitions
            if next_stack_base != 0 {
                let kstack_top = next_stack_base + crate::memory::KERNEL_STACK_SIZE;
                crate::gdt::set_tss_rsp0(kstack_top);
                crate::syscall::set_cpu_kernel_stack(kstack_top);
            }

            match next_frame_kind {
                crate::thread::FrameKind::Cooperative => {
                    crate::thread::switch_context(
                        &raw mut (*prev_tcb_ptr).rsp, 
                        next_rsp
                    );
                },
                crate::thread::FrameKind::Interrupt => {
                    crate::thread::switch_to_interrupt_frame(
                        &raw mut (*prev_tcb_ptr).rsp, 
                        next_rsp
                    );
                }
            }
        }
    }
}