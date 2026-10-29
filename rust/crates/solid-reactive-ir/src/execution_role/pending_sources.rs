//! ADR 0510: the async sources ADR 0504/0505's first-run proofs may meet
//! pending besides a `const` of the component's own body: a module-scope memo
//! of a project module whose scope only creates nodes, and a memo the mounting
//! parent creates in its body and hands the component by an exact prop or
//! through one context provider.
//!
//! Each is pending at the read for the reason a body's own memo is: an async
//! compute's promise settles in a microtask at the earliest
//! (`@solidjs/signals@2.0.0-rc.17` `dist/dev-shared.js:3640-3690`: a result
//! that is still in flight throws `NotReadyError` from the creating pass,
//! which `:4229-4242` turns into `STATUS_PENDING`), and nothing between the
//! creation and the read lets one run: module evaluation, the entry's `render`
//! and the parent's tree are one synchronous job. Such a node holds no flush:
//! with no reader it joins no transaction (`joined` stays false, `:4520`), and
//! a seam that parks skips a loading source, pending and uninitialized with
//! nothing staged (`:2194-2204`); a commit leaves an in-flight node as it is
//! (`:2564-2590`).

use super::*;
use solid_dialect::Primitive;
use solid_facts::ast::{BindingFact, BindingShape, CallFact, FunctionFact};

