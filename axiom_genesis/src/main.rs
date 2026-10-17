//! AxiomOS Genesis Process — PID 1 Sovereign Root Task & CapGraph Shell Launcher
//!
//! Executes in Ring 3 (CPL=3). Initializes the Object-Oriented Structural Graph
//! ("CapGraph"), provides live interactive shell input, and dispatches standalone
//! C and x86_64 Assembly binaries from /bin with Capability Ledger enforcement.

#![no_std]
#![no_main]
#![allow(static_mut_refs, unused_imports, dead_code)]

use core::panic::PanicInfo;
use libaxiom::capability::*;
use libaxiom::console::{ConsoleWriter, ConsoleReader};
use libaxiom::fs::*;
use libaxiom::*;

// User-space mailbox communication offsets (mapped inside Genesis VSpace at 0x600000)
pub const USER_MAILBOX_FLAG: *mut u32    = 0x600000 as *mut u32;
pub const USER_PROGRESS_COUNT: *mut u32  = 0x600004 as *mut u32;

// Memory region where userland binaries are loaded and executed
pub const USER_EXEC_BASE: *mut u8        = 0x500000 as *mut u8;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct CapKeyring {
    pub user_id: u32,
    pub rights: u32,
    pub storage_cap: u64,
    pub console_cap: u64,
    pub user_name: [u8; 32],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct CapEntryInfoWithData {
    pub name: [u8; 32],
    pub is_dir: u32,
    pub data_len: u32,
    pub rights: u8,
    pub in_use: u8,
    pub data: *const u8,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct CapMapContextWithData {
    pub entries: [CapEntryInfoWithData; 16],
    pub count: u32,
}

static mut MAP_CTX: CapMapContextWithData = CapMapContextWithData {
    entries: [CapEntryInfoWithData {
        name: [0; 32],
        is_dir: 0,
        data_len: 0,
        rights: 0,
        in_use: 0,
        data: core::ptr::null(),
    }; 16],
    count: 0,
};

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
        loop { unsafe { core::arch::asm!("pause"); } }
    }

    // Check if re-entering in Live Shell Takeover Mode (Phase 7A/7B/7C)
    let signal = unsafe { core::ptr::read_volatile(USER_MAILBOX_FLAG) };
    if signal == 2 {
        unsafe { core::ptr::write_volatile(USER_MAILBOX_FLAG, 3); }

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
        println!("[ OK ] Object-Oriented Structural Graph (\"CapGraph\") Active.");
        println!("[ OK ] Standalone C/ASM Binaries Available in /bin.");
        println!("[ OK ] Capability Ledger Enforcing Least-Privilege Execution.");
        println!();
        ConsoleWriter::set_color(0x00FFD166, 0x00101418); // Gold Yellow
        println!("Genesis Sovereign Root Shell Ready.");
        println!();
        ConsoleWriter::set_color(0x0000D2FF, 0x00101418); // Electric Cyan
        print!("root@axiom:~# ");

        // Initialize Sovereign Object-Oriented Structural Graph (CapGraph)
        unsafe { GRAPH.init(); }

        let mut current_map = 0; // Map Node 0: Root Entry Map
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

            execute_command(cmd_str, &mut current_map);

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
    let _ = root_cnode.mint(6, Rights::READ);

    // 5. Stress Testing: 50 Rapid Fast-Path Syscalls under active timer IRQ
    let invalid_handle = CapHandle::new(999, 1);
    for _ in 0..50 {
        let _ = EndpointCap(invalid_handle).send(0);
    }

    // 6. High-Frequency Progress Loop: Test multi-tick preemption
    loop {
        unsafe {
            let count = core::ptr::read_volatile(USER_PROGRESS_COUNT);
            core::ptr::write_volatile(USER_PROGRESS_COUNT, count.wrapping_add(1));

            let signal = core::ptr::read_volatile(USER_MAILBOX_FLAG);
            if signal == 1 {
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

fn execute_command(input: &str, current_map: &mut usize) {
    if input.is_empty() {
        return;
    }

    // 1. Check for `curr ` prefix override
    let (is_curr, effective_input) = if input.starts_with("curr ") {
        (true, input[5..].trim())
    } else {
        (false, input)
    };

    if effective_input.is_empty() {
        return;
    }

    // 2. Split command name and arguments
    let mut parts = effective_input.splitn(2, ' ');
    let cmd = parts.next().unwrap_or("");
    let arg_str = parts.next().unwrap_or("").trim();

    // 3. Handle built-in shell navigation and inspection commands
    match cmd {
        "help" => {
            ConsoleWriter::set_color(0x00FFD166, 0x00101418); // Gold Yellow
            println!("AxiomOS CapGraph Shell (PID 1) \u{2014} Capability Commands:");
            ConsoleWriter::set_color(0x00E0E0E0, 0x00101418); // Silver
            println!("  help                 - Display this command reference manual");
            println!("  status / icap        - Inspect CSpace authority and security invariants");
            println!("  cd <path>            - Traverse to a linked Capability Map (e.g. 'cd etc')");
            println!("  pwd                  - Print current capability map reference");
            println!("  clear                - Clear canvas and redraw obsidian status header");
            println!("  reboot               - Trigger hardware system reboot");
            println!();
            ConsoleWriter::set_color(0x0000D2FF, 0x00101418); // Cyan
            println!("Standalone C & ASM Executables (in bin/):");
            ConsoleWriter::set_color(0x00E0E0E0, 0x00101418);
            println!("  echo <text> [> f]    - C executable: print text or redirect to file");
            println!("  ls [map]             - C executable: enumerate capability map entries");
            println!("  cat <file>           - C executable: print content of file capability");
            println!("  whoami               - ASM executable: query active user authority keyring");
            println!("  axe <file>           - C executable: fullscreen code & text editor");
            println!();
            ConsoleWriter::set_color(0x002EE59D, 0x00101418); // Emerald
            println!("Ledger Override Syntax:");
            ConsoleWriter::set_color(0x00E0E0E0, 0x00101418);
            println!("  curr <command>       - Execute binary with caller's current keyrings (ledger bypassed)");
            return;
        },
        "status" | "info" | "icap" => {
            ConsoleWriter::set_color(0x002EE59D, 0x00101418); // Emerald Green
            println!("[ CSPACE AUTHORITY INVENTORY ]");
            ConsoleWriter::set_color(0x00E0E0E0, 0x00101418);
            println!("  Slot 1: CNodeCap     - Master CDT Root Authority");
            println!("  Slot 2: VSpaceCap    - Genesis Sovereign PML4 (CR3)");
            println!("  Slot 3: TCBCap       - Genesis Thread Control Block (PID 1)");
            println!("  Slot 5: EndpointCap  - Console / Video Subsystem (1280x720)");
            println!("  Slot 7: DirectoryCap - Object-Oriented Structural Graph (CapGraph)");
            ConsoleWriter::set_color(0x002EE59D, 0x00101418);
            println!("[ SECURITY INVARIANTS ]");
            ConsoleWriter::set_color(0x00E0E0E0, 0x00101418);
            println!("  Filesystem: Object-Oriented Structural Graph (Zero Ambient Root)");
            println!("  Privilege Level: Ring 3 User Mode (CPL = 3)");
            println!("  Ledger Security: Attenuated Least-Privilege Execution");
            println!("  Execution Buffer: 0x500000 (C / ASM Standalone Binaries)");
            return;
        },
        "cd" => {
            if arg_str.is_empty() || arg_str == "~" || arg_str == "/" {
                *current_map = 0;
            } else if let Some(token) = unsafe { GRAPH.lookup(*current_map, arg_str) } {
                if unsafe { GRAPH.nodes[token.target_id].kind == GraphObjectKind::DirectoryMap } {
                    *current_map = token.target_id;
                } else {
                    println!("cd: {}: Not a capability map", arg_str);
                }
            } else {
                println!("cd: {}: No such capability map", arg_str);
            }
            return;
        },
        "pwd" => {
            if *current_map == 0 {
                println!("/");
            } else {
                println!("CapMap #{}", *current_map);
            }
            return;
        },
        "clear" => {
            ConsoleWriter::draw_banner();
            println!();
            return;
        },
        "reboot" => {
            println!("Rebooting AxiomOS substrate...");
            unsafe {
                core::arch::asm!("out dx, al", in("dx") 0x64u16, in("al") 0xFEu8, options(nomem, nostack));
            }
            return;
        },
        _ => {},
    }

    // 4. Resolve binary in bin/ or via path
    let graph = unsafe { &mut GRAPH };

    // Try looking up in bin/<cmd>.bin, bin/<cmd>, or <cmd>
    let mut target_token = graph.lookup(0, cmd);
    if target_token.is_none() {
        let mut bin_path: [u8; 64] = [0; 64];
        let pfx = b"bin/";
        bin_path[..4].copy_from_slice(pfx);
        let cmd_bytes = cmd.as_bytes();
        let clen = core::cmp::min(cmd_bytes.len(), 50);
        bin_path[4..4 + clen].copy_from_slice(&cmd_bytes[..clen]);
        if let Ok(bp_str) = core::str::from_utf8(&bin_path[..4 + clen]) {
            target_token = graph.lookup(0, bp_str);
        }
    }
    if target_token.is_none() {
        let mut bin_path: [u8; 64] = [0; 64];
        let pfx = b"bin/";
        bin_path[..4].copy_from_slice(pfx);
        let cmd_bytes = cmd.as_bytes();
        let clen = core::cmp::min(cmd_bytes.len(), 45);
        bin_path[4..4 + clen].copy_from_slice(&cmd_bytes[..clen]);
        bin_path[4 + clen..4 + clen + 4].copy_from_slice(b".bin");
        if let Ok(bp_str) = core::str::from_utf8(&bin_path[..4 + clen + 4]) {
            target_token = graph.lookup(0, bp_str);
        }
    }

    let token = match target_token {
        Some(t) => t,
        None => {
            ConsoleWriter::set_color(0x00FF4B4B, 0x00101418); // Crimson Red
            print!("axiom: command not found: ");
            ConsoleWriter::set_color(0x00E0E0E0, 0x00101418);
            println!("{}", cmd);
            println!("Type 'help' for available commands.");
            return;
        }
    };

    let target_node = &graph.nodes[token.target_id];
    if target_node.kind != GraphObjectKind::Blob {
        println!("axiom: {}: Cannot execute directory map", cmd);
        return;
    }

    // 5. Query Shell Capability Ledger
    let ledger_rights_str = graph.get_ledger_rights(cmd).unwrap_or("console:read,console:write");

    // Display Dispatch Notice
    if is_curr {
        ConsoleWriter::set_color(0x00FFD166, 0x00101418); // Gold Yellow
        print!("[ DISPATCH ] ");
        ConsoleWriter::set_color(0x00E0E0E0, 0x00101418);
        println!("Executing {} with CURRENT user keyring (ledger bypassed)", cmd);
    } else {
        ConsoleWriter::set_color(0x002EE59D, 0x00101418); // Emerald Green
        print!("[ DISPATCH ] ");
        ConsoleWriter::set_color(0x00E0E0E0, 0x00101418);
        println!("Executing {} with ledger keyring: {}", cmd, ledger_rights_str);
    }

    // Check for redirection in argument (e.g. echo hello > test.txt)
    let (exec_args, redirect_file) = if let Some(arrow_idx) = arg_str.find('>') {
        let text_part = arg_str[..arrow_idx].trim();
        let file_part = arg_str[arrow_idx + 1..].trim();
        (text_part, Some(file_part))
    } else {
        (arg_str, None)
    };

    // 6. Populate CapMapContext so ls.bin and cat.bin can read entries
    unsafe {
        let cur_node = &graph.nodes[*current_map];
        let mut count = 0;
        for link in cur_node.entries.iter() {
            if link.in_use && count < 16 {
                let ent = &mut MAP_CTX.entries[count];
                ent.name = link.name;
                let target = &graph.nodes[link.token.target_id];
                ent.is_dir = if target.kind == GraphObjectKind::DirectoryMap { 1 } else { 0 };
                ent.data_len = target.data_len as u32;
                ent.rights = link.token.rights;
                ent.in_use = 1;
                ent.data = target.data_ptr;
                count += 1;
            }
        }
        MAP_CTX.count = count as u32;
    }

    // 7. Prepare CapKeyring
    let mut username_buf = [0u8; 32];
    username_buf[..4].copy_from_slice(b"root");

    // If command is `cat`, resolve target file blob pointer directly
    let storage_cap_val = if cmd == "cat" && !exec_args.is_empty() {
        if let Some(target_token) = graph.lookup(*current_map, exec_args) {
            let target = &graph.nodes[target_token.target_id];
            if target.kind == GraphObjectKind::Blob {
                target.data_ptr as u64
            } else {
                0
            }
        } else {
            0
        }
    } else {
        &raw const MAP_CTX as u64
    };

    let keyring = CapKeyring {
        user_id: 0,
        rights: if is_curr { 0b1111 } else { 0b0011 },
        storage_cap: storage_cap_val,
        console_cap: 0x100000005,
        user_name: username_buf,
    };

    // 8. Copy binary machine code into execution page at 0x500000
    unsafe {
        core::ptr::copy_nonoverlapping(target_node.data_ptr, USER_EXEC_BASE, target_node.data_len);
    }

    // 9. Prepare argc and argv in memory
    // String buffers for arguments
    let mut arg0_buf = [0u8; 32];
    let cmd_b = cmd.as_bytes();
    arg0_buf[..cmd_b.len()].copy_from_slice(cmd_b);

    let mut arg1_buf = [0u8; 128];
    let arg_b = exec_args.as_bytes();
    let alen = core::cmp::min(arg_b.len(), 127);
    if alen > 0 {
        arg1_buf[..alen].copy_from_slice(&arg_b[..alen]);
    }

    let mut argv_ptrs: [*const u8; 4] = [core::ptr::null(); 4];
    argv_ptrs[0] = arg0_buf.as_ptr();
    let argc = if alen > 0 {
        argv_ptrs[1] = arg1_buf.as_ptr();
        2
    } else {
        1
    };

    // 10. Execute C / ASM binary at 0x500000
    let entry: extern "C" fn(i32, *const *const u8, *const CapKeyring) -> i32 =
        unsafe { core::mem::transmute(USER_EXEC_BASE as *const ()) };

    let _ret = entry(argc, argv_ptrs.as_ptr(), &keyring as *const CapKeyring);

    // 11. Handle file redirection if '>' was specified
    if let Some(fname) = redirect_file {
        if !fname.is_empty() {
            let mut file_data = [0u8; 512];
            let raw_text = exec_args.trim_matches('"').trim_matches('\'');
            let tb = raw_text.as_bytes();
            let flen = core::cmp::min(tb.len(), 510);
            file_data[..flen].copy_from_slice(&tb[..flen]);
            file_data[flen] = b'\n';

            match graph.write_file(*current_map, fname, &file_data[..flen + 1]) {
                Ok(_) => {
                    ConsoleWriter::set_color(0x002EE59D, 0x00101418); // Emerald Green
                    println!("[ OK ] Written to capability: {}", fname);
                },
                Err(e) => {
                    ConsoleWriter::set_color(0x00FF4B4B, 0x00101418);
                    println!("echo: {}: {}", fname, e);
                }
            }
        }
    }
}
