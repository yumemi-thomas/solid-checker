//! ADR 0976, S3: the functions a promise handler call runs after a suspension.
//!
//! A `.then`, `.catch` or `.finally` callback runs when its promise settles, in a later microtask: it is code written after a suspension whether or not the function holding
//! the chain is `async`. The review of #233 found Chrome raising UNTRACKED_READ_AFTER_AWAIT at the read in `function work() { return pause().then(() => source()); }`
//! handed through a wrapper to `createMemo` (a compute that returns the chained promise), with no row: the accounting listed only the code of an `async` function after its
//! own `await`, and the walks that follow promise chains start only from a computation the producers recognize. There is no census reason (ADR 0973), so a function that is a
//! promise handler is after a suspension, and the reads of the universe and the calls written directly in it are sites like those after an `await`.
//!
//! A handler is a function literal, or an identifier that exactly names a project function, handed as an argument of a promise handler call: a call whose callee the compiler
//! resolves to `Promise.then`, `Promise.catch`, `Promise.finally` or `PromiseLike.then` (before any veto: a project subclass or a patch only makes the call less known), or, with
//! no standard resolution, a member spelled `then`, `catch` or `finally` (a thenable of the project's, or a receiver whose type no fact proves: the text only adds sites, and
//! ends nothing). A callback a thenable runs at once is not after a suspension; treating it as one gives an uncertifiable row, never a violation.

use solid_facts::FileFacts;
use solid_facts::ast::CallFact;
use solid_facts::core::Span;

use crate::HashMap;
use crate::HashSet;
use crate::pipeline::AnalysisContext;

/// The handler functions of the program by file path: the spans of the function literals and project functions handed to a promise handler call.
pub(super) type Handlers<'a> = HashMap<&'a str, HashSet<Span>>;

/// Every function the program hands to a promise handler call, outside dependency source. A debug build asks again without the per-file gate and requires the same answer
/// (the gate only skips files where [`is_handler_call`] can answer nothing).
pub(super) fn handler_functions<'a>(ctx: &AnalysisContext<'a>) -> Handlers<'a> {
    let found = collect(ctx, true);
    if cfg!(debug_assertions) {
        assert_eq!(
            found,
            collect(ctx, false),
            "the per-file gate of the promise handler scan skipped a handler call"
        );
    }
    found
}

fn collect<'a>(ctx: &AnalysisContext<'a>, gated: bool) -> Handlers<'a> {
    let lookup = ctx.semantic_lookup;
    let mut found: Handlers<'a> = HashMap::default();
    for file in lookup.files() {
        if lookup.in_dependency_source(file.path.as_str()) {
            continue;
        }
        // A file where no call can be a promise handler call holds none: such a call resolves to a standard promise handler or is a member spelled like one.
        if gated
            && !lookup.path_resolves_promise_handler(file.path.as_str())
            && !spells_handler_member(file)
        {
            continue;
        }
        for call in &file.ast.calls {
            if call.construct || !is_handler_call(ctx, file, call) {
                continue;
            }
            for argument in &call.arguments {
                if argument.spread {
                    continue;
                }
                let span = file.ast.peel_ts_sugar_span(argument.span);
                if let Some(function) = file.ast.function_at(span) {
                    found
                        .entry(file.path.as_str())
                        .or_default()
                        .insert(function.span);
                } else if let Some((owner, function)) =
                    lookup.function_called_at(file.path.as_str(), argument.span)
                {
                    found
                        .entry(owner.path.as_str())
                        .or_default()
                        .insert(function.span);
                }
            }
        }
    }
    found
}

/// Whether `call` is a promise handler call: resolved to a standard promise handler, or a member spelled like one where no standard declaration resolves.
fn is_handler_call(ctx: &AnalysisContext<'_>, file: &FileFacts, call: &CallFact) -> bool {
    let callee = file.ast.peel_ts_sugar_span(call.callee);
    if let Some(name) = ctx
        .semantic_lookup
        .resolved_standard_call_name(file, callee)
    {
        return crate::runtime_semantics::is_promise_handler_name(name);
    }
    file.ast
        .member_at(callee)
        .is_some_and(|member| spells_handler(file, member))
}

/// Whether the member's property is spelled like a promise handler (a thenable of the project's, or a receiver whose type no fact proves).
fn spells_handler(file: &FileFacts, member: &solid_facts::ast::MemberFact) -> bool {
    matches!(
        file.source_text(member.property),
        Some("then" | "catch" | "finally")
    )
}

/// Whether some member of the file is spelled like a promise handler: the text arm of [`is_handler_call`] answers only for such a member.
fn spells_handler_member(file: &FileFacts) -> bool {
    file.ast
        .members
        .iter()
        .any(|member| spells_handler(file, member))
}

/// Whether the function whose own body holds `span` is a promise handler of the program.
pub(super) fn in_handler(handlers: &Handlers<'_>, file: &FileFacts, span: Span) -> bool {
    handlers.get(file.path.as_str()).is_some_and(|functions| {
        crate::owners::containing_ast_function(&file.ast, span)
            .is_some_and(|function| functions.contains(&function.span))
    })
}
