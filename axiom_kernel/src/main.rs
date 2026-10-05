#![no_std]
#![no_main]

mod interrupts;
mod gdt;
mod memory;
mod object;
mod capability;

use core::panic::PanicInfo;
use memory::BootInfo;
use capability::{CNode, Capability, Rights};
use object::{ObjectRef, ObjectKind, Untyped, ObjectTable};

#[allow(dead_code)]
static HELLO: &[u8] = b"AXIOM KERNEL ONLINE - by Soulfire";

#[no_mangle]
pub extern "C" fn _start(boot_info: *const BootInfo) -> ! {
    unsafe {
        extern "C" {
            static BSS_START: u8;
            static BSS_END: u8;
        }
        let start = core::ptr::addr_of!(BSS_START) as *mut u8;
        let end = core::ptr::addr_of!(BSS_END) as *mut u8;
        let len = end as usize - start as usize;
        core::ptr::write_bytes(start, 0, len);
    }

    let vga_buffer = 0xB8000 as *mut u8;

    // 1. Clear the entire screen to black
    for i in 0..2000 {
        unsafe {
            *vga_buffer.add(i * 2) = b' ';
            *vga_buffer.add(i * 2 + 1) = 0x0F;
        }
    }

    // 1. Validate the physical pointer and magic number
    let is_valid = unsafe {
        !boot_info.is_null() && (*boot_info).magic == 0xC0DEB007
    };

    

    let message = if is_valid {
        b"AXIOM KERNEL ONLINE [BOOT INFO VERIFIED]"
    } else {
        b"AXIOM KERNEL ONLINE [BOOT INFO FAILED]  "
    };

    // 2. Print the status to the top of the VGA buffer
    let mut offset = 0;
    for &byte in message.iter() {
        unsafe {
            *vga_buffer.add(offset) = byte;
            *vga_buffer.add(offset + 1) = if is_valid { 0x0A } else { 0x0C };
        }
        offset += 2;
    }
    
    //print_hex_64(is_valid as u64, 160 * 12, vga_buffer);
    // 3. If valid, prove it by printing the memory map entry count
    if is_valid {
        gdt::init();
        interrupts::init();

        let boot_allocator = unsafe { memory::FrameAllocator::new(boot_info) };
        let mut bitmap_alloc = unsafe { memory::BitmapAllocator::bootstrap(boot_allocator) };

        // ========================================================
        // PHASE 3: UNTYPED MEMORY & RETYPE TRANSACTION
        // ========================================================
        let mut object_table = ObjectTable::new();
        let mut cnode = CNode::new();

        // 1. Grab a raw physical frame and transform it into Untyped authority
        let frame = bitmap_alloc.allocate_frame().unwrap();
        let mut untyped = Untyped {
            physical_base: frame.start_address,
            size: 4096,
            watermark: 0,
        };
        
        // Output 1: Initial Watermark (Row 2, Expect: 0)
        print_hex_64(untyped.watermark, 160 * 2, vga_buffer);

        // 2. SUCCESS TEST: Retype an Endpoint
        let obj_ref1 = capability::retype(
            &mut untyped, 
            ObjectKind::Endpoint, // Requires 128 bytes, 64-byte aligned
            &mut object_table, 
            &mut cnode
        ).unwrap();

        // Output 2: Capability exists in CNode Slot 1 (Row 3, Expect: 1)
        let cap_valid = cnode.slots[1].capability.is_some();
        print_hex_64(cap_valid as u64, 160 * 3, vga_buffer); 

        // Output 3: ObjectRef resolves to correct physical address (Row 4, Expect: frame address)
        if let Some((kind, phys_addr)) = object_table.resolve(obj_ref1) {
            print_hex_64(phys_addr, 160 * 4, vga_buffer);
        }

        // Output 4: Watermark advanced by exactly 128 bytes (Row 5, Expect: 128 / 0x80)
        print_hex_64(untyped.watermark, 160 * 5, vga_buffer);
        // 3. FAILURE TEST: Insufficient memory
        let watermark_before = untyped.watermark;
        let fail_result = capability::retype(
            &mut untyped,
            ObjectKind::CNode, 
            &mut object_table,
            &mut cnode
        );

        // Output 5: Retype correctly returned an Error (Row 6, Expect: 1)
        let is_err = fail_result.is_err();
        print_hex_64(is_err as u64, 160 * 6, vga_buffer); 

        // Output 6: Transactional rollback - Watermark unchanged (Row 7, Expect: 1)
        let unchanged = watermark_before == untyped.watermark;
        print_hex_64(unchanged as u64, 160 * 7, vga_buffer);



        // 1. Destroy the first object
        object_table.destroy_object(obj_ref1);

        // Output 7: Old ObjectRef instantly resolves to None (Row 8, Expect: 1)
        let old_resolved_none = object_table.resolve(obj_ref1).is_none();
        print_hex_64(old_resolved_none as u64, 160 * 8, vga_buffer);

        // 2. Retype a second object (LIFO free-list guarantees exact ObjectTable slot reuse)
        let obj_ref2 = capability::retype(
            &mut untyped,
            ObjectKind::Endpoint,
            &mut object_table,
            &mut cnode
        ).unwrap();

        // Output 8: Generations are distinctly different (Row 9, Expect: 1)
        let gen_diff = obj_ref1.generation() != obj_ref2.generation();
        print_hex_64(gen_diff as u64, 160 * 9, vga_buffer);

        // Output 9: Old ObjectRef STILL resolves to None against reused slot (Row 10, Expect: 1)
        let old_still_none = object_table.resolve(obj_ref1).is_none();
        print_hex_64(old_still_none as u64, 160 * 10, vga_buffer);

        // Output 10: New ObjectRef resolves successfully (Row 11, Expect: 1)
        let new_resolved = object_table.resolve(obj_ref2).is_some();
        print_hex_64(new_resolved as u64, 160 * 11, vga_buffer);
        
        
        //unsafe { core::arch::asm!("int3"); } // Manually trigger a CPU Breakpoint exception
        //unsafe { core::arch::asm!("ud2"); } // Manually Trigger a CPU Kernal Panic
        
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
