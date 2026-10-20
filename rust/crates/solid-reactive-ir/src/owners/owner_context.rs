//! Owner context lookups: providing regions, callback indexes, await continuations, and requirement pushes.

use super::*;

/// The owner context bits that take a function out of the synchronous extent
/// of an owner-providing region written around it. See [`root_owned_at`].
pub(super) const OWNER_CONTEXT_LEAVES_REGION: u8 =
    OWNER_CONTEXT_PROVEN_UNOWNED | OWNER_CONTEXT_LATER_RUN_UNOWNED;

/// The functions standing between `span` and the innermost owner-providing
/// region around it: every function whose body contains `span` and which is
/// itself written inside that region, the region's own callback included.
/// `None` when no region contains `span`.
///
/// `functions` yields `(key, span, body)` for the file's owner nodes; the key
/// comes back unchanged, so the batch pass can carry node indices and a
/// per-file fragment, which has no indices yet, function spans. The innermost
/// region is the only one that needs asking: an outer region's chain is a
/// superset of it, so an outer region answers only where the innermost does.
pub(crate) fn providing_region_chain<K>(
    providing_regions: &[Span],
    functions: impl IntoIterator<Item = (K, Span, Span)>,
    span: Span,
) -> Option<Vec<K>> {
    let region = providing_regions
        .iter()
        .filter(|region| region.contains(span))
        .min_by_key(|region| region.end - region.start)?;
    Some(
        functions
            .into_iter()
            .filter(|(_, function, body)| body.contains(span) && region.contains(*function))
            .map(|(key, _, _)| key)
            .collect(),
    )
}

/// Whether an operation lexically inside an owner-providing region is answered
/// by that region's owner. `chain` is [`providing_region_chain`]'s answer and
/// `context_of` each chain function's propagated owner context. Both owner
/// passes, every operation kind, ask this one question.
///
/// The region's owner is current only for code that runs in the region's
/// synchronous extent, and a function nested in the region leaves that extent
/// when its context carries either bit of [`OWNER_CONTEXT_LEAVES_REGION`]:
///
/// - [`OWNER_CONTEXT_LATER_RUN_UNOWNED`], from a `createRenderEffect` apply
///   ([`solid_dialect::CallbackOwner::InheritsFirstRun`]). `@solidjs/signals`
///   2.0.0-rc.3 and rc.9 (dev and prod client builds) run every apply after
///   the first from the flush with `getOwner() === null`, so an `onCleanup`
///   there under a `createRoot` never runs on dispose and raises
///   `NO_OWNER_CLEANUP`, while the first run's cleanup registers on the root.
/// - [`OWNER_CONTEXT_PROVEN_UNOWNED`], from a callback some invocation of
///   which runs with no owner: a [`solid_dialect::CallbackOwner::None`]
///   position (a `createEffect` apply, a `createReaction` invalidation, a
///   `runWithOwner(null, fn)` callback), a compiler event-handler or
///   directive-apply role, or an `unowned` summary row. Probed on
///   `solid-js`/`@solidjs/signals`/`@solidjs/web` 2.0.0-rc.3 and rc.9, dev and
///   prod client builds, each written inside a `createRoot` (the delegated
///   handler inside a `render` callback): `getOwner()` is `null` in every one
///   of them, the `onCleanup` raises `NO_OWNER_CLEANUP` (dev) and never runs
///   on dispose, and an `onSettled` in the `createEffect` apply runs
///   out-of-band, its returned cleanup raising `SETTLED_CLEANUP_UNOWNED`.
///
/// Such an operation is judged on the owner graph instead, which makes it a
/// violation where the propagated context is proven unowned and uncertifiable
/// where it carries only the later-run bit.
///
/// Only functions *inside* the region are asked. A region written inside the
/// detached function -- a `createRoot` created in a `createEffect` apply, or a
/// compiled JSX child, whose generated render effect owns its expression --
/// answers for what it contains: probed, an `onCleanup` in either runs on that
/// owner's disposal or re-run.
///
/// A callback handed to a reviewed fresh-stack host scheduler (`setTimeout`,
/// `queueMicrotask`, `Promise.then`, an observer constructor, ...) gets the
/// dialect's unowned edge ([`fresh_stack_scheduler_edges`]) and is judged on
/// the graph like the positions above. A nested callback with no owner edge at
/// all (one handed to an unmodelled function, or to a scheduler through a
/// wrapper call) carries neither bit and is still answered by the region: a
/// pre-existing approximation this does not widen.
pub(crate) fn root_owned_at<K>(chain: Option<&[K]>, context_of: impl Fn(&K) -> u8) -> bool {
    chain.is_some_and(|chain| {
        chain
            .iter()
            .all(|function| context_of(function) & OWNER_CONTEXT_LEAVES_REGION == 0)
    })
}

