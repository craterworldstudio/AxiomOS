#![no_std]
#![no_main]

mod interrupts;
mod gdt;
mod memory;
mod object;
mod capability;

use core::panic::PanicInfo;
use memory::BootInfo;
use capability::{CNode, Capability, Rights, CapHandle};
use object::{ObjectRef, ObjectKind, Untyped, ObjectTable};

#[allow(dead_code)]
static HELLO: &[u8] = b"AXIOM KERNEL ONLINE - by Soulfire";

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
            // MILESTONE (BRICK #4A): INVOKE ROUTER & ENDPOINT DISPATCH
            // ========================================================

            let object_table = unsafe { &mut capability::KERNEL_OBJECT_TABLE };
            let cnode = unsafe { &mut capability::GENESIS_CNODE };

            let base_addr1 = bitmap_alloc.allocate_contiguous(1).unwrap();
            let mut untyped1 = object::Untyped {
                physical_base: base_addr1, size: 4096, watermark: 0,
            };

            // 1. Retype a valid Endpoint
            let (_, ep_slot) = capability::retype(
                &mut untyped1, object::ObjectKind::Endpoint, object_table, cnode
            ).unwrap();
            
            let ep_handle = capability::CapHandle::new(ep_slot as u32, cnode.slots[ep_slot].generation).0;

            // TEST 1: Valid SEND -> SUCCESS (Row 8, Expect: 0)
            // (The payload 0xDEADC0DE will be printed by the loopback at Row 9!)
            let res_success = unsafe { invoke_syscall(ep_handle, capability::OP_SEND, 0xDEADC0DE, 0) };
            print_hex_64(res_success, 160 * 8, vga_buffer); 

            // TEST 2: Stale Generation -> REJECT (Row 10, Expect: 2)
            let stale_handle = capability::CapHandle::new(ep_slot as u32, 999).0;
            let res_stale = unsafe { invoke_syscall(stale_handle, capability::OP_SEND, 0, 0) };
            print_hex_64(res_stale, 160 * 10, vga_buffer); 

            // TEST 3: Missing SEND (WRITE) Right -> REJECT (Row 11, Expect: 3)
            let mut readonly_cap = cnode.slots[ep_slot].capability.unwrap();
            readonly_cap.rights = capability::Rights::READ; // Strip WRITE authority
            cnode.slots[2].insert(readonly_cap);
            let ro_handle = capability::CapHandle::new(2, cnode.slots[2].generation).0;
            
            let res_rights = unsafe { invoke_syscall(ro_handle, capability::OP_SEND, 0, 0) };
            print_hex_64(res_rights, 160 * 11, vga_buffer); 

            // TEST 4: Wrong Capability Kind -> REJECT (Row 12, Expect: 4)
            // Retype a CNode and attempt to SEND a message to it
            let (_, tcb_slot) = capability::retype(
                &mut untyped1, object::ObjectKind::TCB, object_table, cnode
            ).unwrap();
            let tcb_handle = capability::CapHandle::new(tcb_slot as u32, cnode.slots[tcb_slot].generation).0;
            
            let res_kind = unsafe { invoke_syscall(tcb_handle, capability::OP_SEND, 0, 0) };
            print_hex_64(res_kind, 160 * 12, vga_buffer);
        
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