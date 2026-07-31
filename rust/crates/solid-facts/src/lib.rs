//! The checker's fact model: every fact domain the analysis cross-references.
//!
//! Each module owns one fact domain. `core` holds the identities shared by
//! all of them (spans, source paths and hashes, generations); `ast` holds Oxc
//! syntax facts; `compiler` holds the execution map produced by a Solid JSX
//! compiler together with the provider seam dialects implement; the crate
//! root joins the domains with Type Facts into per-file and per-project facts.

pub mod ast;
pub mod compiler;
pub mod core;

mod project;

pub use project::*;
pub use typefacts;
