//! ADR 1097 (reopened narrowly): a call after an await whose callee is a function that does nothing but call exact setters.
//!
//! ADR 0973 removed every callee reason: reading a callee's body and concluding that it reads nothing was unsound (the review of #233: reads reach a continuation
//! through aliases, getters, a stored generator's iteration, props aliases). This reason does not ask what a body reads. It asks whether the body is, by its facts,
//! nothing but calls of exact setters with inert arguments, which run no project code and read no source ([`super::library_call::runs_nothing`], the same proof a
//! direct setter call after an await gets). The callee, exactly:
//!
//! - the call has no arguments and is no construct, and its callee is an identifier bound by an exact same-file `const` (never written, not exported, an
//!   identifier binding, the reference the one the binding declares), or a chain of such `const x = y` aliases (at most [`ALIAS_DEPTH`]);
//! - the `const` holds a function literal (an arrow or a function expression) that is neither async nor a generator and takes no parameters, whose body passes
//!   [`body_only_calls_setters`]; or it holds an action (a symbol source discovery records as one) whose made-of argument is a generator literal whose body passes
//!   the same, with no `yield`.
//!
//! A body passes when, within it: no function, class, member read, spread, iteration, `yield`, `await` or tagged template is written; every referenced
//! identifier is the callee of a call there; and every call is an exact setter call that runs nothing ([`super::library_call::runs_nothing`] with `setter`: its
//! arguments inert, the signal's `equals` absent or `false`, no built-in touched), or, in a wrapper's body only, an argument-less call of an action that is
//! itself such a callee. Anything else (a helper that runs a callback, an IIFE, a destructured or tuple setter, a store updater, a prop, an import) is not this.

use solid_facts::FileFacts;
use solid_facts::ast::CallFact;
use solid_facts::core::Span;

use crate::pipeline::AnalysisContext;

/// How many `const x = y` aliases the callee may go through.
const ALIAS_DEPTH: usize = 4;

/// Whether the call `call` after an await runs only exact setters (see the module documentation).
pub(super) fn only_calls_setters(
    ctx: &AnalysisContext<'_>,
    file: &FileFacts,
    call: &CallFact,
    object_prototype_touched: &dyn Fn() -> bool,
) -> bool {
    // A body with no proved call never asks `runs_nothing`, so the built-ins question is asked here.
    super::library_call::builtins_untouched(ctx)
        && !call.construct
        && call.arguments.is_empty()
        && callee_body(
            ctx,
            file,
            file.ast.peel_ts_sugar_span(call.callee),
            true,
            object_prototype_touched,
        )
}

/// The function an exact `const` chain at `reference` names passes, as a wrapper (`wrapper`: its body may call actions) or as an action's generator.
fn callee_body(
    ctx: &AnalysisContext<'_>,
    file: &FileFacts,
    reference: Span,
    wrapper: bool,
    object_prototype_touched: &dyn Fn() -> bool,
) -> bool {
    let ast = &file.ast;
    let lookup = ctx.semantic_lookup;
    let mut at = reference;
    for _ in 0..=ALIAS_DEPTH {
        if !crate::owners::is_identifier_span(file, at) {
            return false;
        }
        let Some((binding_file, binding, _)) = lookup.binding_at_reference(file.path.as_str(), at)
        else {
            return false;
        };
        let Some(name) = binding.names.first() else {
            return false;
        };
        if binding_file.path != file.path
            || !binding.immutable
            || binding.shape != solid_facts::ast::BindingShape::Identifier
            || binding.names.len() != 1
            || ast.reference_declaration(at) != Some(name.span)
            || crate::indexes::binding_exported(file, name.span)
            || !matches!(
                ctx.entities.at(file.path.as_str(), name.span),
                Some(symbol) if !crate::value_identity::binding_has_write(file, ctx.entities, symbol)
            )
        {
            return false;
        }
        let Some(initializer) = binding.initializer.map(|span| ast.peel_ts_sugar_span(span)) else {
            return false;
        };
        // A function literal: the wrapper itself.
        if let Some(function) = ast.function_at(initializer) {
            return wrapper
                && !function.r#async
                && !function.generator
                && function.parameters.is_empty()
                && !function.rest_parameter
                && body_only_calls_setters(ctx, file, function, true, object_prototype_touched);
        }
        // An action: the generator it was made of.
        if let Some(made) = ast.call_at(initializer) {
            let symbol = ctx.entities.at(file.path.as_str(), name.span);
            if !symbol.is_some_and(|symbol| ctx.actions.contains_key(symbol)) || made.construct {
                return false;
            }
            let Some(generator) = made
                .arguments
                .first()
                .filter(|argument| !argument.spread)
                .and_then(|argument| ast.function_at(ast.peel_ts_sugar_span(argument.span)))
            else {
                return false;
            };
            return made.arguments.len() == 1
                && generator.generator
                && !generator.r#async
                && generator.parameters.is_empty()
                && !generator.rest_parameter
                && body_only_calls_setters(ctx, file, generator, false, object_prototype_touched);
        }
        // `const x = y`: the next link of the chain.
        at = initializer;
    }
    false
}

