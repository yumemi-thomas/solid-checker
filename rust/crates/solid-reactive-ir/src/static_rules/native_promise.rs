//! Native promises (ADR 0349): an expression whose value is a promise of the
//! intrinsic class, shared by the stack walk and the promise-flight walk.
//!
//! A claim is proven from what the call is: `new Promise`, a static of the
//! global `Promise`, `fetch`, an `async` function's result, a project function
//! that returns one of these on every path, and `then`, `catch` or `finally` of
//! one. It holds only while the Promise premise holds
//! ([`promise_intrinsics_intact`](crate::indexes::SemanticLookup)). A typed
//! `Promise<T>` or `PromiseLike<T>` proves nothing: a subclass or a thenable has
//! that type too.

use crate::pipeline::AnalysisContext;
use crate::runtime_semantics::standard_call_name;
use solid_facts::FileFacts;
use solid_facts::ast::{CallFact, FunctionFact};
use solid_facts::core::Span;

/// Nesting limit for the claim through project functions and chains.
const MAX_DEPTH: usize = 6;

/// The global `Promise`: the identifier `Promise`, bound by no declaration the
/// parser saw.
pub(super) fn is_global_promise(file: &FileFacts, span: Span) -> bool {
    let span = file.ast.peel_ts_sugar_span(span);
    file.source_text(span) == Some("Promise") && file.ast.reference_declaration(span).is_none()
}

/// A call to a static member of the global `Promise` (`Promise.all(...)`).
pub(super) fn is_global_promise_static(file: &FileFacts, callee: Span) -> bool {
    let callee = file.ast.peel_ts_sugar_span(callee);
    !file.ast.computed_members.contains(&callee)
        && file
            .ast
            .member_at(callee)
            .is_some_and(|member| is_global_promise(file, member.object))
}

/// Whether `call` is the built-in `name` itself, not a value that merely
/// shares its declaration's signature (a Proxy or a subclass inherits it) and
/// not a shadowed global (ADR 0349). A `Constructor.construct` name is a `new`
/// of the global class; any other dotted name is a static or a method of the
/// global receiver.
pub(super) fn direct_builtin(file: &FileFacts, call: &CallFact, name: &str) -> bool {
    let callee = file.ast.peel_ts_sugar_span(call.callee);
    let global = |span: Span, expected: &str| {
        file.source_text(span) == Some(expected) && file.ast.reference_declaration(span).is_none()
    };
    if let Some(class) = name.strip_suffix("Constructor.construct") {
        call.construct && global(callee, class)
    } else if let Some((receiver, _)) = name.split_once('.') {
        file.ast.member_at(callee).is_some_and(|member| {
            if receiver == "WindowOrWorkerGlobalScope" {
                ["window", "globalThis", "self"]
                    .iter()
                    .any(|name| global(member.object, name))
            } else {
                global(
                    member.object,
                    receiver.strip_suffix("Constructor").unwrap_or(receiver),
                )
            }
        })
    } else {
        global(callee, name)
    }
}

/// Whether the function the callee names is an `async` project function whose
/// value is the one the call runs.
pub(super) fn async_helper<'x>(
    ctx: &AnalysisContext<'x>,
    file: &FileFacts,
    call: &CallFact,
) -> Option<(&'x FileFacts, &'x FunctionFact)> {
    if call.construct {
        return None;
    }
    let callee = file.ast.peel_ts_sugar_span(call.callee);
    if file.ast.member_at(callee).is_some() {
        return None;
    }
    let (helper_file, helper) = ctx
        .semantic_lookup
        .function_called_at(file.path.as_str(), callee)?;
    (helper.r#async
        && !helper.generator
        && helper.method_name.is_none()
        && ctx
            .semantic_lookup
            .function_value_is_current(helper_file, helper))
    .then_some((helper_file, helper))
}

/// Whether the expression at `span` is a native promise, under the Promise
/// premise: the claim of [`native_promise_claim`] while no visible write
/// reaches the global `Promise` or its handlers.
pub(super) fn native_promise(
    ctx: &AnalysisContext<'_>,
    file: &FileFacts,
    span: Span,
    depth: usize,
) -> bool {
    ctx.semantic_lookup.promise_intrinsics_intact() && native_promise_claim(ctx, file, span, depth)
}

/// What the expression at `span` is when no visible write changes the
/// intrinsics. A premise-withdrawn caller uses it to report an obligation
/// rather than a silent clean.
pub(super) fn native_promise_claim(
    ctx: &AnalysisContext<'_>,
    file: &FileFacts,
    span: Span,
    depth: usize,
) -> bool {
    if depth > MAX_DEPTH {
        return false;
    }
    let span = file.ast.peel_ts_sugar_span(span);
    let Some(call) = file.ast.call_at(span) else {
        return false;
    };
    if async_helper(ctx, file, call).is_some() {
        return true;
    }
    let callee = file.ast.peel_ts_sugar_span(call.callee);
    // An `async` function literal called in place returns the intrinsic promise.
    if !call.construct
        && file
            .ast
            .function_at(callee)
            .is_some_and(|function| function.r#async && !function.generator)
    {
        return true;
    }
    let standard = ctx
        .semantic_lookup
        .resolved_callee_call(file, call.callee)
        .and_then(standard_call_name);
    match (call.construct, standard) {
        (true, Some("PromiseConstructor.construct")) => is_global_promise(file, call.callee),
        (
            false,
            Some(
                "PromiseConstructor.resolve"
                | "PromiseConstructor.reject"
                | "PromiseConstructor.all"
                | "PromiseConstructor.allSettled"
                | "PromiseConstructor.race"
                | "PromiseConstructor.any",
            ),
        ) => is_global_promise_static(file, callee),
        (false, Some("Promise.then" | "Promise.catch" | "Promise.finally")) => file
            .ast
            .member_at(callee)
            .filter(|_| !file.ast.computed_members.contains(&callee))
            .is_some_and(|member| native_promise_claim(ctx, file, member.object, depth + 1)),
        (false, Some(name @ ("fetch" | "WindowOrWorkerGlobalScope.fetch"))) => {
            direct_builtin(file, call, name)
        }
        (false, None) => returns_native_promise_claim(ctx, file, call, depth),
        _ => false,
    }
}

/// A call to a project function that, on every path, returns a native promise
/// (`const pause = () => new Promise(...)`).
fn returns_native_promise_claim(
    ctx: &AnalysisContext<'_>,
    file: &FileFacts,
    call: &CallFact,
    depth: usize,
) -> bool {
    if call.construct {
        return false;
    }
    let callee = file.ast.peel_ts_sugar_span(call.callee);
    if file.ast.member_at(callee).is_some() {
        return false;
    }
    let Some((helper_file, helper)) = ctx
        .semantic_lookup
        .function_called_at(file.path.as_str(), callee)
    else {
        return false;
    };
    if helper.generator
        || helper.method_name.is_some()
        || !ctx
            .semantic_lookup
            .function_value_is_current(helper_file, helper)
    {
        return false;
    }
    let mut returned = helper_file
        .ast
        .own_returns(helper)
        .chain(helper.expression_return.iter())
        .peekable();
    if returned.peek().is_none() {
        return false;
    }
    returned.all(|returned| {
        returned
            .argument
            .is_some_and(|argument| native_promise_claim(ctx, helper_file, argument, depth + 1))
    })
}
