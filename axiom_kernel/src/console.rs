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

        // 6. Handle extended navigation keys (Arrow keys, Delete, Home, End)
        if is_extended {
            if (scancode & 0x80) != 0 {
                return 0; // Ignore extended break codes
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
