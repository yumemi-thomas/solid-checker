//! Instantiate value Gets at a package call, never on a getter declaration.
//! The first slice admits fresh closed literals and one non-escaped const
//! binding. Spreads and aliases remain obligations. ADR 0274 adds a local
//! helper that forwards its exact parameter to the package call, and every
//! getter a hazard-free prefix reaches. ADR 0283 adds a project function's
//! own member Gets of a parameter, on its every-completion path.

use super::*;
use solid_facts::ast::{BindingShape, ObjectGetPropertyFact, ObjectGetShapeFact};

/// The call a receiver's getters are instantiated at, and what reading them
/// there needs. Shared by the summary rows and a project function's own
/// parameter Gets.
pub(super) struct ReadContext<'a, 'b> {
    pub file: &'a solid_facts::FileFacts,
    pub call: &'a solid_facts::ast::CallFact,
    pub valid: bool,
    pub execution: ExecutionRole,
    pub label: &'a str,
    pub source_kinds: &'a super::transport_reads::SourceKindsRead<'a>,
    pub accessors: &'a super::transport_reads::AccessorsRead<'a>,
    pub entities: &'a EntitySymbols,
    pub symbol_names: &'a HashMap<SymbolId, SymbolId>,
    pub lookup: &'a SemanticLookup<'b>,
}

/// The binder names the declaration, not a same-spelled local. The object
/// must never escape to an alias, member dispatch or descriptor mutation.
/// A spread *of* it cannot change its own descriptors and is allowed; a
/// spread *inside* its literal is not resolved in this initial slice.
fn receiver<'a>(
    file: &'a solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    parameter: usize,
) -> Option<&'a ObjectGetShapeFact> {
    let ast = &file.ast;
    let argument = call.arguments.get(parameter)?;
    if argument.spread {
        return None;
    }
    let mut span = ast.peel_ts_sugar_span(argument.span);
    if let Some(declaration) = ast.reference_declaration(span) {
        // Direct `eval` reaches the binding with no static reference, so it
        // may delete or redefine a getter before the call (ADR 0283).
        if direct_eval_in(file, Span::new(0, u32::MAX)) {
            return None;
        }
        let mut candidates = ast.bindings.iter().filter(|binding| {
            binding.shape == BindingShape::Identifier
                && binding.immutable
                && binding.names.len() == 1
                && binding.names[0].span == declaration
        });
        let binding = candidates.next()?;
        if candidates.next().is_some()
            || binding.declaration.end > call.span.start
            || containing_ast_function(ast, binding.declaration).map(|owner| owner.span)
                != containing_ast_function(ast, call.span).map(|owner| owner.span)
            || ast
                .loop_statements
                .iter()
                .any(|looped| looped.contains(call.span))
            // Only a module-level binding can be exported. An exported
            // function's span contains its whole body, so a local `const`
            // inside `export function App() {}` is not itself exported.
            || (containing_ast_function(ast, binding.declaration).is_none()
                && ast
                    .exports
                    .iter()
                    .any(|export| export.span.contains(binding.declaration)))
            || ast
                .exports
                .iter()
                .flat_map(|export| &export.specifiers)
                .any(|specifier| {
                    ast.reference_declaration(specifier.local.span) == Some(declaration)
                })
        {
            return None;
        }
        for (reference, bound) in &ast.reference_declarations {
            if *bound != declaration {
                continue;
            }
            let allowed = ast.peel_ts_sugar_span(argument.span) == *reference
                || ast.spreads.iter().any(|spread| {
                    if ast.peel_ts_sugar_span(spread.argument) != *reference
                        || spread.span.start < call.span.end
                    {
                        return false;
                    }
                    let call_owner =
                        containing_ast_function(ast, call.span).map(|owner| owner.span);
                    let spread_owner =
                        containing_ast_function(ast, spread.span).map(|owner| owner.span);
                    // A same-frame later read cannot mutate this allocation
                    // before its sole construction. A future function literal
                    // handed to a later same-frame call likewise cannot run
                    // yet. A hoisted/named callback is not such a proof.
                    spread_owner == call_owner
                        || ast.arguments_containing(spread.span).any(|(outer, slot)| {
                            let argument = &outer.arguments[slot];
                            outer.span.start > call.span.end
                                && !argument.spread
                                && Some(ast.peel_ts_sugar_span(argument.span)) == spread_owner
                                && containing_ast_function(ast, outer.span).map(|owner| owner.span)
                                    == call_owner
                        })
                });
            if !allowed {
                return None;
            }
        }
        span = ast.peel_ts_sugar_span(binding.initializer?);
    }
    ast.object_get_shapes
        .iter()
        .find(|shape| shape.span == span && shape.closed)
}

