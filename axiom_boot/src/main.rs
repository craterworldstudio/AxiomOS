#![no_std]
#![no_main]

mod uefi;
mod gop;

use core::panic::PanicInfo;
use uefi::*;
use gop::*;

// The kernel entry address in physical memory
const KERNEL_LOAD_ADDR: u64 = 0x100000;
const KERNEL_MAX_PAGES: usize = 128; // 512 KiB buffer

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct MemoryMapEntry {
    pub base: u64,
    pub length: u64,
    pub region_type: u32,
    pub acpi_extended_attributes: u32,
}

pub const MAX_E820_ENTRIES: usize = 256;
static mut E820_MAP: [MemoryMapEntry; MAX_E820_ENTRIES] = [MemoryMapEntry {
    base: 0,
    length: 0,
    region_type: 0,
    acpi_extended_attributes: 0,
}; MAX_E820_ENTRIES];

#[repr(C)]
#[derive(Clone, Copy, Debug)]
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

static mut BOOT_INFO: BootInfo = BootInfo {
    magic: 0xC0DEB007,
    version: 0x02,
    memory_map_ptr: 0,
    memory_map_len: 0,
    memory_map_entry_size: core::mem::size_of::<MemoryMapEntry>() as u64,
    framebuffer_base: 0,
    framebuffer_size: 0,
    horizontal_resolution: 0,
    vertical_resolution: 0,
    pixels_per_scan_line: 0,
};

// UTF-16 print helper for UEFI ConOut
unsafe fn print_uefi(con_out: *mut EfiSimpleTextOutputProtocol, s: &str) {
    if con_out.is_null() { return; }
    let mut buf = [0u16; 128];
    let mut i = 0;
    for b in s.bytes() {
        if b == b'\n' {
            if i + 2 < buf.len() {
                buf[i] = '\r' as u16;
                buf[i + 1] = '\n' as u16;
                i += 2;
            }
        } else if i + 1 < buf.len() {
            buf[i] = b as u16;
            i += 1;
        }
    }
    buf[i] = 0;
    let _ = ((*con_out).output_string)(con_out, buf.as_ptr());
}