pub(crate) fn owner_callback_index(
    nodes: &[OwnerNode],
    nodes_by_path: &HashMap<String, Vec<usize>>,
    by_symbol: &HashMap<SymbolId, usize>,
    file: &solid_facts::FileFacts,
    argument: Span,
    entities: &EntitySymbols,
) -> Option<usize> {
    nodes_by_path
        .get(file.path.as_str())
        .into_iter()
        .flatten()
        .copied()
        .filter(|index| argument.contains(nodes[*index].span))
        // The argument's function can itself contain nested callbacks or a
        // returned closure. Select the outermost contained function: choosing
        // the smallest one assigns the caller's owner semantics to a nested
        // callback and leaves the actual argument unclassified.
        .max_by_key(|index| nodes[*index].span.end - nodes[*index].span.start)
        .or_else(|| {
            entities
                .get(&location(file.path.shared(), argument))
                .and_then(|symbol| by_symbol.get(symbol))
                .copied()
        })
}

pub(crate) const fn owner_edge_context(kind: OwnerEdgeKind, source: u8) -> u8 {
    match kind {
        OwnerEdgeKind::Preserve => source,
        OwnerEdgeKind::Owned => OWNER_CONTEXT_OWNED,
        OwnerEdgeKind::Unowned => OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_PROVEN_UNOWNED,
        OwnerEdgeKind::Conditional => OWNER_CONTEXT_OWNED | OWNER_CONTEXT_UNOWNED,
        // Not proven unowned: a later run happens only if the primitive runs
        // the callback again, which needs a change nothing here proves. A
        // source that is itself proven unowned stays proven: then every run,
        // first or later, is unowned.
        OwnerEdgeKind::InheritsFirstRun => {
            source | OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_LATER_RUN_UNOWNED
        }
        OwnerEdgeKind::Leaf => OWNER_CONTEXT_LEAF,
    }
}

/// Calls that run after an `await` on every path through their async
/// function's own body, keyed by file and callee start.
///
/// The positions are the producer's `calls_after_await` dominance fact (the
/// same one `reactive-read-after-await` proves reads with): branches, `&&`,
/// `try`/`catch`, loops and `switch` merge conservatively, and a nested
/// closure is never scanned, so a callback handed to `runWithOwner` after the
/// `await` is not one of them. Such a call runs from a promise continuation,
/// on an otherwise empty stack. Whether an owner is current there is the
/// dialect's fresh-stack answer, the same one host schedulers get
/// ([`Dialect::fresh_stack_callback_owner`]): Solid 2.0's owner is a
/// synchronous dynamic scope, so none is. Empty for a dialect that does not
/// answer [`solid_dialect::CallbackOwner::None`].
pub(crate) struct AwaitContinuations<'a> {
    pub(super) starts: HashMap<&'a str, HashSet<u64>>,
}

impl<'a> AwaitContinuations<'a> {
    pub(crate) fn new(facts: &'a ProjectFacts, lookup: &SemanticLookup<'_>) -> Self {
        let mut starts = HashMap::<&'a str, HashSet<u64>>::default();
        if lookup.dialect.fresh_stack_callback_owner() == Some(solid_dialect::CallbackOwner::None) {
            for file in facts.typescript.files() {
                for function in file.async_functions.iter() {
                    for call in &function.calls_after_await {
                        starts
                            .entry(call.path.as_ref())
                            .or_default()
                            .insert(call.start_byte);
                    }
                }
            }
        }
        Self { starts }
    }

    /// Whether the call whose callee starts at `start` runs after an `await`.
    pub(crate) fn contains(&self, path: &str, start: u32) -> bool {
        self.starts
            .get(path)
            .is_some_and(|starts| starts.contains(&u64::from(start)))
    }
}

