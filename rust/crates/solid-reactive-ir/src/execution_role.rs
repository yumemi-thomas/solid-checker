//! Execution-role classification.
//!
//! Given a source span, classify the reactive execution context it runs in
//! (tracked JSX, deferred callback, effect apply, event handler, …). This is
//! the compiler-fact classifier plus the semantic (AST-driven) classifier and
//! the two role-keyed read helpers that consume its result.

use crate::role_log::role_arm;
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
mod write_components;
pub use write_components::{WALK_STEP_BOUND, bound_walks, follow_every_path};
pub(crate) use write_components::{every_path, every_path_asked, walk_bound};
use write_components::{visit_contains, visit_insert};
mod pending_sources;
pub(crate) use pending_sources::*;
mod cleanup_handle;
use cleanup_handle::*;
mod site_reach;
pub(crate) use site_reach::*;
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

/// ADR 0540: the function an effect runs as its apply at `argument` of
/// `call`. An exact bundle literal (`createEffect(compute, { effect, error })`,
/// rc.17's `EffectBundle`) runs only its `effect` member there: `runEffect`
/// calls the `error` member before it opens the strict-read window
/// (`@solidjs/signals@2.0.0-rc.17` `dist/dev.js:1966-1989`). `None` for an
/// object whose members are not exact (no member is proven the apply).
pub(crate) fn effect_apply_span(
    call: &solid_facts::ast::CallFact,
    argument: usize,
) -> Option<Span> {
    let argument = call.arguments.get(argument)?;
    if argument.runtime_value_kind != solid_facts::ast::RuntimeValueKind::Object {
        return Some(argument.span);
    }
    argument
        .literal_members
        .iter()
        .rev()
        .find(|member| member.key == "effect")
        .map(|member| member.value)
        .filter(|_| !argument.literal_members.is_empty())
}

/// ADR 0548: the `error` member of an exact effect bundle at `argument` of
/// `call`, where the dialect runs it from the flush's error branch with no
/// owner and before the strict-read window opens
/// ([`Dialect::bundle_error_member_runs_unowned`]). A bundle with a spread, a
/// computed key or an accessor names no member exactly.
pub(crate) fn bundle_error_member(
    dialect: &dyn Dialect,
    primitive: Primitive,
    call: &solid_facts::ast::CallFact,
    argument: usize,
) -> Option<Span> {
    let value = call.arguments.get(argument)?;
    if !dialect.bundle_error_member_runs_unowned(primitive, argument)
        || value.spread
        || !value.exact_object_literal
    {
        return None;
    }
    value
        .literal_members
        .iter()
        .rev()
        .find(|member| member.key == "error")
        .map(|member| member.value)
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

/// ADR 0857: whether the callback at `argument` runs during the call with the
/// caller's listener: inline, and not one a listener-clearing primitive runs.
pub(crate) fn callback_inherits_listener(
    dialect: &dyn Dialect,
    primitive: Primitive,
    argument: usize,
    argument_count: usize,
) -> bool {
    dialect
        .callback_semantics_at(primitive, argument, argument_count)
        .execution
        == Some(Execution::Inline)
        && !callback_runs_outside_tracking(dialect, primitive, argument, argument_count)
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
        return role_arm!(
            "execution_role.execution_role_where.unknown",
            ExecutionRole::Unknown
        );
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
        return role_arm!(
            "execution_role.execution_role_where.discarded-rendering",
            ExecutionRole::DiscardedRendering
        );
    }
    if allowed.iter().any(|region| region.contains(span)) {
        return role_arm!(
            "execution_role.execution_role_where.deferred-callback",
            ExecutionRole::DeferredCallback
        );
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
        return role_arm!("execution_role.execution_role_where.compiler-region", role);
    }
    role_arm!(
        "execution_role.execution_role_where.unknown-2",
        ExecutionRole::Unknown
    )
}

/// Whether the event of the handler the compiler wires at `callback` keeps the
/// host-delivery proof ([`crate::runtime_semantics::event_keeps_delivery_proof`]),
/// read at the delivering element. A handler no attribute names (a spread) has
/// no measured event: denied.
pub(crate) fn handler_event_denied(file: &solid_facts::FileFacts, callback: Span) -> bool {
    !file.ast.jsx_containing(callback).any(|element| {
        element.attributes.iter().any(|attribute| {
            attribute
                .expression
                .or(attribute.value)
                .is_some_and(|value| value.contains(callback))
                && file
                    .source_text(attribute.name)
                    .is_some_and(crate::runtime_semantics::event_keeps_delivery_proof)
        })
    })
}

