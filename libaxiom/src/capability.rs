//! AxiomOS Strongly Typed Capability Handles & Rights

use crate::syscall::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rights(pub u8);

impl Rights {
    pub const NONE: Rights  = Rights(0b0000);
    pub const READ: Rights  = Rights(0b0001);
    pub const WRITE: Rights = Rights(0b0010);
    pub const EXEC: Rights  = Rights(0b0100);
    pub const GRANT: Rights = Rights(0b1000);
    pub const ALL: Rights   = Rights(0b1111);

    #[inline(always)]
    pub fn contains(&self, other: Rights) -> bool {
        (self.0 & other.0) == other.0
    }

    #[inline(always)]
    pub fn is_subset(&self, parent: Rights) -> bool {
        parent.contains(*self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct CapHandle(pub u64);

impl CapHandle {
    pub const fn new(index: u32, generation: u32) -> Self {
        Self(((generation as u64) << 32) | (index as u64))
    }

    pub fn index(&self) -> u32 {
        (self.0 & 0xFFFFFFFF) as u32
    }

    pub fn generation(&self) -> u32 {
        (self.0 >> 32) as u32
    }

    pub fn is_null(&self) -> bool {
        self.0 == 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectKind {
    Null,
    Endpoint,
    TCB,
    VSpace,
    CNode,
    Untyped,
}

impl ObjectKind {
    pub fn as_u64(&self) -> u64 {
        match self {
            Self::Null => 0,
            Self::Endpoint => 1,
            Self::TCB => 2,
            Self::VSpace => 3,
            Self::CNode => 4,
            Self::Untyped => 5,
        }
    }
}

/// An Endpoint capability handle for synchronous IPC.
#[derive(Debug, Clone, Copy)]
pub struct EndpointCap(pub CapHandle);

impl EndpointCap {
    pub fn from_handle(handle: CapHandle) -> Self {
        Self(handle)
    }

    /// Performs an atomic RPC call: delivers `payload` to a receiver and blocks waiting for reply.
    pub fn call(&self, payload: u64) -> Result<u64, SyscallError> {
        unsafe {
            let res = invoke_short(self.0.0, OP_CALL, payload, 0);
            SyscallError::from_code(res)
        }
    }

    /// Sends a message payload to a receiver waiting on this endpoint.
    pub fn send(&self, payload: u64) -> Result<(), SyscallError> {
        unsafe {
            let res = invoke_short(self.0.0, OP_SEND, payload, 0);
            SyscallError::from_code(res).map(|_| ())
        }
    }

    /// Receives a message payload from a sender waiting on this endpoint.
    pub fn recv(&self) -> Result<u64, SyscallError> {
        unsafe {
            let res = invoke_short(self.0.0, OP_RECEIVE, 0, 0);
            SyscallError::from_code(res)
        }
    }
}

/// A CNode capability handle for capability management (mint, revoke, delete).
#[derive(Debug, Clone, Copy)]
pub struct CNodeCap(pub CapHandle);

impl CNodeCap {
    pub fn from_handle(handle: CapHandle) -> Self {
        Self(handle)
    }

    /// Mints a derived capability from this capability into `dest_slot` with attenuated rights.
    pub fn mint(&self, dest_slot: usize, rights: Rights) -> Result<CapHandle, SyscallError> {
        unsafe {
            let res = invoke_short(self.0.0, OP_MINT, dest_slot as u64, rights.0 as u64);
            // On success, kernel returns the new CapHandle encoded as u64
            if res <= 8 {
                SyscallError::from_code(res).map(|_| CapHandle(0))
            } else {
                Ok(CapHandle(res))
            }
        }
    }

    /// Recursively revokes all descendant capabilities derived from `slot`.
    pub fn revoke(&self, slot: usize) -> Result<(), SyscallError> {
        unsafe {
            let res = invoke_short(self.0.0, OP_REVOKE, slot as u64, 0);
            SyscallError::from_code(res).map(|_| ())
        }
    }

    /// Deletes the capability in `slot`, unlinking it and revoking its children.
    pub fn delete(&self, slot: usize) -> Result<(), SyscallError> {
        unsafe {
            let res = invoke_short(self.0.0, OP_DELETE, slot as u64, 0);
            SyscallError::from_code(res).map(|_| ())
        }
    }
}
