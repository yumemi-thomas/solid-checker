//! ADR 1097 (reopened narrowly, widened by the coordinator's ruling of 2026-11-19): a call after an await that runs nothing but proved setter-like calls.
//!
//! ADR 0973 removed every callee reason: reading a callee's body and concluding that it reads nothing was unsound (the review of #233: reads reach a continuation
//! through aliases, getters, a stored generator's iteration, props aliases). This reason does not ask what a body reads. It asks whether the body is, by its facts,
//! nothing but calls each of which is proved by a fact the pass already proves elsewhere:
//!
//! - an exact setter call that runs nothing ([`super::library_call::runs_nothing`] with `setter`: a `createSignal` setter whose arguments are inert and whose
//!   `equals` is absent or `false`, or a plain store's setter with a literal-only updater, ADR 1089 D), called by the setter's own name or through exact `const`
//!   aliases of it ([`resolve_alias`]);
//! - `refresh(accessor)`: a call of a primitive whose one argument the dialect says is a target it neither calls nor hands on
//!   (`Dialect::argument_is_untouched_target`, ADR 1089 C), the argument an identifier whose value is an accessor;
//! - an argument-less call of a qualifying callee: through exact `const` aliases, a same-file, unexported, never-written `const` holding a parameterless
//!   function literal that is neither async nor a generator, whose body qualifies (transitively, at most [`DEPTH`] deep, never itself again);
//! - an argument-less call of an action that source discovery records, whose made-of generator has no `yield` and whose body qualifies;
//! - an argument-less IIFE of such a function literal.
//!
//! Outside the proved calls' own arguments (and the IIFEs' own bodies, asked on their own), a body holds no function, class, member, spread, iteration,
//! `yield`, `await`, tagged template, JSX, load, assignment or `delete`, and every identifier it references is the callee of one of its calls. A destructured
//! setter from anything but a `createSignal` or `createStore` tuple, a member, a spread, a getter, a non-inert argument, an exported or written callee, a
//! helper that runs a callback, a parameter: none of these is proved.

use solid_facts::FileFacts;
use solid_facts::ast::{CallFact, FunctionFact};
use solid_facts::core::Span;

use crate::pipeline::AnalysisContext;

/// How many calls deep a body may go through qualifying callees.
const DEPTH: usize = 4;
/// How many `const x = y` aliases one name may go through.
const ALIAS_DEPTH: usize = 4;

/// Whether the call `call` after an await runs only proved calls (see the module documentation).
pub(super) fn only_calls_setters(
    ctx: &AnalysisContext<'_>,
    file: &FileFacts,
    call: &CallFact,
    object_prototype_touched: &dyn Fn() -> bool,
) -> bool {
    let mut visiting = Vec::new();
    proved_call(ctx, file, call, 0, &mut visiting, object_prototype_touched).is_some()
}

/// A same-file `const` identifier binding at the reference `at`: never written, not exported, the one name of its binding, the reference the one it declares.
/// Its initializer, peeled.
fn exact_const_initializer(ctx: &AnalysisContext<'_>, file: &FileFacts, at: Span) -> Option<Span> {
    let ast = &file.ast;
    if !crate::owners::is_identifier_span(file, at) {
        return None;
    }
    let (binding_file, binding, _) = ctx.semantic_lookup.binding_at_reference(file.path.as_str(), at)?;
    let [name] = binding.names.as_slice() else {
        return None;
    };
    let exact = binding_file.path == file.path
        && binding.immutable
        && binding.shape == solid_facts::ast::BindingShape::Identifier
        && ast.reference_declaration(at) == Some(name.span)
        && !crate::indexes::binding_exported(file, name.span)
        && matches!(
            ctx.entities.at(file.path.as_str(), name.span),
            Some(symbol) if !crate::value_identity::binding_has_write(file, ctx.entities, symbol)
        );
    exact.then(|| binding.initializer.map(|span| ast.peel_ts_sugar_span(span)))?
}

/// The reference a chain of exact `const x = y` aliases at `at` ends at: `at` itself when its binding is no such alias.
fn resolve_alias(ctx: &AnalysisContext<'_>, file: &FileFacts, at: Span) -> Option<Span> {
    let mut at = at;
    for _ in 0..ALIAS_DEPTH {
        match exact_const_initializer(ctx, file, at) {
            Some(next) if crate::owners::is_identifier_span(file, next) => at = next,
            _ => return Some(at),
        }
    }
    None
}

