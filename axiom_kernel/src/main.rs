#![no_std]
#![no_main]

mod interrupts;
mod gdt;
mod memory;

use core::panic::PanicInfo;
use memory::BootInfo;

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
        let count = unsafe { (*boot_info).memory_map_len };
        let count_msg = b" | ENTRIES: ";
        for &byte in count_msg.iter() {
            unsafe {
                *vga_buffer.add(offset) = byte;
                *vga_buffer.add(offset + 1) = 0x0F;
            }
            offset += 2;
        }

        let hex_chars = b"0123456789ABCDEF";
        let high = hex_chars[((count >> 4) & 0x0F) as usize];
        let low = hex_chars[(count & 0x0F) as usize];

        unsafe {
            *vga_buffer.add(offset) = high;
            *vga_buffer.add(offset + 1) = 0x0E;
            *vga_buffer.add(offset + 2) = low;
            *vga_buffer.add(offset + 3) = 0x0E;
        }

        // --- SUBSYSTEM INITIALIZATION ---
        gdt::init();

        let vga = 0xB8000 as *mut u8;

        interrupts::init();



        
        //unsafe { core::arch::asm!("int3"); } // Manually trigger a CPU Breakpoint exception
        //unsafe { core::arch::asm!("ud2"); } // Manually Trigger a CPU Kernal Panic




        
        // --- ALLOCATION & DEALLOCATION TEST ---
        let boot_allocator = unsafe { memory::FrameAllocator::new(boot_info) };
        let mut bitmap_alloc = unsafe { memory::BitmapAllocator::bootstrap(boot_allocator) };

        // 1. Allocate a frame. This should grab frame 512 (0x200000).
        let frame1 = bitmap_alloc.allocate_frame().unwrap();
        print_hex_64(frame1.start_address, 160, vga_buffer); // Row 1

        // 2. Allocate another frame. This should grab frame 513 (0x201000).
        let frame2 = bitmap_alloc.allocate_frame().unwrap();
        print_hex_64(frame2.start_address, 320, vga_buffer); // Row 2

        // 3. Deallocate the FIRST frame (0x200000).
        bitmap_alloc.deallocate_frame(frame1);

        // 4. Allocate a third frame. 
        // A bump allocator would give 0x202000. 
        // Our true allocator should reuse the newly freed 0x200000!
        let frame3 = bitmap_alloc.allocate_frame().unwrap();
        print_hex_64(frame3.start_address, 480, vga_buffer); // Row 3
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