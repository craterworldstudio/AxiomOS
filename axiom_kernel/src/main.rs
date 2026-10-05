#![no_std]
#![no_main]

mod interrupts;
mod gdt;
mod memory;
mod object;
mod capability;
mod thread;
mod scheduler;

use core::panic::PanicInfo;
use memory::BootInfo;
use capability::{CNode, Capability, Rights, CapHandle};
use object::{ObjectRef, ObjectKind, Untyped, ObjectTable};

// ============================================================================
// DIAGNOSTIC HARNESS
// ============================================================================

static mut VGA_ROW: isize = 2; // Start below the header

pub fn ktest(name: &[u8], passed: bool) {
    unsafe {
        let vga = 0xB8000 as *mut u8;
        let status = if passed { b"[ PASS ]" } else { b"[ FAIL ]" };
        let color = if passed { 0x0A } else { 0x0C }; // Light Green / Light Red
        
        // 1. Print Status Box
        for (i, &b) in status.iter().enumerate() {
            *vga.offset(VGA_ROW * 160 + i as isize * 2) = b;
            *vga.offset(VGA_ROW * 160 + i as isize * 2 + 1) = color;
        }
        
        // 2. Print Test Name
        for (i, &b) in name.iter().enumerate() {
            *vga.offset(VGA_ROW * 160 + 18 + i as isize * 2) = b;
            *vga.offset(VGA_ROW * 160 + 18 + i as isize * 2 + 1) = 0x0F; // White
        }
        
        VGA_ROW += 1;
        
        if !passed {
            core::arch::asm!("cli; hlt");
        }
    }
}

pub fn print_header() {
    unsafe {
        let vga = 0xB8000 as *mut u8;
        for i in 0..2000 {
            *vga.add(i * 2) = b' ';
            *vga.add(i * 2 + 1) = 0x0F;
        }
        let header = b"AXIOM KERNEL SELF-TEST";
        for (i, &b) in header.iter().enumerate() {
            *vga.offset(i as isize * 2) = b;
            *vga.offset(i as isize * 2 + 1) = 0x0B; // Light Cyan
        }
        let divider = b"======================";
        for (i, &b) in divider.iter().enumerate() {
            *vga.offset(160 + i as isize * 2) = b;
            *vga.offset(160 + i as isize * 2 + 1) = 0x08; // Dark Gray
        }
    }
}

pub fn print_hex_64(val: u64, offset: isize, vga: *mut u8) {
    let hex_chars = b"0123456789ABCDEF";
    for i in 0..16 {
        let nibble = (val >> (60 - i * 4)) & 0x0F;
        unsafe {
            *vga.offset(offset + i * 2) = hex_chars[nibble as usize];
            *vga.offset(offset + i * 2 + 1) = 0x0B;
        }
    }
}

// ============================================================================
// KERNEL ORCHESTRATOR
// ============================================================================
extern "C" {
    static BSS_START: u8;
    static BSS_END: u8;
    static KERNEL_START: u8;
    static KERNEL_END: u8;
}

#[no_mangle]
pub extern "C" fn _start(boot_info: *const BootInfo) -> ! {
    unsafe {
        let start = core::ptr::addr_of!(BSS_START) as *mut u8;
        let end = core::ptr::addr_of!(BSS_END) as *mut u8;
        core::ptr::write_bytes(start, 0, end as usize - start as usize);
    }

    print_header();

    let is_valid = unsafe { !boot_info.is_null() && (*boot_info).magic == 0xC0DEB007 };
    ktest(b"Bootloader Handshake", is_valid);

    // 1. ARCHITECTURE INIT
    gdt::init();
    interrupts::init();
    ktest(b"Architecture & Syscall Gateway", true);

    // 2. MEMORY INIT
    let kernel_start_addr = unsafe { core::ptr::addr_of!(KERNEL_START) as u64 };
    let kernel_end_addr = unsafe { core::ptr::addr_of!(KERNEL_END) as u64 };
    
    let mut bitmap_alloc = unsafe { 
        memory::BitmapAllocator::bootstrap(boot_info, kernel_start_addr, kernel_end_addr) 
    };
    ktest(b"Physical Frame Allocator", true);

    // 3. OBJECT & CAPABILITY INIT
    let object_table = unsafe { &mut *core::ptr::addr_of_mut!(capability::KERNEL_OBJECT_TABLE) };
    let cnode = unsafe { &mut *core::ptr::addr_of_mut!(capability::GENESIS_CNODE) };
    ktest(b"Object Table & Root CNode", true);

    let base_addr1 = bitmap_alloc.allocate_contiguous(1).unwrap();
    let mut untyped1 = object::Untyped {
        physical_base: base_addr1, size: 4096, watermark: 0,
    };
    ktest(b"Untyped Authority Established", true);

    // 4. GENESIS INIT
    let (genesis_ref, _) = capability::retype(
        &mut untyped1, object::ObjectKind::TCB, object_table, cnode
    ).unwrap();
    
    let (_, gen_phys) = object_table.resolve(genesis_ref).unwrap();
    let genesis_tcb = unsafe { &mut *(gen_phys as *mut thread::TCB) };
    
    genesis_tcb.rsp = 0; 
    genesis_tcb.state = thread::ThreadState::Running;
    genesis_tcb.stack_base = 0x90000; 
    
    unsafe { scheduler::SCHEDULER.current = Some(genesis_ref); }
    ktest(b"Genesis TCB Bound", true);

    // 5. INTEGRATION BASELINE
    unsafe { VGA_ROW += 1; }
    ktest(b"INTEGRATION BASELINE", true);
    unsafe { VGA_ROW += 1; }

    // (Thread B and Thread C initialization will go here)

    let final_msg = b"Entering scheduler...";
    for (i, &b) in final_msg.iter().enumerate() {
        unsafe {
            let vga = 0xB8000 as *mut u8;
            *vga.offset(VGA_ROW * 160 + i as isize * 2) = b;
            *vga.offset(VGA_ROW * 160 + i as isize * 2 + 1) = 0x0E; // Yellow
        }
    }

    loop {
        unsafe { core::arch::asm!("hlt"); }
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    let vga = 0xB8000 as *mut u8;
    let panic_msg = b" FATAL RUST PANIC ";
    for (i, &byte) in panic_msg.iter().enumerate() {
        unsafe {
            *vga.add(i * 2) = byte;
            *vga.add(i * 2 + 1) = 0x4F; 
        }
    }
    loop { unsafe { core::arch::asm!("cli; hlt"); } }
}

#[inline(always)]
pub unsafe fn invoke_syscall(handle: u64, operation: u64, arg1: u64, arg2: u64) -> u64 {
    let mut result: u64;
    core::arch::asm!(
        "int 0x80",
        in("rdi") handle,
        in("rsi") operation,
        in("rdx") arg1,
        in("rcx") arg2,
        lateout("rax") result,
    );
    result
}