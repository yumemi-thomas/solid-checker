//! File-local static rules the pipeline runs right after source discovery:
//! checks that need discovered sources and TypeScript facts, but none of the
//! reactive stages behind them.

use crate::HashSet;
use crate::cleanup::callback_argument_literal;
use crate::execution_role::{allowed_callback_spans, semantic_execution_role};
use crate::identity::SymbolId;
use crate::owners::{
    component_binding_name, component_props_parameter_fix, containing_ast_function,
    enclosing_function_label, function_binding_name,
};
use crate::pipeline::{AnalysisContext, ProgramDraft};
use crate::symbols::AsyncSymbolRoots;
use crate::{
    ReactiveRead, ReactiveSourceKind, StaticDefect, StaticDefectKind, known_primitive, location,
};
use solid_facts::core::Span;
use typefacts::Location;

mod after_await;
mod native_promise;
mod pre_await;

/// The static-prepass stage: every prepass rule, in their pipeline order.
pub(crate) fn static_prepass(
    ctx: &AnalysisContext<'_>,
    mut chunks: Option<&mut crate::late_chunks::LateChunks>,
    draft: &mut ProgramDraft,
) {
    let inputs = crate::late_chunks::ChunkInputs::of(ctx);
    let per_file = crate::late_chunks::FileChunks::run(
        chunks
            .as_deref_mut()
            .map(|chunks| &mut chunks.props_destructure),
        &inputs,
        |file| component_props_destructure(ctx, &inputs.discovery, file),
    );
    push_defects(draft, &per_file);
    reactive_read_after_await(ctx, draft);
    let per_file = crate::late_chunks::FileChunks::run(
        chunks
            .as_deref_mut()
            .map(|chunks| &mut chunks.value_references),
        &inputs,
        |file| primitive_value_references(ctx, file),
    );
    push_defects(draft, &per_file);
    let per_file = crate::late_chunks::FileChunks::run(
        chunks.map(|chunks| &mut chunks.result_methods),
        &inputs,
        |file| primitive_result_methods(ctx, file),
    );
    push_defects(draft, &per_file);
}

/// A chunked pass's defects, in file order.
fn push_defects(draft: &mut ProgramDraft, per_file: &[std::sync::Arc<Vec<StaticDefect>>]) {
    for defects in per_file {
        for defect in defects.iter() {
            draft.push_defect(defect.clone());
        }
    }
}

/// ADR 0338: a member call on the result of a primitive whose result holds an
/// audited getter (`createSignal(1).slice(0)`, `createMemo(fn).bind(null)`,
/// `.reverse().reverse()`) is that value's own method, so the call is not the
/// primitive and the value it returns has no provenance the analysis can
/// state: `.slice(0)` copies the tuple and keeps the getter, `.slice(1)` holds
/// the setter, `.toString()` is a string. Silence would certify a read through
/// a helper (`access(value)`) the engine never saw as a getter, so the call is
/// an obligation, never a violation and never a silent clean.
fn primitive_result_methods(
    ctx: &AnalysisContext<'_>,
    file: &solid_facts::FileFacts,
) -> Vec<StaticDefect> {
    use solid_dialect::ResultSlot;
    let audited = |file: &solid_facts::FileFacts, call: &solid_facts::ast::CallFact| {
        crate::call_primitive_name_cached(
            ctx.semantic_lookup,
            file,
            call,
            ctx.entities,
            ctx.symbol_names,
            ctx.dialect,
        )
        .and_then(|name| name.primitive())
        .filter(|primitive| {
            [ResultSlot::Whole, ResultSlot::TupleItem(0)]
                .into_iter()
                .any(|slot| {
                    ctx.dialect
                        .accessor_getter_length(*primitive, slot)
                        .is_some()
                })
        })
        .and_then(|primitive| Some((primitive, ctx.dialect.name_of(primitive)?)))
    };
    let mut defects = Vec::new();
    {
        let ast = &file.ast;
        for call in &ast.calls {
            let Some(member) = ast.member_at(ast.peel_ts_sugar_span(call.callee)) else {
                continue;
            };
            // Walk down the receiver chain to the call that roots it.
            let mut receiver = ast.peel_ts_sugar_span(member.object);
            let mut root = None;
            loop {
                // A receiver held in a `const pair = createSignal(1)` binding
                // is the primitive's result as well.
                let held = ast.reference_declaration(receiver).and_then(|declaration| {
                    ast.bindings
                        .iter()
                        .find(|binding| {
                            binding.shape == solid_facts::ast::BindingShape::Identifier
                                && binding.names.iter().any(|name| name.span == declaration)
                        })
                        .and_then(|binding| ast.call_at(binding.call_initializer?))
                });
                let Some(inner) = ast.call_at(receiver).or(held) else {
                    break;
                };
                if let Some(found) = audited(file, inner) {
                    root = Some(found);
                    break;
                }
                let Some(next) = ast.member_at(ast.peel_ts_sugar_span(inner.callee)) else {
                    break;
                };
                receiver = ast.peel_ts_sugar_span(next.object);
            }
            let Some((_, spelling)) = root else {
                continue;
            };
            defects.push(StaticDefect {
                kind: StaticDefectKind::ReactiveDispatchUnresolved {
                    callee: spelling.to_owned(),
                    member: file.source_text(member.property).map(str::to_owned),
                },
                location: location(file.path.shared(), call.span),
                analysis_context: crate::PRIMITIVE_RESULT_METHOD.to_owned(),
                fixes: vec![],
                uncertain: true,
            });
        }
    }
    defects
}

