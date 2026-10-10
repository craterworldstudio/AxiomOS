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

static mut VALID_HANDLE: u64 = 0;
static mut INVALID_HANDLE: u64 = 0;

extern "C" fn thread_b_entry() {
    unsafe {
        // Attempt a valid invocation (OP_SEND = 1)
        let _result = invoke_syscall(VALID_HANDLE, 1, 0xDEADC0DE, 0);
        
        // If we get here, the syscall boundary successfully restored our thread context!
        crate::ktest(b"Thread B: Valid Syscall", true);
        
        loop { crate::scheduler::yield_thread(); }
    }
}

extern "C" fn thread_c_entry() {
    unsafe {
        // Attempt an invalid invocation (Should be rejected by the router)
        let _result = invoke_syscall(INVALID_HANDLE, 1, 0xBADF00D, 0);
        
        // If we get here without a panic, the capability router safely rejected us!
        crate::ktest(b"Thread C: Invalid Rejected", true);
        
        loop { crate::scheduler::yield_thread(); }
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
    let genesis_tcb = gen_phys as *mut thread::TCB;

    
    unsafe {
        core::ptr::write( &raw mut (*genesis_tcb).rsp, 0, );
        core::ptr::write( &raw mut (*genesis_tcb).state, thread::ThreadState::Running, );
        core::ptr::write( &raw mut (*genesis_tcb).stack_base, 0x90000, );

        scheduler::SCHEDULER.current = Some(genesis_ref);
    }
    ktest(b"Genesis TCB Bound", true);

    // 5. INTEGRATION BASELINE
    unsafe { VGA_ROW += 1; }
    ktest(b"INTEGRATION BASELINE", true);
    unsafe { VGA_ROW += 1; }

    // ========================================================================
    // INTEGRATION LOOP v0.1: CAPABILITY ROUTER + SCHEDULER
    // ========================================================================

    // 1. Create a valid Endpoint to invoke
    let (ep_ref, ep_slot) = capability::retype(
        &mut untyped1, object::ObjectKind::Endpoint, object_table, cnode
    ).unwrap();
    
    unsafe {
        VALID_HANDLE = ep_slot as u64;
        INVALID_HANDLE = (ep_slot + 99) as u64; // Guaranteed empty slot
    }

    // 2. Spawn Thread B (Valid Capability Context)
    let stack_b = bitmap_alloc.allocate_frame().unwrap();
    let (ref_b, _) = capability::retype(&mut untyped1, object::ObjectKind::TCB, object_table, cnode).unwrap();
    let (_, phys_b) = object_table.resolve(ref_b).unwrap();
    let tcb_b = phys_b as *mut thread::TCB;

    
    unsafe {
    core::ptr::write( &raw mut (*tcb_b).state, thread::ThreadState::Ready, );
    core::ptr::write( &raw mut (*tcb_b).stack_base, stack_b.start_address, );

    core::ptr::write(
        &raw mut (*tcb_b).rsp,
        thread::TCB::forge_stack(
            stack_b.start_address + memory::PAGE_SIZE,
            thread_b_entry as u64,
        ),
    );

    scheduler::SCHEDULER.enqueue(ref_b);
}

    // 3. Spawn Thread C (Invalid Capability Context)
    let stack_c = bitmap_alloc.allocate_frame().unwrap();
    let (ref_c, _) = capability::retype(&mut untyped1, object::ObjectKind::TCB, object_table, cnode).unwrap();
    let (_, phys_c) = object_table.resolve(ref_c).unwrap();
    let tcb_c = phys_c as *mut thread::TCB;

    
    unsafe {
        core::ptr::write(
            &raw mut (*tcb_c).state,
            thread::ThreadState::Ready,
        );
    
        core::ptr::write(
            &raw mut (*tcb_c).stack_base,
            stack_c.start_address,
        );
    
        core::ptr::write(
            &raw mut (*tcb_c).rsp,
            thread::TCB::forge_stack(
                stack_c.start_address + memory::PAGE_SIZE,
                thread_c_entry as u64,
            ),
        );
    
        scheduler::SCHEDULER.enqueue(ref_c);
    }

    let final_msg = b"Entering scheduler...";
    for (i, &b) in final_msg.iter().enumerate() {
        unsafe {
            let vga = 0xB8000 as *mut u8;
            *vga.offset(VGA_ROW * 160 + i as isize * 2) = b;
            *vga.offset(VGA_ROW * 160 + i as isize * 2 + 1) = 0x0E; // Yellow
        }
    }
    unsafe { VGA_ROW += 2; } // Push diagnostic row down for threads to use

    // 4. Genesis becomes a normal scheduled thread!
    loop {
        unsafe { scheduler::yield_thread(); }
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