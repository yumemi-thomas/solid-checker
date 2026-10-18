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
    execution_role_where(facts, span, allowed, None, |_| true)
}

fn execution_role_where(
    facts: &solid_facts::compiler::ExecutionMap,
    span: Span,
    allowed: &[Span],
    within: Option<Span>,
    callback_applies: impl Fn(&solid_facts::compiler::CallbackRole) -> bool,
) -> ExecutionRole {
    // Only facts strictly inside `within` place `span`, when given.
    let inside =
        |region: Span| within.is_none_or(|within| within.contains(region) && within != region);
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
        (region.span.contains(span) && inside(region.span)).then_some((
            region.span,
            ExecutionRole::TrackedJsx,
            2_u8,
        ))
    });
    let untracked = facts.untracked_regions.iter().filter_map(|region| {
        (region.span.contains(span) && inside(region.span)).then_some((
            region.span,
            ExecutionRole::UntrackedRendering,
            1_u8,
        ))
    });
    let callbacks = facts.callback_roles.iter().filter_map(|callback| {
        (callback.span.contains(span) && inside(callback.span) && callback_applies(callback))
            .then_some((
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
    // A component's function child is projected `Deferred` because the
    // receiving component, not the compiler, invokes it
    // (`solid_facts::compiler`): that says nothing about when, and neither
    // does a tracked region around the element. Like an attribute's
    // function, it is placed by the arms that follow the value into the
    // component, or stays unproven; only facts inside it place code there.
    let within = file
        .ast
        .functions_body_containing(span)
        .filter(|function| component_child_function(file, function.span))
        .min_by_key(|function| function.span.end - function.span.start)
        .map(|function| function.span);
    execution_role_where(&file.compiler, span, allowed, within, |callback| {
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
    // ADR 0315: write legality keeps the role it had before the use-site arm;
    // that arm places only reads.
    let direct = semantic_execution_role_within(
        file,
        span,
        allowed,
        entities,
        symbol_names,
        lookup,
        &mut HashSet::from([write_classification_marker()]),
    );
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
                    prop,
                    &[],
                    lookup,
                    &mut HashSet::new(),
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

/// [`prop_literal_invoked_during_render`] for a function written as a
/// component's sole child, which the component receives as `children`. A
/// `children` attribute competes with it and a spread may be merged around
/// it, so both refuse.
fn child_literal_invoked_during_render(
    file: &solid_facts::FileFacts,
    literal: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let Some(element) = file.ast.jsx_containing(literal).find(|element| {
        !intrinsic_element(file, element)
            && element
                .children
                .iter()
                .any(|child| jsx_child_expression(file, *child) == Some(literal))
    }) else {
        return false;
    };
    if !element.spreads.is_empty()
        || jsx_children(file, element).len() != 1
        || element
            .attributes
            .iter()
            .any(|attribute| file.source_text(attribute.name) == Some("children"))
    {
        return false;
    }
    let Some((component_file, component)) =
        lookup.function_called_at(file.path.as_str(), element.name.span)
    else {
        return false;
    };
    component_invokes_prop_in_body(component_file, component, "children", lookup)
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
    // Every use of `props` in the component must be a static member read
    // that is not written, deleted, or called (a call hands `props` over as
    // `this`), except the call of `prop` itself; and no member named `prop`
    // (or a computed one) may be written or deleted on any object. Anything
    // else -- an alias, a spread, an argument (a single-source `merge(props)`
    // is `props`), a destructuring -- may replace `props.<prop>` before the
    // call, which then runs something else.
    let peel = |span: Span| file.ast.peel_ts_sugar_span(span);
    let written = |member: Span| {
        file.ast
            .assignments
            .iter()
            .any(|assignment| assignment.target.contains(member))
            || file
                .ast
                .deleted_targets
                .iter()
                .any(|target| target.contains(member))
    };
    if file
        .ast
        .reference_declarations
        .iter()
        .filter(|(reference, declaration)| *declaration == name.span && *reference != name.span)
        .any(|(reference, _)| {
            !file.ast.members.iter().any(|member| {
                peel(member.object) == *reference
                    && !written(member.span)
                    && member_static_key(file, member).is_some_and(|key| {
                        key == prop
                            || !(file
                                .ast
                                .calls
                                .iter()
                                .any(|call| peel(call.callee) == member.span)
                                || flow_tagged_or_chained(file, member.span))
                    })
            })
        })
        || file.ast.members.iter().any(|member| {
            component.span.contains(member.span)
                && written(member.span)
                && member_static_key(file, member).is_none_or(|key| key == prop)
        })
    {
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
/// - a spread onto a project component that satisfies this proof, or onto a
///   dialect component that renders the intrinsic element a string prop
///   names, when that prop is proven a string literal for this rendering
///   chain (ADR 0313: Kobalte's `Polymorphic` renders
///   `<Dynamic {...others} component={props.as}/>`, and `as` is the
///   `"button"` its `Button` writes unless a caller passes one); or
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
    shape: &PropsShape,
    lookup: &SemanticLookup<'_>,
    visiting: &mut HashSet<(String, Span, String)>,
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
    // Track the active path, not completed sibling proofs. The same component
    // prop can be forwarded twice without being a recursive cycle.
    let mut visiting = visiting.clone();
    if !visiting.insert((file.path.to_string(), function.span, prop.to_owned())) {
        return false;
    }
    let mut aliases: Vec<PropsView> = vec![(symbol.clone(), name.span, shape.clone())];
    let mut followed = 0;
    let mut references = 0usize;
    while let Some((alias, declaration, alias_shape)) = aliases.get(followed).cloned() {
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
                        member.span,
                        &aliases,
                        lookup,
                        &mut visiting,
                    )
            } else if let Some(view) = props_view_call(file, reference, lookup) {
                match spread_onto(file, view) {
                    Some(element) => spread_reaches_only_events(
                        file,
                        element,
                        prop,
                        &aliases,
                        lookup,
                        &mut visiting,
                    ),
                    None => match const_binding_of(file, view, lookup) {
                        Some((binding, binding_declaration)) => {
                            let view_shape = view_call_shape(file, view, &aliases, lookup);
                            aliases.push((binding, binding_declaration, view_shape));
                            true
                        }
                        None => false,
                    },
                }
            } else if let Some(element) = spread_onto(file, reference) {
                spread_reaches_only_events(file, element, prop, &aliases, lookup, &mut visiting)
            } else if let Some((binding, binding_declaration)) =
                const_binding_of(file, reference, lookup)
            {
                // `const rest = props`, `const root = view as Props`: the same
                // object under another name.
                aliases.push((binding, binding_declaration, alias_shape.clone()));
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
    member: Span,
    views: &[PropsView],
    lookup: &SemanticLookup<'_>,
    visiting: &mut HashSet<(String, Span, String)>,
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
            component_prop_reaches_only_events(file, element, next, views, lookup, visiting)
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
            if member_in_exclusive_named_event_handler(file, member, views, lookup, visiting) {
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
                if component_prop_reaches_only_events(file, element, prop, views, lookup, visiting)
                {
                    return true;
                }
            }
            false
        }
    }
}

/// A local arrow whose complete reference census consists only of event
/// attributes or exact component props that reach events. One use is insufficient:
/// an eager call or an escaped alias could invoke the prop during rendering.
fn member_in_exclusive_named_event_handler(
    file: &solid_facts::FileFacts,
    member: Span,
    views: &[PropsView],
    lookup: &SemanticLookup<'_>,
    visiting: &mut HashSet<(String, Span, String)>,
) -> bool {
    use solid_facts::ast::{FunctionKind, IdentifierRole};
    let Some(handler) = containing_ast_function(&file.ast, member) else {
        return false;
    };
    if handler.kind != FunctionKind::Arrow
        || handler.r#async
        || handler.generator
        || containing_ast_function(&file.ast, handler.span).is_none()
        || lookup.function_component_status(file, handler) != ComponentStatus::No
        || !lookup.function_value_is_current(file, handler)
        || file.ast.identifiers.iter().any(|id| {
            id.role == IdentifierRole::Reference && file.source_text(id.span) == Some("eval")
        })
    {
        return false;
    }
    let Some(binding) = function_binding_name(file, handler) else {
        return false;
    };
    let Some(symbol) = lookup.entities().at(file.path.as_str(), binding.span) else {
        return false;
    };
    if file.ast.jsx_elements.iter().any(|element| {
        lookup.entities().at(file.path.as_str(), element.name.span) == Some(symbol)
            || file.ast.reference_declaration(element.name.span) == Some(binding.span)
    }) {
        return false;
    }
    // Include undemanded AST references as well as the Type Facts index, and
    // require both sources to agree on the exact local declaration.
    let mut references = file
        .ast
        .identifiers
        .iter()
        .filter(|id| {
            id.role == IdentifierRole::Reference
                && file.ast.reference_declaration(id.span) == Some(binding.span)
        })
        .map(|id| id.span)
        .collect::<HashSet<_>>();
    for reference in lookup.symbol_references(symbol.as_str()) {
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
        if reference == binding.span {
            continue;
        }
        if file.ast.reference_declaration(reference) != Some(binding.span) {
            return false;
        }
        references.insert(reference);
    }
    !references.is_empty()
        && references.iter().all(|reference| {
            file.compiler.callback_roles.iter().any(|callback| {
                callback.role == solid_facts::compiler::CallbackRoleKind::EventHandler
                    && intrinsic_attribute_value(file, callback.span)
                    && file.ast.peel_ts_sugar_span(callback.span) == *reference
            }) || component_prop_literal(file, *reference).is_some_and(|(element, prop)| {
                component_prop_reaches_only_events(file, element, prop, views, lookup, visiting)
            })
        })
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
    element: &solid_facts::ast::JsxElementFact,
    prop: &str,
    views: &[PropsView],
    lookup: &SemanticLookup<'_>,
    visiting: &mut HashSet<(String, Span, String)>,
) -> bool {
    if intrinsic_element(file, element) {
        return prop.starts_with("on") && !prop.contains(':');
    }
    component_prop_reaches_only_events(file, element, prop, views, lookup, visiting)
}

/// Whether the component `element` renders runs its prop `prop` only after
/// the render: a project component whose every use of it is proven
/// ([`prop_reaches_only_events`]). A package component is never proven.
fn component_prop_reaches_only_events(
    file: &solid_facts::FileFacts,
    element: &solid_facts::ast::JsxElementFact,
    prop: &str,
    views: &[PropsView],
    lookup: &SemanticLookup<'_>,
    visiting: &mut HashSet<(String, Span, String)>,
) -> bool {
    // ADR 0313: a dialect component that renders the intrinsic element a
    // string prop names (`<Dynamic component="button" {...rest}/>`) assigns
    // every other prop as a spread onto that element does. Only when the
    // prop's value on this element is proven a string literal.
    if let Some(tag) = lookup.intrinsic_tag_prop(file, element.name.span) {
        return prop != tag
            && prop.starts_with("on")
            && !prop.contains(':')
            && element_prop_is_string(file, element, tag, views);
    }
    if let Some((next_file, next_function)) =
        lookup.function_called_at(file.path.as_str(), element.name.span)
    {
        let shape = element_props_shape(file, element, views);
        return prop_reaches_only_events(next_file, next_function, prop, &shape, lookup, visiting);
    }
    false
}

/// ADR 0313: what one proof of [`prop_reaches_only_events`] knows about the
/// keys of one props object, or one view of it, along the rendering chain it
/// follows. Every key the object can hold, each with whether its value is
/// exactly a string literal; `None` when a key the proof cannot name may be
/// present.
///
/// The chain starts at the element whose prop literal is being placed: a
/// project element's props are exactly its attributes, its spreads and its
/// children, so with no spread of an unknown object they are closed. Each
/// hop keeps that knowledge only through the dialect's props views
/// ([`view_call_shape`]) and `const` aliases, and loses it at anything else.
/// Adding a key, or marking a value not a string, only ever withholds a
/// proof.
type PropsShape = Option<std::collections::BTreeMap<String, bool>>;

/// A props object or a view of it in the component a proof walks: its
/// symbol, its declaration and its [`PropsShape`].
type PropsView = (SymbolId, Span, PropsShape);

/// The [`PropsShape`] of the view whose binding `reference` (after
/// transparent TypeScript wrappers) names, when it is one of `views`.
fn view_shape_at<'v>(
    file: &solid_facts::FileFacts,
    views: &'v [PropsView],
    reference: Span,
) -> Option<&'v PropsShape> {
    let declaration = file
        .ast
        .reference_declaration(file.ast.peel_ts_sugar_span(reference))?;
    views
        .iter()
        .find(|(_, view, _)| *view == declaration)
        .map(|(_, _, shape)| shape)
}

/// The [`PropsShape`] of the props `element` renders with, in attribute
/// order: an attribute sets its key, a spread of one of `views` sets that
/// view's keys over what came before, and any other spread opens the shape.
/// Children add `children`. A value is a string literal only when written as
/// one (`as="button"`) or read as a static member of a view whose shape
/// proves it one (`component={props.as}`).
fn element_props_shape(
    file: &solid_facts::FileFacts,
    element: &solid_facts::ast::JsxElementFact,
    views: &[PropsView],
) -> PropsShape {
    let mut keys = std::collections::BTreeMap::new();
    for item in element_props_in_order(element) {
        match item {
            Ok(attribute) => {
                // A namespaced attribute's key is not its spelling here.
                if attribute.namespace.is_some() {
                    return None;
                }
                let name = file.source_text(attribute.name)?;
                keys.insert(
                    name.to_owned(),
                    attribute_value_is_string(file, attribute, views),
                );
            }
            Err(argument) => {
                let view = view_shape_at(file, views, argument)?.as_ref()?;
                keys.extend(view.iter().map(|(key, string)| (key.clone(), *string)));
            }
        }
    }
    if !element.self_closing || !element.children.is_empty() {
        keys.insert("children".to_owned(), false);
    }
    Some(keys)
}

/// An element's attributes (`Ok`) and spread arguments (`Err`) in source
/// order, which is the order the compiled props object takes them in.
fn element_props_in_order(
    element: &solid_facts::ast::JsxElementFact,
) -> Vec<Result<&solid_facts::ast::JsxAttributeFact, Span>> {
    let mut items = element
        .attributes
        .iter()
        .map(|attribute| (attribute.span.start, Ok(attribute)))
        .chain(
            element
                .spreads
                .iter()
                .map(|spread| (spread.span.start, Err(spread.argument))),
        )
        .collect::<Vec<_>>();
    items.sort_by_key(|(start, _)| *start);
    items.into_iter().map(|(_, item)| item).collect()
}

/// Whether the prop `key` of the props `element` renders with is exactly a
/// string literal: the last attribute or spread that can set it does so with
/// one. A later attribute settles the key over an unknown spread before it
/// (`<Dynamic {...rest} component="a"/>`); an unknown spread after one opens
/// it again.
fn element_prop_is_string(
    file: &solid_facts::FileFacts,
    element: &solid_facts::ast::JsxElementFact,
    key: &str,
    views: &[PropsView],
) -> bool {
    let mut string = false;
    for item in element_props_in_order(element) {
        match item {
            Ok(attribute) => {
                if attribute.namespace.is_some() {
                    string = false;
                } else if file.source_text(attribute.name) == Some(key) {
                    string = attribute_value_is_string(file, attribute, views);
                }
            }
            Err(argument) => match view_shape_at(file, views, argument).and_then(Option::as_ref) {
                Some(view) => {
                    if let Some(value) = view.get(key) {
                        string = *value;
                    }
                }
                None => string = false,
            },
        }
    }
    string
}

/// Whether an attribute's value is exactly a string literal: written as one,
/// or a static member of a view whose shape proves that key one.
fn attribute_value_is_string(
    file: &solid_facts::FileFacts,
    attribute: &solid_facts::ast::JsxAttributeFact,
    views: &[PropsView],
) -> bool {
    use solid_facts::ast::JsxAttributeValueKind;
    match attribute.value_kind {
        JsxAttributeValueKind::String => true,
        JsxAttributeValueKind::Expression => attribute.expression.is_some_and(|expression| {
            let expression = file.ast.peel_ts_sugar_span(expression);
            file.ast
                .members
                .iter()
                .find(|member| member.span == expression)
                .is_some_and(|member| {
                    file.source_text(member.property).is_some_and(|key| {
                        view_shape_at(file, views, member.object)
                            .and_then(Option::as_ref)
                            .is_some_and(|shape| shape.get(key) == Some(&true))
                    })
                })
        }),
        _ => false,
    }
}

/// The [`PropsShape`] of the props view `call` makes
/// ([`props_view_call`]): an `omit` of one view by string-literal keys drops
/// them; a `merge` of views and complete object literals takes each source's
/// keys over the earlier ones'. Any other argument opens the shape.
fn view_call_shape(
    file: &solid_facts::FileFacts,
    call: Span,
    views: &[PropsView],
    lookup: &SemanticLookup<'_>,
) -> PropsShape {
    use solid_facts::ast::ArgumentLiteralFact;
    let call = file.ast.call_at(call)?;
    let primitive = lookup.primitive_at_call(file, call.span)?;
    if call.arguments.iter().any(|argument| argument.spread) {
        return None;
    }
    if lookup.dialect.splits_props(primitive) {
        let (source, hidden) = call.arguments.split_first()?;
        let mut keys = view_shape_at(file, views, source.span)?.clone()?;
        for key in hidden {
            let ArgumentLiteralFact::String(key) = &key.literal_value else {
                return None;
            };
            keys.remove(key.as_str());
        }
        return Some(keys);
    }
    if !lookup.dialect.merges_props_reactivity(primitive) {
        return None;
    }
    let mut keys = std::collections::BTreeMap::new();
    for argument in &call.arguments {
        if let Some(view) = view_shape_at(file, views, argument.span) {
            keys.extend(view.clone()?);
        } else if let ArgumentLiteralFact::Object(properties) = &argument.literal_value {
            // Every property a data property with a static key; a getter, a
            // spread or a computed key makes the fact `Unknown`.
            for property in properties {
                keys.insert(
                    property.name.to_string(),
                    matches!(property.value, ArgumentLiteralFact::String(_)),
                );
            }
        } else {
            return None;
        }
    }
    Some(keys)
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
/// Whether `span` sits in a function, inside `region`, that is written as the
/// whole child expression of a component element: the component receives it
/// as `children` and decides when it runs. An intrinsic element's function
/// child is not one; the compiler inserts and runs it.
fn component_child_function_within(
    file: &solid_facts::FileFacts,
    region: Span,
    span: Span,
) -> bool {
    file.ast.functions_body_containing(span).any(|function| {
        region.contains(function.span) && component_child_function(file, function.span)
    })
}

/// Whether `function` is written as the whole child expression of a
/// component element (`<Kids>{(x) => …}</Kids>`).
fn component_child_function(file: &solid_facts::FileFacts, function: Span) -> bool {
    file.ast
        .functions
        .iter()
        .any(|candidate| candidate.span == function)
        && file.ast.jsx_containing(function).any(|element| {
            !intrinsic_element(file, element)
                && element
                    .children
                    .iter()
                    .any(|child| jsx_child_expression(file, *child) == Some(function))
        })
}

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
    if let Some((callee_file, callee, exact)) =
        project_callee_at_parameter(file, call, index, lookup)
    {
        return !(direct
            && exact
            && invokes_parameter_during_call(callee_file, callee, index, lookup));
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
    true
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
/// ADR 0320: whether the code at `span` runs after an await its async
/// function reaches on every path (an unconditional await before it in the
/// same function): an ownerless continuation on a later task, never in a
/// render window or a tracking scope.
pub(crate) fn follows_unconditional_await(file: &solid_facts::FileFacts, span: Span) -> bool {
    containing_ast_function(&file.ast, span).is_some_and(|function| {
        function.r#async
            && file.ast.unconditional_awaits.iter().any(|awaited| {
                awaited.end <= span.start
                    && function.body.contains(*awaited)
                    && containing_ast_function(&file.ast, *awaited)
                        .is_some_and(|owner| owner.span == function.span)
            })
    })
}

/// The functions whose body holds `span`, innermost first and short of the
/// first component, that take their role from a named callback position
/// (`onClick={press}`, `untrack(read)`): the role [`named_callback_roles`]
/// admits for them describes that position, not a direct call of the same
/// function, so it holds for `span` only when every invocation does.
fn named_callbacks_within_component<'f>(
    file: &'f solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> Vec<&'f solid_facts::ast::FunctionFact> {
    let roles = lookup.named_callback_roles(file);
    let mut enclosing = file.ast.functions_body_containing(span).collect::<Vec<_>>();
    // Innermost first: a function nested in another starts later.
    enclosing.sort_by_key(|function| std::cmp::Reverse(function.span.start));
    enclosing
        .into_iter()
        .take_while(|function| {
            lookup.function_component_status(file, function) == ComponentStatus::No
        })
        .filter(|function| roles.get(function.span).admitted)
        .collect()
}

/// ADR 0320: whether every reference to the named function `function` is a
/// position that admits it as a named callback -- an event-handler
/// attribute value, or an argument of a known primitive call -- so the role
/// those positions give it is the role of every invocation. A direct call,
/// any other use, an export, a reference in another file or a direct `eval`
/// in the file refuses. Answers for open programs too, where ADR 0319 asks
/// nothing: the census is of one file-local binding.
fn named_callback_census_holds(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
) -> bool {
    use solid_facts::ast::IdentifierRole;
    let Some(binding) = crate::owners::function_binding_name(file, function) else {
        return false;
    };
    if crate::owners::own_method_name(file, function).is_some()
        || flow_binding_exported(file, binding.span)
        || file.ast.identifiers.iter().any(|id| {
            id.role == IdentifierRole::Reference && file.source_text(id.span) == Some("eval")
        })
    {
        return false;
    }
    let Some(symbol) = lookup.entities().at(file.path.as_str(), binding.span) else {
        return false;
    };
    let mut references = file
        .ast
        .identifiers
        .iter()
        .filter(|id| {
            id.role == IdentifierRole::Reference
                && file.ast.reference_declaration(id.span) == Some(binding.span)
        })
        .map(|id| id.span)
        .collect::<HashSet<_>>();
    for reference in lookup.symbol_references(symbol.as_str()) {
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
        if reference != binding.span {
            if file.ast.reference_declaration(reference) != Some(binding.span) {
                return false;
            }
            references.insert(reference);
        }
    }
    if references.is_empty() {
        return false;
    }
    let primitives = lookup.primitives(file);
    references.into_iter().all(|reference| {
        file.compiler.callback_roles.iter().any(|callback| {
            callback.role == solid_facts::compiler::CallbackRoleKind::EventHandler
                && callback.span == reference
        }) || file.ast.calls.iter().enumerate().any(|(index, call)| {
            known_primitive(&primitives.calls[index]).is_some_and(|primitive| {
                call.arguments
                    .iter()
                    .enumerate()
                    .any(|(argument_index, argument)| {
                        !argument.spread
                            && file.ast.peel_ts_sugar_span(argument.span) == reference
                            && crate::owners::callback_execution_at_call(
                                file,
                                call,
                                primitive,
                                argument_index,
                                lookup,
                            )
                            .is_some()
                    })
            })
        })
    })
}

/// ADR 0320: whether a function a named callback position admits is invoked
/// only in the role it is admitted with: its ADR 0319 invocation-flow answer
/// is an excluded role, or its file-local reference census holds.
fn named_callback_invocations_hold(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
    flow_answers: bool,
) -> bool {
    (flow_answers
        && lookup.program_closed
        && lookup
            .invocation_flow_answers()
            .get(&(file.path.to_string(), function.span))
            .is_some_and(|answer| flow_safe_role(*answer)))
        || named_callback_census_holds(file, function, lookup)
}

/// ADR 0320: whether an excluded execution role at `span` (an event, a
/// deferred callback, a tracked position, deleted code) also holds for every
/// invocation of the functions around it. A role read off a named
/// function's callback references says nothing about a direct call of the
/// same function -- `press()` in a component body beside `onClick={press}`
/// -- and a dispatch obligation, unlike a read, is not re-raised at that
/// call. So each function between `span` and its component that a named
/// callback position admits must have an ADR 0319 invocation-flow answer
/// that is itself an excluded role; any other role is read off the position
/// the code is written in, which every invocation shares.
pub(crate) fn excluded_role_holds(
    file: &solid_facts::FileFacts,
    span: Span,
    role: ExecutionRole,
    lookup: &SemanticLookup<'_>,
) -> bool {
    match role {
        ExecutionRole::DiscardedRendering => true,
        ExecutionRole::TrackedJsx
        | ExecutionRole::EventCallback
        | ExecutionRole::DeferredCallback => named_callbacks_within_component(file, span, lookup)
            .into_iter()
            .all(|function| named_callback_invocations_hold(file, function, lookup, true)),
        _ => false,
    }
}

/// ADR 0320: whether the code at `span` cannot run in a render window or a
/// tracking scope because of an await: it follows an await its async function
/// reaches on every path, or it may follow one and every invocation of that
/// function runs as an event, a deferred callback or a tracked computation
/// (ADR 0319) -- so it runs either in that role, before the await, or on a
/// later task after it.
pub(crate) fn await_continuation_decides_nothing(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    if follows_unconditional_await(file, span) {
        return true;
    }
    let Some(function) = containing_ast_function(&file.ast, span) else {
        return false;
    };
    function.r#async
        && follows_await_in_async_function(file, span)
        && lookup.program_closed
        && lookup
            .invocation_flow_answers()
            .get(&(file.path.to_string(), function.span))
            .is_some_and(|role| flow_safe_role(*role))
}

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
    let role = semantic_role_before_awaits(
        file,
        span,
        allowed,
        entities,
        symbol_names,
        lookup,
        classifying,
    );
    // ADR 0323: code after an await its async function reaches on every path
    // runs from a promise continuation, on a later task. The strict-read
    // window is the synchronous extent of `untrack(fn, label)` (`strictRead`
    // is restored in its `finally`, `@solidjs/signals@2.0.0-rc.13`
    // `dist/dev-shared.js:5801-5885`), so a read there has no window and no
    // tracking scope: none of the reporting roles describes it. Only
    // statement-level awaits count (`unconditional_awaits`); a skippable one
    // (`x ??= await p`, an optional-call argument, a labelled break) keeps
    // the role its position earned.
    if role.reports_untracked_read() && follows_unconditional_await(file, span) {
        return ExecutionRole::Unknown;
    }
    role
}

fn semantic_role_before_awaits(
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
    // ADR 0323 (review round 10): a callback that runs synchronously at its
    // call with the listener cleared but the strict-read window untouched
    // (`createRoot`, `runWithOwner`) reports where the call does. Reads only:
    // a write directly in a created root is `CreatedRoot`'s question, decided
    // before this function is asked.
    if !classifying.contains(&write_classification_marker())
        && let Some(call) = file
            .ast
            .arguments_containing(span)
            .find_map(|(call, index)| {
                call_primitive_name(file, call, entities, symbol_names, dialect)
                    .as_ref()
                    .and_then(PrimitiveName::primitive)
                    .filter(|primitive| {
                        dialect.callback_inherits_calling_window(*primitive, index)
                            && callback_execution_at_call(file, call, *primitive, index, lookup)
                                .is_some()
                            && direct_callback_contains(file, call.arguments[index].span, span)
                    })
                    .map(|_| call)
            })
    {
        let at_call = semantic_execution_role_within(
            file,
            call.callee,
            allowed,
            entities,
            symbol_names,
            lookup,
            classifying,
        );
        return match at_call {
            // The listener is cleared: the read subscribes nothing of the
            // computation the call sits in, and no window is open there.
            ExecutionRole::TrackedJsx => ExecutionRole::DeferredCallback,
            // One-shot module evaluation is no component window, and the
            // role before this ADR was the silent deferred one.
            ExecutionRole::ModuleInitialization => ExecutionRole::Unknown,
            other => other,
        };
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
        // A function written as a component's child expression
        // (`<Kids>{(x) => …}</Kids>`) is the `children` prop's value, handed
        // to the component exactly as an attribute's is; the region lowering
        // that child does not run it. Such a region places nothing, and the
        // flow and lexical arms below decide where the function runs.
        let mut regions = file
            .compiler
            .tracked_regions
            .iter()
            .filter(|region| {
                region.span.contains(span)
                    && !component_child_function_within(file, region.span, span)
            })
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
    if let Some(role) = callee_use_site_role(file, span, lookup, classifying) {
        return role;
    }
    if let Some(role) = invocation_flow_role(file, span, lookup, classifying) {
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

/// The `classifying` entry that marks a write-legality classification, which
/// the ADR 0315 arm does not answer.
fn write_classification_marker() -> (String, Span) {
    ("\0write-legality".to_owned(), Span::new(0, 0))
}

/// ADR 0315: code directly in a function literal that a project callee runs
/// only through one parameter, every call of which is in one proven role,
/// takes that role.
///
/// The literal is exactly one of: the argument at a slot of an exact project
/// callee; the value of a data or method property of a closed object literal
/// (no getter, spread, computed or duplicate key) that is exactly such an
/// argument; or the value of a project component's prop. The callee
/// ([`callee_parameter_use_role`]) uses the parameter only to read other
/// properties and to call the literal's: then the literal runs only where
/// those calls run. Each call's own role is classified in the callee's file,
/// not by where the literal is written, and must be an event, a deferred
/// callback or a tracked computation, the same for every call; that role is
/// the literal's. Anything else answers nothing here. Write legality never
/// asks this arm.
fn callee_use_site_role(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
    classifying: &mut HashSet<(String, Span)>,
) -> Option<ExecutionRole> {
    if classifying.contains(&write_classification_marker()) {
        return None;
    }
    let literal = containing_ast_function(&file.ast, span)?;
    if !literal.body.contains(span)
        || literal.r#async
        || literal.generator
        || lookup.function_component_status(file, literal) != ComponentStatus::No
    {
        return None;
    }
    let (callee_file, callee, index, field, siblings) = literal_use_site(file, literal, lookup)?;
    let key = (file.path.to_string(), literal.span);
    if !classifying.insert(key.clone()) {
        return None;
    }
    let receiver_safe = |name: &str| sibling_ignores_receiver(file, siblings, name, lookup);
    let role = callee_parameter_use_role(
        callee_file,
        callee,
        index,
        field.as_deref(),
        &receiver_safe,
        lookup,
        classifying,
    );
    classifying.remove(&key);
    role
}

/// Where a function literal is handed to a project callee, for
/// [`callee_use_site_role`]: the callee, the parameter slot, and the property
/// of that parameter the literal is (`None` for the parameter itself).
fn literal_use_site<'a>(
    file: &solid_facts::FileFacts,
    literal: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'a>,
) -> Option<(
    &'a solid_facts::FileFacts,
    &'a solid_facts::ast::FunctionFact,
    usize,
    Option<String>,
    Siblings,
)> {
    let exact_argument = |value: Span| {
        file.ast.arguments_containing(value).find(|(call, index)| {
            let argument = &call.arguments[*index];
            !argument.spread
                && !call.construct
                && file.ast.peel_ts_sugar_span(argument.span) == value
                && lookup.primitive_at_call(file, call.span).is_none()
        })
    };
    let at_argument = |value: Span| {
        let (call, index) = exact_argument(value)?;
        let (callee_file, callee, exact) = project_callee_at_parameter(file, call, index, lookup)?;
        exact.then_some((callee_file, callee, index))
    };
    // The literal itself is the argument.
    if let Some((callee_file, callee, index)) = at_argument(literal.span) {
        return Some((callee_file, callee, index, None, Siblings::None));
    }
    // A property of an options object that is the argument.
    let property = file.ast.object_properties.iter().find(|property| {
        !property.computed && file.ast.peel_ts_sugar_span(property.value) == literal.span
    });
    let key = match (property, crate::owners::own_method_name(file, literal)) {
        (Some(property), _) => property.key,
        // An object literal's own method, not a class's.
        (None, Some(name))
            if file
                .ast
                .object_properties
                .iter()
                .any(|property| property.key == name.span) =>
        {
            name.span
        }
        (None, Some(_)) => return None,
        (None, None) => {
            // A project component's prop. A spread may carry anything, and a
            // non-arrow function value receives the props object as `this`
            // and could run another prop out of the callee's sight.
            let (element, prop) = component_prop_literal(file, literal.span)?;
            if !element.spreads.is_empty()
                || element.attributes.iter().any(|attribute| {
                    attribute
                        .expression
                        .is_some_and(|value| receiver_function_at(file, value))
                })
            {
                return None;
            }
            let (callee_file, callee) =
                lookup.function_called_at(file.path.as_str(), element.name.span)?;
            return Some((
                callee_file,
                callee,
                0,
                Some(prop.to_owned()),
                Siblings::Element(element.span),
            ));
        }
    };
    let object = file
        .ast
        .object_get_shapes
        .iter()
        .filter(|shape| shape.span.contains(key))
        .min_by_key(|shape| shape.span.end - shape.span.start)?;
    let name = static_key_text(file.source_text(key)?, false)?;
    let name = name.as_str();
    if !object.closed
        || object
            .properties
            .iter()
            .any(|property| property.getter.is_some())
        || !object
            .properties
            .iter()
            .any(|property| property.key.as_str() == name)
    {
        return None;
    }
    // A method or function expression in the literal receives the object as
    // `this` and could run a property out of the callee's sight.
    if file.ast.object_properties.iter().any(|property| {
        object.span.contains(property.span)
            && (property.method || receiver_function_at(file, property.value))
    }) {
        return None;
    }
    let (callee_file, callee, index) = at_argument(object.span)?;
    Some((
        callee_file,
        callee,
        index,
        Some(name.to_owned()),
        Siblings::Object(object.span),
    ))
}

/// The caller-side object a callee's parameter holds, for the receiver
/// question of [`callee_parameter_use_role`]: an options literal, a JSX
/// element's props, or nothing (the literal itself is the parameter).
#[derive(Clone, Copy)]
enum Siblings {
    None,
    Object(Span),
    Element(Span),
}

/// Whether the caller's value for the property `name` of `siblings` cannot use
/// a `this` it is called with: an arrow (or a `const` bound to one) or a
/// non-function literal. An absent property proves nothing: code outside the
/// analyzed program may have installed one on `Object.prototype`
/// (`project-callback-uses`' `SiblingAbsent`).
fn sibling_ignores_receiver(
    file: &solid_facts::FileFacts,
    siblings: Siblings,
    name: &str,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let arrow = |span: Span| {
        file.ast.functions.iter().any(|function| {
            function.span == span && function.kind == solid_facts::ast::FunctionKind::Arrow
        })
    };
    let arrow_value = |value: Span| {
        let value = file.ast.peel_ts_sugar_span(value);
        arrow(value)
            || lookup
                .binding_at_reference(file.path.as_str(), value)
                .is_some_and(|(binding_file, binding, _)| {
                    binding_file.path == file.path
                        && binding.immutable
                        && binding.shape == solid_facts::ast::BindingShape::Identifier
                        && binding.initializer.is_some_and(|initializer| {
                            arrow(file.ast.peel_ts_sugar_span(initializer))
                        })
                })
    };
    match siblings {
        Siblings::None => false,
        Siblings::Object(object) => {
            let direct = file
                .ast
                .object_properties
                .iter()
                .filter(|property| {
                    object.contains(property.span)
                        && !property.computed
                        && property_static_key(file, property).as_deref() == Some(name)
                        && !file.ast.object_get_shapes.iter().any(|inner| {
                            inner.span != object
                                && object.contains(inner.span)
                                && inner.span.contains(property.span)
                        })
                })
                .max_by_key(|property| property.span.start);
            match direct {
                None => false,
                Some(property) => {
                    !property.method
                        && (arrow_value(property.value)
                            || property.value_kind == solid_facts::ast::RuntimeValueKind::Primitive)
                }
            }
        }
        Siblings::Element(element) => {
            let Some(element) = file
                .ast
                .jsx_elements
                .iter()
                .find(|candidate| candidate.span == element)
            else {
                return false;
            };
            match element
                .attributes
                .iter()
                .rev()
                .find(|attribute| file.source_text(attribute.name) == Some(name))
            {
                None => false,
                Some(attribute) => match attribute.value_kind {
                    solid_facts::ast::JsxAttributeValueKind::String
                    | solid_facts::ast::JsxAttributeValueKind::Boolean => true,
                    solid_facts::ast::JsxAttributeValueKind::Expression => {
                        attribute.expression.is_some_and(arrow_value)
                    }
                    _ => false,
                },
            }
        }
    }
}

/// Whether `value` is exactly a function that binds its own `this`: not an
/// arrow.
fn receiver_function_at(file: &solid_facts::FileFacts, value: Span) -> bool {
    let value = file.ast.peel_ts_sugar_span(value);
    file.ast.functions.iter().any(|function| {
        function.span == value && function.kind != solid_facts::ast::FunctionKind::Arrow
    })
}

/// The one role every call of `function`'s parameter `index` (or of its
/// property `field`) runs in, when the parameter is used for nothing else;
/// see [`callee_use_site_role`].
fn callee_parameter_use_role(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    index: usize,
    field: Option<&str>,
    receiver_safe: &dyn Fn(&str) -> bool,
    lookup: &SemanticLookup<'_>,
    classifying: &mut HashSet<(String, Span)>,
) -> Option<ExecutionRole> {
    if function.r#async || function.generator {
        return None;
    }
    let parameter = function.parameters.get(index)?;
    let [name] = parameter.names.as_slice() else {
        return None;
    };
    if parameter.shape != solid_facts::ast::BindingShape::Identifier
        || parameter.initializer.is_some()
        || (function.rest_parameter && index + 1 == function.parameters.len())
        || crate::indexes::binding_written(file, name.span)
        || file.ast.identifiers.iter().any(|identifier| {
            function.span.contains(identifier.span)
                && matches!(
                    file.source_text(identifier.span),
                    Some("eval" | "arguments")
                )
        })
    {
        return None;
    }
    let targets = file
        .ast
        .assignments
        .iter()
        .map(|assignment| assignment.target)
        .chain(file.ast.iteration_targets.iter().copied())
        .collect::<Vec<_>>();
    let mut role = None::<ExecutionRole>;
    let mut calls = 0usize;
    for (reference, declaration) in &file.ast.reference_declarations {
        if *declaration != name.span || *reference == name.span {
            continue;
        }
        if !function.body.contains(*reference) {
            return None;
        }
        let invoked =
            match field {
                None => *reference,
                Some(field) => {
                    let member =
                        file.ast.members.iter().find(|member| {
                            file.ast.peel_ts_sugar_span(member.object) == *reference
                        })?;
                    if file
                        .ast
                        .computed_members
                        .binary_search(&member.span)
                        .is_ok()
                        || targets.iter().any(|target| target.contains(member.span))
                    {
                        return None;
                    }
                    if member_static_key(file, member)? != field {
                        // Reading another property runs nothing of the caller's
                        // literal. Calling it, tagging a template with it or
                        // reaching through it (`p.g.call(p)`) hands the object
                        // over as `this`, and that function -- an inherited one,
                        // for an absent property -- could run this property out
                        // of the callee's sight.
                        let next = file
                            .source
                            .get(member.span.end as usize..)
                            .map(str::trim_start)
                            .unwrap_or_default();
                        let hands_receiver = next.starts_with(['(', '`', '.', '['])
                            || next.starts_with("?.")
                            || file.ast.calls.iter().any(|call| {
                                file.ast.peel_ts_sugar_span(call.callee) == member.span
                            })
                            || file.ast.members.iter().any(|outer| {
                                file.ast.peel_ts_sugar_span(outer.object) == member.span
                            });
                        if hands_receiver && !receiver_safe(&member_static_key(file, member)?) {
                            return None;
                        }
                        continue;
                    }
                    member.span
                }
            };
        let peeled = |span: Span| file.ast.peel_ts_sugar_span(span);
        // Testing the property runs nothing (`props.f ? … : …`,
        // `props.f && …`): a falsy value is not the literal, and a truthy one
        // is not the `&&`'s value.
        if field.is_some()
            && (file
                .ast
                .conditional_expressions
                .iter()
                .any(|conditional| peeled(conditional.test) == invoked)
                || file.ast.logical_expressions.iter().any(|logical| {
                    logical.operator == solid_facts::ast::LogicalOperatorKind::And
                        && peeled(logical.left) == invoked
                }))
        {
            continue;
        }
        // Forwarded as exactly another project component's prop, which this
        // proof places in turn.
        if field.is_some()
            && let Some((element, prop)) = forwarded_component_prop(file, invoked)
        {
            if !element.spreads.is_empty()
                || element.attributes.iter().any(|attribute| {
                    attribute
                        .expression
                        .is_some_and(|value| receiver_function_at(file, value))
                })
            {
                return None;
            }
            let (next_file, next) =
                lookup.function_called_at(file.path.as_str(), element.name.span)?;
            let key = (file.path.to_string(), element.span);
            if !classifying.insert(key.clone()) {
                return None;
            }
            let element_span = element.span;
            let next_safe = |name: &str| {
                sibling_ignores_receiver(file, Siblings::Element(element_span), name, lookup)
            };
            let forwarded = callee_parameter_use_role(
                next_file,
                next,
                0,
                Some(prop),
                &next_safe,
                lookup,
                classifying,
            );
            classifying.remove(&key);
            let forwarded = forwarded?;
            if role.is_some_and(|role| role != forwarded) {
                return None;
            }
            role = Some(forwarded);
            calls += 1;
            continue;
        }
        let call =
            file.ast.calls.iter().find(|call| {
                !call.construct && file.ast.peel_ts_sugar_span(call.callee) == invoked
            })?;
        // Every function between the callee and the call is handed, as itself,
        // to a call or a JSX attribute: no helper, returned or stored closure
        // runs it, and no async function runs it after an `await`.
        for nested in file.ast.functions.iter().filter(|nested| {
            nested.span != function.span
                && function.span.contains(nested.span)
                && nested.span.contains(call.span)
        }) {
            // Handed to a primitive, a standard-library call or an intrinsic
            // element's attribute, whose timing the role below reads; and the
            // call is in its body, not a default initializer that runs only
            // when an argument is left out.
            let handed = file
                .ast
                .arguments_containing(nested.span)
                .any(|(outer, slot)| {
                    !outer.arguments[slot].spread
                        && file.ast.peel_ts_sugar_span(outer.arguments[slot].span) == nested.span
                        && (lookup.primitive_at_call(file, outer.span).is_some()
                            || lookup
                                .resolved_callee_call(file, outer.callee)
                                .and_then(|resolved| resolved.declaration.as_ref())
                                .is_some_and(|declaration| declaration.standard_library))
                })
                || file.ast.jsx_elements.iter().any(|element| {
                    intrinsic_element(file, element)
                        && element.attributes.iter().any(|attribute| {
                            attribute.expression.is_some_and(|value| {
                                file.ast.peel_ts_sugar_span(value) == nested.span
                            })
                        })
                });
            if nested.r#async || nested.generator || !handed || !nested.body.contains(call.span) {
                return None;
            }
        }
        let here = semantic_execution_role_within(
            file,
            call.callee,
            &[],
            lookup.entities(),
            lookup.symbol_names(),
            lookup,
            classifying,
        );
        if !matches!(
            here,
            ExecutionRole::EventCallback
                | ExecutionRole::DeferredCallback
                | ExecutionRole::TrackedJsx
        ) || role.is_some_and(|role| role != here)
        {
            return None;
        }
        role = Some(here);
        calls += 1;
    }
    (calls > 0).then_some(role).flatten()
}

/// The project component element and prop name `value` is exactly the
/// value of, when it is one ([`component_prop_literal`] for any value).
fn forwarded_component_prop(
    file: &solid_facts::FileFacts,
    value: Span,
) -> Option<(&solid_facts::ast::JsxElementFact, &str)> {
    let (element, attribute) = file.ast.jsx_containing(value).find_map(|element| {
        element
            .attributes
            .iter()
            .find(|attribute| {
                attribute.namespace.is_none()
                    && attribute.value_kind == solid_facts::ast::JsxAttributeValueKind::Expression
                    && attribute
                        .expression
                        .is_some_and(|expression| file.ast.peel_ts_sugar_span(expression) == value)
            })
            .map(|attribute| (element, attribute))
    })?;
    if intrinsic_element(file, element) {
        return None;
    }
    Some((element, file.source_text(attribute.name)?))
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
        None => false,
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
    fresh_stack_callback_call(file, span, lookup).map(|_| ExecutionRole::DeferredCallback)
}

/// The selected scheduler declaration enclosing a literal callback, following
/// only audited inline callback carriers. Consumers that rely on receiver
/// independence must additionally refuse `catch`/`finally`'s receiver `then`.
pub(crate) fn fresh_stack_callback_call<'file, 'lookup>(
    file: &'file solid_facts::FileFacts,
    span: Span,
    lookup: &'lookup SemanticLookup<'_>,
) -> Option<(
    &'file solid_facts::ast::CallFact,
    &'lookup typefacts::ResolvedCall,
)> {
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
                return Some((call, resolved));
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
            // A component's function child is its `children` prop, under the
            // same proof. Only the proof answers here: a control-flow
            // component's children callback is placed below.
            if component_child_function(file, literal.span)
                && child_literal_invoked_during_render(file, literal.span, lookup)
            {
                return false;
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
        if let Some((callee_file, callee, exact)) =
            project_callee_at_parameter(file, call, index, lookup)
        {
            let direct =
                !argument.spread && file.ast.peel_ts_sugar_span(argument.span) == literal.span;
            if !(direct
                && exact
                && invokes_parameter_during_call(callee_file, callee, index, lookup))
            {
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
        return true;
    }
}

/// ADR 0323: whether an anonymous callback around `span`, which no compiler
/// role places, reaches only consumers whose invocation question the read
/// rules already answer with [`callee_callback_timing`]: every function
/// layer between `span` and its component is
///
/// - an IIFE (the literal is the callee of a call), or
/// - an argument (directly, or inside an array or object literal argument) of
///   a call that a project function resolves for, or of a standard-library
///   call the audited rows model as running that argument inline.
///
/// Anything else answers `false`: a component attribute, an unknown or
/// package callee, a primitive, a standard-library call that may keep or
/// schedule the function, and a literal that is merely stored. Those
/// callbacks keep the silence a props or store read had before this ADR.
///
/// The walk mirrors [`callee_callback_timing_within`] on purpose (the same
/// outermost-literal and innermost-call choice), so a read admitted here is
/// the read that function then proves or withholds; it never makes a claim.
pub(crate) fn unclassified_callback_reaches_modelled_consumer(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    use crate::runtime_semantics::RuntimeArgumentBehavior;
    let mut span = span;
    let mut innermost = true;
    loop {
        // A default-parameter initializer runs when its function is called.
        if parameter_default_owner(file, span).is_some() {
            return false;
        }
        let Some(literal) = containing_ast_function(&file.ast, span) else {
            return true;
        };
        // The component the callbacks are written in. An anonymous function
        // that is itself the innermost layer and may be a component is a
        // renderer (`render: (row) => <tr/>`), whose parameter is not a props
        // object: it keeps the silence it had. A layer that is only an
        // uncertain component proves nothing about where its body runs.
        match lookup.function_component_status(file, literal) {
            ComponentStatus::Proven if !innermost => return true,
            ComponentStatus::No => {}
            ComponentStatus::Proven | ComponentStatus::Uncertain => return false,
        }
        innermost = false;
        // A function bound to a name is the interprocedural summary's
        // business, not an anonymous callback.
        if literal.method_name.is_some() || function_binding_name(file, literal).is_some() {
            return false;
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
            // An IIFE runs where it is written.
            let Some(call) = file
                .ast
                .calls
                .iter()
                .filter(|call| file.ast.peel_ts_sugar_span(call.callee) == literal.span)
                .min_by_key(|call| call.span.end - call.span.start)
            else {
                return false;
            };
            span = call.span;
            continue;
        };
        if let Some(primitive) = lookup.primitive_at_call(file, call.span) {
            // A callback that runs synchronously in its call's own window
            // (`createRoot`, `runWithOwner`) is a layer like an IIFE: it runs
            // where the call does.
            if lookup
                .dialect
                .callback_inherits_calling_window(primitive, index)
                && callback_execution_at_call(file, call, primitive, index, lookup).is_some()
                && direct_callback_contains(file, call.arguments[index].span, span)
            {
                span = call.span;
                continue;
            }
            return false;
        }
        if project_callee_at_parameter(file, call, index, lookup).is_some() {
            span = call.span;
            continue;
        }
        let argument = &call.arguments[index];
        let Some(resolved) = lookup
            .resolved_callee_call(file, call.callee)
            .filter(|resolved| {
                resolved
                    .declaration
                    .as_ref()
                    .is_some_and(|declaration| declaration.standard_library)
            })
        else {
            return false;
        };
        let callability = lookup
            .entity_at(file.path.as_str(), argument.span)
            .and_then(|entity| entity.callability);
        if matches!(
            crate::runtime_semantics::argument_behavior(resolved, callability, index),
            Some(RuntimeArgumentBehavior::InlineCallback)
        ) && direct_callback_contains(file, argument.span, span)
        {
            span = call.span;
            continue;
        }
        return false;
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

/// ADR 0278: the project function `call` runs, resolved as the call sites
/// here always have, and whether argument `index` provably lands in that
/// function's parameter `index`. `None` when no project function resolves.
/// `Some((_, false))` when one does but the mapping is not exact:
/// - the callee is a member whose own declaration is not that function
///   (`f.call(thisArg, …)` shifts every argument);
/// - its binding may have been replaced since (ADR 0221);
/// - a spread argument before `index` may fill that parameter.
pub(crate) fn project_callee_at_parameter<'a>(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    index: usize,
    lookup: &SemanticLookup<'a>,
) -> Option<(
    &'a solid_facts::FileFacts,
    &'a solid_facts::ast::FunctionFact,
    bool,
)> {
    // A local method declaration cannot bypass ADR 0299's receiver escape
    // and initialization census when deciding callback timing either.
    if crate::object_members::call_withheld(file, call.callee, lookup) {
        return None;
    }
    if let Some((file, function)) = lookup.object_member_function(file, call.callee) {
        return Some((
            file,
            function,
            !call
                .arguments
                .iter()
                .take(index.saturating_add(1))
                .any(|argument| argument.spread),
        ));
    }
    let declared = lookup
        .callee_symbol(file, call.callee)
        .and_then(|symbol| lookup.function_for_symbol(symbol));
    let (callee_file, callee) =
        declared.or_else(|| lookup.function_called_at(file.path.as_str(), call.callee))?;
    // A member callee resolves to its own declaration above. Reaching a
    // function only through the entity at the whole member span names the
    // receiver's root instead: `f` in `f.call(thisArg, …)`.
    let callee_span = file.ast.peel_ts_sugar_span(call.callee);
    let member = file
        .ast
        .members
        .iter()
        .any(|member| member.span == callee_span)
        || file
            .ast
            .computed_members
            .binary_search(&callee_span)
            .is_ok();
    let exact = !(member && declared.is_none())
        && (!member || member_runs_declared_method(file, callee_span, callee_file, callee, lookup))
        && lookup.function_value_is_current(callee_file, callee)
        && !call
            .arguments
            .iter()
            .take(index.saturating_add(1))
            .any(|argument| argument.spread);
    Some((callee_file, callee, exact))
}

/// ADR 0279: whether a member call `receiver.name(…)` runs the declared
/// method `method`, rather than an override or a replacement. A module
/// namespace member qualifies. Otherwise no assignment anywhere may write a
/// member of that name, and the receiver is a `const` bound to the object
/// literal that holds the method, or to `new C()` of the exact class that
/// declares it.
fn member_runs_declared_method(
    file: &solid_facts::FileFacts,
    callee: Span,
    method_file: &solid_facts::FileFacts,
    method: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
) -> bool {
    // A module namespace member: the namespace object is frozen, and the
    // export binding's own currency is checked by the caller.
    if lookup
        .namespace_member_function(file, callee)
        .is_some_and(|(namespace_file, function)| {
            namespace_file.path == method_file.path && function.span == method.span
        })
    {
        return true;
    }
    let Some(member) = file.ast.members.iter().find(|member| member.span == callee) else {
        return false;
    };
    let Some(name) = file.source_text(member.property) else {
        return false;
    };
    // ADR 0314: a fresh object literal's chain holds only `Object.prototype`.
    // A class instance's is checked with its class below.
    if lookup.member_may_be_reassigned_on_builtin(name) {
        return false;
    }
    let receiver = file.ast.peel_ts_sugar_span(member.object);
    let Some((binding_file, binding, _)) =
        lookup.binding_at_reference(file.path.as_str(), receiver)
    else {
        return false;
    };
    if !binding.immutable || binding.shape != solid_facts::ast::BindingShape::Identifier {
        return false;
    }
    let Some(initializer) = binding.initializer else {
        return false;
    };
    // The receiver must not escape: `Object.assign(bus, …)` or
    // `Object.defineProperty(bus, "run", …)` replaces the method without an
    // assignment (review round 4). Every reference reads a member of it, and
    // no module export exposes it to an importer.
    let Some(declaration) = binding.names.first().map(|name| name.span) else {
        return false;
    };
    let only_member_reads = binding_file
        .ast
        .reference_declarations
        .iter()
        .filter(|(_, bound)| *bound == declaration)
        .all(|(reference, _)| {
            binding_file
                .ast
                .members
                .iter()
                .any(|member| binding_file.ast.peel_ts_sugar_span(member.object) == *reference)
        });
    // Only a module-level binding can be exported: an exported function's
    // span contains its whole body (as in `property_gets::receiver`).
    let module_level = containing_ast_function(&binding_file.ast, declaration).is_none();
    let exported = binding_file.ast.exports.iter().any(|export| {
        (module_level && export.span.contains(declaration))
            || export.specifiers.iter().any(|specifier| {
                binding_file.ast.reference_declaration(specifier.local.span) == Some(declaration)
            })
    });
    if !only_member_reads || exported {
        return false;
    }
    let initializer = binding_file.ast.peel_ts_sugar_span(initializer);
    if binding.initializer_value_kind == solid_facts::ast::RuntimeValueKind::Object {
        return binding_file.path == method_file.path && initializer.contains(method.span);
    }
    let Some(construction) = binding_file
        .ast
        .call_at(initializer)
        .filter(|construction| construction.construct)
    else {
        return false;
    };
    let Some((class_file, class)) = lookup
        .callee_symbol(binding_file, construction.callee)
        .and_then(|symbol| lookup.class_for_symbol(symbol))
    else {
        return false;
    };
    // Nor may the class's prototype be reachable for patching.
    let class_exposed = class.name.as_ref().is_some_and(|class_name| {
        class_file
            .ast
            .reference_declarations
            .iter()
            .filter(|(_, bound)| *bound == class_name.span)
            .any(|(reference, _)| {
                class_file.ast.members.iter().any(|member| {
                    class_file.ast.peel_ts_sugar_span(member.object) == *reference
                        && class_file.source_text(member.property) == Some("prototype")
                }) || class_file.ast.calls.iter().any(|call| {
                    call.arguments.iter().any(|argument| {
                        class_file.ast.peel_ts_sugar_span(argument.span) == *reference
                    })
                })
            })
    });
    class_file.path == method_file.path
        && class.span.contains(method.span)
        && !class_exposed
        && lookup.class_instance_is_exact(class_file, class, name, 4)
        && !lookup.member_may_be_reassigned_on_class(name, class_file, class)
}

/// ADR 0280: how the calls of `function`'s parameter `index` stand on
/// `call`, given what its arguments prove. `Guaranteed` when one of them
/// always runs, `Unreachable` when none can, `None` when the arguments prove
/// nothing or the parameter is written.
pub(crate) fn parameter_call_standing(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    function_file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    index: usize,
    lookup: &SemanticLookup<'_>,
) -> Option<solid_facts::ast::SiteStanding> {
    use solid_facts::ast::SiteStanding;
    let parameter = function.parameters.get(index)?;
    if parameter.shape != solid_facts::ast::BindingShape::Identifier {
        return None;
    }
    let name = parameter.names.first()?;
    if crate::indexes::binding_written(function_file, name.span) {
        return None;
    }
    let symbol = lookup
        .entities()
        .at(function_file.path.as_str(), name.span)?;
    let sites = function_file
        .ast
        .calls
        .iter()
        .filter(|inner| {
            crate::owners::written_directly_in(&function_file.ast, function, inner.span)
                && lookup.callee_symbol(function_file, inner.callee) == Some(symbol.as_str())
        })
        .map(|inner| inner.span)
        .collect::<Vec<_>>();
    if sites.is_empty() {
        return None;
    }
    let values =
        crate::argument_values::parameter_values(file, call, function_file, function, lookup)
            .filter(|values| !values.is_empty())?;
    let standings = solid_facts::ast::argument_site_standings(
        std::path::Path::new(function_file.path.as_str()),
        &function_file.source,
        function.span,
        &values,
        &lookup.never_returning_calls_within(function_file, function),
        &sites,
    )?;
    // Every other reference -- an alias, an argument (`list.forEach(cb)`),
    // a member -- may run the callback too, so ruling out the direct calls
    // proves no absence (review round 4).
    let only_direct_calls = function_file
        .ast
        .reference_declarations
        .iter()
        .filter(|(_, declaration)| *declaration == name.span)
        .all(|(reference, _)| {
            function_file.ast.calls.iter().any(|inner| {
                sites.contains(&inner.span)
                    && function_file.ast.peel_ts_sugar_span(inner.callee) == *reference
            })
        });
    if standings.contains(&SiteStanding::Guaranteed) {
        Some(SiteStanding::Guaranteed)
    } else if only_direct_calls
        && standings
            .iter()
            .all(|standing| *standing == SiteStanding::Unreachable)
    {
        Some(SiteStanding::Unreachable)
    } else {
        Some(SiteStanding::Possible)
    }
}

/// Whether the synchronous project function `function` calls its parameter
/// `index` anywhere written directly in its own body, by the parameter's
/// symbol: the call may run during a call of `function`, on some paths
/// (ADR 0277). [`invokes_parameter_during_call`] is the every-completion
/// subset.
pub(crate) fn calls_parameter_in_body(
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
    let Some(symbol) = parameter
        .names
        .first()
        .and_then(|name| lookup.entities().at(file.path.as_str(), name.span))
    else {
        return false;
    };
    file.ast.calls.iter().any(|call| {
        containing_ast_function(&file.ast, call.span)
            .is_some_and(|owner| owner.span == function.span)
            && lookup.callee_symbol(file, call.callee) == Some(symbol.as_str())
    })
}

/// Whether the synchronous project function `function` calls its parameter
/// `index` on every completion of its own body -- not in a closure it
/// creates, not behind a branch, loop or early exit -- by the parameter's
/// symbol. Destructured, rest and missing parameters prove
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
    if lookup
        .entities()
        .at(file.path.as_str(), name.span)
        .is_none()
    {
        return false;
    }
    // ADR 0277: a call of the parameter on the function's every-completion
    // path, not merely one written in its body. `if (flag) cb()` or a loop
    // body may never call it. The table is bound into the cross-file proof
    // digest, so moving that call invalidates retained caller results.
    lookup.parameter_called_on_every_completion(file.path.as_str(), function.span, index)
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
        // ADR 0323 (review round 10): the slot's *callback* runs on result
        // access; an expression written at the slot (`omit(props, props.k)`)
        // is evaluated by the call, where it is written.
        file.ast
            .functions_within(call.arguments[index].span)
            .any(|function| function.span.contains(span))
            && lookup
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
            let proven = callback_execution_at_call(file, call, primitive, argument_index, lookup)
                .is_some()
                || dialect
                    .effect_apply_defer_options(primitive, argument_index)
                    .is_some();
            // ADR 0323: a callback that runs in its call's own window has no
            // role of its own: it follows the call (the flow walk's `Call`).
            let inherits = dialect.callback_inherits_calling_window(primitive, argument_index);
            let untracked =
                !inherits && dialect.reports_untracked_reads_at(primitive, argument_index, count);
            // The role describes the phase if invoked. ADR 0302 gates every
            // diagnostic on the separate call-site invocation premise.
            let effect_apply =
                effect_apply_argument(dialect, primitive, count) == Some(argument_index);
            let tracked = !is_effect(primitive)
                && dialect
                    .callback_semantics_at(primitive, argument_index, count)
                    .tracks_reads;
            let deferred = !inherits
                && callback_runs_outside_tracking(dialect, primitive, argument_index, count);
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

/// Message-only: join an apply read to a read of the exact same reactive
/// declaration in its paired compute. No name or type-alias matching, and no
/// claim that a conditional compute read necessarily executes on every run.
pub(super) fn apply_compute_reads(
    facts: &solid_facts::ProjectFacts,
    reads: &[crate::ReactiveRead],
    lookup: &SemanticLookup<'_>,
    accessors: &HashMap<SymbolId, (SymbolId, typefacts::Location)>,
    origins: &HashMap<SymbolId, (SymbolId, SymbolId, typefacts::Location)>,
) -> HashSet<typefacts::Location> {
    let callback = |file: &solid_facts::FileFacts, argument: Span| {
        let argument = file.ast.peel_ts_sugar_span(argument);
        file.ast
            .functions
            .iter()
            .find(|function| function.span == argument)
            .map(|function| (file.path.to_string(), function.span))
            .or_else(|| {
                if !file.ast.identifiers.iter().any(|identifier| {
                    identifier.span == argument
                        && identifier.role == solid_facts::ast::IdentifierRole::Reference
                }) {
                    return None;
                }
                lookup
                    .entity_symbol(file, argument)
                    .and_then(|symbol| lookup.function_for_symbol(symbol))
                    .filter(|(file, function)| lookup.function_value_is_current(file, function))
                    .map(|(file, function)| (file.path.to_string(), function.span))
            })
    };
    let mut pairs = HashMap::<(String, Span), Vec<Option<(String, Span)>>>::new();
    for file in &facts.files {
        for call in &file.ast.calls {
            let Some(primitive) = lookup.primitive_at_call(file, call.span) else {
                continue;
            };
            let Some(apply_index) =
                effect_apply_argument(lookup.dialect, primitive, call.arguments.len())
            else {
                continue;
            };
            let Some(compute_index) = (0..call.arguments.len()).find(|index| {
                lookup
                    .dialect
                    .callback_semantics_at(primitive, *index, call.arguments.len())
                    .execution
                    == Some(Execution::Tracked)
            }) else {
                continue;
            };
            let Some(apply) = callback(file, call.arguments[apply_index].span) else {
                continue;
            };
            let compute = callback(file, call.arguments[compute_index].span);
            pairs.entry(apply).or_default().push(compute);
        }
    }
    let function_at = |read: &crate::ReactiveRead| {
        let file = lookup.file_by_path(&read.location.path)?;
        let span = Span::new(
            u32::try_from(read.location.start_byte).ok()?,
            u32::try_from(read.location.end_byte).ok()?,
        );
        containing_ast_function(&file.ast, span)
            .map(|function| (file.path.to_string(), function.span))
    };
    // A declaration site can allocate several sources (two calls of a
    // factory). Correlate immutable getter values, not allocation templates.
    let mut values = HashMap::<SymbolId, Option<(Option<SymbolId>, bool)>>::new();
    for file in &facts.files {
        for binding in file.ast.bindings.iter().filter(|binding| binding.immutable) {
            for name in &binding.names {
                let Some(symbol) = lookup.entities().at(file.path.as_str(), name.span).cloned()
                else {
                    continue;
                };
                let alias = (binding.shape == solid_facts::ast::BindingShape::Identifier)
                    .then_some(binding.initializer_identifier.as_ref())
                    .flatten()
                    .and_then(|name| lookup.entities().at(file.path.as_str(), name.span))
                    .cloned();
                let global = !file.ast.any_function_body_containing(name.span)
                    && !file
                        .ast
                        .classes
                        .iter()
                        .any(|class| class.span.contains(name.span))
                    && !file.ast.is_inside_module_block(name.span);
                if values
                    .insert(symbol.clone(), Some((alias, global)))
                    .is_some()
                {
                    values.insert(symbol, None);
                }
            }
        }
    }
    let value_at =
        |file: &solid_facts::FileFacts, span: Span| {
            let call = file.ast.calls.iter().find(|call| call.span == span)?;
            let callee = file.ast.peel_ts_sugar_span(call.callee);
            if !file.ast.identifiers.iter().any(|id| {
                id.span == callee && id.role == solid_facts::ast::IdentifierRole::Reference
            }) {
                return None;
            }
            let mut symbol = lookup.entities().at(file.path.as_str(), callee)?.clone();
            let mut visiting = HashSet::new();
            loop {
                if !visiting.insert(symbol.clone()) {
                    return None;
                }
                let (alias, global) = values.get(&symbol)?.as_ref()?;
                if let Some(alias) = alias
                    && values.get(alias).is_some_and(Option::is_some)
                {
                    symbol = alias.clone();
                    continue;
                }
                return accessors.contains_key(&symbol).then_some((symbol, *global));
            }
        };
    let read_value = |read: &crate::ReactiveRead| {
        let file = lookup.file_by_path(&read.location.path)?;
        let span = Span::new(
            u32::try_from(read.location.start_byte).ok()?,
            u32::try_from(read.location.end_byte).ok()?,
        );
        if let Some((symbol, _)) = value_at(file, span) {
            return Some(symbol);
        }
        // A helper's template-local capture is not an instance identity.
        // Only a module's immutable singleton may be recovered from origin.
        let origin = read.origin.as_ref()?;
        let file = lookup.file_by_path(&origin.path)?;
        let span = Span::new(
            u32::try_from(origin.start_byte).ok()?,
            u32::try_from(origin.end_byte).ok()?,
        );
        value_at(file, span)
            .filter(|(_, global)| *global)
            .map(|(symbol, _)| symbol)
    };
    let mut tracked_sources = reads
        .iter()
        .filter(|read| {
            !read.is_uncertifiable()
                && !read.summary_attributed
                && matches!(
                    read.execution,
                    ExecutionRole::DeferredCallback | ExecutionRole::TrackedJsx
                )
        })
        .filter_map(|read| {
            function_at(read)
                .zip(read_value(read))
                .map(|(function, value)| {
                    (function, value, read.declaration.clone(), read.kind.clone())
                })
        })
        .collect::<HashSet<_>>();
    // Local read collection can omit a named compute whose invocation has no
    // reachability row. Its exact compute position and a direct call of a proven
    // accessor still establish the wording observation. Do not follow helpers,
    // nested callbacks, asynchronous bodies, or type-only accessor candidates.
    let computes = pairs
        .values()
        .flatten()
        .filter_map(Clone::clone)
        .collect::<HashSet<_>>();
    let mut guaranteed = HashMap::<(String, Span), HashSet<Span>>::new();
    for file in &facts.files {
        for call in file
            .ast
            .calls
            .iter()
            .filter(|call| !call.construct && call.arguments.is_empty())
        {
            let Some(function) = containing_ast_function(&file.ast, call.span) else {
                continue;
            };
            if !computes.contains(&(file.path.to_string(), function.span))
                || function.r#async
                || function.generator
            {
                continue;
            }
            if file.ast.any_jsx_containing(call.span) {
                continue;
            }
            let Some(symbol) = lookup.callee_symbol(file, call.callee) else {
                continue;
            };
            let Some((_, declaration)) = accessors.get(symbol) else {
                continue;
            };
            let declaration = origins.get(symbol).map_or(declaration, |origin| &origin.2);
            let Some((value, _)) = value_at(file, call.span) else {
                continue;
            };
            let source = (
                (file.path.to_string(), function.span),
                value,
                declaration.clone(),
                "accessor".into(),
            );
            if tracked_sources.contains(&source) {
                continue;
            }
            let calls = guaranteed
                .entry((file.path.to_string(), function.span))
                .or_insert_with(|| {
                    solid_facts::ast::synchronously_unconditional_calls(
                        std::path::Path::new(file.path.as_str()),
                        &file.source,
                        function.span,
                        &[],
                    )
                    .unwrap_or_default()
                    .into_iter()
                    .collect()
                });
            if !calls.contains(&call.span) {
                continue;
            }
            tracked_sources.insert(source);
        }
    }
    reads
        .iter()
        .filter(|read| {
            read.execution == ExecutionRole::EffectApply
                && read.kind.as_ref() == "accessor"
                && !read.is_uncertifiable()
                && !read.summary_attributed
                && !read.package_internal
        })
        .filter(|read| {
            function_at(read)
                .and_then(|apply| pairs.get(&apply))
                .is_some_and(|computes| {
                    // Several uses of a named apply need the same answer at every pair.
                    computes.iter().all(|compute| {
                        compute.as_ref().is_some_and(|compute| {
                            read_value(read).is_some_and(|value| {
                                tracked_sources.contains(&(
                                    compute.clone(),
                                    value,
                                    read.declaration.clone(),
                                    read.kind.clone(),
                                ))
                            })
                        })
                    })
                })
        })
        .map(|read| read.location.clone())
        .collect()
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
        let Some(primitive) = known_primitive(&primitives.calls[call_index]) else {
            continue;
        };
        for index in deferred_callback_positions(dialect, primitive, call.arguments.len()) {
            if callback_execution_at_call(file, call, primitive, index, lookup).is_none() {
                continue;
            }
            if let Some(argument) = call.arguments.get(index) {
                spans.push(argument.span);
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

// ---------------------------------------------------------------------------
// ADR 0319: invocation flow.
//
// Where a function value written in the program can be invoked, followed
// forward through the positions a closed program gives it: bindings,
// object-literal members, spreads, returns to their call sites, arguments to
// exact project callees, component props, and the host's own event
// dispatch. Every invocation site found must be one proven role; anything the
// walk does not understand answers nothing.
// ---------------------------------------------------------------------------

/// How many holder positions one query may visit before it gives up.
const FLOW_BUDGET: usize = 800;

/// What object a holder of the tracked value is, when the walk knows: the
/// object literal it was created as, or the props a JSX element renders a
/// component with (inside the component that element is written in, whose
/// own props came from `parent`). It answers whether calling another member
/// with that object as `this` can reach the tracked value.
#[derive(Clone)]
enum FlowOrigin<'a> {
    Unknown,
    Object(&'a solid_facts::FileFacts, Span),
    Element(&'a solid_facts::FileFacts, Span, Box<FlowOrigin<'a>>),
}

/// A holder the walk followed: its path and span, the member path it held
/// the value at, the context (calls and rendering elements) it was in, and
/// the identity of its origin, on which receiver proofs depend.
type FlowVisit = (
    String,
    Span,
    Vec<String>,
    Vec<(String, Span)>,
    Vec<(String, Span)>,
);

/// A call of an exact project callee the walk entered through a parameter:
/// the callee's path and span, and the call's file and span.
type FlowFrame<'a> = (String, Span, &'a solid_facts::FileFacts, Span);

/// A component the walk entered through its props parameter: the
/// component's path and span, the element that rendered it, and the origin
/// that element gives its props.
struct PropsFrame<'a> {
    path: String,
    component: Span,
    element_file: &'a solid_facts::FileFacts,
    element: Span,
    /// The member path of the props the walk entered with.
    followed: Vec<String>,
    origin: FlowOrigin<'a>,
}

/// One way the tracked value is invoked: at a call expression (with the
/// callee activations the walk was inside when it found it), or by the host
/// in a role (an intrinsic element's event dispatch).
#[derive(Clone)]
enum Invocation<'a> {
    Call(&'a solid_facts::FileFacts, Span, Vec<FlowFrame<'a>>),
    Role(ExecutionRole),
}

struct InvocationFlow<'a, 'l, 'c> {
    lookup: &'l SemanticLookup<'a>,
    classifying: &'c mut HashSet<(String, Span)>,
    budget: usize,
    /// Holders already followed in the current invocation census, with the
    /// call context they were reached in.
    seen: HashSet<FlowVisit>,
    /// Calls of exact project callees the walk entered through a parameter:
    /// a return from that callee goes to that call, not to every call.
    frames: Vec<FlowFrame<'a>>,
    /// Components the walk is inside through their props parameter, with
    /// the element that rendered each: what `props.x` reads there.
    props_stack: Vec<PropsFrame<'a>>,
    /// The owner question ([`unowned_event_literals`]): a primitive's
    /// callback slot refuses, and a call's role comes only from the walk
    /// itself or a compiler event-handler fact, never from the read role at
    /// the call (which `createRoot` and `runWithOwner` pass through from
    /// their caller although they supply an owner).
    owner_mode: bool,
}

/// ADR 0319: the one role every invocation of the function literal directly
/// around `span` runs in, followed forward through the program. Reads only:
/// write legality never asks.
fn invocation_flow_role(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
    classifying: &mut HashSet<(String, Span)>,
) -> Option<ExecutionRole> {
    if classifying.contains(&write_classification_marker()) || !lookup.program_closed {
        return None;
    }
    let file = lookup.file_by_path(file.path.as_str())?;
    let literal = containing_ast_function(&file.ast, span)?;
    // An async function runs in its invocation's role only up to its first
    // await; what may follow one is a continuation no invocation places.
    if !literal.body.contains(span)
        || (literal.r#async && follows_await_in_async_function(file, span))
    {
        return None;
    }
    // Every answer is computed once per build, context-free, and the set of
    // answers is what the cross-file proof digest binds: a cached fragment
    // that consumed one is reused only while every answer is unchanged.
    if COMPUTING_FLOW_ANSWERS.with(std::cell::Cell::get) {
        return literal_flow_role(file, literal, lookup, classifying);
    }
    lookup
        .invocation_flow_answers()
        .get(&(file.path.to_string(), literal.span))
        .copied()
}

thread_local! {
    /// Set while [`invocation_flow_answers`] computes the answers: a nested
    /// role query then computes directly instead of re-entering the memo.
    static COMPUTING_FLOW_ANSWERS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// ADR 0319: the invocation-flow answer of every function literal of a
/// closed program, keyed by path and literal span; only proven roles are
/// kept. Computed once per build, each answer from an empty context.
pub(crate) fn invocation_flow_answers(
    lookup: &SemanticLookup<'_>,
) -> HashMap<(String, Span), ExecutionRole> {
    let mut answers = HashMap::new();
    if !lookup.program_closed {
        return answers;
    }
    let outer = COMPUTING_FLOW_ANSWERS.with(|computing| computing.replace(true));
    for file in lookup.files() {
        for literal in &file.ast.functions {
            let mut classifying = HashSet::new();
            if let Some(role) = literal_flow_role(file, literal, lookup, &mut classifying) {
                answers.insert((file.path.to_string(), literal.span), role);
            }
        }
    }
    COMPUTING_FLOW_ANSWERS.with(|computing| computing.set(outer));
    answers
}

/// The flow answer for one literal, computed in the context `classifying`.
fn literal_flow_role(
    file: &solid_facts::FileFacts,
    literal: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
    classifying: &mut HashSet<(String, Span)>,
) -> Option<ExecutionRole> {
    let file = lookup.file_by_path(file.path.as_str())?;
    let literal = file
        .ast
        .functions
        .iter()
        .find(|function| function.span == literal.span)?;
    if literal.generator || lookup.function_component_status(file, literal) != ComponentStatus::No {
        return None;
    }
    if lookup.files().iter().any(|file| {
        file.ast
            .module_hazards
            .iter()
            .any(|hazard| hazard.kind == solid_facts::ast::ModuleHazardKind::Eval)
    }) {
        return None;
    }
    let key = (format!("\0flow:{}", file.path.as_str()), literal.span);
    if !classifying.insert(key.clone()) {
        return None;
    }
    let mut flow = InvocationFlow {
        lookup,
        classifying,
        budget: FLOW_BUDGET,
        seen: HashSet::new(),
        frames: Vec::new(),
        props_stack: Vec::new(),
        owner_mode: false,
    };
    let role = flow.function_role(file, literal).flatten();
    flow.classifying.remove(&key);
    role
}

/// Every function literal of a closed program whose invocations, followed
/// forward as ADR 0319 follows them, all run as a host event dispatch: an
/// intrinsic element's `on…` attribute or spread key, or a function the
/// compiler classifies as an event handler. Such a literal runs with no
/// owner, exactly as the same literal written on the element does. Any
/// invocation through a primitive's callback slot refuses: who owns that run
/// is the owner graph's question, not this walk's.
pub(crate) fn unowned_event_literals(lookup: &SemanticLookup<'_>) -> HashSet<(String, Span)> {
    let mut literals = HashSet::new();
    if !lookup.program_closed
        || lookup.files().iter().any(|file| {
            file.ast
                .module_hazards
                .iter()
                .any(|hazard| hazard.kind == solid_facts::ast::ModuleHazardKind::Eval)
        })
    {
        return literals;
    }
    for file in lookup.files() {
        for literal in &file.ast.functions {
            if literal.generator
                || lookup.function_component_status(file, literal) != ComponentStatus::No
            {
                continue;
            }
            let mut classifying = HashSet::new();
            let mut flow = InvocationFlow {
                lookup,
                classifying: &mut classifying,
                budget: FLOW_BUDGET,
                seen: HashSet::new(),
                frames: Vec::new(),
                props_stack: Vec::new(),
                owner_mode: true,
            };
            if flow.function_role(file, literal) == Some(Some(ExecutionRole::EventCallback)) {
                literals.insert((file.path.to_string(), literal.span));
            }
        }
    }
    literals
}

impl<'a> InvocationFlow<'a, '_, '_> {
    /// The one safe role of every invocation of `function`; `Some(None)`
    /// when the (complete) census finds no invocation at all.
    fn function_role(
        &mut self,
        file: &'a solid_facts::FileFacts,
        function: &'a solid_facts::ast::FunctionFact,
    ) -> Option<Option<ExecutionRole>> {
        let invocations = self.function_invocations(file, function)?;
        let mut role = None::<ExecutionRole>;
        for invocation in invocations {
            let here = match invocation {
                Invocation::Role(role) => role,
                // A call written in a function nothing invokes never runs.
                Invocation::Call(call_file, call, frames) => {
                    match self.site_role(call_file, call, frames)? {
                        Some(here) => here,
                        None => continue,
                    }
                }
            };
            role = Some(match role {
                None => here,
                Some(role) if role == here => role,
                // ADR 0323: invocations in different roles that all keep a
                // read out of the render window (an event, a deferred
                // callback, a tracked computation, an explicit `untrack`) say
                // one thing for read classification: no invocation reports
                // the read. No single role describes the mix without
                // overclaiming for some other consumer (a tracked role claims
                // a subscription the `untrack` run does not make), so the
                // answer is the no-claim role, which every read rule
                // suppresses and which a caller merging it treats as the mix
                // it is. A reporting role among them refuses, as before.
                Some(role) if non_reporting_mix(role) && non_reporting_mix(here) => {
                    ExecutionRole::Unknown
                }
                Some(_) => return None,
            });
        }
        Some(role)
    }

    /// The role of the call at `call`: a proven role at its position, or the
    /// one role of every invocation of the plain function it is written in;
    /// `Some(None)` when nothing invokes that function, so the call never
    /// runs.
    fn site_role(
        &mut self,
        file: &'a solid_facts::FileFacts,
        call: Span,
        frames: Vec<FlowFrame<'a>>,
    ) -> Option<Option<ExecutionRole>> {
        let callee = file.ast.call_at(call)?.callee;
        if self.owner_mode {
            return self.owner_site_role(file, call, frames);
        }
        let here = semantic_execution_role_within(
            file,
            callee,
            &[],
            self.lookup.entities(),
            self.lookup.symbol_names(),
            self.lookup,
            self.classifying,
        );
        // ADR 0320: a role read off a named function's callback references
        // ignores its direct calls (`press()` beside `onClick={press}`); the
        // walk below asks for every invocation of that function instead.
        if flow_safe_role(here)
            && named_callbacks_within_component(file, call, self.lookup)
                .into_iter()
                .all(|function| named_callback_invocations_hold(file, function, self.lookup, false))
        {
            return Some(Some(here));
        }
        let owner = containing_ast_function(&file.ast, call)?;
        if !owner.body.contains(call)
            || (owner.r#async && follows_await_in_async_function(file, call))
            || owner.generator
            || self.lookup.function_component_status(file, owner) != ComponentStatus::No
        {
            return None;
        }
        let key = (format!("\0flow:{}", file.path.as_str()), owner.span);
        if !self.classifying.insert(key.clone()) {
            return None;
        }
        // The function runs in the callee activations the call was found in:
        // a value it returns goes back to those calls.
        let outer = std::mem::replace(&mut self.frames, frames);
        let role = self.function_role(file, owner);
        self.frames = outer;
        self.classifying.remove(&key);
        role
    }

    /// [`Self::site_role`] for the owner question: the call is written
    /// directly in a function the compiler classifies as an event handler,
    /// or it takes the one role of every invocation of the plain function it
    /// is written in.
    fn owner_site_role(
        &mut self,
        file: &'a solid_facts::FileFacts,
        call: Span,
        frames: Vec<FlowFrame<'a>>,
    ) -> Option<Option<ExecutionRole>> {
        let owner = containing_ast_function(&file.ast, call)?;
        if !owner.body.contains(call)
            || (owner.r#async && follows_await_in_async_function(file, call))
            || owner.generator
            || self.lookup.function_component_status(file, owner) != ComponentStatus::No
        {
            return None;
        }
        if file.compiler.callback_roles.iter().any(|callback| {
            callback.role == solid_facts::compiler::CallbackRoleKind::EventHandler
                && file.ast.peel_ts_sugar_span(callback.span) == owner.span
        }) {
            return Some(Some(ExecutionRole::EventCallback));
        }
        let key = (format!("\0owner-flow:{}", file.path.as_str()), owner.span);
        if !self.classifying.insert(key.clone()) {
            return None;
        }
        let outer = std::mem::replace(&mut self.frames, frames);
        let role = self.function_role(file, owner);
        self.frames = outer;
        self.classifying.remove(&key);
        role
    }

    /// The context the walk is in, as a `seen` key component: the callee
    /// calls it entered through a parameter and the elements whose props it
    /// is inside. Receiver proofs depend on the elements.
    fn context(&self) -> Vec<(String, Span)> {
        self.frames
            .iter()
            .map(|(_, _, file, call)| (file.path.to_string(), *call))
            .chain(
                self.props_stack
                    .iter()
                    .map(|frame| (frame.element_file.path.to_string(), frame.element)),
            )
            .collect()
    }

    /// Every invocation of the function value `function` denotes.
    fn function_invocations(
        &mut self,
        file: &'a solid_facts::FileFacts,
        function: &'a solid_facts::ast::FunctionFact,
    ) -> Option<Vec<Invocation<'a>>> {
        // A script's top-level declarations are globals any file can reach
        // by name; no census of this file's references is complete.
        if !file_is_module(file) {
            return None;
        }
        // Each census is complete on its own: a holder another census already
        // followed still contributes its invocations here.
        let outer = std::mem::take(&mut self.seen);
        let result = self.function_invocations_inner(file, function);
        self.seen = outer;
        result
    }

    fn function_invocations_inner(
        &mut self,
        file: &'a solid_facts::FileFacts,
        function: &'a solid_facts::ast::FunctionFact,
    ) -> Option<Vec<Invocation<'a>>> {
        let mut out = Vec::new();
        if let Some(method) = crate::owners::own_method_name(file, function) {
            // An object literal's own method: the object holds it. (A function
            // nested in a method's body carries the method's name in the facts
            // but is not the method.)
            let property = file
                .ast
                .object_properties
                .iter()
                .find(|property| property.key == method.span && !property.computed)?;
            let object = innermost_object_literal(file, property.key)?;
            if receiver_used_within(file, object.span) {
                return None;
            }
            let name = static_key_text(file.source_text(method.span)?, false)?;
            self.uses(
                file,
                object.span,
                vec![name],
                FlowOrigin::Object(file, object.span),
                &mut out,
            )?;
            return Some(out);
        }
        if function.kind == solid_facts::ast::FunctionKind::Declaration {
            let name = function.name.as_ref()?;
            if flow_binding_exported(file, name.span)
                || crate::indexes::binding_written(file, name.span)
            {
                return None;
            }
            for (reference, _) in
                file.ast
                    .reference_declarations
                    .iter()
                    .filter(|(reference, declaration)| {
                        *declaration == name.span && *reference != name.span
                    })
            {
                self.uses(file, *reference, Vec::new(), FlowOrigin::Unknown, &mut out)?;
            }
            return Some(out);
        }
        // A named function expression can also hand itself on by its own
        // name (`function me() { slot.fn = me; }`).
        if let Some(name) = function.name.as_ref()
            && file
                .ast
                .reference_declarations
                .iter()
                .any(|(reference, declaration)| {
                    *declaration == name.span && *reference != name.span
                })
        {
            return None;
        }
        self.uses(
            file,
            function.span,
            Vec::new(),
            FlowOrigin::Unknown,
            &mut out,
        )?;
        Some(out)
    }

    /// Follow the value the expression at `expr` evaluates to, holding the
    /// tracked function at member `path` (empty: the function itself).
    /// `origin` is the object literal that value is, when known.
    #[allow(clippy::too_many_lines)]
    fn uses(
        &mut self,
        file: &'a solid_facts::FileFacts,
        expr: Span,
        path: Vec<String>,
        origin: FlowOrigin<'a>,
        out: &mut Vec<Invocation<'a>>,
    ) -> Option<()> {
        if self.budget == 0 || !file_is_module(file) {
            return None;
        }
        self.budget -= 1;
        let expr = file.ast.peel_ts_sugar_span(expr);
        if !self.seen.insert((
            file.path.to_string(),
            expr,
            path.clone(),
            self.context(),
            origin_identity(&origin),
        )) {
            return Some(());
        }
        let peel = |span: Span| file.ast.peel_ts_sugar_span(span);
        // Called.
        if let Some(call) = file.ast.calls.iter().find(|call| peel(call.callee) == expr) {
            if call.construct || !path.is_empty() {
                return None;
            }
            out.push(Invocation::Call(file, call.span, self.frames.clone()));
            return Some(());
        }
        // The object of a member access.
        if let Some(member) = file
            .ast
            .members
            .iter()
            .find(|member| peel(member.object) == expr)
        {
            if file
                .ast
                .assignments
                .iter()
                .any(|assignment| assignment.target.contains(member.span))
            {
                return None;
            }
            // A computed member counts only with a literal key.
            let key = member_static_key(file, member)?;
            let key = key.as_str();
            let called = file
                .ast
                .calls
                .iter()
                .any(|call| peel(call.callee) == member.span)
                || flow_tagged_or_chained(file, member.span);
            return match path.split_first() {
                // A property of the function itself: `length`, `name`. A
                // call through one (`bind`, `call`) runs it.
                None => (!called).then_some(()),
                Some((first, rest)) if first == key => {
                    self.uses(file, member.span, rest.to_vec(), FlowOrigin::Unknown, out)
                }
                // Another property: read, or called with this object as
                // `this`, which must be a function that cannot use it.
                Some(_) => {
                    if called {
                        origin_member_ignores_receiver(&origin, key, self.lookup, 6).then_some(())
                    } else {
                        Some(())
                    }
                }
            };
        }
        // An argument.
        if let Some((call, index)) = file
            .ast
            .arguments_containing(expr)
            .find(|(call, index)| peel(call.arguments[*index].span) == expr)
        {
            if call.construct
                || call
                    .arguments
                    .iter()
                    .take(index + 1)
                    .any(|argument| argument.spread)
            {
                return None;
            }
            // ADR 0323: the function itself handed to a primitive's callback
            // slot runs in that slot's role, when the dialect states one that
            // keeps a read out of the render window (`untrack(options.key)`,
            // `createMemo(options.key)`). A container holding it, an apply
            // slot, a slot that stores the function or runs it on result
            // access, and every unmodelled slot refuse.
            if let Some(primitive) = self.lookup.primitive_at_call(file, call.span) {
                if !path.is_empty() || self.owner_mode {
                    return None;
                }
                // A callback that runs synchronously in the call's own window
                // (`createRoot`, `runWithOwner`) runs wherever the call does:
                // its invocation is the call.
                if self
                    .lookup
                    .dialect
                    .callback_inherits_calling_window(primitive, index)
                    && callback_execution_at_call(file, call, primitive, index, self.lookup)
                        .is_some()
                {
                    out.push(Invocation::Call(file, call.span, self.frames.clone()));
                    return Some(());
                }
                let role = primitive_callback_slot_role(file, call, index, self.lookup)?;
                out.push(Invocation::Role(role));
                return Some(());
            }
            let (callee_file, callee, exact) =
                project_callee_at_parameter(file, call, index, self.lookup)?;
            if !exact
                || self.lookup.function_component_status(callee_file, callee) != ComponentStatus::No
            {
                return None;
            }
            let callee_file = self.lookup.file_by_path(callee_file.path.as_str())?;
            self.frames
                .push((callee_file.path.to_string(), callee.span, file, call.span));
            let result = self.parameter_uses(callee_file, callee, index, path, origin, out);
            self.frames.pop();
            return result;
        }
        // A JSX attribute value or spread.
        for element in file.ast.jsx_containing(expr) {
            if let Some(attribute) = element.attributes.iter().find(|attribute| {
                attribute
                    .expression
                    .is_some_and(|value| peel(value) == expr)
            }) {
                if attribute.namespace.is_some() {
                    return None;
                }
                let name = file.source_text(attribute.name)?;
                if intrinsic_element(file, element) {
                    return (path.is_empty() && flow_event_key(name)).then(|| {
                        out.push(Invocation::Role(ExecutionRole::EventCallback));
                    });
                }
                let mut prop_path = vec![name.to_owned()];
                prop_path.extend(path);
                return self.component_props_uses(
                    file,
                    element,
                    prop_path,
                    self.enclosing_props(file, element.span),
                    out,
                );
            }
            if element
                .spreads
                .iter()
                .any(|spread| peel(spread.argument) == expr)
            {
                if intrinsic_element(file, element) {
                    return match path.as_slice() {
                        [] => Some(()),
                        [key] if flow_event_key(key) => {
                            out.push(Invocation::Role(ExecutionRole::EventCallback));
                            Some(())
                        }
                        _ => None,
                    };
                }
                return self.component_props_uses(
                    file,
                    element,
                    path,
                    self.enclosing_props(file, element.span),
                    out,
                );
            }
            // A component's sole child expression is its `children` prop,
            // set after every attribute and spread. Several children make an
            // array, and a `children` attribute competes: both refuse.
            if element
                .children
                .iter()
                .any(|child| jsx_child_expression(file, *child) == Some(expr))
            {
                if intrinsic_element(file, element)
                    || jsx_children(file, element).len() != 1
                    || element
                        .attributes
                        .iter()
                        .any(|attribute| file.source_text(attribute.name) == Some("children"))
                {
                    return None;
                }
                let mut prop_path = vec!["children".to_owned()];
                prop_path.extend(path);
                return self.component_props_uses(
                    file,
                    element,
                    prop_path,
                    self.enclosing_props(file, element.span),
                    out,
                );
            }
        }
        // A property value of an object literal.
        if let Some(property) = file
            .ast
            .object_properties
            .iter()
            .find(|property| !property.computed && peel(property.value) == expr)
        {
            let object = innermost_object_literal(file, property.key)?;
            let key = property_static_key(file, property)?;
            if object
                .properties
                .iter()
                .any(|candidate| candidate.key.as_str() == key && candidate.getter.is_some())
                || receiver_used_within(file, object.span)
            {
                return None;
            }
            let mut member_path = vec![key];
            member_path.extend(path);
            return self.uses(
                file,
                object.span,
                member_path,
                FlowOrigin::Object(file, object.span),
                out,
            );
        }
        // Spread into an object literal.
        if let Some(spread) = file
            .ast
            .spreads
            .iter()
            .find(|spread| peel(spread.argument) == expr)
        {
            let object = file
                .ast
                .object_get_shapes
                .iter()
                .filter(|shape| shape.span.contains(spread.span) && shape.span != spread.span)
                .min_by_key(|shape| shape.span.end - shape.span.start)?;
            if path.is_empty() {
                return Some(());
            }
            // A later own property of the same key replaces it.
            if file.ast.object_properties.iter().any(|property| {
                object.span.contains(property.span)
                    && property.span.start > spread.span.start
                    && property_static_key(file, property).as_deref() == Some(path[0].as_str())
                    && innermost_object_literal(file, property.key)
                        .is_some_and(|inner| inner.span == object.span)
            }) {
                return Some(());
            }
            // The spread must be directly in that literal.
            if file.ast.object_get_shapes.iter().any(|inner| {
                inner.span != object.span
                    && object.span.contains(inner.span)
                    && inner.span.contains(spread.span)
            }) {
                return None;
            }
            // As for an own property: no getter of that key, and no function
            // in the literal that may reach the copied member through `this`.
            if object
                .properties
                .iter()
                .any(|candidate| candidate.key.as_str() == path[0] && candidate.getter.is_some())
                || receiver_used_within(file, object.span)
            {
                return None;
            }
            return self.uses(
                file,
                object.span,
                path,
                FlowOrigin::Object(file, object.span),
                out,
            );
        }
        // Returned.
        let returned_from = file
            .ast
            .returns
            .iter()
            .find(|returned| {
                returned
                    .argument
                    .is_some_and(|argument| peel(argument) == expr)
            })
            .and_then(|returned| containing_ast_function(&file.ast, returned.span))
            .or_else(|| {
                file.ast.functions.iter().find(|function| {
                    function.kind == solid_facts::ast::FunctionKind::Arrow
                        && peel(function.body) == expr
                })
            });
        if let Some(function) = returned_from {
            if function.r#async || function.generator {
                return None;
            }
            if let Some(index) = self.frames.iter().rposition(|(path, span, _, _)| {
                path == file.path.as_str() && *span == function.span
            }) {
                let (_, _, call_file, call) = self.frames[index];
                let saved = self.frames.split_off(index);
                let result = self.uses(call_file, call, path, origin.clone(), out);
                self.frames.extend(saved);
                return result;
            }
            for invocation in self.function_invocations(file, function)? {
                if let Invocation::Call(call_file, call, frames) = invocation {
                    let outer = std::mem::replace(&mut self.frames, frames);
                    let result = self.uses(call_file, call, path.clone(), origin.clone(), out);
                    self.frames = outer;
                    result?;
                }
            }
            return Some(());
        }
        // Bound to a `const`.
        if let Some(binding) = file.ast.bindings.iter().find(|binding| {
            binding
                .initializer
                .is_some_and(|initializer| peel(initializer) == expr)
        }) {
            let [name] = binding.names.as_slice() else {
                return None;
            };
            if binding.shape != solid_facts::ast::BindingShape::Identifier
                || !(binding.immutable || !crate::indexes::binding_written(file, name.span))
                || flow_binding_exported(file, name.span)
            {
                return None;
            }
            for (reference, _) in
                file.ast
                    .reference_declarations
                    .iter()
                    .filter(|(reference, declaration)| {
                        *declaration == name.span && *reference != name.span
                    })
            {
                self.uses(file, *reference, path.clone(), origin.clone(), out)?;
            }
            return Some(());
        }
        // A conditional's arm, or its test.
        if let Some(conditional) = file.ast.conditional_expressions.iter().find(|conditional| {
            peel(conditional.consequent) == expr || peel(conditional.alternate) == expr
        }) {
            return self.uses(file, conditional.span, path, FlowOrigin::Unknown, out);
        }
        if file
            .ast
            .conditional_expressions
            .iter()
            .any(|conditional| peel(conditional.test) == expr)
        {
            return Some(());
        }
        if let Some(logical) = file
            .ast
            .logical_expressions
            .iter()
            .find(|logical| peel(logical.left) == expr || peel(logical.right) == expr)
        {
            if peel(logical.left) == expr
                && logical.operator == solid_facts::ast::LogicalOperatorKind::And
            {
                // Its value is the left operand only when that is falsy.
                return Some(());
            }
            return self.uses(file, logical.span, path, FlowOrigin::Unknown, out);
        }
        None
    }

    /// The uses of `function`'s parameter `index`, holding the value at
    /// `path`.
    fn parameter_uses(
        &mut self,
        file: &'a solid_facts::FileFacts,
        function: &'a solid_facts::ast::FunctionFact,
        index: usize,
        path: Vec<String>,
        origin: FlowOrigin<'a>,
        out: &mut Vec<Invocation<'a>>,
    ) -> Option<()> {
        if !file_is_module(file) {
            return None;
        }
        let parameter = function.parameters.get(index)?;
        let [name] = parameter.names.as_slice() else {
            return None;
        };
        if parameter.shape != solid_facts::ast::BindingShape::Identifier
            || parameter.initializer.is_some()
            || (function.rest_parameter && index + 1 == function.parameters.len())
            || crate::indexes::binding_written(file, name.span)
            || file.ast.identifiers.iter().any(|identifier| {
                function.span.contains(identifier.span)
                    && matches!(
                        file.source_text(identifier.span),
                        Some("arguments" | "eval")
                    )
            })
        {
            return None;
        }
        for (reference, _) in file
            .ast
            .reference_declarations
            .iter()
            .filter(|(reference, declaration)| *declaration == name.span && *reference != name.span)
        {
            if !function.body.contains(*reference) {
                return None;
            }
            self.uses(file, *reference, path.clone(), origin.clone(), out)?;
        }
        Some(())
    }

    /// The uses of the props a project component element renders with,
    /// holding the value at `path`.
    fn component_props_uses(
        &mut self,
        file: &'a solid_facts::FileFacts,
        element: &solid_facts::ast::JsxElementFact,
        path: Vec<String>,
        enclosing: FlowOrigin<'a>,
        out: &mut Vec<Invocation<'a>>,
    ) -> Option<()> {
        let (component_file, component) = self
            .lookup
            .function_called_at(file.path.as_str(), element.name.span)?;
        if component.r#async || component.generator {
            return None;
        }
        // The tag must run that declaration: its binding is current, and a
        // member tag (`Menu.Item`) reaches exactly the declared member.
        let tag = file.ast.peel_ts_sugar_span(element.name.span);
        let member_tag = file.ast.members.iter().any(|member| member.span == tag)
            || file.ast.computed_members.binary_search(&tag).is_ok();
        if !self
            .lookup
            .function_value_is_current(component_file, component)
            || (member_tag
                && !member_runs_declared_method(file, tag, component_file, component, self.lookup))
        {
            return None;
        }
        let component_file = self.lookup.file_by_path(component_file.path.as_str())?;
        // A component rendering itself through this element again, holding
        // the value under the same member path (a tree of branches): the
        // recursive activation holds it at the same positions with the same
        // call context, so its invocations are the ones this activation
        // finds. Only its receiver proofs could differ, and they resolve
        // through the elements of the cycle back to the ones already proven,
        // provided every element in the cycle forwards each prop unchanged or
        // gives it a value that ignores `this`. A cycle that moves the value
        // to another prop (`b={props.a}`) is a different path and is followed
        // again, until it repeats or the budget runs out.
        if let Some(index) = self.props_stack.iter().position(|frame| {
            frame.element_file.path == file.path
                && frame.element == element.span
                && frame.followed == path
        }) {
            return self.props_stack[index..]
                .iter()
                .all(|frame| self.cycle_element_forwards(frame))
                .then_some(());
        }
        let origin = FlowOrigin::Element(file, element.span, Box::new(enclosing));
        self.props_stack.push(PropsFrame {
            path: component_file.path.to_string(),
            component: component.span,
            element_file: file,
            element: element.span,
            followed: path.clone(),
            origin: origin.clone(),
        });
        let result = self.parameter_uses(component_file, component, 0, path, origin, out);
        self.props_stack.pop();
        result
    }

    /// For an element on a rendering cycle: it has no spread, and each
    /// member the rendered component calls on its props is the same-named
    /// prop of the component the element is written in, forwarded unchanged,
    /// a value that ignores `this`, or undefined.
    fn cycle_element_forwards(&self, frame: &PropsFrame<'a>) -> bool {
        let file = frame.element_file;
        let Some(element) = file
            .ast
            .jsx_elements
            .iter()
            .find(|candidate| candidate.span == frame.element)
        else {
            return false;
        };
        let children = jsx_children(file, element);
        if !element.spreads.is_empty() || children.len() > 1 {
            return false;
        }
        // The props parameter of the component the element is written in.
        let Some(writer) = self.props_stack.iter().rev().find(|outer| {
            outer.path == file.path.as_str() && outer.component.contains(element.span)
        }) else {
            return false;
        };
        let Some(writer) = file
            .ast
            .functions
            .iter()
            .find(|function| function.span == writer.component)
        else {
            return false;
        };
        let Some(writer_props) = props_parameter(file, writer) else {
            return false;
        };
        let forwards = |value: Span, key: &str| {
            let value = jsx_value_expression(file, value);
            file.ast.members.iter().any(|member| {
                member.span == value
                    && member_static_key(file, member).as_deref() == Some(key)
                    && file
                        .ast
                        .reference_declaration(file.ast.peel_ts_sugar_span(member.object))
                        == Some(writer_props)
            }) || value_ignores_receiver(file, value, &FlowOrigin::Unknown, self.lookup, 6)
        };
        // Members the rendered component calls on its props: each must be
        // forwarded, given a value that ignores `this`, or undefined.
        let Some((component_file, component)) = self
            .lookup
            .function_called_at(file.path.as_str(), element.name.span)
        else {
            return false;
        };
        let Some(component_props) = props_parameter(component_file, component) else {
            return false;
        };
        component_file.ast.calls.iter().all(|call| {
            let callee = component_file.ast.peel_ts_sugar_span(call.callee);
            let Some(member) = component_file.ast.members.iter().find(|member| {
                member.span == callee
                    && component.span.contains(member.span)
                    && component_file
                        .ast
                        .reference_declaration(component_file.ast.peel_ts_sugar_span(member.object))
                        == Some(component_props)
            }) else {
                return true;
            };
            let Some(key) = member_static_key(component_file, member) else {
                return false;
            };
            let key = key.as_str();
            if key == "children" && !children.is_empty() {
                return children
                    .first()
                    .is_some_and(|child| forwards(*child, "children"));
            }
            match element
                .attributes
                .iter()
                .rev()
                .find(|attribute| file.source_text(attribute.name) == Some(key))
            {
                Some(attribute) => match attribute.value_kind {
                    solid_facts::ast::JsxAttributeValueKind::String
                    | solid_facts::ast::JsxAttributeValueKind::Boolean => true,
                    solid_facts::ast::JsxAttributeValueKind::Expression => {
                        attribute.namespace.is_none()
                            && attribute
                                .expression
                                .is_some_and(|value| forwards(value, key))
                    }
                    _ => false,
                },
                None => {
                    !self.lookup.member_may_be_reassigned_on_builtin(key)
                        && !self.lookup.member_may_be_reassigned_on_builtin("__proto__")
                        && !self.lookup.property_may_be_defined_reflectively_as(key)
                }
            }
        })
    }

    /// The props of the component activation an element at `element` is
    /// written in, when the walk entered that component through its props.
    fn enclosing_props(&self, file: &solid_facts::FileFacts, element: Span) -> FlowOrigin<'a> {
        self.props_stack
            .iter()
            .rev()
            .find(|frame| frame.path == file.path.as_str() && frame.component.contains(element))
            .map_or(FlowOrigin::Unknown, |frame| frame.origin.clone())
    }
}

/// ADR 0323: the role a function value handed to argument `index` of the
/// primitive call `call` runs in, when it is one that keeps a read out of the
/// render window: a tracked computation or a deferred (explicitly untracked,
/// registered, scheduled) callback. The reporting slots (an effect's apply, a
/// hydrate/render entry, an invalidation callback), a slot that stores the
/// function, runs it only on result access or needs a returned invoker, and
/// any slot the dialect does not model answer `None`.
fn primitive_callback_slot_role(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    index: usize,
    lookup: &SemanticLookup<'_>,
) -> Option<ExecutionRole> {
    let argument = &call.arguments[index];
    if argument.spread
        || !lookup
            .resolved_callee_call(file, call.callee)
            .is_some_and(|resolved| resolved.validity == typefacts::ResolvedCallValidity::Valid)
    {
        return None;
    }
    let primitive = lookup.primitive_at_call(file, call.span)?;
    let dialect = lookup.dialect;
    let semantics = dialect.callback_semantics_at(primitive, index, call.arguments.len());
    let execution = semantics.execution?;
    if semantics.requires_return_invocation
        || semantics.stores_as_value
        || dialect.callback_runs_on_result_access(primitive, index, call.arguments.len())
        || dialect.reports_untracked_reads_at(primitive, index, call.arguments.len())
        || dialect.apply_callback_argument(primitive) == Some(index)
    {
        return None;
    }
    if semantics.tracks_reads {
        return Some(ExecutionRole::TrackedJsx);
    }
    (execution == Execution::Deferred || dialect.runs_callback_deferred(primitive))
        .then_some(ExecutionRole::DeferredCallback)
}

/// Whether a role, or the mix of them [`InvocationFlow::function_role`]
/// answers as [`ExecutionRole::Unknown`] (which only that merge produces), keeps
/// a read out of every reporting role.
fn non_reporting_mix(role: ExecutionRole) -> bool {
    flow_safe_role(role) || role == ExecutionRole::Unknown
}

/// Whether a role keeps a read out of the render window and untracked
/// reporting: an event, a deferred callback, or a tracked computation.
fn flow_safe_role(role: ExecutionRole) -> bool {
    matches!(
        role,
        ExecutionRole::EventCallback | ExecutionRole::DeferredCallback | ExecutionRole::TrackedJsx
    )
}

/// An un-namespaced `on…` key: the runtime attaches it as an event listener.
fn flow_event_key(key: &str) -> bool {
    key.len() > 2 && key.starts_with("on") && !key.contains(':')
}

/// The innermost object literal whose own property key is at `key`.
fn innermost_object_literal(
    file: &solid_facts::FileFacts,
    key: Span,
) -> Option<&solid_facts::ast::ObjectGetShapeFact> {
    file.ast
        .object_get_shapes
        .iter()
        .filter(|shape| shape.span.contains(key))
        .min_by_key(|shape| shape.span.end - shape.span.start)
}

/// Whether any non-arrow function written in `span` may use `this`: a method
/// of an object literal there receives the object.
fn receiver_used_within(file: &solid_facts::FileFacts, span: Span) -> bool {
    file.ast.functions.iter().any(|function| {
        span.contains(function.span)
            && function.kind != solid_facts::ast::FunctionKind::Arrow
            && function_uses_receiver(file, function)
    })
}

/// Whether source text mentions `this` or `super` as a word.
pub(crate) fn mentions_receiver(text: &str) -> bool {
    mentions_this(text)
}

fn mentions_this(text: &str) -> bool {
    ["this", "super"].iter().any(|word| {
        text.match_indices(word).any(|(at, _)| {
            let before = text[..at].chars().next_back();
            let after = text[at + word.len()..].chars().next();
            !before.is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '$' || c == '.')
                && !after.is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '$')
        })
    })
}

/// Whether a non-arrow function may use the object it is called on: its
/// text (parameter defaults included) mentions `this` or `super` (a method's
/// `super.m()` runs the prototype's `m` with the same `this`).
fn function_uses_receiver(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
) -> bool {
    file.source_text(function.span).is_none_or(mentions_this)
}

/// Whether a source file is an ECMAScript module: it has an import or an
/// export. Anything else is a script, whose top-level declarations are
/// globals shared with every other script and reachable from any module.
pub(crate) fn file_is_module(file: &solid_facts::FileFacts) -> bool {
    !file.ast.imports.is_empty() || file.ast.module_level_exports().next().is_some()
}

/// Whether the member at `member` is a template tag, or reached through
/// (`p.g.call(…)`), which can hand its object over as `this`.
fn flow_tagged_or_chained(file: &solid_facts::FileFacts, member: Span) -> bool {
    let next = file
        .source
        .get(member.end as usize..)
        .map(str::trim_start)
        .unwrap_or_default();
    next.starts_with(['`', '(']) || next.starts_with("?.(")
}

/// An element's children as the JSX transform keeps them: whitespace-only
/// text that spans a line break is dropped.
fn jsx_children(
    file: &solid_facts::FileFacts,
    element: &solid_facts::ast::JsxElementFact,
) -> Vec<Span> {
    element
        .children
        .iter()
        .copied()
        .filter(|child| {
            !file
                .source_text(*child)
                .is_some_and(|text| text.trim().is_empty() && text.contains('\n'))
        })
        .collect()
}

/// The runtime property key a literal key token spells: an identifier name,
/// a quoted string, a no-substitution template (computed only) or a canonical
/// array index. Anything else, or any escape sequence, has no static name
/// here. A non-computed numeric key is normalized as JavaScript does only for
/// plain decimal integers; `1.0`, `0x1`, `1e3` refuse.
fn static_key_text(text: &str, computed: bool) -> Option<String> {
    let text = text.trim();
    if text.contains('\\') {
        return None;
    }
    let quoted = |open: char| {
        text.strip_prefix(open)
            .and_then(|inner| inner.strip_suffix(open))
            .filter(|inner| !inner.contains(open))
            .map(str::to_owned)
    };
    if let Some(inner) = quoted('"').or_else(|| quoted('\'')) {
        return Some(inner);
    }
    if computed && let Some(inner) = quoted('`').filter(|inner| !inner.contains("${")) {
        return Some(inner);
    }
    if !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit()) {
        let canonical = text == "0" || !text.starts_with('0');
        return (canonical && text.parse::<i32>().is_ok()).then(|| text.to_owned());
    }
    let identifier = !computed
        && text
            .chars()
            .next()
            .is_some_and(|first| !first.is_ascii_digit())
        && text
            .chars()
            .all(|character| character == '_' || character == '$' || character.is_alphanumeric());
    identifier.then(|| text.to_owned())
}

/// The static key of an object-literal property, or `None`. A non-computed,
/// non-shorthand `__proto__: v` (quoted or not) is no own property at all: it
/// sets the literal's prototype (ECMA-262 `PropertyDefinitionEvaluation`), so
/// it has no key here. The computed `["__proto__"]: v` and the shorthand
/// `{ __proto__ }` do define an own property.
pub(crate) fn property_static_key(
    file: &solid_facts::FileFacts,
    property: &solid_facts::ast::ObjectPropertyFact,
) -> Option<String> {
    let key = static_key_text(file.source_text(property.key)?, property.computed)?;
    let shorthand = property.key == property.value;
    (property.computed || shorthand || key != "__proto__").then_some(key)
}

/// The static key a member access reads: its identifier, or the literal key
/// of a computed member (`o["run"]`, `o[1]`); `None` for any other computed
/// member.
fn member_static_key(
    file: &solid_facts::FileFacts,
    member: &solid_facts::ast::MemberFact,
) -> Option<String> {
    if file
        .ast
        .computed_members
        .binary_search(&member.span)
        .is_ok()
    {
        return file
            .ast
            .literal_computed_members
            .binary_search_by_key(&member.span, |fact| fact.span)
            .ok()
            .map(|index| file.ast.literal_computed_members[index].key.to_string());
    }
    static_key_text(file.source_text(member.property)?, false)
}

/// The objects an origin names, innermost first: a `seen` key component, so
/// a holder reached with a different origin is followed (and its receiver
/// proofs run) again.
fn origin_identity(origin: &FlowOrigin<'_>) -> Vec<(String, Span)> {
    let mut identity = Vec::new();
    let mut origin = origin;
    loop {
        match origin {
            FlowOrigin::Unknown => return identity,
            FlowOrigin::Object(file, span) => {
                identity.push((file.path.to_string(), *span));
                return identity;
            }
            FlowOrigin::Element(file, span, enclosing) => {
                identity.push((file.path.to_string(), *span));
                origin = enclosing;
            }
        }
    }
}

/// The declaration span of a component's props parameter: its first
/// parameter, a plain identifier that is never reassigned.
fn props_parameter(
    file: &solid_facts::FileFacts,
    component: &solid_facts::ast::FunctionFact,
) -> Option<Span> {
    let parameter = component.parameters.first()?;
    let [name] = parameter.names.as_slice() else {
        return None;
    };
    (parameter.shape == solid_facts::ast::BindingShape::Identifier
        && parameter.initializer.is_none()
        && !crate::indexes::binding_written(file, name.span))
    .then_some(name.span)
}

/// The expression a JSX attribute value or child at `value` holds: a
/// container's braces peeled to the member or function that is the whole of
/// what they enclose (anything else stays the container, which no proof
/// recognizes).
fn jsx_value_expression(file: &solid_facts::FileFacts, value: Span) -> Span {
    let value = file.ast.peel_ts_sugar_span(value);
    let Some(inner) = file
        .source_text(value)
        .and_then(|text| text.strip_prefix('{'))
        .and_then(|text| text.strip_suffix('}'))
        .map(str::trim)
    else {
        return value;
    };
    file.ast
        .members
        .iter()
        .map(|member| member.span)
        .chain(file.ast.functions.iter().map(|function| function.span))
        .find(|candidate| value.contains(*candidate) && file.source_text(*candidate) == Some(inner))
        .map_or(value, |candidate| file.ast.peel_ts_sugar_span(candidate))
}

/// Whether member `key` of the object `origin` describes is a value that
/// cannot use that object as `this` when called: an arrow (or a `const`
/// bound to one), a non-arrow function that never mentions `this`, a
/// non-function literal, or, for a component's props, an attribute
/// forwarding such a value from the enclosing component's props. An absent
/// member proves nothing: `Object.prototype` may carry one.
fn origin_member_ignores_receiver(
    origin: &FlowOrigin<'_>,
    key: &str,
    lookup: &SemanticLookup<'_>,
    depth: usize,
) -> bool {
    if depth == 0 {
        return false;
    }
    match origin {
        FlowOrigin::Unknown => false,
        FlowOrigin::Object(file, object) => {
            let Some(property) = file
                .ast
                .object_properties
                .iter()
                .filter(|property| {
                    object.contains(property.span)
                        && property_static_key(file, property).as_deref() == Some(key)
                        && innermost_object_literal(file, property.key)
                            .is_some_and(|inner| inner.span == *object)
                })
                .max_by_key(|property| property.span.start)
            else {
                return false;
            };
            // The property must be the member's final value: no spread and no
            // key without a static name after it may replace it, and a getter
            // or setter is no plain value.
            let own = |span: Span| {
                innermost_object_literal(file, span).is_some_and(|inner| inner.span == *object)
            };
            if (!property.data && !property.method)
                || file.ast.spreads.iter().any(|spread| {
                    object.contains(spread.span)
                        && spread.span.start > property.span.start
                        && file
                            .ast
                            .object_get_shapes
                            .iter()
                            .filter(|shape| {
                                shape.span.contains(spread.span) && shape.span != spread.span
                            })
                            .min_by_key(|shape| shape.span.end - shape.span.start)
                            .is_some_and(|shape| shape.span == *object)
                })
                || file.ast.object_properties.iter().any(|other| {
                    object.contains(other.span)
                        && other.span.start > property.span.start
                        && own(other.key)
                        && property_static_key(file, other).is_none()
                })
            {
                return false;
            }
            if property.method {
                return file
                    .ast
                    .functions
                    .iter()
                    .find(|function| {
                        function
                            .method_name
                            .as_ref()
                            .is_some_and(|name| name.span == property.key)
                    })
                    .is_some_and(|function| !function_uses_receiver(file, function));
            }
            value_ignores_receiver(file, property.value, &FlowOrigin::Unknown, lookup, depth)
        }
        FlowOrigin::Element(file, element, enclosing) => {
            let Some(element) = file
                .ast
                .jsx_elements
                .iter()
                .find(|candidate| candidate.span == *element)
            else {
                return false;
            };
            if !element.spreads.is_empty() {
                return false;
            }
            let children = jsx_children(file, element);
            if key == "children" && !children.is_empty() {
                let [child] = children.as_slice() else {
                    return false;
                };
                return value_ignores_receiver(file, *child, enclosing, lookup, depth);
            }
            match element
                .attributes
                .iter()
                .rev()
                .find(|attribute| file.source_text(attribute.name) == Some(key))
            {
                // No attribute gives the props object that key, and no
                // assignment or reflective definition anywhere can give it
                // one or reach the built-in prototype it would otherwise be
                // read from: the read is `undefined`, or an
                // `Object.prototype` built-in that invokes nothing it holds.
                None => {
                    !lookup.member_may_be_reassigned_on_builtin(key)
                        && !lookup.member_may_be_reassigned_on_builtin("__proto__")
                        && !lookup.property_may_be_defined_reflectively_as(key)
                }
                Some(attribute) => match attribute.value_kind {
                    solid_facts::ast::JsxAttributeValueKind::String
                    | solid_facts::ast::JsxAttributeValueKind::Boolean => true,
                    solid_facts::ast::JsxAttributeValueKind::Expression => {
                        attribute.expression.is_some_and(|value| {
                            value_ignores_receiver(file, value, enclosing, lookup, depth)
                        })
                    }
                    _ => false,
                },
            }
        }
    }
}

/// Whether the value at `value` (in an element written in a component whose
/// props `enclosing` describes) is one that cannot use a `this` it is called
/// with; see [`origin_member_ignores_receiver`].
fn value_ignores_receiver(
    file: &solid_facts::FileFacts,
    value: Span,
    enclosing: &FlowOrigin<'_>,
    lookup: &SemanticLookup<'_>,
    depth: usize,
) -> bool {
    let value = jsx_value_expression(file, value);
    if let Some(function) = file
        .ast
        .functions
        .iter()
        .find(|function| function.span == value)
    {
        return function.kind == solid_facts::ast::FunctionKind::Arrow
            || !function_uses_receiver(file, function);
    }
    if let Some(declaration) = file.ast.reference_declaration(value) {
        if let Some(function) = file.ast.functions.iter().find(|function| {
            function
                .name
                .as_ref()
                .is_some_and(|name| name.span == declaration)
        }) {
            return !function_uses_receiver(file, function);
        }
        if let Some(binding) = file.ast.bindings.iter().find(|binding| {
            binding.immutable && binding.names.iter().any(|name| name.span == declaration)
        }) {
            return binding.initializer.is_some_and(|initializer| {
                let initializer = file.ast.peel_ts_sugar_span(initializer);
                file.ast.functions.iter().any(|function| {
                    function.span == initializer
                        && (function.kind == solid_facts::ast::FunctionKind::Arrow
                            || !function_uses_receiver(file, function))
                })
            });
        }
        // A component's props parameter read as `props.x`: the enclosing
        // element's attribute `x`.
    }
    // `props.x` of the component the enclosing element renders, read in
    // that component: the enclosing element's member `x`.
    if let Some(member) = file.ast.members.iter().find(|member| member.span == value)
        && file
            .ast
            .computed_members
            .binary_search(&member.span)
            .is_err()
        && let FlowOrigin::Element(element_file, element, _) = enclosing
        && let Some(element) = element_file
            .ast
            .jsx_elements
            .iter()
            .find(|candidate| candidate.span == *element)
        && let Some((component_file, component)) =
            lookup.function_called_at(element_file.path.as_str(), element.name.span)
        && component_file.path == file.path
        && component.span.contains(member.span)
        && let Some(key) = member_static_key(file, member)
        && let key = key.as_str()
        && let Some(parameter) = component.parameters.first()
        && let [name] = parameter.names.as_slice()
        && parameter.shape == solid_facts::ast::BindingShape::Identifier
        && !crate::indexes::binding_written(file, name.span)
        && file
            .ast
            .reference_declaration(file.ast.peel_ts_sugar_span(member.object))
            == Some(name.span)
    {
        return origin_member_ignores_receiver(enclosing, key, lookup, depth - 1);
    }
    file.ast.array_literals.binary_search(&value).is_ok()
        || file
            .source_text(value)
            .is_some_and(|text| text.starts_with(['"', '\'']) || text.parse::<f64>().is_ok())
}

/// Whether the binding declared at `declaration` is exported.
pub(crate) fn flow_binding_exported(file: &solid_facts::FileFacts, declaration: Span) -> bool {
    let module_level = containing_ast_function(&file.ast, declaration).is_none();
    // In a script every top-level binding is global.
    if module_level && !file_is_module(file) {
        return true;
    }
    file.ast.exports.iter().any(|export| {
        (module_level && export.span.contains(declaration))
            || export.specifiers.iter().any(|specifier| {
                file.ast.reference_declaration(specifier.local.span) == Some(declaration)
            })
    })
}
