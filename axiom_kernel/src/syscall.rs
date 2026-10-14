// ============================================================================
// Axiom Kernel — Hardware Fast-Path System Call Gateway (syscall / sysretq)
// ============================================================================
use core::arch::asm;

pub const IA32_EFER: u32           = 0xC000_0080;
pub const IA32_STAR: u32           = 0xC000_0081;
pub const IA32_LSTAR: u32          = 0xC000_0082;
pub const IA32_SFMASK: u32         = 0xC000_0084;
pub const IA32_KERNEL_GS_BASE: u32 = 0xC000_0102;

#[repr(C, align(64))]
pub struct CpuLocal {
    pub user_rsp: u64,
    pub kernel_rsp: u64,
    pub scratch: u64,
}

pub static mut CPU_LOCAL: CpuLocal = CpuLocal {
    user_rsp: 0,
    kernel_rsp: 0,
    scratch: 0,
};

#[inline(always)]
pub unsafe fn wrmsr(msr: u32, value: u64) {
    let eax = value as u32;
    let edx = (value >> 32) as u32;
    asm!("wrmsr", in("ecx") msr, in("eax") eax, in("edx") edx, options(nomem, nostack));
}

#[inline(always)]
pub unsafe fn rdmsr(msr: u32) -> u64 {
    let eax: u32;
    let edx: u32;
    asm!("rdmsr", in("ecx") msr, out("eax") eax, out("edx") edx, options(nomem, nostack));
    ((edx as u64) << 32) | (eax as u64)
}

pub unsafe fn set_cpu_kernel_stack(rsp0: u64) {
    CPU_LOCAL.kernel_rsp = rsp0;
}

/// Initializes hardware fast-path system call MSRs and per-CPU state.
pub unsafe fn init() {
    extern "C" {
        fn syscall_entry();
    }

    // 1. Enable System Call Extensions (SCE) in IA32_EFER (bit 0)
    let efer = rdmsr(IA32_EFER);
    wrmsr(IA32_EFER, efer | 1);

    // 2. Program IA32_STAR:
    // Bits 47:32 = Kernel CS (0x08), Kernel SS is implicitly 0x10.
    // Bits 63:48 = User CS/SS base (0x10).
    // On sysretq: User CS = (0x10 + 16) | 3 = 0x23, User SS = (0x10 + 8) | 3 = 0x1B.
    let star = (0x0010_u64 << 48) | (0x0008_u64 << 32);
    wrmsr(IA32_STAR, star);

    // 3. Program IA32_LSTAR with 64-bit target entry point
    let lstar = syscall_entry as *const () as u64;
    wrmsr(IA32_LSTAR, lstar);

    // 4. Program IA32_SFMASK: mask out IF (bit 9), DF (bit 10), TF (bit 8), AC, NT, IOPL
    // Mask: 0x47700 ensures interrupts are disabled upon entering kernel mode
    wrmsr(IA32_SFMASK, 0x0004_7700);

    // 5. Initialize IA32_KERNEL_GS_BASE to point to our CpuLocal structure
    wrmsr(IA32_KERNEL_GS_BASE, (&raw const CPU_LOCAL) as u64);
}