/// The owner context of an operation that runs after an `await`
/// ([`AwaitContinuations`]): proven unowned on every run, whatever context
/// the enclosing function was entered with.
pub(super) const fn await_continuation_context(context: u8) -> u8 {
    (context | OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_PROVEN_UNOWNED)
        & !(OWNER_CONTEXT_OWNED
            | OWNER_CONTEXT_LEAF
            | OWNER_CONTEXT_COMPONENT_UNCERTAIN
            | OWNER_CONTEXT_LATER_RUN_UNOWNED)
}

pub(crate) fn owner_context_at(
    nodes: &[OwnerNode],
    nodes_by_path: &HashMap<String, Vec<usize>>,
    contexts: &[u8],
    path: &str,
    span: Span,
) -> u8 {
    containing_function_indexed(nodes, nodes_by_path, path, span)
        .map_or(OWNER_CONTEXT_UNOWNED, |index| contexts[index])
}

/// Whether the operation at `span` runs only after the same invocation of its
/// function has seen a non-null owner: it is the right operand of
/// `getOwner() && …`, or sits in the consequent of `getOwner() ? … : …` or of
/// `if (getOwner()) …`, where the test is a call of Solid's own `getOwner`
/// resolved by symbol (a same-named local function proves nothing).
///
/// Such an operation never executes without an owner, so "no scope's disposal
/// can trigger it" is false on every path that reaches it: probed on the
/// audited 2.0.0-rc.9, dev and prod, an exported helper written this way and
/// called at module scope skips `onCleanup` (no `NO_OWNER_CLEANUP`), while the
/// unguarded helper beside it warns. The guard and the operation must share
/// one function body with no `await` between them -- an owner seen before an
/// `await` is not current after it -- and nothing else is read as a guard:
/// `owner && …` over a binding, `!getOwner() || …`, and early returns keep
/// their requirement.
pub(super) fn guarded_by_owner_probe(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let owner_probe = |test: Span| {
        let test = file.ast.peel_ts_sugar_span(test);
        file.ast.calls.iter().any(|call| {
            call.span == test
                && call.arguments.is_empty()
                && lookup.primitive_at_call(file, call.span) == Some(Primitive::GetOwner)
        })
    };
    let function = containing_ast_function(&file.ast, span).map(|function| function.span);
    let same_invocation = |test: Span, region: Span| {
        containing_ast_function(&file.ast, test).map(|function| function.span) == function
            && !file
                .ast
                .awaits
                .iter()
                .any(|awaited| region.contains(*awaited) && awaited.start < span.start)
    };
    file.ast.logical_expressions.iter().any(|logical| {
        logical.operator == solid_facts::ast::LogicalOperatorKind::And
            && logical.right.contains(span)
            && owner_probe(logical.left)
            && same_invocation(logical.left, logical.right)
    }) || file.ast.conditional_expressions.iter().any(|conditional| {
        conditional.consequent.contains(span)
            && owner_probe(conditional.test)
            && same_invocation(conditional.test, conditional.consequent)
    }) || file.ast.if_regions.iter().any(|region| {
        region.consequent.contains(span)
            && owner_probe(region.test)
            && same_invocation(region.test, region.consequent)
    })
}