/// The body of `function` is nothing but exact setter calls (and, where `actions`, argument-less calls of actions that are such callees).
fn body_only_calls_setters(
    ctx: &AnalysisContext<'_>,
    file: &FileFacts,
    function: &solid_facts::ast::FunctionFact,
    actions: bool,
    object_prototype_touched: &dyn Fn() -> bool,
) -> bool {
    let ast = &file.ast;
    let body = function.body;
    let inside = |span: Span| body.contains(span);
    if ast.functions.iter().any(|other| other.span != function.span && inside(other.span))
        || ast.classes.iter().any(|class| inside(class.span))
        || ast.members.iter().any(|member| inside(member.span))
        || ast.spreads.iter().any(|spread| inside(spread.span))
        || ast.iterated_operands.iter().any(|operand| inside(*operand))
        || ast.yields.iter().any(|yielded| inside(yielded.span))
        || ast.awaits.iter().any(|awaited| inside(*awaited))
        || ast.tagged_templates.iter().any(|tagged| inside(tagged.span))
        // The second review of ADR 1097's widening: operators that run project code with no call (`in` and `instanceof` reach a Proxy trap or
        // `Symbol.hasInstance`, a coercion reaches `toString`, a computed key coerces its value), `for…in` over a Proxy, and `this`, which no fact names:
        // none is written in the body. `primitive_results` holds every such binary and prefix operator.
        || ast.primitive_results.iter().any(|operator| inside(*operator))
        || ast.coercing_operands.iter().any(|operand| inside(*operand))
        || ast.template_literals.iter().any(|template| inside(template.span))
        || ast.object_properties.iter().any(|property| inside(property.span))
        || ast.computed_keys.iter().any(|key| inside(key.read))
        || ast.loop_statements.iter().any(|statement| inside(*statement))
        || ast.jsx_elements.iter().any(|element| inside(element.span))
        || ast.module_loads.iter().any(|load| inside(load.span))
        || ast.assignments.iter().any(|assignment| inside(assignment.target))
        || ast.deleted_targets.iter().any(|target| inside(*target))
    {
        return false;
    }
    let calls = ast
        .calls
        .iter()
        .filter(|call| inside(call.span))
        .collect::<Vec<_>>();
    let callees = calls
        .iter()
        .map(|call| ast.peel_ts_sugar_span(call.callee))
        .collect::<Vec<_>>();
    let referenced_only_as_callees = ast.identifiers.iter().all(|identifier| {
        identifier.role != solid_facts::ast::IdentifierRole::Reference
            || !inside(identifier.span)
            || callees.contains(&identifier.span)
    });
    referenced_only_as_callees
        && calls.iter().all(|call| {
            !call.construct
                && (super::library_call::runs_nothing(
                    ctx,
                    file,
                    call,
                    "setter",
                    object_prototype_touched,
                ) || (actions
                    && call.arguments.is_empty()
                    && callee_body(
                        ctx,
                        file,
                        ast.peel_ts_sugar_span(call.callee),
                        false,
                        object_prototype_touched,
                    )))
        })
}
