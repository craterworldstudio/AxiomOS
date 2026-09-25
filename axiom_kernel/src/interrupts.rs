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
        // Wire Vector 3 (Breakpoint) to our assembly stub
        IDT[3].set_handler(breakpoint_stub as *const () as u64);
        // Wire Vector 8 (Double Fault)
        IDT[8].set_handler(double_fault_stub as *const () as u64);
        // Instruct the CPU to switch to IST Index 1 (0-indexed as 0 in our code)
        IDT[8].ist = crate::gdt::DOUBLE_FAULT_IST_INDEX as u8 + 1;

        // Calculate the table limit (size in bytes - 1)
        IDTR.limit = (core::mem::size_of::<[IdtEntry; 256]>() - 1) as u16;
        IDTR.base = (&raw const IDT) as *const IdtEntry as u64;

        // Tell the CPU where to find our table
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