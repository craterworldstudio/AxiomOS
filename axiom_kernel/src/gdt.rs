// ==========================================
// Global Descriptor Table & Task State Segment
// ==========================================
use core::arch::asm;

pub const KERNEL_CODE_SELECTOR: u16 = 0x08;
pub const KERNEL_DATA_SELECTOR: u16 = 0x10;
pub const TSS_SELECTOR: u16 = 0x18;

pub const DOUBLE_FAULT_IST_INDEX: usize = 0;

#[repr(C, packed)]
pub struct TaskStateSegment {
    reserved_1: u32,
    pub privilege_stack_table: [u64; 3],
    reserved_2: u64,
    pub interrupt_stack_table: [u64; 7],
    reserved_3: u64,
    reserved_4: u16,
    pub iomap_base: u16,
}

impl TaskStateSegment {
    pub const fn new() -> Self {
        Self {
            reserved_1: 0,
            privilege_stack_table: [0; 3],
            reserved_2: 0,
            interrupt_stack_table: [0; 7],
            reserved_3: 0,
            reserved_4: 0,
            iomap_base: 0,
        }
    }
}

// Allocate a 16 KiB emergency stack, strictly 16-byte aligned for x86-64
#[repr(align(16))]
struct DoubleFaultStack(#[allow(dead_code)] [u8; 16384]);

static mut DOUBLE_FAULT_STACK: DoubleFaultStack = DoubleFaultStack([0; 16384]);
pub static mut TSS: TaskStateSegment = TaskStateSegment::new();

#[repr(C, packed)]
pub struct GdtDescriptor {
    pub limit: u16,
    pub base: u64,
}

// 0: Null, 1: Code, 2: Data, 3 & 4: 16-byte TSS
static mut GDT: [u64; 5] = [0; 5];
static mut GDTR: GdtDescriptor = GdtDescriptor { limit: 0, base: 0 };

pub fn init() {
    unsafe {
        // 1. Map the top of the emergency stack into IST slot 1
        let stack_start = (&raw const DOUBLE_FAULT_STACK) as *const DoubleFaultStack as u64;
        let stack_end = stack_start + 16384;
        TSS.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX] = stack_end;

        // 2. Encode the GDT Entries
        GDT[0] = 0; // Null
        GDT[1] = 0x00209A0000000000; // 0x08: Code (Present, Ring 0, Exec/Read, 64-bit)
        GDT[2] = 0x0000920000000000; // 0x10: Data (Present, Ring 0, Read/Write)

        // 0x18: 16-byte TSS Descriptor
        let tss_base = (&raw const TSS) as *const TaskStateSegment as u64;
        let tss_limit = (core::mem::size_of::<TaskStateSegment>() - 1) as u64;

        let tss_low = (tss_limit & 0xFFFF)
            | ((tss_base & 0xFFFFFF) << 16)
            | (0b1001 << 40) // Type: 64-bit TSS (Available)
            | (1 << 47)      // Present
            | (((tss_limit >> 16) & 0x0F) << 48)
            | (((tss_base >> 24) & 0xFF) << 56);
        
        let tss_high = tss_base >> 32;

        GDT[3] = tss_low;
        GDT[4] = tss_high;

        // 3. Load the GDT
        GDTR.limit = (core::mem::size_of::<[u64; 5]>() - 1) as u16;
        GDTR.base = (&raw const GDT) as *const u64 as u64;
        
        asm!(
            "lgdt [{}]",
            in(reg) (&raw const GDTR),
            options(readonly, nostack, preserves_flags)
        );

        // 4. Reload Data Segments and flush Code Segment via far return
        asm!(
            "mov ds, {data:x}",
            "mov es, {data:x}",
            "mov fs, {data:x}",
            "mov gs, {data:x}",
            "mov ss, {data:x}",
            "push {code}",           // Push new CS selector
            "lea {tmp}, [2f + rip]", // Push instruction pointer to label 2 (avoids LLVM bug)
            "push {tmp}",
            "retfq",                 // Far return pops RIP, then CS
            "2:",
            data = in(reg) KERNEL_DATA_SELECTOR,
            code = in(reg) KERNEL_CODE_SELECTOR as u64,
            tmp = out(reg) _,        // Let the compiler dynamically pick the scratch register
        );

        // 5. Load Task Register (LTR) to inform CPU about the TSS
        asm!(
            "ltr {tss:x}",
            tss = in(reg) TSS_SELECTOR,
            options(nostack, preserves_flags)
        );
    }
}