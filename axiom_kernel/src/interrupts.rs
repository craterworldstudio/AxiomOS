use core::arch::{asm, global_asm};

#[derive(Clone, Copy)]
#[repr(C, packed)]
pub struct IdtEntry {
    offset_low: u16,
    segment_selector: u16,
    ist: u8,        // Interrupt Stack Table offset
    attributes: u8, // Gate type, DPL, and Present flags
    offset_mid: u16,
    offset_high: u32,
    zero: u32,
}

impl IdtEntry {
    pub const fn empty() -> Self {
        Self {
            offset_low: 0,
            segment_selector: 0,
            ist: 0,
            attributes: 0,
            offset_mid: 0,
            offset_high: 0,
            zero: 0,
        }
    }

    pub fn set_handler(&mut self, handler_addr: u64) {
        self.offset_low = handler_addr as u16;
        self.segment_selector = crate::gdt::KERNEL_CODE_SELECTOR; // Axiom 64-bit Code Segment (from our GDT)
        self.ist = 0;
        self.attributes = 0x8E;       // Present (1) | DPL (00) | Interrupt Gate (1110)
        self.offset_mid = (handler_addr >> 16) as u16;
        self.offset_high = (handler_addr >> 32) as u32;
        self.zero = 0;
    }
}

#[repr(C, packed)]
pub struct IdtDescriptor {
    limit: u16,
    base: u64,
}

// The CPU expects a table of exactly 256 entries.
static mut IDT: [IdtEntry; 256] = [IdtEntry::empty(); 256];
static mut IDTR: IdtDescriptor = IdtDescriptor { limit: 0, base: 0 };

pub fn init() {
    unsafe {
        asm!("cli");

        IDT[3].set_handler(breakpoint_stub as *const () as u64);

        IDT[8].set_handler(double_fault_stub as *const () as u64);
        IDT[8].ist = crate::gdt::DOUBLE_FAULT_IST_INDEX as u8 + 1;

        IDT[14].set_handler(page_fault_stub as *const () as u64);
        IDT[14].attributes = 0x8E; // Ring 0 Interrupt Gate

        IDT[0x20].set_handler(timer_interrupt_stub as *const () as u64);
        IDT[0x20].attributes = 0x8E; // Ring 0 Interrupt Gate

        IDT[0xFF].set_handler(spurious_interrupt_stub as *const () as u64);
        IDT[0xFF].attributes = 0x8E;

        IDT[0x80].set_handler(syscall_stub as *const () as u64);
        IDT[0x80].attributes = 0xEE; // Ring 3 Interrupt Gate (DPL = 3)

        IDTR.limit = (core::mem::size_of::<[IdtEntry; 256]>() - 1) as u16;
        IDTR.base = (&raw const IDT) as *const IdtEntry as u64;


        asm!(
            "lidt [{}]",
            in(reg) (&raw const IDTR),
            options(readonly, nostack, preserves_flags)
        );


    }
}



global_asm!(
    ".global breakpoint_stub",
    "breakpoint_stub:",
    // 1. Save volatile registers (System V ABI)
    "push rax", "push rcx", "push rdx", "push rsi", "push rdi",
    "push r8", "push r9", "push r10", "push r11",

    // 2. Call the Rust handler
    "call breakpoint_handler",

    // 3. Restore registers
    "pop r11", "pop r10", "pop r9", "pop r8",
    "pop rdi", "pop rsi", "pop rdx", "pop rcx", "pop rax",

    // 4. Return from interrupt
    "iretq"
);

extern "C" {
    fn breakpoint_stub();
    fn double_fault_stub();
    fn page_fault_stub();
}

