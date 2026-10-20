//! The Solid dialect: one interface over the two language versions the checker
//! certifies.
//!
//! Everything version-specific about Solid's *vocabulary* lives here — which
//! names are primitives, which argument of a call is its callback, which JSX
//! tags open a boundary, which primitives may not be created under a leaf
//! owner. The reactive engine asks; it does not know.
//!
//! See ADR 0006, "Reopen the Solid version seam". The decision this crate
//! exists to make possible: one engine, two dialects, rather than one engine
//! per branch.
//!
//! # What does not belong here
//!
//! Syntax. A JSX attribute is a JSX attribute in both dialects, so
//! `solid-ast-facts` takes no dialect. If a rule needs dialect knowledge at the
//! syntax layer, the rule is in the wrong tier.
//!
//! Module map (every public path is re-exported at the crate root):
//!
//! | Module | Responsibility |
//! | --- | --- |
//! | `version` | `Version`, classification of an installed `solid-js`, and the installation review types. |
//! | `vocabulary` | `Primitive` and the small role enums it is described with. |
//! | `packages` | Package-name predicates over the carried dialects and `HostTargetCondition`. |
//! | `semantics` | Callback, owner, boundary and type-role enums and records a dialect answers with. |
//! | `dialect` | The `Dialect` trait. |
//! | `solid_2`, `exports` | The Solid 2.0 vocabulary and the generated export index. |

#![forbid(unsafe_code)]

pub mod exports;
mod solid_2;

mod version;
pub use version::*;
mod vocabulary;
pub use vocabulary::*;
mod packages;
pub use packages::*;
mod semantics;
pub use semantics::*;
mod dialect;
pub use dialect::*;

pub use exports::Position as ExportPosition;
pub use solid_2::Solid2;

/// Every vocabulary this build carries, in ascending version order.
///
/// The cross-dialect helpers below ask their question of each entry, and the
/// tests that loop it assert one property per vocabulary. Both iterate *this*
/// rather than enumerating [`Version`]
/// because the question is "what do the vocabularies I have say", which is not
/// the same question as "which Solid versions exist" — [`Version::V1`] is
/// retained for classification long after a build stops carrying its
/// vocabulary, so detection can recognise an installed 1.x runtime in order to
/// refuse it rather than silently analyzing it as 2.0 (ADR 0110).
///
/// Retiring a dialect is therefore an edit *here*, plus deleting whatever is
/// genuinely differential. Nothing that merely happens to run over every
/// vocabulary needs touching.
pub const DIALECTS: &[&'static dyn Dialect] = &[&Solid2];

/// A lazily built name → primitive index. One static per dialect.
pub(crate) type NameIndex = std::sync::OnceLock<std::collections::HashMap<&'static str, Primitive>>;

/// Looks a name up across `(name, primitive)` tables through a hash index
/// built once on first use.
///
/// [`Dialect::primitive`] sits on hot engine paths — every call expression
/// and import the engine classifies asks it — and a linear scan of a
/// ~50-entry table there is pure overhead. The tables stay the source of
/// truth (the tests iterate and cross-check them); the map is only the
/// access path, so the semantics are exactly the scan's. Table order cannot
/// matter: the sortedness tests hold each table free of duplicate names, so
/// no entry can shadow another.
pub(crate) fn lookup(
    index: &'static NameIndex,
    tables: &[&'static [(&'static str, Primitive)]],
    name: &str,
) -> Option<Primitive> {
    index
        .get_or_init(|| {
            tables
                .iter()
                .flat_map(|table| table.iter().copied())
                .collect()
        })
        .get(name)
        .copied()
}

/// Reverse of [`lookup`].
pub(crate) fn reverse(
    table: &[(&'static str, Primitive)],
    primitive: Primitive,
) -> Option<&'static str> {
    table
        .iter()
        .find(|(_, candidate)| *candidate == primitive)
        .map(|(name, _)| *name)
}

#[cfg(test)]
mod tests;