/// The span to leave out of the body's own scans when `call` is proved: its arguments for a setter or a `refresh`, its whole span for an IIFE, nothing more
/// for an argument-less callee. `None` when the call is not proved.
fn proved_call(
    ctx: &AnalysisContext<'_>,
    file: &FileFacts,
    call: &CallFact,
    depth: usize,
    visiting: &mut Vec<Span>,
    object_prototype_touched: &dyn Fn() -> bool,
) -> Option<Span> {
    let ast = &file.ast;
    if call.construct {
        return None;
    }
    let callee = ast.peel_ts_sugar_span(call.callee);
    let arguments = Span::new(callee.end, call.span.end);
    // An IIFE of a function literal whose body qualifies.
    if let Some(function) = ast.function_at(callee) {
        return (call.arguments.is_empty()
            && plain_function(function)
            && body_qualifies(ctx, file, function, depth + 1, visiting, object_prototype_touched))
        .then_some(call.span);
    }
    let target = resolve_alias(ctx, file, callee)?;
    // An exact setter, by its own name or an alias of it: the same proof a direct setter call after an await gets.
    let mut named = call.clone();
    named.callee = target;
    if super::library_call::runs_nothing(ctx, file, &named, "setter", object_prototype_touched) {
        return Some(arguments);
    }
    // `refresh(accessor)`: ADR 1089 C's untouched target.
    if let Some(primitive) = ctx.semantic_lookup.primitive_at_call(file, call.span) {
        let [argument] = call.arguments.as_slice() else {
            return None;
        };
        let value = ast.peel_ts_sugar_span(argument.span);
        return (!argument.spread
            && ctx.dialect.argument_is_untouched_target(primitive, 0)
            && crate::owners::is_identifier_span(file, value)
            && ctx
                .entities
                .value_symbol(file, value)
                .is_some_and(|symbol| ctx.accessors.contains_key(symbol)))
        .then_some(arguments);
    }
    if !call.arguments.is_empty() {
        return None;
    }
    let initializer = exact_const_initializer(ctx, file, target)?;
    // A qualifying callee: a function literal whose body qualifies, never one the chain is already in.
    if let Some(function) = ast.function_at(initializer) {
        return (plain_function(function)
            && !visiting.contains(&function.span)
            && body_qualifies(ctx, file, function, depth + 1, visiting, object_prototype_touched))
        .then_some(Span::new(call.span.end, call.span.end));
    }
    // An action source discovery records, made of a generator whose body qualifies.
    let made = ast.call_at(initializer)?;
    let name = ctx.semantic_lookup.binding_at_reference(file.path.as_str(), target)?.1.names.first()?.span;
    let action = ctx
        .entities
        .at(file.path.as_str(), name)
        .is_some_and(|symbol| ctx.actions.contains_key(symbol));
    let [made_of] = made.arguments.as_slice() else {
        return None;
    };
    let generator = (!made_of.spread)
        .then(|| ast.function_at(ast.peel_ts_sugar_span(made_of.span)))
        .flatten()?;
    (action
        && !made.construct
        && generator.generator
        && !generator.r#async
        && generator.parameters.is_empty()
        && !generator.rest_parameter
        && !visiting.contains(&generator.span)
        && body_qualifies(ctx, file, generator, depth + 1, visiting, object_prototype_touched))
    .then_some(Span::new(call.span.end, call.span.end))
}

/// A parameterless function literal that is neither async nor a generator.
fn plain_function(function: &FunctionFact) -> bool {
    !function.r#async && !function.generator && function.parameters.is_empty() && !function.rest_parameter
}

/// The body of `function` qualifies: every outer call in it is proved, and outside the proved calls' left-out spans it holds none of the shapes the module
/// documentation names, and references no name but its calls' callees.
fn body_qualifies(
    ctx: &AnalysisContext<'_>,
    file: &FileFacts,
    function: &FunctionFact,
    depth: usize,
    visiting: &mut Vec<Span>,
    object_prototype_touched: &dyn Fn() -> bool,
) -> bool {
    if depth > DEPTH {
        return false;
    }
    visiting.push(function.span);
    let answer = body_qualifies_inner(ctx, file, function, depth, visiting, object_prototype_touched);
    visiting.pop();
    answer
}

fn body_qualifies_inner(
    ctx: &AnalysisContext<'_>,
    file: &FileFacts,
    function: &FunctionFact,
    depth: usize,
    visiting: &mut Vec<Span>,
    object_prototype_touched: &dyn Fn() -> bool,
) -> bool {
    let ast = &file.ast;
    let body = function.body;
    let calls = ast.calls.iter().filter(|call| body.contains(call.span)).collect::<Vec<_>>();
    // The outer calls: in no other call of the body's arguments or IIFE. Each must be proved; what it leaves out is asked by its own proof.
    let outer = calls
        .iter()
        .filter(|call| {
            !calls.iter().any(|other| {
                other.span != call.span
                    && other.span.contains(call.span)
                    && !Span::new(other.span.start, ast.peel_ts_sugar_span(other.callee).end).contains(call.span)
            }) && !ast
                .functions
                .iter()
                .any(|inner| inner.span != function.span && body.contains(inner.span) && inner.span.contains(call.span))
        })
        .collect::<Vec<_>>();
    let mut left_out = Vec::with_capacity(outer.len());
    for call in &outer {
        match proved_call(ctx, file, call, depth, visiting, object_prototype_touched) {
            Some(span) => left_out.push(span),
            None => return false,
        }
    }
    let free = |span: Span| body.contains(span) && !left_out.iter().any(|out| out.contains(span));
    if ast.functions.iter().any(|other| other.span != function.span && free(other.span))
        || ast.classes.iter().any(|class| free(class.span))
        || ast.members.iter().any(|member| free(member.span))
        || ast.spreads.iter().any(|spread| free(spread.span))
        || ast.iterated_operands.iter().any(|operand| free(*operand))
        || ast.yields.iter().any(|yielded| free(yielded.span))
        || ast.awaits.iter().any(|awaited| free(*awaited))
        || ast.tagged_templates.iter().any(|tagged| free(tagged.span))
        || ast.jsx_elements.iter().any(|element| free(element.span))
        || ast.module_loads.iter().any(|load| free(load.span))
        || ast.assignments.iter().any(|assignment| free(assignment.target))
        || ast.deleted_targets.iter().any(|target| free(*target))
    {
        return false;
    }
    let callees = outer.iter().map(|call| ast.peel_ts_sugar_span(call.callee)).collect::<Vec<_>>();
    ast.identifiers.iter().all(|identifier| {
        identifier.role != solid_facts::ast::IdentifierRole::Reference
            || !free(identifier.span)
            || callees.contains(&identifier.span)
    })
}
