//! File-local static rules the pipeline runs right after source discovery:
//! checks that need discovered sources and TypeScript facts, but none of the
//! reactive stages behind them.

use crate::cleanup::callback_argument_literal;
use crate::execution_role::{allowed_callback_spans, semantic_execution_role};
use crate::identity::SymbolId;
use crate::owners::{
    component_binding_name, component_props_parameter_fix, containing_ast_function,
    enclosing_function_label, function_binding_name,
};
use crate::pipeline::{AnalysisContext, ProgramDraft};
use crate::runtime_semantics::is_proven_array_filter;
use crate::symbols::async_symbol_root;
use crate::{
    ReactiveSourceKind, StaticDefect, StaticDefectKind, known_primitive, location, primitive_name,
};
use solid_facts::core::Span;
use std::collections::HashSet;
use typefacts::Location;

/// The static-prepass stage: every prepass rule, in their pipeline order.
pub(crate) fn static_prepass(ctx: &AnalysisContext<'_>, draft: &mut ProgramDraft) {
    component_props_destructure(ctx, draft);
    reactive_read_after_await(ctx, draft);
}

/// SC1003: a reactive object destructured outside tracking. Component
/// parameter patterns are necessarily setup-time reads; body bindings are
/// classified through the same semantic execution-role engine as direct reads.
///
/// Two precision gates:
/// - **Roles.** Destructuring inside an event handler, a deferred/leaf
///   callback (`onSettled`, action bodies), an effect's apply callback, an
///   `untrack` callback, or a directive application reads fresh values at
///   call time — legal at runtime — so only setup-time contexts (component
///   body, module scope, unclassified spans) are flagged.
/// - **Caller-proven props.** Under a dialect requiring caller proof, a
///   destructure that binds only proven-static props is not a reactive read
///   at all; an unprovable one is reported as a proof obligation.
fn component_props_destructure(ctx: &AnalysisContext<'_>, draft: &mut ProgramDraft) {
    for file in &ctx.facts.files {
        for function in &file.ast.functions {
            // Solid invokes components with one props object. Reject the
            // overwhelmingly common non-destructured functions before the
            // whole-project component-identity lookup.
            if function.parameters.len() <= 1
                && let Some(parameter) = function
                    .parameters
                    .first()
                    .filter(|parameter| parameter.shape == solid_facts::ast::BindingShape::Object)
                && ctx
                    .semantic_lookup
                    .function_may_be_component(file, function)
            {
                let location = location(file.path.shared(), parameter.pattern);
                let mut uncertain = ctx
                    .semantic_lookup
                    .function_component_status(file, function)
                    == crate::indexes::ComponentStatus::Uncertain;
                uncertain |= match destructured_props_use(ctx, parameter, &location) {
                    crate::source_discovery::PropUse::Static => continue,
                    crate::source_discovery::PropUse::Reactive => false,
                    crate::source_discovery::PropUse::Unknown => true,
                };
                draft.push_defect(StaticDefect {
                    kind: StaticDefectKind::ReactiveObjectDestructure {
                        source: "props".into(),
                        component_props: true,
                    },
                    location,
                    analysis_context: component_binding_name(file, function)
                        .map_or_else(String::new, |name| {
                            file.source_text(name.span).unwrap_or_default().to_owned()
                        }),
                    fixes: component_props_parameter_fix(
                        ctx.facts,
                        ctx.semantic_lookup.dialect,
                        file,
                        function,
                        parameter,
                        ctx.entities,
                    )
                    .into_iter()
                    .collect(),
                    uncertain,
                });
            }
        }
        if !file
            .ast
            .bindings
            .iter()
            .any(|binding| binding.shape == solid_facts::ast::BindingShape::Object)
        {
            continue;
        }
        let allowed = allowed_callback_spans(file, ctx.semantic_lookup);
        for binding in &file.ast.bindings {
            if binding.shape != solid_facts::ast::BindingShape::Object {
                continue;
            }
            let prop_symbol = binding
                .initializer_identifier
                .as_ref()
                .and_then(|identifier| {
                    ctx.entities
                        .get(&location(file.path.shared(), identifier.span))
                });
            let prop_declaration = prop_symbol
                .and_then(|symbol| ctx.prop_sources.get(symbol))
                .map(|(_, declaration)| declaration.clone());
            let props = prop_declaration.is_some();
            let reactive_object = props || binding_initializes_reactive_store(ctx, file, binding);
            if !reactive_object {
                continue;
            }
            let role = semantic_execution_role(
                file,
                binding.pattern,
                &allowed,
                ctx.entities,
                ctx.symbol_names,
                ctx.semantic_lookup,
            );
            // Fresh-at-call-time contexts: tracked scopes re-run and
            // re-subscribe; event handlers, deferred/leaf callbacks, effect
            // apply, untrack, and directive application read current values
            // when they execute. None of those misbehaves at runtime.
            if matches!(
                role,
                crate::ExecutionRole::TrackedJsx
                    | crate::ExecutionRole::EventCallback
                    | crate::ExecutionRole::DeferredCallback
                    | crate::ExecutionRole::UntrackedCallback
                    | crate::ExecutionRole::EffectApply
                    | crate::ExecutionRole::DirectiveApply
            ) {
                continue;
            }
            // The same guard the strict-read member loop applies: a
            // destructure inside a body-defined handler or helper executes
            // when that function is invoked and reads values fresh at that
            // moment — only a proven setup-time role above makes it a
            // once-frozen snapshot.
            if crate::owners::inside_non_component_function(
                file,
                binding.pattern,
                ctx.semantic_lookup,
            ) && crate::execution_role::named_callback_execution_role(
                file,
                binding.pattern,
                ctx.semantic_lookup,
            )
            .is_none()
            {
                continue;
            }
            let mut uncertain =
                prop_symbol.is_some_and(|symbol| ctx.uncertain_prop_sources.contains(symbol));
            if let Some(declaration) = &prop_declaration {
                match destructured_props_use(ctx, binding, declaration) {
                    crate::source_discovery::PropUse::Static => continue,
                    crate::source_discovery::PropUse::Reactive => {}
                    crate::source_discovery::PropUse::Unknown => uncertain = true,
                }
            }
            let location = location(file.path.shared(), binding.pattern);
            let source = binding
                .initializer
                .and_then(|span| file.source_text(span))
                .unwrap_or("reactive object")
                .to_owned();
            draft.push_defect(StaticDefect {
                kind: StaticDefectKind::ReactiveObjectDestructure {
                    source,
                    component_props: props,
                },
                location,
                analysis_context: enclosing_function_label(file, binding.pattern),
                fixes: vec![],
                uncertain,
            });
        }
    }
}