fn getter_function(
    file: &solid_facts::FileFacts,
    getter: Span,
) -> Option<&solid_facts::ast::FunctionFact> {
    file.ast.function_at(getter)
}

/// Whether `getter` may reach its receiver. The receiver is a fresh
/// unaliased literal, so a getter can reach it only as its receiver: `this`,
/// `super` (whose member calls receive the literal), or code `eval` builds.
/// A false match only withholds.
fn reaches_receiver(
    file: &solid_facts::FileFacts,
    getter: &solid_facts::ast::FunctionFact,
) -> bool {
    file.source_text(getter.body)
        .is_none_or(mentions_receiver_route)
        || direct_eval_in(file, getter.span)
}

/// The accessor reads a Get of `property` performs through `getter` at
/// `context.call`, appended to `reads`: located at the call, with the
/// getter's read as origin and the call's role. Only a call on the getter's
/// own every-completion path (`site_on_every_completion`, ADR 0283) counts; a
/// call in a branch arm or after an early `return` is left out rather than
/// reported as unproven. Returns whether the body holds anything this proof
/// leaves open.
fn getter_reads(
    context: &ReadContext<'_, '_>,
    property: &ObjectGetPropertyFact,
    getter: &solid_facts::ast::FunctionFact,
    guaranteed: bool,
    reads: &mut Vec<ReactiveRead>,
    dependencies: &mut HashSet<InterproceduralResultDependency>,
) -> bool {
    let mut unresolved = false;
    for call in context.file.ast.calls.iter().filter(|call| {
        crate::owners::written_directly_in(&context.file.ast, getter, call.span)
            && getter.body.contains(call.span)
    }) {
        // A class written in the getter defers its field initializers to
        // instantiation; nothing proves when, or that, they run.
        if inside_class_in(&context.file.ast, getter.body, call.span) {
            unresolved = true;
            continue;
        }
        if !site_on_every_completion(context.lookup, context.file, getter.span, call.span) {
            unresolved = true;
            continue;
        }
        let Some(symbol) = context
            .entities
            .get(&location(context.file.path.shared(), call.callee))
        else {
            unresolved = true;
            continue;
        };
        if context.source_kinds.get(symbol) != Some(&ReactiveSourceKind::Accessor)
            || !source_has_runtime_witness(
                symbol,
                context.accessors,
                context.entities,
                context.symbol_names,
                context.lookup,
            )
        {
            // Local helper calls and arbitrary package calls need their
            // own entered-body proof; do not call them inert.
            unresolved = true;
            continue;
        }
        let Some((display, declaration)) = context.accessors.get(symbol) else {
            unresolved = true;
            continue;
        };
        dependencies.insert(InterproceduralResultDependency::Symbol(symbol.clone()));
        // A read written directly in a reached getter body executes under
        // the statically-present-code premise; `entry_call` is no longer
        // the only guaranteed read.
        reads.push(ReactiveRead {
            kind: "accessor".into(),
            accessor: display.to_string().into(),
            location: location(context.file.path.shared(), context.call.span),
            declaration: declaration.clone(),
            execution: context.execution,
            context: enclosing_function_label(context.file, context.call.span).into(),
            via: context.label.into(),
            origin: Some(location(context.file.path.shared(), call.span)),
            origin_context: format!("get {}", property.key).into(),
            uncertain: false,
            missing_jsx_census: missing_jsx_census(
                context.file,
                context.call.span,
                context.execution,
            ),
            host_callback_timing: host_callback_timing(
                context.file,
                context.call.span,
                context.execution,
                context.lookup,
            ),
            callee_callback_timing: callee_callback_timing(
                context.file,
                context.call.span,
                context.execution,
                context.lookup,
            ),
            project_consumer_non_strict: false,
            callback_invocation_unproven: !guaranteed,
            summary_attributed: !guaranteed,
            transported: false,
        });
    }
    // Member reads can themselves enter getters or a reactive proxy.
    // This first slice proves accessor Calls only; a member-only getter
    // never becomes a clean result just because that call loop is empty.
    unresolved
        || context.file.ast.members.iter().any(|member| {
            crate::owners::written_directly_in(&context.file.ast, getter, member.span)
                && getter.body.contains(member.span)
        })
}

/// Whether `within` holds a direct `eval`, which reaches local bindings and
/// `this` with no static reference. Transparent TypeScript wrappers erase at
/// run time, so `(eval as typeof eval)(...)` and `eval!(...)` are direct too;
/// an escaped spelling is refused by the callers' backslash checks.
pub(crate) fn direct_eval_in(file: &solid_facts::FileFacts, within: Span) -> bool {
    file.ast.module_hazards.iter().any(|hazard| {
        hazard.kind == solid_facts::ast::ModuleHazardKind::Eval && within.contains(hazard.span)
    }) || file
        .eval_call_spans()
        .iter()
        .any(|call| within.contains(*call))
}

