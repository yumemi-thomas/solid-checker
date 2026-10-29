//! Retained value literals: whether a function literal runs in an object or array that outlives the call.

use super::*;

/// Whether the code at `span` sits in an anonymous function literal, written
/// inside the summary node `owner`, that the node's call does **not** run: the
/// value of an object-literal property (`return { busy: () => state() }`, or a
/// getter) or an element of a returned array literal
/// (`return [state, () => state() > 0]`).
///
/// [`discover_summary_nodes`] admits a function only when it is bound, a
/// method, or in the TypeScript function universe, so such a literal is no
/// node and the nearest node containing its reads is the enclosing hook. That
/// folds `state()` into the *call* of the hook -- which only builds the object
/// -- and every component body that calls the hook is then charged with a
/// strict-window read of `state` it never performs (about 192 of 1,962
/// measured `strict-read-untracked` violations; `createMutation().busy`). The
/// closure runs when its holder invokes it, so its reads are neither the
/// owner's nor reachable from the owner's call edges.
///
/// Only the two shapes whose holder is a plain value qualify, and only when no
/// call argument inside the owner contains the literal: an object or array
/// handed to a call (`track({ run: () => state() })`) may be invoked by that
/// call, which is the callee-timing question and keeps its legacy attribution.
/// A method or a bound arrow is a node of its own and is never asked here. The
/// walk covers every function between `span` and the owner, so a read in
/// `() => items.map(() => state())` inside a property closure is covered by the
/// property closure.
pub(super) fn runs_in_retained_value_literal(
    file: &solid_facts::FileFacts,
    span: Span,
    owner: &SummaryNode,
) -> bool {
    crate::execution_role::functions_around(file, span)
        .into_iter()
        .any(|literal| {
            literal.span != owner.span
            && owner.body.contains(literal.span)
            && literal.name.is_none()
            && literal.method_name.is_none()
            && function_binding_name(file, literal).is_none()
            // The indexed argument query goes first: most anonymous literals
            // are callbacks, and the property scan below is linear.
            && !file
                .ast
                .arguments_containing(literal.span)
                .any(|(call, _)| owner.body.contains(call.span))
            && is_retained_value_position(file, literal.span, owner.body)
        })
}

/// Whether the call at `span`, written inside a component, sits in a function
/// literal that nothing proves runs while the component body does: a literal
/// stored in a binding or an object property, returned, or written as a JSX
/// attribute value (`const api = { go: () => helper() }`,
/// `return () => helper()`, `ref={() => helper()}`).
///
/// The summary path may attribute a call to the body only when invocation
/// during that body is established, and the lexical `inside_component` role
/// that [`semantic_execution_role`] falls back to proves nothing of the kind,
/// so this asks first and never depends on that fallback answering "body".
///
/// The walk goes outward from the innermost function. A (possible) component
/// is the body itself: not this case. An IIFE runs where it is written, so the
/// walk continues from its call; a control-flow component's render callback
/// runs while its children render. A literal that is a call argument is *not*
/// answered here: whether that callee runs it during the call is the
/// callee-timing question [`callee_callback_timing`] already reports (a
/// not-proven result, never a proven one), so it keeps that answer.
pub(super) fn runs_in_unproven_stored_literal(
    file: &solid_facts::FileFacts,
    span: Span,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let mut span = span;
    loop {
        let Some(literal) = containing_ast_function(&file.ast, span) else {
            return false;
        };
        if lookup.function_component_status(file, literal) != ComponentStatus::No {
            return false;
        }
        // Calling a generator function runs none of its body.
        if literal.generator {
            return false;
        }
        if file
            .ast
            .arguments_containing(literal.span)
            .any(|(call, index)| {
                file.ast
                    .functions_within(call.arguments[index].span)
                    .filter(|function| function.span.contains(literal.span))
                    .max_by_key(|function| function.span.end - function.span.start)
                    .is_some_and(|outer| outer.span == literal.span)
            })
        {
            return false;
        }
        if let Some(call) = file
            .ast
            .calls
            .iter()
            .filter(|call| file.ast.peel_ts_sugar_span(call.callee) == literal.span)
            .min_by_key(|call| call.span.end - call.span.start)
        {
            span = call.span;
            continue;
        }
        // ADR 0489: a function bound to a name whose only run is its one call
        // (ADR 0479, `layer_run`'s `InPlace`) runs where that call is, as an
        // IIFE does: the walk continues from the call.
        if let crate::execution_role::LayerRun::InPlace(call) =
            crate::execution_role::layer_run(file, literal, lookup)
            && call != span
            && !literal.span.contains(call)
        {
            span = call;
            continue;
        }
        return control_flow_execution_role(file, span, entities, symbol_names, lookup.dialect)
            .is_none();
    }
}