/// The caller classification of the props a destructuring pattern binds: the
/// named slots when they are all it binds, the whole object when a rest
/// element (or an unresolved slot) captures the remainder.
fn destructured_props_use(
    ctx: &AnalysisContext<'_>,
    binding: &solid_facts::ast::BindingFact,
    declaration: &Location,
) -> crate::source_discovery::PropUse {
    if binding.names.len() > binding.object_slots.len() {
        return ctx.props_reactivity.object_use(declaration);
    }
    ctx.props_reactivity.names_use(
        declaration,
        binding
            .object_slots
            .iter()
            .map(|slot| slot.property.as_str()),
    )
}

fn binding_initializes_reactive_store(
    ctx: &AnalysisContext<'_>,
    file: &solid_facts::FileFacts,
    binding: &solid_facts::ast::BindingFact,
) -> bool {
    if binding
        .initializer_identifier
        .as_ref()
        .and_then(|identifier| {
            ctx.entities
                .get(&location(file.path.shared(), identifier.span))
        })
        .is_some_and(|symbol| ctx.source_kinds.get(symbol) == Some(&ReactiveSourceKind::Store))
    {
        return true;
    }
    let Some(call) = binding
        .call_initializer
        .and_then(|initializer| file.ast.call_at(initializer))
    else {
        return false;
    };
    let contracted_store = ctx
        .entities
        .get(&location(file.path.shared(), call.callee))
        .and_then(|symbol| ctx.contracted.get(symbol))
        .and_then(|binding| binding.summary.returns.known())
        .and_then(Option::as_ref)
        .is_some_and(|returned| returned.kind == "store-path");
    contracted_store
        || known_primitive(&primitive_name(
            file.path.as_str(),
            call.callee,
            call.static_callee(&file.source),
            ctx.entities,
            ctx.symbol_names,
            ctx.dialect,
        ))
        .is_some_and(|primitive| ctx.dialect.returns_store(primitive))
}

