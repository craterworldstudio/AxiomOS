use core::panic::PanicInfo;
use memory::BootInfo;
use capability::{CNode, Capability, Rights, CapHandle};
use object::{ObjectRef, ObjectKind, Untyped, ObjectTable};
use thread::switch_context;


#[allow(dead_code)]
static HELLO: &[u8] = b"AXIOM KERNEL ONLINE - by Soulfire";

static mut GENESIS_RSP: u64 = 0;
static mut THREAD_B_RSP: u64 = 0;
static mut PING_PONG_COUNTER: u64 = 0;

// ---- test harness (needed for the wait-queue verification block) ----
static mut VGA_ROW: isize = 2;

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
        let color  = if passed { 0x0A_u8  } else { 0x0C_u8  };
        for (i, &b) in status.iter().enumerate() {
            *vga.offset(VGA_ROW * 160 + i as isize * 2)     = b;
            *vga.offset(VGA_ROW * 160 + i as isize * 2 + 1) = color;
        }
        for (i, &b) in name.iter().enumerate() {
            *vga.offset(VGA_ROW * 160 + 18 + i as isize * 2)     = b;
            *vga.offset(VGA_ROW * 160 + 18 + i as isize * 2 + 1) = 0x0F;
        }
        VGA_ROW += 1;
        if !passed { core::arch::asm!("cli; hlt"); }
    }
}

pub fn print_header() {
    unsafe {
        let console = &raw mut crate::console::CONSOLE;
        if (*console).is_graphics {
            (*console).set_color(0x0000D2FF, 0x00101418); // Electric Cyan
            (*console).write_str("================================================================================\n");
            (*console).write_str(" AXIOM OS KERNEL SUBSTRATE (DEVELOPMENT MODE)\n");
            (*console).write_str("================================================================================\n\n");
        }

        let vga = 0xB8000 as *mut u8;
        for i in 0..2000 {
            *vga.add(i * 2) = b' ';
            *vga.add(i * 2 + 1) = 0x0F;
        }
        let header = b"AXIOM DEV SUBSTRATE";
        for (i, &b) in header.iter().enumerate() {
            *vga.offset(i as isize * 2) = b;
            *vga.offset(i as isize * 2 + 1) = 0x0B;
        }
        let divider = b"===================";
        for (i, &b) in divider.iter().enumerate() {
            *vga.offset(160 + i as isize * 2) = b;
            *vga.offset(160 + i as isize * 2 + 1) = 0x08;
        }
    }
}
// ---------------------------------------------------------------------

extern "C" fn dev_thread_b() {
    let mut step = 0;
    loop {
        unsafe {
            let console = &raw mut crate::console::CONSOLE;
            if (*console).is_graphics && step % 50 == 0 {
                (*console).set_color(0x002EE59D, 0x00101418);
                (*console).write_str("[ DEV ] Thread B scheduled and alive\n");
            }
            let vga = 0xB8000 as *mut u8;
            *vga.offset(20 * 160 + (step % 80) * 2) = b'B';
            *vga.offset(20 * 160 + (step % 80) * 2 + 1) = 0x0A; // Green
        }
        step += 1;
        for _ in 0..3_000_000 { unsafe { core::arch::asm!("nop"); } }
        unsafe { crate::scheduler::yield_thread(); }
    }
}

extern "C" fn dev_thread_c() {
    let mut step = 0;
    loop {
        unsafe {
            let console = &raw mut crate::console::CONSOLE;
            if (*console).is_graphics && step % 50 == 0 {
                (*console).set_color(0x00FFD166, 0x00101418);
                (*console).write_str("[ DEV ] Thread C scheduled and alive\n");
            }
            let vga = 0xB8000 as *mut u8;
            *vga.offset(21 * 160 + (step % 80) * 2) = b'C';
            *vga.offset(21 * 160 + (step % 80) * 2 + 1) = 0x0C; // Red
        }
        step += 1;
        for _ in 0..3_000_000 { unsafe { core::arch::asm!("nop"); } }
        unsafe { crate::scheduler::yield_thread(); }
    }
}

