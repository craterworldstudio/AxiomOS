//! AxiomOS User-Space Console Output Driver & Formatting
//!
//! Provides capability-backed printing, colored output, and screen control
//! via the root Console endpoint (slot 5).

use crate::capability::{CapHandle, EndpointCap};
use crate::syscall::*;
use crate::ROOT_SLOT_CONSOLE;

pub struct ConsoleWriter;

impl ConsoleWriter {
    /// Clears the screen via the root console capability.
    pub fn clear() {
        let ep = EndpointCap::from_handle(CapHandle::new(ROOT_SLOT_CONSOLE as u32, 1));
        unsafe {
            let _ = invoke_short(ep.0.0, OP_CONSOLE_CLEAR, 0, 0);
        }
    }

    /// Clears the screen and draws the high-resolution AxiomOS Desktop status bar.
    pub fn draw_banner() {
        let ep = EndpointCap::from_handle(CapHandle::new(ROOT_SLOT_CONSOLE as u32, 1));
        unsafe {
            let _ = invoke_short(ep.0.0, OP_CONSOLE_BANNER, 0, 0);
        }
    }

    /// Changes active text foreground and background color (0x00RRGGBB).
    pub fn set_color(fg: u32, bg: u32) {
        let ep = EndpointCap::from_handle(CapHandle::new(ROOT_SLOT_CONSOLE as u32, 1));
        unsafe {
            let _ = invoke_short(ep.0.0, OP_CONSOLE_COLOR, fg as u64, bg as u64);
        }
    }

    /// Writes raw string characters to the console in 8-byte packed syscall chunks.
    pub fn write_str(s: &str) {
        let ep = EndpointCap::from_handle(CapHandle::new(ROOT_SLOT_CONSOLE as u32, 1));
        let bytes = s.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            let chunk_len = core::cmp::min(8, bytes.len() - i);
            let mut chunk: u64 = 0;
            for j in 0..chunk_len {
                chunk |= (bytes[i + j] as u64) << (j * 8);
            }
            unsafe {
                let _ = invoke_short(ep.0.0, OP_CONSOLE_WRITE, chunk, chunk_len as u64);
            }
            i += chunk_len;
        }
    }
}

impl core::fmt::Write for ConsoleWriter {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        ConsoleWriter::write_str(s);
        Ok(())
    }
}

pub struct ConsoleReader;

impl ConsoleReader {
    /// Non-blocking query of a pending keystroke from the console capability.
    pub fn get_char() -> Option<char> {
        let ep = EndpointCap::from_handle(CapHandle::new(ROOT_SLOT_CONSOLE as u32, 1));
        unsafe {
            let res = invoke_short(ep.0.0, OP_CONSOLE_GETCHAR, 0, 0);
            if res != 0 && res < 128 {
                Some(res as u8 as char)
            } else {
                None
            }
        }
    }

    /// Reads a complete line of text from the console into `buf`.
    /// Echoes characters visually to the console, handles Backspace rubout,
    /// and returns the number of bytes read when Enter is pressed.
    pub fn read_line(buf: &mut [u8]) -> usize {
        let mut len = 0;
        let max_len = buf.len();

        loop {
            if let Some(ch) = Self::get_char() {
                if ch == '\n' {
                    ConsoleWriter::write_str("\n");
                    break;
                } else if ch == '\x08' { // Backspace
                    if len > 0 {
                        len -= 1;
                        ConsoleWriter::write_str("\x08");
                    }
                } else if (ch as u8) >= 32 && (ch as u8) <= 126 { // Printable ASCII
                    if len + 1 < max_len {
                        buf[len] = ch as u8;
                        len += 1;
                        let s = [ch as u8];
                        if let Ok(slice_str) = core::str::from_utf8(&s) {
                            ConsoleWriter::write_str(slice_str);
                        }
                    }
                }
            } else {
                unsafe { core::arch::asm!("pause"); }
            }
        }

        len
    }
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ({
        use core::fmt::Write;
        let _ = write!($crate::console::ConsoleWriter, $($arg)*);
    });
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ({
        $crate::print!("{}\n", format_args!($($arg)*));
    });
}
