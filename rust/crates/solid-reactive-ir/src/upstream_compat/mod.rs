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
use solid_facts::core::Span;
use typefacts::Location;

/// The source text a span covers, or `""` when the span is not readable.
///
/// Every rule here locates its report by span and phrases it with the
/// author's own spelling, so this is the one shared way to get from one to
/// the other.
pub(super) fn text(file: &FileFacts, span: Span) -> &str {
    file.source_text(span).unwrap_or_default()
}

/// Whether a JSX tag names a DOM element rather than a component.
///
/// JSX's own rule: a lowercase-led tag is an intrinsic element, anything else
/// is a value reference. Rules about DOM attributes and listeners apply only
/// to the former.
pub(super) fn is_lowercase_led(name: &str) -> bool {
    name.starts_with(|character: char| character.is_ascii_lowercase())
}

/// Everything one file's upstream-compat checks may consult.
///
/// The reactive maps are the reason the decomposed `reactivity` rules can be
/// semantic rather than syntactic: upstream decides what is reactive from
/// naming conventions and call shapes, while these are the sources the engine
/// *proved*, through TypeScript symbol resolution, package contracts, and
/// cross-file propagation.
pub(super) struct UpstreamCompatContext<'a> {
    /// The vocabulary these rules are checking against. Consulted rather than
    /// assumed: which argument slot of a primitive is a tracked scope is a
    /// per-dialect fact, and the one place the two versions differ most.
    pub(super) dialect: &'a dyn solid_dialect::Dialect,
    pub(super) lookup: &'a SemanticLookup<'a>,
    pub(super) entities: &'a EntitySymbols,
    /// Proven reactive accessors, by symbol: display name and declaration.
    pub(super) accessors: &'a HashMap<SymbolId, (SymbolId, Location)>,
    /// Whether each proven source is an accessor or a store path.
    pub(super) source_kinds: &'a HashMap<SymbolId, ReactiveSourceKind>,
    /// Component props roots proven by component shape and propagated type
    /// facts. Member names may be unresolved (for example an inferred `any`),
    /// but the props object itself remains a reactive proxy.
    pub(super) prop_sources: &'a HashMap<SymbolId, (SymbolId, Location)>,
    /// The proven-source symbol at each exact TypeScript reference location,
    /// indexed path → byte range. Entity facts intentionally cover only
    /// semantically interesting expression shapes; ordinary operator operands
    /// can therefore have a symbol reference but no entity row. Rules that
    /// need exact identifier identity use this as the type-fact fallback
    /// rather than matching source text — indexed once here because scanning
    /// every source's reference list per identifier is quadratic in project
    /// size. Owned: the reference lists it is built from live shorter than
    /// this struct's borrows.
    pub(super) source_reference_index: HashMap<String, HashMap<(u64, u64), SymbolId>>,
    /// The imported symbols a package contract describes.
    ///
    /// Read as the negative: a callee absent from here, absent from the
    /// dialect's primitives, and with no body in the project is one whose
    /// reactive behaviour nothing in the analysis knows.
    pub(super) contracted: &'a HashMap<SymbolId, crate::contracts::ResolvedContractBinding>,
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