/// The binding naming `declared` in `file`, when it is a `const` of one
/// identifier initialized by the dialect's `createMemo` with one argument, an
/// async function literal (not a generator) and no options, so no
/// `loadingValue` masks the first flight (`dist/dev-shared.js:4630-4633`).
pub(super) fn async_memo_at<'f>(
    file: &'f solid_facts::FileFacts,
    declared: Span,
    lookup: &SemanticLookup<'_>,
) -> Option<(&'f BindingFact, &'f CallFact)> {
    let ast = &file.ast;
    let binding = ast
        .bindings
        .iter()
        .find(|binding| binding.names.iter().any(|name| name.span == declared))?;
    let memo = binding
        .call_initializer
        .and_then(|span| ast.call_at(span))?;
    let async_compute = match memo.arguments.as_slice() {
        [compute] if !compute.spread => ast
            .function_at(ast.peel_ts_sugar_span(compute.span))
            .is_some_and(|function| function.r#async && !function.generator),
        _ => false,
    };
    (binding.immutable
        && binding.shape == BindingShape::Identifier
        && async_compute
        && lookup.primitive_at_call(file, memo.span) == Some(Primitive::CreateMemo))
    .then_some((binding, memo))
}

/// The memo's compute is one primitive literal (`async () => 1`): its body
/// runs no code at creation, and its result is a plain value one microtask
/// away.
pub(super) fn memo_compute_is_literal(file: &solid_facts::FileFacts, memo: &CallFact) -> bool {
    let ast = &file.ast;
    memo.arguments
        .first()
        .and_then(|argument| ast.function_at(ast.peel_ts_sugar_span(argument.span)))
        .is_some_and(|compute| {
            compute.parameters.is_empty()
                && !compute.rest_parameter
                && file
                    .source_text(compute.body)
                    .is_some_and(primitive_literal_body)
        })
}

/// A call that only creates a node, written as the whole initializer of a
/// `const`: the dialect's `createMemo` of one function literal (no parameter,
/// not a generator) whose body is one primitive literal; `createSignal` of at
/// most one primitive literal, bound to a name or taken apart by an array
/// pattern with no default (the returned tuple is an array,
/// `dist/dev.js:2412-2422`, and the iterator intact, ADR 0349); or
/// `createContext` of at most one primitive literal, which builds a function
/// and runs nothing (`solid-js@2.0.0-rc.17` `dist/solid.dev.js:9-21`).
pub(crate) fn inert_creation(
    file: &solid_facts::FileFacts,
    call: &CallFact,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let ast = &file.ast;
    let Some(binding) = ast.bindings.iter().find(|binding| {
        binding.immutable
            && binding
                .initializer
                .is_some_and(|initializer| ast.peel_ts_sugar_span(initializer) == call.span)
    }) else {
        return false;
    };
    let literal_arguments = |most: usize| {
        call.arguments.len() <= most
            && call.arguments.iter().all(|argument| {
                !argument.spread
                    && file
                        .source_text(ast.peel_ts_sugar_span(argument.span))
                        .is_some_and(primitive_literal_body)
            })
    };
    let named = binding.shape == BindingShape::Identifier;
    !call.construct
        && match lookup.primitive_at_call(file, call.span) {
            Some(Primitive::CreateMemo) => {
                named
                    && matches!(call.arguments.as_slice(), [compute] if !compute.spread
                        && ast.function_at(ast.peel_ts_sugar_span(compute.span))
                            .is_some_and(|function| !function.generator))
                    && memo_compute_is_literal(file, call)
            }
            Some(Primitive::CreateSignal) => {
                literal_arguments(1)
                    && (named
                        || (binding.shape == BindingShape::Array
                            && lookup.array_iteration_intact()
                            && file
                                .source_text(binding.pattern)
                                .is_some_and(|pattern| !pattern.contains('='))))
            }
            Some(Primitive::CreateContext) => named && literal_arguments(1),
            _ => false,
        }
}

/// ADR 0510: evaluating the module runs nothing but [`inert_creation`]s: at
/// module scope every call is one, every function a declaration, a `const`'s
/// whole initializer or such a creation's compute, every name referenced the
/// callee of one, and there is no member access, tagged template, JSX, class,
/// `await`, `throw` or module hazard (`indexes::module_scope`'s census, with
/// the creations admitted).
pub(crate) fn module_scope_only_creates(
    file: &solid_facts::FileFacts,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let ast = &file.ast;
    let scope = |span: Span| crate::indexes::at_module_scope(ast, span);
    let creations = ast
        .calls
        .iter()
        .filter(|call| scope(call.span))
        .collect::<Vec<_>>();
    if !creations
        .iter()
        .all(|call| inert_creation(file, call, lookup))
    {
        return false;
    }
    let computes = creations
        .iter()
        .filter_map(|call| call.arguments.first())
        .map(|argument| ast.peel_ts_sugar_span(argument.span))
        .collect::<Vec<_>>();
    let callees = creations
        .iter()
        .map(|call| ast.peel_ts_sugar_span(call.callee))
        .collect::<Vec<_>>();
    ast.module_hazards.is_empty()
        && !ast.members.iter().any(|member| scope(member.span))
        && !ast.computed_members.iter().any(|member| scope(*member))
        && !ast.tagged_template_tags.iter().any(|tag| scope(*tag))
        && !ast.jsx_elements.iter().any(|element| scope(element.span))
        && !ast.jsx_fragments.iter().any(|fragment| scope(*fragment))
        && !ast.classes.iter().any(|class| scope(class.span))
        && !ast.awaits.iter().any(|awaited| scope(*awaited))
        && !ast
            .throw_statements
            .iter()
            .any(|statement| scope(*statement))
        && ast.identifiers.iter().all(|identifier| {
            !crate::indexes::runtime_reference(ast, identifier)
                || !scope(identifier.span)
                || callees.contains(&identifier.span)
        })
        && ast.functions.iter().all(|function| {
            !scope(function.span)
                || crate::indexes::declared_function(ast, function)
                || computes.contains(&function.span)
        })
}

/// A mount site whose root `render` lets a module-scope source stay pending
/// until the read: [`render_runs_first`] (no module or entry code before it
/// that could run a microtask checkpoint, read or settle anything, and every
/// module loaded seen), the source's module not the entry, and the entry a
/// root of evaluation: no project file imports or re-exports it (the
/// census's imports), so every module it loads, with its own cycles, has
/// finished evaluating before its body runs and a module-scope `const` there
/// is initialized before the `render` (ECMAScript `InnerModuleEvaluation`; a
/// module that imports the entry could leave a dependency of the component
/// unevaluated, its `const` in its temporal dead zone). And no other root
/// (a project file none imports) reaches the source's module (ADR 0510
/// review): a page that loads such a module script before the entry would
/// evaluate the source first, and the microtask checkpoint between the two
/// scripts settles it. Premise: the page loads no script beyond the
/// project's files that imports the source.
fn module_source_mount(
    lookup: &SemanticLookup<'_>,
    site: &crate::owners::MountSite<'_>,
    source_file: &solid_facts::FileFacts,
) -> bool {
    let (_, _, edges, _) = lookup.module_scope_census();
    let entry = site.entry.path.as_str();
    let imported = |path: &str| edges.iter().any(|(_, target)| target == path);
    let reaches_source = |root: &str| {
        let mut seen = vec![root];
        let mut next = 0;
        while let Some(&at) = seen.get(next) {
            next += 1;
            if at == source_file.path.as_str() {
                return true;
            }
            for (_, target) in edges.iter().filter(|(from, _)| from == at) {
                if !seen.contains(&target.as_str()) {
                    seen.push(target.as_str());
                }
            }
        }
        false
    };
    let other_root_reaches = edges
        .iter()
        .map(|(from, _)| from.as_str())
        .any(|root| root != entry && !imported(root) && reaches_source(root));
    site.entry.path != source_file.path
        && render_runs_first(lookup, site.entry, site.render)
        && !imported(entry)
        && !other_root_reaches
}

/// ADR 0510: the source of the effect `effect`'s apply read is a module-scope
/// `const` async memo of a project module (`binding`, `memo` in
/// `source_file`), pending until the read:
/// - its compute is one primitive literal, and its module's scope only
///   creates ([`module_scope_only_creates`], by the census), so evaluating it
///   neither throws nor reads anything;
/// - the component takes plain parameters and runs nothing in its body before
///   the effect is created that could read the source or throw
///   ([`body_runs_nothing_between`] from the body's start);
/// - a mount of the component satisfies [`module_source_mount`];
/// - a `createEffect` also meets ADR 0505's flush premises
///   ([`user_apply_flush_premises`]).
pub(super) fn module_source_pending(
    file: &solid_facts::FileFacts,
    component: &FunctionFact,
    effect: &CallFact,
    source_file: &solid_facts::FileFacts,
    binding: &BindingFact,
    memo: &CallFact,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let path = source_file.path.as_str();
    let render_effect =
        lookup.primitive_at_call(file, effect.span) == Some(Primitive::CreateRenderEffect);
    let source = source_name_in(file, source_file, binding, lookup);
    crate::indexes::at_module_scope(&source_file.ast, binding.declaration)
        && !path.contains("/node_modules/")
        && ![".d.ts", ".d.mts", ".d.cts"]
            .iter()
            .any(|suffix| path.ends_with(suffix))
        && memo_compute_is_literal(source_file, memo)
        && {
            let (loud, creating, _, _) = lookup.module_scope_census();
            !loud.iter().any(|loud| loud == path) || creating.iter().any(|only| only == path)
        }
        && plain_component(component)
        && body_runs_nothing_between(
            file,
            component,
            component.body.start,
            effect.span.start,
            BodyAdmits {
                context: None,
                source,
                hook: None,
                handed: None,
            },
            lookup,
        )
        && (render_effect
            || user_apply_flush_premises(
                file,
                component,
                effect,
                None,
                BodyAdmits {
                    context: None,
                    source,
                    hook: None,
                    handed: None,
                },
                lookup,
            ))
        && crate::owners::module_render_sites(lookup, file, component)
            .iter()
            .any(|site| module_source_mount(lookup, site, source_file))
}

/// Not async or a generator, with plain parameters (no pattern, default or
/// rest): nothing runs before the body.
pub(super) fn plain_component(component: &FunctionFact) -> bool {
    !component.r#async
        && !component.generator
        && !component.rest_parameter
        && component.parameters.iter().all(|parameter| {
            parameter.shape == BindingShape::Identifier && parameter.initializer.is_none()
        })
}

/// How the parent hands its memo to the component: the prop name a tag
/// attribute passes it under, or the key of a provider's value object and the
/// context's declaration.
enum Handed {
    Prop,
    Context { context: Span },
}

/// ADR 0510: the source of the read at `site` (`props.N()` or `ctx.N()`) is
/// a `const` async memo (`binding`, `memo`) the component's mounting parent
/// creates in its own body and hands over:
/// - `props.N()`: `props` the component's one plain parameter, never
///   written, and the parent's tag `<C N={x} />` passing the memo's name `x`;
///   the compiler hands an identifier as the prop's value, and reading
///   `props.N` returns it;
/// - `ctx.N()`: `ctx` a `const` of the component's body initialized by the
///   dialect's `useContext(K)`, and the tag the one child of the provider
///   `<K value={{ N: x }}>` (ADR 0510's holder, `owners::provided_child`),
///   whose value `getContext` returns to the body (`@solidjs/signals@2.0.0-rc.17`
///   `dist/dev.js:1811-1824`: the provider's `setContext` runs on the root it
///   creates before `children` builds the memos that render the tag,
///   `solid-js@2.0.0-rc.17` `dist/solid.dev.js:11-15`);
///
/// and the parent's body created the memo on every completion before its
/// tree, with a compute of one primitive literal. The parent is the last
/// parent of a mount site (ADR 0507's climb with ADR 0510's admissions); the
/// component runs nothing in its body before the effect that could read the
/// source or throw (a `useContext` of the provided context finds it); a
/// `createEffect` also meets ADR 0505's premises on that mount.
#[allow(clippy::too_many_arguments)]
pub(super) fn parent_source_pending(
    file: &solid_facts::FileFacts,
    site: Span,
    component: &FunctionFact,
    effect: &CallFact,
    parent_file: &solid_facts::FileFacts,
    binding: &BindingFact,
    memo: &CallFact,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let ast = &file.ast;
    let Some(parent) =
        crate::owners::containing_ast_function(&parent_file.ast, binding.declaration)
    else {
        return false;
    };
    if (parent_file.path == file.path && parent.span == component.span)
        || !memo_compute_is_literal(parent_file, memo)
        || !crate::interproc::site_on_every_completion(lookup, parent_file, parent.span, memo.span)
        || !plain_component(component)
    {
        return false;
    }
    let Some(read) = ast.call_at(site) else {
        return false;
    };
    // ADR 0511: the callee, or a closed alias of the member declared in the
    // body before the effect (a later one is in its temporal dead zone at
    // the first run).
    let callee = ast.peel_ts_sugar_span(read.callee);
    let Some(read_member) = handed_member_alias(file, callee) else {
        return false;
    };
    if read_member != callee
        && (crate::owners::containing_ast_function(ast, read_member)
            .is_none_or(|function| function.span != component.span)
            || read_member.end > effect.span.start)
    {
        return false;
    }
    let Some(member) = ast.members.iter().find(|member| member.span == read_member) else {
        return false;
    };
    let Some(name) = file.source_text(member.property) else {
        return false;
    };
    let object = ast.peel_ts_sugar_span(member.object);
    let Some(object_declaration) = ast.reference_declaration(object) else {
        return false;
    };
    let written = |declaration: Span| {
        ast.assignments.iter().any(|assignment| {
            ast.reference_declaration(ast.peel_ts_sugar_span(assignment.target))
                == Some(declaration)
        })
    };
    let handed = if let [parameter] = component.parameters.as_slice()
        && parameter
            .names
            .iter()
            .any(|named| named.span == object_declaration)
    {
        Handed::Prop
    } else {
        let Some(holder) = ast.bindings.iter().find(|candidate| {
            candidate.immutable
                && candidate.shape == BindingShape::Identifier
                && candidate
                    .names
                    .iter()
                    .any(|named| named.span == object_declaration)
        }) else {
            return false;
        };
        let Some(lookup_call) = holder.call_initializer.and_then(|span| ast.call_at(span)) else {
            return false;
        };
        let context = match lookup_call.arguments.as_slice() {
            [argument] if !argument.spread => {
                ast.reference_declaration(ast.peel_ts_sugar_span(argument.span))
            }
            _ => None,
        };
        let Some(context) = context else {
            return false;
        };
        // Declared before the effect, on every completion: the first run
        // reads it on the call.
        if crate::owners::containing_ast_function(ast, holder.declaration)
            .is_none_or(|function| function.span != component.span)
            || holder.declaration.end > effect.span.start
            || !crate::interproc::site_on_every_completion(
                lookup,
                file,
                component.span,
                lookup_call.span,
            )
            || lookup.primitive_at_call(file, lookup_call.span) != Some(Primitive::UseContext)
            || parent_file.path != file.path
        {
            return false;
        }
        Handed::Context { context }
    };
    if written(object_declaration) || !only_called_members(file, object_declaration, read_member) {
        return false;
    }
    let memo_name = binding.names.first().map(|named| named.span);
    let provided = match handed {
        Handed::Context { context } => Some(context),
        Handed::Prop => None,
    };
    let render_effect =
        lookup.primitive_at_call(file, effect.span) == Some(Primitive::CreateRenderEffect);
    body_runs_nothing_between(
        file,
        component,
        component.body.start,
        effect.span.start,
        BodyAdmits {
            context: provided,
            source: None,
            hook: None,
            handed: Some(object_declaration),
        },
        lookup,
    ) && (render_effect
        || user_apply_flush_premises(
            file,
            component,
            effect,
            None,
            BodyAdmits {
                context: provided,
                source: None,
                hook: None,
                handed: Some(object_declaration),
            },
            lookup,
        ))
        && crate::owners::module_render_sites(lookup, file, component)
            .iter()
            .any(|site| {
                let Some((caller, last, tag)) = site.parents.last() else {
                    return false;
                };
                let passes = match handed {
                    Handed::Prop => crate::owners::tag_passes_prop(caller, *tag, name, memo_name),
                    Handed::Context { context } => {
                        crate::owners::tag_provided(caller, *tag, context, name, memo_name)
                    }
                };
                caller.path == parent_file.path
                    && last.span == parent.span
                    && passes
                    && (render_effect || render_runs_first(lookup, site.entry, site.render))
            })
}

/// The declaration `file` names a module-scope source by: its own name in its
/// own module, or the local name of a runtime import of it, where the source
/// module's one value export under the imported name (a duplicate is a
/// TypeScript error) binds exactly the memo's declaration: `export const m`,
/// or `export { m as x }` whose local resolves to it (ADR 0510 review: never
/// an import matched by the declaration's text, `export { m as x, f as m }`).
fn source_name_in(
    file: &solid_facts::FileFacts,
    source_file: &solid_facts::FileFacts,
    binding: &BindingFact,
    lookup: &SemanticLookup<'_>,
) -> Option<Span> {
    let declared = binding.names.first()?.span;
    if source_file.path == file.path {
        return Some(declared);
    }
    let source_ast = &source_file.ast;
    let exported_as = |name: &str| {
        let mut entries = source_ast
            .exports
            .iter()
            .filter(|export| export.module.is_none() && !export.type_only)
            .flat_map(|export| export.specifiers.iter().chain(&export.declarations))
            .filter(|entry| !entry.type_only && entry.exported.as_str() == name);
        let entry = entries.next()?;
        if entries.next().is_some() {
            return None;
        }
        (entry.local.span == declared
            || source_ast.reference_declaration(entry.local.span) == Some(declared))
        .then_some(())
    };
    let mut locals = file
        .ast
        .imports
        .iter()
        .filter(|import| {
            !import.type_only
                && lookup.relative_module_path(file.path.as_str(), &import.module)
                    == Some(source_file.path.as_str())
        })
        .flat_map(|import| &import.bindings)
        .filter(|imported| {
            !imported.type_only
                && imported
                    .imported
                    .as_deref()
                    .is_some_and(|name| exported_as(name).is_some())
        })
        .map(|imported| imported.local.span);
    let local = locals.next()?;
    locals.next().is_none().then_some(local)
}

/// Every reference to the handed object (`props`, `ctx`) is the object of a
/// member that is called (`props.data()`) or is the read's own closed alias
/// (`alias`, ADR 0511): nothing aliases it, hands it on,
/// takes it apart or writes a member of it, so the member the read calls is
/// the value the mount handed over.
fn only_called_members(file: &solid_facts::FileFacts, declaration: Span, alias: Span) -> bool {
    let ast = &file.ast;
    ast.identifiers
        .iter()
        .filter(|identifier| {
            identifier.role == solid_facts::ast::IdentifierRole::Reference
                && ast.reference_declaration(identifier.span) == Some(declaration)
        })
        .all(|identifier| {
            ast.members.iter().any(|member| {
                ast.peel_ts_sugar_span(member.object) == identifier.span
                    && (member.span == alias
                        || ast.calls.iter().any(|call| {
                            !call.construct && ast.peel_ts_sugar_span(call.callee) == member.span
                        }))
                    && !ast
                        .assignments
                        .iter()
                        .any(|assignment| assignment.target.contains(member.span))
            })
        })
}

/// ADR 0511: `hook` (in `hook_file`) is an inert project hook: a function of
/// an analyzed project file (not a dependency or a declaration file), not
/// async or a generator, with a block body and no parameter, whose own code
/// runs nothing but [`inert_creation`]s (each call one, every nested
/// function such a creation's compute), throws nothing before its end
/// ([`throws_nothing_before`]), references only its own names and the
/// creations' callees, and has one `return`, in its straight-line flow, of
/// an object literal whose properties are plain data properties naming its
/// own bindings (`return { a1 };`). Its call creates those nodes under the
/// caller's owner and returns a fresh object: nothing there reads, throws or
/// lets a microtask run.
pub(crate) fn inert_hook(
    hook_file: &solid_facts::FileFacts,
    hook: &FunctionFact,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let ast = &hook_file.ast;
    let path = hook_file.path.as_str();
    let own = |span: Span| {
        hook.body.contains(span)
            && crate::owners::containing_ast_function(ast, span)
                .is_some_and(|function| function.span == hook.span)
    };
    let creations = ast
        .calls
        .iter()
        .filter(|call| own(call.span))
        .collect::<Vec<_>>();
    if path.contains("/node_modules/")
        || [".d.ts", ".d.mts", ".d.cts"]
            .iter()
            .any(|suffix| path.ends_with(suffix))
        || hook.r#async
        || hook.generator
        || hook.expression_body
        || !hook.parameters.is_empty()
        || hook.rest_parameter
        || !creations
            .iter()
            .all(|call| inert_creation(hook_file, call, lookup))
    {
        return false;
    }
    let computes = creations
        .iter()
        .filter_map(|call| call.arguments.first())
        .map(|argument| ast.peel_ts_sugar_span(argument.span))
        .collect::<Vec<_>>();
    let callees = creations
        .iter()
        .map(|call| ast.peel_ts_sugar_span(call.callee))
        .collect::<Vec<_>>();
    let returns = ast
        .returns
        .iter()
        .filter(|returned| own(returned.span))
        .collect::<Vec<_>>();
    let [returned] = returns.as_slice() else {
        return false;
    };
    let Some(object) = returned.argument.map(|span| ast.peel_ts_sugar_span(span)) else {
        return false;
    };
    let object_literal = returned.straight_line
        && hook_file
            .source_text(object)
            .is_some_and(|text| text.starts_with('{'))
        && ast
            .object_properties
            .iter()
            .filter(|property| object.contains(property.span))
            .all(|property| {
                property.data
                    && !property.computed
                    && ast
                        .reference_declaration(ast.peel_ts_sugar_span(property.value))
                        .is_some_and(|declaration| hook.body.contains(declaration))
            });
    object_literal
        && ast.functions.iter().all(|function| {
            function.span == hook.span
                || !hook.span.contains(function.span)
                || computes.contains(&function.span)
        })
        && !ast.jsx_elements.iter().any(|element| own(element.span))
        && !ast.jsx_fragments.iter().any(|fragment| own(*fragment))
        && !ast.awaits.iter().any(|awaited| own(*awaited))
        && throws_nothing_before(
            hook_file,
            hook,
            hook.body.start,
            hook.body.end,
            BodyAdmits::default(),
            lookup,
        )
        && ast.identifiers.iter().all(|identifier| {
            !crate::indexes::runtime_reference(ast, identifier)
                || !own(identifier.span)
                || callees.contains(&identifier.span)
                || ast
                    .reference_declaration(identifier.span)
                    .is_some_and(|declaration| hook.body.contains(declaration))
        })
}

/// ADR 0511: the call in `component`'s own body, before the effect at
/// `effect`, of the inert hook `hook` (in `hook_file`): a call with no
/// argument whose callee resolves exactly to the hook (an identifier bound
/// to it that nothing writes), on every completion of the body, as a
/// `const`'s whole initializer bound to a name or taken apart by an object
/// pattern.
fn hook_call_in_body<'f>(
    file: &'f solid_facts::FileFacts,
    component: &FunctionFact,
    effect: &CallFact,
    hook_file: &solid_facts::FileFacts,
    hook: &FunctionFact,
    lookup: &SemanticLookup<'_>,
) -> Option<&'f CallFact> {
    let ast = &file.ast;
    let mut calls = ast.calls.iter().filter(|call| {
        call.span.end <= effect.span.start
            && crate::owners::containing_ast_function(ast, call.span)
                .is_some_and(|function| function.span == component.span)
            && project_callee_at_parameter(file, call, 0, lookup).is_some_and(
                |(callee_file, callee, _)| {
                    callee_file.path == hook_file.path && callee.span == hook.span
                },
            )
    });
    let call = calls.next()?;
    let callee = ast.peel_ts_sugar_span(call.callee);
    let named = hook.name.as_ref().map(|name| name.span);
    (calls.next().is_none()
        && !call.construct
        && call.arguments.is_empty()
        && !ast.members.iter().any(|member| member.span == callee)
        && ast
            .identifiers
            .iter()
            .any(|identifier| identifier.span == callee)
        && named.is_none_or(|name| !crate::indexes::binding_written(hook_file, name))
        && ast
            .reference_declaration(callee)
            .is_some_and(|declaration| !crate::indexes::binding_written(file, declaration))
        && crate::interproc::site_on_every_completion(lookup, file, component.span, call.span)
        && ast.bindings.iter().any(|binding| {
            binding.immutable
                && matches!(
                    binding.shape,
                    BindingShape::Identifier | BindingShape::Object
                )
                && binding
                    .initializer
                    .is_some_and(|initializer| ast.peel_ts_sugar_span(initializer) == call.span)
        }))
    .then_some(call)
}

