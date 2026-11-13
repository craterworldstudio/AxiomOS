use crate::uefi::*;

#[derive(Clone, Copy, Debug)]
pub struct GopFramebufferInfo {
    pub base: u64,
    pub size: u64,
    pub width: u32,
    pub height: u32,
    pub pitch: u32,
}

#[inline(always)]
unsafe fn serial_putc(c: u8) {
    core::arch::asm!("out dx, al", in("dx") 0x3F8u16, in("al") c, options(nomem, nostack));
}

unsafe fn serial_puts(s: &[u8]) {
    for &b in s {
        serial_putc(b);
    }
}

unsafe fn serial_print_num(mut n: u32) {
    if n == 0 {
        serial_putc(b'0');
        return;
    }
    let mut buf = [0u8; 10];
    let mut i = 0;
    while n > 0 {
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
        i += 1;
    }
    while i > 0 {
        i -= 1;
        serial_putc(buf[i]);
    }
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

    // Try to find a crisp 16:9 widescreen mode: prefer 1280x720, then 1920x1080
    // Fallback: 1024x768
    let mut chosen_mode = mode.mode;
    let max_mode = mode.max_mode;
    let mut found_720p = false;
    let mut found_1080p = false;
    let mut fallback_mode = None;

    serial_puts(b"[GOP] Available Display Modes:\n");
    for m in 0..max_mode {
        let mut info_size: usize = 0;
        let mut info_ptr: *mut EfiGraphicsOutputModeInformation = core::ptr::null_mut();
        let query_status = (gop.query_mode)(gop, m, &raw mut info_size, &raw mut info_ptr);
        if query_status == EFI_SUCCESS && !info_ptr.is_null() {
            let info = &*info_ptr;
            serial_puts(b"  Mode ");
            serial_print_num(m);
            serial_puts(b": ");
            serial_print_num(info.horizontal_resolution);
            serial_puts(b"x");
            serial_print_num(info.vertical_resolution);
            serial_puts(b"\n");

            if info.horizontal_resolution == 1280 && info.vertical_resolution == 720 {
                chosen_mode = m;
                found_720p = true;
            } else if !found_720p && info.horizontal_resolution == 1920 && info.vertical_resolution == 1080 {
                chosen_mode = m;
                found_1080p = true;
            } else if !found_720p && !found_1080p && info.horizontal_resolution == 1024 && info.vertical_resolution == 768 {
                fallback_mode = Some(m);
            }
        }
    }

    if !found_720p && !found_1080p {
        if let Some(fb) = fallback_mode {
            chosen_mode = fb;
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
