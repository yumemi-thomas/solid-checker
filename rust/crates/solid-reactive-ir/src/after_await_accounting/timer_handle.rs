//! ADR 0984: a timer handle held in a `let` is an inert argument (the follow-up of ADR 0976).
//!
//! A standard call is inert only when no argument can carry project code onto its stack (`library_call.rs`). `clearTimeout(copiedTimer)` coerces its argument, and a `let` no fact
//! proves primitive is not inert: `proven_primitive` follows a `const` only. The common `let` is a timer handle: declared with no value (or `undefined`), written only by calls of
//! the unpatched global `setTimeout` or `setInterval`, read only by `clearTimeout` and `clearInterval`. In a browser a handle is a number; in Node it is an object (`Timeout`),
//! whose `Symbol.toPrimitive` is a library's, not a project's, and the proof is the browser's alone (`--runtime-target browser`).
//!
//! The proof is complete or it is none. Every reference of the binding is one of:
//!
//! - the target of a plain `=` assignment (never `+=`, `++`, `??=`, a destructuring or a loop target) whose value is a call of `setTimeout` or `setInterval` the compiler resolves to
//!   the standard library's declaration, written as an identifier callee (`window.setTimeout` names a member a project may replace), in a program that leaves every built-in alone
//!   ([`super::library_call::builtins_untouched`], which the caller has asked), or the unshadowed `undefined`;
//! - the sole argument of a call of `clearTimeout` or `clearInterval` resolved the same way.
//!
//! Anything else makes it none: the handle copied (`const r = { t }`), handed to a function, returned, compared, spread, written by a call nothing resolves, written through a name
//! that is not its own, exported, or a parameter. The binding is a `let` of one identifier, declared with no initializer (or `undefined`, or a call of this kind). No type is asked: the
//! annotation `ReturnType<typeof setTimeout>` proves nothing.

use solid_facts::FileFacts;
use solid_facts::ast::BindingShape;
use solid_facts::core::Span;

use crate::pipeline::AnalysisContext;
use crate::runtime_semantics;

/// Whether the argument at `value` is a `let` timer handle every reference of which is a proven write or a clear (module docs).
pub(super) fn is_timer_handle(ctx: &AnalysisContext<'_>, file: &FileFacts, value: Span) -> bool {
    // A handle is a number in a browser and an object in Node: the browser target alone.
    if ctx.rule_options.runtime.target != Some(crate::RuntimeTarget::Browser) {
        return false;
    }
    let Some(value) = file.peel_parentheses(value) else {
        return false;
    };
    if !crate::owners::is_identifier_span(file, value) {
        return false;
    }
    let lookup = ctx.semantic_lookup;
    let Some((binding_file, binding, _)) = lookup.binding_at_reference(file.path.as_str(), value)
    else {
        return false;
    };
    let [name] = binding.names.as_slice() else {
        return false;
    };
    let ast = &file.ast;
    if binding_file.path != file.path
        || binding.immutable
        || !binding.runtime_let
        || binding.shape != BindingShape::Identifier
        || crate::execution_role::flow_binding_exported(binding_file, name.span)
        || ast.functions.iter().any(|function| {
            function
                .parameters
                .iter()
                .any(|parameter| parameter.names.iter().any(|bound| bound.span == name.span))
                || function
                    .rest_parameter_names
                    .iter()
                    .any(|bound| bound.span == name.span)
        })
    {
        return false;
    }
    // The initializer: none, or a value of the same kind as a write.
    if let Some(initializer) = binding.initializer
        && !is_timer_value(ctx, file, initializer)
    {
        return false;
    }
    let references = ast.references_to(name.span);
    if references.is_empty() {
        return false;
    }
    references.iter().all(|reference| {
        let at = ast.peel_ts_sugar_span(*reference);
        if ast
            .write_targets
            .iter()
            .any(|target| ast.peel_ts_sugar_span(*target) == at)
        {
            // A write: exactly a plain assignment of a timer call (or `undefined`) to this name.
            return ast.assignments.iter().any(|assignment| {
                ast.peel_ts_sugar_span(assignment.target) == at
                    && !assignment.reads_target
                    && assignment.logical.is_none()
                    && assignment.array_slots.is_empty()
                    && is_timer_value(ctx, file, assignment.value_span)
            });
        }
        is_cleared(ctx, file, at)
    })
}

/// Whether the expression at `value` is the unshadowed `undefined` or a call of the standard `setTimeout` or `setInterval`.
fn is_timer_value(ctx: &AnalysisContext<'_>, file: &FileFacts, value: Span) -> bool {
    let Some(value) = file.peel_parentheses(value) else {
        return false;
    };
    // The unshadowed `undefined`: no declaration of the program binds the name.
    if matches!(file.source_text(value), Some("undefined"))
        && file.ast.reference_declaration(value).is_none()
    {
        return true;
    }
    file.ast.call_at(value).is_some_and(|call| {
        !call.construct && standard_call_of(ctx, file, call, &["setTimeout", "setInterval"])
    })
}

/// Whether the reference at `at` is the sole argument of a standard `clearTimeout` or `clearInterval` call.
fn is_cleared(ctx: &AnalysisContext<'_>, file: &FileFacts, at: Span) -> bool {
    file.ast.arguments_containing(at).any(|(call, index)| {
        index == 0
            && call.arguments.len() == 1
            && !call.arguments[0].spread
            && !call.construct
            && file.ast.peel_ts_sugar_span(call.arguments[0].span) == at
            && standard_call_of(ctx, file, call, &["clearTimeout", "clearInterval"])
    })
}

/// Whether `call` is written with an identifier callee and the compiler resolves it to the standard library's declaration of one of `names`.
fn standard_call_of(
    ctx: &AnalysisContext<'_>,
    file: &FileFacts,
    call: &solid_facts::ast::CallFact,
    names: &[&str],
) -> bool {
    let callee = file.ast.peel_ts_sugar_span(call.callee);
    crate::owners::is_identifier_span(file, callee)
        && file.ast.reference_declaration(callee).is_none()
        && ctx
            .semantic_lookup
            .resolved_callee_call(file, callee)
            .and_then(runtime_semantics::standard_call_name)
            .is_some_and(|name| names.contains(&name))
}
