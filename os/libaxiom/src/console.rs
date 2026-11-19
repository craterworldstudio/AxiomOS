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

    /// Sets write cursor position (row, col) in-place without clearing screen.
    pub fn set_cursor(row: usize, col: usize) {
        let ep = EndpointCap::from_handle(CapHandle::new(ROOT_SLOT_CONSOLE as u32, 1));
        unsafe {
            let _ = invoke_short(ep.0.0, OP_CONSOLE_SET_CURSOR, row as u64, col as u64);
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

pub const KEY_CTRL_C: u8    = 3;
pub const KEY_BACKSPACE: u8 = 8;
pub const KEY_TAB: u8       = 9;
pub const KEY_ENTER: u8     = 10;
pub const KEY_UP: u8        = 128;
pub const KEY_DOWN: u8      = 129;
pub const KEY_LEFT: u8      = 130;
pub const KEY_RIGHT: u8     = 131;
pub const KEY_DELETE: u8    = 132;
pub const KEY_HOME: u8      = 133;
pub const KEY_END: u8       = 134;

pub struct HistoryRing {
    pub entries: [[u8; 128]; 32],
    pub lengths: [usize; 32],
    pub count: usize,
    pub head: usize,
}

impl HistoryRing {
    pub const fn new() -> Self {
        Self {
            entries: [[0; 128]; 32],
            lengths: [0; 32],
            count: 0,
            head: 0,
        }
    }

    pub fn push(&mut self, cmd: &[u8]) {
        if cmd.is_empty() { return; }
        if self.count > 0 {
            let prev_idx = (self.head + 31) % 32;
            let prev_len = self.lengths[prev_idx];
            if prev_len == cmd.len() && &self.entries[prev_idx][..prev_len] == cmd {
                return;
            }
        }
        let clen = core::cmp::min(cmd.len(), 127);
        self.entries[self.head][..clen].copy_from_slice(&cmd[..clen]);
        self.lengths[self.head] = clen;
        self.head = (self.head + 1) % 32;
        if self.count < 32 {
            self.count += 1;
        }
    }

    pub fn get(&self, offset_from_newest: usize) -> Option<&[u8]> {
        if offset_from_newest >= self.count {
            return None;
        }
        let idx = (self.head + 32 - 1 - offset_from_newest) % 32;
        let len = self.lengths[idx];
        Some(&self.entries[idx][..len])
    }
}

pub static mut DEFAULT_HISTORY: HistoryRing = HistoryRing::new();

pub struct ConsoleReader;

impl ConsoleReader {
    /// Non-blocking query of a pending raw keystroke byte from the console capability.
    pub fn get_raw_key() -> Option<u8> {
        let ep = EndpointCap::from_handle(CapHandle::new(ROOT_SLOT_CONSOLE as u32, 1));
        unsafe {
            let res = invoke_short(ep.0.0, OP_CONSOLE_GETCHAR, 0, 0);
            if res != 0 {
                Some(res as u8)
            } else {
                None
            }
        }
    }

    /// Non-blocking query of a pending keystroke as char.
    pub fn get_char() -> Option<char> {
        Self::get_raw_key().map(|b| b as char)
    }

    /// Reads a line with command history, navigation arrows, cursor positioning, and Ctrl+C.
    /// Returns Some(len) on Enter, or None if cancelled via Ctrl+C.
    pub fn read_line_with_history(buf: &mut [u8], history: &mut HistoryRing) -> Option<usize> {
        let mut len = 0;
        let mut cursor = 0;
        let max_len = buf.len();

        let mut history_idx: Option<usize> = None;
        let mut saved_line = [0u8; 128];
        let mut saved_len = 0;

        loop {
            if let Some(key) = Self::get_raw_key() {
                match key {
                    KEY_CTRL_C => {
                        ConsoleWriter::set_color(0x00E05555, 0x00101418); // Crimson ^C
                        ConsoleWriter::write_str("^C\n");
                        ConsoleWriter::set_color(0x00D8DEE9, 0x00101418);
                        return None;
                    },
                    KEY_ENTER | b'\r' => {
                        ConsoleWriter::write_str("\n");
                        if len > 0 {
                            history.push(&buf[..len]);
                        }
                        return Some(len);
                    },
                    KEY_BACKSPACE => {
                        if cursor > 0 {
                            if cursor == len {
                                len -= 1;
                                cursor -= 1;
                                ConsoleWriter::write_str("\x08");
                            } else {
                                for i in (cursor - 1)..(len - 1) {
                                    buf[i] = buf[i + 1];
                                }
                                len -= 1;
                                cursor -= 1;
                                ConsoleWriter::write_str("\x11");
                                let slice = unsafe { core::str::from_utf8_unchecked(&buf[cursor..len]) };
                                ConsoleWriter::write_str(slice);
                                ConsoleWriter::write_str(" ");
                                for _ in 0..(len - cursor + 1) {
                                    ConsoleWriter::write_str("\x11");
                                }
                            }
                        }
                    },
                    KEY_DELETE => {
                        if cursor < len {
                            for i in cursor..(len - 1) {
                                buf[i] = buf[i + 1];
                            }
                            len -= 1;
                            let slice = unsafe { core::str::from_utf8_unchecked(&buf[cursor..len]) };
                            ConsoleWriter::write_str(slice);
                            ConsoleWriter::write_str(" ");
                            for _ in 0..(len - cursor + 1) {
                                ConsoleWriter::write_str("\x11");
                            }
                        }
                    },
                    KEY_LEFT => {
                        if cursor > 0 {
                            cursor -= 1;
                            ConsoleWriter::write_str("\x11");
                        }
                    },
                    KEY_RIGHT => {
                        if cursor < len {
                            cursor += 1;
                            ConsoleWriter::write_str("\x12");
                        }
                    },
                    KEY_HOME => {
                        while cursor > 0 {
                            cursor -= 1;
                            ConsoleWriter::write_str("\x11");
                        }
                    },
                    KEY_END => {
                        while cursor < len {
                            cursor += 1;
                            ConsoleWriter::write_str("\x12");
                        }
                    },
                    KEY_UP => {
                        if history.count > 0 {
                            let next_idx = match history_idx {
                                None => {
                                    let clen = core::cmp::min(len, 127);
                                    saved_line[..clen].copy_from_slice(&buf[..clen]);
                                    saved_len = clen;
                                    0
                                },
                                Some(curr) => {
                                    if curr + 1 < history.count { curr + 1 } else { curr }
                                }
                            };
                            history_idx = Some(next_idx);
                            if let Some(hist_data) = history.get(next_idx) {
                                while cursor < len {
                                    ConsoleWriter::write_str("\x12");
                                    cursor += 1;
                                }
                                for _ in 0..len {
                                    ConsoleWriter::write_str("\x08");
                                }
                                let clen = core::cmp::min(hist_data.len(), max_len - 1);
                                buf[..clen].copy_from_slice(&hist_data[..clen]);
                                len = clen;
                                cursor = clen;
                                let slice = unsafe { core::str::from_utf8_unchecked(&buf[..len]) };
                                ConsoleWriter::write_str(slice);
                            }
                        }
                    },
                    KEY_DOWN => {
                        if let Some(curr) = history_idx {
                            while cursor < len {
                                ConsoleWriter::write_str("\x12");
                                cursor += 1;
                            }
                            for _ in 0..len {
                                ConsoleWriter::write_str("\x08");
                            }

                            if curr > 0 {
                                let next_idx = curr - 1;
                                history_idx = Some(next_idx);
                                if let Some(hist_data) = history.get(next_idx) {
                                    let clen = core::cmp::min(hist_data.len(), max_len - 1);
                                    buf[..clen].copy_from_slice(&hist_data[..clen]);
                                    len = clen;
                                    cursor = clen;
                                    let slice = unsafe { core::str::from_utf8_unchecked(&buf[..len]) };
                                    ConsoleWriter::write_str(slice);
                                }
                            } else {
                                history_idx = None;
                                let clen = core::cmp::min(saved_len, max_len - 1);
                                buf[..clen].copy_from_slice(&saved_line[..clen]);
                                len = clen;
                                cursor = clen;
                                if len > 0 {
                                    let slice = unsafe { core::str::from_utf8_unchecked(&buf[..len]) };
                                    ConsoleWriter::write_str(slice);
                                }
                            }
                        }
                    },
                    32..=126 => {
                        if len + 1 < max_len {
                            if cursor == len {
                                buf[len] = key;
                                len += 1;
                                cursor += 1;
                                let s = [key];
                                let slice = unsafe { core::str::from_utf8_unchecked(&s) };
                                ConsoleWriter::write_str(slice);
                            } else {
                                for i in (cursor..len).rev() {
                                    buf[i + 1] = buf[i];
                                }
                                buf[cursor] = key;
                                len += 1;
                                let slice = unsafe { core::str::from_utf8_unchecked(&buf[cursor..len]) };
                                ConsoleWriter::write_str(slice);
                                cursor += 1;
                                for _ in 0..(len - cursor) {
                                    ConsoleWriter::write_str("\x11");
                                }
                            }
                        }
                    },
                    _ => {}
                }
            } else {
                unsafe { core::arch::asm!("pause"); }
            }
        }
    }

    /// Reads a password from the console, optionally masking characters with '*'.
    /// Returns Some(len) on Enter, or None if aborted with Ctrl+C.
    pub fn read_password(buf: &mut [u8], mask: bool) -> Option<usize> {
        let mut len = 0;
        let max_len = buf.len();

        loop {
            if let Some(b) = Self::get_raw_key() {
                match b {
                    b'\r' | b'\n' => {
                        ConsoleWriter::write_str("\n");
                        return Some(len);
                    },
                    3 => { // Ctrl+C
                        ConsoleWriter::write_str("^C\n");
                        return None;
                    },
                    8 | 127 => { // Backspace
                        if len > 0 {
                            len -= 1;
                            if mask {
                                ConsoleWriter::write_str("\x08 \x08");
                            }
                        }
                    },
                    32..=126 => { // Printable ASCII
                        if len < max_len {
                            buf[len] = b;
                            len += 1;
                            if mask {
                                ConsoleWriter::write_str("*");
                            }
                        }
                    },
                    _ => {}
                }
            } else {
                unsafe { core::arch::asm!("pause"); }
            }
        }
    }

    /// Reads a complete line of text using the default command history.
    pub fn read_line(buf: &mut [u8]) -> usize {
        Self::read_line_with_history(buf, unsafe { &mut *core::ptr::addr_of_mut!(DEFAULT_HISTORY) }).unwrap_or(0)
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
