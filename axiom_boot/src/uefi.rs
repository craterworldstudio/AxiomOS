#![allow(dead_code)]

pub type EfiStatus = usize;
pub type EfiHandle = *mut core::ffi::c_void;

pub const EFI_SUCCESS: EfiStatus = 0;
pub const EFI_LOAD_ERROR: EfiStatus = 1 | (1 << 63);
pub const EFI_INVALID_PARAMETER: EfiStatus = 2 | (1 << 63);
pub const EFI_UNSUPPORTED: EfiStatus = 3 | (1 << 63);
pub const EFI_BUFFER_TOO_SMALL: EfiStatus = 5 | (1 << 63);
pub const EFI_NOT_FOUND: EfiStatus = 14 | (1 << 63);

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct EfiGuid {
    pub data1: u32,
    pub data2: u16,
    pub data3: u16,
    pub data4: [u8; 8],
}

pub static EFI_GRAPHICS_OUTPUT_PROTOCOL_GUID: EfiGuid = EfiGuid {
    data1: 0x9042a9de,
    data2: 0x23dc,
    data3: 0x4a38,
    data4: [0x96, 0xfb, 0x7a, 0xde, 0xd0, 0x80, 0x51, 0x6a],
};

pub static EFI_LOADED_IMAGE_PROTOCOL_GUID: EfiGuid = EfiGuid {
    data1: 0x5b1b31a1,
    data2: 0x9562,
    data3: 0x11d2,
    data4: [0x8e, 0x3f, 0x00, 0xa0, 0xc9, 0x69, 0x72, 0x3b],
};

pub static EFI_SIMPLE_FILE_SYSTEM_PROTOCOL_GUID: EfiGuid = EfiGuid {
    data1: 0x964e5b22,
    data2: 0x6459,
    data3: 0x11d2,
    data4: [0x8e, 0x39, 0x00, 0xa0, 0xc9, 0x69, 0x72, 0x3b],
};

pub static EFI_FILE_INFO_GUID: EfiGuid = EfiGuid {
    data1: 0x09576e92,
    data2: 0x6d3f,
    data3: 0x11d2,
    data4: [0x8e, 0x39, 0x00, 0xa0, 0xc9, 0x69, 0x72, 0x3b],
};

#[repr(C)]
pub struct EfiTableHeader {
    pub signature: u64,
    pub revision: u32,
    pub header_size: u32,
    pub crc32: u32,
    pub reserved: u32,
}

#[repr(C)]
pub struct EfiSimpleTextOutputProtocol {
    pub reset: unsafe extern "efiapi" fn(*mut EfiSimpleTextOutputProtocol, bool) -> EfiStatus,
    pub output_string: unsafe extern "efiapi" fn(*mut EfiSimpleTextOutputProtocol, *const u16) -> EfiStatus,
    pub test_string: *mut core::ffi::c_void,
    pub query_mode: *mut core::ffi::c_void,
    pub set_mode: *mut core::ffi::c_void,
    pub set_attribute: *mut core::ffi::c_void,
    pub clear_screen: unsafe extern "efiapi" fn(*mut EfiSimpleTextOutputProtocol) -> EfiStatus,
    pub set_cursor_position: *mut core::ffi::c_void,
    pub enable_cursor: *mut core::ffi::c_void,
    pub mode: *mut core::ffi::c_void,
}

#[repr(u32)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum EfiAllocateType {
    AllocateAnyPages = 0,
    AllocateMaxAddress = 1,
    AllocateAddress = 2,
    MaxAllocateType = 3,
}

