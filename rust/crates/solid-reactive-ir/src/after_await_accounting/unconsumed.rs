//! ADR 1090 (G1 residual): the promise of the function a read-after-await site is written in is never consumed.
//!
//! rc.21's dev build raises UNTRACKED_READ_AFTER_AWAIT only for a read whose V8 async stack holds the frame `__solidAsyncCompute_<id>`
//! (`@solidjs/signals@2.0.0-rc.21` `dist/dev-shared.js` `watchAsyncTail`, `attributedFlight`): the async function that awaits an async
//! computation's returned promise. V8's async frames are the chain of code awaiting the current promise; a promise nothing awaits, returns or
//! hands on ends that chain at the function itself, so no read in its continuation can carry the frame. This module proves that the promise of
//! the function holding a site is never consumed, by exact facts only:
//!
//! - a named async function (a declaration or a `const` binding, its value current, not a method, not exported, not entered by unseen code, no
//!   reference in another file) with at least one reference, every one a direct call whose result is discarded (`f();` as a statement); or
//! - a function literal handed to the standard `Promise.prototype.then` (as its fulfilment or rejection handler) of a call whose result is
//!   discarded, on a receiver that is a call of a standard-library declaration (a native promise; an argumentless call by the callee's own symbol, declared
//!   only in the default library): the reaction's derived promise is consumed by
//!   nothing.
//!
//! Anything else (an export, a returned, awaited, stored or handed-on call, a custom thenable, a named function expression) is not proven.

use solid_facts::FileFacts;
use solid_facts::core::Span;

use crate::local_access::HandOff;

/// Whether the promise of the function that holds `at` is never consumed (see the module documentation).
pub(super) fn holder_promise_unconsumed(
    ctx: &crate::pipeline::AnalysisContext<'_>,
    file: &FileFacts,
    at: Span,
) -> bool {
    let lookup = ctx.semantic_lookup;
    let Some(function) = crate::owners::owning_function(&file.ast, at) else {
        return false;
    };
    if function.generator || function.method_name.is_some() {
        return false;
    }
    let ast = &file.ast;
    let named =
        function.name.is_some() || crate::owners::function_binding_name(file, function).is_some();
    if !named {
        return then_handler_unconsumed(ctx, file, function.span);
    }
    if function.kind == solid_facts::ast::FunctionKind::Expression && function.name.is_some() {
        return false;
    }
    if !lookup.function_value_is_current(file, function)
        || crate::execution_role::entered_by_unseen_code(file, function, lookup)
        || !crate::local_access::foreign_reference_hand_offs(file, function, lookup).is_empty()
        || lookup
            .function_call_sites(file.path.as_str(), function.span)
            .iter()
            .any(|(caller, _)| caller.path != file.path)
    {
        return false;
    }
    // An export a host may call (a framework's loader, run inside a computation) is never proven, referenced or not; and a function with no reference at all is no
    // discarded call (`all` of nothing proves nothing).
    let exported = crate::owners::function_binding_name(file, function)
        .is_some_and(|name| crate::indexes::binding_exported(file, name.span))
        || ast.exports_containing(function.span).next().is_some();
    let Some(references) =
        crate::local_access::function_reference_hand_offs(file, function, lookup)
    else {
        return false;
    };
    if exported || references.is_empty() {
        return false;
    }
    references.into_iter().all(|(reference, hand_off)| {
        matches!(hand_off, HandOff::Call)
            && ast
                .calls
                .iter()
                .find(|call| ast.peel_ts_sugar_span(call.callee) == reference)
                .is_some_and(|call| !call.construct && call.result_discarded)
    })
}

/// `literal` is the fulfilment or rejection handler of a discarded call of the standard `Promise.prototype.then` on a native promise.
fn then_handler_unconsumed(
    ctx: &crate::pipeline::AnalysisContext<'_>,
    file: &FileFacts,
    literal: Span,
) -> bool {
    let lookup = ctx.semantic_lookup;
    let ast = &file.ast;
    let Some((call, _)) = ast.arguments_containing(literal).find(|(call, index)| {
        *index <= 1
            && !call.arguments[*index].spread
            && ast.peel_ts_sugar_span(call.arguments[*index].span) == literal
    }) else {
        return false;
    };
    let declaration = |callee: Span| {
        lookup
            .resolved_callee_call(file, callee)
            .and_then(|resolved| resolved.declaration.as_ref())
            .filter(|declaration| declaration.standard_library)
    };
    if call.construct
        || !call.result_discarded
        || declaration(call.callee).map(|declaration| declaration.qualified_name.as_ref())
            != Some("Promise.then")
    {
        return false;
    }
    let Some(member) = ast.member_at(ast.peel_ts_sugar_span(call.callee)) else {
        return false;
    };
    let receiver = ast.peel_ts_sugar_span(member.object);
    ast.calls
        .iter()
        .find(|inner| inner.span == receiver)
        .is_some_and(|inner| {
            // An argumentless call carries no resolved-call fact: the callee's own symbol, declared only in the default library, says the same.
            !inner.construct
                && (declaration(inner.callee).is_some()
                    || matches!(
                        crate::static_rules::callee_home(ctx, file, inner),
                        Some(crate::static_rules::CalleeHome::DefaultLibrary)
                    ))
        })
}