/// SC1002: reactive reads after an await inside a tracked async computation,
/// where dependency tracking has already ended. Accessor calls come from the
/// TypeScript-side dominance analysis; store-path and props member reads are
/// proven against the straight-line awaits the AST facts record
/// (`unconditional_awaits`), with the same precision guards — no conditional
/// dominance, no nested closures except the separately proven synchronous
/// Array#filter callback path.
fn reactive_read_after_await(ctx: &AnalysisContext<'_>, draft: &mut ProgramDraft) {
    // The producer reports an async-function fact for every project
    // function, so both per-function steps below must stay cheap: the file
    // lookup goes through this index instead of a linear scan, and the
    // tracked-computation proof — a whole-project call scan — runs only for
    // a function that has something to report. A function with no
    // after-await calls and no unconditional await cannot produce a finding,
    // so resolving its computation context is pure cost.
    let files_by_path: std::collections::HashMap<&str, &solid_facts::FileFacts> = ctx
        .facts
        .files
        .iter()
        .map(|file| (file.path.as_str(), file))
        .collect();
    for typescript_file in ctx.facts.typescript.files() {
        for function in typescript_file.async_functions.iter() {
            let member_site = ctx
                .dialect
                .reports_member_reads_after_await()
                .then(|| member_read_site(&files_by_path, function))
                .flatten();
            if function.calls_after_await.is_empty() && member_site.is_none() {
                continue;
            }
            let Some(analysis_context) = tracked_async_computation_context(ctx, function) else {
                continue;
            };
            let mut reported_calls = HashSet::new();
            for call in &function.calls_after_await {
                let Some(symbol) = ctx.entities.get(call) else {
                    continue;
                };
                let Some((name, _)) = ctx.accessors.get(symbol) else {
                    continue;
                };
                let ast_call = ctx
                    .facts
                    .files
                    .iter()
                    .find(|file| *file.path.as_str() == *call.path)
                    .and_then(|file| {
                        file.ast
                            .calls
                            .iter()
                            .find(|candidate| {
                                u64::from(candidate.callee.start) == call.start_byte
                                    && u64::from(candidate.callee.end) == call.end_byte
                            })
                            .map(|candidate| (file, candidate))
                    });
                let display = ast_call
                    .and_then(|(file, candidate)| candidate.static_callee(&file.source))
                    .unwrap_or(name);
                let diagnostic_location = Location {
                    path: call.path.clone(),
                    start_byte: call.start_byte,
                    end_byte: call.end_byte,
                };
                reported_calls.insert((call.path.to_string(), call.start_byte, call.end_byte));
                draft.push_defect(StaticDefect {
                    kind: StaticDefectKind::ReactiveReadAfterAwait {
                        accessor: display.to_string(),
                    },
                    location: diagnostic_location,
                    analysis_context: analysis_context.clone(),
                    fixes: vec![],
                    uncertain: false,
                });
            }
            if let Some(site) = member_site {
                member_reads_after_await(ctx, draft, site, &analysis_context);
                let (file, ast_function, boundary) = site;
                for filter in file.ast.calls_within(ast_function.body) {
                    if filter.span.start < boundary
                        || containing_ast_function(&file.ast, filter.span)
                            .is_none_or(|owner| owner.span != ast_function.span)
                    {
                        continue;
                    }
                    let Some(callback) = inline_standard_callback(ctx, file, filter) else {
                        report_opaque_standard_callback(
                            ctx,
                            draft,
                            file,
                            filter,
                            &analysis_context,
                        );
                        continue;
                    };
                    // The callback body runs before the awaiting computation
                    // resumes, so its reads are the awaiting function's reads:
                    // the same accessor-call and member-read proofs apply, with
                    // the callback itself as the owning function.
                    member_reads_after_await(
                        ctx,
                        draft,
                        (file, callback, boundary),
                        &analysis_context,
                    );
                    for callback_call in file.ast.calls_within(callback.body) {
                        if callback_call.span.start < boundary
                            || containing_ast_function(&file.ast, callback_call.span)
                                .is_none_or(|owner| owner.span != callback.span)
                        {
                            continue;
                        }
                        let Some(symbol) = ctx
                            .entities
                            .get(&location(file.path.shared(), callback_call.callee))
                        else {
                            continue;
                        };
                        let Some((name, _)) = ctx.accessors.get(symbol) else {
                            continue;
                        };
                        let key = (
                            file.path.to_string(),
                            u64::from(callback_call.callee.start),
                            u64::from(callback_call.callee.end),
                        );
                        if !reported_calls.insert(key) {
                            continue;
                        }
                        let display = callback_call.static_callee(&file.source).unwrap_or(name);
                        draft.push_defect(StaticDefect {
                            kind: StaticDefectKind::ReactiveReadAfterAwait {
                                accessor: display.to_string(),
                            },
                            location: location(file.path.shared(), callback_call.callee),
                            analysis_context: analysis_context.clone(),
                            fixes: vec![],
                            uncertain: false,
                        });
                    }
                }
            }
        }
    }
}

