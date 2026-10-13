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
    pub framebuffer_base: u64,
    pub framebuffer_size: u64,
    pub horizontal_resolution: u32,
    pub vertical_resolution: u32,
    pub pixels_per_scan_line: u32,
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
        // Strictly reserve the entire low 1 MiB: IVT, BDA, EBDA, bootloader, 
        // BootInfo (0x6000), page tables, VGA buffer, and bootstrap stack (0x90000).
        allocator.reserve_region(0x0000, 0x100000);
        // The Axiom Kernel Image (dynamically sourced from linker!)
        allocator.reserve_region(kernel_start, kernel_end);

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageFlags(pub u64);

impl PageFlags {
    pub const NONE: PageFlags       = PageFlags(0);
    pub const PRESENT: PageFlags    = PageFlags(1 << 0);
    pub const WRITABLE: PageFlags   = PageFlags(1 << 1);
    pub const USER: PageFlags       = PageFlags(1 << 2);
    pub const HUGE_2MB: PageFlags   = PageFlags(1 << 7);

    #[inline(always)]
    pub fn bits(&self) -> u64 { self.0 }
    #[inline(always)]
    pub fn contains(&self, other: PageFlags) -> bool { (self.0 & other.0) == other.0 }
}

impl core::ops::BitOr for PageFlags {
    type Output = Self;
    #[inline(always)]
    fn bitor(self, rhs: Self) -> Self {
        PageFlags(self.0 | rhs.0)
    }
}

impl core::ops::BitOrAssign for PageFlags {
    #[inline(always)]
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapError {
    UnalignedVirtualAddress,
    UnalignedPhysicalAddress,
    NonCanonicalAddress,
    ConflictingMapping,
    AllocationFailed,
}

pub static mut BOOT_CR3: u64 = 0;

pub unsafe fn init_boot_cr3() {
    let cr3: u64;
    core::arch::asm!("mov {}, cr3", out(reg) cr3, options(nomem, nostack));
    BOOT_CR3 = cr3;
}

#[repr(C, align(4096))]
pub struct VSpace {
    pub pml4_phys: u64,
}

impl VSpace {
    /// Constructs a private user address space (PML4).
    /// Clones supervisor-only kernel identity mappings (including framebuffer and low 2 MiB)
    /// while keeping kernel memory strictly U/S=0 (Supervisor only).
    pub fn create(
        pml4_phys: u64,
        allocator: &mut BitmapAllocator,
    ) -> Result<Self, MapError> {
        let pml4 = pml4_phys as *mut u64;
        
        unsafe {
            // 1. Zero all 512 entries in the root PML4
            for i in 0..512 {
                core::ptr::write_volatile(pml4.add(i), 0);
            }

            // 2. Clone supervisor-only high mappings from BOOT_CR3 (framebuffer, ACPI, etc.)
            let boot_cr3 = BOOT_CR3;
            if boot_cr3 != 0 {
                let boot_pml4 = (boot_cr3 & !0xFFF) as *const u64;
                for i in 1..512 {
                    let boot_val = core::ptr::read_volatile(boot_pml4.add(i));
                    if boot_val & PageFlags::PRESENT.bits() != 0 {
                        // Clear bit 2 (U/S = 0): Strictly supervisor-only
                        core::ptr::write_volatile(pml4.add(i), boot_val & !4);
                    }
                }
            }

            // 3. Construct private PDPT for PML4[0] (low 512 GiB)
            let pdpt_frame = allocator.allocate_frame().ok_or(MapError::AllocationFailed)?;
            let pdpt_phys = pdpt_frame.start_address;
            let pdpt = pdpt_phys as *mut u64;
            for i in 0..512 {
                core::ptr::write_volatile(pdpt.add(i), 0);
            }

            // Clone boot_pdpt[1..512] as supervisor-only so MMIO and GOP framebuffer (e.g. at 2 GiB)
            // remain accessible to kernel execution under this VSpace
            if boot_cr3 != 0 {
                let boot_pml4 = (boot_cr3 & !0xFFF) as *const u64;
                let boot_pml4_0 = core::ptr::read_volatile(boot_pml4);
                if boot_pml4_0 & PageFlags::PRESENT.bits() != 0 {
                    let boot_pdpt = (boot_pml4_0 & 0x000F_FFFF_FFFF_F000) as *const u64;
                    for i in 1..512 {
                        let boot_val = core::ptr::read_volatile(boot_pdpt.add(i));
                        if boot_val & PageFlags::PRESENT.bits() != 0 {
                            // Clear bit 2 (U/S = 0): Strictly supervisor-only
                            core::ptr::write_volatile(pdpt.add(i), boot_val & !4);
                        }
                    }
                }
            }

            // Link PML4[0] -> PDPT with USER permissions so user pages underneath are accessible
            let ancestor_flags = PageFlags::PRESENT.bits() | PageFlags::WRITABLE.bits() | PageFlags::USER.bits();
            core::ptr::write_volatile(pml4, pdpt_phys | ancestor_flags);

            // 4. Construct private PD for PDPT[0] (low 1 GiB)
            let pd_frame = allocator.allocate_frame().ok_or(MapError::AllocationFailed)?;
            let pd_phys = pd_frame.start_address;
            let pd = pd_phys as *mut u64;
            for i in 0..512 {
                core::ptr::write_volatile(pd.add(i), 0);
            }
            // Link PDPT[0] -> PD with USER permissions
            core::ptr::write_volatile(pdpt, pd_phys | ancestor_flags);

            // 5. Map Kernel Memory in PD[0] (0x000000..0x1FFFFF, first 2 MiB)
            // Contains kernel code, data, BSS, TSS, GDT, IDT, RSP0 stack, BootInfo.
            // MUST BE SUPERVISOR ONLY (U/S = 0):
            let kernel_pd_flags = PageFlags::PRESENT.bits() | PageFlags::WRITABLE.bits() | PageFlags::HUGE_2MB.bits();
            core::ptr::write_volatile(pd, 0x0000_0000 | kernel_pd_flags);
        }

        Ok(Self { pml4_phys })
    }

