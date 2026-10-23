#![allow(dead_code)]

use crate::memory::BootInfo;

// Standard 8x16 bitmap font for ASCII 32..127
// Each character is 16 bytes (one byte per row, 8 bits wide).
include!("font_8x16.rs");

pub struct Console {
    pub fb_base: *mut u32,
    pub width: usize,
    pub height: usize,
    pub pitch: usize,
    pub col: usize,
    pub row: usize,
    pub max_cols: usize,
    pub max_rows: usize,
    pub fg_color: u32, // 0x00RRGGBB
    pub bg_color: u32,
    pub is_graphics: bool,
    pub shift_pressed: bool,
    pub ctrl_pressed: bool,
    pub alt_pressed: bool,
    pub extended_prefix: bool,
}

pub static mut CONSOLE: Console = Console {
    fb_base: core::ptr::null_mut(),
    width: 80,
    height: 25,
    pitch: 80,
    col: 0,
    row: 0,
    max_cols: 80,
    max_rows: 25,
    fg_color: 0x00E0E0E0, // Crisp silver-white
    bg_color: 0x00101418, // Deep modern terminal dark slate
    is_graphics: false,
    shift_pressed: false,
    ctrl_pressed: false,
    alt_pressed: false,
    extended_prefix: false,
};

impl Console {
    pub unsafe fn init(&mut self, boot_info: *const BootInfo) {
        if !boot_info.is_null() && (*boot_info).framebuffer_base != 0 {
            let bi = &*boot_info;
            self.fb_base = bi.framebuffer_base as *mut u32;
            self.width = bi.horizontal_resolution as usize;
            self.height = bi.vertical_resolution as usize;
            self.pitch = bi.pixels_per_scan_line as usize;
            self.max_cols = self.width / 8;
            self.max_rows = self.height / 16;
            self.col = 0;
            self.row = 0;
            self.is_graphics = true;

            self.clear();
        } else {
            // Fallback to 80x25 VGA text mode at 0xB8000
            self.fb_base = 0xB8000 as *mut u32;
            self.width = 80;
            self.height = 25;
            self.pitch = 80;
            self.max_cols = 80;
            self.max_rows = 25;
            self.col = 0;
            self.row = 0;
            self.is_graphics = false;
        }
    }

    pub unsafe fn clear(&mut self) {
        if self.is_graphics && !self.fb_base.is_null() {
            let total_pixels = self.pitch * self.height;
            for i in 0..total_pixels {
                *self.fb_base.add(i) = self.bg_color;
            }
        } else {
            let vga = 0xB8000 as *mut u8;
            for i in 0..2000 {
                *vga.add(i * 2) = b' ';
                *vga.add(i * 2 + 1) = 0x0F;
            }
        }
        self.col = 0;
        self.row = 0;
    }

    pub unsafe fn set_color(&mut self, fg: u32, bg: u32) {
        self.fg_color = fg;
        self.bg_color = bg;
    }

    pub unsafe fn set_cursor(&mut self, row: usize, col: usize) {
        if row < self.max_rows {
            self.row = row;
        }
        if col < self.max_cols {
            self.col = col;
        }
    }

    pub unsafe fn write_char(&mut self, c: u8) {
        if c == 0x08 { // Backspace (rubout)
            if self.col > 0 {
                self.col -= 1;
                if self.is_graphics && !self.fb_base.is_null() {
                    self.draw_glyph(self.col * 8, self.row * 16, b' ', self.fg_color, self.bg_color);
                } else {
                    let vga = 0xB8000 as *mut u8;
                    let offset = (self.row * 80 + self.col) * 2;
                    *vga.add(offset) = b' ';
                    *vga.add(offset + 1) = 0x0F;
                }
            }
            return;
        }

        if c == b'\n' {
            self.col = 0;
            self.row += 1;
            if self.row >= self.max_rows {
                self.scroll();
            }
            return;
        }

        if c == b'\r' {
            self.col = 0;
            return;
        }

        if self.is_graphics && !self.fb_base.is_null() {
            self.draw_glyph(self.col * 8, self.row * 16, c, self.fg_color, self.bg_color);
        } else {
            let vga = 0xB8000 as *mut u8;
            let offset = (self.row * 80 + self.col) * 2;
            *vga.add(offset) = c;
            *vga.add(offset + 1) = 0x0F;
        }

        self.col += 1;
        if self.col >= self.max_cols {
            self.col = 0;
            self.row += 1;
            if self.row >= self.max_rows {
                self.scroll();
            }
        }
    }

    pub unsafe fn write_str(&mut self, s: &str) {
        for b in s.bytes() {
            self.write_char(b);
        }
    }