/// The function a standard-library method provably invokes *inline*, or
/// `None` when nothing proves that.
///
/// Three separate facts have to hold, and none of them follows from the
/// spelling `.filter`:
///
/// - the callee resolves to the exact built-in `Array`/`ReadonlyArray`
///   `filter` declaration (a project-defined or shadowed `filter` resolves
///   elsewhere and a `Promise#then` callback is deferred, not inline);
/// - the argument itself is potentially callable — sampled at the *argument*
///   span, the position `argument_behavior` classifies, never at the callee,
///   whose callability answers a different question;
/// - the callback is the literal function written in argument position.
///   `rows.filter(makePredicate(post => …))` hands the arrow to a wrapper that
///   may stash it and run it under a later tracking scope, so it is not proof.
///
/// An `async` callback suspends at its own first await, so its body is not one
/// synchronous extent and stays outside this proof.
fn inline_standard_callback<'f>(
    ctx: &AnalysisContext<'_>,
    file: &'f solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
) -> Option<&'f solid_facts::ast::FunctionFact> {
    let argument = call.arguments.first()?;
    let resolved = ctx
        .semantic_lookup
        .resolved_callee_call(file, call.callee)?;
    let callability = ctx
        .semantic_lookup
        .smallest_contained_callability(file.path.as_str(), argument.span);
    if !is_proven_array_filter(resolved, callability) {
        return None;
    }
    let callback = callback_argument_literal(file, argument.span)?;
    (!callback.r#async).then_some(callback)
}

/// Preserve the proof obligation for an exact built-in synchronous callback
/// position whose body cannot be inspected. Invalid argument shapes do not
/// reach `is_proven_array_filter` and remain TypeScript-owned.
fn report_opaque_standard_callback(
    ctx: &AnalysisContext<'_>,
    draft: &mut ProgramDraft,
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    analysis_context: &str,
) {
    let Some(argument) = call.arguments.first() else {
        return;
    };
    let Some(resolved) = ctx.semantic_lookup.resolved_callee_call(file, call.callee) else {
        return;
    };
    let callability = ctx
        .semantic_lookup
        .smallest_contained_callability(file.path.as_str(), argument.span);
    if !is_proven_array_filter(resolved, callability) {
        return;
    }
    draft.push_defect(StaticDefect {
        kind: StaticDefectKind::ReactiveCallbackUnresolved {
            callee: call
                .static_callee(&file.source)
                .unwrap_or("Array.prototype.filter")
                .to_owned(),
        },
        location: location(file.path.shared(), argument.span),
        analysis_context: analysis_context.to_owned(),
        fixes: vec![],
        uncertain: false,
    });
}

/// The resolved location of an async function's member-read analysis: its
/// file, its AST function, and the end of the first await in the function's
/// own unconditional flow. `None` means the function has no straight-line
/// await, so no member read can be dominated by one and the rule has nothing
/// to prove there.
fn member_read_site<'f>(
    files_by_path: &std::collections::HashMap<&str, &'f solid_facts::FileFacts>,
    function: &typefacts::AsyncFunctionFact,
) -> Option<(
    &'f solid_facts::FileFacts,
    &'f solid_facts::ast::FunctionFact,
    u32,
)> {
    let file = files_by_path.get(&*function.expression.path)?;
    let start = u32::try_from(function.expression.start_byte).ok()?;
    let end = u32::try_from(function.expression.end_byte).ok()?;
    let expression = file.ast.peel_ts_sugar_span(Span::new(start, end));
    let ast_function = file
        .ast
        .functions
        .iter()
        .find(|candidate| candidate.span == expression)?;
    let boundary = file
        .ast
        .unconditional_awaits
        .iter()
        .filter(|awaited| {
            ast_function.body.contains(**awaited)
                && containing_ast_function(&file.ast, **awaited)
                    .is_some_and(|owner| owner.span == ast_function.span)
        })
        .map(|awaited| awaited.end)
        .min()?;
    Some((file, ast_function, boundary))
}

