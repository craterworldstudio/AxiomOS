#![no_std]
#![no_main]

use core::panic::PanicInfo;

static HELLO: &[u8] = b"AXIOM KERNEL ONLINE";

#[no_mangle]
pub extern "C" fn _start() -> ! {
    let vga_buffer = 0xb8000 as *mut u8;

    for (i, &byte) in HELLO.iter().enumerate() {
        unsafe {
            // Write the character byte
            *vga_buffer.offset(i as isize * 2) = byte;
            // Write the color byte (0xA = Light Green foreground, 0x0 = Black background)
            *vga_buffer.offset(i as isize * 2 + 1) = 0x0A;
        }
    }

    // The universe is online. Halt.
    loop {}
}

/// This function is called on kernel panic.
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}