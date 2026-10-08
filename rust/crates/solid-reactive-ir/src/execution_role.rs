//! Execution-role classification.
//!
//! Given a source span, classify the reactive execution context it runs in
//! (tracked JSX, deferred callback, effect apply, event handler, …). This is
//! the compiler-fact classifier plus the semantic (AST-driven) classifier and
//! the two role-keyed read helpers that consume its result.

use std::collections::{HashMap, HashSet};

use solid_dialect::{Dialect, Execution, Primitive};
use solid_facts::core::Span;

use super::{
    EntitySymbols, ExecutionRole, PrimitiveName, SemanticLookup, SymbolId, call_primitive_name,
    exact_jsx_primitive_name, jsx_primitive_name, known_primitive, location,
};
#[path = "project_consumer.rs"]
mod project_consumer;
pub(crate) use project_consumer::ReadConsumerSummaries;

use crate::indexes::ComponentStatus;
use crate::owners::{
    callback_execution_at_call, callback_owner_at_call, containing_ast_function,
    enclosing_function_label, function_binding_name, returned_callback_execution_at_call,
    returned_callback_invocation_sites, returned_primitive_invocation,
};

/// Whether the untracked-rendering role at `span` rests on the *absence* of a
/// compiler census entry rather than on a compiler fact.
///
/// `semantic_execution_role_within` reaches [`ExecutionRole::UntrackedRendering`]
/// two ways. Either a compiler fact said so — an untracked region, a render
/// callback role — or nothing in the execution map classified the span at all
/// and the span is inside a component body. The second way is a proof only
/// while the census is complete over the JSX the source actually contains, and
/// it is not: each producer censuses the JSX *it* lowers, so a source-level JSX
/// expression the producer dropped reaches the checker as a hole
/// indistinguishable from "the compiler proved this never re-runs". Two ways a
/// hole arrives, and this cannot tell them apart because it does not need to:
/// the producer never censused the expression (a nested non-hydratable `<head>`
/// under 1.x; a *template-root* void element's children under 2.0, whose
/// `lower_dom_element` gates on `!is_void_element`), or it censused the
/// expression and then **retracted** the site during lowering because the path
/// discarded the child list (2.0's nested dynamic-`textContent` placeholder, the
/// textarea `value` fold, the inert `<noscript>` fast path).
///
/// Absence of a fact is not a fact. Where the narrowest JSX region containing
/// the read carries no census entry whatsoever, the read is an uncertifiable
/// proof obligation — never a proven untracked read, and equally never a
/// certification that the expression was deleted and is therefore safe, which
/// would be a second claim this has no evidence for either.
///
pub(crate) fn missing_jsx_census(
    file: &solid_facts::FileFacts,
    span: Span,
    execution: ExecutionRole,
) -> bool {
    // Any other role was decided by a fact — a dialect-proven primitive
    // callback, an event or ref census entry, module initialization (an
    // AST-proven one-shot context that needs no census at all), a discarded
    // region (the compiler reported on the JSX and said the code is deleted) —
    // so a census hole cannot be what put the read there.
    if execution != ExecutionRole::UntrackedRendering {
        return false;
    }
    missing_jsx_census_region(file, span)
}

/// Whether the narrowest JSX region containing `span` has no compiler census
/// entry. Ownership uses the structural question directly: unlike a reactive
/// read, an owner requirement does not carry an [`ExecutionRole`] whose
/// fallback classification can gate the check.
pub(crate) fn missing_jsx_census_region(file: &solid_facts::FileFacts, span: Span) -> bool {
    narrowest_jsx_region_containing(file, span).is_some_and(|region| !census_touches(file, region))
}

/// The innermost source-level JSX region containing `span`: an attribute
/// expression container, a spread container, or a child. solid-facts owns this
/// syntax, and it is deliberately read from the AST rather than from the
/// execution map — the whole question is what the source has that the census
/// does not.
///
/// Fragments are reached only indirectly. A fragment has no element fact — no
/// name, no attributes — so `jsx_containing` (which walks `jsx_elements`) never
/// yields one; a read inside `<>…</>` is found only through the enclosing
/// *element's* child span that covers the fragment. That is not a live hole at
/// the current pins, probed rather than assumed: a fragment nested inside an
/// element fails the compile outright in both producers ("Fragments and spread
/// children are not implemented in the AST-native milestone yet"), so the only
/// fragment reaching the checker is a component's top-level one, and its
/// children are censused as tracked regions — the role never falls through to
/// `UntrackedRendering`, so this function is not consulted for them. When a
/// producer starts lowering nested fragments, the child-span path is where a
/// fragment's own expression containers would need to appear.
fn narrowest_jsx_region_containing(file: &solid_facts::FileFacts, span: Span) -> Option<Span> {
    file.ast
        .jsx_containing(span)
        .flat_map(|element| {
            let attributes = element
                .attributes
                .iter()
                .filter(|attribute| {
                    attribute.value_kind == solid_facts::ast::JsxAttributeValueKind::Expression
                })
                .filter_map(|attribute| attribute.value);
            let spreads = element.spreads.iter().map(|spread| spread.span);
            attributes
                .chain(spreads)
                .chain(element.children.iter().copied())
        })
        .filter(|region| region.contains(span))
        .min_by_key(|region| region.end - region.start)
}

/// Whether the compiler deleted the code at `span`.
///
/// The one execution fact that is not a claim about *how* code runs but about
/// whether it is emitted at all, which is why its consumers consult it before
/// every other classification rather than alongside them.
pub(crate) fn discarded_region_contains(file: &solid_facts::FileFacts, span: Span) -> bool {
    file.compiler
        .discarded_regions
        .iter()
        .any(|region| region.span.contains(span))
}

/// Whether the compiler's census says anything at all about `region`.
///
/// Overlap, not containment in either direction: a census entry inside the
/// region and a wider entry covering it are both the compiler having reported
/// on this JSX. Only a region no entry touches is a hole.
fn census_touches(file: &solid_facts::FileFacts, region: Span) -> bool {
    let overlaps = |candidate: Span| candidate.start < region.end && region.start < candidate.end;
    let facts = &file.compiler;
    facts
        .jsx_operations
        .iter()
        .any(|operation| overlaps(operation.span))
        || facts
            .tracked_regions
            .iter()
            .any(|tracked| overlaps(tracked.span))
        || facts
            .untracked_regions
            .iter()
            .any(|untracked| overlaps(untracked.span))
        // A discarded region is the compiler having reported on this JSX — it
        // said the code is deleted. That is a fact, so the region is not a
        // hole, and `missing_jsx_census` must not turn it into an obligation.
        || facts
            .discarded_regions
            .iter()
            .any(|discarded| overlaps(discarded.span))
        || facts.callback_roles.iter().any(|role| overlaps(role.span))
}

/// The effect primitives: the ones 2.0 spells `(compute, apply)`.
fn is_effect(primitive: Primitive) -> bool {
    matches!(
        primitive,
        Primitive::CreateEffect | Primitive::CreateRenderEffect
    )
}

/// The argument an effect runs *after* its compute, if this dialect has one.
///
/// 2.0's `createEffect(compute, apply)` has it at index 1. 1.x's second
/// argument is a seed value threaded to the next run as `prev`, so 1.x has
/// none — and this used to be the literal `1` for both. A read in a 1.x seed
/// would be classified `EffectApply`, reporting it as running in a phase that
/// version does not have.
fn effect_apply_argument(
    dialect: &dyn Dialect,
    primitive: Primitive,
    argument_count: usize,
) -> Option<usize> {
    if !is_effect(primitive) {
        return None;
    }
    (0..argument_count).find(|index| {
        dialect
            .callback_semantics_at(primitive, *index, argument_count)
            .execution
            == Some(Execution::Deferred)
    })
}

fn callback_runs_outside_tracking(
    dialect: &dyn Dialect,
    primitive: Primitive,
    argument: usize,
    argument_count: usize,
) -> bool {
    let semantics = dialect.callback_semantics_at(primitive, argument, argument_count);
    match semantics.execution {
        None => false,
        // A tracked callback creates its own observer unless the primitive's
        // exact runtime contract explicitly overrides that classification.
        Some(Execution::Tracked) => !semantics.tracks_reads,
        // Deferred callbacks execute after/outside the current tracking pass.
        Some(Execution::Deferred) => true,
        // Inline means the callback inherits the caller's Listener. Only
        // primitives such as untrack/createRoot/runWithOwner that explicitly
        // clear Listener belong to the outside-tracking set.
        Some(Execution::Inline) => dialect.runs_callback_deferred(primitive),
    }
}

/// The argument positions holding a callback that runs outside the
/// surrounding tracking scope — an effect's apply argument, or a deferred
/// executor's whole callback.
///
/// A strict subset of [`Dialect::callback_positions`], which also answers for
/// `createMemo`, `createSignal` and the rest of the tracked index-0 set. The
/// two questions are independent: position says *where* a callback sits,
/// [`Dialect::runs_callback_deferred`] says *how it executes*.
fn deferred_callback_positions(
    dialect: &dyn Dialect,
    primitive: Primitive,
    argument_count: usize,
) -> Vec<usize> {
    (0..argument_count)
        .filter(|index| callback_runs_outside_tracking(dialect, primitive, *index, argument_count))
        .filter(|index| !dialect.reports_untracked_reads_at(primitive, *index, argument_count))
        .collect()
}

pub(super) fn execution_role(
    facts: &solid_facts::compiler::ExecutionMap,
    span: Span,
    allowed: &[Span],
) -> ExecutionRole {
    execution_role_where(facts, span, allowed, |_| true)
}

fn execution_role_where(
    facts: &solid_facts::compiler::ExecutionMap,
    span: Span,
    allowed: &[Span],
    callback_applies: impl Fn(&solid_facts::compiler::CallbackRole) -> bool,
) -> ExecutionRole {
    // Deletion dominates, rather than competing on region width like the rest.
    // A discarded region is code the emitter removed, and removed code cannot
    // contain code that runs: any narrower region inside it would describe
    // lowering that the deletion took with it. Every one of the producers'
    // `Elided` spans is a single attribute or child *value* expression, never a
    // wider enclosing construct, so a discarded region cannot swallow a live
    // sibling — checked against both pins' emission sites, not assumed.
    //
    // Ahead of `allowed` too: a deferred-callback span inside a deleted value
    // would otherwise publish a `deferred` timing for a callback nothing ever
    // invokes, which is a positive claim rather than the absence of one.
    if facts
        .discarded_regions
        .iter()
        .any(|region| region.span.contains(span))
    {
        return ExecutionRole::DiscardedRendering;
    }
    if allowed.iter().any(|region| region.contains(span)) {
        return ExecutionRole::DeferredCallback;
    }
    let tracked = facts.tracked_regions.iter().filter_map(|region| {
        region
            .span
            .contains(span)
            .then_some((region.span, ExecutionRole::TrackedJsx, 2_u8))
    });
    let untracked = facts.untracked_regions.iter().filter_map(|region| {
        region
            .span
            .contains(span)
            .then_some((region.span, ExecutionRole::UntrackedRendering, 1_u8))
    });
    let callbacks = facts.callback_roles.iter().filter_map(|callback| {
        (callback.span.contains(span) && callback_applies(callback)).then_some((
            callback.span,
            match callback.role {
                solid_facts::compiler::CallbackRoleKind::EventHandler => {
                    ExecutionRole::EventCallback
                }
                solid_facts::compiler::CallbackRoleKind::Deferred => {
                    ExecutionRole::DeferredCallback
                }
                solid_facts::compiler::CallbackRoleKind::DirectiveApply => {
                    ExecutionRole::DirectiveApply
                }
                solid_facts::compiler::CallbackRoleKind::Render => {
                    ExecutionRole::UntrackedRendering
                }
            },
            0_u8,
        ))
    });
    if let Some((_, role, _)) = tracked
        .chain(untracked)
        .chain(callbacks)
        .min_by_key(|(region, _, tie_break)| (region.end - region.start, *tie_break))
    {
        return role;
    }
    ExecutionRole::Unknown
}

/// Compiler execution role for code that actually runs at `span`.
///
/// A callback-role span covers the complete JSX value expression handed to
/// the compiler. Only a function value inside that expression executes in the
/// callback phase; eager subexpressions such as `onClick={makeHandler()}` or
/// `ref={[makeDirective()]}` execute while rendering and must fall through to
/// their tracked/untracked region or enclosing component.
fn source_execution_role(
    file: &solid_facts::FileFacts,
    span: Span,
    allowed: &[Span],
) -> ExecutionRole {
    execution_role_where(&file.compiler, span, allowed, |callback| {
        // A component's `ref` is a prop, not an ownerless ref application.
        (callback.role != solid_facts::compiler::CallbackRoleKind::DirectiveApply
            || crate::owners::ref_application_on_intrinsic(file, callback.span))
            && (matches!(
                callback.role,
                solid_facts::compiler::CallbackRoleKind::Deferred
                    | solid_facts::compiler::CallbackRoleKind::Render
            ) || file
                .ast
                .functions_within(callback.span)
                .max_by_key(|function| function.span.end - function.span.start)
                .is_some_and(|function| function.body.contains(span)))
    })
}

pub(super) fn semantic_execution_role(
    file: &solid_facts::FileFacts,
    span: Span,
    allowed: &[Span],
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
) -> ExecutionRole {
    semantic_execution_role_within(
        file,
        span,
        allowed,
        entities,
        symbol_names,
        lookup,
        &mut HashSet::new(),
    )
}

/// What the operation's runtime guard does when a root owner is the ambient
/// owner, which is the case directly in a `createRoot` body
/// ([`solid_dialect::Dialect::callback_runs_in_created_root`]) and, where the
/// dialect says so, directly in a component body
/// ([`solid_dialect::Dialect::component_body_runs_under_root`]).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RootBodyGuard {
    /// The guard rejects the operation under a root, as under any
    /// children-capable owner: signal setters, `refresh` and action calls on
    /// every Solid 2 release, store setters where the release's guard does not
    /// exempt roots.
    Rejects,
    /// The guard exempts a root: store setters on a release whose guard tests
    /// `!context._root` ([`solid_dialect::Dialect::store_setter_guard_exempts_roots`]).
    Exempts,
}

/// Classifies a write or action through resolved project call sites when its
/// own source span has no compiler execution fact.
///
/// Any proven tracking-phase invocation makes the operation unsafe. Otherwise
/// a proven imperative role is retained, while cycles with no independently
/// classified call site remain `Unknown`.
///
/// `root_body` says what the operation's guard does directly under a root
/// owner; see [`RootBodyGuard`].
pub(super) fn semantic_write_execution_role(
    file: &solid_facts::FileFacts,
    span: Span,
    allowed: &[Span],
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
    root_body: RootBodyGuard,
) -> ExecutionRole {
    let allowed = write_allowed_spans(file, allowed, lookup);
    semantic_write_execution_role_within(
        file,
        span,
        &allowed,
        entities,
        symbol_names,
        lookup,
        root_body,
        &mut HashSet::new(),
    )
}

/// The regions a write may treat as imperative.
///
/// [`allowed_callback_spans`] lists every callback whose *reads* run outside
/// the surrounding tracking pass, and `createRoot`'s body is one: it clears
/// the listener. For a write that is the wrong question. The body runs under
/// the root, a children-capable owner, and containment in an allowed region
/// is tested before a nested memo or effect compute is recognized, so every
/// write anywhere inside a root used to be classified as legal. The root's own
/// body is decided by [`WriteRegionAdjustment::CreatedRoot`] instead, and what
/// is nested in it by its own callback.
fn write_allowed_spans(
    file: &solid_facts::FileFacts,
    allowed: &[Span],
    lookup: &SemanticLookup<'_>,
) -> Vec<Span> {
    let roots = file
        .ast
        .calls
        .iter()
        .filter_map(|call| {
            let primitive = lookup.primitive_at_call(file, call.span)?;
            Some(
                call.arguments
                    .iter()
                    .enumerate()
                    .filter(move |(index, _)| {
                        lookup
                            .dialect
                            .callback_runs_in_created_root(primitive, *index)
                    })
                    .map(|(_, argument)| argument.span),
            )
        })
        .flatten()
        .collect::<HashSet<_>>();
    allowed
        .iter()
        .copied()
        .filter(|span| !roots.contains(span))
        .collect()
}

/// How the innermost containing callback changes a write's legality region,
/// when it does. Never consulted for reads: reads keep their own classifier.
enum WriteRegionAdjustment {
    /// The write sits in a children-forbidden leaf callback
    /// (`createTrackedEffect`, `onSettled`), which the runtime's write guard
    /// exempts — the write is legal regardless of what surrounds the leaf.
    LeafScope,
    /// The write sits in an owner-transparent inline callback (`untrack`),
    /// which clears tracking but keeps the caller's owner context; the write
    /// is exactly as legal as at the wrapped call itself, so classify there.
    CallSite(Span),
    /// The write sits in a callback that shares its call site's owner only on
    /// its first run ([`solid_dialect::CallbackOwner::InheritsFirstRun`], 2.0
    /// `createRenderEffect`'s apply). Later runs come from the flush, where
    /// writes are legal; the first run is as legal as the call site.
    FirstRunAtCallSite(Span),
    /// The write sits directly in a callback that runs during the call under
    /// a root owner the call creates (`createRoot`'s body). The ambient owner
    /// is that root wherever the call is, so the call site does not matter:
    /// the operation's [`RootBodyGuard`] decides.
    CreatedRoot,
}