/// ADR 0511: the source of the effect `effect`'s apply read is a `const`
/// async memo (`binding`, `memo`) an inert project hook creates
/// ([`inert_hook`]; `const a1 = createMemo(async () => 1); return { a1 };`),
/// on every completion, with a compute of one primitive literal, and the
/// component calls that hook once in its body before the effect
/// ([`hook_call_in_body`]): the memo is created there, under the component's
/// owner, pending until a microtask (`@solidjs/signals@2.0.0-rc.17`
/// `dist/dev-shared.js:3640-3690`), and the exact transport carried the read
/// to its declaration. The component takes plain parameters and runs nothing
/// else in its body before the effect; a `createEffect` also meets ADR
/// 0505's premises on a mount whose `render` runs first.
pub(super) fn hook_source_pending(
    file: &solid_facts::FileFacts,
    component: &FunctionFact,
    effect: &CallFact,
    hook_file: &solid_facts::FileFacts,
    binding: &BindingFact,
    memo: &CallFact,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let Some(hook) = crate::owners::containing_ast_function(&hook_file.ast, binding.declaration)
    else {
        return false;
    };
    if (hook_file.path == file.path && hook.span == component.span)
        || !memo_compute_is_literal(hook_file, memo)
        || !crate::interproc::site_on_every_completion(lookup, hook_file, hook.span, memo.span)
        || !plain_component(component)
        || !inert_hook(hook_file, hook, lookup)
    {
        return false;
    }
    let Some(call) = hook_call_in_body(file, component, effect, hook_file, hook, lookup) else {
        return false;
    };
    let admits = BodyAdmits {
        context: None,
        source: None,
        hook: Some(call.span),
        handed: None,
    };
    let render_effect =
        lookup.primitive_at_call(file, effect.span) == Some(Primitive::CreateRenderEffect);
    body_runs_nothing_between(
        file,
        component,
        component.body.start,
        effect.span.start,
        admits,
        lookup,
    ) && (render_effect
        || (user_apply_flush_premises(file, component, effect, None, admits, lookup)
            && crate::owners::module_render_sites(lookup, file, component)
                .iter()
                .any(|site| render_runs_first(lookup, site.entry, site.render))))
}