#[no_mangle]
pub extern "efiapi" fn efi_main(image_handle: EfiHandle, system_table: *mut EfiSystemTable) -> EfiStatus {
    unsafe {
        let st = &*system_table;
        let bs = &*st.boot_services;
        let con_out = st.con_out;

        print_uefi(con_out, "[AXIOM BOOT] UEFI Sovereign Substrate Initializing...\n");

        // 1. Initialize GOP TrueColor graphics
        let gop_res = init_gop(bs);
        let gop_info = match gop_res {
            Ok(info) => {
                print_uefi(con_out, "[AXIOM BOOT] GOP Graphics Initialized.\n");
                info
            },
            Err(_) => {
                print_uefi(con_out, "[WARN] GOP init failed, proceeding without high-res framebuffer.\n");
                GopFramebufferInfo { base: 0, size: 0, width: 0, height: 0, pitch: 0 }
            }
        };

        // 2. Locate Simple File System to load axiom_kernel.bin
        let mut loaded_image_ptr: *mut core::ffi::c_void = core::ptr::null_mut();
        let status = (bs.handle_protocol)(
            image_handle,
            &raw const EFI_LOADED_IMAGE_PROTOCOL_GUID,
            &raw mut loaded_image_ptr,
        );
        if status != EFI_SUCCESS || loaded_image_ptr.is_null() {
            print_uefi(con_out, "[ERROR] Failed to obtain LoadedImageProtocol\n");
            return status;
        }

        let loaded_image = &*(loaded_image_ptr as *mut EfiLoadedImageProtocol);
        let mut fs_ptr: *mut core::ffi::c_void = core::ptr::null_mut();
        let status = (bs.handle_protocol)(
            loaded_image.device_handle,
            &raw const EFI_SIMPLE_FILE_SYSTEM_PROTOCOL_GUID,
            &raw mut fs_ptr,
        );
        if status != EFI_SUCCESS || fs_ptr.is_null() {
            print_uefi(con_out, "[ERROR] Failed to obtain SimpleFileSystemProtocol\n");
            return status;
        }

        let fs = &*(fs_ptr as *mut EfiSimpleFileSystemProtocol);
        let mut root_dir_ptr: *mut EfiFileProtocol = core::ptr::null_mut();
        let status = (fs.open_volume)(fs_ptr as *mut EfiSimpleFileSystemProtocol, &raw mut root_dir_ptr);
        if status != EFI_SUCCESS || root_dir_ptr.is_null() {
            print_uefi(con_out, "[ERROR] Failed to open root volume\n");
            return status;
        }

        let root_dir = &mut *root_dir_ptr;

        // Open axiom_kernel.bin
        // UTF-16 "axiom_kernel.bin\0"
        let kernel_name: [u16; 17] = [
            'a' as u16, 'x' as u16, 'i' as u16, 'o' as u16, 'm' as u16, '_' as u16,
            'k' as u16, 'e' as u16, 'r' as u16, 'n' as u16, 'e' as u16, 'l' as u16,
            '.' as u16, 'b' as u16, 'i' as u16, 'n' as u16, 0,
        ];

        let mut kernel_file_ptr: *mut EfiFileProtocol = core::ptr::null_mut();
        let status = (root_dir.open)(
            root_dir,
            &raw mut kernel_file_ptr,
            kernel_name.as_ptr(),
            EFI_FILE_MODE_READ,
            0,
        );
        if status != EFI_SUCCESS || kernel_file_ptr.is_null() {
            print_uefi(con_out, "[ERROR] axiom_kernel.bin not found on ESP volume\n");
            return status;
        }

        let kernel_file = &mut *kernel_file_ptr;

        // 3. Allocate physical memory for the kernel at 0x100000
        let mut kernel_dest: u64 = KERNEL_LOAD_ADDR;
        let status = (bs.allocate_pages)(
            EfiAllocateType::AllocateAddress,
            EfiMemoryType::EfiLoaderCode,
            KERNEL_MAX_PAGES,
            &raw mut kernel_dest,
        );
        if status != EFI_SUCCESS && status != 0 {
            // If already allocated or unavailable at exact address, report warning but proceed if accessible
            print_uefi(con_out, "[WARN] Kernel page allocation returned non-zero\n");
        }

        // Read kernel binary into 0x100000
        let mut kernel_read_size = KERNEL_MAX_PAGES * 4096;
        let status = (kernel_file.read)(
            kernel_file,
            &raw mut kernel_read_size,
            KERNEL_LOAD_ADDR as *mut core::ffi::c_void,
        );
        if status != EFI_SUCCESS {
            print_uefi(con_out, "[ERROR] Failed to read axiom_kernel.bin into memory\n");
            return status;
        }

        let _ = (kernel_file.close)(kernel_file);
        let _ = (root_dir.close)(root_dir);

        print_uefi(con_out, "[AXIOM BOOT] Kernel placed at 0x100000. Transitioning to Long Mode Core...\n");

        // 4. Capture UEFI Memory Map & Convert to Axiom E820 Format
        const MEM_MAP_BUF_SIZE: usize = 16384;
        let mut mem_map_buf = [0u8; MEM_MAP_BUF_SIZE];
        let mut map_size = MEM_MAP_BUF_SIZE;
        let mut map_key: usize = 0;
        let mut desc_size: usize = 0;
        let mut desc_version: u32 = 0;

        let status = (bs.get_memory_map)(
            &raw mut map_size,
            mem_map_buf.as_mut_ptr() as *mut EfiMemoryDescriptor,
            &raw mut map_key,
            &raw mut desc_size,
            &raw mut desc_version,
        );

        if status != EFI_SUCCESS {
            print_uefi(con_out, "[ERROR] Failed to get memory map\n");
            return status;
        }

        // Translate UEFI memory descriptors to Axiom MemoryMapEntry
        let num_descriptors = map_size / desc_size;
        let mut e820_count = 0;

        for i in 0..num_descriptors {
            if e820_count >= MAX_E820_ENTRIES { break; }
            let desc_ptr = mem_map_buf.as_ptr().add(i * desc_size) as *const EfiMemoryDescriptor;
            let desc = &*desc_ptr;

            let is_usable = desc.memory_type == EfiMemoryType::EfiConventionalMemory as u32
                || desc.memory_type == EfiMemoryType::EfiLoaderCode as u32
                || desc.memory_type == EfiMemoryType::EfiLoaderData as u32
                || desc.memory_type == EfiMemoryType::EfiBootServicesCode as u32
                || desc.memory_type == EfiMemoryType::EfiBootServicesData as u32;

            E820_MAP[e820_count] = MemoryMapEntry {
                base: desc.physical_start,
                length: desc.number_of_pages * 4096,
                region_type: if is_usable { 1 } else { 2 },
                acpi_extended_attributes: 1,
            };
            e820_count += 1;
        }

        // Fill BootInfo
        BOOT_INFO.memory_map_ptr = core::ptr::addr_of!(E820_MAP) as u64;
        BOOT_INFO.memory_map_len = e820_count as u64;
        BOOT_INFO.framebuffer_base = gop_info.base;
        BOOT_INFO.framebuffer_size = gop_info.size;
        BOOT_INFO.horizontal_resolution = gop_info.width;
        BOOT_INFO.vertical_resolution = gop_info.height;
        BOOT_INFO.pixels_per_scan_line = gop_info.pitch;

        // 5. Transfer BootInfo and E820 map to dedicated physical location 0x6000
        const BOOT_INFO_PHYS: u64 = 0x6000;
        const E820_MAP_PHYS: u64 = 0x6020;

        core::ptr::copy_nonoverlapping(
            (&raw const E820_MAP) as *const MemoryMapEntry,
            E820_MAP_PHYS as *mut MemoryMapEntry,
            e820_count,
        );

        BOOT_INFO.memory_map_ptr = E820_MAP_PHYS;
        BOOT_INFO.memory_map_len = e820_count as u64;
        core::ptr::write(BOOT_INFO_PHYS as *mut BootInfo, BOOT_INFO);

        // 6. Exit Boot Services — Sovereign takeover of the CPU
        map_size = MEM_MAP_BUF_SIZE;
        let _ = (bs.get_memory_map)(
            &raw mut map_size,
            mem_map_buf.as_mut_ptr() as *mut EfiMemoryDescriptor,
            &raw mut map_key,
            &raw mut desc_size,
            &raw mut desc_version,
        );

        let exit_status = (bs.exit_boot_services)(image_handle, map_key);
        if exit_status != EFI_SUCCESS {
            map_size = MEM_MAP_BUF_SIZE;
            let _ = (bs.get_memory_map)(
                &raw mut map_size,
                mem_map_buf.as_mut_ptr() as *mut EfiMemoryDescriptor,
                &raw mut map_key,
                &raw mut desc_size,
                &raw mut desc_version,
            );
            let retry_status = (bs.exit_boot_services)(image_handle, map_key);
            if retry_status != EFI_SUCCESS {
                return retry_status;
            }
        }

        // 7. JUMP TO AXIOM KERNEL ENTRY POINT
        // Set pristine stack pointer to 0x90000 and jump with RDI = 0x6000
        core::arch::asm!(
            "mov rsp, 0x90000",
            "jmp {0}",
            in(reg) KERNEL_LOAD_ADDR,
            in("rdi") BOOT_INFO_PHYS,
            options(noreturn)
        );
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {
        unsafe {
            core::arch::asm!("cli; hlt");
        }
    }
}
