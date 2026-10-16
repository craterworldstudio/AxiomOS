//! # libaxiom
//!
//! AxiomOS User-Space Runtime & Capability Client Library.
//!
//! `libaxiom` is the standard `#![no_std]` foundation for all Ring 3 tasks,
//! operating system services, servers, and applications running on AxiomOS.
//! It encapsulates the hardware fast-path system call gateway (`syscall` / `sysretq`),
//! strongly typed capability handles, synchronous IPC rendezvous (`OP_CALL`),
//! and safe user-space memory management without ambient authority.

#![no_std]

pub mod syscall;
pub mod capability;
pub mod ipc;
pub mod allocator;
pub mod console;

pub use syscall::{SyscallError, invoke, invoke_short};
pub use capability::{CapHandle, Rights, ObjectKind, EndpointCap, CNodeCap};
pub use ipc::{rpc_call, ipc_send, ipc_recv};
pub use allocator::BumpAllocator;
pub use console::ConsoleWriter;

// ============================================================================
// ROOT CAPABILITY SLOTS (Genesis Process CSpace Initial Layout)
// ============================================================================
pub const ROOT_SLOT_CNODE: usize   = 1; // Self Root CNode handle
pub const ROOT_SLOT_VSPACE: usize  = 2; // Self Root VSpace (PML4) handle
pub const ROOT_SLOT_TCB: usize     = 3; // Self Root TCB handle
pub const ROOT_SLOT_UNTYPED: usize = 4; // Root Untyped memory authority handle
pub const ROOT_SLOT_CONSOLE: usize = 5; // System console / video endpoint handle
