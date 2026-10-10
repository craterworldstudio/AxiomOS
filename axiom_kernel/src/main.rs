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

mod interrupts;
mod gdt;
mod memory;
mod object;
mod capability;
mod thread;
mod scheduler;

#[cfg(not(feature = "prod-mode"))]
include!("main_dev.rs");

#[cfg(feature = "prod-mode")]
include!("main_prod.rs");
