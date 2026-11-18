//! ADR 1090 (G1 residual): the promise of the function a read-after-await site is written in is never consumed.
//!
//! rc.21's dev build raises UNTRACKED_READ_AFTER_AWAIT only for a read whose V8 async stack holds the frame `__solidAsyncCompute_<id>`
//! (`@solidjs/signals@2.0.0-rc.21` `dist/dev-shared.js` `watchAsyncTail`, `attributedFlight`): the async function that awaits an async
//! computation's returned promise. V8's async frames are the chain of code awaiting the current promise; a promise nothing awaits, returns or
//! hands on ends that chain at the function itself, so no read in its continuation can carry the frame. This module proves that by exact facts
//! only, for a read in a named async function (a declaration or a `const` binding) when all of these hold:
//!
//! - the function is `async` in code, not a generator, a method or a named function expression, and its value is current;
//! - it is written in a module (a classic script's top-level function is a global another file may call as `window.f()`);
//! - it is not exported, unseen code does not enter it, and no other file references or calls it;
//! - the program has no direct `eval` and no code-evaluating call (code a string holds may call the function where its promise is kept);
//! - it has at least one reference, and every one is a direct call whose result is discarded (`f();` as a statement);
//! - the read is past the prefix: an await of the same function on every run (`AstFacts::unconditional_awaits`) ends before it. A read that
//!   may run before the first suspension runs on the caller's stack, inside whatever computation made the call (review F2 of be4990c64).
//!
//! Anything else is not proven: an export, a returned, awaited, stored or handed-on call, a function literal. The `Promise.prototype.then`
//! handler case of be4990c64 was dropped (review F1): a receiver is a native promise only when no `Promise` subclass or patch can stand in,
//! and no exact fact says so.

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
    if !named || !function.r#async {
        return false;
    }
    if function.kind == solid_facts::ast::FunctionKind::Expression && function.name.is_some() {
        return false;
    }
    if !crate::execution_role::file_is_module(file)
        || lookup.hazard_present(crate::base::Hazard::Eval)
        || lookup.hazard_present(crate::base::Hazard::CodeEvaluatingCall)
        || !past_prefix(file, function.span, at)
        || !lookup.function_value_is_current(file, function)
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

/// An await of the function at `function` that runs on every run of it ends before `at`: the code at `at` runs only after the function suspended.
fn past_prefix(file: &FileFacts, function: Span, at: Span) -> bool {
    let ast = &file.ast;
    ast.unconditional_awaits.iter().any(|await_span| {
        await_span.end <= at.start
            && function.contains(*await_span)
            && crate::owners::owning_function(ast, *await_span)
                .is_some_and(|owner| owner.span == function)
    })
}
