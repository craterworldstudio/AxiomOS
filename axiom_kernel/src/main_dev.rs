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
// ---------------------------------------------------------------------

extern "C" fn thread_b_entry() {
    let vga = 0xB8000 as *mut u8;
    let mut col = 0; // Local state preserved on Thread B's stack!
    loop {
        unsafe {
            *vga.offset(160 * 14 + col * 2) = b'B';
            *vga.offset(160 * 14 + col * 2 + 1) = 0x09; 
            
            col = (col + 1) % 80; // Move right, wrap at edge
            
            // Increased delay so human eyes can see the context switch
            for _ in 0..5_000_000 { core::arch::asm!("nop"); }
            scheduler::yield_thread(); 
        }
    }
}

extern "C" fn thread_c_entry() {
    let vga = 0xB8000 as *mut u8;
    let mut col = 0; // Local state preserved on Thread C's stack!
    loop {
        unsafe {
            *vga.offset(160 * 15 + col * 2) = b'C';
            *vga.offset(160 * 15 + col * 2 + 1) = 0x0C; 
            
            col = (col + 1) % 80;
            
            for _ in 0..5_000_000 { core::arch::asm!("nop"); }
            scheduler::yield_thread(); 
        }
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
    

    let vga_buffer = 0xB8000 as *mut u8;

    // 1. Clear the entire screen to black
    for i in 0..2000 {
        unsafe {
            *vga_buffer.add(i * 2) = b' ';
            *vga_buffer.add(i * 2 + 1) = 0x0F;
        }
    }

    // 1. Validate the physical pointer and magic number
    let is_valid = unsafe { !boot_info.is_null() && (*boot_info).magic == 0xC0DEB007 };

    let message = if is_valid {
        b"AXIOM KERNEL ONLINE [BOOT INFO VERIFIED]"
    } else {
        b"AXIOM KERNEL ONLINE [BOOT INFO FAILED]  "
    };

    // 2. Print the status to the top of the VGA buffer
    let mut offset = 0;
    for &byte in message.iter() {
        *vga_buffer.add(offset) = byte;
        *vga_buffer.add(offset + 1) = if is_valid { 0x0A } else { 0x0C };
        offset += 2;
    }
    
    //print_hex_64(is_valid as u64, 160 * 12, vga_buffer);
    // 3. If valid, prove it by printing the memory map entry count
    if is_valid {
        gdt::init();
        interrupts::init();

        let kernel_start_addr = core::ptr::addr_of!(KERNEL_START) as u64;
        let kernel_end_addr = core::ptr::addr_of!(KERNEL_END) as u64;

        //boot_allocator = unsafe { memory::FrameAllocator::new(boot_info) };
        let mut bitmap_alloc = unsafe { memory::BitmapAllocator::bootstrap(
            boot_info, kernel_start_addr, kernel_end_addr
        ) };

        // ========================================================
        // MILESTONE (BRICK #5B): COOPERATIVE SCHEDULER
        // ========================================================

        // Re-establish our kernel data plane handles
        let object_table = unsafe { &mut *core::ptr::addr_of_mut!(capability::KERNEL_OBJECT_TABLE) };
        let cnode = unsafe { &mut *core::ptr::addr_of_mut!(capability::GENESIS_CNODE) };

        let base_addr1 = bitmap_alloc.allocate_contiguous(1).unwrap();
        let mut untyped1 = object::Untyped {
            physical_base: base_addr1, size: 4096, watermark: 0,
        };

        // 1. BOOTSTRAP GENESIS
        let (genesis_ref, _) = capability::retype(
            &mut untyped1, object::ObjectKind::TCB, object_table, cnode
        ).unwrap();
        
        let (_, gen_phys) = object_table.resolve(genesis_ref).unwrap();
        let genesis_tcb = unsafe { &mut *(gen_phys as *mut thread::TCB) };
        
        // Genesis's RSP is left uninitialized; it will be captured on the first yield.
        genesis_tcb.rsp = 0; 
        genesis_tcb.state = thread::ThreadState::Running;
        // Metadata only: marks the pre-existing bootloader stack region
        genesis_tcb.stack_base = 0x90000; 
        
        // Hand Genesis to the Scheduler
        unsafe { scheduler::SCHEDULER.current = Some(genesis_ref); }

        // 2. CREATE THREAD B
        let stack_b = bitmap_alloc.allocate_frame().unwrap();
        let (ref_b, _) = capability::retype(&mut untyped1, object::ObjectKind::TCB, object_table, cnode).unwrap();
        let (_, phys_b) = object_table.resolve(ref_b).unwrap();
        let tcb_b = unsafe { &mut *(phys_b as *mut thread::TCB) };
        
        tcb_b.state = thread::ThreadState::Ready;
        tcb_b.stack_base = stack_b.start_address;
        tcb_b.rsp = thread::TCB::forge_stack(stack_b.start_address + memory::PAGE_SIZE, thread_b_entry as u64);
        
        unsafe { scheduler::SCHEDULER.enqueue(ref_b); }

        // 3. CREATE THREAD C
        let stack_c = bitmap_alloc.allocate_frame().unwrap();
        let (ref_c, _) = capability::retype(&mut untyped1, object::ObjectKind::TCB, object_table, cnode).unwrap();
        let (_, phys_c) = object_table.resolve(ref_c).unwrap();
        let tcb_c = unsafe { &mut *(phys_c as *mut thread::TCB) };
        
        tcb_c.state = thread::ThreadState::Ready;
        tcb_c.stack_base = stack_c.start_address;
        tcb_c.rsp = thread::TCB::forge_stack(stack_c.start_address + memory::PAGE_SIZE, thread_c_entry as u64);

        unsafe { scheduler::SCHEDULER.enqueue(ref_c); }

    // ========================================================================
    // BRICK #6A-B: STATEFUL RENDEZVOUS SEMANTICS
    // ========================================================================
    unsafe { VGA_ROW += 1; }

    // Allocate 4 TCBs purely for state testing
    let (ref_a, _) = capability::retype(&mut untyped1, object::ObjectKind::TCB, object_table, cnode).unwrap();
    let (ref_b, _) = capability::retype(&mut untyped1, object::ObjectKind::TCB, object_table, cnode).unwrap();
    let (ref_c, _) = capability::retype(&mut untyped1, object::ObjectKind::TCB, object_table, cnode).unwrap();
    let (ref_d, _) = capability::retype(&mut untyped1, object::ObjectKind::TCB, object_table, cnode).unwrap();
    
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
    let recv_a = ep.endpoint_receive(ref_a, object_table);
    ktest(b"Rendezvous A: Receiver blocks", recv_a.is_none() && get_state(ref_a) == thread::ThreadState::BlockedReceive);

    // Test B: Sender calls SEND (wakes receiver A)
    let send_b = ep.endpoint_send(ref_b, 0x1111, object_table);
    ktest(b"Rendezvous B: Sender wakes A, payload transfers", 
        send_b == Some(ref_a) && get_state(ref_a) == thread::ThreadState::Ready && get_payload(ref_a) == 0x1111);

    // Test C: Sender calls SEND (no receiver)
    let send_c = ep.endpoint_send(ref_c, 0x2222, object_table);
    ktest(b"Rendezvous C: Sender blocks, holds payload", 
        send_c.is_none() && get_state(ref_c) == thread::ThreadState::BlockedSend && get_payload(ref_c) == 0x2222);

    // Test D: Receiver calls RECEIVE (wakes sender C)
    let recv_d = ep.endpoint_receive(ref_d, object_table);
    ktest(b"Rendezvous D: Receiver wakes C, pulls payload", 
        recv_d == Some(ref_c) && get_state(ref_c) == thread::ThreadState::Ready && get_payload(ref_d) == 0x2222);

    // Test E: FIFO Ordering (A and B block receive, C and D send)
    ep.endpoint_receive(ref_a, object_table);
    ep.endpoint_receive(ref_b, object_table);
    
    ep.endpoint_send(ref_c, 0xC0DE, object_table);
    ep.endpoint_send(ref_d, 0xCAFE, object_table);
    
    ktest(b"Rendezvous E: Strict FIFO transfer order", 
        get_payload(ref_a) == 0xC0DE && get_payload(ref_b) == 0xCAFE);

    // ========================================================================
    // BRICK #6B: RUNNABLE SCHEDULER STATE MACHINE
    // ========================================================================
    unsafe { VGA_ROW += 1; }

    // Test 1: Deterministic Enqueue/Dequeue
    unsafe {
        scheduler::SCHEDULER = scheduler::Scheduler::new();
        set_ready(ref_a); set_ready(ref_b); set_ready(ref_c);

        scheduler::SCHEDULER.enqueue(ref_a);
        scheduler::SCHEDULER.enqueue(ref_b);
        scheduler::SCHEDULER.enqueue(ref_c);
        
        let dq1 = scheduler::SCHEDULER.dequeue();
        let dq2 = scheduler::SCHEDULER.dequeue();
        let dq3 = scheduler::SCHEDULER.dequeue();
        let dq4 = scheduler::SCHEDULER.dequeue();
        
        ktest(b"Sched 1: Pure FIFO Dequeue (A, B, C, None)", 
            dq1 == Some(ref_a) && dq2 == Some(ref_b) && dq3 == Some(ref_c) && dq4 == None);
    }

    // Test 2: Blocking removes thread from run queue
    unsafe {
        scheduler::SCHEDULER = scheduler::Scheduler::new();
        let mut ep2 = object::Endpoint::new();
        set_ready(ref_a); set_ready(ref_b);

        scheduler::SCHEDULER.enqueue(ref_b);
        scheduler::SCHEDULER.current = Some(ref_a);

        // A executes RECEIVE and blocks.
        let woke_peer = ep2.endpoint_receive(ref_a, object_table);
        if let Some(peer) = woke_peer { scheduler::SCHEDULER.enqueue(peer); }
        
        scheduler::SCHEDULER.current = scheduler::SCHEDULER.dequeue();
        
        ktest(b"Sched 2: Blocked thread yields CPU to B", 
            scheduler::SCHEDULER.current == Some(ref_b) && get_state(ref_a) == thread::ThreadState::BlockedReceive);
    }

    // Test 3: Rendezvous enqueues woken thread
    unsafe {
        scheduler::SCHEDULER = scheduler::Scheduler::new();
        let mut ep3 = object::Endpoint::new();
        set_ready(ref_a); set_ready(ref_b);

        // Force A to block first
        ep3.endpoint_receive(ref_a, object_table);
        scheduler::SCHEDULER.current = Some(ref_b);

        // B executes SEND, waking A.
        let woke_peer = ep3.endpoint_send(ref_b, 0x7777, object_table);
        if let Some(peer) = woke_peer { scheduler::SCHEDULER.enqueue(peer); }
        
        // B stays in rotation
        scheduler::SCHEDULER.enqueue(ref_b);
        scheduler::SCHEDULER.current = scheduler::SCHEDULER.dequeue();
        
        ktest(b"Sched 3: Rendezvous wakes A and schedules it", 
            scheduler::SCHEDULER.current == Some(ref_a) && get_payload(ref_a) == 0x7777);
    }

    unsafe { VGA_ROW += 1; }
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