/// Whether `span` is inside a handler of a denied event.
pub(crate) fn in_denied_handler(file: &solid_facts::FileFacts, span: Span) -> bool {
    file.compiler.callback_roles.iter().any(|callback| {
        callback.role == solid_facts::compiler::CallbackRoleKind::EventHandler
            && callback.span.contains(span)
            && handler_event_denied(file, callback.span)
    })
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
    let role = execution_role_where(&file.compiler, span, allowed, within, |callback| {
        // A component's `ref` is a prop, not an ownerless ref application.
        (callback.role != solid_facts::compiler::CallbackRoleKind::DirectiveApply
            || crate::owners::ref_application_on_intrinsic(file, callback.span))
            && (matches!(
                callback.role,
                solid_facts::compiler::CallbackRoleKind::Deferred
                    | solid_facts::compiler::CallbackRoleKind::Render
            ) || file.ast.functions_within(callback.span).any(|function| {
                // Only a function the attribute's expression yields is the
                // handler (or the ref) the compiler binds; any other function
                // in it runs, or not, while the expression evaluates at the
                // element's setup: `onClick={(log(), h)}`'s IIFE runs in the
                // body, under its window (rc.17 warns a read there).
                function.body.contains(span)
                    && crate::owners::yielded_value_contains(file, callback.span, function.span)
            }))
    });
    if role == ExecutionRole::EventCallback && in_denied_handler(file, span) {
        return role_arm!("execution_role.denied-event", ExecutionRole::Unknown);
    }
    role
}

pub(super) fn semantic_execution_role(
    file: &solid_facts::FileFacts,
    span: Span,
    allowed: &[Span],
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
) -> ExecutionRole {
    crate::role_log::decide("role", file.path.as_str(), span, || {
        semantic_execution_role_within(
            file,
            span,
            allowed,
            entities,
            symbol_names,
            lookup,
            &mut HashSet::default(),
        )
    })
}

/// [`semantic_execution_role`] for a reactive read. A read in a function an
/// intrinsic `ref` applies ([`crate::owners::ref_applied_element`]: the
/// literal written in the attribute, the function a bare identifier there
/// holds, a literal the factory of a two-phase directive returns) runs while
/// its element is created: rc.17's `ref` (`@solidjs/web`
/// `dist/web.dev.js:1016-1022`) applies it under `runWithOwner(null)`, which
/// clears the owner and the observer (`@solidjs/signals@2.0.0-rc.17`
/// `dist/dev-shared.js:5758-5786`) and leaves the strict-read window of the
/// code creating the element as it is. On an element created in a component
/// body's untracked rendering the read is that body's read
/// ([`ExecutionRole::UntrackedRendering`]); the same through the layers that
/// run in place on the applied function's stack (an IIFE, a helper's inline
/// callback, [`climb_in_place_layers`]). An element created anywhere else (a
/// flow row, a component's children, a tracked expression) leaves the read
/// unplaced, as does a function the ref receives that no proof applies (one
/// nested in the applied function, a generator): an obligation, never a
/// silent clean.
pub(crate) fn read_execution_role(
    file: &solid_facts::FileFacts,
    span: Span,
    allowed: &[Span],
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
) -> ExecutionRole {
    crate::role_log::decide("read", file.path.as_str(), span, || {
        read_execution_role_unlogged(file, span, allowed, entities, symbol_names, lookup)
    })
}

fn read_execution_role_unlogged(
    file: &solid_facts::FileFacts,
    span: Span,
    allowed: &[Span],
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
) -> ExecutionRole {
    let role = semantic_execution_role(file, span, allowed, entities, symbol_names, lookup);
    // Deleted code and code past an await keep their answers.
    if matches!(
        role,
        ExecutionRole::DiscardedRendering | ExecutionRole::Unreported
    ) {
        return role;
    }
    if let Some(applied) =
        ref_applied_read_role(file, span, allowed, entities, symbol_names, lookup)
    {
        return dynamic_read_role(file, span, applied, lookup);
    }
    if role == ExecutionRole::DirectiveApply {
        return role_arm!(
            "execution_role.read_execution_role_unlogged.unknown",
            ExecutionRole::Unknown
        );
    }
    dynamic_read_role(file, span, role, lookup)
}

pub(crate) fn dynamic_read_role(
    file: &solid_facts::FileFacts,
    span: Span,
    role: ExecutionRole,
    lookup: &SemanticLookup<'_>,
) -> ExecutionRole {
    if role == ExecutionRole::UntrackedRendering
        && let Some(exclusive) = dynamic_component_entry(file, span, lookup)
    {
        return if exclusive {
            role_arm!(
                "execution_role.dynamic_read_role.exclusive-entry",
                ExecutionRole::UntrackedUnwindowed
            )
        } else {
            role_arm!(
                "execution_role.dynamic_read_role.shared-entry",
                ExecutionRole::Unknown
            )
        };
    }
    role
}

