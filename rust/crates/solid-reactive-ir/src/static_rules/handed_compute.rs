//! ADR 0873: an async function that reaches a tracked primitive through a
//! project wrapper's parameter (`const useQ = (fn) => createMemo(fn);
//! useQ(async () => { await x; return n(); })`). The producer's and the
//! direct proof's calls name the function only at the primitive itself, so
//! its reads past an await had no row.

use crate::pipeline::AnalysisContext;
use solid_facts::FileFacts;
use solid_facts::ast::{BindingShape, CallFact, FunctionFact};

/// How far a parameter is followed through further wrappers.
const MAX_FORWARDS: usize = 6;

/// Where an async function handed to a call runs.
pub(super) enum HandedCompute {
    /// As the compute of a tracked primitive, on every run of the call.
    Tracked(String),
    /// Nothing proves it is one, or that it is not: `via` is the call that
    /// takes it, or none past the walk's bound.
    Unplaced { via: Option<String> },
}

/// The strongest answer over the calls that hand `function` on: `None` when
/// none does, or each runs it as a primitive's untracked callback.
pub(super) fn handed_compute(
    ctx: &AnalysisContext<'_>,
    file: &FileFacts,
    function: &FunctionFact,
) -> Option<HandedCompute> {
    let mut answer = None;
    for call in &file.ast.calls {
        for (index, argument) in call.arguments.iter().enumerate() {
            if argument.spread
                || (file.ast.peel_ts_sugar_span(argument.span) != function.span
                    && crate::owners::exact_function_argument(
                        file,
                        argument.span,
                        ctx.semantic_lookup,
                    )
                    .is_none_or(|(path, span)| path != file.path.as_str() || span != function.span))
            {
                continue;
            }
            answer = stronger(answer, at_call(ctx, file, call, index, MAX_FORWARDS));
        }
    }
    answer
}

fn stronger(held: Option<HandedCompute>, next: Option<HandedCompute>) -> Option<HandedCompute> {
    match (held, next) {
        (Some(HandedCompute::Tracked(context)), _) | (_, Some(HandedCompute::Tracked(context))) => {
            Some(HandedCompute::Tracked(context))
        }
        (held, next) => held.or(next),
    }
}

/// What the callee of `call` does with its argument at `index`.
fn at_call(
    ctx: &AnalysisContext<'_>,
    file: &FileFacts,
    call: &CallFact,
    index: usize,
    depth: usize,
) -> Option<HandedCompute> {
    let lookup = ctx.semantic_lookup;
    let via = || {
        Some(
            file.source_text(file.ast.peel_ts_sugar_span(call.callee))
                .unwrap_or("this call")
                .chars()
                .take(80)
                .collect::<String>(),
        )
    };
    // The direct proof's: a primitive's position, tracked or not.
    if lookup.primitive_at_call(file, call.span).is_some() {
        return None;
    }
    // A standard callee is the carrier rules': a timer, a promise's handler.
    if lookup
        .resolved_callee_call(file, call.callee)
        .and_then(|resolved| resolved.declaration.as_ref())
        .is_some_and(|declaration| declaration.standard_library)
    {
        return None;
    }
    let Some(depth) = depth.checked_sub(1) else {
        return Some(HandedCompute::Unplaced { via: None });
    };
    match crate::execution_role::project_callee_at_parameter(file, call, index, lookup) {
        Some((callee_file, callee, true)) => parameter_fate(ctx, callee_file, callee, index, depth)
            .map(|fate| match fate {
                HandedCompute::Unplaced { via: None } => HandedCompute::Unplaced { via: via() },
                fate => fate,
            }),
        _ => Some(HandedCompute::Unplaced { via: via() }),
    }
}

/// Each use of the wrapper's parameter: only a whole argument of a tracked
/// primitive written in the wrapper's own straight-line body, or of a further
/// wrapper, is followed. Anything else (a call, a slot, a default, an
/// `arguments` alias, a later or conditional primitive) proves nothing.
fn parameter_fate(
    ctx: &AnalysisContext<'_>,
    file: &FileFacts,
    wrapper: &FunctionFact,
    index: usize,
    depth: usize,
) -> Option<HandedCompute> {
    let open = || Some(HandedCompute::Unplaced { via: None });
    let Some(parameter) = wrapper.parameters.get(index) else {
        return open();
    };
    let Some(name) = parameter.names.first() else {
        return open();
    };
    if parameter.shape != BindingShape::Identifier
        || parameter.initializer.is_some()
        || crate::indexes::binding_written(file, name.span)
        || file
            .source_text(wrapper.span)
            .is_none_or(|text| crate::indexes::mentions_word(text, "arguments"))
    {
        return open();
    }
    let mut answer = None;
    for &reference in file.ast.references_to(name.span) {
        if reference == name.span {
            continue;
        }
        // `f();` written as a statement in a synchronous wrapper's own body
        // runs the function in place where the wrapper runs, as an IIFE there
        // would, and its promise reaches nothing: the in-place walk places it,
        // and no primitive takes it as a compute here. A used result
        // (`return f()`, `f().then(g)`) may be a compute's value, and an async
        // wrapper may run it past its own await, on the compute's flight
        // (review D12): both stay open.
        if !wrapper.r#async
            && !wrapper.generator
            && file.ast.calls.iter().any(|call| {
                call.result_discarded
                    && !call.construct
                    && file.ast.peel_ts_sugar_span(call.callee) == reference
                    && crate::owners::written_directly_in(&file.ast, wrapper, call.span)
            })
        {
            continue;
        }
        let Some((call, position)) = file.ast.arguments_containing(reference).find(|(call, at)| {
            !call.arguments[*at].spread
                && file.ast.peel_ts_sugar_span(call.arguments[*at].span) == reference
        }) else {
            return open();
        };
        let lookup = ctx.semantic_lookup;
        let fate = if let Some(primitive) = lookup.primitive_at_call(file, call.span) {
            if !ctx
                .dialect
                .callback_semantics_at(primitive, position, call.arguments.len())
                .tracks_reads
            {
                None
            } else if wrapper.r#async
                || wrapper.generator
                || !crate::owners::written_directly_in(&file.ast, wrapper, call.span)
                || !file.ast.straight_line_calls.contains(&call.span)
            {
                open()
            } else {
                Some(HandedCompute::Tracked(format!(
                    "{} async computation",
                    ctx.dialect.name_of(primitive).unwrap_or("compute")
                )))
            }
        } else {
            at_call(ctx, file, call, position, depth)
        };
        answer = stronger(answer, fate);
    }
    answer
}
