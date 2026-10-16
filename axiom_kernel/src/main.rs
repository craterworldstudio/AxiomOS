// ============================================================================
// Axiom Kernel — Entry-Point Dispatcher
//
//   ./build.sh        — Dev build   (main_dev.rs)  [default]
//   ./build.sh -prod  — Prod build  (main_prod.rs)
//
// Controlled by the Cargo feature "prod-mode".
// Do NOT put kernel logic here — edit main_dev.rs or main_prod.rs.
// ============================================================================
#![no_std]
#![no_main]
#![allow(static_mut_refs, dead_code, unused_variables, unused_unsafe, unused_imports)]

mod interrupts;
mod gdt;
mod memory;
mod object;
mod capability;
mod thread;
mod scheduler;
mod console;
mod apic;
mod syscall;

#[inline(always)]
pub unsafe fn serial_out(b: u8) {
    core::arch::asm!("out dx, al", in("dx") 0x3F8_u16, in("al") b, options(nomem, nostack));
}

pub unsafe fn serial_print(s: &str) {
    for b in s.bytes() {
        serial_out(b);
    }
}

pub unsafe fn serial_print_hex(val: u64) {
    let hex_chars = b"0123456789ABCDEF";
    serial_print("0x");
    for i in (0..16).rev() {
        let nibble = ((val >> (i * 4)) & 0xF) as usize;
        serial_out(hex_chars[nibble]);
    }
}

#[cfg(not(feature = "prod-mode"))]
include!("main_dev.rs");

#[cfg(feature = "prod-mode")]
include!("main_prod.rs");