/// Whether `span` lies in a class or JSX element written inside `within`. A
/// field initializer runs at instantiation, and the compiler wraps a dynamic
/// JSX prop or child in a getter its consumer runs; neither runs where it is
/// written.
fn inside_class_in(ast: &solid_facts::ast::AstFacts, within: Span, span: Span) -> bool {
    ast.classes
        .iter()
        .any(|class| within.contains(class.span) && class.span.contains(span))
        || ast
            .jsx_elements
            .iter()
            .any(|element| within.contains(element.span) && element.span.contains(span))
        || ast
            .jsx_fragments
            .iter()
            .any(|fragment| within.contains(*fragment) && fragment.contains(span))
}

/// ADR 0283: the getter reads a project function's own member Gets of its
/// parameters perform at `context.call`, and the result dependencies they
/// rest on. Emits nothing unless every step is proven, so an unproven case
/// keeps the getter's existing uncertifiable read and gains nothing.
///
/// Per argument: the receiver is ADR 0225's exact closed literal; the callee
/// is one current project function with a summary node, reached with an
/// exact argument-to-parameter mapping (`project_callee_at_parameter`, ADR
/// 0278); no getter of the receiver may reach the receiver; and the
/// function Gets that key on its every-completion path. Each getter
/// then contributes only the accessor calls on its own every-completion
/// path. `has_summary_node` keeps the `FunctionSource` dependency live: a
/// dependency on a function without a node stays `Missing` across every
/// edit, so a cached answer could never be invalidated.
pub(super) fn parameter_reads(
    context: &ReadContext<'_, '_>,
    has_summary_node: impl Fn(&str, Span) -> bool,
) -> (Vec<ReactiveRead>, HashSet<InterproceduralResultDependency>) {
    let mut reads = Vec::new();
    let mut dependencies = HashSet::default();
    // Invalid calls belong to TypeScript. A construction runs a class body
    // whose fields and `super` call this slice does not model.
    if !context.valid || context.call.construct {
        return (reads, dependencies);
    }
    for index in 0..context.call.arguments.len() {
        let Some(shape) = receiver(context.file, context.call, index) else {
            continue;
        };
        if shape
            .properties
            .iter()
            .all(|property| property.getter.is_none())
        {
            continue;
        }
        let Some((callee_file, callee, exact)) = crate::execution_role::project_callee_at_parameter(
            context.file,
            context.call,
            index,
            context.lookup,
        ) else {
            continue;
        };
        if !has_summary_node(callee_file.path.as_str(), callee.span) {
            continue;
        }
        // Recorded for every evaluated pair, refusals included: the answer
        // is a function of the callee's body, which no read summary states.
        dependencies.insert(InterproceduralResultDependency::FunctionSource(
            callee_file.path.to_string(),
            callee.span,
        ));
        if !exact {
            continue;
        }
        // Every getter, not only the read one: the function may Get another
        // key first, and a getter that reaches its receiver may delete or
        // redefine the one read later.
        let Some(getters) = shape
            .properties
            .iter()
            .filter_map(|property| property.getter.map(|getter| (property, getter)))
            .map(|(property, getter)| {
                getter_function(context.file, getter)
                    .filter(|getter| !reaches_receiver(context.file, getter))
                    .map(|getter| (property, getter))
            })
            .collect::<Option<Vec<_>>>()
        else {
            continue;
        };
        let mut keys = parameter_gets(callee_file, callee, context.lookup)
            .into_iter()
            .filter(|(parameter, _)| *parameter == index)
            .map(|(_, key)| key)
            .collect::<Vec<_>>();
        keys.sort_unstable();
        keys.dedup();
        for (property, getter) in getters
            .into_iter()
            .filter(|(property, _)| keys.binary_search(&property.key.as_str()).is_ok())
        {
            // Anything a getter leaves open is already the getter's own
            // uncertifiable read; there is nothing further to owe it.
            let _ = getter_reads(
                context,
                property,
                getter,
                true,
                &mut reads,
                &mut dependencies,
            );
        }
    }
    (reads, dependencies)
}

/// ADR 0283: the `(parameter, key)` Gets `function` performs on every normal
/// completion of a call of it, before any suspension. Empty unless its
/// value is current.
fn parameter_gets<'a>(
    file: &'a solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
) -> Vec<(usize, &'a str)> {
    if !lookup.function_value_is_current(file, function) {
        return Vec::new();
    }
    parameter_get_sites(file, function)
        .into_iter()
        .filter(|(_, _, site)| site_on_every_completion(lookup, file, function.span, *site))
        .map(|(parameter, key, _)| (parameter, key))
        .collect()
}

