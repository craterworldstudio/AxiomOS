// ============================================================================
// Axiom Kernel — Local APIC (LAPIC) & Calibrated Preemption Timer Driver
// ============================================================================
use core::arch::asm;

pub const TIMER_VECTOR: u8 = 0x20; // IDT Vector 32
pub const SPURIOUS_VECTOR: u8 = 0xFF; // IDT Vector 255

// LAPIC Register Offsets
const LAPIC_ID: u32 = 0x020;
const LAPIC_VERSION: u32 = 0x030;
const LAPIC_TPR: u32 = 0x080;
const LAPIC_EOI: u32 = 0x0B0;
const LAPIC_SVR: u32 = 0x0F0;
const LAPIC_ESR: u32 = 0x280;
const LAPIC_LVT_TIMER: u32 = 0x320;
const LAPIC_TIMER_INIT_COUNT: u32 = 0x380;
const LAPIC_TIMER_CURR_COUNT: u32 = 0x390;
const LAPIC_TIMER_DIV: u32 = 0x3E0;

// LAPIC Flags
const SVR_APIC_ENABLE: u32 = 1 << 8;
const LVT_TIMER_PERIODIC: u32 = 1 << 17;
const LVT_MASKED: u32 = 1 << 16;
const DIVIDE_BY_16: u32 = 0x03;

pub static mut LAPIC_BASE_PHYS: u64 = 0;
pub static mut LAPIC_BASE_VIRT: u64 = 0;
pub static mut TIMER_TICKS_PER_10MS: u32 = 0;
pub static mut TIMER_TICKS_TOTAL: u64 = 0;

#[inline(always)]
pub unsafe fn read_lapic(offset: u32) -> u32 {
    let ptr = (LAPIC_BASE_VIRT + offset as u64) as *const u32;
    core::ptr::read_volatile(ptr)
}

#[inline(always)]
pub unsafe fn write_lapic(offset: u32, value: u32) {
    let ptr = (LAPIC_BASE_VIRT + offset as u64) as *mut u32;
    core::ptr::write_volatile(ptr, value);
}

#[inline(always)]
pub unsafe fn eoi() {
    write_lapic(LAPIC_EOI, 0);
}

#[inline(always)]
unsafe fn inb(port: u16) -> u8 {
    let value: u8;
    asm!("in al, dx", in("dx") port, out("al") value, options(nomem, nostack, preserves_flags));
    value
}

#[inline(always)]
unsafe fn outb(port: u16, value: u8) {
    asm!("out dx, al", in("dx") port, in("al") value, options(nomem, nostack, preserves_flags));
}

/// Explicitly masks all IRQs on the legacy 8259 dual-PIC.
pub unsafe fn mask_legacy_pic() {
    outb(0x21, 0xFF); // Master PIC IMR
    outb(0xA1, 0xFF); // Slave PIC IMR
}