/// The write-legality adjustment for the innermost callback directly
/// containing `span`, per the dialect's owner-context model.
///
/// Direct containment (the span's own function is the callback function)
/// guarantees at most one candidate: a nested callback's contents are that
/// callback's business on the next recursion step.
fn write_region_adjustment(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> Option<WriteRegionAdjustment> {
    file.ast
        .arguments_containing(span)
        .find_map(|(call, index)| {
            if !direct_callback_contains(file, call.arguments[index].span, span) {
                return None;
            }
            let primitive = lookup.primitive_at_call(file, call.span)?;
            if lookup.dialect.leaf_scopes_allow_writes()
                && callback_owner_at_call(file, call, primitive, index, lookup)
                    == Some(solid_dialect::CallbackOwner::Leaf)
            {
                return Some(WriteRegionAdjustment::LeafScope);
            }
            if lookup
                .dialect
                .callback_runs_in_created_root(primitive, index)
                && callback_execution_at_call(file, call, primitive, index, lookup)
                    == Some(Execution::Inline)
            {
                return Some(WriteRegionAdjustment::CreatedRoot);
            }
            if lookup
                .dialect
                .callback_preserves_owner_write_context(primitive)
                && callback_execution_at_call(file, call, primitive, index, lookup).is_some()
            {
                return Some(WriteRegionAdjustment::CallSite(call.span));
            }
            if callback_owner_at_call(file, call, primitive, index, lookup)
                == Some(solid_dialect::CallbackOwner::InheritsFirstRun)
            {
                return Some(WriteRegionAdjustment::FirstRunAtCallSite(call.span));
            }
            None
        })
        .or_else(|| nested_in_leaf_scope(file, span, lookup))
}

/// A closure nested in a leaf callback (`createTrackedEffect`, `onSettled`):
/// a timer, a listener, a continuation, a helper the callback defines. It runs
/// later (deferred) or inside the leaf, and the runtime's write guard exempts
/// both. Only the *innermost* primitive callback containing the span decides --
/// an owner-creating primitive nested in the leaf (`createMemo`) is its own
/// scope and keeps its own answer -- and a call that is no primitive (the
/// scheduler a closure is handed to) is looked through.
fn nested_in_leaf_scope(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> Option<WriteRegionAdjustment> {
    if !lookup.dialect.leaf_scopes_allow_writes() {
        return None;
    }
    let (call, index) = file
        .ast
        .arguments_containing(span)
        .filter(|(call, _)| lookup.primitive_at_call(file, call.span).is_some())
        .min_by_key(|(call, index)| {
            let argument = call.arguments[*index].span;
            argument.end - argument.start
        })?;
    let primitive = lookup.primitive_at_call(file, call.span)?;
    (callback_owner_at_call(file, call, primitive, index, lookup)
        == Some(solid_dialect::CallbackOwner::Leaf))
    .then_some(WriteRegionAdjustment::LeafScope)
}

#[allow(clippy::too_many_arguments)]
fn semantic_write_execution_role_within(
    file: &solid_facts::FileFacts,
    span: Span,
    allowed: &[Span],
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
    root_body: RootBodyGuard,
    visiting: &mut HashSet<(String, Span)>,
) -> ExecutionRole {
    // Write legality follows the runtime's *owner* context, which is not
    // always the read-execution context: leaf scopes are write-legal though
    // their reads track, and `untrack` writes answer to the enclosing owner
    // though its reads do not subscribe. Resolve those two adjustments first
    // (innermost outward) and only then classify normally.
    let mut span = span;
    loop {
        match write_region_adjustment(file, span, lookup) {
            Some(WriteRegionAdjustment::LeafScope) => return ExecutionRole::DeferredCallback,
            Some(WriteRegionAdjustment::CallSite(outer)) => span = outer,
            // A root body is a one-shot, untracked run under a
            // children-capable owner -- in dev builds exactly what a
            // component body is, since `solid-js` runs every component body
            // in `createRoot(…, { transparent: true })` then `untrack`. So a
            // rejected operation there takes the component body's role; an
            // exempted one takes the inline untracked callback's, which
            // reports nothing.
            Some(WriteRegionAdjustment::CreatedRoot) => match root_body {
                RootBodyGuard::Rejects => return ExecutionRole::UntrackedRendering,
                RootBodyGuard::Exempts => return ExecutionRole::UntrackedCallback,
            },
            Some(WriteRegionAdjustment::FirstRunAtCallSite(call)) => {
                // Where the call site forbids the write, the first run throws
                // exactly when it runs during the call -- which needs the
                // compute to settle synchronously on its first pass, with no
                // `defer`/`schedule` option, and on rc.9 outside a staged
                // transaction. None of that is proven here, so the write is
                // neither a violation nor legal: it is unclassified. A call
                // site that allows the write leaves every run legal, and the
                // write keeps its own (apply) role.
                let first_run = semantic_write_execution_role_within(
                    file,
                    call,
                    allowed,
                    entities,
                    symbol_names,
                    lookup,
                    root_body,
                    visiting,
                );
                match first_run {
                    ExecutionRole::DiscardedRendering => return first_run,
                    ExecutionRole::Unknown => return ExecutionRole::Unknown,
                    role if role.reports_disallowed_write() => return ExecutionRole::Unknown,
                    _ => break,
                }
            }
            None => {
                if let Some(role) = named_callback_write_role(
                    file,
                    span,
                    allowed,
                    entities,
                    symbol_names,
                    lookup,
                    root_body,
                    visiting,
                ) {
                    return role;
                }
                break;
            }
        }
    }
    let direct = semantic_execution_role(file, span, allowed, entities, symbol_names, lookup);
    // Directly in a component body the dev owner is the component's root, so
    // a guard that exempts roots exempts the body too. Only the body itself:
    // tracked JSX in it runs in a render effect, and a callback nested in it
    // is judged as itself.
    if direct == ExecutionRole::UntrackedRendering
        && root_body == RootBodyGuard::Exempts
        && runs_directly_in_component_root(file, span, lookup)
    {
        return ExecutionRole::UntrackedCallback;
    }
    // The rendering role places code by where it is written. For a write in a
    // function literal nested in the body that is a proof only when the
    // literal provably runs during the body; otherwise the write's role is
    // what its invocation sites prove, which the walk below derives.
    let direct = if direct == ExecutionRole::UntrackedRendering
        && !nested_literal_runs_during_body(file, span, entities, symbol_names, lookup)
    {
        ExecutionRole::Unknown
    } else {
        direct
    };
    // A function literal that is a JSX attribute's value -- an event handler,
    // a callback prop -- is handed to the element; constructing it runs
    // nothing, and whatever expression wraps the JSX (`cond && <el onClick=… />`,
    // a ternary, a `.map` callback, an argument to a helper call) does not
    // change that. The tracked or rendering role such a wrapper gives the
    // *expression* is not a proof about the function inside it.
    if direct.reports_disallowed_write() && enclosed_by_jsx_attribute_function(file, span) {
        return ExecutionRole::Unknown;
    }
    // A tracked role reaches every closure written inside the tracked region
    // or callback, including ones only stored there (`createMemo(() => ({ go:
    // () => set(1) }))`). The role is a proof for the closure only if the
    // closure runs inline in the scope that earned it.
    if direct == ExecutionRole::TrackedJsx
        && enclosed_by_stored_function(file, span, entities, symbol_names, lookup)
    {
        return ExecutionRole::Unknown;
    }
    // Code after an `await` in an async function runs on a later task, with no
    // owner; neither the lexical role nor the role at the function's call sites
    // describes it. Not proven to follow an await on every path (an `await`
    // inside a branch), so the answer is no claim rather than a legal write.
    if (direct == ExecutionRole::Unknown || direct.reports_disallowed_write())
        && follows_await_in_async_function(file, span)
    {
        return ExecutionRole::Unknown;
    }
    if direct != ExecutionRole::Unknown {
        return direct;
    }
    let Some(function) = crate::owners::containing_ast_function(&file.ast, span) else {
        return ExecutionRole::Unknown;
    };
    let key = (file.path.to_string(), function.span);
    if !visiting.insert(key.clone()) {
        return ExecutionRole::Unknown;
    }
    let mut imperative = None;
    for (caller_file, callee) in lookup.function_call_sites(file.path.as_str(), function.span) {
        let caller_allowed = write_allowed_spans(
            caller_file,
            &allowed_callback_spans(caller_file, lookup),
            lookup,
        );
        let role = semantic_write_execution_role_within(
            caller_file,
            callee,
            &caller_allowed,
            entities,
            symbol_names,
            lookup,
            root_body,
            visiting,
        );
        if role.reports_disallowed_write() {
            visiting.remove(&key);
            return role;
        }
        if role != ExecutionRole::Unknown {
            imperative.get_or_insert(role);
        }
    }
    visiting.remove(&key);
    imperative.unwrap_or(ExecutionRole::Unknown)
}

/// Whether `span` sits in a function literal that is the value of a JSX
/// attribute, or in a function nested inside one.
fn enclosed_by_jsx_attribute_function(file: &solid_facts::FileFacts, span: Span) -> bool {
    file.ast
        .functions_body_containing(span)
        .any(|function| jsx_attribute_value_function(file, function.span))
}

/// Whether `span` sits in a function literal, written strictly inside `region`,
/// that the region's own evaluation is not proven to run: a JSX attribute's
/// value (an event handler, a callback prop), an argument handed to a call that
/// is not proven to invoke it during the call, or a stored literal. What such a
/// function does runs when something calls it, not in the region's pass.
fn attribute_function_within(
    file: &solid_facts::FileFacts,
    region: Span,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    file.ast.functions_body_containing(span).any(|function| {
        region.contains(function.span)
            && region != function.span
            && (jsx_attribute_value_function(file, function.span)
                || call_argument_invocation_unproven(file, function, lookup))
    })
}

/// Whether `span`, written in the function whose body is `owner_body`, runs
/// outside that function's own call according to the compiler:
///
/// - in a tracked region of the same function, an attribute or child expression
///   the compiler lowers into a tracked effect, so its reads are subscribed
///   when the region runs. A function that is an attribute value inside the
///   region (a handler, a callback prop) or an argument not proven to run
///   inline is not the region's tracking pass;
/// - in a component property the compiler lowers to a getter, which runs when
///   the consuming component reads the property, never while this function
///   builds the JSX. A consumer that reads it untracked is reported there.
pub(crate) fn runs_outside_owner_call(
    file: &solid_facts::FileFacts,
    span: Span,
    owner_body: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    use solid_facts::compiler::{CompilerExecutionDisposition, CompilerOperationKind};
    file.compiler.tracked_regions.iter().any(|region| {
        owner_body.contains(region.span)
            && region.span.contains(span)
            && !attribute_function_within(file, region.span, span, lookup)
    }) || file
        .compiler
        .semantic_model
        .operations
        .iter()
        .any(|operation| {
            operation.kind == CompilerOperationKind::ComponentProperty
                && operation.execution.disposition
                    == CompilerExecutionDisposition::ComponentPropertyGetter
                && owner_body.contains(operation.span)
                && operation.span.contains(span)
        })
}

/// ADR 0199: [`ExecutionRole::EventCallback`] for code directly in a function
/// literal written as a project component's prop, when that component only
/// ever hands the prop to DOM event dispatch.
///
/// Constructing a callback prop runs nothing; the consumer decides when it
/// runs. Here the consumer is resolved exactly (the tag's symbol is one
/// project function) and every use it makes of the prop is proven to be an
/// intrinsic element's event handler ([`prop_reaches_only_events`]). Then the
/// literal runs, if ever, when the browser dispatches that event, exactly as a
/// literal written on the element itself does, and takes that literal's role.
/// Anything less -- an unresolved tag, a consumer that calls the prop, keeps
/// it, or lets the props object escape -- answers nothing here, and the
/// literal keeps its unproven timing ([`callee_callback_timing`]).
fn forwarded_event_prop_role(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> Option<ExecutionRole> {
    // Any enclosing literal, not only the innermost: whatever runs inside a
    // literal that runs only on dispatch runs only after it, exactly as code
    // nested in a handler written on the element does. Nested primitives and
    // inline callbacks were classified by the arms ahead of this one.
    file.ast
        .functions_body_containing(span)
        .any(|literal| {
            component_prop_literal(file, literal.span).is_some_and(|(element, prop)| {
                component_prop_reaches_only_events(
                    file,
                    element,
                    None,
                    prop,
                    lookup,
                    &mut HashSet::new(),
                    &[],
                )
            })
        })
        .then_some(ExecutionRole::EventCallback)
}

/// ADR 0204: whether a function literal written as a project component's prop
/// runs while that component renders.
///
/// The tag resolves to one project function (`function_called_at`), and the
/// element has no spread, which could replace the prop. The component's props
/// parameter is one identifier with no default and is never assigned, and the
/// component's own body calls `props.<prop>` directly: not in a nested
/// function, not in JSX (a tracked region or a prop getter), not in a default
/// parameter. Rendering the element runs the component's body untracked, so
/// the call there runs the literal in that body's strict-read window.
///
/// The call may sit in a branch; like a read written in the body, it is
/// proven to run whenever that code does. Every other consumer -- one that
/// forwards the prop, keeps it, or calls it only from a closure -- proves
/// nothing here.
fn prop_literal_invoked_during_render(
    file: &solid_facts::FileFacts,
    literal: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let Some((element, prop)) = component_prop_literal(file, literal) else {
        return false;
    };
    if !element.spreads.is_empty()
        || element
            .attributes
            .iter()
            .filter(|attribute| {
                attribute.namespace.is_none() && file.source_text(attribute.name) == Some(prop)
            })
            .count()
            != 1
    {
        return false;
    }
    let Some((component_file, component)) =
        lookup.function_called_at(file.path.as_str(), element.name.span)
    else {
        return false;
    };
    component_invokes_prop_in_body(component_file, component, prop, lookup)
}

/// Whether `component`'s own body calls `props.<prop>` directly; see
/// [`prop_literal_invoked_during_render`].
fn component_invokes_prop_in_body(
    file: &solid_facts::FileFacts,
    component: &solid_facts::ast::FunctionFact,
    prop: &str,
    lookup: &SemanticLookup<'_>,
) -> bool {
    if component.r#async || component.generator {
        return false;
    }
    let Some(parameter) = component.parameters.first() else {
        return false;
    };
    if parameter.shape != solid_facts::ast::BindingShape::Identifier
        || parameter.initializer.is_some()
    {
        return false;
    }
    let Some(name) = parameter.names.first() else {
        return false;
    };
    let Some(symbol) = lookup.entities().at(file.path.as_str(), name.span) else {
        return false;
    };
    let names_props = |span: Span| {
        lookup
            .entities()
            .at(file.path.as_str(), file.ast.peel_ts_sugar_span(span))
            == Some(symbol)
    };
    // `props = …` or `props.<prop> = …` would make the call run something else.
    if file.ast.assignments.iter().any(|assignment| {
        component.span.contains(assignment.target)
            && (names_props(assignment.target)
                || file.ast.members.iter().any(|member| {
                    member.span == file.ast.peel_ts_sugar_span(assignment.target)
                        && names_props(member.object)
                        && file.source_text(member.property) == Some(prop)
                }))
    }) {
        return false;
    }
    file.ast.calls.iter().any(|call| {
        crate::owners::written_directly_in(&file.ast, component, call.span)
            && !file.ast.any_jsx_containing(call.span)
            && file.ast.members.iter().any(|member| {
                member.span == file.ast.peel_ts_sugar_span(call.callee)
                    && names_props(member.object)
                    && file.source_text(member.property) == Some(prop)
            })
    })
}

/// The project component element and prop name a function literal is
/// exactly the value of, when it is one.
fn component_prop_literal(
    file: &solid_facts::FileFacts,
    literal: Span,
) -> Option<(&solid_facts::ast::JsxElementFact, &str)> {
    let (element, attribute) = file.ast.jsx_containing(literal).find_map(|element| {
        element
            .attributes
            .iter()
            .find(|attribute| {
                attribute.namespace.is_none()
                    && attribute.value_kind == solid_facts::ast::JsxAttributeValueKind::Expression
                    && attribute
                        .expression
                        .is_some_and(|value| file.ast.peel_ts_sugar_span(value) == literal)
            })
            .map(|attribute| (element, attribute))
    })?;
    if intrinsic_element(file, element) {
        return None;
    }
    Some((element, file.source_text(attribute.name)?))
}

/// Whether `value` is exactly the value of an attribute of an intrinsic
/// element.
fn intrinsic_attribute_value(file: &solid_facts::FileFacts, value: Span) -> bool {
    let value = file.ast.peel_ts_sugar_span(value);
    file.ast.jsx_containing(value).any(|element| {
        intrinsic_element(file, element)
            && element.attributes.iter().any(|attribute| {
                attribute
                    .expression
                    .is_some_and(|expression| file.ast.peel_ts_sugar_span(expression) == value)
            })
    })
}

/// Whether a JSX element is an intrinsic (DOM) element: a lowercase, undotted
/// tag, which JSX resolves as an element name rather than a binding.
fn intrinsic_element(
    file: &solid_facts::FileFacts,
    element: &solid_facts::ast::JsxElementFact,
) -> bool {
    element.member_object.is_none()
        && file
            .source_text(element.name.span)
            .and_then(|name| name.chars().next())
            .is_some_and(|first| first.is_ascii_lowercase())
}

/// Whether the component `function` uses its prop `prop` only as DOM event
/// dispatch.
///
/// The props parameter is one identifier, and it and every view of it this
/// proof follows (a `const` bound to a props merge or split the dialect names,
/// [`solid_dialect::Dialect::merges_props_reactivity`] and
/// [`solid_dialect::Dialect::splits_props`]) is used only as
///
/// - a static member access: `props.<other>` is another prop, and every
///   `props.<prop>` is exactly the value of an intrinsic element's attribute
///   the compiler classifies as an event handler
///   (`<button onClick={props.onPress}>`), inside the function written as such
///   a handler (`onClick={(event) => props.onPress?.(event)}`), or exactly the
///   value of another project component's prop that satisfies this proof;
/// - a spread onto an intrinsic element, when `prop` is an un-namespaced `on…`
///   name: the runtime's spread assigns such a property as an event listener
///   (`@solidjs/web` `assignProp`), the one other property it runs, `ref`, is
///   not an `on…` name;
/// - a spread onto a project component that satisfies this proof; or
/// - the props argument of such a merge or split, whose result is spread or
///   bound to a `const` as above.
///
/// Any other use (a call argument, a destructuring, `props[key]`, a returned
/// or stored props object) could reach the prop any way at all, so it proves
/// nothing. Neither does a cycle.
fn prop_reaches_only_events(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    prop: &str,
    lookup: &SemanticLookup<'_>,
    visiting: &mut HashSet<(String, Span, String)>,
    chain: &[RenderHop],
) -> bool {
    if function.r#async || function.generator || function.rest_parameter {
        return false;
    }
    let Some(parameter) = function.parameters.first() else {
        return false;
    };
    if parameter.shape != solid_facts::ast::BindingShape::Identifier
        || parameter.initializer.is_some()
    {
        return false;
    }
    let Some(name) = parameter.names.first() else {
        return false;
    };
    let Some(symbol) = lookup.entities().at(file.path.as_str(), name.span) else {
        return false;
    };
    if !visiting.insert((file.path.to_string(), function.span, prop.to_owned())) {
        return false;
    }
    let mut aliases = vec![(symbol.clone(), name.span)];
    let mut followed = 0;
    let mut references = 0usize;
    while let Some((alias, declaration)) = aliases.get(followed).cloned() {
        followed += 1;
        for reference in lookup.symbol_references(alias.as_str()) {
            if reference.path.as_ref() != file.path.as_str() {
                return false;
            }
            let (Ok(start), Ok(end)) = (
                u32::try_from(reference.start_byte),
                u32::try_from(reference.end_byte),
            ) else {
                return false;
            };
            let reference = Span::new(start, end);
            if reference == declaration {
                continue;
            }
            references += 1;
            if !function.body.contains(reference) {
                return false;
            }
            let proven = if let Some(member) = file
                .ast
                .members
                .iter()
                .find(|member| member.object == reference)
            {
                file.source_text(member.property) != Some(prop)
                    || member_reaches_only_events(
                        file,
                        function,
                        member.span,
                        lookup,
                        visiting,
                        chain,
                    )
            } else if let Some(view) = props_view_call(file, reference, lookup) {
                match spread_onto(file, view) {
                    Some(element) => spread_reaches_only_events(
                        file, function, element, prop, lookup, visiting, chain,
                    ),
                    None => match const_binding_of(file, view, lookup) {
                        Some(binding) => {
                            aliases.push(binding);
                            true
                        }
                        None => false,
                    },
                }
            } else if let Some(element) = spread_onto(file, reference) {
                spread_reaches_only_events(file, function, element, prop, lookup, visiting, chain)
            } else if let Some(binding) = const_binding_of(file, reference, lookup) {
                // `const rest = props`, `const root = view as Props`: the same
                // object under another name.
                aliases.push(binding);
                true
            } else {
                false
            };
            if !proven {
                return false;
            }
        }
    }
    references > 0
}

/// Whether one `props.<prop>` member access reaches only event dispatch; see
/// [`prop_reaches_only_events`].
fn member_reaches_only_events(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    member: Span,
    lookup: &SemanticLookup<'_>,
    visiting: &mut HashSet<(String, Span, String)>,
    chain: &[RenderHop],
) -> bool {
    use solid_facts::compiler::CallbackRoleKind;
    let event_handlers = || {
        file.compiler
            .callback_roles
            .iter()
            .filter(|callback| callback.role == CallbackRoleKind::EventHandler)
    };
    // The value of an attribute on some element, peeled exactly.
    let attribute_on = file.ast.jsx_containing(member).find_map(|element| {
        element
            .attributes
            .iter()
            .find(|attribute| {
                attribute.namespace.is_none()
                    && attribute
                        .expression
                        .is_some_and(|value| file.ast.peel_ts_sugar_span(value) == member)
            })
            .map(|attribute| (element, attribute))
    });
    match attribute_on {
        Some((element, _)) if intrinsic_element(file, element) => {
            event_handlers().any(|callback| callback.span.contains(member))
        }
        Some((element, attribute)) => file.source_text(attribute.name).is_some_and(|next| {
            component_prop_reaches_only_events(
                file,
                element,
                Some(function),
                next,
                lookup,
                visiting,
                chain,
            )
        }),
        None => {
            if event_handlers().any(|callback| {
                intrinsic_attribute_value(file, callback.span)
                    && file
                        .ast
                        .functions_within(callback.span)
                        .max_by_key(|handler| handler.span.end - handler.span.start)
                        .is_some_and(|handler| handler.body.contains(member))
            }) {
                return true;
            }
            // Inside a literal written as another project component's prop
            // that itself reaches only event dispatch.
            let literals = file
                .ast
                .functions_body_containing(member)
                .map(|literal| literal.span)
                .collect::<Vec<_>>();
            for literal in literals {
                let Some((element, prop)) = component_prop_literal(file, literal) else {
                    continue;
                };
                if component_prop_reaches_only_events(
                    file,
                    element,
                    Some(function),
                    prop,
                    lookup,
                    visiting,
                    chain,
                ) {
                    return true;
                }
            }
            false
        }
    }
}

/// The element `value` is spread onto, when `value` is exactly the argument of
/// a JSX spread attribute.
fn spread_onto(
    file: &solid_facts::FileFacts,
    value: Span,
) -> Option<&solid_facts::ast::JsxElementFact> {
    file.ast.jsx_containing(value).find(|element| {
        element
            .spreads
            .iter()
            .any(|spread| file.ast.peel_ts_sugar_span(spread.argument) == value)
    })
}

/// Whether spreading a props object carrying `prop` onto `element` reaches
/// only event dispatch; see [`prop_reaches_only_events`].
fn spread_reaches_only_events(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    element: &solid_facts::ast::JsxElementFact,
    prop: &str,
    lookup: &SemanticLookup<'_>,
    visiting: &mut HashSet<(String, Span, String)>,
    chain: &[RenderHop],
) -> bool {
    if intrinsic_element(file, element) {
        return prop.starts_with("on") && !prop.contains(':');
    }
    component_prop_reaches_only_events(file, element, Some(function), prop, lookup, visiting, chain)
}

/// Whether the component `element` renders runs its prop `prop` only after
/// the render: a project component whose every use of it is proven
/// ([`prop_reaches_only_events`]), or a package component whose accepted
/// contract says so ([`package_prop_runs_only_deferred`]).
///
/// `enclosing` is the component whose body writes `element` (`None` for the
/// element that wrote the literal), and `chain` the elements that rendered
/// each enclosing component, outermost first.
fn component_prop_reaches_only_events(
    file: &solid_facts::FileFacts,
    element: &solid_facts::ast::JsxElementFact,
    enclosing: Option<&solid_facts::ast::FunctionFact>,
    prop: &str,
    lookup: &SemanticLookup<'_>,
    visiting: &mut HashSet<(String, Span, String)>,
    chain: &[RenderHop],
) -> bool {
    if let Some((next_file, next_function)) =
        lookup.function_called_at(file.path.as_str(), element.name.span)
    {
        let mut next_chain = chain.to_vec();
        next_chain.push(render_hop(file, element, enclosing, lookup));
        return prop_reaches_only_events(
            next_file,
            next_function,
            prop,
            lookup,
            visiting,
            &next_chain,
        );
    }
    package_prop_runs_only_deferred(file, element, enclosing, prop, lookup, chain)
}

/// What one element in a render chain can put in the props object it hands
/// the component it renders: its attributes by name, and its spreads.
#[derive(Clone, Debug)]
struct RenderHop {
    attributes: Vec<String>,
    spreads: HopSpreads,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HopSpreads {
    /// The element spreads nothing.
    None,
    /// Every spread is the enclosing component's props object, an alias of
    /// it, or an `omit` view of it: a spread that adds no key the props did
    /// not hold.
    PropsViews,
    /// Anything else may add any key.
    Unknown,
}

fn render_hop(
    file: &solid_facts::FileFacts,
    element: &solid_facts::ast::JsxElementFact,
    enclosing: Option<&solid_facts::ast::FunctionFact>,
    lookup: &SemanticLookup<'_>,
) -> RenderHop {
    let attributes = element
        .attributes
        .iter()
        .filter_map(|attribute| file.source_text(attribute.name).map(str::to_owned))
        .collect();
    let spreads = if element.spreads.is_empty() {
        HopSpreads::None
    } else if enclosing.is_some_and(|function| {
        let views = props_key_preserving_views(file, function, lookup);
        !views.is_empty()
            && element.spreads.iter().all(|spread| {
                symbol_at_reference(file, file.ast.peel_ts_sugar_span(spread.argument), lookup)
                    .is_some_and(|symbol| views.contains(&symbol))
            })
    }) {
        HopSpreads::PropsViews
    } else {
        HopSpreads::Unknown
    };
    RenderHop {
        attributes,
        spreads,
    }
}

/// Whether the props object `at`'s element hands its component can hold no
/// `key`: no element on the chain back to the one that wrote the literal
/// writes it, and every spread on the way is a view of the props it received.
fn key_absent(key: &str, at: &RenderHop, chain: &[RenderHop]) -> bool {
    if at.attributes.iter().any(|attribute| attribute == key) {
        return false;
    }
    match at.spreads {
        HopSpreads::None => true,
        HopSpreads::Unknown => false,
        HopSpreads::PropsViews => chain
            .split_last()
            .is_some_and(|(parent, rest)| key_absent(key, parent, rest)),
    }
}

/// The symbol a reference names: its entity row, or the binding it resolves
/// to.
fn symbol_at_reference(
    file: &solid_facts::FileFacts,
    reference: Span,
    lookup: &SemanticLookup<'_>,
) -> Option<SymbolId> {
    lookup
        .entities()
        .at(file.path.as_str(), reference)
        .cloned()
        .or_else(|| {
            lookup
                .binding_at_reference(file.path.as_str(), reference)
                .map(|(_, _, symbol)| symbol)
        })
}

/// The props parameter of `function` and every `const` in its body bound to
/// it, to an alias of such a binding (`view as Props`), or to an `omit` the
/// dialect names over one: the values whose keys are a subset of the props
/// object's. A merge can add keys and is not one of them. Empty when the props
/// parameter is not one identifier.
fn props_key_preserving_views(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
) -> Vec<SymbolId> {
    let Some(parameter) = function.parameters.first() else {
        return Vec::new();
    };
    if parameter.shape != solid_facts::ast::BindingShape::Identifier
        || parameter.initializer.is_some()
    {
        return Vec::new();
    }
    let Some(symbol) = parameter
        .names
        .first()
        .and_then(|name| lookup.entities().at(file.path.as_str(), name.span))
    else {
        return Vec::new();
    };
    let mut views = vec![symbol.clone()];
    loop {
        let before = views.len();
        for binding in file.ast.bindings.iter().filter(|binding| {
            binding.immutable
                && binding.shape == solid_facts::ast::BindingShape::Identifier
                && binding.names.len() == 1
                && function.body.contains(binding.declaration)
        }) {
            let Some(name) = binding.names.first() else {
                continue;
            };
            let Some(bound) = lookup.entities().at(file.path.as_str(), name.span) else {
                continue;
            };
            if views.contains(bound) {
                continue;
            }
            let Some(initializer) = binding.initializer else {
                continue;
            };
            let initializer = file.ast.peel_ts_sugar_span(initializer);
            let source = if let Some(call) = file.ast.call_at(initializer) {
                let omits = lookup
                    .primitive_at_call(file, call.span)
                    .is_some_and(|primitive| lookup.dialect.splits_props(primitive));
                let first = call.arguments.first().filter(|argument| !argument.spread);
                match (omits, first) {
                    (true, Some(argument)) => file.ast.peel_ts_sugar_span(argument.span),
                    _ => continue,
                }
            } else {
                initializer
            };
            if symbol_at_reference(file, source, lookup).is_some_and(|value| views.contains(&value))
            {
                views.push(bound.clone());
            }
        }
        if views.len() == before {
            return views;
        }
    }
}

/// ADR 0207: whether the package component `element` renders runs its prop
/// `prop` only after the render, by its accepted contract.
///
/// The contract states an `event-handler-props` item on argument 0 itself,
/// executed `deferred` (an external event or a queue, never the render's
/// stack); the item is exhaustive for every un-namespaced `on…` member, so
/// `prop` must be one. Every atom of the item's guard must be proven at this
/// element: the one atom read is a `plain` kind at a top-level key, proven
/// when no element on the render chain can put that key in the props object
/// ([`key_absent`]). Any other atom proves nothing.
fn package_prop_runs_only_deferred(
    file: &solid_facts::FileFacts,
    element: &solid_facts::ast::JsxElementFact,
    enclosing: Option<&solid_facts::ast::FunctionFact>,
    prop: &str,
    lookup: &SemanticLookup<'_>,
    chain: &[RenderHop],
) -> bool {
    use crate::contract_semantics::{GuardAtom, MemberClass, ValueKind};
    if !MemberClass::EventHandlerProps.contains(prop) {
        return false;
    }
    let Some(symbol) = lookup.callee_symbol(file, element.name.span) else {
        return false;
    };
    let Some(claims) = lookup.contract_event_handler_props(symbol) else {
        return false;
    };
    let hop = render_hop(file, element, enclosing, lookup);
    claims.iter().any(|claim| {
        claim.parameter == 0
            && claim.path.is_empty()
            && claim.execution == "deferred"
            && claim.guard.iter().all(|atom| match atom {
                GuardAtom::ValueKind {
                    argument: 0,
                    path,
                    kind: ValueKind::Plain,
                } => match path.as_slice() {
                    [key] => key_absent(key, &hop, chain),
                    _ => false,
                },
                _ => false,
            })
    })
}

/// The span of the call `reference` is the props argument of, when that call
/// is a props merge or split the dialect names: a view whose properties read
/// through to the props object and invoke none of them.
fn props_view_call(
    file: &solid_facts::FileFacts,
    reference: Span,
    lookup: &SemanticLookup<'_>,
) -> Option<Span> {
    let (call, _) = file
        .ast
        .arguments_containing(reference)
        .find(|(call, index)| {
            let argument = &call.arguments[*index];
            !argument.spread && file.ast.peel_ts_sugar_span(argument.span) == reference
        })?;
    let primitive = lookup.primitive_at_call(file, call.span)?;
    (lookup.dialect.merges_props_reactivity(primitive) || lookup.dialect.splits_props(primitive))
        .then_some(call.span)
}

/// The `const` identifier binding initialized by exactly the expression at
/// `value`, with its symbol.
fn const_binding_of(
    file: &solid_facts::FileFacts,
    value: Span,
    lookup: &SemanticLookup<'_>,
) -> Option<(SymbolId, Span)> {
    let binding = file.ast.bindings.iter().find(|binding| {
        binding.immutable
            && binding.shape == solid_facts::ast::BindingShape::Identifier
            && binding
                .initializer
                .is_some_and(|initializer| file.ast.peel_ts_sugar_span(initializer) == value)
    })?;
    let name = binding.names.first()?;
    let symbol = lookup.entities().at(file.path.as_str(), name.span)?;
    Some((symbol.clone(), name.span))
}

/// Whether the function written at `function` is the value of a JSX attribute.
fn jsx_attribute_value_function(file: &solid_facts::FileFacts, function: Span) -> bool {
    file.ast.jsx_containing(function).any(|element| {
        element.attributes.iter().any(|attribute| {
            attribute.value_kind == solid_facts::ast::JsxAttributeValueKind::Expression
                && attribute
                    .expression
                    .is_some_and(|value| file.ast.peel_ts_sugar_span(value) == function)
        })
    })
}

/// Whether `literal` is the argument of a call that is not a primitive and is
/// not proven to invoke that argument during the call: an unresolved callee, a
/// project function that keeps or forwards it, a standard-library call not
/// modelled as running it inline, a callee with no accepted `inline` row. A
/// primitive's callback is classified by the dialect arms, so it is not asked
/// here; a literal that is no call's argument is not either.
fn call_argument_invocation_unproven(
    file: &solid_facts::FileFacts,
    literal: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
) -> bool {
    use crate::runtime_semantics::RuntimeArgumentBehavior;
    let Some((call, index)) = file
        .ast
        .arguments_containing(literal.span)
        .filter(|(call, index)| {
            file.ast
                .functions_within(call.arguments[*index].span)
                .filter(|function| function.span.contains(literal.span))
                .max_by_key(|function| function.span.end - function.span.start)
                .is_some_and(|outer| outer.span == literal.span)
        })
        .min_by_key(|(call, _)| call.span.end - call.span.start)
    else {
        return false;
    };
    if lookup.primitive_at_call(file, call.span).is_some() {
        return false;
    }
    let argument = &call.arguments[index];
    let direct = !argument.spread && file.ast.peel_ts_sugar_span(argument.span) == literal.span;
    if let Some((callee_file, callee)) = lookup
        .callee_symbol(file, call.callee)
        .and_then(|symbol| lookup.function_for_symbol(symbol))
        .or_else(|| lookup.function_called_at(file.path.as_str(), call.callee))
    {
        return !(direct && invokes_parameter_during_call(callee_file, callee, index, lookup));
    }
    if let Some(resolved) = lookup
        .resolved_callee_call(file, call.callee)
        .filter(|resolved| {
            resolved
                .declaration
                .as_ref()
                .is_some_and(|declaration| declaration.standard_library)
        })
    {
        let callability = lookup
            .entity_at(file.path.as_str(), argument.span)
            .and_then(|entity| entity.callability);
        return !(direct
            && matches!(
                crate::runtime_semantics::argument_behavior(resolved, callability, index),
                Some(RuntimeArgumentBehavior::InlineCallback)
            ));
    }
    !(direct
        && lookup
            .callee_symbol(file, call.callee)
            .and_then(|symbol| lookup.contract_callbacks(symbol))
            .is_some_and(|rows| {
                rows.iter()
                    .any(|row| row.parameter == index && row.execution == "inline")
            }))
}

/// Whether `span` sits in a function literal that is merely *stored* where it
/// is written -- an object property, a getter, an array element, a returned
/// closure -- or in a function nested inside one. Constructing such a literal
/// runs nothing, so no scope around the construction runs what is inside.
///
/// A literal handed to a call, invoked in place, or written as a control-flow
/// component's render callback is not stored: those have their own proofs.
fn enclosed_by_stored_function(
    file: &solid_facts::FileFacts,
    span: Span,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
) -> bool {
    file.ast.functions_body_containing(span).any(|literal| {
        stored_literal_invocation_unproven(file, literal, lookup)
            && !file
                .ast
                .arguments_containing(literal.span)
                .any(|(call, index)| {
                    let argument = &call.arguments[index];
                    !argument.spread && file.ast.peel_ts_sugar_span(argument.span) == literal.span
                })
            && !file
                .ast
                .calls
                .iter()
                .any(|call| file.ast.peel_ts_sugar_span(call.callee) == literal.span)
            && control_flow_execution_role(
                file,
                literal.body,
                entities,
                symbol_names,
                lookup.dialect,
            )
            .is_none()
    })
}

/// Whether `span` is written in an async function after an `await` of that
/// same function, by position. Only the function's own awaits count: one in a
/// nested closure suspends that closure.
fn follows_await_in_async_function(file: &solid_facts::FileFacts, span: Span) -> bool {
    // A call's arguments are evaluated before the call runs, so an await in
    // the written call's own argument list (`setX(await load())`) also
    // precedes it.
    let arguments = file
        .ast
        .calls
        .iter()
        .filter(|call| call.callee == span || call.span == span)
        .map(|call| Span::new(call.callee.end, call.span.end))
        .min_by_key(|arguments| arguments.end - arguments.start);
    containing_ast_function(&file.ast, span).is_some_and(|function| {
        function.r#async
            && file.ast.awaits.iter().any(|awaited| {
                (awaited.end <= span.start
                    || arguments.is_some_and(|arguments| arguments.contains(*awaited)))
                    && function.body.contains(*awaited)
                    && containing_ast_function(&file.ast, *awaited)
                        .is_some_and(|owner| owner.span == function.span)
            })
    })
}

/// Whether code at `span`, classified [`ExecutionRole::UntrackedRendering`],
/// provably runs during the body it is written in.
///
/// Walks outward from the innermost function containing `span`. A
/// (possible) component is the body itself: the rendering role is its own.
/// Any other function literal runs during the body only as an argument whose
/// invocation during the call is established:
///
/// - a primitive's callback, which the dialect arms of
///   [`semantic_execution_role_within`] already classified;
/// - a literal handed to a project function that invokes that parameter
///   during the call, a standard-library inline callback, or a package
///   callback an accepted contract states inline -- exactly the invocations
///   [`callee_callback_timing`] proves for a read.
///
/// An IIFE runs where it is written, and a control-flow component's render
/// callback runs while the component's children render, so both continue the
/// walk or answer `true`. Any other literal that is no call's argument --
/// stored in a binding, returned from another callback
/// (`keep(() => () => setCount(1))`), a JSX attribute -- and one handed to a
/// function not proven to invoke it during the
/// call (`later(() => setCount(1))`, which keeps it for a timer) prove
/// nothing: `false`. Then the write takes the role its invocation sites
/// prove, and with none it is unclassified, which reports nothing
/// (`docs/rules/reactive-write-in-owned-scope.md`: an unproven write position
/// is never a violation).
pub(super) fn nested_literal_runs_during_body(
    file: &solid_facts::FileFacts,
    span: Span,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
) -> bool {
    // A default-parameter initializer is outside its function's body but
    // runs only when that function is called, never where it is declared.
    if file.ast.functions.iter().any(|function| {
        function.span.contains(span)
            && !function.body.contains(span)
            && span.end <= function.body.start
            && lookup.function_component_status(file, function) == ComponentStatus::No
    }) {
        return false;
    }
    let mut span = span;
    loop {
        let Some(literal) = containing_ast_function(&file.ast, span) else {
            return true;
        };
        if lookup.function_component_status(file, literal) != ComponentStatus::No {
            return true;
        }
        let Some((call, index)) = file
            .ast
            .arguments_containing(literal.span)
            .filter(|(call, index)| {
                file.ast
                    .functions_within(call.arguments[*index].span)
                    .filter(|function| function.span.contains(literal.span))
                    .max_by_key(|function| function.span.end - function.span.start)
                    .is_some_and(|outer| outer.span == literal.span)
            })
            .min_by_key(|(call, _)| call.span.end - call.span.start)
        else {
            // Two literals that are no argument still run in place: an IIFE,
            // invoked where it is written, and a control-flow component's
            // render callback, which the component runs while its children
            // render (`control_flow_execution_role` proved that role).
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
            return control_flow_execution_role(file, span, entities, symbol_names, lookup.dialect)
                .is_some_and(ExecutionRole::reports_disallowed_write);
        };
        match lookup.primitive_at_call(file, call.span) {
            // A primitive whose dialect models no callback at this position
            // (a cleanup registration, say) does not say it runs the
            // function during the call: it keeps it for later, or never.
            Some(primitive)
                if callback_execution_at_call(file, call, primitive, index, lookup).is_none() =>
            {
                return false;
            }
            Some(_) => {}
            None => {
                if callee_callback_timing(file, span, ExecutionRole::UntrackedRendering, lookup) {
                    return false;
                }
            }
        }
        // The call contains the literal strictly, so the walk terminates.
        span = call.span;
    }
}

/// Whether `span` sits directly in the body of a function that is, or may be,
/// a component, on a dialect whose component body runs under a root
/// ([`Dialect::component_body_runs_under_root`]).
///
/// A possible component is included: the only premise that made a write there
/// a violation is that it runs as a component body, and under this guard a
/// component body is legal.
fn runs_directly_in_component_root(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    lookup.dialect.component_body_runs_under_root()
        && containing_ast_function(&file.ast, span).is_some_and(|function| {
            lookup.function_component_status(file, function) != ComponentStatus::No
        })
}

/// The write-legality answer a same-file function passed **by name** takes
/// from the calls that pass it, where an inline callback in the same position
/// would take a [`WriteRegionAdjustment`]: `createRoot(init)` runs `init` as
/// the root body, and `untrack(write)` or `flush(write)` runs `write` exactly
/// as legally as at the call.
///
/// The function is resolved by symbol: the argument must be a bare identifier
/// whose symbol is the function's own binding (`entities.at` on a call span
/// would answer with the callee instead). Only a rejecting site answers,
/// because the function may also run from positions this does not look at --
/// a direct call, another callback slot -- which keep their own
/// classification; one invocation that throws is a violation, while one that
/// does not proves nothing about the others.
#[allow(clippy::too_many_arguments)]
fn named_callback_write_role(
    file: &solid_facts::FileFacts,
    span: Span,
    allowed: &[Span],
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
    root_body: RootBodyGuard,
    visiting: &mut HashSet<(String, Span)>,
) -> Option<ExecutionRole> {
    let function = containing_ast_function(&file.ast, span)?;
    let symbol = function_symbol(file, function, entities)?;
    let key = (file.path.to_string(), function.span);
    if !visiting.insert(key.clone()) {
        return None;
    }
    let mut rejected = None;
    'calls: for call in &file.ast.calls {
        for (index, argument) in call.arguments.iter().enumerate() {
            if argument.value != solid_facts::ast::ArgumentValueKind::Identifier
                || entities.at(file.path.as_str(), argument.span) != Some(symbol)
            {
                continue;
            }
            let Some(primitive) = lookup.primitive_at_call(file, call.span) else {
                continue;
            };
            let execution = callback_execution_at_call(file, call, primitive, index, lookup);
            let site = if lookup
                .dialect
                .callback_runs_in_created_root(primitive, index)
                && execution == Some(Execution::Inline)
            {
                (root_body == RootBodyGuard::Rejects).then_some(ExecutionRole::UntrackedRendering)
            } else if lookup
                .dialect
                .callback_preserves_owner_write_context(primitive)
                && execution.is_some()
            {
                let at_call = semantic_write_execution_role_within(
                    file,
                    call.span,
                    allowed,
                    entities,
                    symbol_names,
                    lookup,
                    root_body,
                    visiting,
                );
                at_call.reports_disallowed_write().then_some(at_call)
            } else {
                None
            };
            if site.is_some() {
                rejected = site;
                break 'calls;
            }
        }
    }
    visiting.remove(&key);
    rejected
}

/// `classifying` is the stack of spans whose role is currently being derived
/// from their own invocation sites. A returned adapter invoked inside its own
/// factory callback — `const a = on(() => a(), fn)`, or two adapters invoked
/// in each other's callbacks — makes that derivation cyclic; a site already on
/// the stack supplies no independent execution context and is skipped instead
/// of re-entered.
fn semantic_execution_role_within(
    file: &solid_facts::FileFacts,
    span: Span,
    allowed: &[Span],
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
    classifying: &mut HashSet<(String, Span)>,
) -> ExecutionRole {
    // Before every semantic path, because the semantic paths answer a question
    // that no longer applies. They classify *how* code executes — the callback
    // that will invoke it, the primitive whose argument it is, the owner it
    // answers to — and a discarded region is the compiler saying the code is
    // not there to execute. Left below them, a read inside a deleted value
    // would take its role from a dialect-proven `untrack()` or effect-apply
    // position and be reported as a proven untracked read, or from a deferred
    // position and be silently *certified*: a positive claim about dead code
    // either way.
    if discarded_region_contains(file, span) {
        return ExecutionRole::DiscardedRendering;
    }
    // Also ahead of every semantic path, for the mirror-image reason: code
    // inside a callback that runs on reads of a returned view (rc.9's `omit`
    // predicate) executes wherever the view is read, which none of the paths
    // below follow. Each of them would place it -- most often in the
    // enclosing component body, the one place it is least likely to run --
    // so the role is left unknown and projection claims nothing about it.
    if result_access_callback_contains(file, span, lookup) {
        return ExecutionRole::Unknown;
    }
    // A function argument the primitive wraps in a memo of its own (2.0's
    // `merge`) runs inside that computation, not in the caller's body.
    if file.ast.functions_body_containing(span).any(|function| {
        file.ast
            .arguments_containing(function.span)
            .any(|(call, index)| {
                file.ast.peel_ts_sugar_span(call.arguments[index].span) == function.span
                    && call_primitive_name(file, call, entities, symbol_names, lookup.dialect)
                        .as_ref()
                        .and_then(PrimitiveName::primitive)
                        .is_some_and(|primitive| {
                            lookup.dialect.wraps_function_arguments_in_memo(primitive)
                        })
            })
    }) {
        return ExecutionRole::Unknown;
    }
    if let Some(role) = context_provider_value_role(file, span, lookup) {
        return role;
    }
    if assigned_member_function_contains(file, span, entities) {
        return ExecutionRole::DeferredCallback;
    }
    if let Some(role) = named_callback_execution_role(file, span, lookup) {
        return role;
    }
    if let Some(role) = returned_callback_execution_role(file, span, lookup, classifying) {
        return role;
    }
    if let Some(role) = returned_factory_callback_execution_role(file, span, lookup, classifying) {
        return role;
    }
    if let Some(role) = contract_returned_invoker_callback_role(file, span, lookup, classifying) {
        return role;
    }
    if let Some(role) = inline_callback_execution_role(file, span, allowed, lookup, classifying) {
        return role;
    }
    let dialect = lookup.dialect;
    if file.ast.arguments_containing(span).any(|(call, index)| {
        call_primitive_name(file, call, entities, symbol_names, dialect)
            .as_ref()
            .and_then(PrimitiveName::primitive)
            .and_then(|primitive| effect_apply_argument(dialect, primitive, call.arguments.len()))
            == Some(index)
            && direct_callback_contains(file, call.arguments[index].span, span)
    }) {
        return ExecutionRole::EffectApply;
    }
    if file.ast.arguments_containing(span).any(|(call, index)| {
        call_primitive_name(file, call, entities, symbol_names, dialect)
            .as_ref()
            .and_then(PrimitiveName::primitive)
            .is_some_and(|primitive| {
                callback_execution_at_call(file, call, primitive, index, lookup).is_some()
                    && dialect.reports_untracked_reads_at(primitive, index, call.arguments.len())
                    && direct_callback_contains(file, call.arguments[index].span, span)
            })
    }) {
        return ExecutionRole::UntrackedCallback;
    }
    // Ahead of the allowed regions: an accepted contract that leaves its
    // `callbacks` enumeration open makes every argument of the call one of
    // unproven timing, but a slot it states as a guaranteed owned computation
    // is proven to run during the call whatever else the export does with it.
    if let Some(role) = contract_owned_computation_callback_role(file, span, lookup) {
        return role;
    }
    if let Some(role) = forwarded_tracked_compute_role(file, span, lookup) {
        return role;
    }
    if allowed.iter().any(|region| region.contains(span)) {
        return ExecutionRole::DeferredCallback;
    }
    if let Some(role) = control_flow_execution_role(file, span, entities, symbol_names, dialect) {
        return role;
    }
    {
        let mut regions = file
            .compiler
            .tracked_regions
            .iter()
            .filter(|region| region.span.contains(span))
            .peekable();
        if regions.peek().is_some() {
            // A region is tracked for its *expression*. A function literal
            // that is a JSX attribute's value inside it (an event handler, a
            // callback prop) is handed to the element and runs when something
            // calls it, not in the region's tracking pass; JSX written inside
            // such a function is a region of its own, which still tracks.
            return if regions
                .any(|region| !attribute_function_within(file, region.span, span, lookup))
            {
                ExecutionRole::TrackedJsx
            } else {
                ExecutionRole::Unknown
            };
        }
    }
    let mut tracked_callbacks = file.ast.arguments_containing(span).filter(|(call, index)| {
        matches!(
            call.arguments[*index].value,
            solid_facts::ast::ArgumentValueKind::Identifier
                | solid_facts::ast::ArgumentValueKind::Function
                | solid_facts::ast::ArgumentValueKind::AsyncFunction
        ) && call_primitive_name(file, call, entities, symbol_names, dialect)
            .as_ref()
            .and_then(PrimitiveName::primitive)
            .is_some_and(|primitive| {
                callback_execution_at_call(file, call, primitive, *index, lookup).is_some()
                    && dialect
                        .callback_semantics_at(primitive, *index, call.arguments.len())
                        .tracks_reads
            })
    });
    if let Some((call, index)) = tracked_callbacks.next() {
        // As for a tracked JSX region: a function handed on inside the
        // callback to a call not proven to invoke it there, or written as an
        // attribute value, runs when something calls it, not in the
        // callback's tracking pass (`client.subscribe(() => setX(..))`).
        return if attribute_function_within(file, call.arguments[index].span, span, lookup) {
            ExecutionRole::Unknown
        } else {
            ExecutionRole::TrackedJsx
        };
    }
    // Ahead of the compiler role and the lexical fallbacks, which place a
    // callback prop's body where the JSX is written.
    if let Some(role) = forwarded_event_prop_role(file, span, lookup) {
        return role;
    }
    let compiler_role = source_execution_role(file, span, allowed);
    if compiler_role != ExecutionRole::Unknown {
        return compiler_role;
    }
    // Ahead of the lexical fallbacks below, which place code by where it is
    // written: a callback a fresh-stack host scheduler runs is written in the
    // component body but does not execute there.
    if let Some(role) = fresh_stack_callback_role(file, span, lookup) {
        return role;
    }
    if lookup.inside_component(file, span) {
        return ExecutionRole::UntrackedRendering;
    }
    if lookup.inside_possible_component(file, span) {
        // Select the component execution branch so the relevant rule is
        // projected, while LocalAccess marks the read uncertifiable. Leaving
        // this Unknown would silently certify the ordinary-helper branch.
        return ExecutionRole::UntrackedRendering;
    }
    // Module initialization is an AST-proven one-shot execution context. It
    // is not a compiler-fact gap: no reactive owner or subscriber can be
    // active before a containing function is invoked.
    if !file.ast.any_function_body_containing(span) {
        return ExecutionRole::ModuleInitialization;
    }
    ExecutionRole::Unknown
}

/// ADR 0200: code directly in a function literal handed, as the whole
/// argument, to a project function that only ever invokes that parameter as
/// the tracked compute of a computation, takes the tracked role.
///
/// The project function `wrapper` must be synchronous, its parameter at the
/// slot one identifier, and every reference to that parameter a direct call
/// `p()` written directly in one function literal `compute`. `compute` must be
/// written directly in `wrapper`'s body, as the whole argument at a slot that
/// tracks the reads of the callback it runs: an accepted contract's
/// guaranteed tracked-compute slot (ADR 0183) or a primitive's tracked
/// callback (`createMemo`'s compute). Then the literal runs only inside that
/// tracked compute, as it would written there itself. A parameter referenced
/// any other way can run the literal somewhere else as well, so it proves
/// nothing.
fn forwarded_tracked_compute_role(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> Option<ExecutionRole> {
    let literal = containing_ast_function(&file.ast, span)?;
    let (call, index) = file.ast.arguments_containing(span).find(|(call, index)| {
        let argument = &call.arguments[*index];
        !argument.spread
            && file.ast.peel_ts_sugar_span(argument.span) == literal.span
            && lookup.primitive_at_call(file, call.span).is_none()
    })?;
    let (wrapper_file, wrapper) = lookup
        .callee_symbol(file, call.callee)
        .and_then(|symbol| lookup.function_for_symbol(symbol))
        .or_else(|| lookup.function_called_at(file.path.as_str(), call.callee))?;
    if wrapper.r#async || wrapper.generator {
        return None;
    }
    let parameter = wrapper.parameters.get(index)?;
    if parameter.shape != solid_facts::ast::BindingShape::Identifier
        || parameter.initializer.is_some()
    {
        return None;
    }
    let name = parameter.names.first()?;
    let symbol = lookup
        .entities()
        .at(wrapper_file.path.as_str(), name.span)?;
    let mut compute: Option<Span> = None;
    for reference in lookup.symbol_references(symbol.as_str()) {
        if reference.path.as_ref() != wrapper_file.path.as_str() {
            return None;
        }
        let (Ok(start), Ok(end)) = (
            u32::try_from(reference.start_byte),
            u32::try_from(reference.end_byte),
        ) else {
            return None;
        };
        let reference = Span::new(start, end);
        if reference == name.span {
            continue;
        }
        // A direct call `p()`, written directly in one function literal.
        let invoked = wrapper_file
            .ast
            .calls
            .iter()
            .any(|inner| wrapper_file.ast.peel_ts_sugar_span(inner.callee) == reference);
        let owner = containing_ast_function(&wrapper_file.ast, reference)?;
        if !invoked || owner.span == wrapper.span || compute.is_some_and(|seen| seen != owner.span)
        {
            return None;
        }
        compute = Some(owner.span);
    }
    let compute = compute?;
    // `compute` is the whole argument of a call written directly in the
    // wrapper's body, at a slot that tracks its callback's reads.
    let (outer, slot) = wrapper_file
        .ast
        .arguments_containing(compute)
        .find(|(outer, slot)| {
            let argument = &outer.arguments[*slot];
            !argument.spread && wrapper_file.ast.peel_ts_sugar_span(argument.span) == compute
        })?;
    if containing_ast_function(&wrapper_file.ast, outer.span)
        .is_none_or(|function| function.span != wrapper.span)
    {
        return None;
    }
    let tracked = match lookup.primitive_at_call(wrapper_file, outer.span) {
        Some(primitive) => {
            callback_execution_at_call(wrapper_file, outer, primitive, slot, lookup).is_some()
                && lookup
                    .dialect
                    .callback_semantics_at(primitive, slot, outer.arguments.len())
                    .tracks_reads
        }
        None => lookup
            .callee_symbol(wrapper_file, outer.callee)
            .and_then(|symbol| lookup.contract_guaranteed_callback_parameters(symbol))
            .is_some_and(|parameters| parameters.contains(&slot)),
    };
    if !tracked {
        return None;
    }
    Some(
        if attribute_function_within(file, call.arguments[index].span, span, lookup) {
            ExecutionRole::Unknown
        } else {
            ExecutionRole::TrackedJsx
        },
    )
}

/// The context of an accessor read directly in a synchronous callback
/// literal whose accepted, closed callback domain describes every use as a
/// tracked invocation under a created owner. The callback may run zero times
/// or on resource access: neither changes the context of a read when it runs.
/// This answer must not establish execution, write legality, or ownership.
pub(super) fn contract_tracked_accessor_read_role(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> Option<ExecutionRole> {
    if discarded_region_contains(file, span) {
        return None;
    }
    let literal = containing_ast_function(&file.ast, span)?;
    // Suspension, generator resumption, named self-escapes and parameter
    // defaults need their own execution proof. Keep this slice to arrows.
    if literal.r#async
        || literal.generator
        || literal.kind != solid_facts::ast::FunctionKind::Arrow
        || !literal.body.contains(span)
    {
        return None;
    }
    file.ast.arguments_containing(span).find(|(call, index)| {
        let argument = &call.arguments[*index];
        !argument.spread
            && file.ast.peel_ts_sugar_span(argument.span) == literal.span
            && direct_callback_contains(file, argument.span, span)
            && lookup.primitive_at_call(file, call.span).is_none()
            && lookup
                .callee_symbol(file, call.callee)
                .is_some_and(|symbol| lookup.contract_callback_reads_are_tracked(symbol, *index))
            && !attribute_function_within(file, argument.span, span, lookup)
    })?;
    Some(ExecutionRole::TrackedJsx)
}

/// ADR 0183: code directly in a function literal handed, as the whole
/// argument, to an accepted contract export that states the slot is invoked
/// on every call, during it, as the tracked compute of an owned computation
/// the export creates ([`crate::ContractExport::guaranteed_callback_parameters`]).
///
/// That compute runs under the computation's own children-capable owner
/// wherever the export is called, exactly as a `createMemo` compute does, so
/// the role is the tracked callback's. A function nested in the literal, an
/// element of an array or object written at the slot, a spread, and a function
/// passed by name are not answered here. As for a primitive's tracked
/// callback, a function handed on inside the literal, or written as an
/// attribute value, is not proven to run in the compute's pass.
fn contract_owned_computation_callback_role(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> Option<ExecutionRole> {
    let literal = containing_ast_function(&file.ast, span)?;
    let (call, index) = file.ast.arguments_containing(span).find(|(call, index)| {
        let argument = &call.arguments[*index];
        !argument.spread
            && file.ast.peel_ts_sugar_span(argument.span) == literal.span
            && direct_callback_contains(file, argument.span, span)
            && lookup.primitive_at_call(file, call.span).is_none()
            && lookup
                .callee_symbol(file, call.callee)
                .and_then(|symbol| lookup.contract_guaranteed_callback_parameters(symbol))
                .is_some_and(|parameters| parameters.contains(index))
    })?;
    Some(
        if attribute_function_within(file, call.arguments[index].span, span, lookup) {
            ExecutionRole::Unknown
        } else {
            ExecutionRole::TrackedJsx
        },
    )
}

/// [`ExecutionRole::DeferredCallback`] for code in a callback that a reviewed
/// fresh-stack host scheduler runs (`runtime_semantics::FRESH_STACK_SCHEDULERS`:
/// `setTimeout`, `queueMicrotask`, `Promise.then`, `requestAnimationFrame`, the
/// observer constructors, ...), wherever the scheduling call is written.
///
/// The scheduler invokes the callback from a task or microtask queue on an
/// otherwise empty execution-context stack, so none of the scheduling code's
/// dynamic state is current there: no listener (the read subscribes to nothing
/// and nothing re-runs the callback), no owner, and no strict-read label.
/// Probed on 2.0.0-rc.3 and rc.9, dev and prod, from a component body: a read
/// in the body raises `STRICT_READ_UNTRACKED` and the same read in each such
/// callback does not; `getObserver()` and `getOwner()` are `null` there; a
/// write or an action there does not raise `REACTIVE_WRITE_IN_OWNED_SCOPE` or
/// `ACTION_CALLED_IN_OWNED_SCOPE`; a pending async read there does not raise
/// `PENDING_ASYNC_UNTRACKED_READ` -- a re-ask serves the settled value, and a
/// source that never settled throws a plain `NotReadyError`, exactly as it does
/// in a listener dispatched after mount. That is the role's meaning -- after
/// the scheduling call returns, outside its tracking pass -- the same one a
/// deferred primitive position gets.
///
/// The host fact comes from the compiler-selected standard-library declaration
/// through [`crate::runtime_semantics::argument_behavior`], never from
/// spelling, so a local `setTimeout` keeps its lexical role. Only a function
/// literal handed to the scheduler is its callback; `setTimeout(wrap(fn))`
/// hands over whatever `wrap` returns. Code in a callback that a
/// standard-library call runs inline (`list.forEach(fn)`) executes wherever
/// that call does, so the walk continues outward from the call. Any other
/// enclosing call ends the walk with no answer, leaving the arms after this
/// one to decide.
///
/// The other deferring host callbacks (`addEventListener`, `bind`'s bound
/// arguments, `PromiseLike.then`, Geolocation) are deliberately not answered:
/// each can run on its invoker's stack -- probed, a listener dispatched or a
/// bound function called in the component body, or a synchronous thenable,
/// runs inside the strict-read window and raises `STRICT_READ_UNTRACKED`. They
/// keep the lexical role, and [`host_callback_timing`] makes a read there
/// uncertifiable rather than a proven violation.
fn fresh_stack_callback_role(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> Option<ExecutionRole> {
    use crate::runtime_semantics::RuntimeArgumentBehavior;
    let mut span = span;
    loop {
        let (call, index) = file.ast.arguments_containing(span).find(|(call, index)| {
            matches!(
                call.arguments[*index].value,
                solid_facts::ast::ArgumentValueKind::Function
                    | solid_facts::ast::ArgumentValueKind::AsyncFunction
            ) && direct_callback_contains(file, call.arguments[*index].span, span)
        })?;
        let resolved = lookup.resolved_callee_call(file, call.callee)?;
        let callability = lookup
            .entity_at(file.path.as_str(), call.arguments[index].span)
            .and_then(|entity| entity.callability);
        match crate::runtime_semantics::argument_behavior(resolved, callability, index)? {
            RuntimeArgumentBehavior::FreshStackCallback => {
                return Some(ExecutionRole::DeferredCallback);
            }
            // The call contains `span` strictly, so the walk terminates.
            RuntimeArgumentBehavior::InlineCallback => span = call.span,
            RuntimeArgumentBehavior::DeferredCallback
            | RuntimeArgumentBehavior::RetainedValue
            | RuntimeArgumentBehavior::ValueOnly => return None,
        }
    }
}

/// Whether a read classified in `execution` is **uncertifiable** because it sits
/// in a callback a host API retains and may invoke on its invoker's stack
/// (`runtime_semantics::runs_on_invoker_stack`: an `addEventListener`
/// listener, a `bind` bound argument, a `PromiseLike.then` callback, a
/// Geolocation callback).
///
/// Such a callback is written in the component body but runs wherever the host
/// invokes it: inside the body's strict-read window when the body itself hands
/// control back synchronously, after it otherwise. Probed on 2.0.0-rc.3 and
/// rc.9, dev and prod, from a component body: a listener dispatched with
/// `dispatchEvent` or clicked with `el.click()` in the body, a bound function
/// called in the body, and a synchronous thenable all raise
/// `STRICT_READ_UNTRACKED` for a signal read and `PENDING_ASYNC_UNTRACKED_READ`
/// for a pending async read; the same callbacks dispatched after mount, or run
/// by a real promise, raise neither (a never-settled source throws a plain
/// `NotReadyError`). Chrome ran neither Geolocation callback in the body, from
/// an active or a detached document, but the specification's synchronous
/// "call back with error" path exists, so it is treated the same way.
///
/// Registration alone does not decide which case holds, and proving a
/// synchronous invocation in the window needs the dispatch to follow the
/// registration on every path, on the same target, with a matching event type
/// and no removal in between -- facts the syntax facts here do not carry. So
/// the answer is uncertifiable, never a proven violation, for every such read
/// the lexical fallback places in a role that reports untracked reads. A role
/// that reports nothing is left alone: this withholds a claim, it never makes
/// one.
///
/// The walk is [`fresh_stack_callback_role`]'s: the innermost function
/// literal handed directly to a call, continuing outward through
/// standard-library inline callbacks (`el.addEventListener("x", () =>
/// list.forEach(() => read()))`), and ending with no answer at any other call.
pub(crate) fn host_callback_timing(
    file: &solid_facts::FileFacts,
    span: Span,
    execution: ExecutionRole,
    lookup: &SemanticLookup<'_>,
) -> bool {
    use crate::runtime_semantics::RuntimeArgumentBehavior;
    if !execution.reports_untracked_read() {
        return false;
    }
    let mut span = span;
    loop {
        let Some((call, index)) = file.ast.arguments_containing(span).find(|(call, index)| {
            matches!(
                call.arguments[*index].value,
                solid_facts::ast::ArgumentValueKind::Function
                    | solid_facts::ast::ArgumentValueKind::AsyncFunction
            ) && direct_callback_contains(file, call.arguments[*index].span, span)
        }) else {
            return false;
        };
        let Some(resolved) = lookup.resolved_callee_call(file, call.callee) else {
            return false;
        };
        let callability = lookup
            .entity_at(file.path.as_str(), call.arguments[index].span)
            .and_then(|entity| entity.callability);
        if crate::runtime_semantics::runs_on_invoker_stack(resolved, callability, index) {
            return true;
        }
        match crate::runtime_semantics::argument_behavior(resolved, callability, index) {
            // The call contains `span` strictly, so the walk terminates.
            Some(RuntimeArgumentBehavior::InlineCallback) => span = call.span,
            _ => return false,
        }
    }
}

/// Whether a read classified in `execution` is **uncertifiable** because it
/// sits in a function literal handed to a *project* function whose body is not
/// proven to invoke it during the call.
///
/// The lexical fallback of [`semantic_execution_role_within`] places code by
/// where it is written, so a literal written in a component body takes the
/// body's role. That is a proof only when the literal runs during the body.
/// Handed to a project function, it runs wherever that function runs it: in
/// its own body during the call, from a closure it returns or keeps, or never.
/// Probed on the audited 2.0.0-rc.9, dev and prod: `localChain([() => n()])`
/// whose returned invoker runs only from a click handler raises no
/// `STRICT_READ_UNTRACKED`, while the same chain invoked in the body, and a
/// helper that invokes its array's elements during the call, both warn. The
/// syntax does not tell these apart, so the read is a proof obligation, not a
/// proven untracked read.
///
/// The one invocation this proves is the direct one: the literal *is* the
/// argument at parameter `index`, the callee is a synchronous function whose
/// own body (not a nested closure) calls that parameter by symbol -- the named
/// callback case the interprocedural summary already covers. Then the literal
/// runs during the call, and the walk continues from the call, exactly as it
/// does through a standard-library inline callback (`items.forEach`). An
/// element of an array or object literal argument proves nothing: whether the
/// callee iterates it, or invokes it later, has no fact here.
///
/// A callee with no body in the project -- a package export seen through its
/// declarations, or a callee nothing resolves -- proves the invocation only
/// through an accepted contract's `inline` row for that parameter; without one
/// the read is a proof obligation too. Primitive and standard-library calls
/// are other arms' business (their dialect and runtime models). This withholds
/// a claim; it never makes one.
pub(crate) fn callee_callback_timing(
    file: &solid_facts::FileFacts,
    span: Span,
    execution: ExecutionRole,
    lookup: &SemanticLookup<'_>,
) -> bool {
    callee_callback_timing_within(file, span, execution, lookup, &mut HashSet::new())
}

/// [`callee_callback_timing`], carrying the invocation sites ADR 0152's arm
/// has already followed: a returned value invoked from inside the very
/// callback it runs has no timing of its own to contribute, and is read as
/// unproven rather than followed forever.
fn callee_callback_timing_within(
    file: &solid_facts::FileFacts,
    span: Span,
    execution: ExecutionRole,
    lookup: &SemanticLookup<'_>,
    following: &mut HashSet<(String, Span)>,
) -> bool {
    use crate::runtime_semantics::RuntimeArgumentBehavior;
    if !execution.reports_untracked_read() {
        return false;
    }
    let mut span = span;
    loop {
        // A default-parameter initializer is written in the enclosing body but
        // runs when its function is *called* without that argument, which no
        // lexical position proves happens during that body.
        if let Some(function) = parameter_default_owner(file, span)
            && lookup.function_component_status(file, function) == ComponentStatus::No
        {
            let Some(index) = function.parameters.iter().position(|parameter| {
                parameter
                    .initializer
                    .is_some_and(|value| value.contains(span))
            }) else {
                // A default inside a destructuring pattern has no argument slot
                // of its own.
                return true;
            };
            // Proven only for a named helper every call of which omits that
            // argument and runs during the body.
            return !(function.method_name.is_none()
                && named_helper_runs_during_body(
                    file,
                    function,
                    Some(index),
                    execution,
                    lookup,
                    following,
                ));
        }
        let Some(literal) = containing_ast_function(&file.ast, span) else {
            return false;
        };
        // The call the literal is written in: an argument in which the literal
        // is the outermost function (directly, or inside an array or object
        // literal), the innermost such call.
        let Some((call, index)) = file
            .ast
            .arguments_containing(literal.span)
            .filter(|(call, index)| {
                file.ast
                    .functions_within(call.arguments[*index].span)
                    .filter(|function| function.span.contains(literal.span))
                    .max_by_key(|function| function.span.end - function.span.start)
                    .is_some_and(|outer| outer.span == literal.span)
            })
            .min_by_key(|(call, _)| call.span.end - call.span.start)
        else {
            // Constructing a callback-valued JSX prop does not invoke it.
            // Its consumer may forward it to a DOM event, retain it, invoke
            // it during rendering, or never call it. The lexical component
            // context proves none of those invocation times. Require the
            // literal itself as the prop value: an IIFE inside that value
            // still executes while the value is evaluated.
            if file.ast.jsx_elements.iter().any(|element| {
                element.attributes.iter().any(|attribute| {
                    attribute
                        .expression
                        .is_some_and(|value| file.ast.peel_ts_sugar_span(value) == literal.span)
                })
            }) {
                // ADR 0204: unless the consumer is proven to call it while it
                // renders.
                return !prop_literal_invoked_during_render(file, literal.span, lookup);
            }
            // An IIFE runs where it is written, so its timing is its
            // surroundings'.
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
            // Only the lexical rendering role places code by where it is
            // written; a role a dialect arm proved for this position (an
            // effect's apply callback, an untrack callback) is not.
            return execution == ExecutionRole::UntrackedRendering
                && literal_invocation_unproven(file, literal, execution, lookup, following);
        };
        if lookup.primitive_at_call(file, call.span).is_some() {
            // A primitive's own callback argument is classified by the
            // dialect arms. A literal nested in a container argument
            // (`merge(props, { get class() { return cls(); } })`) is a value
            // the primitive stores, not a callback it invokes.
            return !(!call.arguments[index].spread
                && file.ast.peel_ts_sugar_span(call.arguments[index].span) == literal.span)
                && stored_literal_invocation_unproven(file, literal, lookup);
        }
        let argument = &call.arguments[index];
        // Resolved by symbol, members included (`bus.addEventListener` of a
        // project object resolves to its method's declaration).
        if let Some((callee_file, callee)) = lookup
            .callee_symbol(file, call.callee)
            .and_then(|symbol| lookup.function_for_symbol(symbol))
            .or_else(|| lookup.function_called_at(file.path.as_str(), call.callee))
        {
            let direct =
                !argument.spread && file.ast.peel_ts_sugar_span(argument.span) == literal.span;
            if !(direct && invokes_parameter_during_call(callee_file, callee, index, lookup)) {
                return true;
            }
            // The call contains the literal strictly, so the walk terminates.
            span = call.span;
            continue;
        }
        let resolved = lookup.resolved_callee_call(file, call.callee);
        if let Some(resolved) = resolved.filter(|resolved| {
            resolved
                .declaration
                .as_ref()
                .is_some_and(|declaration| declaration.standard_library)
        }) {
            let callability = lookup
                .entity_at(file.path.as_str(), argument.span)
                .and_then(|entity| entity.callability);
            match crate::runtime_semantics::argument_behavior(resolved, callability, index) {
                Some(RuntimeArgumentBehavior::InlineCallback)
                    if direct_callback_contains(file, argument.span, span) =>
                {
                    span = call.span;
                    continue;
                }
                // A standard-library call that is not modelled as running this
                // argument inline (it may keep it, schedule it, or hand it to
                // something that does) does not prove the callback runs during
                // the call.
                _ => return true,
            }
        }
        // ADR 0152: the contract's returned value runs the argument on its own
        // invoker's stack. The literal's role was read from the returned
        // value's proven invocations, so its timing is proven exactly when
        // each of them is; with none there is no role to have proven.
        if contract_returned_invoker_slot(file, span, lookup)
            .is_some_and(|invoker| invoker.span == call.span)
        {
            let sites = returned_callback_invocation_sites(file, call, lookup);
            return sites.is_empty()
                || sites.iter().any(|site| {
                    if site.inherited_execution.is_some() {
                        return false;
                    }
                    if !following.insert((site.path.clone(), site.span)) {
                        return true;
                    }
                    lookup
                        .files()
                        .iter()
                        .find(|candidate| candidate.path.as_str() == site.path)
                        .is_none_or(|use_file| {
                            callee_callback_timing_within(
                                use_file, site.span, execution, lookup, following,
                            )
                        })
                });
        }
        // A callee with no body in the project: a package export seen through
        // its declarations, or a callee nothing resolves. Only an accepted
        // contract row can say it invokes the argument during the call.
        let direct = !argument.spread && file.ast.peel_ts_sugar_span(argument.span) == literal.span;
        let contracted_inline = direct
            && lookup
                .callee_symbol(file, call.callee)
                .and_then(|symbol| lookup.contract_callbacks(symbol))
                .is_some_and(|rows| {
                    rows.iter()
                        .any(|row| row.parameter == index && row.execution == "inline")
                });
        if !contracted_inline {
            return true;
        }
        span = call.span;
    }
}

/// Whether a function literal that is no call's argument is *not proven* to run
/// where it is written -- the complement of every position that does:
///
/// - the component body itself, the rendering role's own proof;
/// - a control-flow component's children callback
///   ([`control_flow_execution_role`] proves it);
/// - a function bound to a name whose every reference is a call and whose every
///   call runs during the body ([`named_helper_runs_during_body`]).
///
/// Every other literal is *stored* (an object property, method or getter, an
/// array element, a returned closure, an anonymous value) or handed to a JSX
/// element that is not a rendering control-flow component. Constructing such a
/// literal runs nothing, and wherever it is later called -- from a handler,
/// from the object's reader, from a hook's returned closure, or never -- is not
/// something its lexical position proves.
fn literal_invocation_unproven(
    file: &solid_facts::FileFacts,
    literal: &solid_facts::ast::FunctionFact,
    execution: ExecutionRole,
    lookup: &SemanticLookup<'_>,
    following: &mut HashSet<(String, Span)>,
) -> bool {
    // An anonymous component (`export default function () {}`) is the body
    // itself, which is the rendering role's own proof.
    if lookup.function_component_status(file, literal) != ComponentStatus::No {
        return false;
    }
    if control_flow_execution_role(
        file,
        literal.body,
        lookup.entities(),
        lookup.symbol_names(),
        lookup.dialect,
    )
    .is_some()
    {
        return false;
    }
    // A function passed by name as a control-flow component's children callback
    // (`<For each={xs}>{renderItem}</For>`) is that callback.
    if named_callback_execution_role(file, literal.body, lookup)
        == Some(ExecutionRole::UntrackedRendering)
    {
        return false;
    }
    if literal.method_name.is_none() && function_binding_name(file, literal).is_some() {
        return !named_helper_runs_during_body(file, literal, None, execution, lookup, following);
    }
    true
}

/// Whether `literal` is a function that is *stored* where it is written: an
/// anonymous function expression or an object-literal method or getter, outside
/// every position that runs it ([`literal_invocation_unproven`]). A function
/// bound to a name is the interprocedural summary's business through its call
/// sites, and is left alone here.
fn stored_literal_invocation_unproven(
    file: &solid_facts::FileFacts,
    literal: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
) -> bool {
    (literal.method_name.is_some() || function_binding_name(file, literal).is_none())
        && literal_invocation_unproven(
            file,
            literal,
            ExecutionRole::UntrackedRendering,
            lookup,
            &mut HashSet::new(),
        )
}

/// Whether the function bound to a name runs only during the body it is written
/// in: it has at least one call, every reference to its binding is one of those
/// calls (no alias, no handler, no argument), and every call is itself proven
/// to run during the body.
///
/// A call written in a handler, a timer, a stored closure or a callback prop is
/// unproven by the same walk, so the helper is too; a call cycle is unproven.
///
/// With `default_parameter` the question is about that parameter's default
/// initializer, which runs only when a call leaves the argument out: every call
/// must then pass fewer arguments than the parameter's position, spread-free.
fn named_helper_runs_during_body(
    file: &solid_facts::FileFacts,
    literal: &solid_facts::ast::FunctionFact,
    default_parameter: Option<usize>,
    execution: ExecutionRole,
    lookup: &SemanticLookup<'_>,
    following: &mut HashSet<(String, Span)>,
) -> bool {
    let Some(binding) = function_binding_name(file, literal) else {
        return false;
    };
    let path = file.path.as_str();
    let Some(symbol) = lookup.entities().at(path, binding.span) else {
        return false;
    };
    let sites = lookup.function_call_sites(path, literal.span);
    if sites.is_empty() {
        return false;
    }
    let accounted: HashSet<(String, u32, u32)> = lookup
        .function_call_site_references(path, literal.span)
        .into_iter()
        .map(|(caller, callee)| (caller.path.to_string(), callee.start, callee.end))
        .collect();
    let escapes = lookup
        .symbol_references(symbol.as_str())
        .iter()
        .any(|reference| {
            let start = u32::try_from(reference.start_byte).unwrap_or(u32::MAX);
            let end = u32::try_from(reference.end_byte).unwrap_or(u32::MAX);
            let declaration = reference.path.as_ref() == path
                && start == binding.span.start
                && end == binding.span.end;
            !declaration && !accounted.contains(&(reference.path.to_string(), start, end))
        });
    if escapes {
        return false;
    }
    let key = (path.to_string(), literal.span);
    if !following.insert(key.clone()) {
        return false;
    }
    let proven = sites.iter().all(|(caller, callee)| {
        let omits_argument = default_parameter.is_none_or(|index| {
            caller
                .ast
                .calls
                .iter()
                .find(|call| caller.ast.peel_ts_sugar_span(call.callee) == *callee)
                .is_some_and(|call| {
                    call.arguments.len() <= index && !call.arguments.iter().any(|a| a.spread)
                })
        });
        // The call must itself be written where the rendering role holds: a
        // call in a tracked JSX attribute or child, a memo or a deferred
        // callback runs in that scope, not in the body's strict-read window.
        omits_argument
            && semantic_execution_role(
                caller,
                *callee,
                &allowed_callback_spans(caller, lookup),
                lookup.entities(),
                lookup.symbol_names(),
                lookup,
            ) == ExecutionRole::UntrackedRendering
            && !callee_callback_timing_within(caller, *callee, execution, lookup, following)
    });
    following.remove(&key);
    proven
}

/// The innermost function whose default-parameter initializer contains `span`.
///
/// A parameter initializer lies outside its function's *body*, so the
/// body-based containment queries place a read in it in the surrounding
/// function. Only functions nested in the span's own enclosing body (or any
/// function, at module level) can own it, which bounds the scan.
fn parameter_default_owner(
    file: &solid_facts::FileFacts,
    span: Span,
) -> Option<&solid_facts::ast::FunctionFact> {
    let region = containing_ast_function(&file.ast, span)
        .map_or_else(|| Span::new(0, u32::MAX), |function| function.body);
    file.ast
        .functions_within(region)
        .filter(|function| {
            function.span.contains(span)
                && !function.body.contains(span)
                && function.parameters.iter().any(|parameter| {
                    parameter
                        .initializer
                        .is_some_and(|value| value.contains(span))
                })
        })
        .min_by_key(|function| function.span.end - function.span.start)
}

/// Whether the synchronous project function `function` calls its parameter
/// `index` in its own body -- not in a closure it creates -- by the
/// parameter's symbol. Destructured, rest and missing parameters prove
/// nothing, and neither does an async function or a generator, whose body may
/// run after the call returns.
pub(crate) fn invokes_parameter_during_call(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    index: usize,
    lookup: &SemanticLookup<'_>,
) -> bool {
    if function.r#async || function.generator {
        return false;
    }
    let Some(parameter) = function.parameters.get(index) else {
        return false;
    };
    if parameter.shape != solid_facts::ast::BindingShape::Identifier {
        return false;
    }
    let Some(name) = parameter.names.first() else {
        return false;
    };
    let Some(symbol) = lookup.entities().at(file.path.as_str(), name.span) else {
        return false;
    };
    file.ast.calls.iter().any(|call| {
        containing_ast_function(&file.ast, call.span)
            .is_some_and(|owner| owner.span == function.span)
            && lookup.callee_symbol(file, call.callee) == Some(symbol.as_str())
    })
}

/// Whether `span` lies inside an argument the dialect says runs on reads of
/// the call's returned object ([`Dialect::callback_runs_on_result_access`]).
///
/// Containment, not direct-callback identity: every function written inside
/// such an argument runs, if at all, from inside it. An argument a project
/// wrapper forwards into such a slot
/// ([`SemanticLookup::result_access_forwarded_arguments`]) counts too.
fn result_access_callback_contains(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    if lookup
        .result_access_forwarded_arguments(file.path.as_str())
        .iter()
        .any(|forwarded| forwarded.argument.contains(span))
    {
        return true;
    }
    file.ast.arguments_containing(span).any(|(call, index)| {
        lookup
            .primitive_at_call(file, call.span)
            .is_some_and(|primitive| {
                lookup.dialect.callback_runs_on_result_access(
                    primitive,
                    index,
                    call.arguments.len(),
                )
            })
    })
}

/// Inline callbacks that do not clear the reactive listener inherit the
/// caller's execution role. This covers wrappers such as `batch`,
/// `catchError`'s protected body, and `modifyMutable`; `untrack` and other
/// explicit untracked callbacks are classified by the later dialect branch.
fn inline_callback_execution_role(
    file: &solid_facts::FileFacts,
    span: Span,
    allowed: &[Span],
    lookup: &SemanticLookup<'_>,
    classifying: &mut HashSet<(String, Span)>,
) -> Option<ExecutionRole> {
    file.ast
        .arguments_containing(span)
        .find_map(|(call, index)| {
            if !direct_callback_contains(file, call.arguments[index].span, span) {
                return None;
            }
            let primitive = lookup.primitive_at_call(file, call.span)?;
            if callback_execution_at_call(file, call, primitive, index, lookup)?
                != Execution::Inline
                || callback_runs_outside_tracking(
                    lookup.dialect,
                    primitive,
                    index,
                    call.arguments.len(),
                )
            {
                return None;
            }
            let key = (file.path.to_string(), call.span);
            if !classifying.insert(key.clone()) {
                return None;
            }
            let role = semantic_execution_role_within(
                file,
                call.span,
                allowed,
                lookup.entities(),
                lookup.symbol_names(),
                lookup,
                classifying,
            );
            classifying.remove(&key);
            // The callback runs where the call runs. The lexical rendering
            // role of the call is proven only when the call itself runs during
            // the body; a call written in a nested helper that runs later
            // (`const isCurrent = id => id === latest(() => draft())`) is not.
            if role == ExecutionRole::UntrackedRendering
                && !nested_literal_runs_during_body(
                    file,
                    call.span,
                    lookup.entities(),
                    lookup.symbol_names(),
                    lookup,
                )
            {
                return Some(ExecutionRole::Unknown);
            }
            Some(role)
        })
}

/// Compose an inline callback of a higher-order factory with the proven use of
/// the returned function. For example, `on` reads its dependency inline when
/// the returned adapter runs: `createEffect(on(...))` therefore tracks that
/// read, while a direct top-level adapter call does not.
fn returned_factory_callback_execution_role(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
    classifying: &mut HashSet<(String, Span)>,
) -> Option<ExecutionRole> {
    // Every proof this composition needs starts from
    // `callback_requires_return_invocation`, which Solid 2.0 leaves at its
    // `false` default for every primitive.
    if !lookup.models_returned_callbacks() {
        return None;
    }
    file.ast
        .arguments_containing(span)
        .find_map(|(factory_call, index)| {
            if !direct_callback_contains(file, factory_call.arguments[index].span, span) {
                return None;
            }
            let primitive = lookup.primitive_at_call(file, factory_call.span)?;
            let semantics = lookup.dialect.callback_semantics_at(
                primitive,
                index,
                factory_call.arguments.len(),
            );
            if !semantics.requires_return_invocation
                || semantics.execution != Some(Execution::Inline)
            {
                return None;
            }

            role_at_returned_invocations(
                returned_callback_invocation_sites(file, factory_call, lookup),
                lookup,
                classifying,
            )
        })
}

/// The one execution role the proven invocations of a returned function give
/// the callbacks it runs on its invoker's stack, or `None` when there is no
/// proven invocation. Shared by the dialect's returned-callback composition and
/// ADR 0152's contract composition, so the two cannot read the same sites
/// differently.
fn role_at_returned_invocations(
    sites: Vec<crate::owners::ReturnedCallbackInvocationSite>,
    lookup: &SemanticLookup<'_>,
    classifying: &mut HashSet<(String, Span)>,
) -> Option<ExecutionRole> {
    let mut roles = Vec::new();
    for site in sites {
        let role = match site.inherited_execution {
            Some(Execution::Tracked) => Some(ExecutionRole::TrackedJsx),
            Some(Execution::Deferred) => Some(ExecutionRole::DeferredCallback),
            Some(Execution::Inline) | None => {
                let key = (site.path.clone(), site.span);
                if classifying.contains(&key) {
                    // A cyclic invocation site — the adapter calling
                    // itself through its own callback — has no context
                    // of its own to contribute.
                    None
                } else {
                    lookup
                        .files()
                        .iter()
                        .find(|candidate| candidate.path.as_str() == site.path)
                        .map(|use_file| {
                            classifying.insert(key.clone());
                            let role = semantic_execution_role_within(
                                use_file,
                                site.span,
                                &[],
                                lookup.entities(),
                                lookup.symbol_names(),
                                lookup,
                                classifying,
                            );
                            classifying.remove(&key);
                            role
                        })
                }
            }
        };
        roles.extend(role);
    }
    roles.sort_by_key(|role| *role as u8);
    roles.dedup();
    match roles.as_slice() {
        [] => None,
        [role] => Some(*role),
        // The same returned adapter is used in incompatible execution
        // contexts. A single diagnostic site cannot truthfully claim one
        // dominates, so preserve uncertainty instead of manufacturing a
        // false positive in either direction.
        _ => Some(ExecutionRole::DeferredCallback),
    }
}

/// ADR 0152: the call and argument slot at which `span`'s own function is
/// handed, directly, to an accepted contract export whose returned value
/// invokes that slot on its invoker's stack
/// ([`crate::ContractExport::returned_invocations`]).
///
/// The literal must *be* the argument -- not an element of an array or object
/// written there, and not a spread -- and `span` must sit in the literal's own
/// body rather than in a function nested in it, which is what
/// [`direct_callback_contains`] answers.
pub(super) fn contract_returned_invoker_slot<'f>(
    file: &'f solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> Option<&'f solid_facts::ast::CallFact> {
    file.ast
        .arguments_containing(span)
        .find_map(|(call, index)| {
            let argument = &call.arguments[index];
            if argument.spread || !direct_callback_contains(file, argument.span, span) {
                return None;
            }
            let literal = containing_ast_function(&file.ast, span)?;
            if file.ast.peel_ts_sugar_span(argument.span) != literal.span {
                return None;
            }
            let symbol = lookup.callee_symbol(file, call.callee)?;
            lookup
                .contract_returned_invocations(symbol)?
                .contains(&index)
                .then_some(call)
        })
}

/// ADR 0152: a callback handed to a contract export whose returned value runs
/// it on its invoker's stack runs wherever that value is invoked, so its role
/// is the role of the value's proven invocations -- the composition
/// [`returned_factory_callback_execution_role`] makes for a dialect primitive,
/// read here from the accepted contract's nested claim instead of the
/// dialect. With no proven invocation it answers nothing, and the callback
/// stays one of unproven timing ([`callee_callback_timing`]).
fn contract_returned_invoker_callback_role(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
    classifying: &mut HashSet<(String, Span)>,
) -> Option<ExecutionRole> {
    let factory_call = contract_returned_invoker_slot(file, span, lookup)?;
    role_at_returned_invocations(
        returned_callback_invocation_sites(file, factory_call, lookup),
        lookup,
        classifying,
    )
}

fn returned_callback_execution_role(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
    classifying: &mut HashSet<(String, Span)>,
) -> Option<ExecutionRole> {
    file.ast
        .arguments_containing(span)
        .find_map(|(call, index)| {
            if !direct_callback_contains(file, call.arguments[index].span, span) {
                return None;
            }
            let (primitive, result_slot) = returned_primitive_invocation(file, call, lookup)?;
            match lookup
                .dialect
                .returned_callback_semantics_at(primitive, result_slot, index, call.arguments.len())
                .execution?
            {
                Execution::Tracked => Some(ExecutionRole::TrackedJsx),
                Execution::Deferred => Some(ExecutionRole::DeferredCallback),
                // The returned function restores/inherits its caller's execution
                // context. Classify the proven invocation span, outside the
                // callback body itself, so a top-level transition remains
                // untracked while one started from an effect remains tracked.
                Execution::Inline => {
                    let key = (file.path.to_string(), call.span);
                    if !classifying.insert(key.clone()) {
                        return None;
                    }
                    let role = semantic_execution_role_within(
                        file,
                        call.span,
                        &[],
                        lookup.entities(),
                        lookup.symbol_names(),
                        lookup,
                        classifying,
                    );
                    classifying.remove(&key);
                    Some(role)
                }
            }
        })
}

/// Whether `span` is evaluated as the `value` getter of a resolved Solid 1.x
/// `createContext().Provider`.
///
/// Both halves are semantic proof: the JSX member object resolves to the
/// binding initialized by the exact Solid `createContext` primitive, and the
/// final member is `Provider`. An arbitrary component named `SomeProvider` or
/// object with a same-spelled property satisfies neither condition.
fn context_provider_value_role(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> Option<ExecutionRole> {
    let provider_member = lookup.dialect.context_provider_member()?;
    let element = file
        .ast
        .jsx_elements
        .iter()
        .filter(|element| {
            element.attributes.iter().any(|attribute| {
                file.source_text(attribute.local_name) == Some("value")
                    && attribute
                        .expression
                        .is_some_and(|expression| expression.contains(span))
            })
        })
        .min_by_key(|element| element.span.end - element.span.start)?;
    let (Some(object), Some(property)) = (element.member_object, element.member_property) else {
        return None;
    };
    if file.source_text(property) != Some(provider_member) {
        return None;
    }
    if !lookup.is_context_reference(file.path.as_str(), object) {
        return None;
    }
    // Solid 1.x's createProvider implementation eagerly reads props.value
    // inside untrack. A function expression is the one exception: reading
    // the getter only creates/stores the function; its body is not run.
    let stores_function = file.ast.functions_body_containing(span).any(|function| {
        element.attributes.iter().any(|attribute| {
            attribute
                .expression
                .is_some_and(|expression| expression.contains(function.span))
        })
    });
    Some(if stores_function {
        ExecutionRole::DeferredCallback
    } else {
        ExecutionRole::UntrackedRendering
    })
}

pub(super) fn assigned_member_function_contains(
    file: &solid_facts::FileFacts,
    span: Span,
    entities: &EntitySymbols,
) -> bool {
    containing_ast_function(&file.ast, span).is_some_and(|function| {
        file.ast.assignments.iter().any(|assignment| {
            if assignment.value != solid_facts::ast::AssignmentValueKind::Function
                || !assignment.value_span.contains(function.span)
            {
                return false;
            }
            let Some(member) = file
                .ast
                .members
                .iter()
                .find(|member| member.span == assignment.target)
            else {
                return false;
            };
            let Some(object_symbol) = entities.at(file.path.as_str(), member.object) else {
                return false;
            };
            let Some(owner) = containing_ast_function(&file.ast, assignment.target) else {
                return false;
            };
            let caller_owned = owner.parameters.iter().any(|parameter| {
                parameter
                    .names
                    .iter()
                    .any(|name| entities.at(file.path.as_str(), name.span) == Some(object_symbol))
            });
            let returned = file.ast.returns.iter().any(|returned| {
                returned.value == solid_facts::ast::ReturnValueKind::Identifier
                    && containing_ast_function(&file.ast, returned.span)
                        .is_some_and(|candidate| candidate.span == owner.span)
                    && entities.at(file.path.as_str(), returned.span) == Some(object_symbol)
            });
            caller_owned || returned
        })
    })
}

pub(super) fn control_flow_execution_role(
    file: &solid_facts::FileFacts,
    span: Span,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    dialect: &dyn Dialect,
) -> Option<ExecutionRole> {
    let element = file
        .ast
        .jsx_containing(span)
        .filter(|element| {
            jsx_primitive_name(file, element, entities, symbol_names, dialect)
                .as_ref()
                .and_then(PrimitiveName::primitive)
                .is_some_and(|primitive| dialect.renders_children_through_callback(primitive))
        })
        .min_by_key(|element| element.span.end - element.span.start)?;
    // The control-flow component invokes only a function written at its own
    // level: its children callback (`<For>{(item) => …}</For>`) or code run
    // inline from there. A function inside a *nested* element belongs to that
    // element -- `<Show><div onClick={() => …} /></Show>` hands the arrow to
    // the `<div>` as an event handler, which the compiler classifies, and
    // which runs when the event fires, with no owner (probed on the audited
    // 2.0.0-rc.9, dev and prod: a signal write there neither throws nor
    // warns). The named-callback index draws the same line for identifiers.
    //
    // Only a function written in one of the element's *children* is that
    // callback (or code run inline from it). A function in an attribute --
    // the predicate inside `each={items.filter((p) => p.id !== props.id)}`, a
    // `when` expression's callback -- is an ordinary expression of the
    // attribute it is written in, not what the component invokes with the
    // item.
    let callback =
        file.ast
            .functions_body_containing(span)
            .filter(|function| {
                element.span.contains(function.span)
                    && element
                        .children
                        .iter()
                        .any(|child| child.contains(function.span))
                    && !file.ast.jsx_containing(function.span).any(|nested| {
                        nested.span != element.span && element.span.contains(nested.span)
                    })
            })
            .max_by_key(|function| function.body.end - function.body.start)?;
    let owner = containing_ast_function(&file.ast, span)?;
    if owner.span != callback.span {
        return Some(ExecutionRole::DeferredCallback);
    }
    // A fragment has no element fact, so its children are consulted through
    // its own span table: `<>{tone()}</>` is as tracked as `<b>{tone()}</b>`.
    if file
        .ast
        .jsx_containing(span)
        .any(|nested| callback.body.contains(nested.span))
        || file
            .ast
            .jsx_fragments
            .iter()
            .any(|fragment| fragment.contains(span) && callback.body.contains(*fragment))
    {
        Some(ExecutionRole::TrackedJsx)
    } else {
        Some(ExecutionRole::UntrackedRendering)
    }
}

/// A role for a site directly in an exact control-flow children literal.
/// Containment alone is insufficient: `{wrap(() => ... )}` passes the result
/// of `wrap`, not that literal. Unknown wrappers, competing children props and
/// spreads do not acquire a render-body proof through this admission gate.
pub(super) fn direct_control_flow_body_role(
    file: &solid_facts::FileFacts,
    span: Span,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    dialect: &dyn Dialect,
) -> Option<ExecutionRole> {
    let owner = containing_ast_function(&file.ast, span)?;
    if owner.r#async
        || owner.generator
        || !crate::owners::written_directly_in(&file.ast, owner, span)
    {
        return None;
    }
    let exact = file.ast.jsx_containing(owner.span).any(|element| {
        element.spreads.is_empty()
            && element
                .children
                .iter()
                .filter(|child| {
                    file.source_text(**child)
                        .is_none_or(|text| !text.trim().is_empty())
                })
                .count()
                == 1
            && !element
                .attributes
                .iter()
                .any(|attribute| file.source_text(attribute.name) == Some("children"))
            && element
                .children
                .iter()
                .any(|child| jsx_child_expression(file, *child) == Some(owner.span))
            && exact_jsx_primitive_name(file, element, entities, symbol_names, dialect)
                .as_ref()
                .and_then(PrimitiveName::primitive)
                .is_some_and(|primitive| dialect.renders_children_through_callback(primitive))
    });
    if !exact {
        return None;
    }
    control_flow_execution_role(file, span, entities, symbol_names, dialect).filter(|role| {
        matches!(
            role,
            ExecutionRole::UntrackedRendering | ExecutionRole::TrackedJsx
        )
    })
}

/// The expression an expression-container child (`{expression}`) holds, with
/// transparent TypeScript wrappers peeled. `None` for a text, element or
/// fragment child, which holds no expression of its own.
fn jsx_child_expression(file: &solid_facts::FileFacts, child: Span) -> Option<Span> {
    let text = file.source_text(child)?;
    let inner = text.strip_prefix('{')?.strip_suffix('}')?;
    let leading = u32::try_from(inner.len() - inner.trim_start().len()).ok()?;
    let trailing = u32::try_from(inner.len() - inner.trim_end().len()).ok()?;
    let (start, end) = (child.start + 1 + leading, child.end - 1 - trailing);
    (start < end).then(|| file.ast.peel_ts_sugar_span(Span::new(start, end)))
}

/// How this file's primitive calls and control-flow JSX name each of its own
/// functions as a callback.
///
/// Every flag is a property of the *function*, not of the read being
/// classified: which argument positions name it, and how the dialect executes
/// those positions. Deriving them once per file replaces the whole-file call and
/// JSX scans [`named_callback_execution_role`] used to run for every read it was
/// asked about -- five of them per read in the worst case.
#[derive(Default)]
pub(super) struct NamedCallbackRoles {
    by_function: HashMap<Span, NamedCallbackRole>,
}

/// The classification a named callback's positions support, in the order
/// [`named_callback_execution_role`] consults them.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct NamedCallbackRole {
    /// Some position naming this function is one an arm below can truthfully
    /// classify. An effect's tracked compute and an inline adapter argument
    /// (1.x `on`'s deps) satisfy none of them -- admitting those would fall
    /// through to the rendering tail and misreport `createEffect(namedCompute)`
    /// as untracked rendering. Answering None instead defers such reads to
    /// compiler facts and the returned-adapter classifiers.
    admitted: bool,
    untracked_callback: bool,
    effect_apply: bool,
    /// Which effect primitive names this function at its apply position, for
    /// the read-context wording only.
    effect_apply_primitive: EffectApplyPrimitive,
    tracked: bool,
    deferred: bool,
    /// Named at a position whose callback runs when the call's returned
    /// object is read ([`Dialect::callback_runs_on_result_access`]). That
    /// time and scope are the reader's, which this index cannot see, so the
    /// function's own role is unknown and outranks every arm above.
    result_access: bool,
}

/// The effect primitive(s) whose apply position names a function.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum EffectApplyPrimitive {
    #[default]
    None,
    One(Primitive),
    /// Named by the apply positions of two different primitives: the wording
    /// names neither.
    Several,
}

impl EffectApplyPrimitive {
    fn with(self, primitive: Primitive) -> Self {
        match self {
            Self::None => Self::One(primitive),
            Self::One(existing) if existing == primitive => self,
            Self::One(_) | Self::Several => Self::Several,
        }
    }

    const fn single(self) -> Option<Primitive> {
        match self {
            Self::One(primitive) => Some(primitive),
            Self::None | Self::Several => None,
        }
    }
}

impl NamedCallbackRoles {
    fn entry(&mut self, function: Span) -> &mut NamedCallbackRole {
        self.by_function.entry(function).or_default()
    }

    fn get(&self, function: Span) -> NamedCallbackRole {
        self.by_function.get(&function).copied().unwrap_or_default()
    }
}

/// Every way this file's callback positions can name one of its own functions.
///
/// The three maps are the *semantic* alternatives to comparing source text: an
/// exact demanded entity, a proven TypeScript reference to the function's
/// symbol, and -- for a function whose symbol carries a canonical Solid name --
/// that name. Admitting a function because some identifier in a callback
/// position happens to be spelled the same, which is what these replace,
/// accepts a same-spelled binding from any other scope or module.
struct NamedCallbackIndex<'a> {
    /// This file's functions that resolve to a symbol, in AST order.
    functions: Vec<Span>,
    by_symbol: HashMap<&'a str, Vec<usize>>,
    by_reference: HashMap<Span, Vec<usize>>,
    by_canonical_name: HashMap<&'a str, Vec<usize>>,
}

impl<'a> NamedCallbackIndex<'a> {
    fn new(
        file: &solid_facts::FileFacts,
        entities: &'a EntitySymbols,
        symbol_names: &'a HashMap<SymbolId, SymbolId>,
        lookup: &SemanticLookup<'_>,
    ) -> Self {
        let candidates = file
            .ast
            .functions
            .iter()
            .filter_map(|function| {
                Some((function.span, function_symbol(file, function, entities)?))
            })
            .collect::<Vec<_>>();
        let mut by_symbol = HashMap::<&str, Vec<usize>>::new();
        let mut by_reference = HashMap::<Span, Vec<usize>>::new();
        let mut by_canonical_name = HashMap::<&str, Vec<usize>>::new();
        for (index, (_, symbol)) in candidates.iter().enumerate() {
            by_symbol.entry(symbol.as_str()).or_default().push(index);
            if let Some(name) = symbol_names.get(*symbol) {
                by_canonical_name
                    .entry(name.as_str())
                    .or_default()
                    .push(index);
            }
            for reference in lookup.symbol_references(symbol.as_str()) {
                if reference.path.as_ref() != file.path.as_str() {
                    continue;
                }
                let (Ok(start), Ok(end)) = (
                    u32::try_from(reference.start_byte),
                    u32::try_from(reference.end_byte),
                ) else {
                    continue;
                };
                by_reference
                    .entry(Span::new(start, end))
                    .or_default()
                    .push(index);
            }
        }
        Self {
            functions: candidates.iter().map(|(span, _)| *span).collect(),
            by_symbol,
            by_canonical_name,
            by_reference,
        }
    }

    /// The functions an exact demanded entity at `span` names -- the whole of a
    /// bare `createEffect(compute, applyValue)` argument, say.
    fn identity_at(
        &self,
        file: &solid_facts::FileFacts,
        entities: &EntitySymbols,
        span: Span,
    ) -> &[usize] {
        entities
            .at(file.path.as_str(), span)
            .and_then(|symbol| self.by_symbol.get(symbol.as_str()))
            .map_or(&[], Vec::as_slice)
    }

    /// The functions a use at `span` names, by any of the three proofs.
    fn named_at(
        &self,
        file: &solid_facts::FileFacts,
        entities: &EntitySymbols,
        span: Span,
        matched: &mut Vec<usize>,
    ) {
        matched.extend(self.identity_at(file, entities, span));
        if let Some(references) = self.by_reference.get(&span) {
            matched.extend(references.iter().copied());
        }
        if let Some(text) = file.source_text(span)
            && let Some(named) = self.by_canonical_name.get(text)
        {
            matched.extend(named.iter().copied());
        }
    }
}

/// Derive [`NamedCallbackRoles`] for one file. Memoized by
/// [`SemanticLookup::named_callback_roles`]; nothing else should call it.
pub(super) fn named_callback_roles(
    file: &solid_facts::FileFacts,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
) -> NamedCallbackRoles {
    let dialect = lookup.dialect;
    let index = NamedCallbackIndex::new(file, entities, symbol_names, lookup);
    let mut roles = NamedCallbackRoles::default();
    if index.functions.is_empty() {
        return roles;
    }
    let primitives = lookup.primitives(file);
    let mut named = Vec::new();
    for (call_index, call) in file.ast.calls.iter().enumerate() {
        let direct_primitive = known_primitive(&primitives.calls[call_index]);
        let returned_primitive = direct_primitive
            .is_none()
            .then(|| returned_primitive_invocation(file, call, lookup))
            .flatten();
        if direct_primitive.is_none() && returned_primitive.is_none() {
            continue;
        }
        let count = call.arguments.len();
        for (argument_index, argument) in call.arguments.iter().enumerate() {
            // The argument itself, plus the callbacks an options object names
            // through its `effect`/`error` properties.
            named.clear();
            index.named_at(file, entities, argument.span, &mut named);
            for property in &argument.identifier_properties {
                index.named_at(file, entities, property.span, &mut named);
            }
            named.sort_unstable();
            named.dedup();
            if named.is_empty() {
                continue;
            }
            if let Some((_primitive, _result_slot)) = returned_primitive {
                let execution =
                    returned_callback_execution_at_call(file, call, argument_index, lookup);
                let tracked = execution == Some(Execution::Tracked);
                let deferred = execution == Some(Execution::Deferred);
                // Returned-function contracts describe the argument itself,
                // never a callback named inside an options object. Require
                // exact TypeScript identity for the admitted function. Inline
                // returned callbacks inherit the individual call site's role
                // and therefore cannot be collapsed into this per-function
                // index.
                for candidate in index.identity_at(file, entities, argument.span) {
                    let entry = roles.entry(index.functions[*candidate]);
                    entry.admitted |= tracked || deferred;
                    entry.tracked |= tracked;
                    entry.deferred |= deferred;
                }
                continue;
            }
            let primitive = direct_primitive.expect("one primitive kind is proven above");
            if dialect.callback_runs_on_result_access(primitive, argument_index, count) {
                for candidate in index.identity_at(file, entities, argument.span) {
                    let entry = roles.entry(index.functions[*candidate]);
                    entry.admitted = true;
                    entry.result_access = true;
                }
                continue;
            }
            let proven =
                callback_execution_at_call(file, call, primitive, argument_index, lookup).is_some();
            let untracked = dialect.reports_untracked_reads_at(primitive, argument_index, count);
            // Deliberately not gated on `proven`: an effect's apply position is
            // read straight off the dialect signature, as it always was.
            let effect_apply =
                effect_apply_argument(dialect, primitive, count) == Some(argument_index);
            let tracked = !is_effect(primitive)
                && dialect
                    .callback_semantics_at(primitive, argument_index, count)
                    .tracks_reads;
            let deferred =
                callback_runs_outside_tracking(dialect, primitive, argument_index, count);
            for candidate in &named {
                let entry = roles.entry(index.functions[*candidate]);
                entry.admitted |= proven && (untracked || effect_apply || tracked || deferred);
                entry.untracked_callback |= proven && untracked;
                entry.effect_apply |= effect_apply;
                if effect_apply {
                    entry.effect_apply_primitive = entry.effect_apply_primitive.with(primitive);
                }
            }
            // The tracked and deferred arms have always demanded that the whole
            // argument *be* the function, never that an options object mention
            // it.
            for candidate in index.identity_at(file, entities, argument.span) {
                let entry = roles.entry(index.functions[*candidate]);
                entry.tracked |= proven && tracked;
                entry.deferred |= proven && deferred;
            }
        }
    }
    // A *named* function bound to a JSX event handler -- `onPointerDown={onPointerDown}`
    // with the handler declared in the component body.
    //
    // The compiler censuses the attribute, so `callback_roles` carries an
    // `EventHandler` span covering the attribute value; an *inline* arrow gets
    // its role from that span directly, because the arrow's body lies inside
    // it. A named handler's body does not, so nothing classified it and its
    // reads took the enclosing component's `UntrackedRendering` role. A read
    // written in such a body was then silent only because `local_access` gates
    // reads inside an unproven helper, while the same read propagated through
    // one call was reported as a proven untracked read -- 24 of SC1001's 71
    // violations on the consumer corpus, including `ref()` read inside a
    // handler two lines below a silent direct read of the same accessor.
    //
    // `deferred`, which is what an event handler is: it runs after setup,
    // outside the tracking phase, and reads current values when it fires.
    // Identity-exact, like the tracked/deferred arms above -- the attribute
    // value must *be* the function, so `onClick={() => handler()}` keeps
    // classifying the arrow rather than `handler`.
    for callback in &file.compiler.callback_roles {
        if callback.role != solid_facts::compiler::CallbackRoleKind::EventHandler {
            continue;
        }
        // The attribute value must be a bare identifier reference.
        // `entities.at` answers a *call* span with the callee's symbol, so
        // without this `onClick={makeHandler()}` would admit `makeHandler`
        // itself and silence the setup-time writes in its body -- which is a
        // pinned positive in `fixtures/reactive-ir/directive-phases`.
        if !file.ast.identifiers.iter().any(|identifier| {
            identifier.span == callback.span
                && identifier.role == solid_facts::ast::IdentifierRole::Reference
        }) {
            continue;
        }
        for candidate in index.identity_at(file, entities, callback.span) {
            let entry = roles.entry(index.functions[*candidate]);
            entry.admitted = true;
            entry.deferred = true;
        }
    }
    for (element_index, element) in file.ast.jsx_elements.iter().enumerate() {
        if !known_primitive(&primitives.jsx[element_index])
            .is_some_and(|primitive| dialect.renders_children_through_callback(primitive))
        {
            continue;
        }
        for identifier in file.ast.identifiers_within(element.span) {
            // The identifier must *be* a children expression
            // (`<For each={xs}>{renderItem}</For>`). Merely mentioning a
            // helper elsewhere in the element -- `when={visible()}`,
            // `each={rows()}`, a `fallback` -- passes it nothing to invoke as a
            // render callback.
            if identifier.role != solid_facts::ast::IdentifierRole::Reference
                || !element
                    .children
                    .iter()
                    .any(|child| jsx_child_expression(file, *child) == Some(identifier.span))
                || file
                    .ast
                    .jsx_containing(identifier.span)
                    .any(|nested| nested.span != element.span && element.span.contains(nested.span))
            {
                continue;
            }
            named.clear();
            index.named_at(file, entities, identifier.span, &mut named);
            for candidate in &named {
                roles.entry(index.functions[*candidate]).admitted = true;
            }
        }
    }
    roles
}

pub(super) fn named_callback_execution_role(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> Option<ExecutionRole> {
    let roles = lookup.named_callback_roles(file);
    let callback = file
        .ast
        .functions_body_containing(span)
        .find(|function| roles.get(function.span).admitted)?;
    let owner = containing_ast_function(&file.ast, span)?;
    if owner.span != callback.span {
        return Some(ExecutionRole::DeferredCallback);
    }
    let role = roles.get(callback.span);
    if role.result_access {
        return Some(ExecutionRole::Unknown);
    }
    if role.untracked_callback {
        return Some(ExecutionRole::UntrackedCallback);
    }
    if role.effect_apply {
        return Some(ExecutionRole::EffectApply);
    }
    if role.tracked {
        return Some(ExecutionRole::TrackedJsx);
    }
    if role.deferred {
        return Some(ExecutionRole::DeferredCallback);
    }
    if file
        .ast
        .jsx_containing(span)
        .any(|element| callback.body.contains(element.span))
    {
        Some(ExecutionRole::TrackedJsx)
    } else {
        Some(ExecutionRole::UntrackedRendering)
    }
}

pub(super) fn function_symbol<'a>(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    entities: &'a EntitySymbols,
) -> Option<&'a SymbolId> {
    let name = function
        .name
        .as_ref()
        .or_else(|| function_binding_name(file, function))?;
    entities.get(&location(file.path.shared(), name.span))
}

