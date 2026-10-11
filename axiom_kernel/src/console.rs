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

    pub unsafe fn write_char(&mut self, c: u8) {
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
}
