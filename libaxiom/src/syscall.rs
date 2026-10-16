//! AxiomOS User-Space Hardware Fast-Path System Call Gateway

pub const OP_SEND: u64    = 0;
pub const OP_RECEIVE: u64 = 1;
pub const OP_CALL: u64    = 2;
pub const OP_MINT: u64    = 3;
pub const OP_REVOKE: u64  = 4;
pub const OP_DELETE: u64  = 5;
pub const OP_CONSOLE_WRITE: u64  = 6;
pub const OP_CONSOLE_CLEAR: u64  = 7;
pub const OP_CONSOLE_BANNER: u64 = 8;
pub const OP_CONSOLE_COLOR: u64  = 9;

pub const SYSCALL_OK: u64         = 0;
pub const ERR_INVALID_HANDLE: u64 = 1;
pub const ERR_STALE_HANDLE: u64   = 2;
pub const ERR_INVALID_RIGHTS: u64 = 3;
pub const ERR_INVALID_KIND: u64   = 4;
pub const ERR_UNSUPPORTED_OP: u64 = 5;
pub const ERR_CNODE_FULL: u64     = 6;
pub const ERR_SLOT_OCCUPIED: u64  = 7;
pub const ERR_NO_RECEIVER: u64    = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyscallError {
    InvalidHandle,
    StaleHandle,
    InvalidRights,
    InvalidKind,
    UnsupportedOp,
    CNodeFull,
    SlotOccupied,
    NoReceiver,
    Unknown(u64),
}

impl SyscallError {
    pub fn from_code(code: u64) -> Result<u64, Self> {
        match code {
            SYSCALL_OK => Ok(SYSCALL_OK),
            ERR_INVALID_HANDLE => Err(Self::InvalidHandle),
            ERR_STALE_HANDLE => Err(Self::StaleHandle),
            ERR_INVALID_RIGHTS => Err(Self::InvalidRights),
            ERR_INVALID_KIND => Err(Self::InvalidKind),
            ERR_UNSUPPORTED_OP => Err(Self::UnsupportedOp),
            ERR_CNODE_FULL => Err(Self::CNodeFull),
            ERR_SLOT_OCCUPIED => Err(Self::SlotOccupied),
            ERR_NO_RECEIVER => Err(Self::NoReceiver),
            other => Err(Self::Unknown(other)),
        }
    }
}

/// Invokes a kernel capability operation via the x86-64 hardware `syscall` instruction.
///
/// Calling Convention:
/// - RDI: Capability handle
/// - RSI: Operation code (OP_SEND, OP_RECEIVE, OP_CALL, etc.)
/// - RDX: Argument 1
/// - R10: Argument 2 (passed in R10 because hardware syscall destroys RCX)
/// - R8:  Argument 3
/// - R9:  Argument 4
/// - RAX: Return value from kernel
#[inline(always)]
pub unsafe fn invoke(handle: u64, operation: u64, arg1: u64, arg2: u64, arg3: u64, arg4: u64) -> u64 {
    let result: u64;
    core::arch::asm!(
        "syscall",
        in("rdi") handle,
        in("rsi") operation,
        in("rdx") arg1,
        in("r10") arg2,
        in("r8")  arg3,
        in("r9")  arg4,
        lateout("rax") result,
        out("rcx") _, // Clobbered by CPU (saves RIP)
        out("r11") _, // Clobbered by CPU (saves RFLAGS)
        options(nostack)
    );
    result
}

/// Simplified 2-argument capability invocation helper.
#[inline(always)]
pub unsafe fn invoke_short(handle: u64, operation: u64, arg1: u64, arg2: u64) -> u64 {
    invoke(handle, operation, arg1, arg2, 0, 0)
}
