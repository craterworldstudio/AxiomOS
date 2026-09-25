// ==========================================
// Memory Management Subsystem
// ==========================================

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
    //ReservedRegion {
    //    start: 0x00000000,
    //    end: 0x00200000,
    //},

    // 1. Lower Memory Quarantine (IVT, BDA, Bootloader, Page Tables, Stack)
    ReservedRegion {
        start: 0x00000000,
        end: 0x00100000,
    },
    // 2. VGA Hardware Buffer
    ReservedRegion {
        start: 0x000B8000,
        end: 0x000B9000,
    },
    // 3. Axiom Kernel Image
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

        //address < 0x00200000

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

            while addr + PAGE_SIZE <= region_end {
                let candidate = addr;
                addr += PAGE_SIZE;
                self.next_free_address = addr; // Save state for next call

                if !Self::is_reserved(candidate) {
                    return Some(PhysFrame { start_address: candidate });
                }
            }

            self.current_entry_index += 1;
            self.next_free_address = 0;
        }

        None
    }

    /// Scans the E820 map to find the highest physical address on the system.
    pub fn discover_memory_ceiling(&self) -> u64 {
        let entry_count = unsafe { (*self.boot_info).memory_map_len } as usize;
        let map_ptr = unsafe { (*self.boot_info).memory_map_ptr } as *const MemoryMapEntry;
        
        let mut highest_usable_address = 0;

        for i in 0..entry_count {
            let entry = unsafe { &*map_ptr.add(i) };
            
            // Only evaluate entries marked as Usable RAM (Type 1)
            if entry.region_type == 1 {
                let end_address = entry.base + entry.length;
                if end_address > highest_usable_address {
                    highest_usable_address = end_address;
                }
            }
        }
        
        highest_usable_address
    }
}

// Pre-allocate 128 KiB in the mapped .bss section. 
// This is enough to track exactly 4 Gigabytes of physical RAM.
const BITMAP_MAX_BYTES: usize = 131072;
static mut BITMAP_STORAGE: [u8; BITMAP_MAX_BYTES] = [0; BITMAP_MAX_BYTES];

pub struct BitmapAllocator {
    bitmap: &'static mut [u8],
    pub total_frames: usize,
}

impl BitmapAllocator {
    pub unsafe fn bootstrap(boot_allocator: FrameAllocator) -> Self {
        let max_addr = boot_allocator.discover_memory_ceiling();
        let total_frames = (max_addr / PAGE_SIZE) as usize;
        let bitmap_bytes = ((total_frames + 7) / 8) as usize;

        if bitmap_bytes > BITMAP_MAX_BYTES {
            panic!("Fatal: Physical memory exceeds 4GB bitmap storage capacity!");
        }

        let bitmap_ptr = core::ptr::addr_of_mut!(BITMAP_STORAGE) as *mut u8;
        
        // 1. Start with absolute paranoia: EVERYTHING is reserved (0xFF)
        for i in 0..bitmap_bytes {
            core::ptr::write_volatile(bitmap_ptr.add(i), 0xFF);
        }

        let bitmap_slice = core::slice::from_raw_parts_mut(bitmap_ptr, bitmap_bytes);
        
        let mut allocator = Self {
            bitmap: bitmap_slice,
            total_frames,
        };

        // 2. Only carve out (free) regions that the BIOS explicitly certifies as Usable RAM (Type 1)
        let entry_count = (*boot_allocator.boot_info).memory_map_len as usize;
        let map_ptr = (*boot_allocator.boot_info).memory_map_ptr as *const MemoryMapEntry;

        for i in 0..entry_count {
            let entry = &*map_ptr.add(i);
            
            if entry.region_type == 1 {
                let start_frame = (entry.base / PAGE_SIZE) as usize;
                let end_frame = ((entry.base + entry.length) / PAGE_SIZE) as usize;
                
                for frame in start_frame..end_frame {
                    // SAFETY GUARD: Do not let the BIOS free our lower memory quarantine 
                    // or our kernel image (Frames 0 to 511, covering 0x0 to 0x200000)
                    if frame >= 512 {
                        allocator.free_frame(frame);
                    }
                }
            }
        }

        allocator
    }
    
    pub fn read_byte(&self, byte_index: usize) -> u8 {
        if byte_index < self.bitmap.len() {
            self.bitmap[byte_index]
        } else {
            0
        }
    }

    /// Marks a specific physical frame as Free (0)
    fn free_frame(&mut self, frame_index: usize) {
        if frame_index < self.total_frames {
            let byte_index = frame_index / 8;
            let bit_index = frame_index % 8;
            self.bitmap[byte_index] &= !(1 << bit_index);
        }
    }

    /// Marks a specific physical frame as Reserved/Allocated (1)
    fn reserve_frame(&mut self, frame_index: usize) {
        if frame_index < self.total_frames {
            let byte_index = frame_index / 8;
            let bit_index = frame_index % 8;
            self.bitmap[byte_index] |= 1 << bit_index;
        }
    }

    /// Checks if a frame is currently Free
    pub fn is_frame_free(&self, frame_index: usize) -> bool {
        if frame_index >= self.total_frames {
            return false;
        }
        let byte_index = frame_index / 8;
        let bit_index = frame_index % 8;
        (self.bitmap[byte_index] & (1 << bit_index)) == 0
    }

    /// Finds the first available physical frame, marks it as reserved, and returns it.
    pub fn allocate_frame(&mut self) -> Option<PhysFrame> {
        for byte_index in 0..self.bitmap.len() {
            // Fast path: If the byte is 0xFF, all 8 frames are already taken. Skip it.
            if self.bitmap[byte_index] != 0xFF {
                // Find the exact bit (frame) that is free
                for bit_index in 0..8 {
                    if (self.bitmap[byte_index] & (1 << bit_index)) == 0 {
                        let frame_index = byte_index * 8 + bit_index;
                        
                        if frame_index < self.total_frames {
                            self.reserve_frame(frame_index);
                            return Some(PhysFrame {
                                start_address: (frame_index as u64) * PAGE_SIZE,
                            });
                        }
                    }
                }
            }
        }
        None // Out of physical memory
    }

    /// Returns a physical frame back to the available pool.
    pub fn deallocate_frame(&mut self, frame: PhysFrame) {
        let frame_index = (frame.start_address / PAGE_SIZE) as usize;
        self.free_frame(frame_index);
    }
}