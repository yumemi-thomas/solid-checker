//! Execution-role classification.
//!
//! Given a source span, classify the reactive execution context it runs in
//! (tracked JSX, deferred callback, effect apply, event handler, …). This is
//! the compiler-fact classifier plus the semantic (AST-driven) classifier and
//! the two role-keyed read helpers that consume its result.

use crate::{HashMap, HashSet};

use solid_dialect::{Dialect, Execution, Primitive};
use solid_facts::core::Span;

use super::{
    EntitySymbols, ExecutionRole, PrimitiveName, SemanticLookup, SymbolId,
    exact_jsx_primitive_name, jsx_primitive_name, known_primitive, location,
};
#[path = "project_consumer.rs"]
mod project_consumer;

mod jsx_census;
pub(crate) use jsx_census::*;
mod write_roles;
pub(crate) use write_roles::*;
mod cleanup_handle;
use cleanup_handle::*;
mod events;
pub(crate) use events::*;
mod props_shape;
use props_shape::*;
mod body_roles;
pub(crate) use body_roles::*;
mod callee_roles;
pub(crate) use callee_roles::*;
mod callback_standing;
pub(crate) use callback_standing::*;
mod control_flow;
pub(crate) use control_flow::*;
mod named_callbacks;
pub(crate) use named_callbacks::*;
mod read_context;
pub(crate) use read_context::*;
mod invocation_flow;
pub(crate) use invocation_flow::*;
mod event_literals;
pub(crate) use event_literals::*;
mod flow_helpers;
mod flow_memos;
pub(crate) use flow_helpers::*;
pub(crate) use flow_memos::{
    FLOW_NARROW_TABLES_END, FLOW_NARROW_TABLES_FROM, FLOW_TABLES_END, FLOW_TABLES_FROM, FlowTables,
};
pub(crate) use project_consumer::ReadConsumerSummaries;

use crate::indexes::ComponentStatus;
use crate::owners::{
    callback_execution_at_call, callback_owner_at_call, containing_ast_function,
    enclosing_function_label, function_binding_name, returned_callback_execution_at_call,
    returned_callback_invocation_sites, returned_primitive_invocation,
};

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
        // exact runtime semantics explicitly override that classification.
        Some(Execution::Tracked) => !semantics.tracks_reads,
        // Deferred callbacks execute after/outside the current tracking pass.
        Some(Execution::Deferred) => true,
        // Inline means the callback inherits the caller's Listener. Only
        // primitives such as untrack/createRoot/runWithOwner that explicitly
        // clear Listener belong to the outside-tracking set.
        Some(Execution::Inline) => {
            dialect.runs_callback_deferred(primitive)
                || dialect.callback_runs_untracked_without_window(primitive, argument)
        }
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
    // ADR 0360: a file the compiler refused has no facts to place anything.
    if facts.withheld() {
        return ExecutionRole::Unknown;
    }
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
    let within = functions_around(file, span)
        .into_iter()
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
        &mut HashSet::default(),
    )
}

/// What the operation's runtime guard does when a root owner is the ambient
/// owner, which is the case directly in a `createRoot` body
/// ([`solid_dialect::Dialect::callback_runs_in_created_root`]) and, where the
/// dialect says so, directly in a component body
/// ([`solid_dialect::Dialect::component_body_runs_under_root`]).
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum RootBodyGuard {
    /// The guard rejects the operation under a root, as under any
    /// children-capable owner: signal setters, `refresh` and action calls on
    /// every Solid 2 release, store setters where the release's guard does not
    /// exempt roots.
    Rejects,
    /// As [`RootBodyGuard::Rejects`], but the guard also stands down while the
    /// runtime disposes an owner (`!disposalDepth`): signal and store setters
    /// and `refresh` (`@solidjs/signals@2.0.0-rc.17` `dist/dev-shared.js:5620-5626`,
    /// `:5801-5806`, `dist/dev.js:5811-5812`), where an action call's guard does not
    /// ask (`dist/dev.js:1543-1556`).
    RejectsOutsideDisposal,
    /// The guard exempts a root: store setters on a release whose guard tests
    /// `!context._root` ([`solid_dialect::Dialect::store_setter_guard_exempts_roots`]).
    Exempts,
}

#[cfg(test)]
mod tests;
