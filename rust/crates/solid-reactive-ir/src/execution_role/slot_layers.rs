//! A primitive's callback as a function layer (ADR 0944).
//!
//! `layer_run` and `climb_in_place_layers` cross the layers the code around a function runs in place: an IIFE, a
//! standard-library carrier, a project helper that calls its parameter. A Solid primitive's callback is none of
//! them, so a walk that climbed layers ended at `untrack(() => …)`, `latest(() => …)` or `isPending(() => …)`, and
//! each walk decided alone what ending there meant: `onCleanup` in `untrack(() => …)` in `createTrackedEffect` is
//! still the leaf owner's child, and a walk that stopped at the `untrack` certified it clean.
//!
//! The question is the dialect's ([`solid_dialect::Dialect::slot_run`]), asked here once. A walk gives
//! [`climb_through_slots`] its policy, a closure that matches [`SlotRun`] with no wildcard, so a slot a walk does
//! not cross is a decision its own arm states, not a case it fell through.

use solid_dialect::{Primitive, SlotRun};
use solid_facts::FileFacts;
use solid_facts::ast::{ArgumentValueKind, CallFact, FunctionFact};
use solid_facts::core::Span;

use super::InPlaceClimb;
use crate::indexes::SemanticLookup;
use crate::owners::{owning_function, written_directly_in};

/// The primitive call a function is the callback of, and how the dialect says the callback runs.
pub(crate) struct SlotLayer<'f> {
    pub(crate) call: &'f CallFact,
    pub(crate) primitive: Primitive,
    pub(crate) run: SlotRun,
}

/// `function` written as a whole argument of a call the dialect resolves to a primitive (behind TypeScript sugar
/// at most): the slot it fills. `None` for any other function, for a call with a spread up to the argument (the
/// index then names nothing), and for a constructor call.
pub(crate) fn slot_layer<'f>(
    file: &'f FileFacts,
    function: &FunctionFact,
    lookup: &SemanticLookup<'_>,
) -> Option<SlotLayer<'f>> {
    let (call, argument) = file
        .ast
        .arguments_containing(function.span)
        .find(|(call, index)| {
            file.ast.peel_ts_sugar_span(call.arguments[*index].span) == function.span
        })?;
    if call.construct
        || call
            .arguments
            .iter()
            .take(argument + 1)
            .any(|argument| argument.spread)
    {
        return None;
    }
    let primitive = lookup.primitive_at_call(file, call.span)?;
    Some(SlotLayer {
        call,
        primitive,
        run: lookup
            .dialect
            .slot_run(primitive, argument, call.arguments.len()),
    })
}

/// What the owner a call supplies as data (`runWithOwner(owner, fn)`) is, as far as the syntax and the binder
/// prove it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SuppliedOwner {
    /// `null`: the call runs its callback with no owner (`context === null`).
    Null,
    /// `undefined` (`void …` too): no owner either, but the runtime's guards that test `context === null` do not
    /// take it for one.
    Undefined,
    /// The owner the call itself runs under: `getOwner()` written in the call's own function, as the argument or
    /// through an immutable `const`, before the call, in a function that never suspends. The callback then runs
    /// under exactly what the code around the call runs under, whatever that is.
    Ambient,
    /// A definite owner other than the one the call itself runs under: a `const` bound to `getOwner()` written
    /// directly in a proven component's body (ADR 0206), which is non-null and is never a leaf's or an effect
    /// phase's.
    Elsewhere,
    /// Any other value: not followed, so nothing is claimed of it.
    Unproven,
}