/// The one place an owner requirement becomes a finding seed — both owner
/// passes, every operation, every uncertainty source — which is why the
/// discarded-region guard is applied here rather than at each push site.
///
/// Two passes build these candidates (`find_missing_owners` in batch,
/// `discover_owner_file` plus the incremental emission), and the comments above
/// record what happens when a rule is duplicated across them: they drift.
pub(crate) fn push_owner_requirement(
    requirements: &mut Vec<OwnerRequirement>,
    seen: &mut HashSet<(String, u64, u64, String)>,
    operation: &str,
    file: &solid_facts::FileFacts,
    span: Span,
    status: OwnerRequirementStatus,
    lookup: &SemanticLookup<'_>,
) {
    // The compiler deleted this operation. There is no owner question to
    // answer and no obligation to record: "this cleanup will never run" is a
    // claim about a call that never happens, and `uncertain` would be wrong too
    // — nothing is unproven here, the two compilers agree the code is gone.
    // Applied at the funnel for the same reason the divergence escalation below
    // is: a new candidate kind cannot forget it.
    if crate::execution_role::discarded_region_contains(file, span) {
        return;
    }
    // The operation runs only once the same invocation has seen an owner, so
    // there is no unowned execution to report and nothing left unproven.
    if guarded_by_owner_probe(file, span, lookup) {
        return;
    }
    let location = location(file.path.as_str(), span);
    if !seen.insert((
        location.path.to_string(),
        location.start_byte,
        location.end_byte,
        operation.into(),
    )) {
        // ADR 0252: a second requirement of the same operation at the same
        // call (a guarded registration beside an unguarded optional one, say)
        // merges into the first. One certain registration proves the call
        // registers; the context is the call's either way, so only the
        // registration's own certainty folds.
        let operation = crate::OwnerRequirementOperation::from_internal(operation);
        let certain = !status.uncertain;
        if certain
            && let Some(existing) = requirements
                .iter_mut()
                .find(|existing| existing.location == location && existing.operation == operation)
        {
            existing.uncertain = existing.missing_jsx_census;
        }
        return;
    }
    {
        let missing_jsx_census = crate::execution_role::missing_jsx_census_region(file, span);
        // An instance field initializer runs at construction: the owner (or
        // leaf scope) of the function the class is written in says nothing
        // about it (review round 2).
        let constructed = crate::execution_role::inside_instance_field(file, span);
        requirements.push(OwnerRequirement {
            operation: crate::OwnerRequirementOperation::from_internal(operation),
            location,
            uncertain: status.uncertain || missing_jsx_census || constructed,
            runtime_uncertain: status.runtime_uncertain,
            caller_uncertain: status.caller_uncertain,
            conditional_owner: status.conditional_owner,
            later_run_unowned: status.later_run_unowned,
            component_uncertain: status.component_uncertain,
            after_await: status.after_await,
            missing_jsx_census,
            through_dependency: None,
            report: status.report,
        });
    }
}
/// Which callback arguments of a call get an owner edge, and of what kind.
///
/// Shared by both owner passes on purpose: each used to keep its own literal
/// list, and lists kept separately drift.
pub(crate) fn owner_callback_edges(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    primitive: &Option<PrimitiveName>,
    lookup: &SemanticLookup<'_>,
) -> Vec<OwnerCallbackEdge> {
    let Some(primitive) = known_primitive(primitive) else {
        let scheduled = fresh_stack_scheduler_edges(file, call, lookup);
        if !scheduled.is_empty() {
            return scheduled;
        }
        // A call that names no primitive can still be an invocation of a
        // function some primitive returned -- but only where the dialect models
        // such a function. Solid 2.0 models none, so the binding-chain walk
        // below would resolve a primitive and then be told `None` every time.
        if !lookup.models_returned_callbacks() {
            return Vec::new();
        }
        let Some((returned, result_slot)) = returned_primitive_invocation(file, call, lookup)
        else {
            return Vec::new();
        };
        return (0..call.arguments.len())
            .filter_map(|argument| {
                lookup
                    .dialect
                    .returned_callback_semantics_at(
                        returned,
                        result_slot,
                        argument,
                        call.arguments.len(),
                    )
                    .owner
                    .map(|owner| OwnerCallbackEdge {
                        argument,
                        kind: callback_owner_edge_kind(owner),
                        source_path: file.path.to_string(),
                        source: call.span,
                    })
            })
            .collect();
    };
    let mut edges = Vec::new();
    for argument in 0..call.arguments.len() {
        let Some(owner) = callback_owner_at_call(file, call, primitive, argument, lookup) else {
            continue;
        };
        let kind = callback_owner_edge_kind(owner);
        if lookup
            .dialect
            .callback_semantics_at(primitive, argument, call.arguments.len())
            .requires_return_invocation
        {
            edges.extend(
                returned_callback_invocation_sites(file, call, lookup)
                    .into_iter()
                    .map(|site| OwnerCallbackEdge {
                        argument,
                        kind: compose_owner_edge(site.inherited_owner, kind),
                        source_path: site.path,
                        source: site.span,
                    }),
            );
        } else {
            edges.push(OwnerCallbackEdge {
                argument,
                kind,
                source_path: file.path.to_string(),
                source: call.span,
            });
        }
    }
    edges
}

