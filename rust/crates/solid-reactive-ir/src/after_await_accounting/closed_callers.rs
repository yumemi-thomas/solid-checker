//! ADR 1197: the "no async computation runs it" reason of a read-after-await site, under the owner's addendum to ADR 0973's ruling 5
//! (2026-11-23): every call of the async function the site is written in is enumerated and none escapes.
//!
//! The function is a named, same-file, unexported function whose value is never rebound and that no unseen code enters (no method, no
//! component, no `eval` in the file). Every reference to its binding is the callee of a direct call: never stored, returned, handed as an
//! argument, spread or put in an object (ADR 0565's census of hand-offs, [`crate::local_access::function_reference_hand_offs`]: anything but
//! [`HandOff::Call`] fails), and no other file references it. Each call is written in a function whose own code runs it (no JSX expression,
//! tracked region or instance field between, [`crate::execution_role::runs_with_its_function`]), and that function is:
//! - an `on…` handler: the function literal of an `on` + capital attribute of an intrinsic element with no spread;
//! - the body of a proven component ([`crate::indexes::ComponentStatus::Proven`]); or
//! - a helper for which the same holds, at most [`DEPTH`] links up.
//!
//! None of these is a memo's or an effect's compute or a tracked async callback, so no computation's flight is on the async stack of the
//! function's continuation. Anything unresolved (a module-level call, a parameter-list call, an unknown or dynamic callee, a foreign
//! reference) answers `false`: the site keeps its row.

use solid_facts::FileFacts;
use solid_facts::ast::FunctionFact;
use solid_facts::core::Span;

use crate::indexes::SemanticLookup;
use crate::local_access::HandOff;

/// How many caller links the proof follows before it gives up (the site keeps its row).
const DEPTH: usize = 4;

/// Whether every call of `function` is enumerated and runs from an `on…` handler, a component body or a helper that is so in turn.
pub(super) fn closed(
    file: &FileFacts,
    function: &FunctionFact,
    lookup: &SemanticLookup<'_>,
) -> bool {
    closed_at(file, function, lookup, 0)
}

fn closed_at(
    file: &FileFacts,
    function: &FunctionFact,
    lookup: &SemanticLookup<'_>,
    depth: usize,
) -> bool {
    let ast = &file.ast;
    if depth > DEPTH
        || function.generator
        || function.method_name.is_some()
        || ast
            .module_hazards
            .iter()
            .any(|hazard| hazard.kind == solid_facts::ast::ModuleHazardKind::Eval)
        || crate::interproc::property_gets::direct_eval_in(file, Span::new(0, u32::MAX))
        || !lookup.function_value_is_current(file, function)
        || crate::execution_role::entered_by_unseen_code(file, function, lookup)
        || lookup.function_may_be_component(file, function)
    {
        return false;
    }
    let Some(name) = function
        .name
        .as_ref()
        .filter(|_| function.kind == solid_facts::ast::FunctionKind::Declaration)
        .or_else(|| crate::owners::function_binding_name(file, function))
    else {
        return false;
    };
    if crate::indexes::binding_exported(file, name.span)
        || !crate::local_access::foreign_reference_hand_offs(file, function, lookup).is_empty()
        || lookup
            .function_call_sites(file.path.as_str(), function.span)
            .iter()
            .any(|(caller, _)| caller.path != file.path)
    {
        return false;
    }
    let Some(references) =
        crate::local_access::function_reference_hand_offs(file, function, lookup)
    else {
        return false;
    };
    // Every reference the binder resolved to the name, type positions aside, is one the census enumerated as a call.
    let typed = |at: Span| ast.type_queries.iter().any(|query| query.contains(at));
    let enumerated = ast
        .references_to(name.span)
        .iter()
        .filter(|at| !typed(**at))
        .all(|at| references.iter().any(|(reference, _)| reference == at));
    !references.is_empty()
        && enumerated
        && references.iter().all(|(at, hand_off)| {
            typed(*at)
                || (matches!(hand_off, HandOff::Call)
                    && ast
                        .calls
                        .iter()
                        .find(|call| ast.peel_ts_sugar_span(call.callee) == *at)
                        .is_some_and(|call| {
                            !call.construct
                                && !crate::owners::in_parameter_list(ast, call.span)
                                && call_closed(file, call.span, lookup, depth)
                        }))
        })
}

/// The call at `call` runs from a handler, a component body or a closed helper, as its own code.
fn call_closed(file: &FileFacts, call: Span, lookup: &SemanticLookup<'_>, depth: usize) -> bool {
    let Some(caller) = crate::owners::owning_function(&file.ast, call) else {
        return false;
    };
    if !caller.body.contains(call)
        || !crate::execution_role::runs_with_its_function(file, call, caller.span)
    {
        return false;
    }
    intrinsic_handler(file, caller)
        || lookup.function_component_status(file, caller) == crate::indexes::ComponentStatus::Proven
        || closed_at(file, caller, lookup, depth + 1)
}

/// `function` is the function literal of an `on` + capital attribute (no namespace) of an intrinsic element (a lowercase tag, no member
/// tag) with no spread.
fn intrinsic_handler(file: &FileFacts, function: &FunctionFact) -> bool {
    if function.generator || function.name.is_some() {
        return false;
    }
    file.ast.jsx_elements.iter().any(|element| {
        element.member_object.is_none()
            && element.spreads.is_empty()
            && file
                .source_text(element.name.span)
                .is_some_and(|name| name.starts_with(|first: char| first.is_ascii_lowercase()))
            && element.attributes.iter().any(|attribute| {
                attribute.namespace.is_none()
                    && file.source_text(attribute.name).is_some_and(|name| {
                        name.len() > 2
                            && name.starts_with("on")
                            && name.as_bytes()[2].is_ascii_uppercase()
                    })
                    && attribute
                        .expression
                        .is_some_and(|value| file.ast.peel_ts_sugar_span(value) == function.span)
            })
    })
}
