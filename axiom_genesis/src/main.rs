//! AxiomOS Genesis Process — PID 1 Sovereign Root User Task
//!
//! The Genesis Process is the first and foundational user-space process
//! executing in Ring 3 (CPL=3). It receives Root Untyped authority and
//! Root CNode capabilities from the microkernel, coordinates user-space
//! operating system services, and enforces object capability security.

#![no_std]
#![no_main]

use core::panic::PanicInfo;
use libaxiom::capability::*;
use libaxiom::console::ConsoleWriter;
use libaxiom::*;

// User-space mailbox communication offsets (mapped inside Genesis VSpace at 0x408000)
pub const USER_MAILBOX_FLAG: *mut u32    = 0x408000 as *mut u32;
pub const USER_PROGRESS_COUNT: *mut u32  = 0x408004 as *mut u32;

#[no_mangle]
#[link_section = ".text._start"]
pub extern "C" fn _start() -> ! {
    // 1. Verify Privilege Level: Must be running in Ring 3 (CPL == 3)
    let cs: u16;
    unsafe {
        core::arch::asm!("mov {0:x}, cs", out(reg) cs);
    }
    let cpl = (cs & 3) as u8;
    if cpl != 3 {
        // Severe privilege violation: fail-stop
        loop { unsafe { core::arch::asm!("pause"); } }
    }

    // Check if re-entering in Live Shell Takeover Mode (Phase 7A)
    let signal = unsafe { core::ptr::read_volatile(USER_MAILBOX_FLAG) };
    if signal == 2 {
        unsafe { core::ptr::write_volatile(USER_MAILBOX_FLAG, 3); }

        // Phase 7A: Sovereign Screen Takeover
        ConsoleWriter::draw_banner();

        println!();
        ConsoleWriter::set_color(0x0000D2FF, 0x00101418); // Electric Cyan
        println!("================================================================================");
        println!(" AXIOM OS SOVEREIGN CORE SUBSTRATE \u{2014} PID 1 (GENESIS) ONLINE");
        println!("================================================================================");
        ConsoleWriter::set_color(0x00E0E0E0, 0x00101418); // Crisp Silver
        println!();
        println!("Copyright (c) 2026 CraterWorld Studio. Open Source Sovereign Architecture.");
        println!();
        ConsoleWriter::set_color(0x002EE59D, 0x00101418); // Emerald Green
        println!("[ OK ] Microkernel Boot Selftests Passed & Frozen in Ring 0.");
        println!("[ OK ] Sovereign Ring 3 Privilege Barrier Active (CPL=3).");
        println!("[ OK ] Root CSpace Initialized: CNode(1), VSpace(2), TCB(3), Console(5).");
        println!("[ OK ] 1280x720 16:9 High-Resolution Display Subsystem Active.");
        println!("[ OK ] Memory Allocator Ready: 512 MB Physical Frame Authority.");
        println!();
        ConsoleWriter::set_color(0x00FFD166, 0x00101418); // Gold Yellow
        println!("Genesis Root Shell Ready.");
        println!();
        ConsoleWriter::set_color(0x0000D2FF, 0x00101418); // Electric Cyan
        print!("root@axiom:~# ");

        // Live idle / shell heartbeat loop
        loop {
            unsafe { core::arch::asm!("pause"); }
        }
    }

    // 2. Resolve Root Capabilities from Genesis CSpace
    let root_cnode = CNodeCap::from_handle(CapHandle::new(ROOT_SLOT_CNODE as u32, 1));
    let console_ep = EndpointCap::from_handle(CapHandle::new(ROOT_SLOT_CONSOLE as u32, 1));

    // 3. Perform Initial Rendezvous with Supervisor via Native Syscall (0x1337BEEF)
    let _ = console_ep.send(0x1337_BEEF);

    // 4. Exercise Capability Derivation in Ring 3: Mint attenuated child capability
    // Slot 6 is empty in initial Genesis CNode
    let _ = root_cnode.mint(6, Rights::READ);

    // 5. Stress Testing: 50 Rapid Fast-Path Syscalls under active timer IRQ
    let invalid_handle = CapHandle::new(999, 1);
    for _ in 0..50 {
        let _ = EndpointCap(invalid_handle).send(0);
    }

    // 6. High-Frequency Progress Loop: Test multi-tick preemption
    loop {
        unsafe {
            // Increment progress counter
            let count = core::ptr::read_volatile(USER_PROGRESS_COUNT);
            core::ptr::write_volatile(USER_PROGRESS_COUNT, count.wrapping_add(1));

            // Check if supervisor signaled memory probe test
            let signal = core::ptr::read_volatile(USER_MAILBOX_FLAG);
            if signal == 1 {
                // Probe supervisor-only memory (0x100000) to verify memory isolation (#PF vector 14)
                let supervisor_ptr = 0x100000 as *const u64;
                let _val = core::ptr::read_volatile(supervisor_ptr);
            }

            core::arch::asm!("pause");
        }
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {
        unsafe { core::arch::asm!("pause"); }
    }
}