/// The owner edges of callbacks handed to a reviewed fresh-stack host
/// scheduler (`runtime_semantics::FRESH_STACK_SCHEDULERS`: `setTimeout`,
/// `queueMicrotask`, `Promise.then`, the observer constructors, ...).
///
/// Two separately reviewed facts compose here. The host fact -- the scheduler
/// invokes the argument only from a task or microtask queue, on an otherwise
/// empty stack -- comes from the compiler-selected standard-library
/// declaration, never from spelling. The owner fact -- what that empty stack
/// means for the owner -- is the dialect's
/// ([`Dialect::fresh_stack_callback_owner`]); a dialect that does not answer
/// gives no edge, and the callback keeps its lexical answer.
///
/// Only an argument that *is* the callback gets the edge: a function literal
/// or an identifier naming one. `setTimeout(wrap(() => ...))` hands the host
/// whatever `wrap` returns, and the arrow inside may run on `wrap`'s own
/// stack, under the scheduling call's owner.
///
/// Both owner passes reach this through [`owner_callback_edges`], so the
/// batch and incremental graphs cannot disagree about it.
pub(crate) fn fresh_stack_scheduler_edges(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    lookup: &SemanticLookup<'_>,
) -> Vec<OwnerCallbackEdge> {
    let Some(owner) = lookup.dialect.fresh_stack_callback_owner() else {
        return Vec::new();
    };
    let Some(resolved) = lookup.resolved_callee_call(file, call.callee) else {
        return Vec::new();
    };
    call.arguments
        .iter()
        .enumerate()
        .filter(|(_, argument)| {
            matches!(
                argument.value,
                solid_facts::ast::ArgumentValueKind::Function
                    | solid_facts::ast::ArgumentValueKind::AsyncFunction
                    | solid_facts::ast::ArgumentValueKind::Identifier
            )
        })
        .filter(|(index, argument)| {
            let callability = lookup
                .entity_at(file.path.as_str(), argument.span)
                .and_then(|entity| entity.callability);
            crate::runtime_semantics::argument_behavior(resolved, callability, *index)
                .is_some_and(crate::runtime_semantics::RuntimeArgumentBehavior::runs_on_fresh_stack)
        })
        .map(|(argument, _)| OwnerCallbackEdge {
            argument,
            kind: callback_owner_edge_kind(owner),
            source_path: file.path.to_string(),
            source: call.span,
        })
        .collect()
}

pub(crate) const fn callback_owner_edge_kind(owner: solid_dialect::CallbackOwner) -> OwnerEdgeKind {
    match owner {
        solid_dialect::CallbackOwner::Creates => OwnerEdgeKind::Owned,
        solid_dialect::CallbackOwner::Conditional => OwnerEdgeKind::Conditional,
        solid_dialect::CallbackOwner::Inherits => OwnerEdgeKind::Preserve,
        solid_dialect::CallbackOwner::None => OwnerEdgeKind::Unowned,
        solid_dialect::CallbackOwner::InheritsFirstRun => OwnerEdgeKind::InheritsFirstRun,
        solid_dialect::CallbackOwner::Leaf => OwnerEdgeKind::Leaf,
    }
}

/// Compose the owner supplied by a returned function's invocation with the
/// owner behavior of the factory callback itself. An explicit owner result on
/// the inner callback dominates; an inheriting callback keeps the invocation
/// site's context.
pub(crate) const fn compose_owner_edge(
    invocation: OwnerEdgeKind,
    callback: OwnerEdgeKind,
) -> OwnerEdgeKind {
    match callback {
        OwnerEdgeKind::Preserve => invocation,
        _ => callback,
    }
}

/// The argument whose callback runs under an owner this call creates (or, for
/// `runWithOwner`, supplies). The two owner passes share this answer; each
/// used to keep its own literal list, and the lists disagreed with every
/// primitive outside seven names.
pub(crate) fn owner_providing_argument(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    primitive: Option<Primitive>,
    lookup: &SemanticLookup<'_>,
) -> Option<usize> {
    let primitive = primitive?;
    (0..call.arguments.len()).find(|index| {
        callback_owner_at_call(file, call, primitive, *index, lookup)
            == Some(solid_dialect::CallbackOwner::Creates)
    })
}

pub(crate) fn containing_leaf_owner(
    file: &solid_facts::FileFacts,
    span: Span,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
) -> Option<String> {
    file.ast
        .arguments_containing(span)
        .find_map(|(call, index)| {
            let owner = crate::call_primitive_name_cached(
                lookup,
                file,
                call,
                entities,
                symbol_names,
                lookup.dialect,
            )?;
            owner
                .primitive()
                .is_some_and(|primitive| {
                    callback_owner_at_call(file, call, primitive, index, lookup)
                        == Some(solid_dialect::CallbackOwner::Leaf)
                })
                .then(|| owner.to_string())
        })
}
