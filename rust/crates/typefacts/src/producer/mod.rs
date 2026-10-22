//! The producer side of the table-transition wire (feature `producer`).
//!
//! The client decodes transitions in `v3.rs`; this module writes them, sharing
//! the row definitions in [`crate::v3::rows`]. It ports the Go producer's
//! `wire_transition*.go`, `facttable*.go` and `transport_manifest.go`:
//!
//! * [`ProducerTable`] is the table a transition describes;
//! * [`TransitionEncoder`] plans a full, delta, or sparse transition and writes
//!   its string table, packed rows, and header, optionally streaming the rows
//!   into a [`TransitionSink`] (the transition arena);
//! * the manifest functions name the rows that may differ between tables.
//!
//! This crate keeps `unsafe` inside `unsafe_boundary`; [`disable_file_cache`]
//! is the safe handle on its one call, so the producer workspace, which
//! forbids `unsafe`, can still ask macOS not to cache its arena file.

mod compact;
mod encoder;
mod manifest;
mod plan;
mod rows;
mod table;
mod writer;

#[cfg(test)]
mod tests;

pub use crate::v3::TransitionMode;
pub use compact::{
    append_compact_demands_with_flag, compact_demand_shape, expand_compact_demands,
    expand_compact_group,
};
pub use encoder::{
    EncodeError, EncodedTransition, TransitionEncoder, TransitionInput, TransitionSink,
    TransitionSummary, inspect_table_transition, reencode_table_transition,
};
pub use manifest::{
    ChangedClosure, TransportChanges, retained_symbol_candidates, rust_owned_transport_manifest,
    transport_manifest,
};
pub use table::{EntityRows, ProducerTable, source_digest};
pub use writer::wire_symbol_name_bytes;

/// Asks macOS not to keep the file's pages in the unified buffer cache; a
/// no-op elsewhere. The transition arena is transient transport, not cached
/// state, and the Go producer makes the same call on its side.
pub fn disable_file_cache(file: &std::fs::File) -> std::io::Result<()> {
    #[cfg(target_os = "macos")]
    {
        crate::unsafe_boundary::disable_file_cache(file)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = file;
        Ok(())
    }
}
