//! F0 (`f0.md`): whether a resolved call's target summarizes everything running
//! it reaches. The transport drops the obligation of a call it resolves exactly,
//! and that is sound only when the target reads nothing the summary cannot see:
//! a captured parameter, an `arguments` object, `this`, a rebound binding.
//!
//! Default open: a form this does not follow is incomplete. Computed per build
//! and read only by `reactive_analysis.rs`, so no result chunk holds an answer
//! that depends on another file (a stale one in the daemon).

use std::collections::HashSet;

use solid_facts::ast::{FunctionFact, IdentifierRole};
use solid_facts::core::Span;

use crate::indexes::SemanticLookup;

/// How many functions one answer may walk. Past it the answer is open.
const MAX_FUNCTIONS: usize = 256;
/// How many `const`s one captured value may be walked through.
const MAX_ALIASES: usize = 4;

struct Walk<'l, 'a> {
    lookup: &'l SemanticLookup<'a>,
    /// A cycle stays complete only if nothing in it is open: a function met
    /// again adds nothing to the walk that first entered it.
    entered: HashSet<(String, Span)>,
}

/// Whether `function`'s summary is complete, nested functions and calls included.
pub(crate) fn summary_complete(
    file: &solid_facts::FileFacts,
    function: &FunctionFact,
    lookup: &SemanticLookup<'_>,
) -> bool {
    Walk {
        lookup,
        entered: HashSet::new(),
    }
    .complete(file, function)
}

/// [`summary_complete`], once per function for one analysis: both filters of
/// `reactive_analysis.rs` ask about every source call target. Each ask starts a
/// walk of its own, so the answer does not depend on what was asked before.
#[derive(Default)]
pub(crate) struct SummaryMemo(rustc_hash::FxHashMap<(String, Span), bool>);

impl SummaryMemo {
    pub(crate) fn complete(
        &mut self,
        file: &solid_facts::FileFacts,
        function: &FunctionFact,
        lookup: &SemanticLookup<'_>,
    ) -> bool {
        crate::walk_memo::memoized(&mut self.0, (file.path.to_string(), function.span), |_| {
            summary_complete(file, function, lookup)
        })
    }
}

impl Walk<'_, '_> {
    fn complete(&mut self, file: &solid_facts::FileFacts, function: &FunctionFact) -> bool {
        if !self.entered.insert((file.path.to_string(), function.span)) {
            return true;
        }
        if self.entered.len() > MAX_FUNCTIONS {
            return false;
        }
        let this_read = file.ast.members.iter().any(|member| {
            function.span.contains(member.span) && file.source_text(member.object) == Some("this")
        });
        if this_read {
            return false;
        }
        let followed = file
            .ast
            .identifiers
            .iter()
            .filter(|id| id.role == IdentifierRole::Reference && function.span.contains(id.span))
            .all(|id| self.reference_followed(file, function, id.span, MAX_ALIASES));
        if !followed {
            return false;
        }
        file.ast
            .calls
            .iter()
            .filter(|call| function.span.contains(call.span))
            .filter_map(|call| {
                self.lookup
                    .function_called_at(file.path.as_str(), call.callee)
            })
            .all(|(target_file, target)| self.complete(target_file, target))
    }

    fn reference_followed(
        &mut self,
        file: &solid_facts::FileFacts,
        function: &FunctionFact,
        reference: Span,
        aliases: usize,
    ) -> bool {
        if file.source_text(reference) == Some("arguments") {
            return false;
        }
        // An import or a global: its member or call has its own row.
        let Some(declaration) = file.ast.reference_declaration(reference) else {
            return true;
        };
        let binding = file
            .ast
            .bindings
            .iter()
            .find(|binding| binding.names.iter().any(|name| name.span == declaration));
        // A rebound binding holds no one function or object (`t13`, `f16`).
        if binding.is_some_and(|binding| !binding.immutable)
            && called_or_member_root(file, reference)
        {
            return false;
        }
        if function.span.contains(declaration) {
            return true;
        }
        let captured =
            file.ast.functions.iter().any(|outer| {
                outer.span.contains(function.span) && outer.span.contains(declaration)
            });
        if !captured {
            return true;
        }
        // Captured: a parameter of any shape is what the caller chose (`t5`, `f08`).
        if crate::owners::in_parameter_list(&file.ast, declaration) {
            return declared_primitive(file, declaration);
        }
        let Some(binding) = binding.filter(|binding| binding.immutable) else {
            return false;
        };
        let Some(initializer) = binding.initializer else {
            return false;
        };
        let initializer = file.ast.peel_ts_sugar_span(initializer);
        if let Some(target) = file.ast.functions.iter().find(|f| f.span == initializer) {
            return self.complete(file, target);
        }
        // A source: its rows are the summary's (`f15`).
        if file
            .ast
            .calls
            .iter()
            .find(|call| initializer.contains(call.span))
            .is_some_and(|call| self.lookup.primitive_at_call(file, call.span).is_some())
        {
            return true;
        }
        if aliases == 0 {
            return false;
        }
        file.ast
            .identifiers
            .iter()
            .filter(|id| id.role == IdentifierRole::Reference && initializer.contains(id.span))
            .all(|id| self.reference_followed(file, function, id.span, aliases - 1))
    }
}

/// Whether the parameter declared at `name` is annotated with primitive types
/// only: no accessor, setter, store or props object fits such a type, and `tsc`
/// holds the program to it. A cast through `any` is the program's own (`f14`).
fn declared_primitive(file: &solid_facts::FileFacts, name: Span) -> bool {
    const PRIMITIVES: [&str; 7] = [
        "string",
        "number",
        "boolean",
        "bigint",
        "symbol",
        "null",
        "undefined",
    ];
    let source = file.source.as_ref().get(name.end as usize..).unwrap_or("");
    let rest = source.trim_start();
    let rest = rest.strip_prefix('?').unwrap_or(rest).trim_start();
    let Some(annotation) = rest.strip_prefix(':') else {
        return false;
    };
    let end = annotation.find([',', ')', '=']).unwrap_or(annotation.len());
    annotation[..end]
        .split('|')
        .all(|part| PRIMITIVES.contains(&part.trim()))
}

fn called_or_member_root(file: &solid_facts::FileFacts, reference: Span) -> bool {
    file.ast
        .calls_with_peeled_callee(reference)
        .next()
        .is_some()
        || file
            .ast
            .members
            .iter()
            .any(|member| file.ast.peel_ts_sugar_span(member.object) == reference)
}