/// ADR 0511: the member a call's callee names through closed local forms,
/// a named non-computed, non-optional member `X.N` with `X` an identifier:
/// the callee itself (`props.k()`), or an identifier bound by an immutable,
/// never-written `const` of one name whose whole initializer is that member
/// (`const d = props.k`), or a slot of an object pattern with no default or
/// rest whose initializer is an object literal written there, of plain data
/// properties with no spread or computed key, whose property for the slot
/// holds the member (`const { d } = { d: ctx.k }`). Answers the member's
/// span; nothing else (a tuple, a hook's result, a reassigned name) is
/// followed.
pub(crate) fn handed_member_alias(file: &solid_facts::FileFacts, callee: Span) -> Option<Span> {
    let ast = &file.ast;
    let plain_member = |span: Span| {
        let span = ast.peel_ts_sugar_span(span);
        let member = ast.member_at(span)?;
        (!ast.computed_members.contains(&span)
            && ast.optional_members.binary_search(&span).is_err()
            && ast.identifiers.iter().any(|identifier| {
                identifier.span == ast.peel_ts_sugar_span(member.object)
                    && identifier.role == solid_facts::ast::IdentifierRole::Reference
            }))
        .then_some(span)
    };
    let callee = ast.peel_ts_sugar_span(callee);
    if ast.member_at(callee).is_some() {
        return plain_member(callee);
    }
    if !ast
        .identifiers
        .iter()
        .any(|identifier| identifier.span == callee)
    {
        return None;
    }
    let declaration = ast.reference_declaration(callee)?;
    let binding = ast
        .bindings
        .iter()
        .find(|binding| binding.names.iter().any(|name| name.span == declaration))?;
    if !binding.immutable
        || binding.names.len() != 1
        || crate::indexes::binding_written(file, declaration)
    {
        return None;
    }
    let initializer = ast.peel_ts_sugar_span(binding.initializer?);
    match binding.shape {
        BindingShape::Identifier => plain_member(initializer),
        BindingShape::Object => {
            let pattern = file.source_text(binding.pattern)?;
            if pattern.contains('=') || pattern.contains("...") {
                return None;
            }
            let slot = binding
                .object_slots
                .iter()
                .find(|slot| slot.local.span == declaration && slot.plain_identifier_key)?;
            let shape = ast
                .object_get_shapes
                .iter()
                .find(|shape| shape.span == initializer)?;
            if !shape.closed {
                return None;
            }
            let own = ast
                .object_properties
                .iter()
                .filter(|property| {
                    initializer.contains(property.span)
                        && !ast.object_properties.iter().any(|outer| {
                            outer.span != property.span
                                && initializer.contains(outer.span)
                                && outer.span.contains(property.span)
                        })
                })
                .collect::<Vec<_>>();
            if !own
                .iter()
                .all(|property| property.data && !property.computed)
            {
                return None;
            }
            let mut matching = own.iter().filter(|property| {
                file.source_text(property.key)
                    .map(|key| key.trim_matches(['\'', '"']))
                    == Some(slot.property.as_str())
            });
            let property = matching.next()?;
            if matching.next().is_some() {
                return None;
            }
            plain_member(property.value)
        }
        BindingShape::Array => None,
    }
}
