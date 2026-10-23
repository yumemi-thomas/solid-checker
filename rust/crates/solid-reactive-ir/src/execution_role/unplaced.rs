//! The placement blocker of code no role arm placed, read from where the
//! functions around it are written (`crate::placement`).

use super::*;
use crate::placement::PlacementBlocker;

/// Why code at `span` is unplaced when every role arm answered nothing: the
/// innermost function around it that is not run where it is written, and how
/// it is handed on. An IIFE runs in place, so the function around it decides;
/// a component's own body is placed by the arms, so the walk stops there.
///
/// `flow_asked` says the classification asked invocation flow (ADR 0319),
/// which only read classification does; its refused mixes are read only then,
/// and only once computed.
pub(super) fn unplaced_site_blocker(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
    flow_asked: bool,
) -> PlacementBlocker {
    let mixed = lookup.invocation_flow_mixed().filter(|_| flow_asked);
    let mut functions = file.ast.functions_body_containing(span).collect::<Vec<_>>();
    functions.sort_by_key(|function| function.span.end - function.span.start);
    for function in functions {
        if lookup.function_component_status(file, function) != ComponentStatus::No {
            break;
        }
        if mixed.is_some_and(|mixed| mixed.contains(&(file.path.to_string(), function.span))) {
            return PlacementBlocker::RoleMix;
        }
        if component_child_function(file, function.span) {
            return PlacementBlocker::ComponentChildren;
        }
        if jsx_attribute_value_function(file, function.span) {
            return PlacementBlocker::PropCallback;
        }
        if file
            .ast
            .arguments_containing(function.span)
            .any(|(call, index)| {
                let argument = &call.arguments[index];
                !argument.spread && file.ast.peel_ts_sugar_span(argument.span) == function.span
            })
        {
            return PlacementBlocker::CallbackUnknownConsumer;
        }
        if file
            .ast
            .calls
            .iter()
            .any(|call| file.ast.peel_ts_sugar_span(call.callee) == function.span)
        {
            continue;
        }
        if lookup
            .function_call_sites(file.path.as_str(), function.span)
            .is_empty()
        {
            return PlacementBlocker::NoResolvedCaller;
        }
        return if lookup.program_closed {
            PlacementBlocker::CallerFlowUnresolved
        } else {
            PlacementBlocker::OpenProgram
        };
    }
    PlacementBlocker::Other
}

/// [`ExecutionRole::Unknown`] for code at `span` no role arm placed, recording
/// [`unplaced_site_blocker`].
pub(super) fn unplaced_site(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
    flow_asked: bool,
) -> ExecutionRole {
    crate::placement::unplaced(unplaced_site_blocker(file, span, lookup, flow_asked))
}
