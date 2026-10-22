//! Rust producer for the Type Facts v3 lifecycle protocol.
//!
//! S1 of the tsrs migration: everything that does not need a compiler. The
//! process surface (flags, handshake, framing, validation, error codes,
//! threads, cancellation, crash hooks, timings trace) matches the Go producer
//! in apps/solid-typefacts; the compiler arrives behind [`backend::Backend`].

pub mod arena;
pub mod args;
pub mod backend;
pub mod context;
pub mod fake;
pub mod frame;
pub mod gocompat;
pub mod run;
pub mod serve;
pub mod session;
pub mod trace;
pub mod wire;
