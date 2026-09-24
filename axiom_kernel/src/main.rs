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

#[allow(dead_code)]
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

        // --- ALLOCATOR TEST ---
        let mut allocator = unsafe { FrameAllocator::new(boot_info) };
        
        if let Some(f1) = allocator.allocate_frame() { print_hex_64(f1.start_address, 160, vga_buffer); }
        if let Some(f2) = allocator.allocate_frame() { print_hex_64(f2.start_address, 320, vga_buffer); }
        if let Some(f3) = allocator.allocate_frame() { print_hex_64(f3.start_address, 480, vga_buffer); }
    }

    loop {}
}

/// This function is called on kernel panic.
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

// ==========================================
// Memory Management Subsystem
// ==========================================

pub const PAGE_SIZE: u64 = 4096;

#[derive(Debug, Clone, Copy)]
pub struct PhysFrame {
    pub start_address: u64,
}

struct ReservedRegion {
    start: u64,
    end: u64,
}

// Axiom 1 Boot Reservations based on our physical memory map
const RESERVED_REGIONS: [ReservedRegion; 3] = [

    // 1. Lower Memory Quarantine (IVT, BDA, Bootloader, Page Tables, Stack)
    // Completely locks down physical memory from 0x0 to the stack top.
    ReservedRegion {
        start: 0x00000000,
        end: 0x00100000,
    },

    // 2. VGA Hardware Buffer (Now technically redundant, but safe to keep)
    ReservedRegion {
        start: 0x000B8000,
        end: 0x000B9000,
    },

    // 3. Axiom Kernel Image 
    // Safely reserving the entire 1 MiB block from 0x100000 to 0x1FFFFF
    ReservedRegion {
        start: 0x00100000,
        end: 0x00200000,
    },
];

pub struct FrameAllocator {
    boot_info: *const BootInfo,
    current_entry_index: usize,
    next_free_address: u64,
}

impl FrameAllocator {
    pub unsafe fn new(boot_info: *const BootInfo) -> Self {
        FrameAllocator {
            boot_info,
            current_entry_index: 0,
            next_free_address: 0,
        }
    }

    fn is_reserved(address: u64) -> bool {
        for region in RESERVED_REGIONS.iter() {
            if address >= region.start && address < region.end {
                return true;
            }
        }
        false
    }
    
    pub fn allocate_frame(&mut self) -> Option<PhysFrame> {
        let entry_count = unsafe { (*self.boot_info).memory_map_len } as usize;
        let map_ptr = unsafe { (*self.boot_info).memory_map_ptr } as *const MemoryMapEntry;

        while self.current_entry_index < entry_count {
            let entry = unsafe { &*map_ptr.add(self.current_entry_index) };

            // E820 Type 1 is "Usable RAM"
            if entry.region_type != 1 {
                self.current_entry_index += 1;
                self.next_free_address = 0;
                continue;
            }

            // Initialize our search address for this region
            if self.next_free_address == 0 {
                self.next_free_address = entry.base;
            }

            // Align address up to the next 4 KiB boundary
            let mut addr = self.next_free_address;
            let remainder = addr % PAGE_SIZE;
            if remainder != 0 {
                addr += PAGE_SIZE - remainder;
            }

            let region_end = entry.base + entry.length;

            // Search for the next unreserved frame in this region
            while addr + PAGE_SIZE <= region_end {
                let candidate = addr;
                addr += PAGE_SIZE;
                self.next_free_address = addr; // Save state for next call

                if !Self::is_reserved(candidate) {
                    return Some(PhysFrame { start_address: candidate });
                }
            }

            // Region exhausted, move to the next E820 entry
            self.current_entry_index += 1;
            self.next_free_address = 0;
        }

        None // Out of physical memory
    }

    
}

fn print_hex_64(val: u64, offset: isize, vga: *mut u8) {
        let hex_chars = b"0123456789ABCDEF";
        for i in 0..16 {
            let nibble = (val >> (60 - i * 4)) & 0x0F;
            unsafe {
                *vga.offset(offset + i * 2) = hex_chars[nibble as usize];
                *vga.offset(offset + i * 2 + 1) = 0x0B; // Light Cyan text
            }
        }
}