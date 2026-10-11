use core::panic::PanicInfo;
use memory::BootInfo;
use capability::CapHandle;


// ============================================================================
// DIAGNOSTIC HARNESS
// ============================================================================

static mut VGA_ROW: isize = 2; // Start below the header

pub fn ktest(name: &[u8], passed: bool) {
    unsafe {
        // 1. Render to high-resolution graphics console if active
        let console = &raw mut crate::console::CONSOLE;
        if (*console).is_graphics {
            if passed {
                (*console).set_color(0x002EE59D, 0x00101418); // Crisp Emerald Green
                (*console).write_str("[ PASS ] ");
            } else {
                (*console).set_color(0x00FF4B4B, 0x00101418); // Crimson Red
                (*console).write_str("[ FAIL ] ");
            }
            (*console).set_color(0x00E0E0E0, 0x00101418); // Crisp White
            (*console).write_bytes(name);
            (*console).write_str("\n");
        }

        // 2. Dual-mode fallback: also update legacy 80x25 VGA text buffer
        let vga = 0xB8000 as *mut u8;
        let status = if passed { b"[ PASS ]" } else { b"[ FAIL ]" };
        let color = if passed { 0x0A } else { 0x0C };
        
        for (i, &b) in status.iter().enumerate() {
            *vga.offset(VGA_ROW * 160 + i as isize * 2) = b;
            *vga.offset(VGA_ROW * 160 + i as isize * 2 + 1) = color;
        }
        
        for (i, &b) in name.iter().enumerate() {
            *vga.offset(VGA_ROW * 160 + 18 + i as isize * 2) = b;
            *vga.offset(VGA_ROW * 160 + 18 + i as isize * 2 + 1) = 0x0F;
        }
        
        VGA_ROW += 1;
        
        if !passed {
            core::arch::asm!("cli; hlt");
        }
    }
}