    pub unsafe fn write_bytes(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.write_char(b);
        }
    }

    unsafe fn scroll(&mut self) {
        if self.is_graphics && !self.fb_base.is_null() {
            // Shift screen up by 16 scanlines
            let scanline_shift = 16 * self.pitch;
            let total_pixels = self.pitch * self.height;
            let copy_count = total_pixels - scanline_shift;

            // Direct memmove-style scanline copy
            core::ptr::copy(
                self.fb_base.add(scanline_shift),
                self.fb_base,
                copy_count,
            );

            // Blank the bottom 16 scanlines
            let bottom_start = self.fb_base.add(copy_count);
            for i in 0..scanline_shift {
                *bottom_start.add(i) = self.bg_color;
            }

            self.row = self.max_rows - 1;
        } else {
            // VGA text mode scroll: copy rows 1..25 to 0..24
            let vga = 0xB8000 as *mut u8;
            core::ptr::copy(vga.add(160), vga, 160 * 24);
            for col in 0..80 {
                *vga.add(160 * 24 + col * 2) = b' ';
                *vga.add(160 * 24 + col * 2 + 1) = 0x0F;
            }
            self.row = self.max_rows - 1;
        }
    }

    unsafe fn draw_glyph(&self, x: usize, y: usize, c: u8, fg: u32, bg: u32) {
        let glyph_idx = if c >= 32 && c <= 126 { (c - 32) as usize } else { 0 };
        let glyph = &FONT_8X16[glyph_idx];

        for row_idx in 0..16 {
            let row_byte = glyph[row_idx];
            let fb_row_start = self.fb_base.add((y + row_idx) * self.pitch + x);

            for col_idx in 0..8 {
                let bit_set = (row_byte & (0x80 >> col_idx)) != 0;
                let color = if bit_set { fg } else { bg };
                *fb_row_start.add(col_idx) = color;
            }
        }
    }

    pub unsafe fn draw_banner(&mut self) {
        crate::serial_print("[CONSOLE] draw_banner called! is_graphics=");
        crate::serial_print(if self.is_graphics { "true\n" } else { "false\n" });
        self.clear();
        if self.is_graphics && !self.fb_base.is_null() {
            // 1. Draw top status bar background: 28 pixels high across all columns
            // Color: Deep obsidian slate (0x00161B22)
            let header_height = 28;
            for row in 0..header_height {
                let offset = row * self.pitch;
                for col in 0..self.width {
                    *self.fb_base.add(offset + col) = 0x00161B22;
                }
            }
            // 2. Draw accent border line at y = 28 (Electric Cyan: 0x0000D2FF)
            let border_offset = header_height * self.pitch;
            for col in 0..self.width {
                *self.fb_base.add(border_offset + col) = 0x0000D2FF;
            }

            // 3. Render Status Text on Header
            let old_fg = self.fg_color;
            let old_bg = self.bg_color;
            self.col = 2;
            self.row = 0;
            self.bg_color = 0x00161B22;

            self.fg_color = 0x0000D2FF; // Electric Cyan
            self.write_str("[ AXIOM OS ]");

            self.fg_color = 0x00E0E0E0; // Silver White
            self.write_str("  v0.2.0-alpha   |   ");

            self.fg_color = 0x002EE59D; // Emerald Green
            self.write_str("PID 1: Genesis");

            self.fg_color = 0x00E0E0E0;
            self.write_str("   |   ");

            self.fg_color = 0x00FFD166; // Gold Yellow
            self.write_str("Mode: Sovereign Ring 3");

            self.fg_color = 0x00E0E0E0;
            self.write_str("   |   ");

            self.fg_color = 0x0000D2FF;
            self.write_str("Display: 1280x720 16:9");

            // Reset cursor to start below header (Row 3)
            self.row = 3;
            self.col = 2;
            self.fg_color = old_fg;
            self.bg_color = old_bg;
        } else {
            self.row = 1;
            self.col = 0;
            self.write_str("[ AXIOM OS ] Genesis Online\n");
        }
    }

    pub unsafe fn draw_string_at(&self, x: usize, y: usize, s: &str, fg: u32, bg: u32) {
        for (i, b) in s.bytes().enumerate() {
            self.draw_glyph(x + i * 8, y, b, fg, bg);
        }
    }

    pub unsafe fn draw_panic_screen(
        &mut self,
        exception_title: &str,
        sub_title: &str,
        rip: u64,
        rsp: u64,
        cr2: u64,
        error_code: u64,
        cr3: u64,
        rflags: u64,
        cs: u64,
    ) {
        if !self.is_graphics || self.fb_base.is_null() {
            let vga = 0xB8000 as *mut u8;
            for i in 0..2000 {
                *vga.add(i * 2) = b' ';
                *vga.add(i * 2 + 1) = 0x4F; // White on Red
            }
            return;
        }

        // 1. Fill Canvas with Deep Obsidian / Charcoal (0x000E1015)
        let total_pixels = self.pitch * self.height;
        for i in 0..total_pixels {
            *self.fb_base.add(i) = 0x000E1015;
        }

        // 2. High-Impact Crimson Red Header Bar (40px high)
        let header_height = 40;
        for row in 0..header_height {
            let offset = row * self.pitch;
            for col in 0..self.width {
                *self.fb_base.add(offset + col) = 0x00FF3B30; // Vibrant Crimson
            }
        }
        // Accent separator line below header (Gold Yellow: 0x00FFD166)
        let border_offset = header_height * self.pitch;
        for col in 0..self.width {
            *self.fb_base.add(border_offset + col) = 0x00FFD166;
        }

        self.draw_string_at(24, 12, "[ AXIOM OS SOVEREIGN MICROKERNEL -- KERNEL PANIC ]", 0x00FFFFFF, 0x00FF3B30);
        let tag = "Ring 0 Substrate Fault";
        if self.width > 240 {
            self.draw_string_at(self.width - 240, 12, tag, 0x00FFFFFF, 0x00FF3B30);
        }

        // 3. Central Diagnostic Card (width 1000, height 540)
        let box_x0 = if self.width > 1000 { (self.width - 1000) / 2 } else { 20 };
        let box_y0 = 70;
        let box_w = core::cmp::min(1000, self.width - 40);
        let box_h = core::cmp::min(550, self.height - 80);

        // Fill Diagnostic Card Background (Dark Terminal Slate: 0x00151922)
        for r in 0..box_h {
            let y = box_y0 + r;
            let row_start = y * self.pitch;
            for c in 0..box_w {
                *self.fb_base.add(row_start + box_x0 + c) = 0x00151922;
            }
        }

        // Draw Card Borders (2px Crimson 0x00FF3B30)
        for c in 0..box_w {
            *self.fb_base.add(box_y0 * self.pitch + (box_x0 + c)) = 0x00FF3B30;
            *self.fb_base.add((box_y0 + 1) * self.pitch + (box_x0 + c)) = 0x00FF3B30;
            *self.fb_base.add((box_y0 + box_h - 1) * self.pitch + (box_x0 + c)) = 0x00FF3B30;
            *self.fb_base.add((box_y0 + box_h - 2) * self.pitch + (box_x0 + c)) = 0x00FF3B30;
        }
        for r in 0..box_h {
            *self.fb_base.add((box_y0 + r) * self.pitch + box_x0) = 0x00FF3B30;
            *self.fb_base.add((box_y0 + r) * self.pitch + box_x0 + 1) = 0x00FF3B30;
            *self.fb_base.add((box_y0 + r) * self.pitch + (box_x0 + box_w - 1)) = 0x00FF3B30;
            *self.fb_base.add((box_y0 + r) * self.pitch + (box_x0 + box_w - 2)) = 0x00FF3B30;
        }

        // Card Title Bar (Deep Crimson Fill: 0x002B0D12)
        for r in 2..38 {
            let row_start = (box_y0 + r) * self.pitch;
            for c in 2..(box_w - 2) {
                *self.fb_base.add(row_start + box_x0 + c) = 0x002B0D12;
            }
        }
        self.draw_string_at(box_x0 + 24, box_y0 + 12, "CRITICAL SUBSTRATE INVARIANT FAULT", 0x00FF4B4B, 0x002B0D12);

        // Content Area inside Box
        let content_x = box_x0 + 24;
        let mut cur_y = box_y0 + 56;
        let card_bg = 0x00151922;

        // Exception & Reason
        self.draw_string_at(content_x, cur_y, "EXCEPTION : ", 0x00FFD166, card_bg); // Gold
        self.draw_string_at(content_x + 12 * 8, cur_y, exception_title, 0x00FF4B4B, card_bg);
        cur_y += 24;

        self.draw_string_at(content_x, cur_y, "DETAILS   : ", 0x00FFD166, card_bg);
        self.draw_string_at(content_x + 12 * 8, cur_y, sub_title, 0x00E0E0E0, card_bg);
        cur_y += 32;

        // Separator Line
        for c in 0..(box_w - 48) {
            *self.fb_base.add(cur_y * self.pitch + content_x + c) = 0x00252B3B;
        }
        cur_y += 16;

        // Hardware Context
        self.draw_string_at(content_x, cur_y, "REGISTER DIAGNOSTICS (x86_64 HARDWARE CONTEXT):", 0x0000D2FF, card_bg); // Electric Cyan
        cur_y += 24;

        let mut buf_rip = [0u8; 18]; Self::format_hex(rip, &mut buf_rip);
        let mut buf_rsp = [0u8; 18]; Self::format_hex(rsp, &mut buf_rsp);
        let mut buf_cr2 = [0u8; 18]; Self::format_hex(cr2, &mut buf_cr2);
        let mut buf_cr3 = [0u8; 18]; Self::format_hex(cr3, &mut buf_cr3);
        let mut buf_err = [0u8; 18]; Self::format_hex(error_code, &mut buf_err);
        let mut buf_rfl = [0u8; 18]; Self::format_hex(rflags, &mut buf_rfl);
        let mut buf_cs  = [0u8; 18]; Self::format_hex(cs, &mut buf_cs);

        let rip_s = core::str::from_utf8(&buf_rip).unwrap_or("");
        let rsp_s = core::str::from_utf8(&buf_rsp).unwrap_or("");
        let cr2_s = core::str::from_utf8(&buf_cr2).unwrap_or("");
        let cr3_s = core::str::from_utf8(&buf_cr3).unwrap_or("");
        let err_s = core::str::from_utf8(&buf_err).unwrap_or("");
        let rfl_s = core::str::from_utf8(&buf_rfl).unwrap_or("");
        let cs_s  = core::str::from_utf8(&buf_cs).unwrap_or("");

        self.draw_string_at(content_x + 16, cur_y, "RIP : ", 0x00FFD166, card_bg);
        self.draw_string_at(content_x + 16 + 6 * 8, cur_y, rip_s, 0x00FFFFFF, card_bg);
        self.draw_string_at(content_x + 360, cur_y, "RSP : ", 0x00FFD166, card_bg);
        self.draw_string_at(content_x + 360 + 6 * 8, cur_y, rsp_s, 0x00FFFFFF, card_bg);
        cur_y += 20;

        self.draw_string_at(content_x + 16, cur_y, "CR2 : ", 0x00FFD166, card_bg);
        self.draw_string_at(content_x + 16 + 6 * 8, cur_y, cr2_s, 0x00FFFFFF, card_bg);
        self.draw_string_at(content_x + 360, cur_y, "CR3 : ", 0x00FFD166, card_bg);
        self.draw_string_at(content_x + 360 + 6 * 8, cur_y, cr3_s, 0x00FFFFFF, card_bg);
        cur_y += 20;

        self.draw_string_at(content_x + 16, cur_y, "ERR : ", 0x00FFD166, card_bg);
        self.draw_string_at(content_x + 16 + 6 * 8, cur_y, err_s, 0x00FFFFFF, card_bg);
        self.draw_string_at(content_x + 360, cur_y, "RFL : ", 0x00FFD166, card_bg);
        self.draw_string_at(content_x + 360 + 6 * 8, cur_y, rfl_s, 0x00FFFFFF, card_bg);
        cur_y += 20;

        self.draw_string_at(content_x + 16, cur_y, "CS  : ", 0x00FFD166, card_bg);
        self.draw_string_at(content_x + 16 + 6 * 8, cur_y, cs_s, 0x00FFFFFF, card_bg);
        cur_y += 32;

        // Separator Line
        for c in 0..(box_w - 48) {
            *self.fb_base.add(cur_y * self.pitch + content_x + c) = 0x00252B3B;
        }
        cur_y += 16;

        // Sovereign Substrate State
        self.draw_string_at(content_x, cur_y, "SOVEREIGN SECURITY & SUBSTRATE DEFENSE STATE:", 0x002EE59D, card_bg); // Emerald
        cur_y += 24;

        self.draw_string_at(content_x + 16, cur_y, "[x] Invariant Guarantee : Zero Ambient Authority Preserved", 0x00E0E0E0, card_bg);
        cur_y += 18;
        self.draw_string_at(content_x + 16, cur_y, "[x] Scheduler Isolation : All Active Threads & Preemption Frozen", 0x00E0E0E0, card_bg);
        cur_y += 18;
        self.draw_string_at(content_x + 16, cur_y, "[x] Storage Integrity   : ATA PIO Bus Quiesced & Disarmed", 0x00E0E0E0, card_bg);
        cur_y += 32;

        // Separator Line
        for c in 0..(box_w - 48) {
            *self.fb_base.add(cur_y * self.pitch + content_x + c) = 0x00252B3B;
        }
        cur_y += 16;

        // Halting Banner
        self.draw_string_at(content_x, cur_y, "[ SYSTEM HALTED: Microkernel Ring 0 Substrate Protection Engaged ]", 0x00FF4B4B, card_bg);
        cur_y += 20;
        self.draw_string_at(content_x, cur_y, "Emergency Reboot: Press Ctrl+Shift+Alt then type R-E-S-E-T then Enter to reboot.", 0x00FFD166, card_bg);
        cur_y += 18;
        self.draw_string_at(content_x, cur_y, "Manual Reset    : Press physical machine reset button or reboot hypervisor.", 0x00A0A0A0, card_bg);
        cur_y += 22;
        self.draw_string_at(content_x, cur_y, "> Emergency Sequence: [ _ _ _ _ _ ]  (Waiting for Ctrl+Shift+Alt...)", 0x0000D2FF, card_bg);
    }

    pub unsafe fn panic_emergency_reboot_loop(&mut self) -> ! {
        let box_x0 = if self.width > 1000 { (self.width - 1000) / 2 } else { 20 };
        let box_w = core::cmp::min(1000, self.width - 40);
        let content_x = box_x0 + 24;
        let card_bg = 0x00151922;
        let status_y = 70 + 448; // Exactly matches emergency line in draw_panic_screen

        let clear_status_line = |fb_base: *mut u32, pitch: usize| {
            if fb_base.is_null() { return; }
            for r in 0..18 {
                let y = status_y + r;
                let row_start = y * pitch;
                for c in 0..(box_w - 48) {
                    *fb_base.add(row_start + content_x + c) = card_bg;
                }
            }
        };

        // Drain pending keyboard bytes
        for _ in 0..32 {
            let status: u8;
            core::arch::asm!("in al, dx", in("dx") 0x64u16, out("al") status, options(nomem, nostack));
            if (status & 1) != 0 {
                let _: u8;
                core::arch::asm!("in al, dx", in("dx") 0x60u16, out("al") _, options(nomem, nostack));
            } else {
                break;
            }
        }

        let mut ctrl = false;
        let mut shift = false;
        let mut alt = false;
        let mut armed = false;
        let mut ext_prefix = false;
        let mut step = 0usize; // 0: R, 1: E, 2: S, 3: E, 4: T, 5: Enter

        loop {
            let status: u8;
            core::arch::asm!("in al, dx", in("dx") 0x64u16, out("al") status, options(nomem, nostack));
            if (status & 1) == 0 {
                core::arch::asm!("pause", options(nomem, nostack));
                continue;
            }

            let scancode: u8;
            core::arch::asm!("in al, dx", in("dx") 0x60u16, out("al") scancode, options(nomem, nostack));

            if scancode == 0xE0 {
                ext_prefix = true;
                continue;
            }
            let is_ext = ext_prefix;
            ext_prefix = false;

            // Track Shift
            if !is_ext {
                if scancode == 0x2A || scancode == 0x36 {
                    shift = true;
                } else if scancode == 0xAA || scancode == 0xB6 {
                    shift = false;
                }
            }

            // Track Ctrl
            if scancode == 0x1D {
                ctrl = true;
            } else if scancode == 0x9D {
                ctrl = false;
            }

            // Track Alt
            if scancode == 0x38 {
                alt = true;
            } else if scancode == 0xB8 {
                alt = false;
            }

            // Check if Ctrl+Shift+Alt combination is triggered
            if ctrl && shift && alt && !armed {
                armed = true;
                step = 0;
                clear_status_line(self.fb_base, self.pitch);
                self.draw_string_at(content_x, status_y, "> Emergency Sequence: [ _ _ _ _ _ ]  (ARMED! Type R-E-S-E-T then Enter)", 0x00FFD166, card_bg);
                crate::serial_print("[PANIC] Emergency reboot sequence ARMED via Ctrl+Shift+Alt!\n");
            }

            // If it's a key release (break code), ignore it
            if (scancode & 0x80) != 0 {
                continue;
            }

            // Ignore modifier make codes themselves when checking letters
            if scancode == 0x1D || scancode == 0x2A || scancode == 0x36 || scancode == 0x38 {
                continue;
            }

            if armed || (ctrl && shift && alt) {
                match step {
                    0 => {
                        if scancode == 0x13 { // 'R'
                            step = 1;
                            clear_status_line(self.fb_base, self.pitch);
                            self.draw_string_at(content_x, status_y, "> Emergency Sequence: [ R _ _ _ _ ]  (ARMED! Type E-S-E-T then Enter)", 0x00FFD166, card_bg);
                            crate::serial_print("[PANIC] Emergency Key 'R' matched (1/5)\n");
                        }
                    },
                    1 => {
                        if scancode == 0x12 { // 'E'
                            step = 2;
                            clear_status_line(self.fb_base, self.pitch);
                            self.draw_string_at(content_x, status_y, "> Emergency Sequence: [ R E _ _ _ ]  (ARMED! Type S-E-T then Enter)", 0x00FFD166, card_bg);
                            crate::serial_print("[PANIC] Emergency Key 'E' matched (2/5)\n");
                        } else if scancode == 0x13 {
                            step = 1;
                            clear_status_line(self.fb_base, self.pitch);
                            self.draw_string_at(content_x, status_y, "> Emergency Sequence: [ R _ _ _ _ ]  (ARMED! Type E-S-E-T then Enter)", 0x00FFD166, card_bg);
                        } else {
                            step = 0;
                            clear_status_line(self.fb_base, self.pitch);
                            self.draw_string_at(content_x, status_y, "> Emergency Sequence: [ _ _ _ _ _ ]  (ARMED! Type R-E-S-E-T then Enter)", 0x00FFD166, card_bg);
                        }
                    },
                    2 => {
                        if scancode == 0x1F { // 'S'
                            step = 3;
                            clear_status_line(self.fb_base, self.pitch);
                            self.draw_string_at(content_x, status_y, "> Emergency Sequence: [ R E S _ _ ]  (ARMED! Type E-T then Enter)", 0x00FFD166, card_bg);
                            crate::serial_print("[PANIC] Emergency Key 'S' matched (3/5)\n");
                        } else if scancode == 0x13 {
                            step = 1;
                            clear_status_line(self.fb_base, self.pitch);
                            self.draw_string_at(content_x, status_y, "> Emergency Sequence: [ R _ _ _ _ ]  (ARMED! Type E-S-E-T then Enter)", 0x00FFD166, card_bg);
                        } else {
                            step = 0;
                            clear_status_line(self.fb_base, self.pitch);
                            self.draw_string_at(content_x, status_y, "> Emergency Sequence: [ _ _ _ _ _ ]  (ARMED! Type R-E-S-E-T then Enter)", 0x00FFD166, card_bg);
                        }
                    },
                    3 => {
                        if scancode == 0x12 { // 'E'
                            step = 4;
                            clear_status_line(self.fb_base, self.pitch);
                            self.draw_string_at(content_x, status_y, "> Emergency Sequence: [ R E S E _ ]  (ARMED! Type T then Enter)", 0x00FFD166, card_bg);
                            crate::serial_print("[PANIC] Emergency Key 'E' matched (4/5)\n");
                        } else if scancode == 0x13 {
                            step = 1;
                            clear_status_line(self.fb_base, self.pitch);
                            self.draw_string_at(content_x, status_y, "> Emergency Sequence: [ R _ _ _ _ ]  (ARMED! Type E-S-E-T then Enter)", 0x00FFD166, card_bg);
                        } else {
                            step = 0;
                            clear_status_line(self.fb_base, self.pitch);
                            self.draw_string_at(content_x, status_y, "> Emergency Sequence: [ _ _ _ _ _ ]  (ARMED! Type R-E-S-E-T then Enter)", 0x00FFD166, card_bg);
                        }
                    },
                    4 => {
                        if scancode == 0x14 { // 'T'
                            step = 5;
                            clear_status_line(self.fb_base, self.pitch);
                            self.draw_string_at(content_x, status_y, "> Emergency Sequence: [ R E S E T ]  (PRESS ENTER TO REBOOT)", 0x002EE59D, card_bg);
                            crate::serial_print("[PANIC] Emergency Key 'T' matched (5/5) -- Waiting for Enter\n");
                        } else if scancode == 0x13 {
                            step = 1;
                            clear_status_line(self.fb_base, self.pitch);
                            self.draw_string_at(content_x, status_y, "> Emergency Sequence: [ R _ _ _ _ ]  (ARMED! Type E-S-E-T then Enter)", 0x00FFD166, card_bg);
                        } else {
                            step = 0;
                            clear_status_line(self.fb_base, self.pitch);
                            self.draw_string_at(content_x, status_y, "> Emergency Sequence: [ _ _ _ _ _ ]  (ARMED! Type R-E-S-E-T then Enter)", 0x00FFD166, card_bg);
                        }
                    },
                    5 => {
                        if scancode == 0x1C { // Enter
                            clear_status_line(self.fb_base, self.pitch);
                            self.draw_string_at(content_x, status_y, "> EMERGENCY REBOOT CONFIRMED: Resetting machine substrate now...", 0x00FF4B4B, card_bg);
                            crate::serial_print("[PANIC] EMERGENCY REBOOT CONFIRMED! Resetting machine substrate...\n");

                            // Hardware reset sequence:
                            // 1. 8042 Keyboard controller pulse
                            core::arch::asm!("out dx, al", in("dx") 0x64u16, in("al") 0xFEu8, options(nomem, nostack));
                            // 2. PCI reset register
                            core::arch::asm!("out dx, al", in("dx") 0xCF9u16, in("al") 0x06u8, options(nomem, nostack));
                            // 3. QEMU ACPI reset
                            core::arch::asm!("out dx, ax", in("dx") 0x604u16, in("ax") 0x2000u16, options(nomem, nostack));
                            // 4. Triple fault fallback
                            let null_idtr: [u8; 10] = [0; 10];
                            core::arch::asm!("lidt [{}]", in(reg) null_idtr.as_ptr(), options(readonly, nostack));
                            core::arch::asm!("int 3");
                        } else if scancode == 0x13 {
                            step = 1;
                            clear_status_line(self.fb_base, self.pitch);
                            self.draw_string_at(content_x, status_y, "> Emergency Sequence: [ R _ _ _ _ ]  (ARMED! Type E-S-E-T then Enter)", 0x00FFD166, card_bg);
                        }
                    },
                    _ => {
                        step = 0;
                    }
                }
            }
        }
    }

    pub fn format_hex(val: u64, buf: &mut [u8; 18]) {
        buf[0] = b'0';
        buf[1] = b'x';
        let hex = b"0123456789ABCDEF";
        for i in 0..16 {
            buf[2 + i] = hex[((val >> ((15 - i) * 4)) & 0xF) as usize];
        }
    }


    /// Non-blocking read of an ASCII character or special navigation key from PS/2 keyboard controller.
    /// Returns 0 if no keystroke is available.
    pub unsafe fn read_char(&mut self) -> u8 {
        // 1. Check if PS/2 status port 0x64 has output buffer full (bit 0)
        let status: u8;
        core::arch::asm!("in al, dx", in("dx") 0x64u16, out("al") status, options(nomem, nostack));
        if (status & 1) == 0 {
            return 0;
        }

        // 2. Read scancode from port 0x60
        let scancode: u8;
        core::arch::asm!("in al, dx", in("dx") 0x60u16, out("al") scancode, options(nomem, nostack));

        // 3. Handle extended scancode prefix 0xE0
        if scancode == 0xE0 {
            self.extended_prefix = true;
            return 0;
        }

        let is_extended = self.extended_prefix;
        self.extended_prefix = false;

        // 4. Track Shift key state (Set 1: Left Shift = 0x2A, Right Shift = 0x36)
        if !is_extended {
            if scancode == 0x2A || scancode == 0x36 {
                self.shift_pressed = true;
                return 0;
            }
            if scancode == 0xAA || scancode == 0xB6 {
                self.shift_pressed = false;
                return 0;
            }
        }

        // 5. Track Control key state (Left Ctrl = 0x1D / 0x9D, Right Ctrl = 0xE0 0x1D / 0xE0 0x9D)
        if scancode == 0x1D {
            self.ctrl_pressed = true;
            return 0;
        }
        if scancode == 0x9D {
            self.ctrl_pressed = false;
            return 0;
        }

        // 6. Track Alt key state (Left Alt = 0x38 / 0xB8, Right Alt = 0xE0 0x38 / 0xE0 0xB8)
        if scancode == 0x38 {
            self.alt_pressed = true;
            return 0;
        }
        if scancode == 0xB8 {
            self.alt_pressed = false;
            return 0;
        }

        // 7. Handle extended navigation keys (Arrow keys, Delete, Home, End)
        if is_extended {
            if (scancode & 0x80) != 0 {
                return 0; // Ignore extended break codes
            }
            if self.shift_pressed {
                return match scancode {
                    0x48 => 135, // KEY_SHIFT_UP
                    0x50 => 136, // KEY_SHIFT_DOWN
                    0x4B => 137, // KEY_SHIFT_LEFT
                    0x4D => 138, // KEY_SHIFT_RIGHT
                    _ => 0,
                };
            }
            return match scancode {
                0x48 => 128, // KEY_UP
                0x50 => 129, // KEY_DOWN
                0x4B => 130, // KEY_LEFT
                0x4D => 131, // KEY_RIGHT
                0x53 => 132, // KEY_DELETE
                0x47 => 133, // KEY_HOME
                0x4F => 134, // KEY_END
                _ => 0,
            };
        }

        // Ignore regular break codes (bit 7 set)
        if (scancode & 0x80) != 0 {
            return 0;
        }

        // 7. Translate Scancode Set 1 make code to ASCII with shift and ctrl modifiers
        translate_scancode(scancode, self.shift_pressed, self.ctrl_pressed)
    }
}