/// Whether an intrinsic `ref` applies `literal` while creating an element in a
/// component body's untracked rendering ([`crate::owners::ref_applied_element`],
/// [`read_execution_role`]): the literal then runs during that body, in its
/// strict-read window.
pub(crate) fn ref_applied_in_body(
    file: &solid_facts::FileFacts,
    literal: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
) -> bool {
    crate::owners::ref_applied_element(file, literal, lookup).is_some_and(|element| {
        semantic_execution_role(
            file,
            element.name.span,
            &allowed_callback_spans(file, lookup),
            lookup.entities(),
            lookup.symbol_names(),
            lookup,
        ) == ExecutionRole::UntrackedRendering
    })
}

/// The role of a read in a function an intrinsic `ref` applies, or in a
/// layer that runs in place on that function's stack ([`read_execution_role`]).
fn ref_applied_read_role(
    file: &solid_facts::FileFacts,
    span: Span,
    allowed: &[Span],
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
) -> Option<ExecutionRole> {
    let function = containing_ast_function(&file.ast, span)?;
    let (element, climb) = match crate::owners::ref_applied_element(file, function, lookup) {
        Some(element) => (element, None),
        None => {
            // Through the generators stepped at their own call too (ADR 0516,
            // `in_place_run_site`): `(function* () { r(); yield 1; })().next()`
            // in a ref callback runs some of its body, or none, on the stack
            // of that call, so the read is unplaced there, never the deferred
            // role the callback's position gives it.
            let (climb, through_generator) = in_place_run_site(file, span, lookup)?;
            if climb.exhausted {
                return Some(ExecutionRole::Unknown);
            }
            let function = containing_ast_function(&file.ast, climb.at)?;
            (
                crate::owners::ref_applied_element(file, function, lookup)?,
                Some((climb, through_generator)),
            )
        }
    };
    // The element's own tag is where the compiler creates it and applies its
    // ref: the role of that spot is the role of the application.
    let created = semantic_execution_role(
        file,
        element.name.span,
        allowed,
        entities,
        symbol_names,
        lookup,
    );
    if created != ExecutionRole::UntrackedRendering {
        return Some(role_arm!(
            "execution_role.ref_applied_read_role.unknown",
            ExecutionRole::Unknown
        ));
    }
    if let Some((climb, through_generator)) = climb {
        match climb.suspended {
            Some(true) => {
                return Some(role_arm!(
                    "execution_role.ref_applied_read_role.unreported",
                    ExecutionRole::Unreported
                ));
            }
            Some(false) => {
                return Some(role_arm!(
                    "execution_role.ref_applied_read_role.unknown-2",
                    ExecutionRole::Unknown
                ));
            }
            None if climb.in_list || through_generator => {
                return Some(role_arm!(
                    "execution_role.ref_applied_read_role.unknown-3",
                    ExecutionRole::Unknown
                ));
            }
            None => {}
        }
    }
    Some(window_after_awaits(
        file,
        span,
        lookup,
        ExecutionRole::UntrackedRendering,
    ))
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

/// Whether `at` is in the `component` attribute of an exact `<Dynamic>`, and
/// whether it is that attribute's whole value.
fn dynamic_component_attribute(
    file: &solid_facts::FileFacts,
    at: Span,
    lookup: &SemanticLookup<'_>,
) -> (bool, bool) {
    let ast = &file.ast;
    let dialect = lookup.dialect;
    let mut found = (false, false);
    for (tag, prop) in dialect.component_prop_renderers() {
        for element in ast.jsx_elements.iter().filter(|element| {
            crate::execution_paths::named_import(file, dialect, element.name.span, tag)
        }) {
            for attribute in &element.attributes {
                if file.source_text(attribute.name) == Some(*prop)
                    && let Some(value) = attribute.expression
                {
                    found.0 |= value.contains(at);
                    found.1 |= ast.peel_ts_sugar_span(value) == at;
                }
            }
        }
    }
    found
}

/// `const Z = K` where `at` is `K`: the same answer through `Z`'s references.
/// One step, on a binding never reassigned: past that the mount is open.
fn dynamic_alias_mount(
    file: &solid_facts::FileFacts,
    at: Span,
    lookup: &SemanticLookup<'_>,
) -> (bool, bool) {
    let Some(alias) = file.ast.bindings.iter().find(|binding| {
        binding.immutable
            && binding
                .initializer_identifier
                .as_ref()
                .is_some_and(|initializer| initializer.span == at)
    }) else {
        return (false, false);
    };
    let Some(references) = alias
        .names
        .first()
        .and_then(|name| lookup.callee_symbol(file, name.span))
        .and_then(|symbol| lookup.symbol_references_if_present(symbol))
    else {
        return (false, false);
    };
    let mut mounts = false;
    let mut whole = true;
    for reference in references {
        let (Some(candidate), Ok(start), Ok(end)) = (
            lookup.file_by_path(&reference.path),
            u32::try_from(reference.start_byte),
            u32::try_from(reference.end_byte),
        ) else {
            return (mounts, false);
        };
        let span = Span::new(start, end);
        if candidate.path == file.path && alias.names.iter().any(|name| name.span == span) {
            continue;
        }
        let (inside, exact) = dynamic_component_attribute(candidate, span, lookup);
        mounts |= inside;
        whole &= exact;
    }
    (mounts, mounts && whole)
}

/// A closed reference census for a component returned directly by the source
/// literal of an exact dynamic factory. Other entries withhold the placement.
pub(crate) fn dynamic_component_entry(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> Option<bool> {
    let mut span = span;
    let mut climbed = false;
    for _ in 0..32 {
        if let Some((climb, generator)) = in_place_run_site(file, span, lookup) {
            if generator || climb.suspended.is_some() || climb.in_list || climb.among_others {
                return None;
            }
            span = climb.at;
        }
        let function = containing_ast_function(&file.ast, span)?;
        // An intrinsic's `ref` runs during the body that creates the element.
        if let Some(element) = crate::owners::ref_applied_element(file, function, lookup) {
            span = element.span;
            continue;
        }
        let inherited = file
            .ast
            .arguments_containing(function.span)
            .find_map(|(call, index)| {
                (!call.arguments[index].spread
                    && file.ast.peel_ts_sugar_span(call.arguments[index].span) == function.span
                    && lookup
                        .primitive_at_call(file, call.span)
                        .is_some_and(|primitive| {
                            lookup
                                .dialect
                                .callback_preserves_read_window(primitive, index)
                        }))
                .then_some(call.span)
            });
        let Some(call) = inherited else {
            climbed = true;
            break;
        };
        span = call;
    }
    // ADR 0695: a climb that stopped at its bound names no entry, and `None`
    // would keep a rendering role, a warning Chrome does not give at 33 levels.
    if !climbed {
        return Some(false);
    }
    let function = containing_ast_function(&file.ast, span)?;
    let name = function_binding_name(file, function)?;
    if lookup.program_has_eval() {
        return None;
    }
    let symbol = lookup.callee_symbol(file, name.span)?;
    let references = lookup.symbol_references_if_present(symbol)?;
    let mut dynamic = false;
    let mut exclusive = !flow_binding_exported(file, name.span);
    for reference in references {
        // file_by_path records the selected file slot (or missing-path probe).
        let Some(candidate) = lookup.file_by_path(&reference.path) else {
            exclusive = false;
            continue;
        };
        let (Ok(start), Ok(end)) = (
            u32::try_from(reference.start_byte),
            u32::try_from(reference.end_byte),
        ) else {
            exclusive = false;
            continue;
        };
        let at = Span::new(start, end);
        if candidate.path == file.path && at == name.span {
            continue;
        }
        let source = candidate.ast.functions.iter().find(|source| {
            !source.generator
                && source.expression_body
                && candidate.ast.peel_ts_sugar_span(source.body) == at
        });
        let mounted = source.is_some_and(|source| {
            candidate
                .ast
                .arguments_containing(source.span)
                .any(|(call, index)| {
                    index == 0
                        && !call.arguments[0].spread
                        && candidate.ast.peel_ts_sugar_span(call.arguments[0].span) == source.span
                        && lookup
                            .primitive_at_call(candidate, call.span)
                            .is_some_and(|primitive| {
                                lookup.dialect.dynamic_mount_is_unwindowed(primitive)
                            })
                })
        });
        // ADR 0662: `<Dynamic component={K}>` mounts `K` the same way. A reference
        // inside that attribute (`on() ? K : J`) mounts it, not exclusively.
        let (attribute, whole) = dynamic_component_attribute(candidate, at, lookup);
        let (alias_mounts, alias_whole) = dynamic_alias_mount(candidate, at, lookup);
        dynamic |= mounted || attribute || alias_mounts;
        exclusive &= mounted || whole || alias_whole;
    }
    dynamic.then_some(exclusive)
}
