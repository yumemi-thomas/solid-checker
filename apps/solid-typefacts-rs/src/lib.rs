//! Rust producer for the Type Facts v3 lifecycle protocol.
//!
//! Everything that does not need a compiler. The process surface (flags,
//! handshake, framing, validation, error codes, threads, cancellation, crash
//! hooks, timings trace) and the compiler-independent core (session, demand
//! closure, retained demands and contributions, symbol store) match the Go
//! producer in apps/solid-typefacts; the compiler arrives behind
//! [`backend::Backend`].

pub mod arena;
pub mod args;
pub mod backend;
pub mod closure;
pub mod constants;
pub mod context;
pub mod contributions;
pub mod demands;
pub mod durable;
pub mod fake;
pub mod frame;
pub mod gocompat;
pub mod identity;
pub mod import_graph;
pub mod model;
pub mod module_graph;
pub mod nodes;
pub mod overlay;
pub mod project;
pub mod reactor;
pub mod references;
pub mod resolved_calls;
pub mod retained;
pub mod run;
pub mod runtime_facts;
pub mod semantic_runs;
pub mod serve;
pub mod session;
pub mod shape;
pub mod source_facts;
pub mod symbols;
pub mod trace;
pub mod type_facts;
pub mod unsafe_boundary;
pub mod wire;

#[cfg(test)]
mod tests;
