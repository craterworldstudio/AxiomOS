use crate::uefi::*;

#[derive(Clone, Copy, Debug)]
pub struct GopFramebufferInfo {
    pub base: u64,
    pub size: u64,
    pub width: u32,
    pub height: u32,
    pub pitch: u32,
}

pub unsafe fn init_gop(bs: &EfiBootServices) -> Result<GopFramebufferInfo, EfiStatus> {
    let mut gop_ptr: *mut core::ffi::c_void = core::ptr::null_mut();
    let status = (bs.locate_protocol)(
        &raw const EFI_GRAPHICS_OUTPUT_PROTOCOL_GUID,
        core::ptr::null_mut(),
        &raw mut gop_ptr,
    );

    if status != EFI_SUCCESS || gop_ptr.is_null() {
        return Err(status);
    }

    let gop = &mut *(gop_ptr as *mut EfiGraphicsOutputProtocol);
    let mode_ptr = gop.mode;
    if mode_ptr.is_null() {
        return Err(EFI_UNSUPPORTED);
    }
    let mode = &*mode_ptr;

    // Try to find a crisp high-res mode: prefer 1024x768, 1280x720, or 1280x800
    // If not found, keep whatever the current active mode is.
    let mut chosen_mode = mode.mode;
    let max_mode = mode.max_mode;

    for m in 0..max_mode {
        let mut info_size: usize = 0;
        let mut info_ptr: *mut EfiGraphicsOutputModeInformation = core::ptr::null_mut();
        let query_status = (gop.query_mode)(gop, m, &raw mut info_size, &raw mut info_ptr);
        if query_status == EFI_SUCCESS && !info_ptr.is_null() {
            let info = &*info_ptr;
            // Prefer 1024x768 for clean retro/telemetry readability
            if info.horizontal_resolution == 1024 && info.vertical_resolution == 768 {
                chosen_mode = m;
                break;
            }
        }
    }

    if chosen_mode != mode.mode {
        let _ = (gop.set_mode)(gop, chosen_mode);
    }

    // Refresh mode pointer after potential set_mode
    let active_mode = &*gop.mode;
    let active_info = &*active_mode.info;

    Ok(GopFramebufferInfo {
        base: active_mode.frame_buffer_base,
        size: active_mode.frame_buffer_size as u64,
        width: active_info.horizontal_resolution,
        height: active_info.vertical_resolution,
        pitch: active_info.pixels_per_scan_line,
    })
}