    /// Maps a 4 KiB virtual page to a physical frame within this VSpace.
    pub fn map_page(
        &self,
        virt_addr: u64,
        phys_addr: u64,
        flags: PageFlags,
        allocator: &mut BitmapAllocator,
    ) -> Result<(), MapError> {
        map_page(self.pml4_phys, virt_addr, phys_addr, flags, allocator)
    }
}

pub fn map_page(
    pml4_phys: u64,
    virt_addr: u64,
    phys_addr: u64,
    flags: PageFlags,
    allocator: &mut BitmapAllocator,
) -> Result<(), MapError> {
    if virt_addr % PAGE_SIZE != 0 {
        return Err(MapError::UnalignedVirtualAddress);
    }
    if phys_addr % PAGE_SIZE != 0 {
        return Err(MapError::UnalignedPhysicalAddress);
    }
    if (virt_addr >= 0x0000_8000_0000_0000) && (virt_addr < 0xFFFF_8000_0000_0000) {
        return Err(MapError::NonCanonicalAddress);
    }

    let pml4_idx = ((virt_addr >> 39) & 0x1FF) as usize;
    let pdpt_idx = ((virt_addr >> 30) & 0x1FF) as usize;
    let pd_idx   = ((virt_addr >> 21) & 0x1FF) as usize;
    let pt_idx   = ((virt_addr >> 12) & 0x1FF) as usize;

    let pml4 = pml4_phys as *mut u64;

    let ancestor_flags = PageFlags::PRESENT.bits() | PageFlags::WRITABLE.bits()
        | if flags.contains(PageFlags::USER) { PageFlags::USER.bits() } else { 0 };

    // 1. PML4 -> PDPT
    let pml4_entry = unsafe { core::ptr::read_volatile(pml4.add(pml4_idx)) };
    let pdpt_phys = if pml4_entry & PageFlags::PRESENT.bits() != 0 {
        unsafe { core::ptr::write_volatile(pml4.add(pml4_idx), pml4_entry | ancestor_flags); }
        pml4_entry & 0x000F_FFFF_FFFF_F000
    } else {
        let frame = allocator.allocate_frame().ok_or(MapError::AllocationFailed)?;
        let addr = frame.start_address;
        unsafe {
            let ptr = addr as *mut u64;
            for i in 0..512 { core::ptr::write_volatile(ptr.add(i), 0); }
            core::ptr::write_volatile(pml4.add(pml4_idx), addr | ancestor_flags);
        }
        addr
    };

    // 2. PDPT -> PD
    let pdpt = pdpt_phys as *mut u64;
    let pdpt_entry = unsafe { core::ptr::read_volatile(pdpt.add(pdpt_idx)) };
    let pd_phys = if pdpt_entry & PageFlags::PRESENT.bits() != 0 {
        unsafe { core::ptr::write_volatile(pdpt.add(pdpt_idx), pdpt_entry | ancestor_flags); }
        pdpt_entry & 0x000F_FFFF_FFFF_F000
    } else {
        let frame = allocator.allocate_frame().ok_or(MapError::AllocationFailed)?;
        let addr = frame.start_address;
        unsafe {
            let ptr = addr as *mut u64;
            for i in 0..512 { core::ptr::write_volatile(ptr.add(i), 0); }
            core::ptr::write_volatile(pdpt.add(pdpt_idx), addr | ancestor_flags);
        }
        addr
    };

    // 3. PD -> PT
    let pd = pd_phys as *mut u64;
    let pd_entry = unsafe { core::ptr::read_volatile(pd.add(pd_idx)) };
    if pd_entry & PageFlags::HUGE_2MB.bits() != 0 {
        return Err(MapError::ConflictingMapping);
    }
    let pt_phys = if pd_entry & PageFlags::PRESENT.bits() != 0 {
        unsafe { core::ptr::write_volatile(pd.add(pd_idx), pd_entry | ancestor_flags); }
        pd_entry & 0x000F_FFFF_FFFF_F000
    } else {
        let frame = allocator.allocate_frame().ok_or(MapError::AllocationFailed)?;
        let addr = frame.start_address;
        unsafe {
            let ptr = addr as *mut u64;
            for i in 0..512 { core::ptr::write_volatile(ptr.add(i), 0); }
            core::ptr::write_volatile(pd.add(pd_idx), addr | ancestor_flags);
        }
        addr
    };

    // 4. PT -> Leaf
    let pt = pt_phys as *mut u64;
    let current_pt = unsafe { core::ptr::read_volatile(pt.add(pt_idx)) };
    if current_pt & PageFlags::PRESENT.bits() != 0 {
        if (current_pt & 0x000F_FFFF_FFFF_F000) != phys_addr {
            return Err(MapError::ConflictingMapping);
        }
    }
    unsafe {
        core::ptr::write_volatile(pt.add(pt_idx), phys_addr | flags.bits());
    }

    Ok(())
}