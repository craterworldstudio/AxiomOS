use crate::object::ObjectRef;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreadState {
    Ready = 0,
    Running = 1,
    BlockedSend = 2,
    BlockedReceive = 3,
}

#[repr(C)]
pub struct TCB {
    pub rsp: u64,
    pub state: ThreadState,
    pub stack_base: u64,
    
    // IPC State
    pub ipc_payload: u64, 
    pub next_in_queue: Option<ObjectRef>, 
}

impl TCB {
    /// Forges a pristine stack frame for a new execution context.
    pub fn forge_stack(stack_top: u64, entry_point: u64) -> u64 {
        let mut rsp = stack_top;

        // Stack grows DOWN. The last thing to be popped (RIP) must be pushed first!
        rsp -= 8;
        unsafe { *(rsp as *mut u64) = entry_point; }

        // Now push the callee-saved registers in the exact reverse order of the pops.
        // Pop order: r15, r14, r13, r12, rbx, rbp
        // Push order: rbp, rbx, r12, r13, r14, r15
        rsp -= 8; unsafe { *(rsp as *mut u64) = 0xBBBBBBBBBBBBBBBB; } // rbp
        rsp -= 8; unsafe { *(rsp as *mut u64) = 0xBBBBBBBBBBBBBBBB; } // rbx
        rsp -= 8; unsafe { *(rsp as *mut u64) = 0x1212121212121212; } // r12
        rsp -= 8; unsafe { *(rsp as *mut u64) = 0x1313131313131313; } // r13
        rsp -= 8; unsafe { *(rsp as *mut u64) = 0x1414141414141414; } // r14
        rsp -= 8; unsafe { *(rsp as *mut u64) = 0x1515151515151515; } // r15

        rsp
    }
}

core::arch::global_asm!(
    ".global switch_context",
    "switch_context:",
    // 1. Save current context
    "push rbp",
    "push rbx",
    "push r12",
    "push r13",
    "push r14",
    "push r15",

    // 2. Save current RSP to the pointer in RDI
    "mov [rdi], rsp",

    // 3. Load next RSP from RSI
    "mov rsp, rsi",

    // 4. Restore next context
    "pop r15",
    "pop r14",
    "pop r13",
    "pop r12",
    "pop rbx",
    "pop rbp",

    // 5. Jump to the next instruction
    "ret"
);

extern "C" {
    /// RDI = prev_rsp (*mut u64)
    /// RSI = next_rsp (u64)
    pub fn switch_context(prev_rsp: *mut u64, next_rsp: u64);
}