#[repr(u32)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum EfiMemoryType {
    EfiReservedMemoryType = 0,
    EfiLoaderCode = 1,
    EfiLoaderData = 2,
    EfiBootServicesCode = 3,
    EfiBootServicesData = 4,
    EfiRuntimeServicesCode = 5,
    EfiRuntimeServicesData = 6,
    EfiConventionalMemory = 7,
    EfiUnusableMemory = 8,
    EfiACPIReclaimMemory = 9,
    EfiACPIMemoryNVS = 10,
    EfiMemoryMappedIO = 11,
    EfiMemoryMappedIOPortSpace = 12,
    EfiPalCode = 13,
    EfiPersistentMemory = 14,
    EfiMaxMemoryType = 15,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct EfiMemoryDescriptor {
    pub memory_type: u32,
    pub pad: u32,
    pub physical_start: u64,
    pub virtual_start: u64,
    pub number_of_pages: u64,
    pub attribute: u64,
}

#[repr(C)]
pub struct EfiBootServices {
    pub hdr: EfiTableHeader,

    // Task Priority Services
    pub raise_tpl: *mut core::ffi::c_void,
    pub restore_tpl: *mut core::ffi::c_void,

    // Memory Services
    pub allocate_pages: unsafe extern "efiapi" fn(EfiAllocateType, EfiMemoryType, usize, *mut u64) -> EfiStatus,
    pub free_pages: unsafe extern "efiapi" fn(u64, usize) -> EfiStatus,
    pub get_memory_map: unsafe extern "efiapi" fn(*mut usize, *mut EfiMemoryDescriptor, *mut usize, *mut usize, *mut u32) -> EfiStatus,
    pub allocate_pool: unsafe extern "efiapi" fn(EfiMemoryType, usize, *mut *mut core::ffi::c_void) -> EfiStatus,
    pub free_pool: unsafe extern "efiapi" fn(*mut core::ffi::c_void) -> EfiStatus,

    // Event & Timer Services
    pub create_event: *mut core::ffi::c_void,
    pub set_timer: *mut core::ffi::c_void,
    pub wait_for_event: *mut core::ffi::c_void,
    pub signal_event: *mut core::ffi::c_void,
    pub close_event: *mut core::ffi::c_void,
    pub check_event: *mut core::ffi::c_void,

    // Protocol Handler Services
    pub install_protocol_interface: *mut core::ffi::c_void,
    pub reinstall_protocol_interface: *mut core::ffi::c_void,
    pub uninstall_protocol_interface: *mut core::ffi::c_void,
    pub handle_protocol: unsafe extern "efiapi" fn(EfiHandle, *const EfiGuid, *mut *mut core::ffi::c_void) -> EfiStatus,
    pub reserved: *mut core::ffi::c_void,
    pub register_protocol_notify: *mut core::ffi::c_void,
    pub locate_handle: *mut core::ffi::c_void,
    pub locate_device_path: *mut core::ffi::c_void,
    pub install_configuration_table: *mut core::ffi::c_void,

    // Image Services
    pub load_image: *mut core::ffi::c_void,
    pub start_image: *mut core::ffi::c_void,
    pub exit: *mut core::ffi::c_void,
    pub unload_image: *mut core::ffi::c_void,
    pub exit_boot_services: unsafe extern "efiapi" fn(EfiHandle, usize) -> EfiStatus,

    // Miscellaneous Services
    pub get_next_monotonic_count: *mut core::ffi::c_void,
    pub stall: unsafe extern "efiapi" fn(usize) -> EfiStatus,
    pub set_watchdog_timer: *mut core::ffi::c_void,

    // DriverSupport Services
    pub connect_controller: *mut core::ffi::c_void,
    pub disconnect_controller: *mut core::ffi::c_void,

    // Open and Close Protocol Services
    pub open_protocol: *mut core::ffi::c_void,
    pub close_protocol: *mut core::ffi::c_void,
    pub open_protocol_information: *mut core::ffi::c_void,

