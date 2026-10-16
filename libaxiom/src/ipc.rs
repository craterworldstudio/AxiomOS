//! AxiomOS User-Space Synchronous IPC Layer

use crate::capability::{CapHandle, EndpointCap};
use crate::syscall::SyscallError;

/// Synchronous atomic capability RPC: sends `payload` and blocks until the server replies.
#[inline(always)]
pub fn rpc_call(endpoint: CapHandle, payload: u64) -> Result<u64, SyscallError> {
    EndpointCap(endpoint).call(payload)
}

/// Synchronous capability send.
#[inline(always)]
pub fn ipc_send(endpoint: CapHandle, payload: u64) -> Result<(), SyscallError> {
    EndpointCap(endpoint).send(payload)
}

/// Synchronous capability receive.
#[inline(always)]
pub fn ipc_recv(endpoint: CapHandle) -> Result<u64, SyscallError> {
    EndpointCap(endpoint).recv()
}