pub(super) fn argument_references_callback_symbol(
    file: &solid_facts::FileFacts,
    argument: &solid_facts::ast::ArgumentFact,
    symbol: &str,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
) -> bool {
    entities
        .get(&location(file.path.shared(), argument.span))
        .map(SymbolId::as_str)
        == Some(symbol)
        || argument.identifier_properties.iter().any(|property| {
            entities
                .get(&location(file.path.shared(), property.span))
                .map(SymbolId::as_str)
                == Some(symbol)
                || symbol_names.get(symbol).map(SymbolId::as_str) == file.source_text(property.span)
        })
}

pub(super) fn direct_callback_contains(
    file: &solid_facts::FileFacts,
    argument: Span,
    span: Span,
) -> bool {
    if !argument.contains(span) {
        return false;
    }
    let callback = file
        .ast
        .functions_within(argument)
        .max_by_key(|function| function.span.end - function.span.start);
    let owner = containing_ast_function(&file.ast, span);
    match (callback, owner) {
        (Some(callback), Some(owner)) => callback.span == owner.span,
        (None, None) => true,
        _ => false,
    }
}

pub(super) fn read_analysis_context(
    file: &solid_facts::FileFacts,
    span: Span,
    execution: ExecutionRole,
    lookup: &SemanticLookup<'_>,
) -> String {
    if execution == ExecutionRole::EffectApply {
        // The role is shared by every effect primitive's apply position; the
        // name of the primitive whose apply this is is not. It used to be
        // `createEffect` unconditionally, which misnamed every read in a
        // `createRenderEffect` apply. Where the primitive is not one exact
        // answer -- a named function passed as the apply of two different
        // primitives -- or the dialect has no spelling for it, the sentence
        // stays true without naming one: this is read context in a message,
        // not a premise of any proof.
        match effect_apply_primitive(file, span, lookup)
            .and_then(|primitive| lookup.dialect.name_of(primitive))
        {
            Some(name) => format!("{name} apply callback"),
            None => "effect apply callback".into(),
        }
    } else {
        let context = enclosing_function_label(file, span);
        format_read_context(&context, file.ast.any_conditional_test_containing(span))
    }
}