pub fn print_header() {
    unsafe {
        let console = &raw mut crate::console::CONSOLE;
        if (*console).is_graphics {
            (*console).set_color(0x0000D2FF, 0x00101418); // Electric Cyan
            (*console).write_str("================================================================================\n");
            (*console).write_str(" AXIOM OS KERNEL SUBSTRATE\n");
            (*console).write_str("================================================================================\n\n");
        }

        let vga = 0xB8000 as *mut u8;
        for i in 0..2000 {
            *vga.add(i * 2) = b' ';
            *vga.add(i * 2 + 1) = 0x0F;
        }
        let header = b"AXIOM KERNEL SELF-TEST";
        for (i, &b) in header.iter().enumerate() {
            *vga.offset(i as isize * 2) = b;
            *vga.offset(i as isize * 2 + 1) = 0x0B;
        }
        let divider = b"======================";
        for (i, &b) in divider.iter().enumerate() {
            *vga.offset(160 + i as isize * 2) = b;
            *vga.offset(160 + i as isize * 2 + 1) = 0x08;
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
        // Perform a synchronous SEND on the shared Endpoint.
        // If no receiver is waiting yet, the handler will block this thread and yield.
        // When Thread C calls OP_RECEIVE the rendezvous completes and we resume here.
        let result = invoke_syscall(VALID_HANDLE, capability::OP_SEND, 0xCAFE_BABE, 0);
        crate::ktest(b"Thread B: OP_SEND completed", result == capability::SYSCALL_OK);
        loop { crate::scheduler::yield_thread(); }
    }
}

extern "C" fn thread_c_entry() {
    unsafe {
        // Perform a synchronous RECEIVE on the shared Endpoint.
        // If Thread B is already blocked-send the rendezvous fires immediately.
        // Otherwise this thread blocks until B sends.
        let result = invoke_syscall(VALID_HANDLE, capability::OP_RECEIVE, 0, 0);
        crate::ktest(b"Thread C: OP_RECEIVE completed", result == capability::SYSCALL_OK);

        // Verify the payload was delivered into this thread's TCB ipc_payload field.
        let my_ref = crate::scheduler::SCHEDULER.current.unwrap();
        let (_, phys) = crate::capability::KERNEL_OBJECT_TABLE.resolve(my_ref).unwrap();
        let tcb = &*(phys as *const crate::thread::TCB);
        crate::ktest(b"Thread C: payload==0xCAFEBABE", tcb.ipc_payload == 0xCAFE_BABE);

        loop { crate::scheduler::yield_thread(); }
    }
}

extern "C" fn thread_d_entry() {
    unsafe {
        // Probe with a handle whose CNode slot is guaranteed to be empty.
        // The router must reject it cleanly and return without faulting.
        let result = invoke_syscall(INVALID_HANDLE, capability::OP_SEND, 0, 0);
        crate::ktest(
            b"Thread D: bad handle rejected",
            result == capability::ERR_INVALID_HANDLE
                || result == capability::ERR_STALE_HANDLE,
        );
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
        crate::console::CONSOLE.init(boot_info);
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

    // ========================================================================
    // BRICK #6A-A: INTRUSIVE WAIT QUEUE VERIFICATION
    // ========================================================================
    unsafe { VGA_ROW += 1; }

    // Setup: allocate 3 TCBs purely for data-structure testing (no execution)
    let (ref_a, _) = capability::retype(&mut untyped1, object::ObjectKind::TCB, object_table, cnode).unwrap();
    let (ref_b, _) = capability::retype(&mut untyped1, object::ObjectKind::TCB, object_table, cnode).unwrap();
    let (ref_c, _) = capability::retype(&mut untyped1, object::ObjectKind::TCB, object_table, cnode).unwrap();

    let mut test_queue = object::WaitQueue::empty();

    // Test 1: Empty queue pop returns None
    let pop_empty = test_queue.pop(object_table);
    ktest(b"Queue: Empty pop() -> None", pop_empty == None);

    // Test 2: Single push — head == tail == A, A.next == None
    test_queue.push(ref_a, object_table);
    let is_head_tail_a = test_queue.head == Some(ref_a) && test_queue.tail == Some(ref_a);
    let (_, phys_a) = object_table.resolve(ref_a).unwrap();
    let next_a = unsafe {
        core::ptr::read(core::ptr::addr_of_mut!((*(phys_a as *mut thread::TCB)).next_in_queue))
    };
    ktest(b"Queue: push head==tail==A, A.next==None", is_head_tail_a && next_a == None);

    // Test 3: FIFO order A -> B -> C -> None
    test_queue.push(ref_b, object_table);
    test_queue.push(ref_c, object_table);
    let p1 = test_queue.pop(object_table);
    let p2 = test_queue.pop(object_table);
    let p3 = test_queue.pop(object_table);
    let p4 = test_queue.pop(object_table);
    ktest(b"Queue: FIFO A->B->C->None",
        p1 == Some(ref_a) && p2 == Some(ref_b) && p3 == Some(ref_c) && p4 == None);

    // Test 4: Link cleanup — A.next is None after pop
    let next_a_after = unsafe {
        core::ptr::read(core::ptr::addr_of_mut!((*(phys_a as *mut thread::TCB)).next_in_queue))
    };
    ktest(b"Queue: Link cleanup after pop", next_a_after == None);

    // Test 5: Stable reuse — same ref can be pushed and popped twice
    test_queue.push(ref_a, object_table);
    let reuse1 = test_queue.pop(object_table);
    test_queue.push(ref_a, object_table);
    let reuse2 = test_queue.pop(object_table);
    ktest(b"Queue: Stable reuse", reuse1 == Some(ref_a) && reuse2 == Some(ref_a));

    // Test 6: Two independent queues (send/recv) do not interfere
    let mut ep_test = object::Endpoint::new();
    ep_test.send_queue.push(ref_a, object_table);
    ep_test.recv_queue.push(ref_b, object_table);
    let sq_pop = ep_test.send_queue.pop(object_table);
    let rq_pop = ep_test.recv_queue.pop(object_table);
    ktest(b"Queue: send/recv non-interference", sq_pop == Some(ref_a) && rq_pop == Some(ref_b));

    // Test 7: Stale ObjectRef generational safety
    object_table.destroy_object(ref_c);
    let stale_resolve = object_table.resolve(ref_c);
    ktest(b"Queue: Stale ref safely rejected", stale_resolve == None);

    unsafe { VGA_ROW += 1; }
    // ========================================================================

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
        // CNode slots start at generation 1 (see CapabilitySlot::empty()).
        // The handle must encode (index, generation) so the router accepts it.
        VALID_HANDLE   = CapHandle::new(ep_slot as u32, 1).0;
        INVALID_HANDLE = CapHandle::new((ep_slot + 99) as u32, 1).0; // guaranteed empty slot
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

    // 3. Spawn Thread C (OP_RECEIVE — rendezvous partner for Thread B)
    let stack_c = bitmap_alloc.allocate_frame().unwrap();
    let (ref_c, _) = capability::retype(&mut untyped1, object::ObjectKind::TCB, object_table, cnode).unwrap();
    let (_, phys_c) = object_table.resolve(ref_c).unwrap();
    let tcb_c = phys_c as *mut thread::TCB;

    unsafe {
        core::ptr::write(&raw mut (*tcb_c).state,      thread::ThreadState::Ready);
        core::ptr::write(&raw mut (*tcb_c).stack_base, stack_c.start_address);
        core::ptr::write(
            &raw mut (*tcb_c).rsp,
            thread::TCB::forge_stack(stack_c.start_address + memory::PAGE_SIZE, thread_c_entry as u64),
        );
        scheduler::SCHEDULER.enqueue(ref_c);
    }

    // 4. Spawn Thread D (bad-handle rejection test)
    let stack_d = bitmap_alloc.allocate_frame().unwrap();
    let (ref_d, _) = capability::retype(&mut untyped1, object::ObjectKind::TCB, object_table, cnode).unwrap();
    let (_, phys_d) = object_table.resolve(ref_d).unwrap();
    let tcb_d = phys_d as *mut thread::TCB;

    unsafe {
        core::ptr::write(&raw mut (*tcb_d).state,      thread::ThreadState::Ready);
        core::ptr::write(&raw mut (*tcb_d).stack_base, stack_d.start_address);
        core::ptr::write(
            &raw mut (*tcb_d).rsp,
            thread::TCB::forge_stack(stack_d.start_address + memory::PAGE_SIZE, thread_d_entry as u64),
        );
        scheduler::SCHEDULER.enqueue(ref_d);
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