/// The tracked-computation proof for one async function: the call that
/// receives it as a tracked compute callback, named for the finding's
/// analysis context. `None` means no tracked computation receives the
/// function and its after-await reads are not this rule's business.
fn tracked_async_computation_context(
    ctx: &AnalysisContext<'_>,
    function: &typefacts::AsyncFunctionFact,
) -> Option<String> {
    let function_symbol = async_symbol_root(
        ctx.aliases
            .get(function.symbol.as_ref())
            .map_or(function.symbol.as_ref(), SymbolId::as_str),
        &ctx.facts.typescript,
    );
    ctx.facts.files.iter().find_map(|file| {
        file.ast.calls.iter().find_map(|candidate| {
            let argument = candidate.arguments.first()?;
            let lexical = *file.path.as_str() == *function.expression.path
                && argument.span.contains(Span::new(
                    u32::try_from(function.expression.start_byte).ok()?,
                    u32::try_from(function.expression.end_byte).ok()?,
                ));
            let semantic = ctx
                .entities
                .get(&location(file.path.shared(), argument.span))
                .is_some_and(|symbol| {
                    async_symbol_root(symbol, &ctx.facts.typescript) == function_symbol
                });
            if !lexical && !semantic {
                return None;
            }
            let primitive = primitive_name(
                file.path.as_str(),
                candidate.callee,
                candidate.static_callee(&file.source),
                ctx.entities,
                ctx.symbol_names,
                ctx.dialect,
            )?;
            // A tracked callback is what makes this a computation
            // whose reads matter after an await. The list this
            // replaced was 2.0's eight; under 1.x three of them
            // resolve to nothing and `createComputed` was absent.
            primitive
                .primitive()
                .is_some_and(|resolved| {
                    ctx.dialect
                        .callback_semantics_at(resolved, 0, candidate.arguments.len())
                        .tracks_reads
                })
                .then(|| format!("{primitive} async computation"))
        })
    })
}

/// Store-path and component-props member reads dominated by a straight-line
/// await of the same async computation. The dominating await must sit in the
/// function's own unconditional flow (no conditional dominance), and both the
/// await and the read must belong to the function directly (no nested
/// closures). Props follow the caller classification: a proven-static prop is
/// not reactive and stays silent, an unprovable one is a proof obligation.
fn member_reads_after_await(
    ctx: &AnalysisContext<'_>,
    draft: &mut ProgramDraft,
    site: (
        &solid_facts::FileFacts,
        &solid_facts::ast::FunctionFact,
        u32,
    ),
    analysis_context: &str,
) {
    let (file, ast_function, boundary) = site;
    for member in &file.ast.members {
        if member.span.start < boundary
            || !ast_function.body.contains(member.span)
            || containing_ast_function(&file.ast, member.span)
                .is_none_or(|owner| owner.span != ast_function.span)
        {
            continue;
        }
        // Only the complete member chain reads a store path; its prefixes are
        // the same read's steps.
        if file
            .ast
            .members
            .iter()
            .any(|candidate| candidate.object == member.span)
        {
            continue;
        }
        if file.ast.is_plain_assignment_target(member.span) {
            continue;
        }
        let Some(symbol) = ctx
            .entities
            .get(&location(file.path.shared(), member.object))
        else {
            continue;
        };
        let store = ctx.source_kinds.get(symbol) == Some(&ReactiveSourceKind::Store)
            && ctx.accessors.contains_key(symbol);
        let (name, uncertain) = if store {
            let Some((name, _)) = ctx.accessors.get(symbol) else {
                continue;
            };
            (name, false)
        } else if let Some((name, declaration)) = ctx.prop_sources.get(symbol) {
            let property = file.source_text(member.property).unwrap_or_default();
            match ctx.props_reactivity.prop_use(declaration, property) {
                crate::source_discovery::PropUse::Static => continue,
                crate::source_discovery::PropUse::Reactive => {
                    (name, ctx.uncertain_prop_sources.contains(symbol))
                }
                crate::source_discovery::PropUse::Unknown => (name, true),
            }
        } else {
            continue;
        };
        let accessor = file
            .source_text(member.span)
            .and_then(|path| {
                path.find('.')
                    .map(|index| format!("{name}{}", &path[index..]))
            })
            .unwrap_or_else(|| {
                format!(
                    "{name}.{}",
                    file.source_text(member.property).unwrap_or_default()
                )
            });
        draft.push_defect(StaticDefect {
            kind: StaticDefectKind::ReactiveReadAfterAwait { accessor },
            location: location(file.path.shared(), member.span),
            analysis_context: analysis_context.to_owned(),
            fixes: vec![],
            uncertain,
        });
    }
}