#[no_mangle]
pub extern "C" fn _start(boot_info: *const BootInfo) -> ! {
    unsafe {
        extern "C" {
            static KERNEL_START: u8;
            static KERNEL_END: u8;
            static BSS_START: u8;
            static BSS_END: u8;
        }
        let start = core::ptr::addr_of!(BSS_START) as *mut u8;
        let end = core::ptr::addr_of!(BSS_END) as *mut u8;
        let len = end as usize - start as usize;
        core::ptr::write_bytes(start, 0, len);
        crate::console::CONSOLE.init(boot_info);
        print_header();

        let vga_buffer = 0xB8000 as *mut u8;

        // 1. Validate the physical pointer and magic number
        let is_valid = unsafe { !boot_info.is_null() && (*boot_info).magic == 0xC0DEB007 };
        ktest(b"Bootloader Handshake", is_valid);

        if is_valid {
            gdt::init();
            interrupts::init();
            unsafe { memory::init_boot_cr3(); }
            let lapic_ok = unsafe { apic::init() };
            ktest(b"Architecture & Syscall Gateway", true);
            ktest(b"Local APIC & Calibrated Timer Armed", lapic_ok);

            let kernel_start_addr = core::ptr::addr_of!(KERNEL_START) as u64;
            let kernel_end_addr = core::ptr::addr_of!(KERNEL_END) as u64;

            let mut bitmap_alloc = unsafe { memory::BitmapAllocator::bootstrap(
                boot_info, kernel_start_addr, kernel_end_addr
            ) };
            ktest(b"Physical Frame Allocator", true);

            // Re-establish our kernel data plane handles
            let object_table = unsafe { &mut *core::ptr::addr_of_mut!(capability::KERNEL_OBJECT_TABLE) };
            let cnode = unsafe { &mut *core::ptr::addr_of_mut!(capability::GENESIS_CNODE) };
            ktest(b"Object Table & Root CNode", true);

            let base_addr1 = bitmap_alloc.allocate_contiguous(4).unwrap();
            let mut untyped1 = object::Untyped {
                physical_base: base_addr1, size: 4096 * 4, watermark: 0,
            };
            ktest(b"Untyped Authority Established", true);

            // 1. BOOTSTRAP GENESIS
            let (genesis_ref, _) = capability::retype(
                &mut untyped1, object::ObjectKind::TCB, object_table, cnode
            ).unwrap();
            
            let (_, gen_phys) = object_table.resolve(genesis_ref).unwrap();
            let genesis_tcb = unsafe { &mut *(gen_phys as *mut thread::TCB) };
            
            genesis_tcb.rsp = 0; 
            genesis_tcb.state = thread::ThreadState::Running;
            genesis_tcb.stack_base = 0x90000; 
            genesis_tcb.vspace = None;
            genesis_tcb.frame_kind = thread::FrameKind::Cooperative;
            
            unsafe { scheduler::SCHEDULER.current = Some(genesis_ref); }
            ktest(b"Genesis TCB Bound", true);

            // 2. CREATE THREAD B
            let stack_b = bitmap_alloc.allocate_frame().unwrap();
            let (ref_b, _) = capability::retype(&mut untyped1, object::ObjectKind::TCB, object_table, cnode).unwrap();
            let (_, phys_b) = object_table.resolve(ref_b).unwrap();
            let tcb_b = unsafe { &mut *(phys_b as *mut thread::TCB) };
            
            tcb_b.state = thread::ThreadState::Ready;
            tcb_b.stack_base = stack_b.start_address;
            tcb_b.vspace = None;
            tcb_b.frame_kind = thread::FrameKind::Cooperative;
            tcb_b.rsp = thread::TCB::forge_stack(stack_b.start_address + memory::PAGE_SIZE, dev_thread_b as *const () as u64);
            
            unsafe { scheduler::SCHEDULER.enqueue(ref_b); }
            ktest(b"Thread B Created & Scheduled", true);

            // 3. CREATE THREAD C
            let stack_c = bitmap_alloc.allocate_frame().unwrap();
            let (ref_c, _) = capability::retype(&mut untyped1, object::ObjectKind::TCB, object_table, cnode).unwrap();
            let (_, phys_c) = object_table.resolve(ref_c).unwrap();
            let tcb_c = unsafe { &mut *(phys_c as *mut thread::TCB) };
            
            tcb_c.state = thread::ThreadState::Ready;
            tcb_c.stack_base = stack_c.start_address;
            tcb_c.vspace = None;
            tcb_c.frame_kind = thread::FrameKind::Cooperative;
            tcb_c.rsp = thread::TCB::forge_stack(stack_c.start_address + memory::PAGE_SIZE, dev_thread_c as *const () as u64);

            unsafe { scheduler::SCHEDULER.enqueue(ref_c); }
            ktest(b"Thread C Created & Scheduled", true);

            // ========================================================================
            // BRICK #6A-B: STATEFUL RENDEZVOUS SEMANTICS
            // ========================================================================
            unsafe { VGA_ROW += 1; }

            // Allocate 4 dedicated test TCBs purely for synthetic state testing
            let (ref_t1, _) = capability::retype(&mut untyped1, object::ObjectKind::TCB, object_table, cnode).unwrap();
            let (ref_t2, _) = capability::retype(&mut untyped1, object::ObjectKind::TCB, object_table, cnode).unwrap();
            let (ref_t3, _) = capability::retype(&mut untyped1, object::ObjectKind::TCB, object_table, cnode).unwrap();
            let (ref_t4, _) = capability::retype(&mut untyped1, object::ObjectKind::TCB, object_table, cnode).unwrap();
            
            let mut ep = object::Endpoint::new();

            let get_state = |tref: object::ObjectRef| -> thread::ThreadState {
                let (_, phys) = object_table.resolve(tref).unwrap();
                unsafe { core::ptr::read(&raw const (*(phys as *mut thread::TCB)).state) }
            };
            let get_payload = |tref: object::ObjectRef| -> u64 {
                let (_, phys) = object_table.resolve(tref).unwrap();
                unsafe { core::ptr::read(&raw const (*(phys as *mut thread::TCB)).ipc_payload) }
            };
            let set_ready = |tref: object::ObjectRef| {
                let (_, phys) = object_table.resolve(tref).unwrap();
                unsafe { core::ptr::write(&raw mut (*(phys as *mut thread::TCB)).state, thread::ThreadState::Ready); }
            };

            // Test A: Receiver calls RECEIVE (no sender)
            let recv_a = ep.endpoint_receive(ref_t1, object_table);
            ktest(b"Rendezvous A: Receiver blocks", recv_a.is_none() && get_state(ref_t1) == thread::ThreadState::BlockedReceive);

            // Test B: Sender calls SEND (wakes receiver A)
            let send_b = ep.endpoint_send(ref_t2, 0x1111, object_table);
            ktest(b"Rendezvous B: Sender wakes A, payload transfers", 
                send_b == Some(ref_t1) && get_state(ref_t1) == thread::ThreadState::Ready && get_payload(ref_t1) == 0x1111);

            // Test C: Sender calls SEND (no receiver)
            let send_c = ep.endpoint_send(ref_t3, 0x2222, object_table);
            ktest(b"Rendezvous C: Sender blocks, holds payload", 
                send_c.is_none() && get_state(ref_t3) == thread::ThreadState::BlockedSend && get_payload(ref_t3) == 0x2222);

            // Test D: Receiver calls RECEIVE (wakes sender C)
            let recv_d = ep.endpoint_receive(ref_t4, object_table);
            ktest(b"Rendezvous D: Receiver wakes C, pulls payload", 
                recv_d == Some(ref_t3) && get_state(ref_t3) == thread::ThreadState::Ready && get_payload(ref_t4) == 0x2222);

            // Test E: FIFO Ordering (A and B block receive, C and D send)
            ep.endpoint_receive(ref_t1, object_table);
            ep.endpoint_receive(ref_t2, object_table);
            
            ep.endpoint_send(ref_t3, 0xC0DE, object_table);
            ep.endpoint_send(ref_t4, 0xCAFE, object_table);
            
            ktest(b"Rendezvous E: Strict FIFO transfer order", 
                get_payload(ref_t1) == 0xC0DE && get_payload(ref_t2) == 0xCAFE);

            // ========================================================================
            // BRICK #6B: RUNNABLE SCHEDULER STATE MACHINE
            // ========================================================================
            unsafe { VGA_ROW += 1; }

            // Test 1: Deterministic Enqueue/Dequeue
            unsafe {
                scheduler::SCHEDULER = scheduler::Scheduler::new();
                set_ready(ref_t1); set_ready(ref_t2); set_ready(ref_t3);

                scheduler::SCHEDULER.enqueue(ref_t1);
                scheduler::SCHEDULER.enqueue(ref_t2);
                scheduler::SCHEDULER.enqueue(ref_t3);
                
                let dq1 = scheduler::SCHEDULER.dequeue();
                let dq2 = scheduler::SCHEDULER.dequeue();
                let dq3 = scheduler::SCHEDULER.dequeue();
                let dq4 = scheduler::SCHEDULER.dequeue();
                
                ktest(b"Sched 1: Pure FIFO Dequeue (A, B, C, None)", 
                    dq1 == Some(ref_t1) && dq2 == Some(ref_t2) && dq3 == Some(ref_t3) && dq4 == None);
            }

            // Test 2: Blocking removes thread from run queue
            unsafe {
                scheduler::SCHEDULER = scheduler::Scheduler::new();
                let mut ep2 = object::Endpoint::new();
                set_ready(ref_t1); set_ready(ref_t2);

                scheduler::SCHEDULER.enqueue(ref_t2);
                scheduler::SCHEDULER.current = Some(ref_t1);

                // A executes RECEIVE and blocks.
                let woke_peer = ep2.endpoint_receive(ref_t1, object_table);
                if let Some(peer) = woke_peer { scheduler::SCHEDULER.enqueue(peer); }
                
                scheduler::SCHEDULER.current = scheduler::SCHEDULER.dequeue();
                
                ktest(b"Sched 2: Blocked thread yields CPU to B", 
                    scheduler::SCHEDULER.current == Some(ref_t2) && get_state(ref_t1) == thread::ThreadState::BlockedReceive);
            }

            // Test 3: Rendezvous enqueues woken thread
            unsafe {
                scheduler::SCHEDULER = scheduler::Scheduler::new();
                let mut ep3 = object::Endpoint::new();
                set_ready(ref_t1); set_ready(ref_t2);

                // Force A to block first
                ep3.endpoint_receive(ref_t1, object_table);
                scheduler::SCHEDULER.current = Some(ref_t2);

                // B executes SEND, waking A.
                let woke_peer = ep3.endpoint_send(ref_t2, 0x7777, object_table);
                if let Some(peer) = woke_peer { scheduler::SCHEDULER.enqueue(peer); }
                
                // B stays in rotation
                scheduler::SCHEDULER.enqueue(ref_t2);
                scheduler::SCHEDULER.current = scheduler::SCHEDULER.dequeue();
                
                ktest(b"Sched 3: Rendezvous wakes A and schedules it", 
                    scheduler::SCHEDULER.current == Some(ref_t1) && get_payload(ref_t1) == 0x7777);
            }

    unsafe { VGA_ROW += 1; }

    // ========================================================================
    // BRICK #6C: LIVE CONTEXT SWITCH VERIFICATION
    // ========================================================================
    unsafe {
        // 1. Sterilize the scheduler from previous synthetic tests
        scheduler::SCHEDULER = scheduler::Scheduler::new();
        
        // 2. Re-forge pristine stacks for B and C
        let (_, phys_b) = object_table.resolve(ref_b).unwrap();
        let tcb_b = phys_b as *mut thread::TCB;
        core::ptr::write(&raw mut (*tcb_b).state, thread::ThreadState::Ready);
        core::ptr::write(&raw mut (*tcb_b).vspace, None);
        core::ptr::write(&raw mut (*tcb_b).frame_kind, thread::FrameKind::Cooperative);
        let rsp_b = thread::TCB::forge_stack(
            core::ptr::read(&raw const (*tcb_b).stack_base) + memory::PAGE_SIZE, 
            dev_thread_b as *const () as u64
        );
        core::ptr::write(&raw mut (*tcb_b).rsp, rsp_b);

        let (_, phys_c) = object_table.resolve(ref_c).unwrap();
        let tcb_c = phys_c as *mut thread::TCB;
        core::ptr::write(&raw mut (*tcb_c).state, thread::ThreadState::Ready);
        core::ptr::write(&raw mut (*tcb_c).vspace, None);
        core::ptr::write(&raw mut (*tcb_c).frame_kind, thread::FrameKind::Cooperative);
        let rsp_c = thread::TCB::forge_stack(
            core::ptr::read(&raw const (*tcb_c).stack_base) + memory::PAGE_SIZE, 
            dev_thread_c as *const () as u64
        );
        core::ptr::write(&raw mut (*tcb_c).rsp, rsp_c);

        // 3. Enqueue B and C
        scheduler::SCHEDULER.enqueue(ref_b);
        scheduler::SCHEDULER.enqueue(ref_c);
        
        // 4. Genesis becomes a normal scheduled thread!
        let genesis_msg = b"Genesis yielding to live threads...";
        for (i, &b) in genesis_msg.iter().enumerate() {
            let vga = 0xB8000 as *mut u8;
            *vga.offset(VGA_ROW * 160 + i as isize * 2) = b;
            *vga.offset(VGA_ROW * 160 + i as isize * 2 + 1) = 0x0E;
        }
        let console = &raw mut crate::console::CONSOLE;
        if (*console).is_graphics {
            (*console).set_color(0x00FFD166, 0x00101418); // Gold Yellow
            (*console).write_str("\nGenesis yielding to live threads...\n\n");
        }

        loop {
            scheduler::yield_thread();
        }
    }
    // ========================================================================



        //unsafe { core::arch::asm!("int3"); } // Manually trigger a CPU Breakpoint exception
        //unsafe { core::arch::asm!("ud2"); } // Manually Trigger a CPU Kernal Panic
        }
    }

    loop {
        unsafe { core::arch::asm!("hlt"); }
    }
}

/// This function is called on kernel panic.
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    let vga = 0xB8000 as *mut u8;
    let panic_msg = b" FATAL RUST PANIC: UNWRAP FAILED ";
    
    // Print a loud Red warning to the top left of the screen
    for (i, &byte) in panic_msg.iter().enumerate() {
        unsafe {
            *vga.add(i * 2) = byte;
            *vga.add(i * 2 + 1) = 0x4F; // White text on Red background
        }
    }
    
    loop {
        unsafe { core::arch::asm!("cli; hlt"); }
    }
}

pub fn print_hex_64(val: u64, offset: isize, vga: *mut u8) {
        let hex_chars = b"0123456789ABCDEF";
        for i in 0..16 {
            let nibble = (val >> (60 - i * 4)) & 0x0F;
            unsafe {
                *vga.offset(offset + i * 2) = hex_chars[nibble as usize];
                *vga.offset(offset + i * 2 + 1) = 0x0B; // Light Cyan text
            }
        }
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