/// Translates standard PC/AT Scancode Set 1 make codes to ASCII characters.
fn translate_scancode(scancode: u8, shift: bool, ctrl: bool) -> u8 {
    if ctrl {
        return match scancode {
            0x1E => 1,  // Ctrl+A
            0x30 => 2,  // Ctrl+B
            0x2E => 3,  // Ctrl+C
            0x20 => 4,  // Ctrl+D
            0x12 => 5,  // Ctrl+E
            0x21 => 6,  // Ctrl+F
            0x22 => 7,  // Ctrl+G
            0x23 => 8,  // Ctrl+H (Backspace)
            0x17 => 9,  // Ctrl+I (Tab)
            0x24 => 10, // Ctrl+J (LF)
            0x25 => 11, // Ctrl+K
            0x26 => 12, // Ctrl+L
            0x32 => 13, // Ctrl+M (CR)
            0x31 => 14, // Ctrl+N
            0x18 => 15, // Ctrl+O
            0x19 => 16, // Ctrl+P
            0x10 => 17, // Ctrl+Q
            0x13 => 18, // Ctrl+R
            0x1F => 19, // Ctrl+S
            0x14 => 20, // Ctrl+T
            0x16 => 21, // Ctrl+U
            0x2F => 22, // Ctrl+V
            0x11 => 23, // Ctrl+W
            0x2D => 24, // Ctrl+X
            0x15 => 25, // Ctrl+Y
            0x2C => 26, // Ctrl+Z
            _ => 0,
        };
    }

    match scancode {
        0x01 => 27, // ESC
        0x02 => if shift { b'!' } else { b'1' },
        0x03 => if shift { b'@' } else { b'2' },
        0x04 => if shift { b'#' } else { b'3' },
        0x05 => if shift { b'$' } else { b'4' },
        0x06 => if shift { b'%' } else { b'5' },
        0x07 => if shift { b'^' } else { b'6' },
        0x08 => if shift { b'&' } else { b'7' },
        0x09 => if shift { b'*' } else { b'8' },
        0x0A => if shift { b'(' } else { b'9' },
        0x0B => if shift { b')' } else { b'0' },
        0x0C => if shift { b'_' } else { b'-' },
        0x0D => if shift { b'+' } else { b'=' },
        0x0E => 0x08, // Backspace
        0x0F => b'\t',
        0x10 => if shift { b'Q' } else { b'q' },
        0x11 => if shift { b'W' } else { b'w' },
        0x12 => if shift { b'E' } else { b'e' },
        0x13 => if shift { b'R' } else { b'r' },
        0x14 => if shift { b'T' } else { b't' },
        0x15 => if shift { b'Y' } else { b'y' },
        0x16 => if shift { b'U' } else { b'u' },
        0x17 => if shift { b'I' } else { b'i' },
        0x18 => if shift { b'O' } else { b'o' },
        0x19 => if shift { b'P' } else { b'p' },
        0x1A => if shift { b'{' } else { b'[' },
        0x1B => if shift { b'}' } else { b']' },
        0x1C => b'\n', // Enter
        0x1E => if shift { b'A' } else { b'a' },
        0x1F => if shift { b'S' } else { b's' },
        0x20 => if shift { b'D' } else { b'd' },
        0x21 => if shift { b'F' } else { b'f' },
        0x22 => if shift { b'G' } else { b'g' },
        0x23 => if shift { b'H' } else { b'h' },
        0x24 => if shift { b'J' } else { b'j' },
        0x25 => if shift { b'K' } else { b'k' },
        0x26 => if shift { b'L' } else { b'l' },
        0x27 => if shift { b':' } else { b';' },
        0x28 => if shift { b'"' } else { b'\'' },
        0x29 => if shift { b'~' } else { b'`' },
        0x2B => if shift { b'|' } else { b'\\' },
        0x2C => if shift { b'Z' } else { b'z' },
        0x2D => if shift { b'X' } else { b'x' },
        0x2E => if shift { b'C' } else { b'c' },
        0x2F => if shift { b'V' } else { b'v' },
        0x30 => if shift { b'B' } else { b'b' },
        0x31 => if shift { b'N' } else { b'n' },
        0x32 => if shift { b'M' } else { b'm' },
        0x33 => if shift { b'<' } else { b',' },
        0x34 => if shift { b'>' } else { b'.' },
        0x35 => if shift { b'?' } else { b'/' },
        0x39 => b' ',
        _ => 0,
    }
}
