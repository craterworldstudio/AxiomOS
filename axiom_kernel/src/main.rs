#![no_std]
#![no_main]

use core::panic::PanicInfo;

#[repr(C)]
pub struct BootInfo {
    pub magic: u32,
    pub version: u32,
    pub memory_map_ptr: u64,
    pub memory_map_len: u64,
    pub memory_map_entry_size: u64,
}

#[repr(C)]
pub struct MemoryMapEntry {
    pub base: u64,
    pub length: u64,
    pub region_type: u32,
    pub acpi_extended_attributes: u32,
}

static HELLO: &[u8] = b"AXIOM KERNEL ONLINE - by Soulfire";

#[no_mangle]
pub extern "C" fn _start(boot_info: *const BootInfo) -> ! {
    let vga_buffer = 0xB8000 as *mut u8;

    // 1. Validate the physical pointer and magic number
    let is_valid = unsafe {
        !boot_info.is_null() && (*boot_info).magic == 0xC0DEB007
    };

    let message = if is_valid {
        b"AXIOM KERNEL ONLINE [BOOT INFO VERIFIED]"
    } else {
        b"AXIOM KERNEL ONLINE [BOOT INFO FAILED]"
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
    }

    loop {}
}

/// This function is called on kernel panic.
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