/// An object member retains a callable; constructing or passing the object
/// does not invoke it. A JSX-returning member can satisfy component discovery
/// without having a render invocation, so that classification must not lend
/// its definition a body role. Its summary still projects reads at exact
/// calls, with those calls' own execution roles.
pub(super) fn runs_in_object_callable(file: &solid_facts::FileFacts, span: Span) -> bool {
    object_retention_witnesses(file, span).next().is_some()
}

/// Every `(function, property)` pair behind [`runs_in_object_callable`]: an
/// enclosing function of `span` and an object property holding it.
pub(super) fn object_retention_witnesses(
    file: &solid_facts::FileFacts,
    span: Span,
) -> impl Iterator<
    Item = (
        &solid_facts::ast::FunctionFact,
        &solid_facts::ast::ObjectPropertyFact,
    ),
> {
    // ADR 0454: also the functions whose parameter list holds the site, as
    // `functions_around` finds them.
    file.ast.functions_owning(span).flat_map(move |function| {
        let binding = function_binding_name(file, function).map(|name| name.span);
        file.ast
            .object_properties
            .iter()
            .filter(move |property| {
                let value = file
                    .ast
                    .peel_ts_sugar_span(property.shorthand_binding.unwrap_or(property.value));
                value == function.span
                    || binding.is_some_and(|binding| {
                        value == binding || file.ast.reference_declaration(value) == Some(binding)
                    })
            })
            .map(move |property| (function, property))
    })
}

/// [`runs_in_object_callable`] less two dispositions (ADR 0292, 0342): a
/// witness that is the `default` of a module object a dialect component
/// loader renders, and the function's one reference, is a render of that
/// function, not a retention; so is a member of a compound component that
/// only JSX member tags reach, of a function proven a component. It is withdrawn only for a call written directly in that
/// function's own body; every other witness, an enclosing function's
/// included, stays in force.
pub(super) fn runs_in_unrendered_object_callable(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    object_retention_witnesses(file, span).any(|(function, property)| {
        !(crate::owners::written_directly_in(&file.ast, function, span)
            && (lookup.is_lazy_default_render_witness(file, function.span, property.span)
                || lookup.is_compound_render_witness(file, function, property)))
    })
}

pub(super) fn is_retained_value_position(
    file: &solid_facts::FileFacts,
    literal: Span,
    owner_body: Span,
) -> bool {
    // Any property whose value is the literal: a data property, and also a
    // getter or setter, whose body runs on access and never while the object
    // is built.
    file.ast.object_properties.iter().any(|property| {
        property.span.contains(literal) && file.ast.peel_ts_sugar_span(property.value) == literal
    }) || file.ast.returns_within(owner_body).any(|returned| {
        returned
            .elements()
            .iter()
            .flatten()
            .any(|element| file.ast.peel_ts_sugar_span(*element) == literal)
    })
}

/// ADR 0321: the summary identity of a function literal that is exactly the
/// value of an object-literal property with a static key (`{ run: () => … }`,
/// `{ onX: function () { … } }`). It has no binding and no method name, so
/// TypeScript gives it no symbol of its own; the identity is its file and
/// span, which no other function shares. A method (`run() {}`) keeps its
/// method symbol.
pub(crate) fn literal_property_function_symbol(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
) -> Option<SymbolId> {
    literal_property_function_key(file, function)?;
    Some(SymbolId::from(format!(
        "\0literal-property:{}:{}:{}",
        file.path.as_str(),
        function.span.start,
        function.span.end
    )))
}

/// Whether `symbol` is a [`literal_property_function_symbol`].
pub(super) fn is_literal_property_function_symbol(symbol: &SymbolId) -> bool {
    symbol.as_str().starts_with("\0literal-property:")
}

/// The static key of the object-literal property whose value is exactly the
/// function literal `function` (see [`literal_property_function_symbol`]).
pub(super) fn literal_property_function_key(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
) -> Option<String> {
    if function.method_name.is_some() || function_binding_name(file, function).is_some() {
        return None;
    }
    let property = file.ast.object_properties.iter().find(|property| {
        !property.computed
            && property.shorthand_binding.is_none()
            && file.ast.peel_ts_sugar_span(property.value) == function.span
    })?;
    // Only a literal written as a call's argument: its members are reached
    // through that call's parameter, where the dispatch is resolved. A literal
    // returned, bound or stored is reached by member calls on its holder,
    // which name the property's own symbol, and keeps its reads where they
    // were (in the function that writes it).
    let literal = file
        .ast
        .object_get_shapes
        .iter()
        .filter(|shape| shape.span.contains(property.span))
        .min_by_key(|shape| shape.span.end - shape.span.start)?;
    if !file.ast.calls.iter().any(|call| {
        call.arguments.iter().any(|argument| {
            !argument.spread && file.ast.peel_ts_sugar_span(argument.span) == literal.span
        })
    }) {
        return None;
    }
    crate::execution_role::property_static_key(file, property)
}
