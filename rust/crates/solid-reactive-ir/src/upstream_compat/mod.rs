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
use crate::{EntitySymbols, StaticViolation, SymbolId};
use solid_facts::FileFacts;

/// Everything one file's upstream-compat checks may consult.
pub(super) struct UpstreamCompatContext<'a> {
    pub(super) lookup: &'a SemanticLookup<'a>,
    pub(super) entities: &'a EntitySymbols,
    pub(super) symbol_names: &'a HashMap<SymbolId, SymbolId>,
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
