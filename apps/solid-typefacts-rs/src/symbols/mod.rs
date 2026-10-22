//! The symbol machinery of the demand closure (Go: `symbol_intern.go`,
//! `symbol_store.go`, `symbol_closure.go`).
//!
//! In the active protocol the closure leaves symbol closure to the client, so
//! the producer's demand table carries no symbol rows; this machinery is
//! retained, as in Go, for the closure builder the transport manifest was
//! designed around and its tests. Nothing on the `analyze` path builds a
//! [`closure::ClosureBuilder`].

pub mod closure;
pub mod intern;
pub mod store;
