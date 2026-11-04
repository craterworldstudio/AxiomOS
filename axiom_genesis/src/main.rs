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
    pub _pad: [u8; 6],
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
        _pad: [0; 6],
        data: core::ptr::null(),
    }; 16],
    count: 0,
};

#[repr(C)]
#[derive(Clone, Copy)]
pub struct CapBlobContext {
    pub data: [u8; 1024],
    pub length: u32,
    pub modified: u32,
}

static mut BLOB_CTX: CapBlobContext = CapBlobContext {
    data: [0; 1024],
    length: 0,
    modified: 0,
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
        println!("AxiomOS Sovereign Microkernel Substrate (x86_64)");
        ConsoleWriter::set_color(0x00E0E0E0, 0x00101418); // Crisp Silver
        println!("PID 1 (genesis) initialized in Ring 3. Capability Graph mounted.");
        println!("Type 'help' for available system commands.");
        println!();

        // Initialize Sovereign Object-Oriented Structural Graph (CapGraph)
        unsafe { GRAPH.init(); }

        let mut current_map = 0; // Map Node 0: Root Entry Map
        let mut cwd: [u8; 64] = [0; 64];
        cwd[0] = b'/';
        let mut cwd_len: usize = 1;

        // Default initial domain: vaults/root
        if let Some(vr) = unsafe { GRAPH.lookup(0, "vaults/root") } {
            if unsafe { GRAPH.nodes[vr.target_id].kind == GraphObjectKind::DirectoryMap } {
                current_map = vr.target_id;
                let vr_bytes = b"vaults/root";
                cwd[..vr_bytes.len()].copy_from_slice(vr_bytes);
                cwd_len = vr_bytes.len();
            }
        }

        let mut line_buf = [0u8; 128];

        loop {
            ConsoleWriter::set_color(0x005CB3E6, 0x00101418); // Soft Sovereign Cyan
            if let Ok(p) = core::str::from_utf8(&cwd[..cwd_len]) {
                print!("root@axiom:{}# ", p);
            } else {
                print!("root@axiom:vaults/root# ");
            }

            let len = ConsoleReader::read_line(&mut line_buf);
            if len == 0 {
                continue;
            }

            let cmd_str = match core::str::from_utf8(&line_buf[..len]) {
                Ok(s) => s.trim(),
                Err(_) => continue,
            };

            execute_command(cmd_str, &mut current_map, &mut cwd, &mut cwd_len);
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

fn execute_command(input: &str, current_map: &mut usize, cwd: &mut [u8; 64], cwd_len: &mut usize) {
    if input.is_empty() {
        return;
    }

    // 1. Check for `curr ` prefix override
    if input == "curr" || input == "curr -h" || input == "curr --help" {
        ConsoleWriter::set_color(0x00FFD166, 0x00101418); // Gold Yellow
        println!("Usage: curr <command> [args...]");
        ConsoleWriter::set_color(0x00E0E0E0, 0x00101418); // Silver
        println!("Execute binary with caller's current keyrings (ledger bypassed).");
        println!("Options:");
        println!("  -h, --help    Show this command reference manual");
        return;
    }

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
            if arg_str == "-h" || arg_str == "--help" {
                ConsoleWriter::set_color(0x00FFD166, 0x00101418);
                println!("Usage: help [options]");
                ConsoleWriter::set_color(0x00E0E0E0, 0x00101418);
                println!("Display the master command reference manual for AxiomOS.");
                println!("Options:");
                println!("  -h, --help    Show this command reference manual");
                println!("Tip: Run any command with -h (e.g. ls -h, cd -h) for dedicated help.");
                return;
            }
            ConsoleWriter::set_color(0x00FFD166, 0x00101418); // Gold Yellow
            println!("AxiomOS Built-in Commands:");
            ConsoleWriter::set_color(0x00E0E0E0, 0x00101418); // Silver
            println!("  help                 - Display this command reference manual");
            println!("  status / icap        - Inspect CSpace authority and security invariants");
            println!("  cd <path>            - Change working directory");
            println!("  pwd                  - Print current directory path");
            println!("  clear                - Clear console canvas");
            println!("  reboot               - Trigger hardware system reboot");
            println!("  shutdown / poweroff  - Cleanly power down substrate / hypervisor");
            println!("  kpanic [scenario]    - Test Kernel Panic diagnostics (assert/gpf/pf/df/div0)");
            println!();
            ConsoleWriter::set_color(0x0000D2FF, 0x00101418); // Cyan
            println!("Available Executables (/apps/sys):");
            ConsoleWriter::set_color(0x00E0E0E0, 0x00101418);

            // Dynamically execute all binaries registered in /apps/sys with -dh
            let graph = unsafe { &mut GRAPH };
            if let Some(bin_token) = graph.lookup(0, "apps/sys") {
                let bin_node = &graph.nodes[bin_token.target_id];
                if bin_node.kind == GraphObjectKind::DirectoryMap {
                    for link in bin_node.entries.iter() {
                        if link.in_use {
                            let target = &graph.nodes[link.token.target_id];
                            if target.kind == GraphObjectKind::Blob && target.data_len > 0 {
                                unsafe {
                                    core::ptr::copy_nonoverlapping(target.data_ptr, USER_EXEC_BASE, target.data_len);
                                }
                                let mut arg0 = [0u8; 32];
                                let nlen = core::cmp::min(link.name_len, 31);
                                arg0[..nlen].copy_from_slice(&link.name[..nlen]);

                                let mut arg1 = [0u8; 8];
                                arg1[..3].copy_from_slice(b"-dh");

                                let argv_ptrs: [*const u8; 2] = [arg0.as_ptr(), arg1.as_ptr()];
                                let keyring = CapKeyring {
                                    user_id: 0,
                                    rights: 0b1111,
                                    storage_cap: &raw const MAP_CTX as u64,
                                    console_cap: 0x100000005,
                                    user_name: [0; 32],
                                };

                                let entry: extern "C" fn(i32, *const *const u8, *const CapKeyring) -> i32 =
                                    unsafe { core::mem::transmute(USER_EXEC_BASE as *const ()) };
                                let _ = entry(2, argv_ptrs.as_ptr(), &keyring as *const CapKeyring);
                            }
                        }
                    }
                }
            }

            ConsoleWriter::set_color(0x002EE59D, 0x00101418); // Emerald
            println!();
            println!("Ledger Override:");
            ConsoleWriter::set_color(0x00E0E0E0, 0x00101418);
            println!("  curr <command>       - Execute binary with caller's current keyrings (ledger bypassed)");
            return;
        },
        "status" | "info" | "icap" => {
            if arg_str == "-h" || arg_str == "--help" {
                ConsoleWriter::set_color(0x00FFD166, 0x00101418);
                println!("Usage: status [options] (or icap, info)");
                ConsoleWriter::set_color(0x00E0E0E0, 0x00101418);
                println!("Inspect CSpace authority inventory and substrate security invariants.");
                println!("Options:");
                println!("  -h, --help    Show this command reference manual");
                return;
            }
            ConsoleWriter::set_color(0x002EE59D, 0x00101418); // Emerald Green
            println!("[ CSPACE AUTHORITY INVENTORY ]");
            ConsoleWriter::set_color(0x00E0E0E0, 0x00101418);
            println!("  Slot 1: CNodeCap     - Master CDT Root Authority");
            println!("  Slot 2: VSpaceCap    - Genesis Sovereign PML4 (CR3)");
            println!("  Slot 3: TCBCap       - Genesis Thread Control Block (PID 1)");
            println!("  Slot 5: EndpointCap  - Console / Video Subsystem (1280x720)");
            println!("  Slot 7: EndpointCap  - Persistent Storage Drive (ATA PIO / edps/drives/ata0)");
            ConsoleWriter::set_color(0x002EE59D, 0x00101418);
            println!("[ SECURITY INVARIANTS ]");
            ConsoleWriter::set_color(0x00E0E0E0, 0x00101418);
            println!("  Filesystem: Object-Oriented Structural Graph (Zero Ambient Root)");
            println!("  Privilege Level: Ring 3 User Mode (CPL = 3)");
            println!("  Ledger Security: Attenuated Least-Privilege Execution (/meta/caps.ledger)");
            println!("  Execution Buffer: 0x500000 (C / ASM Standalone Binaries)");
            return;
        },
        "cd" => {
            if arg_str == "-h" || arg_str == "--help" {
                ConsoleWriter::set_color(0x00FFD166, 0x00101418);
                println!("Usage: cd [path] [options]");
                ConsoleWriter::set_color(0x00E0E0E0, 0x00101418);
                println!("Change current working capability directory map.");
                println!("Options:");
                println!("  -h, --help    Show this command reference manual");
                println!("  ..            Move up one directory level (supports multi-level ../..)");
                println!("  ~             Navigate to root vault (vaults/root)");
                return;
            }
            let cur_str = core::str::from_utf8(&cwd[..*cwd_len]).unwrap_or("/");
            let target_input = if arg_str.is_empty() { "~" } else { arg_str };
            let (res_buf, res_len) = resolve_path(cur_str, target_input);
            let res_str = core::str::from_utf8(&res_buf[..res_len]).unwrap_or("/");

            if res_str == "/" {
                *current_map = 0;
                cwd[0] = b'/';
                *cwd_len = 1;
            } else if let Some(token) = unsafe { GRAPH.lookup(0, res_str) } {
                if unsafe { GRAPH.nodes[token.target_id].kind == GraphObjectKind::DirectoryMap } {
                    *current_map = token.target_id;
                    cwd[..res_len].copy_from_slice(&res_buf[..res_len]);
                    *cwd_len = res_len;
                } else {
                    println!("cd: {}: Not a directory", arg_str);
                }
            } else {
                println!("cd: {}: No such file or directory", arg_str);
            }
            return;
        },
        "pwd" => {
            if arg_str == "-h" || arg_str == "--help" {
                ConsoleWriter::set_color(0x00FFD166, 0x00101418);
                println!("Usage: pwd [options]");
                ConsoleWriter::set_color(0x00E0E0E0, 0x00101418);
                println!("Print current directory path in the capability graph.");
                println!("Options:");
                println!("  -h, --help    Show this command reference manual");
                return;
            }
            if let Ok(p) = core::str::from_utf8(&cwd[..*cwd_len]) {
                println!("{}", p);
            } else {
                println!("/");
            }
            return;
        },
        "clear" => {
            if arg_str == "-h" || arg_str == "--help" {
                ConsoleWriter::set_color(0x00FFD166, 0x00101418);
                println!("Usage: clear [options]");
                ConsoleWriter::set_color(0x00E0E0E0, 0x00101418);
                println!("Clear console canvas and redraw system status banner.");
                println!("Options:");
                println!("  -h, --help    Show this command reference manual");
                return;
            }
            ConsoleWriter::draw_banner();
            println!();
            return;
        },
        "reboot" => {
            if arg_str == "-h" || arg_str == "--help" {
                ConsoleWriter::set_color(0x00FFD166, 0x00101418);
                println!("Usage: reboot [options]");
                ConsoleWriter::set_color(0x00E0E0E0, 0x00101418);
                println!("Trigger hardware system reboot of the AxiomOS substrate.");
                println!("Options:");
                println!("  -h, --help    Show this command reference manual");
                return;
            }
            ConsoleWriter::set_color(0x00FFD166, 0x00101418); // Gold Yellow
            println!("Rebooting AxiomOS substrate...");
            unsafe {
                libaxiom::syscall::invoke_short(0x100000005, libaxiom::syscall::OP_SYS_REBOOT, 0, 0);
            }
            return;
        },
        "shutdown" | "poweroff" => {
            if arg_str == "-h" || arg_str == "--help" {
                ConsoleWriter::set_color(0x00FFD166, 0x00101418);
                println!("Usage: shutdown [options] (or poweroff)");
                ConsoleWriter::set_color(0x00E0E0E0, 0x00101418);
                println!("Cleanly power down machine substrate or hypervisor via ACPI.");
                println!("Options:");
                println!("  -h, --help    Show this command reference manual");
                return;
            }
            ConsoleWriter::set_color(0x00FFD166, 0x00101418); // Gold Yellow
            println!("Powering down AxiomOS substrate...");
            unsafe {
                libaxiom::syscall::invoke_short(0x100000005, libaxiom::syscall::OP_SYS_SHUTDOWN, 0, 0);
            }
            return;
        },
        "kpanic" | "panic" => {
            let scenario = match arg_str {
                "gpf" | "1" => 1u64,
                "pagefault" | "pf" | "2" => 2u64,
                "doublefault" | "df" | "3" => 3u64,
                "divzero" | "div0" | "4" => 4u64,
                "help" | "-h" | "--help" => {
                    ConsoleWriter::set_color(0x00FFD166, 0x00101418);
                    println!("Usage: kpanic [scenario] [options]");
                    ConsoleWriter::set_color(0x00E0E0E0, 0x00101418);
                    println!("Trigger test hardware exception vectors and diagnostic panic screen.");
                    println!("Scenarios:");
                    println!("  assert        - Kernel Rust assertion failure panic!() [default]");
                    println!("  gpf           - General Protection Fault (#13) test");
                    println!("  pagefault     - Kernel Page Fault (#14) unmapped dereference");
                    println!("  doublefault   - Double Fault (#8) exception test");
                    println!("  divzero       - Divide by Zero (#0) hardware trap test");
                    println!("Options:");
                    println!("  -h, --help    Show this command reference manual");
                    return;
                },
                _ => 0u64, // Default: assert
            };

            ConsoleWriter::set_color(0x00FF4B4B, 0x00101418); // Crimson Red
            println!("[TRIGGER] Invoking Kernel Panic Diagnostic Scenario: {}...", if arg_str.is_empty() { "assert" } else { arg_str });
            unsafe {
                libaxiom::syscall::invoke_short(0x100000005, libaxiom::syscall::OP_KERNEL_PANIC, scenario, 0);
            }
            return;
        },
        _ => {},
    }

    // 4. Resolve binary in bin/ or via path
    let graph = unsafe { &mut GRAPH };

    // Check for redirection in argument (e.g. echo hello > test.txt)
    let (exec_args, redirect_file) = if let Some(arrow_idx) = arg_str.find('>') {
        let text_part = arg_str[..arrow_idx].trim();
        let file_part = arg_str[arrow_idx + 1..].trim();
        (text_part, Some(file_part))
    } else {
        (arg_str, None)
    };

    // If redirected with echo, write file directly and do not echo to screen
    if let Some(fname) = redirect_file {
        if !fname.is_empty() {
            let mut file_data = [0u8; 512];
            let raw_text = exec_args.trim_matches('"').trim_matches('\'');
            let tb = raw_text.as_bytes();
            let flen = core::cmp::min(tb.len(), 510);
            file_data[..flen].copy_from_slice(&tb[..flen]);
            file_data[flen] = b'\n';

            let cur_str = core::str::from_utf8(&cwd[..*cwd_len]).unwrap_or("/");
            let (res_buf, res_len) = resolve_path(cur_str, fname);
            if let Ok(res_str) = core::str::from_utf8(&res_buf[..res_len]) {
                if let Err(e) = graph.write_file(0, res_str, &file_data[..flen + 1]) {
                    ConsoleWriter::set_color(0x00FF4B4B, 0x00101418);
                    println!("echo: {}: {}", fname, e);
                }
            }
            return;
        }
    }

    // Determine target map for directory listings
    let cur_str = core::str::from_utf8(&cwd[..*cwd_len]).unwrap_or("/");
    let mut ls_path: Option<&str> = None;
    let mut is_help_flag = false;

    for token in exec_args.split_whitespace() {
        if token == "-h" || token == "--help" || token == "-dh" {
            is_help_flag = true;
        } else if !token.starts_with('-') {
            ls_path = Some(token);
        }
    }

    let target_map_id = if cmd == "ls" && !is_help_flag {
        if let Some(path_arg) = ls_path {
            let (res_buf, res_len) = resolve_path(cur_str, path_arg);
            let res_str = core::str::from_utf8(&res_buf[..res_len]).unwrap_or("/");
            if res_str == "/" {
                0
            } else if let Some(token) = graph.lookup(0, res_str) {
                if graph.nodes[token.target_id].kind == GraphObjectKind::DirectoryMap {
                    token.target_id
                } else {
                    println!("ls: cannot access '{}': Not a directory", path_arg);
                    return;
                }
            } else {
                println!("ls: cannot access '{}': No such file or directory", path_arg);
                return;
            }
        } else {
            *current_map
        }
    } else {
        *current_map
    };

    // Populate MAP_CTX so ls.bin can read entries
    unsafe {
        let map_node = &graph.nodes[target_map_id];
        let mut count = 0;
        for link in map_node.entries.iter() {
            if link.in_use && count < 16 {
                let ent = &mut MAP_CTX.entries[count];
                ent.name = link.name;
                let target = &graph.nodes[link.token.target_id];
                ent.is_dir = if target.kind == GraphObjectKind::DirectoryMap { 
                    1 
                } else if target.kind == GraphObjectKind::DeviceEndpoint { 
                    2 
                } else { 
                    0 
                };
                ent.data_len = target.data_len as u32;
                ent.rights = link.token.rights;
                ent.in_use = 1;
                ent._pad = [0; 6];
                ent.data = target.data_ptr;
                count += 1;
            }
        }
        MAP_CTX.count = count as u32;
    }

    // Try looking up:
    // - If cmd contains '/': resolve via explicit path (./local or /apps/sys/...)
    // - If cmd is a bare word: search ONLY apps/sys/<cmd>.bin and apps/sys/<cmd>
    let mut target_token = None;

    if cmd.contains('/') {
        if cmd.starts_with("./") {
            let local_cmd = &cmd[2..];
            target_token = graph.lookup(*current_map, local_cmd);
            if target_token.is_none() {
                let mut bin_path: [u8; 64] = [0; 64];
                let cmd_bytes = local_cmd.as_bytes();
                let clen = core::cmp::min(cmd_bytes.len(), 50);
                bin_path[..clen].copy_from_slice(&cmd_bytes[..clen]);
                bin_path[clen..clen + 4].copy_from_slice(b".bin");
                if let Ok(bp_str) = core::str::from_utf8(&bin_path[..clen + 4]) {
                    target_token = graph.lookup(*current_map, bp_str);
                }
            }
        } else {
            let clean_cmd = cmd.trim_matches('/');
            target_token = graph.lookup(0, clean_cmd);
            if target_token.is_none() {
                let mut bin_path: [u8; 64] = [0; 64];
                let cmd_bytes = clean_cmd.as_bytes();
                let clen = core::cmp::min(cmd_bytes.len(), 50);
                bin_path[..clen].copy_from_slice(&cmd_bytes[..clen]);
                bin_path[clen..clen + 4].copy_from_slice(b".bin");
                if let Ok(bp_str) = core::str::from_utf8(&bin_path[..clen + 4]) {
                    target_token = graph.lookup(0, bp_str);
                }
            }
        }
    } else {
        // Bare command: search system binary catalog (apps/sys/<cmd>.bin and apps/sys/<cmd>)
        let mut bin_path: [u8; 64] = [0; 64];
        let pfx = b"apps/sys/";
        bin_path[..pfx.len()].copy_from_slice(pfx);
        let cmd_bytes = cmd.as_bytes();
        let clen = core::cmp::min(cmd_bytes.len(), 45);
        bin_path[pfx.len()..pfx.len() + clen].copy_from_slice(&cmd_bytes[..clen]);
        let ext = b".bin";
        bin_path[pfx.len() + clen..pfx.len() + clen + ext.len()].copy_from_slice(ext);
        let total = pfx.len() + clen + ext.len();
        if let Ok(bp_str) = core::str::from_utf8(&bin_path[..total]) {
            target_token = graph.lookup(0, bp_str);
        }

        if target_token.is_none() {
            let mut bin_path2: [u8; 64] = [0; 64];
            bin_path2[..pfx.len()].copy_from_slice(pfx);
            bin_path2[pfx.len()..pfx.len() + clen].copy_from_slice(&cmd_bytes[..clen]);
            let total2 = pfx.len() + clen;
            if let Ok(bp_str) = core::str::from_utf8(&bin_path2[..total2]) {
                target_token = graph.lookup(0, bp_str);
            }
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
    if target_node.kind == GraphObjectKind::DirectoryMap {
        println!("axiom: {}: Is a directory", cmd);
        return;
    }
    if target_node.kind == GraphObjectKind::DeviceEndpoint {
        println!("axiom: {}: Cannot execute capability endpoint", cmd);
        return;
    }
    if target_node.kind != GraphObjectKind::Blob {
        println!("axiom: {}: Not an executable binary", cmd);
        return;
    }

    // Security & Execution Guard:
    // 1. Check capability rights: must possess RIGHTS_EXEC
    if (token.rights & libaxiom::fs::RIGHTS_EXEC) == 0 {
        ConsoleWriter::set_color(0x00FF4B4B, 0x00101418);
        println!("axiom: {}: Permission denied (not executable)", cmd);
        return;
    }

    // 2. Reject common non-executable document file extensions
    let non_exec_exts = [".txt", ".md", ".sys", ".ledger", ".log", ".csv", ".json"];
    for ext in non_exec_exts {
        if cmd.ends_with(ext) {
            ConsoleWriter::set_color(0x00FF4B4B, 0x00101418);
            println!("axiom: cannot execute '{}': Exec format error", cmd);
            return;
        }
    }

    // 5. Prepare CapKeyring
    let mut username_buf = [0u8; 32];
    username_buf[..4].copy_from_slice(b"root");

    // If command is `cat` or `axe`, resolve target file blob into BLOB_CTX
    let storage_cap_val = if (cmd == "cat" || cmd == "axe") && !exec_args.is_empty() && exec_args != "-dh" && exec_args != "-h" && exec_args != "--help" {
        unsafe {
            BLOB_CTX.length = 0;
            BLOB_CTX.modified = 0;

            let mut file_arg: Option<&str> = None;
            for token in exec_args.split_whitespace() {
                if !token.starts_with('-') {
                    file_arg = Some(token);
                    break;
                }
            }

            if let Some(fpath) = file_arg {
                let cur_str = core::str::from_utf8(&cwd[..*cwd_len]).unwrap_or("/");
                let (res_buf, res_len) = resolve_path(cur_str, fpath);
                let res_str = core::str::from_utf8(&res_buf[..res_len]).unwrap_or("");

                if let Some(target_token) = graph.lookup(0, res_str) {
                    let target = &graph.nodes[target_token.target_id];
                    if target.kind == GraphObjectKind::Blob && target.data_len > 0 {
                        let clen = core::cmp::min(target.data_len, 1024);
                        core::ptr::copy_nonoverlapping(target.data_ptr, BLOB_CTX.data.as_mut_ptr(), clen);
                        BLOB_CTX.length = clen as u32;
                    } else if target.kind == GraphObjectKind::DeviceEndpoint {
                        if cmd == "axe" {
                            println!("axe: cannot edit capability endpoint '{}'", fpath);
                            return;
                        }
                        if target.data_len > 0 && !target.data_ptr.is_null() {
                            let clen = core::cmp::min(target.data_len, 1024);
                            core::ptr::copy_nonoverlapping(target.data_ptr, BLOB_CTX.data.as_mut_ptr(), clen);
                            BLOB_CTX.length = clen as u32;
                        } else {
                            let slot = target.data_len;
                            let desc: &[u8] = if res_str.ends_with("ata0p1") {
                                b"[CAPABILITY ENDPOINT: System Substrate Partition Slice (ata0p1)]\nRange: LBA 2048..10239 (4 MB, Read-Only)\nOperations: OP_STORAGE_READ(20)\n"
                            } else if res_str.ends_with("ata0p2") {
                                b"[CAPABILITY ENDPOINT: Continual VaultStore Partition Slice (ata0p2)]\nRange: LBA 10240..32734 (11.5 MB, Read/Write)\nOperations: OP_STORAGE_READ(20), OP_STORAGE_WRITE(21)\nFormat: Sovereign CapObjects (CoW Generation Monotonic Extents)\n"
                            } else {
                                match slot {
                                    5 => b"[CAPABILITY ENDPOINT: Console Video & Framebuffer Subsystem (Slot 5)]\nOperations: OP_CONSOLE_WRITE(6), CLEAR(7), BANNER(8), COLOR(9), GETCHAR(10), SET_CURSOR(11)\nChannels: Framebuffer (1280x720 32bpp) & PS/2 Keyboard Input\n",
                                    7 => b"[CAPABILITY ENDPOINT: Primary ATA PIO Storage Controller (Slot 7 / ata0)]\nOperations: OP_STORAGE_READ(20), OP_STORAGE_WRITE(21)\nChannels: IDE Port 0x1F0..0x1F7 (512-byte raw sectors)\nPartition Scheme: UEFI GPT (Protective MBR)\n",
                                    _ => b"[CAPABILITY ENDPOINT: Hardware Device Interface]\n",
                                }
                            };
                            let dlen = desc.len();
                            BLOB_CTX.data[..dlen].copy_from_slice(desc);
                            BLOB_CTX.length = dlen as u32;
                        }
                    } else if target.kind == GraphObjectKind::DirectoryMap {
                        if cmd == "axe" {
                            println!("axe: cannot edit directory map '{}'", fpath);
                            return;
                        }
                        let msg = b"cat: Is a capability directory map (use 'ls' to view entries)\n";
                        BLOB_CTX.data[..msg.len()].copy_from_slice(msg);
                        BLOB_CTX.length = msg.len() as u32;
                    }
                }
            }
            &raw mut BLOB_CTX as u64
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

    // 6. Copy binary machine code into execution page at 0x500000
    unsafe {
        core::ptr::copy_nonoverlapping(target_node.data_ptr, USER_EXEC_BASE, target_node.data_len);
    }

    // 7. Prepare argc and argv in memory
    let mut arg0_buf = [0u8; 32];
    let cmd_b = cmd.as_bytes();
    arg0_buf[..cmd_b.len()].copy_from_slice(cmd_b);

    let mut arg_bufs: [[u8; 64]; 8] = [[0u8; 64]; 8];
    let mut argv_ptrs: [*const u8; 9] = [core::ptr::null(); 9];
    argv_ptrs[0] = arg0_buf.as_ptr();
    let mut argc = 1;

    let bytes = exec_args.as_bytes();
    let mut i = 0;
    while i < bytes.len() && argc < 8 {
        while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t') {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }

        let start = i;
        if bytes[i] == b'"' || bytes[i] == b'\'' {
            let q = bytes[i];
            i += 1;
            while i < bytes.len() && bytes[i] != q {
                i += 1;
            }
            if i < bytes.len() {
                i += 1;
            }
        } else {
            while i < bytes.len() && bytes[i] != b' ' && bytes[i] != b'\t' {
                i += 1;
            }
        }

        let token_slice = &bytes[start..i];
        let tlen = core::cmp::min(token_slice.len(), 63);
        arg_bufs[argc - 1][..tlen].copy_from_slice(&token_slice[..tlen]);
        arg_bufs[argc - 1][tlen] = 0;
        argv_ptrs[argc] = arg_bufs[argc - 1].as_ptr();
        argc += 1;
    }

    // 8. Execute C / ASM binary at 0x500000
    let entry: extern "C" fn(i32, *const *const u8, *const CapKeyring) -> i32 =
        unsafe { core::mem::transmute(USER_EXEC_BASE as *const ()) };

    let _ret = entry(argc as i32, argv_ptrs.as_ptr(), &keyring as *const CapKeyring);

    // 9. If axe modified the buffer, flush writeback to CapGraph
    if cmd == "axe" && !exec_args.is_empty() && exec_args != "-dh" {
        unsafe {
            if BLOB_CTX.modified != 0 {
                let len = core::cmp::min(BLOB_CTX.length as usize, 1024);
                let cur_str = core::str::from_utf8(&cwd[..*cwd_len]).unwrap_or("/");
                let (res_buf, res_len) = resolve_path(cur_str, exec_args.trim());
                if let Ok(res_str) = core::str::from_utf8(&res_buf[..res_len]) {
                    let _ = graph.write_file(0, res_str, &BLOB_CTX.data[..len]);
                }
            }
        }
    }
}