/// The owner supplied by the argument at `argument` of `call` ([`SuppliedOwner`]).
pub(crate) fn supplied_owner(
    file: &FileFacts,
    call: &CallFact,
    argument: usize,
    lookup: &SemanticLookup<'_>,
) -> SuppliedOwner {
    let Some(supplied) = call.arguments.get(argument) else {
        return SuppliedOwner::Unproven;
    };
    if supplied.spread
        || call
            .arguments
            .iter()
            .take(argument)
            .any(|argument| argument.spread)
    {
        return SuppliedOwner::Unproven;
    }
    match supplied.value {
        ArgumentValueKind::Null => return SuppliedOwner::Null,
        ArgumentValueKind::Undefined => return SuppliedOwner::Undefined,
        ArgumentValueKind::Identifier
        | ArgumentValueKind::Function
        | ArgumentValueKind::AsyncFunction
        | ArgumentValueKind::Other => {}
    }
    // `void expression` has no argument kind of its own (the same text test `run_with_owner_callback_owner` makes).
    if file
        .source_text(supplied.span)
        .is_some_and(|source| source.trim().starts_with("void "))
    {
        return SuppliedOwner::Undefined;
    }
    let argument_span = file.ast.peel_ts_sugar_span(supplied.span);
    // `const owner = getOwner()` in a proven component's body, used in a callback below it.
    if crate::owners::component_owner_binding(file, argument_span, lookup) {
        return SuppliedOwner::Elsewhere;
    }
    // The call's own function: a function that never suspends, so the value read before the call is the owner the
    // call runs under.
    let Some(function) = owning_function(&file.ast, call.span)
        .filter(|function| function.body.contains(call.span))
        .filter(|function| !function.r#async && !function.generator)
    else {
        return SuppliedOwner::Unproven;
    };
    // `runWithOwner(getOwner(), fn)`.
    if let Some(inner) = file.ast.call_at(argument_span) {
        return if lookup.primitive_at_call(file, inner.span) == Some(Primitive::GetOwner)
            && written_directly_in(&file.ast, function, inner.span)
        {
            SuppliedOwner::Ambient
        } else {
            SuppliedOwner::Unproven
        };
    }
    // `const owner = getOwner(); … runWithOwner(owner, fn)`.
    let Some((binding_file, binding, _)) =
        lookup.binding_at_reference(file.path.as_str(), argument_span)
    else {
        return SuppliedOwner::Unproven;
    };
    let Some(initializer) = binding.call_initializer else {
        return SuppliedOwner::Unproven;
    };
    if !binding.immutable
        || binding.shape != solid_facts::ast::BindingShape::Identifier
        || binding_file.path != file.path
        || lookup.primitive_at_call(binding_file, initializer) != Some(Primitive::GetOwner)
        || !written_directly_in(&file.ast, function, initializer)
        || initializer.end > call.span.start
    {
        return SuppliedOwner::Unproven;
    }
    SuppliedOwner::Ambient
}

/// Whether the call at `call` is written in a JSX expression of its own function: it then runs where the compiler
/// places that expression (an insert's tracked computation), which the compiler roles decide, not the code around
/// the element, so no climb crosses it. A call in a function written inside JSX is in no JSX expression of its own.
/// Without an owning function (module scope) any JSX around the call counts: the refusing answer.
pub(crate) fn call_in_jsx_expression(file: &FileFacts, call: Span) -> bool {
    let own = owning_function(&file.ast, call);
    file.ast
        .jsx_containing(call)
        .any(|element| own.is_none_or(|function| function.span.contains(element.span)))
}

/// Whether the call at `call` may run user code or an effect before it returns, which is what an earlier call in an
/// effect's apply is held to (a nested synchronous effect run lowers the marker the guard on creating with no owner
/// reads). A call of a primitive the dialect says runs nothing (`getOwner()`) cannot; a call that resolves to no
/// primitive may: the answer that makes the claim uncertifiable, never a violation.
pub(crate) fn call_may_run_code(
    file: &FileFacts,
    call: &CallFact,
    lookup: &SemanticLookup<'_>,
) -> bool {
    lookup
        .primitive_at_call(file, call.span)
        .is_none_or(|primitive| !lookup.dialect.runs_no_user_code(primitive))
}

/// [`super::climb_in_place_layers`] that also crosses the primitive slots `crosses` accepts: the climb from
/// `span` alternates the layers code runs in place (an IIFE, a library carrier, a helper's callback) with the
/// callbacks of primitives, up to the call that runs the outermost of them, which is where `at` ends. The flags
/// of every segment are merged, so a caller reads one answer.
///
/// `crosses` is the walk's policy and is asked once per slot, innermost first. It matches [`SlotRun`] with no
/// wildcard and records what it needs of each slot it accepts (the owner, the observer, a thrown error), which
/// this does not know. A slot that is not accepted ends the climb at the call inside it. A callback with a
/// parameter, an `async` or generator callback, and a slot call written in a JSX expression (the compiler places
/// that, not the code around the element) end it too, as they end an in-place layer.
pub(crate) fn climb_through_slots(
    file: &FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
    mut crosses: impl FnMut(&SlotLayer<'_>) -> bool,
) -> Option<InPlaceClimb> {
    let mut merged = InPlaceClimb {
        exhausted: false,
        at: span,
        in_list: false,
        among_others: false,
        suspended: None,
        withheld: false,
    };
    // Bounded: each step moves to the call that holds the previous function.
    for step in 0..16 {
        if let Some(next) = super::climb_in_place_layers(file, merged.at, lookup) {
            merged.in_list |= next.in_list;
            merged.among_others |= next.among_others;
            merged.withheld |= next.withheld;
            merged.suspended = match (merged.suspended, next.suspended) {
                (Some(true), _) | (_, Some(true)) => Some(true),
                (Some(false), _) | (_, Some(false)) => Some(false),
                (None, None) => None,
            };
            merged.at = next.at;
            if next.exhausted {
                merged.exhausted = true;
                break;
            }
        }
        let Some(function) = owning_function(&file.ast, merged.at) else {
            break;
        };
        if function.generator
            || function.r#async
            || !function.parameters.is_empty()
            || function.rest_parameter
            || !function.body.contains(merged.at)
        {
            break;
        }
        let Some(layer) = slot_layer(file, function, lookup) else {
            break;
        };
        if call_in_jsx_expression(file, layer.call.span) || !crosses(&layer) {
            break;
        }
        merged.at = layer.call.span;
        merged.exhausted |= step == 15;
    }
    (merged.at != span).then_some(merged)
}