/// SC1004: a component whose return value hinges on a reactive condition,
/// evaluated once at setup and never again. Runs after the read tables are
/// merged: "reactive condition" is answered by the draft's reads.
///
/// A test is structural only when it selects which returned tree exists:
/// an `if`/`switch` a return sits under (its recorded control tests), or the
/// conditional/logical spine of the returned expression itself. A ternary
/// nested inside a JSX attribute of a returned branch is a tracked binding,
/// not a structural branch, and stays out.
pub(crate) fn component_returns_conditionally(ctx: &AnalysisContext<'_>, draft: &mut ProgramDraft) {
    for file in &ctx.facts.files {
        for function in &file.ast.functions {
            let Some(name) = function_binding_name(file, function).or(function.name.as_ref())
            else {
                continue;
            };
            let component_status = ctx
                .semantic_lookup
                .function_component_status(file, function);
            if component_status == crate::indexes::ComponentStatus::No {
                continue;
            }
            let mut direct_returns = file
                .ast
                .returns
                .iter()
                .filter(|returned| {
                    function.body.contains(returned.span)
                        && containing_ast_function(&file.ast, returned.span)
                            .is_some_and(|owner| owner.span == function.span)
                })
                .collect::<Vec<_>>();
            if let Some(returned) = &function.expression_return {
                direct_returns.push(returned);
            }
            let mut tests = Vec::new();
            for returned in &direct_returns {
                tests.extend(returned.control_tests.iter().copied());
                if let Some(argument) = returned.argument {
                    return_spine_tests(file, argument, &mut tests);
                }
            }
            tests.sort_unstable();
            tests.dedup();
            for test in tests.iter().filter(|test| {
                function.body.contains(**test)
                    && containing_ast_function(&file.ast, **test)
                        .is_some_and(|owner| owner.span == function.span)
            }) {
                let reactive = draft.reads.iter().any(|read| {
                    read.location.path == file.path.as_str().into()
                        && u64::from(test.start) <= read.location.start_byte
                        && read.location.end_byte <= u64::from(test.end)
                });
                if reactive {
                    let location = location(file.path.shared(), *test);
                    draft.push_defect(StaticDefect {
                        kind: StaticDefectKind::ComponentReturnsConditionally,
                        location,
                        analysis_context: file
                            .source_text(name.span)
                            .unwrap_or_default()
                            .to_owned(),
                        fixes: vec![],
                        uncertain: component_status == crate::indexes::ComponentStatus::Uncertain,
                    });
                }
            }
        }
    }
}

/// The structural tests of one returned expression: the top-level conditional
/// chain's tests, and the guards of logical expressions whose right operand
/// renders JSX structure (`return props.user && <Profile/>`, `return cond()
/// || <Fallback/>`). Branches recurse, so a chained ternary and a logical
/// inside a ternary branch each contribute their own test; anything nested
/// deeper — a ternary inside a JSX attribute, say — is evaluated where JSX
/// tracks and is not a structural branch.
fn return_spine_tests(file: &solid_facts::FileFacts, expression: Span, tests: &mut Vec<Span>) {
    let expression = file.ast.peel_ts_sugar_span(expression);
    if let Some(conditional) = file
        .ast
        .conditional_expressions
        .iter()
        .find(|conditional| conditional.span == expression)
    {
        tests.push(conditional.test);
        return_spine_tests(file, conditional.consequent, tests);
        return_spine_tests(file, conditional.alternate, tests);
    } else if let Some(logical) = file
        .ast
        .logical_expressions
        .iter()
        .find(|logical| logical.span == expression)
    {
        if jsx_structure_within(file, logical.right) {
            tests.push(logical.left);
        }
        return_spine_tests(file, logical.left, tests);
        return_spine_tests(file, logical.right, tests);
    }
}

/// Whether a span directly holds JSX structure (an element or fragment).
fn jsx_structure_within(file: &solid_facts::FileFacts, span: Span) -> bool {
    file.ast.jsx_within(span).next().is_some()
        || file
            .ast
            .jsx_fragments
            .iter()
            .any(|fragment| span.contains(*fragment))
}
