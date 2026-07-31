//! The eslint-plugin-solid 0.14.5 rule surface, reproduced over the checker's
//! facts.
//!
//! These are the file-local rules the upstream plugin ships and the Solid 1.x
//! dialect exposes as `v1/<rule>`: JSX prop hygiene, handler conventions,
//! structural preferences, and the fine-grained decomposition of upstream's
//! monolithic `reactivity` rule. Everything here reads the same fact tables
//! the reactive engine reads — Oxc AST facts, resolved primitives, TypeScript
//! entities — never source text through a regex.
//!
//! The pass runs only for the dialects whose catalogs declare these rules
//! (today: Solid 1.x). Each submodule owns one group of rules and emits
//! [`StaticViolation`]s whose identities the dialect's catalog resolves; an
//! identity emitted here without a catalog entry is a bug the solver fails
//! loudly on.

mod attributes;
mod imports;
mod reactivity;
mod structure;
mod syntax;
mod undef;

use std::collections::HashMap;

use crate::indexes::SemanticLookup;
use crate::{EntitySymbols, ReactiveSourceKind, StaticViolation, SymbolId};
use solid_facts::FileFacts;
use typefacts::Location;

/// Everything one file's upstream-compat checks may consult.
///
/// The reactive maps are the reason the decomposed `reactivity` rules can be
/// semantic rather than syntactic: upstream decides what is reactive from
/// naming conventions and call shapes, while these are the sources the engine
/// *proved*, through TypeScript symbol resolution, package contracts, and
/// cross-file propagation.
pub(super) struct UpstreamCompatContext<'a> {
    pub(super) lookup: &'a SemanticLookup<'a>,
    pub(super) entities: &'a EntitySymbols,
    /// Proven reactive accessors, by symbol: display name and declaration.
    pub(super) accessors: &'a HashMap<SymbolId, (SymbolId, Location)>,
    /// Whether each proven source is an accessor or a store path.
    pub(super) source_kinds: &'a HashMap<SymbolId, ReactiveSourceKind>,
}

/// Runs every upstream-compat rule over one file.
pub(super) fn check_file(
    file: &FileFacts,
    context: &UpstreamCompatContext<'_>,
) -> Vec<StaticViolation> {
    let mut violations = Vec::new();
    syntax::check_file(file, context, &mut violations);
    attributes::check_file(file, context, &mut violations);
    structure::check_file(file, context, &mut violations);
    imports::check_file(file, context, &mut violations);
    undef::check_file(file, context, &mut violations);
    reactivity::check_file(file, context, &mut violations);
    violations
}