/// The syntactic half of [`parameter_gets`]: `(parameter, key, member)` for
/// each static member Get of a parameter written in `function`'s body on
/// its straight-line flow (`AstFacts::completion_sites`, then
/// `body_site_runs_during_call`).
///
/// The parameter must be a plain, unwritten, default-less identifier, and
/// every reference to it must be the object of such a Get
/// ([`parameter_get`]). So it never escapes, is never aliased, never reaches
/// another function, and is never written through: the only code that can
/// run against the receiver during the call is its own getters. Refused
/// whole for an async function or a generator (its body may run after the
/// call returns), any directive (the compiler or host may replace the body,
/// as in ADR 0274), a name `arguments` or an escaped identifier (which may
/// spell it) in the function, and a direct `eval` in the file.
fn parameter_get_sites<'a>(
    file: &'a solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
) -> Vec<(usize, &'a str, Span)> {
    let ast = &file.ast;
    if function.r#async
        || function.generator
        || !function.directives.is_empty()
        || !ast.module_directives.is_empty()
        || ast.identifiers.iter().any(|identifier| {
            function.span.contains(identifier.span)
                && file
                    .source_text(identifier.span)
                    .is_none_or(|text| text == "arguments" || text.contains('\\'))
        })
        || direct_eval_in(file, Span::new(0, u32::MAX))
    {
        return Vec::new();
    }
    let mut sites = Vec::new();
    for (index, parameter) in function.parameters.iter().enumerate() {
        if parameter.shape != BindingShape::Identifier
            || parameter.names.len() != 1
            || parameter.initializer.is_some()
        {
            continue;
        }
        let declaration = parameter.names[0].span;
        if crate::indexes::binding_written(file, declaration) {
            continue;
        }
        let Some(gets) = ast
            .reference_declarations
            .iter()
            .filter(|(_, bound)| *bound == declaration)
            .map(|(reference, _)| parameter_get(file, *reference))
            .collect::<Option<Vec<_>>>()
        else {
            continue;
        };
        sites.extend(
            gets.into_iter()
                .filter(|(member, _)| {
                    function.body.contains(*member)
                        && ast.completion_sites.binary_search(member).is_ok()
                        && containing_ast_function(ast, *member)
                            .is_some_and(|owner| owner.span == function.span)
                        && body_site_runs_during_call(file, function, *member)
                })
                .map(|(member, key)| (index, key, member)),
        );
    }
    sites
}

/// The member whose object is `reference`, and its key, when that member
/// only performs a Get of a static key: `p.key`, under transparent
/// TypeScript wrappers. Not computed or optional, not a callee or a tag
/// (which receive `p` as `this`), and inside no write, `delete`,
/// destructuring or iteration target, compound and update writes included.
fn parameter_get(file: &solid_facts::FileFacts, reference: Span) -> Option<(Span, &str)> {
    let ast = &file.ast;
    let member = ast
        .members
        .iter()
        .find(|member| ast.peel_ts_sugar_span(member.object) == reference)?;
    let key = file.source_text(member.property)?;
    let refused = ast.computed_members.binary_search(&member.span).is_ok()
        || ast.optional_members.binary_search(&member.span).is_ok()
        || key.contains('\\')
        || key.starts_with('#')
        || ast
            .calls
            .iter()
            .any(|call| ast.peel_ts_sugar_span(call.callee) == member.span)
        || ast
            .tagged_template_tags
            .iter()
            .any(|tag| ast.peel_ts_sugar_span(*tag) == member.span)
        || !member_is_get(ast, member.span)
        || ast
            .assignments
            .iter()
            .any(|assignment| assignment.target.contains(member.span))
        || ast
            .deleted_targets
            .iter()
            .chain(&ast.iteration_targets)
            .any(|target| target.contains(member.span));
    (!refused).then_some((member.span, key))
}

/// Whether `text` contains a `this` or `super` keyword as a token, or an
/// escaped identifier that may spell one.
fn mentions_receiver_route(text: &str) -> bool {
    let bytes = text.as_bytes();
    let ident = |b: u8| b.is_ascii_alphanumeric() || b == b'_' || b == b'$';
    text.contains('\\')
        || ["this", "super"].iter().any(|word| {
            text.match_indices(word).any(|(at, _)| {
                (at == 0 || !ident(bytes[at - 1]))
                    && bytes.get(at + word.len()).is_none_or(|b| !ident(*b))
            })
        })
}

#[cfg(test)]
#[path = "property_gets/tests.rs"]
mod tests;
