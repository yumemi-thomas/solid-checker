//! The checker's fact model: every fact domain the analysis cross-references.
//!
//! Each module owns one fact domain. `core` holds the identities shared by
//! all of them (spans, source paths and hashes, generations); `ast` holds Oxc
//! syntax facts; `compiler` holds the execution map produced by a Solid JSX
//! compiler together with the provider seam dialects implement; `resolution`
//! holds the compiler's own answer for where each import specifier resolves;
//! the crate root joins the domains with Type Facts into per-file and
//! per-project facts.

pub mod ast;
pub mod compiler;
pub mod core;
pub mod resolution;

mod project;

pub use core::resolve_relative_module_path;
pub use project::{
    FileFacts, JoinError, ProjectFacts, TypeScriptChanges, TypeScriptSymbol, TypeScriptTable,
};
pub use resolution::{AttestedImport, AttestedImportIndex, ImportResolution, SpecifierAttestation};
pub use typefacts;