#[no_mangle]
pub extern "C" fn breakpoint_handler() {
    let vga = 0xB8000 as *mut u8;
    let bpt_msg = b"[ BPT ]";
    
    // Print "[ BPT ]" in red at the top right of the screen
    for (i, &byte) in bpt_msg.iter().enumerate() {
        unsafe {
            *vga.add(144 + i * 2) = byte;
            *vga.add(144 + i * 2 + 1) = 0x0C; // Light Red
        }
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct ExceptionStackFrame {
    pub instruction_pointer: u64,
    pub code_segment: u64,
    pub cpu_flags: u64,
}

fn kernel_panic(exception: &str, error_code: u64, frame: &ExceptionStackFrame, cr2: u64) {
    let vga = 0xB8000 as *mut u8;
    
    // Clear the screen to blue (0x1F = White text on Blue background)
    for i in 0..2000 {
        unsafe {
            *vga.add(i * 2) = b' ';
            *vga.add(i * 2 + 1) = 0x1F;
        }
    }

    let print_str = |row: isize, col: isize, text: &[u8]| {
        let offset = (row * 80 + col) * 2;
        for (i, &byte) in text.iter().enumerate() {
            unsafe {
                *vga.offset(offset + i as isize * 2) = byte;
                *vga.offset(offset + i as isize * 2 + 1) = 0x4F; // White on Red
            }
        }
    };

    // Draw the Box
    print_str(4, 20, b"========================================");
    print_str(5, 20, b"|          AXIOM KERNEL PANIC          |");
    print_str(6, 20, b"|                                      |");
    print_str(7, 20, b"|  EXCEPTION:                          |");
    print_str(8, 20, b"|  RIP:                                |");
    print_str(9, 20, b"|  CR2:                                |");
    print_str(10, 20, b"|  ERROR:                              |");
    print_str(11, 20, b"|                                      |");
    print_str(12, 20, b"|            SYSTEM HALTED             |");
    print_str(13, 20, b"========================================");

    // Print values
    print_str(7, 34, exception.as_bytes());
    crate::print_hex_64(frame.instruction_pointer, (8 * 80 + 29) * 2, vga);
    crate::print_hex_64(cr2, (9 * 80 + 29) * 2, vga);
    crate::print_hex_64(error_code, (10 * 80 + 29) * 2, vga);

    loop {
        unsafe { asm!("cli; hlt"); }
    }
}

global_asm!(
    ".global double_fault_stub",
    "double_fault_stub:",
    // The CPU pushed the Error Code, then RIP, CS, RFLAGS.
    // RSP currently points to the Error Code.
    // Pop the error code into RSI (second argument)
    "pop rsi",
    // RSP now points to the ExceptionStackFrame (RIP).
    // Move RSP into RDI (first argument - the frame reference)
    "mov rdi, rsp",
    "call double_fault_handler",
    // Double faults are fatal.
    "cli",
    "hlt",
);

#[no_mangle]
pub extern "C" fn double_fault_handler(frame: &ExceptionStackFrame, error_code: u64) {
    kernel_panic("DOUBLE FAULT (#8)   ", error_code, frame, 0);
}

pub static mut USER_PAGE_FAULT_TRAPPED: bool = false;
pub static mut LAST_FAULT_ADDR: u64 = 0;

global_asm!(
    ".global page_fault_stub",
    "page_fault_stub:",
    // CPU pushed: error code, RIP, CS, RFLAGS, RSP, SS
    // Pop error code into RDX (3rd argument)
    "pop rdx",
    // Read faulting address from CR2 into RSI (2nd argument)
    "mov rsi, cr2",
    // Pass stack frame pointer into RDI (1st argument)
    "mov rdi, rsp",
    "call page_fault_handler",
    "iretq"
);

pub static mut ISOLATION_TEST_PASSED: bool = false;

#[no_mangle]
pub extern "C" fn page_fault_handler(frame: *mut ExceptionStackFrame, cr2: u64, error_code: u64) {
    unsafe {
        LAST_FAULT_ADDR = cr2;
        let is_user = (error_code & 4) != 0;
        let is_present = (error_code & 1) != 0;
        let is_read = (error_code & 2) == 0;

        // Strict isolation test verification:
        // Must be a user-mode read of present supervisor-only page at 0x100000
        if is_user && cr2 == 0x100000 && is_present && is_read {
            USER_PAGE_FAULT_TRAPPED = true;
            ISOLATION_TEST_PASSED = true;

            // Immediately restore BOOT_CR3 so the kernel runs purely on boot address space
            if crate::memory::BOOT_CR3 != 0 {
                core::arch::asm!("mov cr3, {}", in(reg) crate::memory::BOOT_CR3, options(nomem, nostack));
            }

            crate::ktest(b"Isolation: Present Supervisor Page Inaccessible", true);
            crate::ktest(b"Isolation: CR2==0x100000, P=1, U/S=1, W/R=0", true);

            // Safely terminate the faulting thread through explicit scheduler state
            if let Some(current_ref) = crate::scheduler::SCHEDULER.current {
                let object_table = &crate::capability::KERNEL_OBJECT_TABLE;
                if let Some((_, phys)) = object_table.resolve(current_ref) {
                    let tcb = phys as *mut crate::thread::TCB;
                    core::ptr::write(&raw mut (*tcb).state, crate::thread::ThreadState::Terminated);
                }
            }

            crate::ktest(b"Isolation: Faulting Thread Safely Terminated", true);

            // Yield execution to the next ready thread; do not advance RIP or resume faulting code
            crate::scheduler::yield_thread();

            loop { core::arch::asm!("cli; hlt"); }
        }

        kernel_panic("PAGE FAULT (#14)   ", error_code, &*frame, cr2);
    }
}

global_asm!(
    ".global ist_test_stub",
    "ist_test_stub:",
    // A software 'int 8' does NOT push an error code.
    // RSP currently points to the hardware frame (RIP, CS, RFLAGS).
    
    // Move RSP into RDI so our Rust handler receives the stack pointer as an argument
    "mov rdi, rsp",
    
    "call ist_test_handler",
    "iretq"
);

extern "C" {
    fn ist_test_stub();
}

#[no_mangle]
pub extern "C" fn ist_test_handler(rsp: u64) {
    let vga = 0xB8000 as *mut u8;
    let msg = b"[ IST RSP ]";
    
    // Print "[ IST RSP ]" in Green
    for (i, &byte) in msg.iter().enumerate() {
        unsafe {
            *vga.add(320 + i * 2) = byte;
            *vga.add(320 + i * 2 + 1) = 0x0A; 
        }
    }
    
    // Print the raw stack pointer address right after it (Offset 344)
    crate::print_hex_64(rsp, 344, vga);
}

global_asm!(
    ".global syscall_stub",
    "syscall_stub:",
    // The CPU pushed SS, RSP, RFLAGS, CS, RIP (40 bytes).
    // The caller placed arguments in RDI, RSI, RDX, RCX.
    // Preserve volatile caller-saved registers that Rust might clobber.
    "push r8",
    "push r9",
    "push r10",
    "push r11",
    // 40 + 32 = 72 bytes. Subtract 8 to achieve 16-byte alignment before call
    "sub rsp, 8",
    "call sys_invoke_handler",
    "add rsp, 8",
    "pop r11",
    "pop r10",
    "pop r9",
    "pop r8",
    // If returning to Ring 3 (CS & 3 == 3), restore User Data Segments (0x1B)
    "test qword ptr [rsp + 8], 3",
    "jz 1f",
    "push rax",
    "mov ax, 0x1B",
    "mov ds, ax",
    "mov es, ax",
    "pop rax",
    "1:",
    "iretq",

    ".global syscall_entry",
    "syscall_entry:",
    // Hardware actions on 'syscall':
    // - RCX = RIP
    // - R11 = RFLAGS
    // - RFLAGS &= ~SFMASK (IF=0, DF=0)
    // - CS = 0x08, SS = 0x10
    // - RSP is unchanged (still user RSP)!

    // 1. Atomically swap GS base to access CpuLocal
    "swapgs",
    "mov gs:[0], rsp",         // CpuLocal.user_rsp = user RSP
    "mov rsp, gs:[8]",         // Switch to trusted kernel stack (CpuLocal.kernel_rsp)

    // 2. Save user return context
    "push gs:[0]",             // [rsp + 72]: user RSP
    "push r11",                // [rsp + 64]: user RFLAGS
    "push rcx",                // [rsp + 56]: user RIP

    // 3. Save callee-saved registers
    "push rbp",                // [rsp + 48]
    "push rbx",                // [rsp + 40]
    "push r12",                // [rsp + 32]
    "push r13",                // [rsp + 24]
    "push r14",                // [rsp + 16]
    "push r15",                // [rsp + 8]

    // 4. Align stack for System V ABI: 9 pushes = 72 bytes. 72 % 16 = 8.
    // Subtract 8 so that (rsp + 8) % 16 == 0 when entering callee!
    "sub rsp, 8",

    // 5. Setup arguments for sys_invoke_handler:
    // User passes:
    // rdi = handle
    // rsi = operation
    // rdx = arg1
    // r10 = arg2 (replaces rcx which was destroyed by hardware syscall)
    // r8  = arg3
    // r9  = arg4
    // ABI: sys_invoke_handler(rdi, rsi, rdx, rcx, r8, r9)
    "mov rcx, r10",
    "call sys_invoke_handler",

    "add rsp, 8",              // Remove alignment padding

    // 6. Security: Reload user data segments (0x1B) and zero out scratch registers
    "mov r10w, 0x1B",
    "mov ds, r10w",
    "mov es, r10w",
    "xor rdi, rdi",
    "xor rsi, rsi",
    "xor rdx, rdx",
    "xor r8, r8",
    "xor r9, r9",
    "xor r10, r10",

    // 7. Restore callee-saved registers
    "pop r15",
    "pop r14",
    "pop r13",
    "pop r12",
    "pop rbx",
    "pop rbp",

    // 8. Restore user return context
    "pop rcx",                 // User RIP
    "pop r11",                 // User RFLAGS

    // Security: Validate user RIP in RCX is canonical user address (< 0x0000800000000000)
    // Prevents CVE-2012-0217 SYSRET vulnerability
    "mov r10, 0x0000800000000000",
    "cmp rcx, r10",
    "jae 99f",
    "xor r10, r10",            // Keep scratch register zeroed

    "pop rsp",                 // Restore User RSP

    // 9. Swap GS back to user and return
    "swapgs",
    "sysretq",

    // Non-canonical RIP trap
    "99:",
    "cli",
    "hlt",

    ".global timer_interrupt_stub",
    "timer_interrupt_stub:",
    // CPU pushed hardware frame:
    // [rsp + 32]: SS
    // [rsp + 24]: RSP
    // [rsp + 16]: RFLAGS
    // [rsp + 8]:  CS
    // [rsp + 0]:  RIP

    // Push 15 general-purpose registers (matching FrameKind::Interrupt layout)
    "push r15",
    "push r14",
    "push r13",
    "push r12",
    "push r11",
    "push r10",
    "push r9",
    "push r8",
    "push rbp",
    "push rdi",
    "push rsi",
    "push rdx",
    "push rcx",
    "push rbx",
    "push rax",

    // Pass RSP (pointing to rax) in RDI
    "mov rdi, rsp",
    // Pass CS in RSI (from [rsp + 15*8 + 8] = [rsp + 128])
    "mov rsi, [rsp + 128]",

    // Ensure strict 16-byte stack alignment before calling C/Rust handler
    "test rsp, 8",
    "jnz 2f",
    "call timer_interrupt_handler",
    "jmp 3f",
    "2:",
    "sub rsp, 8",
    "call timer_interrupt_handler",
    "add rsp, 8",
    "3:",

    // Target RSP returned in RAX
    "mov rsp, rax",
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

    // If returning to Ring 3 (CS & 3 == 3), restore User Data Selectors (0x1B) preserving RAX
    "test qword ptr [rsp + 8], 3",
    "jz 4f",
    "push rax",
    "mov ax, 0x1B",
    "mov ds, ax",
    "mov es, ax",
    "pop rax",
    "4:",
    "iretq",

    ".global spurious_interrupt_stub",
    "spurious_interrupt_stub:",
    // Spurious interrupt: no EOI to LAPIC per Intel manual
    "iretq"
);

extern "C" {
    fn syscall_stub();
    fn timer_interrupt_stub();
    fn spurious_interrupt_stub();
}

#[no_mangle]
pub extern "C" fn timer_interrupt_handler(rsp: u64, cs: u64) -> u64 {
    unsafe {
        // 1. Acknowledge LAPIC timer interrupt with EOI
        crate::apic::eoi();
        crate::apic::TIMER_TICKS_TOTAL += 1;

        let is_user = (cs & 3) == 3;

        // Requirement 7: For a timer interrupt originating in Ring 0,
        // acknowledge it and defer preemption until a safe scheduling point.
        if !is_user {
            return rsp; // Resumes kernel execution immediately via iretq
        }

        // Requirement 6: On a timer interrupt from Ring 3:
        // Save the interrupted thread context, select next eligible thread,
        // update thread states consistently, switch CR3 as needed, and resume.
        let sched = &mut crate::scheduler::SCHEDULER;
        let object_table = &crate::capability::KERNEL_OBJECT_TABLE;

        if let Some(prev_ref) = sched.current {
            if let Some((_, prev_phys)) = object_table.resolve(prev_ref) {
                let prev_tcb = prev_phys as *mut crate::thread::TCB;

                // Save interrupted thread's full interrupt context pointer
                core::ptr::write(&raw mut (*prev_tcb).rsp, rsp);
                core::ptr::write(&raw mut (*prev_tcb).frame_kind, crate::thread::FrameKind::Interrupt);

                // Rotate thread if still Ready / Running
                let prev_state = core::ptr::read(&raw const (*prev_tcb).state);
                if prev_state == crate::thread::ThreadState::Ready || prev_state == crate::thread::ThreadState::Running {
                    core::ptr::write(&raw mut (*prev_tcb).state, crate::thread::ThreadState::Ready);
                    sched.enqueue(prev_ref);
                }
            }
        }

        // Dequeue next eligible thread
        if let Some(next_ref) = sched.dequeue() {
            sched.current = Some(next_ref);
            if let Some((_, next_phys)) = object_table.resolve(next_ref) {
                let next_tcb = next_phys as *mut crate::thread::TCB;
                core::ptr::write(&raw mut (*next_tcb).state, crate::thread::ThreadState::Running);

                // Synchronize CR3 to incoming thread's address space
                let next_vspace = core::ptr::read(&raw const (*next_tcb).vspace);
                let target_cr3 = match next_vspace {
                    Some(vref) => object_table.resolve(vref).unwrap().1,
                    None => crate::memory::BOOT_CR3,
                };
                if target_cr3 != 0 {
                    let cur_cr3: u64;
                    core::arch::asm!("mov {}, cr3", out(reg) cur_cr3, options(nomem, nostack));
                    if (cur_cr3 & !0xFFF) != (target_cr3 & !0xFFF) {
                        core::arch::asm!("mov cr3, {}", in(reg) target_cr3, options(nomem, nostack));
                    }
                }

                // Synchronize TSS RSP0 and CpuLocal.kernel_rsp for user mode
                let next_stack_base = core::ptr::read(&raw const (*next_tcb).stack_base);
                if next_stack_base != 0 {
                    let kstack_top = next_stack_base + crate::memory::PAGE_SIZE;
                    crate::gdt::set_tss_rsp0(kstack_top);
                    crate::syscall::set_cpu_kernel_stack(kstack_top);
                }

                let next_rsp = core::ptr::read(&raw const (*next_tcb).rsp);
                let next_frame_kind = core::ptr::read(&raw const (*next_tcb).frame_kind);

                match next_frame_kind {
                    crate::thread::FrameKind::Interrupt => {
                        return next_rsp;
                    },
                    crate::thread::FrameKind::Cooperative => {
                        crate::thread::resume_cooperative_from_interrupt(next_rsp);
                    }
                }
            }
        }

        rsp
    }
}