/// The effect primitive whose apply callback `span` runs in, mirroring the two
/// ways [`semantic_execution_role_within`] reaches `EffectApply`: a named
/// function passed at an apply position, then the innermost inline apply
/// argument containing the span (inline wrappers such as `batch` inside it
/// inherit that apply's role).
fn effect_apply_primitive(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> Option<Primitive> {
    let roles = lookup.named_callback_roles(file);
    if let Some(callback) = file
        .ast
        .functions_body_containing(span)
        .find(|function| roles.get(function.span).admitted)
        && containing_ast_function(&file.ast, span).is_some_and(|owner| owner.span == callback.span)
        && roles.get(callback.span).effect_apply
    {
        return roles.get(callback.span).effect_apply_primitive.single();
    }
    file.ast
        .arguments_containing(span)
        .filter_map(|(call, index)| {
            let primitive = lookup.primitive_at_call(file, call.span)?;
            (effect_apply_argument(lookup.dialect, primitive, call.arguments.len()) == Some(index))
                .then(|| (call.arguments[index].span, primitive))
        })
        .min_by_key(|(argument, _)| argument.end - argument.start)
        .map(|(_, primitive)| primitive)
}

fn format_read_context(context: &str, in_conditional_test: bool) -> String {
    if in_conditional_test {
        if context.is_empty() {
            "while evaluating a condition".into()
        } else {
            format!("{context} while evaluating a condition")
        }
    } else {
        context.into()
    }
}

pub(super) fn async_execution_role(
    file: &solid_facts::FileFacts,
    span: Span,
    execution: ExecutionRole,
) -> ExecutionRole {
    if execution == ExecutionRole::DeferredCallback && file.ast.any_jsx_containing(span) {
        ExecutionRole::TrackedJsx
    } else {
        execution
    }
}

pub(super) fn pending_accessor_probe(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    file.ast.arguments_containing(span).any(|(call, index)| {
        direct_callback_contains(file, call.arguments[index].span, span)
            && lookup
                .primitive_at_call(file, call.span)
                .is_some_and(|primitive| {
                    lookup
                        .dialect
                        .callback_handles_pending_accessor_read(primitive, index)
                })
    })
}

pub(super) fn allowed_callback_spans(
    file: &solid_facts::FileFacts,
    lookup: &SemanticLookup<'_>,
) -> Vec<Span> {
    let dialect = lookup.dialect;
    let primitives = lookup.primitives(file);
    let mut spans = Vec::new();
    for (call_index, call) in file.ast.calls.iter().enumerate() {
        let mut indices =
            known_primitive(&primitives.calls[call_index]).map_or_else(Vec::new, |primitive| {
                deferred_callback_positions(dialect, primitive, call.arguments.len())
                    .into_iter()
                    .filter(|index| {
                        callback_execution_at_call(file, call, primitive, *index, lookup).is_some()
                    })
                    .collect()
            });
        let primitive_indices = indices.len();
        if let Some(symbol) = lookup.callee_symbol(file, call.callee) {
            if let Some(callbacks) = lookup.contract_callbacks(symbol) {
                for callback in &callbacks {
                    let exclusively_deferred = callbacks.iter().all(|candidate| {
                        candidate.parameter != callback.parameter
                            || candidate.execution == "deferred"
                    });
                    if callback.execution == "deferred"
                        && exclusively_deferred
                        && !indices.contains(&callback.parameter)
                    {
                        indices.push(callback.parameter);
                    }
                }
            } else if lookup.unknown_contract_callback_export(symbol).is_some() {
                // The contract explicitly does not prove when this callee runs
                // a caller-supplied callback. The call site already carries
                // that as an SC9005 obligation; additionally reporting a read
                // inside the callback as a *proven* untracked read would
                // assert exactly the timing the contract says it does not
                // have. Treat every argument whose syntax leaves callability
                // open as an unproven-timing callback so the read stays
                // uncertifiable instead of becoming a violation.
                for (index, argument) in call.arguments.iter().enumerate() {
                    if !crate::runtime_semantics::literal_argument_is_not_callable(
                        argument.runtime_value_kind,
                    ) && !indices.contains(&index)
                    {
                        indices.push(index);
                    }
                }
            }
        }
        for (position, index) in indices.into_iter().enumerate() {
            let Some(argument) = call.arguments.get(index) else {
                continue;
            };
            // A contract slot states when the callee runs a *function* it
            // receives, never when the caller evaluates the argument. Only a
            // function literal delivered as the value is wholly callback
            // code; any other argument — `overlay()`, `...overlays()`, a
            // member read — runs in the caller now, and only the functions
            // written inside it can be the deferred code. Shielding the whole
            // expression would turn a proven eager read into a callback one.
            if position < primitive_indices
                || argument.runtime_value_kind == solid_facts::ast::RuntimeValueKind::Function
            {
                spans.push(argument.span);
            } else {
                spans.extend(
                    file.ast
                        .functions_within(argument.span)
                        .map(|function| function.span),
                );
            }
        }
    }
    spans
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use solid_facts::{
        compiler::{
            COMPILER_FACTS_PROTOCOL, CallbackRole, CallbackRoleKind, ExecutionMap, ExecutionRegion,
            RegionReason,
        },
        core::{Generation, SourceHash, SourcePath, Span},
    };

    use super::{execution_role, format_read_context, missing_jsx_census};
    use crate::ExecutionRole;

    fn execution_map() -> ExecutionMap {
        ExecutionMap {
            compiler_facts_protocol: COMPILER_FACTS_PROTOCOL,
            source_hash: SourceHash::of("value"),
            semantic_model: Default::default(),
            tracked_regions: vec![],
            untracked_regions: vec![],
            discarded_regions: vec![],
            ownership_regions: vec![],
            callback_roles: vec![],
            jsx_operations: vec![],
        }
    }

    #[test]
    fn compiler_execution_distinguishes_explicit_untracked_from_unknown() {
        let mut facts = execution_map();
        assert_eq!(
            execution_role(&facts, Span::new(0, 5), &[]),
            ExecutionRole::Unknown
        );
        facts.untracked_regions.push(ExecutionRegion {
            span: Span::new(0, 5),
            reason: RegionReason::JsxChild,
        });
        assert_eq!(
            execution_role(&facts, Span::new(0, 5), &[]),
            ExecutionRole::UntrackedRendering
        );
    }

    #[test]
    fn smallest_compiler_region_wins_across_execution_fact_categories() {
        let mut facts = execution_map();
        facts.untracked_regions.push(ExecutionRegion {
            span: Span::new(0, 100),
            reason: RegionReason::JsxChild,
        });
        facts.callback_roles.push(CallbackRole {
            span: Span::new(40, 60),
            role: CallbackRoleKind::EventHandler,
        });

        assert_eq!(
            execution_role(&facts, Span::new(50, 51), &[]),
            ExecutionRole::EventCallback
        );
    }

    /// One file with a real AST and a hand-built census.
    fn file(source: &str, compiler: ExecutionMap) -> solid_facts::FileFacts {
        const PATH: &str = "app.tsx";
        solid_facts::FileFacts {
            generation: Generation::new(1).unwrap(),
            path: SourcePath::new(PATH).unwrap(),
            source_hash: SourceHash::of(source),
            source: Arc::from(source),
            ast: Arc::new(solid_facts::ast::extract(PATH, source).unwrap()),
            compiler: Arc::new(compiler),
        }
    }

    /// The byte span of the first occurrence of `needle` in `source`.
    fn span_of(source: &str, needle: &str) -> Span {
        let start = source.find(needle).expect("needle occurs in source");
        Span::new(
            u32::try_from(start).unwrap(),
            u32::try_from(start + needle.len()).unwrap(),
        )
    }

    /// The spread arm of the source-level JSX-region lookup, which no fixture
    /// reaches: both census-gap fixtures exercise the child and attribute arms,
    /// because the shapes the pinned producers decline to census are a dropped
    /// `<head>` and a void element's children rather than a spread.
    ///
    /// The census entry over the sibling child is what makes this the spread
    /// arm rather than "nothing was censused at all": the element *is* covered,
    /// and the read is still uncertifiable because the narrowest region
    /// containing it is the spread container, which no entry touches.
    #[test]
    fn census_gap_is_detected_through_a_spread_container() {
        let source = "const view = <div {...props}>{label()}</div>;\n";
        let read = span_of(source, "props");
        let mut census = execution_map();
        census.source_hash = SourceHash::of(source);
        census.tracked_regions.push(ExecutionRegion {
            span: span_of(source, "label()"),
            reason: RegionReason::JsxChild,
        });

        let facts = file(source, census.clone());
        assert!(
            missing_jsx_census(&facts, read, ExecutionRole::UntrackedRendering),
            "the spread container carries no census entry"
        );
        // Any other role was decided by a fact, so the gap cannot be what put
        // the read there.
        assert!(!missing_jsx_census(
            &facts,
            read,
            ExecutionRole::UntrackedCallback
        ));

        // A census entry that touches the spread closes the gap.
        census.untracked_regions.push(ExecutionRegion {
            span: span_of(source, "{...props}"),
            reason: RegionReason::JsxChild,
        });
        assert!(!missing_jsx_census(
            &file(source, census),
            read,
            ExecutionRole::UntrackedRendering
        ));
    }

    /// Deletion dominates. A discarded region is not a narrower or weaker
    /// untracked region: it says the code is gone, so a narrower live region
    /// inside it describes lowering the deletion took with it, and no
    /// width-based competition can be allowed to resurrect it.
    #[test]
    fn a_discarded_region_dominates_every_live_region_inside_it() {
        let mut facts = execution_map();
        facts.discarded_regions.push(ExecutionRegion {
            span: Span::new(0, 100),
            reason: RegionReason::JsxAttribute,
        });
        facts.tracked_regions.push(ExecutionRegion {
            span: Span::new(40, 60),
            reason: RegionReason::JsxChild,
        });
        facts.callback_roles.push(CallbackRole {
            span: Span::new(45, 55),
            role: CallbackRoleKind::EventHandler,
        });

        assert_eq!(
            execution_role(&facts, Span::new(50, 51), &[]),
            ExecutionRole::DiscardedRendering
        );
        // Outside it, nothing changed.
        assert_eq!(
            execution_role(&facts, Span::new(200, 201), &[]),
            ExecutionRole::Unknown
        );
    }

    /// A discarded region is a *fact*, so it is not a census hole. The compiler
    /// reported on this JSX and said the value is deleted; turning that into an
    /// uncertifiable obligation would claim something is missing when nothing
    /// is.
    #[test]
    fn a_discarded_region_closes_the_census_gap_rather_than_being_one() {
        let source = "const view = <span children={ignored()}>{visible()}</span>;\n";
        let read = span_of(source, "ignored()");
        let mut census = execution_map();
        census.source_hash = SourceHash::of(source);
        census
            .jsx_operations
            .push(solid_facts::compiler::JsxOperation {
                span: span_of(source, "ignored()"),
                kind: "dynamic-attribute".into(),
            });
        census.discarded_regions.push(ExecutionRegion {
            span: span_of(source, "ignored()"),
            reason: RegionReason::JsxAttribute,
        });

        assert!(!missing_jsx_census(
            &file(source, census),
            read,
            ExecutionRole::UntrackedRendering
        ));
    }

    #[test]
    fn conditional_context_does_not_repeat_the_function_name_as_a_return_kind() {
        assert_eq!(
            format_read_context("ConditionalReturn", true),
            "ConditionalReturn while evaluating a condition"
        );
        assert_eq!(
            format_read_context("", true),
            "while evaluating a condition"
        );
    }
}