    // Library Services
    pub protocols_per_handle: *mut core::ffi::c_void,
    pub locate_handle_buffer: *mut core::ffi::c_void,
    pub locate_protocol: unsafe extern "efiapi" fn(*const EfiGuid, *mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> EfiStatus,
}

#[repr(C)]
pub struct EfiSystemTable {
    pub hdr: EfiTableHeader,
    pub firmware_vendor: *const u16,
    pub firmware_revision: u32,
    pub console_in_handle: EfiHandle,
    pub con_in: *mut core::ffi::c_void,
    pub console_out_handle: EfiHandle,
    pub con_out: *mut EfiSimpleTextOutputProtocol,
    pub standard_error_handle: EfiHandle,
    pub std_err: *mut EfiSimpleTextOutputProtocol,
    pub runtime_services: *mut core::ffi::c_void,
    pub boot_services: *mut EfiBootServices,
    pub number_of_table_entries: usize,
    pub configuration_table: *mut core::ffi::c_void,
}

// --- GOP Protocol ---

#[repr(C)]
pub struct EfiGraphicsOutputModeInformation {
    pub version: u32,
    pub horizontal_resolution: u32,
    pub vertical_resolution: u32,
    pub pixel_format: u32,
    pub pixel_information: [u32; 4],
    pub pixels_per_scan_line: u32,
}

#[repr(C)]
pub struct EfiGraphicsOutputProtocolMode {
    pub max_mode: u32,
    pub mode: u32,
    pub info: *mut EfiGraphicsOutputModeInformation,
    pub size_of_info: usize,
    pub frame_buffer_base: u64,
    pub frame_buffer_size: usize,
}

#[repr(C)]
pub struct EfiGraphicsOutputProtocol {
    pub query_mode: unsafe extern "efiapi" fn(*mut EfiGraphicsOutputProtocol, u32, *mut usize, *mut *mut EfiGraphicsOutputModeInformation) -> EfiStatus,
    pub set_mode: unsafe extern "efiapi" fn(*mut EfiGraphicsOutputProtocol, u32) -> EfiStatus,
    pub blt: *mut core::ffi::c_void,
    pub mode: *mut EfiGraphicsOutputProtocolMode,
}

// --- File System Protocols ---

pub const EFI_FILE_MODE_READ: u64 = 0x0000000000000001;

#[repr(C)]
pub struct EfiFileProtocol {
    pub revision: u64,
    pub open: unsafe extern "efiapi" fn(*mut EfiFileProtocol, *mut *mut EfiFileProtocol, *const u16, u64, u64) -> EfiStatus,
    pub close: unsafe extern "efiapi" fn(*mut EfiFileProtocol) -> EfiStatus,
    pub delete: unsafe extern "efiapi" fn(*mut EfiFileProtocol) -> EfiStatus,
    pub read: unsafe extern "efiapi" fn(*mut EfiFileProtocol, *mut usize, *mut core::ffi::c_void) -> EfiStatus,
    pub write: unsafe extern "efiapi" fn(*mut EfiFileProtocol, *mut usize, *const core::ffi::c_void) -> EfiStatus,
    pub get_position: *mut core::ffi::c_void,
    pub set_position: unsafe extern "efiapi" fn(*mut EfiFileProtocol, u64) -> EfiStatus,
    pub get_info: unsafe extern "efiapi" fn(*mut EfiFileProtocol, *const EfiGuid, *mut usize, *mut core::ffi::c_void) -> EfiStatus,
}

#[repr(C)]
pub struct EfiSimpleFileSystemProtocol {
    pub revision: u64,
    pub open_volume: unsafe extern "efiapi" fn(*mut EfiSimpleFileSystemProtocol, *mut *mut EfiFileProtocol) -> EfiStatus,
}

#[repr(C)]
pub struct EfiLoadedImageProtocol {
    pub revision: u32,
    pub parent_handle: EfiHandle,
    pub system_table: *mut EfiSystemTable,
    pub device_handle: EfiHandle,
    pub file_path: *mut core::ffi::c_void,
    pub reserved: *mut core::ffi::c_void,
    pub load_options_size: u32,
    pub load_options: *mut core::ffi::c_void,
    pub image_base: *mut core::ffi::c_void,
    pub image_size: u64,
    pub image_code_type: EfiMemoryType,
    pub image_data_type: EfiMemoryType,
    pub unload: *mut core::ffi::c_void,
}
