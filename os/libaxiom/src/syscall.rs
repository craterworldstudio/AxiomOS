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
pub const OP_CONSOLE_GETCHAR: u64 = 10;
pub const OP_CONSOLE_SET_CURSOR: u64 = 11;
pub const OP_TASK_SPAWN: u64 = 12;
pub const OP_TASK_EXIT: u64  = 13;
pub const OP_TASK_WAIT: u64  = 14;
pub const OP_TASK_LIST: u64  = 15;
pub const OP_TASK_KILL: u64  = 16;
pub const OP_STORAGE_READ: u64   = 20;
pub const OP_STORAGE_WRITE: u64  = 21;
pub const OP_CHANNEL_CREATE: u64 = 22;
pub const OP_CHANNEL_READ: u64   = 23;
pub const OP_CHANNEL_WRITE: u64  = 24;
pub const OP_SYS_REBOOT: u64     = 30;
pub const OP_SYS_SHUTDOWN: u64   = 31;
pub const OP_KERNEL_PANIC: u64   = 99;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct ProcessInfo {
    pub pid: u32,
    pub ppid: u32,
    pub state: u32,
    pub _pad: u32,
    pub cr3: u64,
    pub name: [u8; 32],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct TaskSpawnParams {
    pub binary_ptr: u64,
    pub binary_len: u64,
    pub user_id: u32,
    pub rights: u32,
    pub storage_cap: u64,
    pub console_cap: u64,
    pub user_name: [u8; 32],
    pub map_ctx_ptr: u64,
    pub blob_ctx_ptr: u64,
    pub argc: u64,
    pub argv_ptrs: [u64; 8],
    pub stdin_cap: u64,
}


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

/// Spawns a child process in an isolated Ring 3 virtual address space (CR3) with attenuated authority.
#[inline(always)]
pub unsafe fn task_spawn(params: &TaskSpawnParams) -> Result<u32, SyscallError> {
    let res = invoke_short(
        0x100000003, // ROOT_SLOT_TCB
        OP_TASK_SPAWN,
        params as *const TaskSpawnParams as u64,
        0,
    );
    if res > 0 && res < 0x8000_0000 {
        Ok(res as u32)
    } else {
        Err(SyscallError::from_code(res).err().unwrap_or(SyscallError::Unknown(res)))
    }
}

/// Waits for a child process to exit, optionally retrieving written-back blob data.
#[inline(always)]
pub unsafe fn task_wait(child_pid: u32, out_blob: *mut u8) -> Result<u64, SyscallError> {
    let res = invoke_short(
        0x100000003, // ROOT_SLOT_TCB
        OP_TASK_WAIT,
        child_pid as u64,
        out_blob as u64,
    );
    Ok(res)
}

/// Exits the current task with an exit status code.
#[inline(always)]
pub unsafe fn task_exit(code: i32) -> ! {
    invoke_short(
        0x100000003,
        OP_TASK_EXIT,
        code as u64,
        0,
    );
    loop {
        core::arch::asm!("pause");
    }
}

/// Queries active processes in the system.
#[inline(always)]
pub unsafe fn task_list(out_buf: *mut ProcessInfo, max_count: u32) -> Result<u32, SyscallError> {
    let res = invoke_short(
        0x100000003, // ROOT_SLOT_TCB
        OP_TASK_LIST,
        out_buf as u64,
        max_count as u64,
    );
    Ok(res as u32)
}

/// Terminates a process by PID.
#[inline(always)]
pub unsafe fn task_kill(pid: u32) -> Result<(), SyscallError> {
    let res = invoke_short(
        0x100000003, // ROOT_SLOT_TCB
        OP_TASK_KILL,
        pid as u64,
        9, // SIGKILL
    );
    if res == SYSCALL_OK {
        Ok(())
    } else {
        Err(SyscallError::from_code(res).err().unwrap_or(SyscallError::Unknown(res)))
    }
}

pub const ROOT_SLOT_STDIN: u64 = 0x100000006;

/// Creates an IPC channel and returns (read_cap, write_cap).
#[inline(always)]
pub unsafe fn channel_create() -> Result<(crate::capability::CapHandle, crate::capability::CapHandle), SyscallError> {
    let mut handles: [u64; 2] = [0, 0];
    let res = invoke_short(
        0x100000001, // ROOT_SLOT_CNODE
        OP_CHANNEL_CREATE,
        handles.as_mut_ptr() as u64,
        0,
    );
    if res == SYSCALL_OK {
        Ok((crate::capability::CapHandle(handles[0]), crate::capability::CapHandle(handles[1])))
    } else {
        Err(SyscallError::from_code(res).err().unwrap_or(SyscallError::Unknown(res)))
    }
}


