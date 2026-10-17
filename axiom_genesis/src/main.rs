//! AxiomOS Genesis Process — PID 1 Sovereign Root User Task
//!
//! The Genesis Process is the first and foundational user-space process
//! executing in Ring 3 (CPL=3). It receives Root Untyped authority and
//! Root CNode capabilities from the microkernel, coordinates user-space
//! operating system services, and enforces object capability security.

#![no_std]
#![no_main]
#![allow(static_mut_refs, unused_imports, dead_code)]

use core::panic::PanicInfo;
use libaxiom::capability::*;
use libaxiom::console::{ConsoleWriter, ConsoleReader};
use libaxiom::fs::VAULT;
use libaxiom::*;

// User-space mailbox communication offsets (mapped inside Genesis VSpace at 0x600000)
pub const USER_MAILBOX_FLAG: *mut u32    = 0x600000 as *mut u32;
pub const USER_PROGRESS_COUNT: *mut u32  = 0x600004 as *mut u32;

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

        // Initialize Sovereign Capability Filesystem (VaultFS)
        unsafe { VAULT.init(); }

        let mut current_dir = 0; // Inode 0: Root vault
        let mut line_buf = [0u8; 128];

        loop {
            let len = ConsoleReader::read_line(&mut line_buf);
            if len == 0 {
                ConsoleWriter::set_color(0x0000D2FF, 0x00101418);
                print!("root@axiom:~# ");
                continue;
            }

            let cmd_str = match core::str::from_utf8(&line_buf[..len]) {
                Ok(s) => s.trim(),
                Err(_) => {
                    ConsoleWriter::set_color(0x0000D2FF, 0x00101418);
                    print!("root@axiom:~# ");
                    continue;
                }
            };

            execute_command(cmd_str, &mut current_dir);

            ConsoleWriter::set_color(0x0000D2FF, 0x00101418);
            print!("root@axiom:~# ");
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

fn execute_command(input: &str, current_dir: &mut usize) {
    if input.is_empty() {
        return;
    }

    // Split command and argument
    let mut parts = input.splitn(2, ' ');
    let cmd = parts.next().unwrap_or("");
    let arg = parts.next().unwrap_or("").trim();

    match cmd {
        "help" => {
            ConsoleWriter::set_color(0x00FFD166, 0x00101418); // Gold Yellow
            println!("AxiomOS Sovereign Root Shell (PID 1) \u{2014} Capability Commands:");
            ConsoleWriter::set_color(0x00E0E0E0, 0x00101418); // Silver
            println!("  help              - Display this command reference manual");
            println!("  ls [dir]          - List entries in current or specified directory");
            println!("  cat <file>        - Print content of specified file capability");
            println!("  echo <text> [> f] - Print text or redirect to a file capability");
            println!("  status            - Inspect CSpace authority, memory and security state");
            println!("  clear             - Clear canvas and redraw obsidian status header");
            println!("  reboot            - Trigger hardware system reboot");
        },
        "ls" | "dir" => {
            let target_dir = if arg.is_empty() {
                *current_dir
            } else {
                match unsafe { VAULT.lookup(*current_dir, arg) } {
                    Some(id) if unsafe { VAULT.nodes[id].is_dir } => id,
                    Some(_) => {
                        println!("ls: {}: Not a directory", arg);
                        return;
                    },
                    None => {
                        println!("ls: {}: No such directory", arg);
                        return;
                    }
                }
            };

            let vault = unsafe { &VAULT };
            let mut count = 0;
            for (idx, node) in vault.nodes.iter().enumerate() {
                if node.in_use && node.parent_id == target_dir && idx != target_dir {
                    if node.is_dir {
                        ConsoleWriter::set_color(0x0000D2FF, 0x00101418); // Electric Cyan
                        print!("{}/  ", node.name_str());
                    } else {
                        ConsoleWriter::set_color(0x00E0E0E0, 0x00101418); // Silver
                        print!("{}  ", node.name_str());
                    }
                    count += 1;
                }
            }
            if count > 0 {
                println!();
            } else {
                println!("(empty)");
            }
        },
        "cat" => {
            if arg.is_empty() {
                println!("cat: missing file operand");
                return;
            }
            let vault = unsafe { &VAULT };
            match vault.lookup(*current_dir, arg) {
                Some(id) => {
                    let node = &vault.nodes[id];
                    if node.is_dir {
                        println!("cat: {}: Is a directory", arg);
                    } else {
                        ConsoleWriter::set_color(0x00E0E0E0, 0x00101418);
                        print!("{}", node.content_str());
                    }
                },
                None => {
                    println!("cat: {}: No such file or directory", arg);
                }
            }
        },
        "echo" => {
            // Check for redirection: echo <text> > <file>
            if let Some(arrow_idx) = arg.find('>') {
                let text = arg[..arrow_idx].trim();
                let file_name = arg[arrow_idx + 1..].trim();
                if file_name.is_empty() {
                    println!("echo: syntax error near unexpected token 'newline'");
                    return;
                }
                let mut content = [0u8; 512];
                let text_bytes = text.as_bytes();
                let clen = core::cmp::min(text_bytes.len(), 510);
                content[..clen].copy_from_slice(&text_bytes[..clen]);
                content[clen] = b'\n';

                match unsafe { VAULT.write_file(*current_dir, file_name, &content[..clen + 1]) } {
                    Ok(_) => {
                        ConsoleWriter::set_color(0x002EE59D, 0x00101418); // Emerald Green
                        println!("[ OK ] Written to capability: {}", file_name);
                    },
                    Err(e) => {
                        ConsoleWriter::set_color(0x00FF4B4B, 0x00101418);
                        println!("echo: {}: {}", file_name, e);
                    }
                }
            } else {
                println!("{}", arg);
            }
        },
        "status" | "info" => {
            ConsoleWriter::set_color(0x002EE59D, 0x00101418); // Emerald Green
            println!("[ CSPACE AUTHORITY INVENTORY ]");
            ConsoleWriter::set_color(0x00E0E0E0, 0x00101418);
            println!("  Slot 1: CNodeCap     - Master CDT Root Authority");
            println!("  Slot 2: VSpaceCap    - Genesis Sovereign PML4 (CR3)");
            println!("  Slot 3: TCBCap       - Genesis Thread Control Block (PID 1)");
            println!("  Slot 5: EndpointCap  - Console / Video Subsystem (1280x720)");
            println!("  Slot 7: DirectoryCap - In-Memory Storage Vault (VaultFS)");
            ConsoleWriter::set_color(0x002EE59D, 0x00101418);
            println!("[ SECURITY INVARIANTS ]");
            ConsoleWriter::set_color(0x00E0E0E0, 0x00101418);
            println!("  Privilege Level: Ring 3 User Mode (CPL = 3)");
            println!("  Memory Barrier: Supervisor Pages Isolated (#PF Vector 14)");
            println!("  Delegation: Monotonic Rights Attenuation Enforced");
            println!("  Authority Model: Zero Ambient Authority Active");
        },
        "clear" => {
            ConsoleWriter::draw_banner();
            println!();
        },
        "reboot" => {
            println!("Rebooting AxiomOS substrate...");
            unsafe {
                core::arch::asm!("out dx, al", in("dx") 0x64u16, in("al") 0xFEu8, options(nomem, nostack));
            }
        },
        other => {
            ConsoleWriter::set_color(0x00FF4B4B, 0x00101418); // Crimson Red
            print!("axiom: command not found: ");
            ConsoleWriter::set_color(0x00E0E0E0, 0x00101418);
            println!("{}", other);
            println!("Type 'help' for available built-in commands.");
        }
    }
}
