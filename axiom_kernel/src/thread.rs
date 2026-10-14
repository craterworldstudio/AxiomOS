use crate::object::ObjectRef;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreadState {
    Ready = 0,
    Running = 1,
    BlockedSend = 2,
    BlockedReceive = 3,
    Suspended = 4,
    Terminated = 5,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameKind {
    Cooperative = 0,
    Interrupt = 1,
}

#[repr(C)]
pub struct TCB {
    pub rsp: u64,
    pub state: ThreadState,
    pub stack_base: u64,
    
    // IPC State
    pub ipc_payload: u64, 
    pub next_in_queue: Option<ObjectRef>, 
    pub vspace: Option<ObjectRef>,
    pub frame_kind: FrameKind,
    pub cspace: Option<ObjectRef>,
}

impl TCB {
    /// Forges a pristine stack frame for a cooperative kernel thread.
    pub fn forge_stack(stack_top: u64, entry_point: u64) -> u64 {
        let mut rsp = stack_top - 8;

        // Stack grows DOWN. The last thing to be popped (RIP) must be pushed first!
        rsp -= 8;
        unsafe { *(rsp as *mut u64) = entry_point; }

        // Callee-saved registers for switch_context: rbp, rbx, r12, r13, r14, r15
        rsp -= 8; unsafe { *(rsp as *mut u64) = 0; } // rbp
        rsp -= 8; unsafe { *(rsp as *mut u64) = 0; } // rbx
        rsp -= 8; unsafe { *(rsp as *mut u64) = 0; } // r12
        rsp -= 8; unsafe { *(rsp as *mut u64) = 0; } // r13
        rsp -= 8; unsafe { *(rsp as *mut u64) = 0; } // r14
        rsp -= 8; unsafe { *(rsp as *mut u64) = 0; } // r15

        rsp
    }

    /// Forges a complete interrupt frame for transitioning to Ring 3 via iretq.
    /// Matches the exact register push order of timer_interrupt_stub.
    pub fn forge_user_stack(kernel_stack_top: u64, user_entry: u64, user_stack_top: u64) -> u64 {
        let mut rsp = kernel_stack_top;

        // 1. Hardware iretq frame for Ring 3:
        // [rsp + 32]: user SS (0x1B: User Data Selector, RPL=3)
        // [rsp + 24]: user RSP
        // [rsp + 16]: user RFLAGS (0x202: Interrupt flag enabled, reserved bit 1)
        // [rsp + 8]:  user CS (0x23: User Code Selector, RPL=3)
        // [rsp + 0]:  user RIP
        rsp -= 8; unsafe { *(rsp as *mut u64) = 0x1B; }
        rsp -= 8; unsafe { *(rsp as *mut u64) = user_stack_top; }
        rsp -= 8; unsafe { *(rsp as *mut u64) = 0x202; }
        rsp -= 8; unsafe { *(rsp as *mut u64) = 0x23; }
        rsp -= 8; unsafe { *(rsp as *mut u64) = user_entry; }

        // 2. Full 15 general-purpose registers (r15 down to rax)
        // Pushed in order: r15, r14, r13, r12, r11, r10, r9, r8, rbp, rdi, rsi, rdx, rcx, rbx, rax
        for _ in 0..15 {
            rsp -= 8;
            unsafe { *(rsp as *mut u64) = 0; }
        }

        rsp
    }
}

core::arch::global_asm!(
    ".global switch_context",
    "switch_context:",
    // Cooperative switch: save callee-saved registers, switch stack, restore
    "push rbp",
    "push rbx",
    "push r12",
    "push r13",
    "push r14",
    "push r15",
    "mov [rdi], rsp",
    "mov rsp, rsi",
    "pop r15",
    "pop r14",
    "pop r13",
    "pop r12",
    "pop rbx",
    "pop rbp",
    "ret",

    ".global switch_to_interrupt_frame",
    "switch_to_interrupt_frame:",
    // Saves cooperative context for prev, restores complete 15-reg interrupt frame for next, and iretq
    "push rbp",
    "push rbx",
    "push r12",
    "push r13",
    "push r14",
    "push r15",
    "mov [rdi], rsp",
    "mov rsp, rsi",
    "pop rax",
    "pop rbx",
    "pop rcx",
    "pop rdx",
    "pop rsi",
    "pop rdi",
    "pop rbp",
    "pop r8",
    "pop r9",
    "pop r10",
    "pop r11",
    "pop r12",
    "pop r13",
    "pop r14",
    "pop r15",
    "test qword ptr [rsp + 8], 3",
    "jz 1f",
    "push rax",
    "mov ax, 0x1B",
    "mov ds, ax",
    "mov es, ax",
    "pop rax",
    "1:",
    "iretq",

    ".global resume_interrupt_frame",
    "resume_interrupt_frame:",
    // Restores complete 15-reg interrupt frame and executes iretq
    "mov rsp, rdi",
    "pop rax",
    "pop rbx",
    "pop rcx",
    "pop rdx",
    "pop rsi",
    "pop rdi",
    "pop rbp",
    "pop r8",
    "pop r9",
    "pop r10",
    "pop r11",
    "pop r12",
    "pop r13",
    "pop r14",
    "pop r15",
    "test qword ptr [rsp + 8], 3",
    "jz 2f",
    "push rax",
    "mov ax, 0x1B",
    "mov ds, ax",
    "mov es, ax",
    "pop rax",
    "2:",
    "iretq",

    ".global resume_cooperative_from_interrupt",
    "resume_cooperative_from_interrupt:",
    // Restores cooperative callee-saved registers from an interrupt exit and returns
    "mov rsp, rdi",
    "pop r15",
    "pop r14",
    "pop r13",
    "pop r12",
    "pop rbx",
    "pop rbp",
    "sti",
    "ret"
);

extern "C" {
    pub fn switch_context(prev_rsp: *mut u64, next_rsp: u64);
    pub fn switch_to_interrupt_frame(prev_rsp: *mut u64, next_rsp: u64);
    pub fn resume_interrupt_frame(target_rsp: u64) -> !;
    pub fn resume_cooperative_from_interrupt(target_rsp: u64) -> !;
}