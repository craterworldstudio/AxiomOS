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

const BITMAP_MAX_BYTES: usize = 131072;
static mut BITMAP_STORAGE: [u8; BITMAP_MAX_BYTES] = [0; BITMAP_MAX_BYTES];

pub struct BitmapAllocator {
    bitmap: &'static mut [u8],
    pub total_frames: usize,
}

impl BitmapAllocator {
    pub unsafe fn bootstrap(boot_info: *const BootInfo, kernel_start: u64, kernel_end: u64) -> Self {
        let entry_count = (*boot_info).memory_map_len as usize;
        let map_ptr = (*boot_info).memory_map_ptr as *const MemoryMapEntry;

        // 1. Discover the absolute physical ceiling
        let mut highest_usable_address = 0;
        for i in 0..entry_count {
            let entry = &*map_ptr.add(i);
            if entry.region_type == 1 {
                let end_address = entry.base + entry.length;
                if end_address > highest_usable_address {
                    highest_usable_address = end_address;
                }
            }
        }

        let total_frames = (highest_usable_address / PAGE_SIZE) as usize;
        let bitmap_bytes = ((total_frames + 7) / 8) as usize;

        if bitmap_bytes > BITMAP_MAX_BYTES {
            panic!("Fatal: Physical memory exceeds 4GB bitmap storage capacity!");
        }

        let bitmap_ptr = core::ptr::addr_of_mut!(BITMAP_STORAGE) as *mut u8;
        
        // Default State: Paranoia. Every single frame is RESERVED (0xFF)
        for i in 0..bitmap_bytes {
            core::ptr::write_volatile(bitmap_ptr.add(i), 0xFF);
        }

        let bitmap_slice = core::slice::from_raw_parts_mut(bitmap_ptr, bitmap_bytes);
        let mut allocator = Self {
            bitmap: bitmap_slice,
            total_frames,
        };

        // 2. Iterate E820 Map: Free ONLY the frames firmware guarantees as Usable RAM
        for i in 0..entry_count {
            let entry = &*map_ptr.add(i);
            if entry.region_type == 1 {
                allocator.free_region(entry.base, entry.base + entry.length);
            }
        }

        // 3. Axiom Boot-Time Reservations (Overlaying the Free RAM)
        
        // IVT, BDA, and basic BIOS structures
        allocator.reserve_region(0x0000, 0x1000);
        // BootInfo & E820 Map
        allocator.reserve_region(0x6000, 0x7000);
        // Stage 1 & Stage 2 Bootloader
        allocator.reserve_region(0x7C00, 0x8E00);
        // Initial Identity Page Tables (PML4, PDPT, PD)
        allocator.reserve_region(0x9000, 0xC000);
        // VGA Hardware Buffer
        allocator.reserve_region(0xB8000, 0xB9000);
        // The Axiom Kernel Image (dynamically sourced from linker!)
        allocator.reserve_region(kernel_start, kernel_end);

        // Note: We deliberately DO NOT reserve 0x10000 (The Stage 2 temporary 
        // kernel load buffer). It is now safely reclaimed as free memory!

        allocator
    }

    /// Marks a strictly enclosed physical region as FREE.
    pub fn free_region(&mut self, physical_start: u64, physical_end: u64) {
        let mut start_frame = (physical_start / PAGE_SIZE) as usize;
        if physical_start % PAGE_SIZE != 0 {
            start_frame += 1; // Align inward (up)
        }
        let end_frame = (physical_end / PAGE_SIZE) as usize; // Align inward (down)
        
        for frame in start_frame..end_frame {
            self.free_frame(frame);
        }
    }

    /// Marks a potentially overlapping physical region as RESERVED.
    pub fn reserve_region(&mut self, physical_start: u64, physical_end: u64) {
        let start_frame = (physical_start / PAGE_SIZE) as usize; // Align outward (down)
        let end_frame = ((physical_end + PAGE_SIZE - 1) / PAGE_SIZE) as usize; // Align outward (up)
        
        for frame in start_frame..end_frame {
            self.reserve_frame(frame);
        }
    }

    fn free_frame(&mut self, frame_index: usize) {
        if frame_index < self.total_frames {
            let byte_index = frame_index / 8;
            let bit_index = frame_index % 8;
            self.bitmap[byte_index] &= !(1 << bit_index);
        }
    }

    fn reserve_frame(&mut self, frame_index: usize) {
        if frame_index < self.total_frames {
            let byte_index = frame_index / 8;
            let bit_index = frame_index % 8;
            self.bitmap[byte_index] |= 1 << bit_index;
        }
    }

    pub fn is_frame_free(&self, frame_index: usize) -> bool {
        if frame_index >= self.total_frames {
            return false;
        }
        let byte_index = frame_index / 8;
        let bit_index = frame_index % 8;
        (self.bitmap[byte_index] & (1 << bit_index)) == 0
    }

    pub fn allocate_frame(&mut self) -> Option<PhysFrame> {
        for byte_index in 0..self.bitmap.len() {
            if self.bitmap[byte_index] != 0xFF {
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
        None 
    }

    pub fn deallocate_frame(&mut self, frame: PhysFrame) {
        let frame_index = (frame.start_address / PAGE_SIZE) as usize;
        self.free_frame(frame_index);
    }

    /// Finds a contiguous sequence of free physical frames, marks them as RESERVED,
    /// and returns the starting physical address. This is how Untyped authority
    /// officially claims physical ownership from the ledger.
    pub fn allocate_contiguous(&mut self, num_frames: usize) -> Option<u64> {
        if num_frames == 0 {
            return None;
        }

        let mut contiguous_count = 0;
        let mut start_frame = 0;

        for frame_index in 0..self.total_frames {
            if self.is_frame_free(frame_index) {
                if contiguous_count == 0 {
                    start_frame = frame_index;
                }
                contiguous_count += 1;

                if contiguous_count == num_frames {
                    // Claim the entire contiguous region from the ledger
                    for i in 0..num_frames {
                        self.reserve_frame(start_frame + i);
                    }
                    return Some((start_frame as u64) * PAGE_SIZE);
                }
            } else {
                // We hit a reserved frame, reset the contiguous counter
                contiguous_count = 0; 
            }
        }
        
        None
    }
}