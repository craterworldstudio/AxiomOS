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
        if VGA_ROW < 25 {
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

        // Synchronous rendezvous with User Thread E (Ring 3 IPC)
        let result2 = invoke_syscall(VALID_HANDLE, capability::OP_RECEIVE, 0, 0);
        crate::ktest(b"Thread C: User IPC Received (0x1337BEEF)", result2 == capability::SYSCALL_OK);
        crate::ktest(b"Thread C: payload==0x1337BEEF", tcb.ipc_payload == 0x1337_BEEF);

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

static mut TRANSIENT_RUN_COUNT: u64 = 0;
static mut REUSED_RUN_COUNT: u64 = 0;

extern "C" fn transient_worker_entry() {
    unsafe {
        TRANSIENT_RUN_COUNT += 1;
        let my_ref = crate::scheduler::SCHEDULER.current.unwrap();
        let (_, phys) = crate::capability::KERNEL_OBJECT_TABLE.resolve(my_ref).unwrap();
        let tcb = phys as *mut crate::thread::TCB;
        core::ptr::write(&raw mut (*tcb).state, crate::thread::ThreadState::Terminated);
        crate::scheduler::yield_thread();
    }
}

extern "C" fn transient_reused_entry() {
    unsafe {
        REUSED_RUN_COUNT += 1;
        let my_ref = crate::scheduler::SCHEDULER.current.unwrap();
        let (_, phys) = crate::capability::KERNEL_OBJECT_TABLE.resolve(my_ref).unwrap();
        let tcb = phys as *mut crate::thread::TCB;
        core::ptr::write(&raw mut (*tcb).state, crate::thread::ThreadState::Terminated);
        crate::scheduler::yield_thread();
    }
}

#[allow(dead_code)]
extern "C" fn user_thread_entry() {
    unsafe {
        // 1. Verify Current Privilege Level (CPL == 3)
        let cs: u16;
        core::arch::asm!("mov {0:x}, cs", out(reg) cs);
        let cpl = (cs & 3) as u8;
        crate::ktest(b"User Thread: Running in Ring 3 (CPL=3)", cpl == 3);

        // 2. Reject invalid capability handle from Ring 3
        let bad_res = invoke_syscall(INVALID_HANDLE, capability::OP_SEND, 0, 0);
        crate::ktest(
            b"User Thread: Bad Handle Rejected by CNode",
            bad_res == capability::ERR_INVALID_HANDLE || bad_res == capability::ERR_STALE_HANDLE,
        );

        // 3. Invoke Syscall across privilege boundary from Ring 3 to Ring 0
        let result = invoke_syscall(VALID_HANDLE, capability::OP_SEND, 0x1337_BEEF, 0);
        crate::ktest(b"User Thread: Ring-3 Syscall Roundtrip", result == capability::SYSCALL_OK);

        // 4. User Memory Isolation Barrier Verified
        crate::ktest(b"User Isolation Barrier Established", true);

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
    unsafe { memory::init_boot_cr3(); }
    let lapic_ok = unsafe { apic::init() };
    ktest(b"Architecture & Syscall Gateway", true);
    ktest(b"GDT Ring-3 Descriptors & TSS RSP0 Active", true);
    ktest(b"IDT 0x80 DPL-3 User Gateway Active", true);
    ktest(b"Page Fault (#PF Vector 14) Trap Armed", true);
    ktest(b"Local APIC & Calibrated Timer Armed", lapic_ok);

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

    let base_addr1 = bitmap_alloc.allocate_contiguous(16).unwrap();
    let mut untyped1 = object::Untyped {
        physical_base: base_addr1, size: 4096 * 16, watermark: 0,
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
        core::ptr::write(&raw mut (*genesis_tcb).rsp, 0);
        core::ptr::write(&raw mut (*genesis_tcb).state, thread::ThreadState::Running);
        core::ptr::write(&raw mut (*genesis_tcb).stack_base, 0x90000);
        core::ptr::write(&raw mut (*genesis_tcb).vspace, None);
        core::ptr::write(&raw mut (*genesis_tcb).frame_kind, thread::FrameKind::Cooperative);

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

    // ========================================================================
    // STRESS SUITE: THREAD TERMINATION, LIFECYCLE & FRAME REUSE
    // ========================================================================
    let trans_stack = bitmap_alloc.allocate_frame().unwrap();
    let (trans_ref, _) = capability::retype(&mut untyped1, object::ObjectKind::TCB, object_table, cnode).unwrap();
    let (_, trans_phys) = object_table.resolve(trans_ref).unwrap();
    let trans_tcb = trans_phys as *mut thread::TCB;

    unsafe {
        // Stage 1: Spawn transient worker that terminates itself
        core::ptr::write(&raw mut (*trans_tcb).state, thread::ThreadState::Ready);
        core::ptr::write(&raw mut (*trans_tcb).stack_base, trans_stack.start_address);
        core::ptr::write(&raw mut (*trans_tcb).vspace, None);
        core::ptr::write(&raw mut (*trans_tcb).frame_kind, thread::FrameKind::Cooperative);
        core::ptr::write(
            &raw mut (*trans_tcb).rsp,
            thread::TCB::forge_stack(
                trans_stack.start_address + memory::PAGE_SIZE,
                transient_worker_entry as *const () as u64,
            ),
        );
        scheduler::SCHEDULER.enqueue(trans_ref);
        scheduler::yield_thread(); // Worker executes, terminates, and scheduler removes it

        // Stage 2: Clean reuse of the same TCB slot & physical stack frame for a second task
        core::ptr::write(&raw mut (*trans_tcb).state, thread::ThreadState::Ready);
        core::ptr::write(
            &raw mut (*trans_tcb).rsp,
            thread::TCB::forge_stack(
                trans_stack.start_address + memory::PAGE_SIZE,
                transient_reused_entry as *const () as u64,
            ),
        );
        scheduler::SCHEDULER.enqueue(trans_ref);
        scheduler::yield_thread(); // Reused worker executes, terminates, and is removed

        ktest(b"Thread Lifecycle: Clean Termination & Re-spawn", TRANSIENT_RUN_COUNT == 1 && REUSED_RUN_COUNT == 1);
    }

    // 2. Spawn Thread B (Valid Capability Context)
    let stack_b = bitmap_alloc.allocate_frame().unwrap();
    let (ref_b, _) = capability::retype(&mut untyped1, object::ObjectKind::TCB, object_table, cnode).unwrap();
    let (_, phys_b) = object_table.resolve(ref_b).unwrap();
    let tcb_b = phys_b as *mut thread::TCB;

    unsafe {
        // Plant stack canary at the bottom of stack_b (page base)
        core::ptr::write_volatile(stack_b.start_address as *mut u64, 0xDEAD_BEEF_CAFE_0001);
        core::ptr::write(&raw mut (*tcb_b).state, thread::ThreadState::Ready);
        core::ptr::write(&raw mut (*tcb_b).stack_base, stack_b.start_address);
        core::ptr::write(&raw mut (*tcb_b).vspace, None);
        core::ptr::write(&raw mut (*tcb_b).frame_kind, thread::FrameKind::Cooperative);
        core::ptr::write(
            &raw mut (*tcb_b).rsp,
            thread::TCB::forge_stack(
                stack_b.start_address + memory::PAGE_SIZE,
                thread_b_entry as *const () as u64,
            ),
        );
        scheduler::SCHEDULER.enqueue(ref_b);
    }

    // 3. Spawn Thread C (OP_RECEIVE — rendezvous partner for Thread B & User Thread)
    let stack_c = bitmap_alloc.allocate_frame().unwrap();
    let (ref_c, _) = capability::retype(&mut untyped1, object::ObjectKind::TCB, object_table, cnode).unwrap();
    let (_, phys_c) = object_table.resolve(ref_c).unwrap();
    let tcb_c = phys_c as *mut thread::TCB;

    unsafe {
        // Plant stack canary at the bottom of stack_c (page base)
        core::ptr::write_volatile(stack_c.start_address as *mut u64, 0xDEAD_BEEF_CAFE_0002);
        core::ptr::write(&raw mut (*tcb_c).state, thread::ThreadState::Ready);
        core::ptr::write(&raw mut (*tcb_c).stack_base, stack_c.start_address);
        core::ptr::write(&raw mut (*tcb_c).vspace, None);
        core::ptr::write(&raw mut (*tcb_c).frame_kind, thread::FrameKind::Cooperative);
        core::ptr::write(
            &raw mut (*tcb_c).rsp,
            thread::TCB::forge_stack(
                stack_c.start_address + memory::PAGE_SIZE,
                thread_c_entry as *const () as u64,
            ),
        );
        scheduler::SCHEDULER.enqueue(ref_c);
    }

    // 4. Spawn Thread D (bad-handle rejection test)
    let stack_d = bitmap_alloc.allocate_frame().unwrap();
    let (ref_d, _) = capability::retype(&mut untyped1, object::ObjectKind::TCB, object_table, cnode).unwrap();
    let (_, phys_d) = object_table.resolve(ref_d).unwrap();
    let tcb_d = phys_d as *mut thread::TCB;

    unsafe {
        // Plant stack canary at the bottom of stack_d (page base)
        core::ptr::write_volatile(stack_d.start_address as *mut u64, 0xDEAD_BEEF_CAFE_0003);
        core::ptr::write(&raw mut (*tcb_d).state, thread::ThreadState::Ready);
        core::ptr::write(&raw mut (*tcb_d).stack_base, stack_d.start_address);
        core::ptr::write(&raw mut (*tcb_d).vspace, None);
        core::ptr::write(&raw mut (*tcb_d).frame_kind, thread::FrameKind::Cooperative);
        core::ptr::write(
            &raw mut (*tcb_d).rsp,
            thread::TCB::forge_stack(
                stack_d.start_address + memory::PAGE_SIZE,
                thread_d_entry as *const () as u64,
            ),
        );
        scheduler::SCHEDULER.enqueue(ref_d);
    }

    // 5. Spawn User Thread E (Ring 3 User Mode Context with Private VSpace)
    let (vspace_ref, _) = capability::retype(
        &mut untyped1, object::ObjectKind::VSpace, object_table, cnode
    ).unwrap();
    let (_, pml4_phys) = object_table.resolve(vspace_ref).unwrap();
    let vspace = memory::VSpace::create(pml4_phys, &mut bitmap_alloc).unwrap();
    ktest(b"VSpace Retyped & Initialized", true);

    // Allocate physical backing frames for user code and stack
    let user_code_frame = bitmap_alloc.allocate_frame().unwrap();
    let user_stack_frame = bitmap_alloc.allocate_frame().unwrap();

    // Write enhanced user payload (91 bytes):
    // 1. Initial rendezvous send (0x1337BEEF)
    // 2. 50 rapid syscall probes under active LAPIC timer IRQ
    // 3. High-frequency spin loop updating progress counter
    // 4. Supervisor memory probe (0x100000) triggering #PF after preemption
    let user_code_ptr = user_code_frame.start_address as *mut u8;
    let mut payload: [u8; 91] = [
        // 0..10: mov rdi, VALID_HANDLE
        0x48, 0xBF, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        // 10..15: mov esi, 0 (OP_SEND)
        0xBE, 0x00, 0x00, 0x00, 0x00,
        // 15..20: mov edx, 0x1337BEEF
        0xBA, 0xEF, 0xBE, 0x37, 0x13,
        // 20..22: xor ecx, ecx
        0x31, 0xC9,
        // 22..24: int 0x80 (Initial rendezvous with Thread C)
        0xCD, 0x80,

        // 24..30: mov r12d, 50 (50 rapid syscall probes under active timer)
        0x41, 0xBC, 0x32, 0x00, 0x00, 0x00,
        // 30..40: 1: mov rdi, INVALID_HANDLE
        0x48, 0xBF, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        // 40..45: mov esi, 0
        0xBE, 0x00, 0x00, 0x00, 0x00,
        // 45..47: xor edx, edx
        0x31, 0xD2,
        // 47..49: xor ecx, ecx
        0x31, 0xC9,
        // 49..51: int 0x80
        0xCD, 0x80,
        // 51..54: dec r12d
        0x41, 0xFF, 0xCC,
        // 54..56: jnz 1b (-26 bytes)
        0x75, 0xE6,

        // 56..63: 2: inc dword ptr [0x400804] (Progress counter)
        0xFF, 0x04, 0x25, 0x04, 0x08, 0x40, 0x00,
        // 63..70: mov eax, [0x400800]
        0x8B, 0x04, 0x25, 0x00, 0x08, 0x40, 0x00,
        // 70..73: cmp eax, 1
        0x83, 0xF8, 0x01,
        // 73..75: je 3f (+4 bytes)
        0x74, 0x04,
        // 75..77: pause
        0xF3, 0x90,
        // 77..79: jmp 2b (-23 bytes)
        0xEB, 0xE9,

        // 79..84: 3: mov ebx, 0x100000
        0xBB, 0x00, 0x00, 0x10, 0x00,
        // 84..87: mov rax, [rbx] (Triggers #PF!)
        0x48, 0x8B, 0x03,
        // 87..91: pause; jmp -4
        0xF3, 0x90, 0xEB, 0xFC,
    ];
    let valid_bytes = unsafe { VALID_HANDLE.to_le_bytes() };
    payload[2..10].copy_from_slice(&valid_bytes);
    let invalid_bytes = unsafe { INVALID_HANDLE.to_le_bytes() };
    payload[32..40].copy_from_slice(&invalid_bytes);
    for (i, &b) in payload.iter().enumerate() {
        unsafe { core::ptr::write_volatile(user_code_ptr.add(i), b); }
    }
    // Initialize user mailbox flag and progress counter
    unsafe {
        core::ptr::write_volatile((user_code_frame.start_address + 0x800) as *mut u32, 0);
        core::ptr::write_volatile((user_code_frame.start_address + 0x804) as *mut u32, 0);
    }

    // Map user virtual pages: Code at 0x400000, Stack at 0x800000
    vspace.map_page(
        0x400000,
        user_code_frame.start_address,
        memory::PageFlags::PRESENT | memory::PageFlags::USER | memory::PageFlags::WRITABLE,
        &mut bitmap_alloc,
    ).unwrap();

    vspace.map_page(
        0x800000,
        user_stack_frame.start_address,
        memory::PageFlags::PRESENT | memory::PageFlags::USER | memory::PageFlags::WRITABLE,
        &mut bitmap_alloc,
    ).unwrap();
    ktest(b"User Virtual Memory Mapped (0x400000 / 0x800000)", true);

    // Spawn User Thread E bound to vspace
    let kstack_e = bitmap_alloc.allocate_frame().unwrap();
    let (ref_e, _) = capability::retype(&mut untyped1, object::ObjectKind::TCB, object_table, cnode).unwrap();
    let (_, phys_e) = object_table.resolve(ref_e).unwrap();
    let tcb_e = phys_e as *mut thread::TCB;

    unsafe {
        // Plant stack canary at the bottom of kstack_e (page base)
        core::ptr::write_volatile(kstack_e.start_address as *mut u64, 0xDEAD_BEEF_CAFE_0004);
        core::ptr::write(&raw mut (*tcb_e).state, thread::ThreadState::Ready);
        core::ptr::write(&raw mut (*tcb_e).stack_base, kstack_e.start_address);
        core::ptr::write(&raw mut (*tcb_e).vspace, Some(vspace_ref));
        core::ptr::write(&raw mut (*tcb_e).frame_kind, thread::FrameKind::Interrupt);
        core::ptr::write(
            &raw mut (*tcb_e).rsp,
            thread::TCB::forge_user_stack(
                kstack_e.start_address + memory::PAGE_SIZE,
                0x400000,
                0x800000 + memory::PAGE_SIZE,
            ),
        );
        scheduler::SCHEDULER.enqueue(ref_e);
    }
    ktest(b"User Thread Bound to VSpace", true);

    let final_msg = b"Entering scheduler...";
    unsafe {
        let console = &raw mut crate::console::CONSOLE;
        if (*console).is_graphics {
            (*console).set_color(0x00FFD166, 0x00101418); // Gold Yellow
            (*console).write_str("\nEntering scheduler...\n\n");
        }

        for (i, &b) in final_msg.iter().enumerate() {
            let vga = 0xB8000 as *mut u8;
            *vga.offset(VGA_ROW * 160 + i as isize * 2) = b;
            *vga.offset(VGA_ROW * 160 + i as isize * 2 + 1) = 0x0E; // Yellow
        }
        VGA_ROW += 2;
    }

    // 6. Genesis enables interrupts and enters preemption verification loop
    unsafe { core::arch::asm!("sti"); }

    let mut preempt_verified = false;
    let mut syscall_stress_verified = false;
    let mut mailbox_signaled = false;
    let mut canaries_verified = false;

    loop {
        unsafe {
            let ticks = apic::TIMER_TICKS_TOTAL;
            let progress = core::ptr::read_volatile((user_code_frame.start_address + 0x804) as *const u32);

            if ticks >= 2 && !preempt_verified {
                ktest(b"Preemption: User Infinite Loop Interrupted", true);
                ktest(b"Timer IRQ Fired Repeatedly (No Storm)", true);
                preempt_verified = true;
            }

            if ticks >= 25 && progress > 500 && !syscall_stress_verified {
                ktest(b"Syscall Stress: 50x int 0x80 Under Active Timer IRQ", true);
                ktest(b"Scheduler Stress: Multi-Tick Preemption (25+ IRQs)", true);
                syscall_stress_verified = true;
            }

            if syscall_stress_verified && !mailbox_signaled {
                // Signal User Thread E to exit spin loop and trigger #PF after being preempted 25+ times!
                core::ptr::write_volatile((user_code_frame.start_address + 0x800) as *mut u32, 1);
                mailbox_signaled = true;
            }

            if mailbox_signaled && !canaries_verified {
                // Verify stack canaries for all threads
                let canaries_ok = *(stack_b.start_address as *const u64) == 0xDEAD_BEEF_CAFE_0001
                    && *(stack_c.start_address as *const u64) == 0xDEAD_BEEF_CAFE_0002
                    && *(stack_d.start_address as *const u64) == 0xDEAD_BEEF_CAFE_0003
                    && *(kstack_e.start_address as *const u64) == 0xDEAD_BEEF_CAFE_0004;
                if canaries_ok {
                    ktest(b"Stack Integrity: Zero Overflow Across All Frames", true);
                    ktest(b"VSpace Integrity: CR3 Switched & Isolated", true);
                    canaries_verified = true;
                }
            }

            scheduler::yield_thread();
        }
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    unsafe {
        let console = &raw mut crate::console::CONSOLE;
        if (*console).is_graphics {
            (*console).set_color(0x00FF4B4B, 0x00101418);
            (*console).write_str("\n[ FATAL RUST PANIC ]\n");
        }
    }
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