/// A dialect primitive referenced as a value rather than called by name
/// (`const effect = createEffect; effect(…)`, `Reflect.apply(createEffect,
/// …)`, `afterEach(cleanup)`, `[Solid.createEffect]`). Every rule finds a
/// primitive at a call whose callee resolves to it, so the calls such a value
/// reaches are invisible to all of them: an obligation at the reference, never
/// a silent clean.
///
/// Every reference counts except the ones a rule already reads or that are
/// not values: the callee and its plain member chain, a JSX tag, a type query
/// or type name (annotations, `implements`, interface heritage), an import or
/// export specifier and the declaration. Listing value positions instead
/// missed class fields, `yield` and JSX attribute values.
fn primitive_value_references(
    ctx: &AnalysisContext<'_>,
    file: &solid_facts::FileFacts,
) -> Vec<StaticDefect> {
    let mut defects = Vec::new();
    {
        let ast = &file.ast;
        // The callee of a call that resolves to a primitive, and the plain
        // member chain it is (`createEffect`, `Solid.createEffect`): the rules
        // read these. Anything else is a value the call reaches: a method of
        // the primitive (`createEffect.call(…)`, `.bind(…)`) or a choice
        // (`(flag ? a : b)(…)`).
        let mut callees = HashSet::default();
        for call in ast.calls.iter().filter(|call| {
            crate::call_primitive_name_cached(
                ctx.semantic_lookup,
                file,
                call,
                ctx.entities,
                ctx.symbol_names,
                ctx.dialect,
            )
            .is_some()
        }) {
            let mut span = ast.peel_ts_sugar_span(call.callee);
            callees.insert(span);
            while let Some(member) = ast.member_at(span) {
                callees.insert(member.property);
                span = ast.peel_ts_sugar_span(member.object);
                callees.insert(span);
            }
        }
        let candidates = ast
            .identifiers
            .iter()
            .filter(|identifier| identifier.role == solid_facts::ast::IdentifierRole::Reference)
            .map(|identifier| identifier.span)
            .chain(
                ast.members
                    .iter()
                    .filter(|member| !ast.computed_members.contains(&member.span))
                    .map(|member| member.property),
            );
        let mut seen = HashSet::default();
        for span in candidates {
            if callees.contains(&span)
                || ast.jsx_elements.iter().any(|element| {
                    element.name.span.contains(span)
                        || element.closing_name.is_some_and(|name| name.contains(span))
                })
                || ast.type_queries.iter().any(|query| query.contains(span))
                || ast.type_names.iter().any(|name| name.contains(span))
                || crate::indexes::reference_is_declaration_or_module_surface(file, span)
                || !seen.insert(span)
            {
                continue;
            }
            let Some(primitive) = crate::referenced_primitive(
                file,
                span,
                ctx.entities,
                ctx.symbol_names,
                ctx.dialect,
            ) else {
                continue;
            };
            defects.push(StaticDefect {
                kind: StaticDefectKind::ReactiveDispatchUnresolved {
                    callee: primitive.to_string(),
                    member: None,
                },
                location: location(file.path.shared(), span),
                analysis_context: crate::PRIMITIVE_VALUE.to_owned(),
                fixes: vec![],
                uncertain: true,
            });
        }
    }
    defects
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
fn component_props_destructure(
    ctx: &AnalysisContext<'_>,
    tables: &crate::late_chunks::DiscoveryTables<'_>,
    file: &solid_facts::FileFacts,
) -> Vec<StaticDefect> {
    let mut defects = Vec::new();
    {
        for function in &file.ast.functions {
            // Solid invokes components with one props object. Reject the
            // overwhelmingly common non-destructured functions before the
            // whole-project component-identity lookup.
            if function.parameters.len() <= 1
                && let Some(parameter) = function
                    .parameters
                    .first()
                    .filter(|parameter| parameter.shape == solid_facts::ast::BindingShape::Object)
                // The compiler deleted this function. A component declared
                // inside a value the emitter removed cannot be rendered from
                // anywhere -- the declaration went with the value, and nothing
                // outside the deleted expression can name it -- so "this
                // component never updates" is a claim about code that does not
                // run. See the body-binding loop below for the same gate and
                // the reason it is a positive lookup rather than a role.
                && !crate::execution_role::discarded_region_contains(file, parameter.pattern)
                && ctx
                    .semantic_lookup
                    .function_may_be_component(file, function)
            {
                let location = location(file.path.shared(), parameter.pattern);
                let mut uncertain = ctx
                    .semantic_lookup
                    .function_component_status(file, function)
                    == crate::indexes::ComponentStatus::Uncertain;
                uncertain |= match destructured_props_use(tables, parameter, &location) {
                    crate::source_discovery::PropUse::Static => continue,
                    crate::source_discovery::PropUse::Reactive => false,
                    crate::source_discovery::PropUse::Unknown => true,
                };
                defects.push(StaticDefect {
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
            return defects;
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
                .and_then(|symbol| tables.prop_source(symbol))
                .map(|(_, declaration)| declaration.clone());
            let props = prop_declaration.is_some();
            let reactive_object =
                props || binding_initializes_reactive_store(ctx, tables, file, binding);
            if !reactive_object {
                continue;
            }
            // The compiler deleted this destructure, so there is no
            // snapshot to be stale: SC1003's claim is that the bindings are
            // read once and then never update, and code the emitter removed is
            // read zero times in either compiler's output.
            //
            // A positive lookup and an early return, the shape
            // `push_owner_requirement` uses, rather than a
            // `ExecutionRole::DiscardedRendering` arm in the allowlist below.
            // That list means something else -- "this context is fresh at call
            // time, so the destructure is legal" -- and a deleted destructure
            // is not legal-because-fresh, it is absent. Adding the role there
            // would make the two claims indistinguishable to the next reader,
            // and would silently answer for any future role added to the list.
            if crate::execution_role::discarded_region_contains(file, binding.pattern) {
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
            // An unknown role proves no setup-time read: it is either no fact
            // at all, or invocations in several non-reporting roles (ADR
            // 0323's merge: a tracked memo and an `untrack`, an event and a
            // lazy memo), each fresh at call time. Not a proven snapshot.
            let mut uncertain = matches!(
                role,
                crate::ExecutionRole::Unknown | crate::ExecutionRole::Unreported
            ) || prop_symbol
                .is_some_and(|symbol| tables.uncertain_prop_source(symbol));
            if let Some(declaration) = &prop_declaration {
                match destructured_props_use(tables, binding, declaration) {
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
            defects.push(StaticDefect {
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
    defects
}

/// The caller classification of the props a destructuring pattern binds: the
/// named slots when they are all it binds, the whole object when a rest
/// element (or an unresolved slot) captures the remainder.
fn destructured_props_use(
    tables: &crate::late_chunks::DiscoveryTables<'_>,
    binding: &solid_facts::ast::BindingFact,
    declaration: &Location,
) -> crate::source_discovery::PropUse {
    if binding.names.len() > binding.object_slots.len() {
        return tables.props_object_use(declaration);
    }
    tables.props_names_use(
        declaration,
        binding
            .object_slots
            .iter()
            .map(|slot| slot.property.as_str()),
    )
}

fn binding_initializes_reactive_store(
    ctx: &AnalysisContext<'_>,
    tables: &crate::late_chunks::DiscoveryTables<'_>,
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
        .is_some_and(|symbol| tables.source_kind(symbol) == Some(&ReactiveSourceKind::Store))
    {
        return true;
    }
    let Some(call) = binding
        .call_initializer
        .and_then(|initializer| file.ast.call_at(initializer))
    else {
        return false;
    };
    known_primitive(&crate::call_primitive_name_cached(
        ctx.semantic_lookup,
        file,
        call,
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
    after_await::run(ctx, draft);
}

/// SC1002 through a call: a call after the first await of a tracked async
/// computation whose callee reads an accessor in its own body (`await load();
/// return byUser(posts)`). The prepass above proves only a read written in the
/// computation itself; the callee's read reaches this call as a read the
/// interprocedural pass attributed to it (`ReactiveRead::origin`), which the
/// prepass runs too early to see. That read runs after the await, so it is not
/// tracked, but whether the callee reads on this call rather than on some
/// paths is not proven: the finding is an obligation, never a violation.
pub(crate) fn reactive_read_after_await_through_calls(
    ctx: &AnalysisContext<'_>,
    draft: &mut ProgramDraft,
) {
    let files_by_path: std::collections::HashMap<&str, &solid_facts::FileFacts> = ctx
        .facts
        .files
        .iter()
        .map(|file| (file.path.as_str(), file))
        .collect();
    let mut attributed = std::collections::HashMap::<(&str, u64, u64), Vec<&ReactiveRead>>::new();
    for read in draft.reads.iter().filter(|read| read.origin.is_some()) {
        attributed
            .entry((
                read.location.path.as_ref(),
                read.location.start_byte,
                read.location.end_byte,
            ))
            .or_default()
            .push(read);
    }
    if attributed.is_empty() {
        return;
    }
    let mut computations: Option<AsyncComputations> = None;
    let mut defects = Vec::new();
    let proven_identity = StaticDefectKind::ReactiveReadAfterAwait {
        accessor: String::new(),
    }
    .dedup_identity();
    for typescript_file in ctx.facts.typescript.files() {
        for function in typescript_file.async_functions.iter() {
            let calls: Vec<(
                &solid_facts::FileFacts,
                &solid_facts::ast::CallFact,
                &[&ReactiveRead],
            )> = function
                .calls_after_await
                .iter()
                .filter_map(|call| {
                    let file = *files_by_path.get(call.path.as_ref())?;
                    let candidate = file.ast.calls.iter().find(|candidate| {
                        u64::from(candidate.callee.start) == call.start_byte
                            && u64::from(candidate.callee.end) == call.end_byte
                    })?;
                    let reads = attributed.get(&(
                        file.path.as_str(),
                        u64::from(candidate.span.start),
                        u64::from(candidate.span.end),
                    ))?;
                    Some((file, candidate, reads.as_slice()))
                })
                .collect();
            if calls.is_empty() {
                continue;
            }
            let index = computations
                .get_or_insert_with(|| AsyncComputations::new(ctx, &ctx.facts.typescript));
            let Some(analysis_context) = tracked_async_computation_context(ctx, function, index)
            else {
                continue;
            };
            for (file, candidate, reads) in calls {
                // The prepass owns an accessor callee and a typed candidate.
                if call_accessor_symbol(ctx, file, candidate).is_some_and(|symbol| {
                    ctx.accessors.contains_key(symbol) || typed_unknown(ctx, symbol)
                }) {
                    continue;
                }
                let mut reported = HashSet::default();
                for read in reads {
                    // ADR 0195: a source already read before the first
                    // suspension is a dependency, and stays one.
                    if ctx.entities.get(&read.declaration).is_some_and(|symbol| {
                        tracked_before_first_suspension(ctx, &files_by_path, function, symbol)
                    }) || !reported.insert(read.accessor.clone())
                    {
                        continue;
                    }
                    // The prepass already proved this read: the helper reads
                    // on every completion of the call, so the obligation the
                    // summary leaves is discharged at the read itself.
                    if read.origin.as_ref().is_some_and(|origin| {
                        draft.seen_diagnostics.contains(&(
                            proven_identity,
                            origin.path.clone(),
                            origin.start_byte,
                        ))
                    }) {
                        continue;
                    }
                    defects.push(StaticDefect {
                        kind: StaticDefectKind::ReactiveReadAfterAwaitThroughCall {
                            accessor: read.accessor.to_string(),
                            via: read.via.to_string(),
                        },
                        location: location(file.path.shared(), candidate.span),
                        analysis_context: analysis_context.clone(),
                        fixes: vec![],
                        uncertain: true,
                    });
                }
            }
        }
    }
    for defect in defects {
        draft.push_defect(defect);
    }
}

/// ADR 0294: a typed candidate whose implementation is not visible.
fn typed_unknown(ctx: &AnalysisContext<'_>, symbol: &SymbolId) -> bool {
    !ctx.accessors.contains_key(symbol)
        && ctx
            .typed_sources
            .get(symbol)
            .is_some_and(|typed| !typed.implementation_visible)
}

fn typed_after_await_obligation(
    file: &solid_facts::FileFacts,
    callee: solid_facts::core::Span,
) -> StaticDefect {
    StaticDefect {
        kind: StaticDefectKind::ReactiveDispatchUnresolved {
            callee: file.source_text(callee).unwrap_or("this value").to_owned(),
            member: None,
        },
        location: location(file.path.shared(), callee),
        analysis_context: crate::TYPE_ONLY_SOURCE.to_owned(),
        fixes: vec![],
        uncertain: true,
    }
}

/// The resolved location of an async function's member-read analysis: its
/// file, its AST function, and the end of the first await in the function's
/// own unconditional flow. `None` means the function has no straight-line
/// await, so no member read can be dominated by one and the rule has nothing
/// to prove there.
/// The accessor symbol a call reads through, resolved as
/// [`reactive_read_after_await`] resolves an after-await call: a member call
/// by its (non-computed) property, any other call by its callee.
fn call_accessor_symbol<'s>(
    ctx: &'s AnalysisContext<'_>,
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
) -> Option<&'s SymbolId> {
    let callee = file.ast.peel_ts_sugar_span(call.callee);
    if let Some(member) = file.ast.member_at(callee) {
        return (!file.ast.computed_members.contains(&member.span))
            .then(|| ctx.entities.at(file.path.as_str(), member.property))
            .flatten();
    }
    ctx.entities.at(file.path.as_str(), call.callee)
}

/// ADR 0195: whether `symbol` is read, in the async function's own
/// straight-line flow, before the function's first suspension of any kind (a
/// conditional await included: a read after one may already run detached).
/// Such a read runs on every execution, inside the tracking window, so the
/// source is a dependency of the computation and a later read of it after an
/// `await` loses nothing. rc.13's dev build agrees: `checkPostAwaitRead` stays
/// silent for a source already among the computation's dependencies.
fn tracked_before_first_suspension(
    ctx: &AnalysisContext<'_>,
    files_by_path: &std::collections::HashMap<&str, &solid_facts::FileFacts>,
    function: &typefacts::AsyncFunctionFact,
    symbol: &SymbolId,
) -> bool {
    let Some(file) = files_by_path.get(&*function.expression.path) else {
        return false;
    };
    let (Ok(start), Ok(end)) = (
        u32::try_from(function.expression.start_byte),
        u32::try_from(function.expression.end_byte),
    ) else {
        return false;
    };
    let expression = file.ast.peel_ts_sugar_span(Span::new(start, end));
    let Some(ast_function) = file.ast.function_at(expression) else {
        return false;
    };
    let own = |span: Span| {
        ast_function.body.contains(span)
            && containing_ast_function(&file.ast, span)
                .is_some_and(|owner| owner.span == ast_function.span)
    };
    // A function that never suspends (a computation returning a promise it
    // did not `await`) runs entirely inside its tracking window.
    let first_suspension = file
        .ast
        .awaits
        .iter()
        .chain(&file.ast.implicit_suspensions)
        .filter(|suspension| own(**suspension))
        .map(|suspension| suspension.start)
        .min()
        .unwrap_or(u32::MAX);
    file.ast.straight_line_calls.iter().any(|span| {
        span.end <= first_suspension
            && own(*span)
            && file
                .ast
                .call_at(*span)
                .and_then(|call| call_accessor_symbol(ctx, file, call))
                .is_some_and(|candidate| candidate == symbol)
    })
}

/// The store paths a function reads in its own straight-line flow before its
/// first suspension, as `(root, "a.b")` (ADR 0195 for store paths). The read
/// ran inside the tracking window, so every key along the path is a dependency
/// of the computation, and `checkPostAwaitRead` stays silent for a later read
/// of a key that is already among the dependencies (`dev-shared.js`,
/// `el._deps`). Only plain, non-optional, non-computed paths count.
pub(super) fn tracked_store_paths_before_first_suspension(
    ctx: &AnalysisContext<'_>,
    files_by_path: &std::collections::HashMap<&str, &solid_facts::FileFacts>,
    function: &typefacts::AsyncFunctionFact,
) -> Vec<(SymbolId, String)> {
    let Some(file) = files_by_path.get(&*function.expression.path) else {
        return Vec::new();
    };
    let (Ok(start), Ok(end)) = (
        u32::try_from(function.expression.start_byte),
        u32::try_from(function.expression.end_byte),
    ) else {
        return Vec::new();
    };
    let expression = file.ast.peel_ts_sugar_span(Span::new(start, end));
    let Some(ast_function) = file.ast.function_at(expression) else {
        return Vec::new();
    };
    let own = |span: Span| {
        ast_function.body.contains(span)
            && containing_ast_function(&file.ast, span)
                .is_some_and(|owner| owner.span == ast_function.span)
    };
    let first_suspension = file
        .ast
        .awaits
        .iter()
        .chain(&file.ast.implicit_suspensions)
        .filter(|suspension| own(**suspension))
        .map(|suspension| suspension.start)
        .min()
        .unwrap_or(u32::MAX);
    let mut paths = Vec::new();
    for member in &file.ast.members {
        if member.span.end > first_suspension
            || !own(member.span)
            || file
                .ast
                .completion_sites
                .binary_search(&member.span)
                .is_err()
            || file.ast.is_plain_assignment_target(member.span)
        {
            continue;
        }
        let Some(symbol) = ctx
            .entities
            .get(&location(file.path.shared(), member.object))
        else {
            continue;
        };
        if ctx.source_kinds.get(symbol) != Some(&ReactiveSourceKind::Store)
            || !ctx.accessors.contains_key(symbol)
        {
            continue;
        }
        if let Some(path) = store_path_text(file, member.span) {
            paths.push((symbol.clone(), path));
        }
    }
    paths
}

/// `a.b.c` of a member chain written with plain dots and identifiers.
fn store_path_text(file: &solid_facts::FileFacts, span: Span) -> Option<String> {
    let text = file.source_text(span)?;
    let path: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    let plain = path
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '$' || c == '.');
    (plain && path.contains('.') && !path.starts_with('.') && !path.ends_with('.')).then_some(path)
}

/// Whether the path `read` is already a dependency given the paths `tracked`
/// read before the first suspension: the same root and a tracked path that is
/// `read` or extends it (reading `a.b` tracks the key of `a` too).
fn store_path_tracked(tracked: &[(SymbolId, String)], symbol: &SymbolId, read: &str) -> bool {
    tracked.iter().any(|(root, path)| {
        root == symbol
            && path.starts_with(read)
            && (path.len() == read.len() || path[read.len()..].starts_with('.'))
    })
}

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
    let ast_function = file.ast.function_at(expression)?;
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
    index: &AsyncComputations,
) -> Option<String> {
    let function_symbol = index.roots.root(
        ctx.aliases
            .get(function.symbol.as_ref())
            .map_or(function.symbol.as_ref(), SymbolId::as_str),
    );
    // The calls that name this function, in project order: a call whose first
    // argument IS the function's expression (an async function merely nested in
    // the argument -- an async IIFE, an async closure the computation returns
    // -- is not suspended by the primitive), or whose argument resolves to the
    // same async root. The first one whose callee tracks reads
    // decides, exactly as a scan over every call of every file would.
    let mut candidates = std::collections::BTreeSet::<(usize, usize)>::new();
    let range = u32::try_from(function.expression.start_byte)
        .ok()
        .zip(u32::try_from(function.expression.end_byte).ok())
        .map(|(start, end)| Span::new(start, end));
    // A range that does not fit a span cannot be compared, which refused every
    // call of the function's own file, semantic or not.
    let own_file =
        |file_index: usize| *ctx.facts.files[file_index].path.as_str() == *function.expression.path;
    if let Some(range) = range {
        for (file_index, file) in ctx.facts.files.iter().enumerate() {
            if !own_file(file_index) {
                continue;
            }
            for (call_index, call) in file.ast.calls.iter().enumerate() {
                if call
                    .arguments
                    .first()
                    .is_some_and(|argument| file.ast.peel_ts_sugar_span(argument.span) == range)
                {
                    candidates.insert((file_index, call_index));
                }
            }
        }
    }
    if let Some(semantic) = index.by_argument_root.get(function_symbol) {
        candidates.extend(
            semantic
                .iter()
                .copied()
                .filter(|&(file_index, _)| range.is_some() || !own_file(file_index)),
        );
    }
    candidates.into_iter().find_map(|(file_index, call_index)| {
        let file = &ctx.facts.files[file_index];
        let candidate = &file.ast.calls[call_index];
        let primitive = crate::call_primitive_name_cached(
            ctx.semantic_lookup,
            file,
            candidate,
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
}

/// What [`tracked_async_computation_context`] asks of the whole project, built
/// once per prepass instead of once per async function: the alias chains of
/// async functions, and every call whose first argument resolves to a symbol,
/// keyed by that symbol's async root.
struct AsyncComputations<'t> {
    roots: AsyncSymbolRoots<'t>,
    /// `(file index, call index)` pairs in project order.
    by_argument_root: std::collections::HashMap<String, Vec<(usize, usize)>>,
}

impl<'t> AsyncComputations<'t> {
    fn new(ctx: &AnalysisContext<'_>, table: &'t solid_facts::TypeScriptTable) -> Self {
        let roots = AsyncSymbolRoots::new(table);
        let mut by_argument_root = std::collections::HashMap::<String, Vec<(usize, usize)>>::new();
        for (file_index, file) in ctx.facts.files.iter().enumerate() {
            for (call_index, call) in file.ast.calls.iter().enumerate() {
                let Some(symbol) = call
                    .arguments
                    .first()
                    .and_then(|argument| ctx.entities.at(file.path.as_str(), argument.span))
                else {
                    continue;
                };
                by_argument_root
                    .entry(roots.root(symbol).to_owned())
                    .or_default()
                    .push((file_index, call_index));
            }
        }
        Self {
            roots,
            by_argument_root,
        }
    }
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
    store_tracking: &dyn Fn(&SymbolId, Option<&str>) -> pre_await::Tracking,
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
            // A key read on every run before the first suspension is a
            // dependency already; a key another path may alias is not proven
            // either way.
            match store_tracking(symbol, store_path_text(file, member.span).as_deref()) {
                pre_await::Tracking::Tracked => continue,
                pre_await::Tracking::Possible => (name, true),
                pre_await::Tracking::Untracked => (name, false),
            }
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
                // A proven read makes the condition reactive. A read that is
                // only an uncertifiable obligation makes it possibly reactive,
                // which is an obligation here too, never a violation.
                let (mut proven, mut possible) = (false, false);
                for read in draft.reads.iter().filter(|read| {
                    read.location.path == file.path.as_str().into()
                        && u64::from(test.start) <= read.location.start_byte
                        && read.location.end_byte <= u64::from(test.end)
                }) {
                    if read.is_uncertifiable() {
                        possible = true;
                    } else {
                        proven = true;
                    }
                }
                if proven || possible {
                    let location = location(file.path.shared(), *test);
                    draft.push_defect(StaticDefect {
                        kind: StaticDefectKind::ComponentReturnsConditionally,
                        location,
                        analysis_context: file
                            .source_text(name.span)
                            .unwrap_or_default()
                            .to_owned(),
                        fixes: vec![],
                        uncertain: !proven
                            || component_status == crate::indexes::ComponentStatus::Uncertain,
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
    if let Some(conditional) = file.ast.conditional_at(expression) {
        tests.push(conditional.test);
        return_spine_tests(file, conditional.consequent, tests);
        return_spine_tests(file, conditional.alternate, tests);
    } else if let Some(logical) = file.ast.logical_at(expression) {
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

/// SC9012 for a callback that runs when the call's returned object is read
/// ([`solid_dialect::Dialect::callback_runs_on_result_access`]; rc.9's
/// `omit(props, hidden)` predicate).
///
/// Code inside such a callback has no execution role: it runs wherever the
/// returned view is read, in the reader's tracking scope and under the
/// reader's owner (or once per property during the call where `Proxy` is
/// unavailable), and the engine does not follow reads of a view back to the
/// call that made it. So it can never be a violation -- the reader's scope is
/// not knowable where the predicate is written -- and it cannot be certified
/// either unless the predicate is proven inert. This rule is that proof, and
/// reports the obligation wherever it fails.
///
/// Inert means an inspectable function body -- the literal written at the
/// position, or a `function` declaration or `const` arrow in the same file the
/// argument names exactly -- in which:
///
/// - the engine recorded no reactive read, write, action invocation or async
///   read, and no identifier refers to an accessor, setter, action, reactive
///   source or props binding ([`RESULT_ACCESS_REACTIVE_OPERATION`]); and
/// - every call resolves to a standard-library declaration
///   ([`RESULT_ACCESS_OPAQUE_CALL`]). A project helper, a package export or an
///   unresolved callee may do anything in the reader's scope.
///
/// A value with no inspectable body reports
/// [`RESULT_ACCESS_BODY_UNRESOLVED`], unless it is proven not to be a function
/// at all (a key-list call that happens to have two arguments, such as
/// `omit(props, "a")`). Two shapes are left to other passes and say so:
///
/// - a **parameter** forwarded into the position: its body is the caller's,
///   and the unknown-callback obligation already stays open for it;
/// - a **spread** argument, which TypeScript types as the key-list form.
///
/// [`RESULT_ACCESS_REACTIVE_OPERATION`]: crate::RESULT_ACCESS_REACTIVE_OPERATION
/// [`RESULT_ACCESS_OPAQUE_CALL`]: crate::RESULT_ACCESS_OPAQUE_CALL
/// [`RESULT_ACCESS_BODY_UNRESOLVED`]: crate::RESULT_ACCESS_BODY_UNRESOLVED
pub(crate) fn result_access_callbacks(ctx: &AnalysisContext<'_>, draft: &mut ProgramDraft) {
    let lookup = ctx.semantic_lookup;
    let mut obligations = Vec::new();
    for file in &ctx.facts.files {
        for call in &file.ast.calls {
            let count = call.arguments.len();
            let Some(primitive) = lookup.primitive_at_call(file, call.span) else {
                continue;
            };
            for (index, argument) in call.arguments.iter().enumerate() {
                if !ctx
                    .dialect
                    .callback_runs_on_result_access(primitive, index, count)
                {
                    continue;
                }
                let Some(reason) = result_access_obligation(ctx, draft, file, argument) else {
                    continue;
                };
                let callee = call
                    .static_callee(&file.source)
                    .or_else(|| ctx.dialect.name_of(primitive))
                    .unwrap_or("the call")
                    .to_owned();
                obligations.push(StaticDefect {
                    kind: StaticDefectKind::ResultAccessCallbackUnplaced { callee },
                    location: location(file.path.shared(), argument.span),
                    analysis_context: reason.to_owned(),
                    fixes: vec![],
                    uncertain: false,
                });
            }
        }
        // The same slot one project wrapper away: the argument of an
        // in-project call whose callee forwards that parameter into it.
        for forwarded in lookup.result_access_forwarded_arguments(file.path.as_str()) {
            let Some((call, argument)) = file.ast.calls.iter().find_map(|call| {
                call.arguments
                    .iter()
                    .find(|argument| argument.span == forwarded.argument)
                    .map(|argument| (call, argument))
            }) else {
                continue;
            };
            let Some(reason) = result_access_obligation(ctx, draft, file, argument) else {
                continue;
            };
            let primitive = ctx
                .dialect
                .name_of(forwarded.primitive)
                .unwrap_or("the call");
            let callee = match call.static_callee(&file.source) {
                Some(wrapper) => format!("{primitive} (through {wrapper})"),
                None => primitive.to_owned(),
            };
            obligations.push(StaticDefect {
                kind: StaticDefectKind::ResultAccessCallbackUnplaced { callee },
                location: location(file.path.shared(), argument.span),
                analysis_context: reason.to_owned(),
                fixes: vec![],
                uncertain: false,
            });
        }
    }
    for obligation in obligations {
        draft.push_defect(obligation);
    }
}

/// The named reason `argument` leaves its result-access callback
/// uncertifiable, or `None` when it is proven inert, proven not a function,
/// or owned by another pass (see [`result_access_callbacks`]).
fn result_access_obligation(
    ctx: &AnalysisContext<'_>,
    draft: &ProgramDraft,
    file: &solid_facts::FileFacts,
    argument: &solid_facts::ast::ArgumentFact,
) -> Option<&'static str> {
    use solid_facts::ast::ArgumentValueKind;
    if argument.spread {
        return None;
    }
    let body = match argument.value {
        ArgumentValueKind::Function | ArgumentValueKind::AsyncFunction => {
            callback_argument_literal(file, argument.span)
        }
        ArgumentValueKind::Identifier => match argument.binding_declaration {
            Some(declaration) if declared_as_parameter(file, declaration) => return None,
            Some(declaration) => same_file_function(file, declaration),
            None => None,
        },
        _ => None,
    };
    let Some(body) = body else {
        let proven_value =
            crate::runtime_semantics::literal_argument_is_not_callable(argument.runtime_value_kind)
                || ctx
                    .semantic_lookup
                    .smallest_contained_callability(file.path.as_str(), argument.span)
                    == Some(typefacts::Callability::NonCallable);
        return (!proven_value).then_some(crate::RESULT_ACCESS_BODY_UNRESOLVED);
    };
    let region = body.span;
    let path = file.path.as_str();
    let inside = |location: &Location| {
        &*location.path == path
            && u32::try_from(location.start_byte).is_ok_and(|start| region.start <= start)
            && u32::try_from(location.end_byte).is_ok_and(|end| end <= region.end)
    };
    let reactive_symbol = |symbol: &SymbolId| {
        ctx.accessors.contains_key(symbol)
            || ctx.setters.contains_key(symbol)
            || ctx.actions.contains_key(symbol)
            || ctx.prop_sources.contains_key(symbol)
            || ctx.uncertain_prop_sources.contains(symbol)
            || ctx.source_kinds.contains_key(symbol)
    };
    if draft.reads.iter().any(|read| inside(&read.location))
        || draft.writes.iter().any(|write| inside(&write.location))
        || draft
            .action_invocations
            .iter()
            .any(|action| inside(&action.location))
        || draft.async_reads.iter().any(|read| inside(&read.location))
        || file.ast.identifiers_within(region).any(|identifier| {
            identifier.role == solid_facts::ast::IdentifierRole::Reference
                && ctx
                    .entities
                    .at(path, identifier.span)
                    .is_some_and(reactive_symbol)
        })
    {
        return Some(crate::RESULT_ACCESS_REACTIVE_OPERATION);
    }
    let opaque_call = file.ast.calls_within(region).any(|call| {
        !ctx.semantic_lookup
            .resolved_callee_call(file, call.callee)
            .and_then(|resolved| resolved.declaration.as_ref())
            .is_some_and(|declaration| declaration.standard_library)
    });
    opaque_call.then_some(crate::RESULT_ACCESS_OPAQUE_CALL)
}

/// Whether the binding declared at `declaration` is a parameter of a function
/// in this file.
fn declared_as_parameter(file: &solid_facts::FileFacts, declaration: Span) -> bool {
    file.ast.functions.iter().any(|function| {
        function
            .parameters
            .iter()
            .flat_map(|parameter| &parameter.names)
            .chain(&function.rest_parameter_names)
            .any(|name| name.span == declaration)
    })
}

/// The function a same-file binding declared at `declaration` denotes: a
/// `function` declaration of that name, or a `const` whose initializer is a
/// function literal. Anything reassignable or indirect is not a body.
pub(crate) fn same_file_function(
    file: &solid_facts::FileFacts,
    declaration: Span,
) -> Option<&solid_facts::ast::FunctionFact> {
    if let Some(function) = file.ast.functions.iter().find(|function| {
        function.kind == solid_facts::ast::FunctionKind::Declaration
            && function
                .name
                .as_ref()
                .is_some_and(|name| name.span == declaration)
    }) {
        return Some(function);
    }
    let binding = file.ast.bindings.iter().find(|binding| {
        binding.immutable
            && binding.initializer_function
            && binding.names.len() == 1
            && binding.names.iter().any(|name| name.span == declaration)
    })?;
    callback_argument_literal(file, binding.initializer?)
}