/// Detects, validates, and initializes the Local APIC.
/// Returns true if successfully enabled in xAPIC MMIO mode.
pub unsafe fn init() -> bool {
    // 1. Mask legacy PIC to avoid spurious ISA interrupts
    mask_legacy_pic();

    // 2. Read IA32_APIC_BASE MSR (0x1B)
    let eax: u32;
    let edx: u32;
    asm!("rdmsr", in("ecx") 0x1Bu32, out("eax") eax, out("edx") edx, options(nomem, nostack));
    let msr = ((edx as u64) << 32) | (eax as u64);

    let is_bsp = (msr & (1 << 8)) != 0;
    let global_enable = (msr & (1 << 11)) != 0;
    let x2apic_enable = (msr & (1 << 10)) != 0;
    let base_phys = msr & 0x000F_FFFF_FFFF_F000;

    LAPIC_BASE_PHYS = base_phys;
    // Under Axiom identity mapping, the MMIO physical address matches virtual address
    LAPIC_BASE_VIRT = base_phys;

    // Verify xAPIC mode (x2APIC not enabled)
    if x2apic_enable {
        // x2APIC requires MSR accesses; our platform target is xAPIC MMIO
        return false;
    }

    // Ensure global enable bit is set
    if !global_enable {
        let new_msr = msr | (1 << 11);
        let new_eax = new_msr as u32;
        let new_edx = (new_msr >> 32) as u32;
        asm!("wrmsr", in("ecx") 0x1Bu32, in("eax") new_eax, in("edx") new_edx, options(nomem, nostack));
    }

    // 3. Enable Spurious Interrupt Vector register with Software Enable bit (bit 8)
    write_lapic(LAPIC_SVR, SVR_APIC_ENABLE | (SPURIOUS_VECTOR as u32));

    // Clear Task Priority Register to accept all interrupts
    write_lapic(LAPIC_TPR, 0);

    // 4. Calibrate the LAPIC Timer against PIT Channel 2
    calibrate_timer();

    // 5. Arm the LAPIC Timer in Periodic Mode with calibrated ticks
    let count = TIMER_TICKS_PER_10MS;
    if count == 0 {
        return false;
    }

    write_lapic(LAPIC_TIMER_DIV, DIVIDE_BY_16);
    // Program Initial Count
    write_lapic(LAPIC_TIMER_INIT_COUNT, count);
    // Unmask timer in Periodic Mode on TIMER_VECTOR (0x20)
    write_lapic(LAPIC_LVT_TIMER, LVT_TIMER_PERIODIC | (TIMER_VECTOR as u32));

    true
}

/// Calibrates LAPIC timer ticks over a 10 millisecond interval using PIT Channel 2.
unsafe fn calibrate_timer() {
    // PIT operates at 1,193,182 Hz. 10 ms = ~11,932 PIT ticks.
    const PIT_10MS_TICKS: u16 = 11932;

    // 1. Configure LAPIC timer: divide by 16, masked
    write_lapic(LAPIC_TIMER_DIV, DIVIDE_BY_16);
    write_lapic(LAPIC_LVT_TIMER, LVT_MASKED);

    // 2. Set up PIT Channel 2 in one-shot mode (Mode 0)
    // Port 0x61: bits 0 & 1 control Channel 2 gate and speaker
    let mut port_61 = inb(0x61);
    port_61 &= !0x03; // Disable speaker and gate
    outb(0x61, port_61);

    // Send control word: Channel 2, LSB then MSB, Mode 0, Binary
    outb(0x43, 0b1011_0000);

    // Write counter value (11932)
    outb(0x42, (PIT_10MS_TICKS & 0xFF) as u8);
    outb(0x42, ((PIT_10MS_TICKS >> 8) & 0xFF) as u8);

    // 3. Start LAPIC countdown from max value
    write_lapic(LAPIC_TIMER_INIT_COUNT, 0xFFFF_FFFF);

    // 4. Enable PIT Channel 2 gate (bit 0 = 1, bit 1 = 0)
    port_61 = (inb(0x61) & !0x02) | 0x01;
    outb(0x61, port_61);

    // 5. Poll until PIT Channel 2 OUT bit (bit 5) goes high
    let mut timeout: u32 = 0;
    while (inb(0x61) & 0x20) == 0 {
        timeout += 1;
        if timeout > 10_000_000 {
            break;
        }
    }

    // 6. Read elapsed LAPIC ticks
    let current = read_lapic(LAPIC_TIMER_CURR_COUNT);
    // Mask LVT timer
    write_lapic(LAPIC_LVT_TIMER, LVT_MASKED);

    // Disable PIT Channel 2
    port_61 = inb(0x61) & !0x01;
    outb(0x61, port_61);

    let elapsed = 0xFFFF_FFFF_u32.saturating_sub(current);
    if elapsed > 1000 {
        TIMER_TICKS_PER_10MS = elapsed;
    } else {
        // Fallback to reasonable 10ms estimate if PIT hardware is unrouted
        TIMER_TICKS_PER_10MS = 100_000;
    }
}
