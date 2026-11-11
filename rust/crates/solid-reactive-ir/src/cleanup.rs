//! Leaf-owner diagnostics, and the cleanup-return *classification* the
//! ownership rules consume.
//!
//! Detects `onCleanup`/leaf-owner misuse, and answers the owner-analysis
//! subsystem's question "does this callback hand the owner a cleanup"
//! (`function_cleanup_return_proof`), which SC4001 and SC4001 depend on.
//!
//! It deliberately reports nothing about a returned value's *legality*. Solid
//! 2.0 types the effect callback's return as `(() => void) | void`
//! (`EffectFunction` in `@solidjs/signals`), so every illegal return is
//! already a TypeScript error and AGENTS.md's absolute rule puts it out of
//! scope; the classification below survives because ownership and disposal are
//! not expressible as a type. The ledger entry and the `tsc` evidence are in
//! `fixtures/tsc-oracle/README.md`.

use crate::HashMap;
use std::cell::RefCell;

use solid_dialect::{CleanupRule, Primitive, SlotObserver, SlotOwner, SlotRun, SlotThrows};
use solid_facts::FileFacts;
use solid_facts::core::Span;
use typefacts::{ResolvedCallValidity, RuntimeValueDomain};

use super::{Fix, LeafOwnerOperation, PrimitiveName, SemanticLookup, SymbolId, TextEdit, location};
use crate::execution_role::{ClimbStop, direct_callback_runs};
use crate::owners::{callback_owner_at_call, containing_ast_function};
use crate::pipeline::{AnalysisContext, ProgramDraft};

/// Runs the project-level leaf-owner stage: each file's operations are a
/// late chunk ([`crate::late_chunks`]), reused while the file and every read
/// it recorded (the lookup's answers and hand-outs, and which symbols name an
/// accessor, a setter or an action, and the transport targets it asked)
/// answer as they did.
pub(crate) fn collect_project(
    ctx: &AnalysisContext<'_>,
    chunks: Option<&mut crate::late_chunks::FileChunks<Vec<LeafOwnerOperation>>>,
    draft: &mut ProgramDraft,
) {
    let mut inputs = crate::late_chunks::ChunkInputs::of(ctx);
    inputs.discovery = inputs
        .discovery
        .with_source_call_targets(&draft.source_call_targets);
    let per_file = crate::late_chunks::FileChunks::run(chunks, &inputs, |file| {
        leaf_owner_operations_for_file(
            file,
            ctx.symbol_names,
            inputs.discovery,
            ctx.semantic_lookup,
        )
    });
    let operations = per_file
        .iter()
        .flat_map(|operations| operations.iter().cloned())
        .collect::<Vec<_>>();
    draft.leaf_operations.extend(operations);
}

/// The owner a site runs under, relative to the leaf callback it is written in, once the primitive slots between
/// them are crossed (ADR 0944).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CrossedOwner {
    /// The leaf's own: every slot inherits it, or hands `runWithOwner` the owner the call itself runs under.
    Leaf,
    /// `runWithOwner(null, …)`, `undefined` or another owner (a component's): no leaf owner is current, so
    /// nothing the leaf forbids is.
    Detached,
    /// An owner supplied as a value the walk does not follow: whether it is the leaf's is not proven.
    Unproven,
}

/// What the primitive slots between a site and the leaf callback do to the context the site runs in (ADR 0944):
/// the owner, whether the observer is still active, and whether the primitive of any of them is attributed
/// without the binder confirming it (ADR 0711).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Crossed {
    owner: CrossedOwner,
    /// A crossed slot clears tracking (`untrack`, `runWithOwner`): `resolve` and `until` meet no observer guard.
    observer_cleared: bool,
    /// A crossed slot takes what its callback throws (`isPending`).
    swallows: bool,
    identity_unproven: bool,
}

/// What a leaf operation becomes at a site reached through `crossed` slots, or `None` when the site is no
/// longer this walk's claim: the kind (tracking off turns the observer guard's call into the creation the leaf
/// owner refuses) and whether the owner it needs is not proven.
fn through_slots_kind(
    kind: crate::LeafOwnerOperationKind,
    crossed: &Crossed,
) -> Option<(crate::LeafOwnerOperationKind, bool)> {
    use crate::LeafOwnerOperationKind as Kind;
    let kind = match kind {
        Kind::ObserverGuarded(primitive) if crossed.observer_cleared => Kind::Primitive(primitive),
        other => other,
    };
    let owner_needed = |kind: Kind| match crossed.owner {
        CrossedOwner::Leaf => Some((kind, false)),
        CrossedOwner::Detached => None,
        CrossedOwner::Unproven => Some((kind, true)),
    };
    match kind {
        // A cleanup or a creation meets the leaf owner's guard only where the callback runs under that owner.
        Kind::Cleanup | Kind::Primitive(_) | Kind::ObserverGuarded(_) => owner_needed(kind),
        // The per-key tracking needs an active observer as well.
        Kind::ObserverCacheTracking => {
            if crossed.observer_cleared {
                None
            } else {
                owner_needed(kind)
            }
        }
        // `flush()`'s guard reads the queue's own marker, whatever owner and observer the callback has.
        Kind::Flush | Kind::UnresolvedCallback | Kind::UnmodelledSlot => Some((kind, false)),
    }
}

/// `crossed` once the callback of the primitive call at `call`, which runs in place as `run` says, is crossed
/// (ADR 0944): the owner it runs under, whether tracking is off in it, whether the call takes what it throws, and
/// whether the primitive is attributed without the binder confirming it. `None` for a callback that has an owner of
/// its own (`createRoot` under a leaf throws before it runs the callback) or none whatever the caller's: it is not
/// the leaf's extent.
fn cross_slot(
    file: &FileFacts,
    call: &solid_facts::ast::CallFact,
    run: solid_dialect::InPlaceRun,
    lookup: &SemanticLookup<'_>,
    identity_proven: bool,
    mut crossed: Crossed,
) -> Option<Crossed> {
    use crate::execution_role::{SuppliedOwner, supplied_owner};
    match run.observer {
        SlotObserver::Clears => crossed.observer_cleared = true,
        SlotObserver::Keeps | SlotObserver::Unstated => {}
    }
    match run.throws {
        SlotThrows::Swallows => crossed.swallows = true,
        SlotThrows::Escapes => {}
    }
    crossed.owner = match run.owner {
        SlotOwner::Inherits => crossed.owner,
        SlotOwner::Supplied { argument } => match supplied_owner(file, call, argument, lookup) {
            // The owner the call itself runs under: whatever the walk has so far.
            SuppliedOwner::Ambient => crossed.owner,
            SuppliedOwner::Null | SuppliedOwner::Undefined | SuppliedOwner::Elsewhere => {
                CrossedOwner::Detached
            }
            SuppliedOwner::Unproven => CrossedOwner::Unproven,
        },
        SlotOwner::Creates | SlotOwner::Detached => return None,
    };
    crossed.identity_unproven |= !identity_proven;
    Some(crossed)
}

/// ADR 0944: what the callbacks the primitive call at `call` runs in place do under the leaf owner, for the
/// ones the walk reads as a function it follows: a callback named by value (`untrack(cb)`) is read as a helper
/// called there, and a callback written as a literal is the text of the body being walked (a helper's, or an
/// argument followed), which no lexical loop reads. `named_only` leaves the literals to the lexical walk of the
/// leaf's own extent. `false` when a callback could not be read.
#[allow(clippy::too_many_arguments)]
fn slot_callback_operations(
    resolution: LeafScopeResolution<'_, '_>,
    file: &FileFacts,
    call: &solid_facts::ast::CallFact,
    primitive: &PrimitiveName,
    kinds: &mut Reached,
    visited: &mut Visited,
    depth: usize,
    guaranteed: bool,
    named_only: bool,
) -> bool {
    let Some(kind) = primitive.primitive() else {
        return true;
    };
    let lookup = resolution.lookup;
    let dialect = lookup.dialect;
    let identity_proven = primitive_identity_proven(dialect, file, call, primitive);
    let mut complete = true;
    for (index, argument) in call.arguments.iter().enumerate() {
        // An index after a spread names nothing.
        if argument.spread {
            break;
        }
        let SlotRun::InPlace(run) = dialect.slot_run(kind, index, call.arguments.len()) else {
            continue;
        };
        if named_only && callback_argument_literal(file, argument.span).is_some() {
            continue;
        }
        if crate::runtime_semantics::literal_argument_is_not_callable(argument.runtime_value_kind) {
            continue;
        }
        let peeled = file.ast.peel_ts_sugar_span(argument.span);
        // An accessor, a setter or an action cannot open a leaf scope of its own.
        let symbol = lookup.entities().at(file.path.as_str(), peeled);
        if symbol.is_some_and(|symbol| resolution.tables.safe_call(symbol)) {
            continue;
        }
        let Some(crossed) = cross_slot(
            file,
            call,
            run,
            lookup,
            identity_proven,
            Crossed {
                owner: CrossedOwner::Leaf,
                observer_cleared: false,
                swallows: false,
                identity_unproven: false,
            },
        ) else {
            continue;
        };
        let mut inner = Vec::new();
        // What the slots on the way make of the callback's operations is applied below, so its visits must not
        // suppress the same body entered directly later in the walk (`isPending(g); g();`, ADR 0961); the entries
        // above it still end a cycle.
        let mut entered = visited.clone();
        complete &= argument_body_operations(
            resolution,
            file,
            argument.span,
            &mut inner,
            &mut entered,
            depth,
            guaranteed,
        );
        for (kind, guaranteed, identity_unproven, sites) in inner {
            let Some((kind, owner_unproven)) = through_slots_kind(kind, &crossed) else {
                continue;
            };
            // `flush()` fails with a plain throw, which a call that takes what its callback throws reports
            // nothing for. What `resolve` and `until` do under an observer is left to the stored-throw pass,
            // which knows the host: an obligation in a tracked effect, a refused creation in `onSettled`.
            if crossed.swallows && matches!(kind, crate::LeafOwnerOperationKind::Flush) {
                continue;
            }
            record(
                kinds,
                kind,
                guaranteed && !owner_unproven,
                identity_unproven || crossed.identity_unproven,
                sites,
            );
        }
    }
    complete
}

/// Pushes what a walk reached through the call at `call` as the operations it is reported as: each anchored at the
/// call, the code of the leaf scope that introduces the defect, with the callee it was reached through, what the
/// slots between the call and the leaf make of it (ADR 0944), and a callback the walk could not read as an
/// obligation.
#[allow(clippy::too_many_arguments)]
fn push_reached(
    operations: &mut Vec<LeafOwnerOperation>,
    owner: &str,
    call_site_gate: Option<&typefacts::Location>,
    file: &FileFacts,
    call: &solid_facts::ast::CallFact,
    kinds: Reached,
    complete: bool,
    crossed: Option<&Crossed>,
    step_unproven: bool,
) {
    let via = file.source_text(call.callee).unwrap_or_default().to_owned();
    for (kind, guaranteed, identity_unproven, sites) in kinds {
        let (kind, owner_unproven, slot_identity_unproven) = match crossed {
            Some(crossed) => {
                let Some((kind, owner_unproven)) = through_slots_kind(kind, crossed) else {
                    continue;
                };
                (kind, owner_unproven, crossed.identity_unproven)
            }
            None => (kind, false, false),
        };
        push_crossed(
            operations,
            LeafOwnerOperation {
                kind,
                owner: owner.to_owned(),
                location: location(file.path.shared(), call.callee),
                fix: None,
                call_site_gate: call_site_gate.cloned(),
                uncertain: owner_unproven || slot_identity_unproven,
                possible: !guaranteed || step_unproven,
                via: Some(via.clone()),
                identity_unproven,
                sites,
                computation_error_unobserved: false,
            },
            crossed,
        );
    }
    if !complete {
        operations.push(LeafOwnerOperation {
            kind: crate::LeafOwnerOperationKind::UnresolvedCallback,
            owner: owner.to_owned(),
            location: location(file.path.shared(), call.callee),
            fix: None,
            call_site_gate: call_site_gate.cloned(),
            uncertain: true,
            possible: false,
            via: Some(via),
            identity_unproven: false,
            sites: Vec::new(),
            computation_error_unobserved: false,
        });
    }
}

/// Pushes `operation` found through `crossed` slots, unless it fails with a plain throw and a crossed slot takes
/// what its callback throws (ADR 0944): nothing is reported then. A cleanup or a creation the leaf owner refuses
/// emits its diagnostic before it throws, so it is.
fn push_crossed(
    operations: &mut Vec<LeafOwnerOperation>,
    operation: LeafOwnerOperation,
    crossed: Option<&Crossed>,
) {
    if crossed.is_some_and(|crossed| crossed.swallows) && operation.fails_with_plain_throw() {
        return;
    }
    operations.push(operation);
}

pub(super) fn leaf_owner_operations_for_file(
    file: &FileFacts,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    tables: crate::late_chunks::DiscoveryTables<'_>,
    lookup: &SemanticLookup<'_>,
) -> Vec<LeafOwnerOperation> {
    let Some(file) = lookup.file_by_path(file.path.as_str()) else {
        return Vec::new();
    };
    let entities = lookup.entities();
    let dialect = lookup.dialect;
    let completions = RefCell::default();
    let resolution = LeafScopeResolution {
        lookup,
        symbol_names,
        tables,
        completions: &completions,
    };
    let mut operations = Vec::new();
    for owner_call in &file.ast.calls {
        // The compiler deleted this call. Gated at the owner call, which is
        // this pass's single entry: every operation below -- a forbidden
        // primitive, an unresolved callback, a cross-file helper's operations
        // -- exists only because a leaf scope opens here, and a call the
        // emitter removed opens none. "These nested primitives are never
        // disposed" describes a disposal that never comes due.
        //
        // A positive lookup and an early return, the shape
        // `push_owner_requirement` uses, because this pass consults no
        // execution facts at all and so has no role to give a
        // `DiscardedRendering` arm to. `uncertain` would be wrong for the same
        // reason it is wrong there: nothing here is unproven, the two
        // compilers agree the code is gone.
        //
        // The gate is on the owner call rather than on each push site because
        // deletion travels down, not up: a producer `Elided` span is a single
        // attribute or child value expression, so any nested call it contains
        // is contained with it, while a leaf callback resolved in *another*
        // file is only reachable from this deleted call site.
        if crate::execution_role::discarded_region_contains(file, owner_call.span) {
            continue;
        }
        let owner = crate::call_primitive_name_cached(
            lookup,
            file,
            owner_call,
            entities,
            symbol_names,
            dialect,
        );
        let Some(owner) = owner.as_ref() else {
            continue;
        };
        // The leaf owners: an owner whose callback is the end of the ownership
        // chain, so anything created inside it never gets disposed.
        //
        // Asked of the dialect rather than matched here. The pair this used to
        // hardcode -- `onSettled` and `createTrackedEffect` -- is 2.0-only, so
        // under 1.x these rules could not fire at all. Taking the argument
        // index from the same answer also drops the assumption that the
        // callback is always first.
        let Some(callback_argument) = owner
            .primitive()
            .into_iter()
            .flat_map(|primitive| {
                (0..owner_call.arguments.len()).filter(move |index| {
                    callback_owner_at_call(file, owner_call, primitive, *index, lookup)
                        == Some(solid_dialect::CallbackOwner::Leaf)
                })
            })
            .next()
            .and_then(|index| owner_call.arguments.get(index))
        else {
            continue;
        };
        let region = callback_argument.span;
        // 2.0's `onSettled` is a leaf owner only when the call runs under a
        // live children-capable owner; out-of-band it enqueues a plain
        // callback where none of these operations throw. This pass is
        // lexical, so record the call for the owner fixed point to resolve
        // against the propagated owner graph.
        let call_site_gate = owner
            .primitive()
            .filter(|primitive| dialect.leaf_owner_requires_owned_call_site(*primitive))
            .map(|_| location(file.path.shared(), owner_call.span));
        // Both leaf-scope paths need the leaf callback itself, not just the
        // argument text it was written in, and the answer is the same for
        // every call in the region — compute it once per owner call.
        //
        // The callback must be an exact function value. Direct literals and
        // identifiers are resolved as before. A call is followed only through
        // a closed local return: one unconditional return of a function
        // literal, or one unconditional return of the exact callback
        // parameter. This proves the value received by the owner without
        // trusting a name, package, conditional, or wrapper with unknown
        // behavior. Every other valid opaque shape becomes SC9012.
        let callback_literal = callback_argument_literal(file, region);
        // ADR 0918: a callback named by value (a `const` or function
        // declaration, a `const` copy of one, a receiver-closed literal's
        // slot) whose function is in this file is walked as a literal is,
        // so each operation is reported where it runs in the body.
        let by_value = (callback_literal.is_none() && file.ast.call_at(region).is_none())
            .then(|| callback_value_in_file(lookup, file, region))
            .flatten();
        if callback_literal.is_none() && by_value.is_none() && file.ast.call_at(region).is_none() {
            let callback_span = file.ast.peel_ts_sugar_span(region);
            // Preserve the identifier path's existing diagnostic anchor: the
            // owner receives the identifier itself, while the forbidden body
            // is only evidence for that operation.
            let exact_callback = entities
                .at(file.path.as_str(), callback_span)
                .and_then(|symbol| lookup.function_for_symbol(symbol));
            if let Some((callback_file, callback)) = exact_callback {
                let mut kinds = Vec::new();
                let mut visited = Vec::new();
                let complete = function_forbidden_operations(
                    resolution,
                    callback_file,
                    callback,
                    &mut kinds,
                    &mut visited,
                    8,
                    None,
                    Entry::LeafBody,
                );
                let via = file
                    .source_text(callback_span)
                    .unwrap_or_default()
                    .to_owned();
                for (kind, guaranteed, identity_unproven, sites) in kinds {
                    operations.push(LeafOwnerOperation {
                        kind,
                        owner: owner.to_string(),
                        location: location(file.path.shared(), callback_span),
                        fix: None,
                        call_site_gate: call_site_gate.clone(),
                        uncertain: false,
                        possible: !guaranteed,
                        via: Some(via.clone()),
                        identity_unproven,
                        sites,
                        computation_error_unobserved: false,
                    });
                }
                if complete {
                    continue;
                }
                operations.push(LeafOwnerOperation {
                    kind: crate::LeafOwnerOperationKind::UnresolvedCallback,
                    owner: owner.to_string(),
                    location: location(file.path.shared(), callback_span),
                    fix: None,
                    call_site_gate,
                    uncertain: true,
                    possible: false,
                    via: None,
                    identity_unproven: false,
                    sites: Vec::new(),
                    computation_error_unobserved: false,
                });
                continue;
            }
        }
        let Some((callback_file, leaf_callback)) = callback_literal
            .or(by_value)
            .map(|callback| (file, callback))
            .or_else(|| callback_argument_function(lookup, file, region, 8))
        else {
            let valid_call = lookup
                .resolved_callee_call(file, owner_call.callee)
                .is_some_and(|call| call.validity == ResolvedCallValidity::Valid);
            if !valid_call {
                // Invalid callback shapes belong to TypeScript. A missing
                // validity fact cannot safely distinguish those from a valid
                // opaque callback, so it cannot produce this obligation.
                continue;
            }
            let callback_span = file.ast.peel_ts_sugar_span(region);
            operations.push(LeafOwnerOperation {
                kind: crate::LeafOwnerOperationKind::UnresolvedCallback,
                owner: owner.to_string(),
                location: location(file.path.shared(), callback_span),
                fix: None,
                call_site_gate,
                uncertain: true,
                possible: false,
                via: None,
                identity_unproven: false,
                sites: Vec::new(),
                computation_error_unobserved: false,
            });
            continue;
        };
        let callback_region = leaf_callback.span;
        // A call in IIFE layers the callback runs in place is judged at its
        // own line, as a direct call is: that is where rc.21 raises the
        // code (the strict judge, Chrome). The IIFE call itself takes no
        // helper path, so the operation is not claimed at it too. `None`:
        // past the bound, no answer: an obligation, never a drop.
        // ADR 0929: a sync generator literal stepped once by `.next()` at its
        // own call (`(function* () { … })().next()`) is such a layer for the
        // part of its body that first step runs (ADR 0521's `StepReach`):
        // `Reaches` runs in place, `Never` does not run, and `Unproven` (a
        // `yield` may stop the step first) runs in place on some paths only,
        // the second half of the answer.
        let climb = |span: Span| -> Option<(bool, bool)> {
            let ast = &callback_file.ast;
            let mut at = span;
            let mut step_unproven = false;
            for _ in 0..16 {
                let Some(function) = crate::owners::owning_function(ast, at) else {
                    return Some((false, step_unproven));
                };
                // A parameter list is the helper path's (argument facts,
                // defaults): only a layer with none is the leaf's own text.
                if function.span == leaf_callback.span
                    || function.r#async
                    || !function.parameters.is_empty()
                    || function.rest_parameter
                    || !function.body.contains(at)
                {
                    return Some((false, step_unproven));
                }
                let layer = if function.generator {
                    let Some(step) =
                        crate::execution_role::stepped_generator_in_place(callback_file, at, &[])
                    else {
                        return Some((false, step_unproven));
                    };
                    match step.first_step {
                        crate::execution_role::StepReach::Reaches => {}
                        crate::execution_role::StepReach::Unproven => step_unproven = true,
                        crate::execution_role::StepReach::Never => {
                            return Some((false, step_unproven));
                        }
                    }
                    step.created
                } else {
                    let Some(iife) = ast.calls_with_peeled_callee(function.span).next() else {
                        return Some((false, step_unproven));
                    };
                    iife.span
                };
                if direct_callback_runs(callback_file, leaf_callback.span, layer) {
                    return Some((true, step_unproven));
                }
                at = layer;
            }
            None
        };
        let through_iife = |span: Span| climb(span).map(|(runs, _)| runs);
        // Whether a stepped generator on the way may stop before `span`.
        let step_unproven = |span: Span| climb(span).is_some_and(|(_, unproven)| unproven);
        // ADR 0944: the code at `span` runs in the leaf's own extent through callbacks of primitives (and
        // IIFEs between them) that the dialect says run in place, on the stack of their call: `untrack(() =>
        // onCleanup(..))` is still the leaf owner's child. `Ok` only where a primitive slot is crossed: a
        // path of IIFE layers alone is `through_iife`'s, a direct site needs neither. A callback that has an
        // owner of its own (`createRoot`), runs later or in a computation of its own is not crossed: the
        // path answers why it stopped ([`ClimbStop`], ADR 0959), and the caller decides what each stop means.
        let through_slots = |span: Span| -> Result<Crossed, ClimbStop> {
            let ast = &callback_file.ast;
            let mut at = span;
            let mut slots = Vec::new();
            for _ in 0..16 {
                if direct_callback_runs(callback_file, leaf_callback.span, at) {
                    break;
                }
                let Some(function) = crate::owners::owning_function(ast, at) else {
                    return Err(ClimbStop::NoFunction);
                };
                if !function.body.contains(at) {
                    return Err(ClimbStop::NoFunction);
                }
                // ADR 0959: a callback with a parameter, an async or a generator callback is no layer this path
                // reads: of a primitive that runs it in place it may hold any forbidden operation (an
                // obligation), of any other it runs where that primitive or no slot places it.
                if function.r#async
                    || function.generator
                    || !function.parameters.is_empty()
                    || function.rest_parameter
                {
                    return Err(
                        match crate::execution_role::slot_layer(callback_file, function, lookup) {
                            Some(layer) => match layer.run {
                                SlotRun::InPlace(_) => ClimbStop::UnmodelledInPlace,
                                SlotRun::Computation | SlotRun::Deferred | SlotRun::Unknown => {
                                    ClimbStop::UnmodelledElsewhere
                                }
                            },
                            None => ClimbStop::NotSlot,
                        },
                    );
                }
                if let Some(iife) = ast.calls_with_peeled_callee(function.span).next() {
                    at = iife.span;
                    continue;
                }
                let Some(layer) =
                    crate::execution_role::slot_layer(callback_file, function, lookup)
                else {
                    return Err(ClimbStop::NotSlot);
                };
                // A slot written in a JSX expression runs where the compiler places that expression.
                if crate::execution_role::call_in_jsx_expression(callback_file, layer.call.span) {
                    return Err(ClimbStop::JsxPlaced);
                }
                let identity_proven = crate::call_primitive_name_cached(
                    lookup,
                    callback_file,
                    layer.call,
                    entities,
                    symbol_names,
                    dialect,
                )
                .is_some_and(|name| {
                    primitive_identity_proven(dialect, callback_file, layer.call, &name)
                });
                at = layer.call.span;
                slots.push((layer, identity_proven));
            }
            if !direct_callback_runs(callback_file, leaf_callback.span, at) {
                return Err(ClimbStop::Bound);
            }
            if slots.is_empty() {
                return Err(ClimbStop::NoSlot);
            }
            let mut crossed = Crossed {
                owner: CrossedOwner::Leaf,
                observer_cleared: false,
                swallows: false,
                identity_unproven: false,
            };
            // The outermost slot first: the owner a site runs under is the innermost owner the slots set.
            for (layer, identity_proven) in slots.iter().rev() {
                match layer.run {
                    SlotRun::InPlace(run) => {
                        crossed = cross_slot(
                            callback_file,
                            layer.call,
                            run,
                            lookup,
                            *identity_proven,
                            crossed,
                        )
                        .ok_or(ClimbStop::Declined)?;
                    }
                    SlotRun::Computation | SlotRun::Deferred | SlotRun::Unknown => {
                        return Err(ClimbStop::RunsElsewhere);
                    }
                }
            }
            Ok(crossed)
        };
        // ADR 0883: JSX in the leaf's own text creates a computation or a root
        // with no call to name it. A creating call elsewhere is the helper
        // paths'; JSX there is the owner rule's (`jsx-effect`).
        for site in crate::owners::jsx_sites(callback_file, lookup) {
            if site.kind == crate::owners::JsxSiteKind::MaybeRenderEffect
                || site.in_getter
                || !callback_region.contains(site.span)
            {
                continue;
            }
            // ADR 0944: JSX in the callback of a primitive that runs in place creates under the callback's owner.
            let crossed = through_slots(site.span).ok();
            if !(direct_callback_runs(callback_file, leaf_callback.span, site.span)
                || through_iife(site.span) != Some(false)
                || crossed.is_some())
            {
                continue;
            }
            let (kind, slots_unproven) = match &crossed {
                Some(crossed) => {
                    let Some((kind, owner_unproven)) = through_slots_kind(
                        crate::LeafOwnerOperationKind::Primitive("JSX".into()),
                        crossed,
                    ) else {
                        continue;
                    };
                    (kind, owner_unproven || crossed.identity_unproven)
                }
                None => (
                    crate::LeafOwnerOperationKind::Primitive("JSX".into()),
                    false,
                ),
            };
            push_crossed(
                &mut operations,
                LeafOwnerOperation {
                    kind,
                    owner: owner.to_string(),
                    location: location(callback_file.path.shared(), site.span),
                    fix: None,
                    call_site_gate: call_site_gate.clone(),
                    uncertain: slots_unproven,
                    possible: step_unproven(site.span),
                    via: None,
                    identity_unproven: false,
                    sites: Vec::new(),
                    computation_error_unobserved: false,
                },
                crossed.as_ref(),
            );
        }
        for call in &callback_file.ast.calls {
            if call.span == owner_call.span || !callback_region.contains(call.span) {
                continue;
            }
            // And the call must sit in that callback's own synchronous
            // extent: a call inside a nested function (an event handler built
            // in the callback) runs later, in that function's scope, where
            // the leaf scope is no longer live.
            // ADR 0454: a call in the callback's own parameter list runs in
            // the leaf scope when the owner calls it without the argument
            // (rc.17 calls `onSettled`'s callback with none), which nothing
            // here proves for every owner: it is judged like a call in the
            // body and kept uncertifiable below.
            let own_list = crate::owners::owning_function(&callback_file.ast, call.span)
                .is_some_and(|owner| {
                    owner.span == leaf_callback.span && !owner.body.contains(call.span)
                });
            // ADR 0492: a call in callbacks that project helpers call in their
            // own body (ADR 0469's climb), written in the callback, runs on
            // the stack of the outermost helper call, as ADR 0478 reads it:
            // not in a parameter list, not after an await. An IIFE, an inline
            // standard-library callback and a named function are the helper
            // and host paths' (`helper_forbidden_operations`, the host-callback
            // census), which judge them at their own call.
            let in_place = || {
                let Some(climb) =
                    crate::execution_role::climb_in_place_layers(callback_file, call.span, lookup)
                else {
                    return false;
                };
                if climb.in_list
                    || climb.suspended.is_some()
                    || !direct_callback_runs(callback_file, leaf_callback.span, climb.at)
                {
                    return false;
                }
                let ast = &callback_file.ast;
                let mut at = call.span;
                for _ in 0..16 {
                    if at == climb.at {
                        return true;
                    }
                    let Some(function) = crate::owners::owning_function(ast, at) else {
                        return false;
                    };
                    let Some((helper, _)) =
                        ast.arguments_containing(function.span)
                            .find(|(container, index)| {
                                ast.peel_ts_sugar_span(container.arguments[*index].span)
                                    == function.span
                            })
                    else {
                        return false;
                    };
                    // A project function: no standard-library or
                    // dependency declaration (a primitive's callback is the
                    // owner model's, not a helper's).
                    let project = lookup
                        .resolved_callee_call(callback_file, helper.callee)
                        .and_then(|resolved| resolved.declaration.as_ref())
                        .is_some_and(|declaration| {
                            !declaration.standard_library
                                && !declaration.source_file.contains("/node_modules/")
                        });
                    if !project {
                        return false;
                    }
                    at = helper.span;
                }
                // The climb's own bound: the last step may land exactly on it.
                at == climb.at
            };
            // Only a call whose function is itself an argument can be in a
            // helper's callback, a syntactic test that spares the climb for
            // every other call (this pass runs on every build).
            let handed = || {
                crate::owners::owning_function(&callback_file.ast, call.span).is_some_and(
                    |function| {
                        function.span != leaf_callback.span
                            && callback_file.ast.arguments_containing(function.span).any(
                                |(container, index)| {
                                    callback_file
                                        .ast
                                        .peel_ts_sugar_span(container.arguments[index].span)
                                        == function.span
                                },
                            )
                    },
                )
            };
            let iife_call = callback_file
                .ast
                .function_at(callback_file.ast.peel_ts_sugar_span(call.callee))
                .is_some_and(|literal| {
                    !literal.r#async
                        // ADR 0929: a generator literal the climb steps
                        // through (`(function* () { … })().next()`).
                        && (!literal.generator
                            || (crate::owners::generator_iterator_used(callback_file, literal)
                                .is_some_and(|created| created.span == call.span)
                                && crate::owners::iterator_stepped_at(callback_file, call.span)))
                        && literal.parameters.is_empty()
                        && !literal.rest_parameter
                })
                && (direct_callback_runs(callback_file, leaf_callback.span, call.span)
                    || through_iife(call.span) != Some(false));
            if iife_call {
                continue;
            }
            if through_iife(call.span).is_none() {
                operations.push(LeafOwnerOperation {
                    kind: crate::LeafOwnerOperationKind::UnresolvedCallback,
                    owner: owner.to_string(),
                    location: location(callback_file.path.shared(), call.callee),
                    fix: None,
                    call_site_gate: call_site_gate.clone(),
                    uncertain: true,
                    possible: false,
                    via: None,
                    identity_unproven: false,
                    sites: Vec::new(),
                    computation_error_unobserved: false,
                });
                continue;
            }
            // ADR 0944: through the callbacks of primitives that run in place.
            let path = through_slots(call.span);
            let crossed = path.as_ref().ok().copied();
            if !own_list
                && !direct_callback_runs(callback_file, leaf_callback.span, call.span)
                && through_iife(call.span) != Some(true)
                && !(handed() && in_place())
                && crossed.is_none()
            {
                // ADR 0959: what the stop of the climb means. A callback of a primitive that runs it in place
                // that the climb cannot read, and a climb that ran out of layers, are obligations of this rule,
                // unless the call is one no operation can come from (a primitive the leaf owner never refuses);
                // every other stop is the dialect's or the compiler's proof that the code runs elsewhere, or a
                // layer other walks own (an ordinary function, an IIFE, a direct site).
                let obligation = path.err().is_some_and(ClimbStop::owes_obligation);
                if obligation {
                    // A call of a primitive the leaf owner never refuses cannot operate; a call that is no
                    // primitive (a helper, a member) may, which is the polarity of an obligation.
                    // An accessor or a setter opens no leaf scope of its own (the helper walk's `safe_call`).
                    let reads_or_writes = entities
                        .at(
                            callback_file.path.as_str(),
                            callback_file.ast.peel_ts_sugar_span(call.callee),
                        )
                        .is_some_and(|symbol| {
                            resolution.tables.safe_call(symbol) && !resolution.tables.action(symbol)
                        });
                    let may_operate = if reads_or_writes {
                        false
                    } else if let Some(primitive) = crate::call_primitive_name_cached(
                        lookup,
                        callback_file,
                        call,
                        entities,
                        symbol_names,
                        dialect,
                    ) {
                        forbidden_operation_kind(dialect, callback_file, call, &primitive).is_some()
                    } else {
                        true
                    };
                    if may_operate {
                        operations.push(LeafOwnerOperation {
                            kind: crate::LeafOwnerOperationKind::UnmodelledSlot,
                            owner: owner.to_string(),
                            location: location(callback_file.path.shared(), call.callee),
                            fix: None,
                            call_site_gate: call_site_gate.clone(),
                            uncertain: true,
                            possible: true,
                            via: None,
                            identity_unproven: false,
                            sites: Vec::new(),
                            computation_error_unobserved: false,
                        });
                    }
                }
                continue;
            }
            let primitive = crate::call_primitive_name_cached(
                lookup,
                callback_file,
                call,
                entities,
                symbol_names,
                dialect,
            );
            let Some(primitive) = primitive else {
                // Not a primitive: an exactly-resolved in-project helper
                // called here runs its synchronous extent in this leaf
                // scope, so a forbidden operation inside it executes here.
                let mut kinds = Vec::new();
                let mut visited = Vec::new();
                let complete = helper_forbidden_operations(
                    resolution,
                    callback_file,
                    call,
                    &mut kinds,
                    &mut visited,
                    8,
                    None,
                    true,
                );
                if kinds.is_empty() && complete {
                    continue;
                }
                push_reached(
                    &mut operations,
                    &owner.to_string(),
                    call_site_gate.as_ref(),
                    callback_file,
                    call,
                    kinds,
                    complete,
                    crossed.as_ref(),
                    step_unproven(call.span),
                );
                continue;
            };
            // ADR 0944: a callback named by value that a primitive runs in place (`untrack(cb)`) is a function
            // called here, read as a helper is; one written as a literal is text of this walk.
            {
                let mut named = Vec::new();
                let mut named_visited = Vec::new();
                let named_complete = slot_callback_operations(
                    resolution,
                    callback_file,
                    call,
                    &primitive,
                    &mut named,
                    &mut named_visited,
                    8,
                    true,
                    true,
                );
                if !named.is_empty() || !named_complete {
                    push_reached(
                        &mut operations,
                        &owner.to_string(),
                        call_site_gate.as_ref(),
                        callback_file,
                        call,
                        named,
                        named_complete,
                        crossed.as_ref(),
                        step_unproven(call.span),
                    );
                }
            }
            let Some(kind) = forbidden_operation_kind(dialect, callback_file, call, &primitive)
            else {
                continue;
            };
            // ADR 0944: what the slots on the way make of the call; an owner or a primitive identity they
            // leave unproven keeps it an obligation.
            let (kind, slots_unproven) = match &crossed {
                Some(crossed) => {
                    let Some((kind, owner_unproven)) = through_slots_kind(kind, crossed) else {
                        continue;
                    };
                    (kind, owner_unproven || crossed.identity_unproven)
                }
                None => (kind, false),
            };
            // ADR 0711: the attribution must name the primitive the callee
            // is bound to; otherwise the operation is an obligation.
            let identity_proven =
                primitive_identity_proven(dialect, callback_file, call, &primitive);
            // Only `onCleanup` has a rewrite -- return the cleanup
            // instead of registering it -- and only where the owner reads
            // a returned function as cleanup. That is a 2.0 idea: 1.x's
            // leaf owner threads return values elsewhere, so offering the
            // rewrite there would introduce a bug, not fix one.
            // A body named by value may have other callers, where the
            // rewrite would change what runs (ADR 0918).
            let fix = (by_value.is_none()
                && crossed.is_none()
                && primitive.primitive() == Some(Primitive::OnCleanup)
                && owner
                    .primitive()
                    .is_some_and(|owner| dialect.accepts_cleanup_return(owner)))
            .then(|| terminal_cleanup_fix(callback_file, leaf_callback.span, call))
            .flatten();
            push_crossed(
                &mut operations,
                LeafOwnerOperation {
                    kind,
                    owner: owner.to_string(),
                    location: location(callback_file.path.shared(), call.callee),
                    fix,
                    call_site_gate: call_site_gate.clone(),
                    uncertain: !identity_proven || slots_unproven,
                    possible: step_unproven(call.span),
                    via: None,
                    identity_unproven: !identity_proven,
                    sites: Vec::new(),
                    computation_error_unobserved: false,
                },
                crossed.as_ref(),
            );
        }
        // ADR 0537: a tagged template written in the callback's own extent
        // calls its tag there (ADR 0531), as a call of it would.
        for template in &callback_file.ast.tagged_templates {
            // ADR 0929: in a stepped generator's first step too, which the
            // call loop above no longer claims at the generator's own call.
            let in_generator =
                crate::owners::containing_ast_function(&callback_file.ast, template.span)
                    .is_some_and(|function| function.generator);
            if !callback_region.contains(template.span)
                || !(direct_callback_runs(callback_file, leaf_callback.span, template.span)
                    || (in_generator && through_iife(template.span) != Some(false)))
            {
                continue;
            }
            let step_unproven =
                step_unproven(template.span) || through_iife(template.span).is_none();
            let mut kinds = Vec::new();
            let mut visited = Vec::new();
            let complete = tag_forbidden_operations(
                resolution,
                callback_file,
                template,
                &mut kinds,
                &mut visited,
                8,
                true,
            );
            let via = callback_file
                .source_text(template.tag)
                .unwrap_or_default()
                .to_owned();
            for (kind, guaranteed, identity_unproven, sites) in kinds {
                operations.push(LeafOwnerOperation {
                    kind,
                    owner: owner.to_string(),
                    location: location(callback_file.path.shared(), template.tag),
                    fix: None,
                    call_site_gate: call_site_gate.clone(),
                    uncertain: false,
                    possible: !guaranteed || step_unproven,
                    via: Some(via.clone()),
                    identity_unproven,
                    sites,
                    computation_error_unobserved: false,
                });
            }
            if !complete {
                operations.push(LeafOwnerOperation {
                    kind: crate::LeafOwnerOperationKind::UnresolvedCallback,
                    owner: owner.to_string(),
                    location: location(callback_file.path.shared(), template.tag),
                    fix: None,
                    call_site_gate: call_site_gate.clone(),
                    uncertain: true,
                    possible: false,
                    via: Some(via),
                    identity_unproven: false,
                    sites: Vec::new(),
                    computation_error_unobserved: false,
                });
            }
        }
    }
    // An operation in an instance field initializer runs when an instance is
    // constructed, not while the leaf callback the class is written in runs
    // (review round 2): no proven forbidden call.
    for operation in &mut operations {
        let Some(operation_file) = lookup.file_by_path(operation.location.path.as_ref()) else {
            continue;
        };
        let at = Span::new(
            u32::try_from(operation.location.start_byte).unwrap_or(u32::MAX),
            u32::try_from(operation.location.end_byte).unwrap_or(u32::MAX),
        );
        // ADR 0454: written in a parameter list, so it runs only when its
        // function is called without the argument.
        if crate::owners::in_parameter_list(&operation_file.ast, at) {
            operation.uncertain = true;
            operation.possible = false;
        }
        if crate::execution_role::inside_instance_field(operation_file, at) {
            operation.uncertain = true;
        }
    }
    operations
}

/// Resolve the function value an owner actually receives at an exact callback
/// argument. In addition to literals and identifiers, this follows a very
/// small closed-world return shape for local functions: the callee must be an
/// exact local function, and any compiler-resolved call fact present must be
/// valid. It must have exactly one unconditional return, and that return must
/// be a function literal or the exact callback parameter. A conditional
/// return, local alias, member, package function, or invalid fact remains
/// unresolved.
fn callback_argument_function<'a>(
    lookup: &SemanticLookup<'a>,
    file: &'a FileFacts,
    argument: Span,
    depth: usize,
) -> Option<(&'a FileFacts, &'a solid_facts::ast::FunctionFact)> {
    if depth == 0 {
        return None;
    }
    if let Some(function) = callback_argument_literal(file, argument) {
        return Some((file, function));
    }
    let raw_argument = argument;
    let argument = file.ast.peel_ts_sugar_span(raw_argument);
    let Some(call) = file
        .ast
        .call_at(raw_argument)
        .or_else(|| file.ast.call_at(argument))
    else {
        let symbol = lookup.entities().at(file.path.as_str(), argument)?;
        return lookup.function_for_symbol(symbol);
    };
    let symbol = lookup.entities().at(file.path.as_str(), call.callee)?;
    let (factory_file, factory) = lookup.function_for_symbol(symbol)?;
    if lookup
        .resolved_callee_call(file, call.callee)
        .is_some_and(|resolved| resolved.validity != ResolvedCallValidity::Valid)
    {
        return None;
    }
    let returned = exact_function_return(factory_file, factory)?;
    let returned_value = returned.argument?;
    match returned.value {
        solid_facts::ast::ReturnValueKind::Function => factory_file
            .ast
            .function_at(returned_value)
            .map(|function| (factory_file, function)),
        solid_facts::ast::ReturnValueKind::Identifier => {
            let returned_symbol = lookup
                .entities()
                .at(factory_file.path.as_str(), returned_value)?;
            let parameter_index = factory.parameters.iter().position(|parameter| {
                parameter.names.iter().any(|name| {
                    lookup.entities().at(factory_file.path.as_str(), name.span)
                        == Some(returned_symbol)
                })
            })?;
            let argument = call.arguments.get(parameter_index)?;
            if argument.spread {
                return None;
            }
            callback_argument_function(lookup, file, argument.span, depth - 1)
        }
        solid_facts::ast::ReturnValueKind::Undefined
        | solid_facts::ast::ReturnValueKind::Call
        | solid_facts::ast::ReturnValueKind::Member
        | solid_facts::ast::ReturnValueKind::Other => None,
    }
}

/// ADR 0918: the function a callback argument holds by value, when it is
/// declared in `file`: what [`crate::owners::exact_function_argument`]
/// resolves (a `const` holding the function, a chain of `const` copies, a
/// receiver-closed object literal's data slot), or else the exact function
/// the identifier's symbol names (a function declaration). A generator runs
/// none of its body when called, so it is never one; neither is a function
/// in another file, which stays the summary at the argument.
fn callback_value_in_file<'a>(
    lookup: &SemanticLookup<'a>,
    file: &'a FileFacts,
    argument: Span,
) -> Option<&'a solid_facts::ast::FunctionFact> {
    let function = match crate::owners::exact_function_argument(file, argument, lookup) {
        Some((path, span)) => {
            if path != file.path.as_str() {
                return None;
            }
            file.ast.function_at(span)?
        }
        None => {
            let symbol = lookup
                .entities()
                .at(file.path.as_str(), file.ast.peel_ts_sugar_span(argument))?;
            let (callback_file, function) = lookup.function_for_symbol(symbol)?;
            if callback_file.path != file.path {
                return None;
            }
            function
        }
    };
    (!function.generator).then_some(function)
}

/// The sole unconditional return of a local callback adapter.
fn exact_function_return<'a>(
    file: &'a FileFacts,
    function: &'a solid_facts::ast::FunctionFact,
) -> Option<&'a solid_facts::ast::ReturnFact> {
    if let Some(returned) = function.expression_return.as_ref() {
        return Some(returned);
    }
    let returned = file
        .ast
        .returns
        .iter()
        .filter(|returned| {
            containing_ast_function(&file.ast, returned.span)
                .is_some_and(|owner| owner.span == function.span)
        })
        .collect::<Vec<_>>();
    match returned.as_slice() {
        [only] if !only.conditional => Some(only),
        _ => None,
    }
}

/// The function literal written *directly* in a callback argument, or `None`
/// when the argument is any other expression.
///
/// Every rule that reasons about what runs inside the callback a callee
/// receives needs this, and only a literal in argument position makes the
/// enclosing argument text and that callback the same region.
/// `owner(makeCb())` and `owner(wrap(() => …))` both contain a call — the
/// first evaluated under the enclosing owner before the leaf scope exists, the
/// second handed to an opaque wrapper that decides whether and when it runs —
/// so neither is proof and both fail closed here.
///
/// Parentheses and whitespace are the only fillers a literal tolerates
/// between the argument's bounds and its own; anything else means the
/// function is an operand rather than the argument.
pub(crate) fn callback_argument_literal(
    file: &FileFacts,
    argument: Span,
) -> Option<&solid_facts::ast::FunctionFact> {
    let function = file
        .ast
        .functions_within(argument)
        .max_by_key(|function| function.span.end - function.span.start)?;
    let start = usize::try_from(argument.start).ok()?;
    let end = usize::try_from(argument.end).ok()?;
    let inner_start = usize::try_from(function.span.start).ok()?;
    let inner_end = usize::try_from(function.span.end).ok()?;
    // Opening parentheses only before the function and closing ones only
    // after it: `(() => get)()` ends in `)()`, an invocation of the literal,
    // which is its result and not the function.
    let filler = |text: &str, paren: u8| {
        text.bytes()
            .all(|byte| byte.is_ascii_whitespace() || byte == paren)
    };
    let (before, after) = (
        file.source.get(start..inner_start)?,
        file.source.get(inner_end..end)?,
    );
    (filler(before, b'(')
        && filler(after, b')')
        && before.bytes().filter(|byte| *byte == b'(').count()
            == after.bytes().filter(|byte| *byte == b')').count())
    .then_some(function)
}

/// The leaf-owner operation kind a call performs under `dialect`, or `None`
/// when the call is not a forbidden operation.
///
/// Shared by the lexical path (a primitive written inside the leaf callback)
/// and the dynamic-extent path (the same primitive reached through an exactly
/// resolved helper), so the two cannot answer differently for one call.
fn forbidden_operation_kind(
    dialect: &dyn solid_dialect::Dialect,
    file: &FileFacts,
    call: &solid_facts::ast::CallFact,
    primitive: &PrimitiveName,
) -> Option<crate::LeafOwnerOperationKind> {
    let kind = primitive.primitive()?;
    leaf_forbids(dialect, file, call, kind).then(|| match kind {
        Primitive::OnCleanup => crate::LeafOwnerOperationKind::Cleanup,
        Primitive::Flush => crate::LeafOwnerOperationKind::Flush,
        _ if dialect.leaf_call_throws_observer_guard(kind) => {
            crate::LeafOwnerOperationKind::ObserverGuarded(primitive.to_string())
        }
        _ => crate::LeafOwnerOperationKind::Primitive(primitive.to_string()),
    })
}

/// ADR 0711: whether the binder confirms the primitive attributed to `call`:
/// the callee is a named import binding whose imported name is that
/// primitive's, or a non-computed member of a namespace import of a module the
/// dialect owns whose property is. Anything else (a local alias, a wrapper's
/// value, an import whose target the producer could not read) proves no
/// identity, whatever symbol the producer put at the callee.
fn primitive_identity_proven(
    dialect: &dyn solid_dialect::Dialect,
    file: &FileFacts,
    call: &solid_facts::ast::CallFact,
    primitive: &PrimitiveName,
) -> bool {
    primitive_identity_proven_at(dialect, file, call.callee, primitive)
}

/// [`primitive_identity_proven`] for the value at `callee`, whether or not a
/// call holds it (ADR 0825: a primitive used as a value).
pub(crate) fn primitive_identity_proven_at(
    dialect: &dyn solid_dialect::Dialect,
    file: &FileFacts,
    callee: Span,
    primitive: &PrimitiveName,
) -> bool {
    use solid_facts::ast::ImportKind;
    let Some(kind) = primitive.primitive() else {
        return false;
    };
    let ast = &file.ast;
    let names = |name: &str| dialect.primitive(name) == Some(kind);
    let binding_at = |declaration: Span| {
        ast.imports.iter().find_map(|import| {
            import
                .bindings
                .iter()
                .find(|binding| binding.local.span == declaration)
                .map(|binding| (import, binding))
        })
    };
    let callee = ast.peel_ts_sugar_span(callee);
    if let Some(member) = ast.member_at(callee) {
        let object = ast.peel_ts_sugar_span(member.object);
        return ast
            .reference_declaration(object)
            .and_then(binding_at)
            .is_some_and(|(import, binding)| {
                binding.kind == ImportKind::Namespace
                    && dialect.owns_module(import.module.as_str())
                    && file.source_text(member.property).is_some_and(names)
            });
    }
    ast.reference_declaration(callee)
        .and_then(binding_at)
        .is_some_and(|(_, binding)| {
            binding.kind != ImportKind::Namespace && binding.imported.as_deref().is_some_and(names)
        })
}

/// Whether the dialect forbids this primitive call under a leaf owner.
fn leaf_forbids(
    dialect: &dyn solid_dialect::Dialect,
    file: &FileFacts,
    call: &solid_facts::ast::CallFact,
    kind: Primitive,
) -> bool {
    match dialect.cleanup_rule(kind) {
        CleanupRule::Always => true,
        // `createSignal(fn)` registers work; `createSignal(0)` does not.
        // Flattening this arm into the unconditional one would turn every
        // plainly seeded signal under a leaf owner into a false positive.
        CleanupRule::WhenFirstArgumentIsFunction => call
            .arguments
            .first()
            .is_some_and(|argument| file.ast.functions_within(argument.span).next().is_some()),
        CleanupRule::Never => false,
    }
}

/// ADR 0475: whether this primitive call creates an owner or computation the
/// leaf-owner rules forbid (SC3001's `Primitive` kind: not `onCleanup` or
/// `flush`, which create nothing). The runtime refuses the same creations in
/// an effect apply, where no owner is current.
pub(crate) fn creates_forbidden_primitive(
    dialect: &dyn solid_dialect::Dialect,
    file: &FileFacts,
    call: &solid_facts::ast::CallFact,
    kind: Primitive,
) -> bool {
    !matches!(kind, Primitive::OnCleanup | Primitive::Flush)
        && leaf_forbids(dialect, file, call, kind)
}

/// Collects the forbidden-operation kinds an exactly-resolved helper performs
/// in its own *synchronous extent* — its body minus nested function bodies,
/// which calling the helper does not execute — following further exact helper
/// calls transitively up to `depth`.
///
/// This is the dynamic-extent half of the leaf-owner rules: `onCleanup` or
/// `flush` in a helper called synchronously from a leaf callback throws at
/// runtime exactly as the inline spelling does. A callee comes from an exact
/// entity join or the shared source callable transport. An unresolved,
/// ambiguous, or unavailable package callee stays explicitly uncertifiable.
/// The project-wide lookups both leaf-scope walkers thread through every hop.
#[derive(Clone, Copy)]
struct LeafScopeResolution<'a, 'lookup> {
    lookup: &'a SemanticLookup<'lookup>,
    symbol_names: &'a HashMap<SymbolId, SymbolId>,
    /// Asked, recorded: which calls cannot open a leaf scope of their own
    /// (accessors, setters, and actions); the setters among them, whose
    /// function argument runs (ADR 0210); ADR 0550: the actions, whose first
    /// step runs in the scope; and the transport targets.
    tables: crate::late_chunks::DiscoveryTables<'a>,
    /// ADR 0275: per function, the calls it runs on every normal completion.
    completions: &'a RefCell<HashMap<(String, Span), Vec<Span>>>,
}

/// A forbidden operation the walk reached, and whether the leaf scope's call
/// runs it on every normal completion (ADR 0275). Only a guaranteed one is a
/// violation; the others are proof obligations. The last field says no proven
/// run names the primitive: only an attribution the binder does not confirm
/// (ADR 0711) reached it.
type Reached = Vec<(
    crate::LeafOwnerOperationKind,
    bool,
    bool,
    Vec<typefacts::Location>,
)>;

/// `sites`: where the operation is written (ADR 0942). An operation reached through a helper is reported at the helper call, so the site
/// itself is kept to let another rule's hand-off to this one (SC4001's `LeafOwned`) find its row.
fn record(
    kinds: &mut Reached,
    kind: crate::LeafOwnerOperationKind,
    guaranteed: bool,
    identity_unproven: bool,
    sites: Vec<typefacts::Location>,
) {
    match kinds.iter_mut().find(|(known, ..)| *known == kind) {
        Some((_, known, open, known_sites)) => {
            *known |= guaranteed;
            *open &= identity_unproven;
            for site in sites {
                if !known_sites.contains(&site) {
                    known_sites.push(site);
                }
            }
        }
        None => kinds.push((kind, guaranteed, identity_unproven, sites)),
    }
}

/// The function bodies a walk has entered, by the file, the span and how they were entered ([`entry_key`]): a cycle
/// ends where a body is entered again the same way, and a body entered again under another context (another
/// argument standing, a stepped action, another `this`) is walked again (ADR 0961).
type Visited = Vec<(String, Span, String)>;

/// How the walk entered a function body (ADR 0275).
#[derive(Clone, Copy)]
enum Entry<'v> {
    /// The leaf callback itself. Every call written directly in it is
    /// reached, under the statically-present-code premise.
    LeafBody,
    /// Called from the leaf scope. A call in the body runs at that call only
    /// when it runs on every normal completion of the body and the hop here
    /// was itself `guaranteed`: a branch may depend on this call's arguments.
    Called { guaranteed: bool },
    /// ADR 0280: called from the leaf scope by a call whose arguments prove
    /// `values` for the body's parameters, so a branch they decide is
    /// decided for this call.
    CalledWith {
        guaranteed: bool,
        values: &'v [solid_facts::ast::ParameterValue],
    },
    /// ADR 0550: an action's stepped generator, entered by a call of the
    /// action from the leaf scope: its first step runs there, on the caller's
    /// stack and under its owner (`@solidjs/signals@2.0.0-rc.17`
    /// `dist/dev.js:1573-1600`).
    ActionStep { guaranteed: bool },
}

/// The identity of how a function body is entered, for [`Visited`]: the kind of entry, whether it is guaranteed, the
/// argument standings it proves, and the class `this` is exact for.
fn entry_key(entry: Entry<'_>, exact_this: &ExactThis) -> String {
    let how = match entry {
        Entry::LeafBody => "leaf".to_owned(),
        Entry::Called { guaranteed } => format!("called:{guaranteed}"),
        Entry::CalledWith { guaranteed, values } => format!("with:{guaranteed}:{values:?}"),
        Entry::ActionStep { guaranteed } => format!("step:{guaranteed}"),
    };
    format!("{how}|{exact_this:?}")
}

/// Whether `inner`, written directly in `function`, runs on every call that
/// enters `function` the way `entry` says.
fn reached_on_every_call(
    resolution: LeafScopeResolution<'_, '_>,
    file: &FileFacts,
    function: &solid_facts::ast::FunctionFact,
    inner: Span,
    entry: Entry<'_>,
) -> bool {
    match entry {
        Entry::LeafBody => true,
        Entry::Called { guaranteed: false }
        | Entry::CalledWith {
            guaranteed: false, ..
        }
        | Entry::ActionStep { guaranteed: false } => false,
        // ADR 0550: the calls the first step runs on every completion before
        // the generator's first `yield`.
        Entry::ActionStep { guaranteed: true } => {
            resolution
                .completions
                .borrow_mut()
                .entry((format!("{}#first-step", file.path.as_str()), function.span))
                .or_insert_with(|| {
                    solid_facts::ast::first_step_unconditional_calls(
                        std::path::Path::new(file.path.as_str()),
                        &file.source,
                        function.span,
                        &[],
                    )
                    .unwrap_or_default()
                })
                .contains(&inner)
                && !resolution
                    .lookup
                    .never_returning_call_before(file, function, inner)
        }
        Entry::Called { guaranteed: true }
        | Entry::CalledWith {
            guaranteed: true, ..
        } => {
            resolution
            .completions
            .borrow_mut()
            .entry((file.path.as_str().to_owned(), function.span))
            .or_insert_with(|| {
                // Under a declared host the analysed source's host-constant
                // conditions are already literals (ADR 0166), as for the
                // owner requirements of ADR 0161. An `async` function states
                // its calls before its first `await`; a generator or an
                // unparsable file states nothing.
                solid_facts::ast::synchronously_unconditional_calls(
                    std::path::Path::new(file.path.as_str()),
                    &file.source,
                    function.span,
                    &[],
                )
                .unwrap_or_default()
            })
            .contains(&inner)
            // ADR 0278: a `never`-returning guard before it may throw into a
            // caller that catches and carries on.
            && !resolution
                .lookup
                .never_returning_call_before(file, function, inner)
        }
    }
}

/// ADR 0209: the class a call's `this` is exactly an instance of, when the
/// function the call is written in is a method declared in that class and was
/// entered through such an instance. Inside an inherited method `this` may be
/// a subclass instance that overrides what `this.m` names, so no class is
/// carried there.
type ExactThis = Option<(String, Span)>;

/// ADR 0209: the method a member call runs, when its receiver is exactly an
/// instance of one project class.
///
/// The receiver is either `this` under [`ExactThis`], or a `const` bound
/// directly to `new C(…)` whose callee resolves to a project class. Then the
/// object's class is exactly `C`. The method TypeScript resolves runs when it
/// is declared in `C` itself, or, through `this` in `C`'s own method, when `C`
/// inherits it. Not when
/// some assignment writes a member of that name, or anything writes through a
/// prototype in `C`'s chain (`member_may_be_reassigned_on_class`, ADR 0314). The method itself is
/// returned with the class its `this` is exact for, when it is declared in
/// that class.
fn exact_instance_method<'a>(
    lookup: &SemanticLookup<'a>,
    file: &FileFacts,
    call: &solid_facts::ast::CallFact,
    exact_this: &ExactThis,
) -> Option<(&'a FileFacts, &'a solid_facts::ast::FunctionFact, ExactThis)> {
    if call.construct {
        return None;
    }
    let callee = file.ast.peel_ts_sugar_span(call.callee);
    let member = file.ast.member_at(callee)?;
    let name = file.source_text(member.property)?;
    let receiver = file.ast.peel_ts_sugar_span(member.object);
    let through_this = file.source_text(receiver) == Some("this");
    let class = if through_this {
        let (path, span) = exact_this
            .clone()
            .filter(|(path, _)| path == file.path.as_str())?;
        let class_file = lookup.file_by_path(path.as_str())?;
        let class = class_file
            .ast
            .classes
            .iter()
            .find(|class| class.span == span)?;
        // ADR 0314: no write reaches this class's chain under `name`.
        if !lookup.class_instance_is_exact(class_file, class, name, 4)
            || lookup.member_may_be_reassigned_on_class(name, class_file, class)
        {
            return None;
        }
        (path, span)
    } else {
        let (binding_file, binding, _) =
            lookup.binding_at_reference(file.path.as_str(), receiver)?;
        if !binding.immutable || binding.shape != solid_facts::ast::BindingShape::Identifier {
            return None;
        }
        let construction = binding_file
            .ast
            .call_at(binding_file.ast.peel_ts_sugar_span(binding.initializer?))
            .filter(|construction| construction.construct)?;
        let class_symbol = lookup.callee_symbol(binding_file, construction.callee)?;
        let (class_file, class) = lookup.class_for_symbol(class_symbol)?;
        // The constructor chain must hand back this instance, and no own
        // field may shadow the method (ADR 0209 as amended).
        if !lookup.class_instance_is_exact(class_file, class, name, 4)
            || lookup.member_may_be_reassigned_on_class(name, class_file, class)
        {
            return None;
        }
        (class_file.path.to_string(), class.span)
    };
    let symbol = lookup.callee_symbol(file, call.callee)?;
    let (method_file, method) = lookup.function_for_symbol(symbol)?;
    method.method_name.as_ref()?;
    let own = method_file.path.as_str() == class.0 && class.1.contains(method.span);
    // Through a binding, TypeScript resolves the member on the binding's type,
    // which an annotation can widen to a superclass (`const d: Base = new
    // Derived()` names `Base.run` while `Derived.run` runs). Only a method
    // declared in the constructed class itself is the one that runs. Inside
    // that class's own method, `this` is typed by that class, so an inherited
    // resolution is exact too.
    if !own && !through_this {
        return None;
    }
    Some((method_file, method, own.then_some(class)))
}

/// `guaranteed`: the leaf scope's call runs `call` on every normal completion
/// (ADR 0275).
#[allow(clippy::too_many_arguments)]
fn helper_forbidden_operations(
    resolution: LeafScopeResolution<'_, '_>,
    call_file: &FileFacts,
    call: &solid_facts::ast::CallFact,
    kinds: &mut Reached,
    visited: &mut Visited,
    depth: usize,
    exact_this: ExactThis,
    guaranteed: bool,
) -> bool {
    let LeafScopeResolution { lookup, .. } = resolution;
    if depth == 0 {
        return false;
    }
    // ADR 0550: a call of an action a `const` holds runs the generator's first
    // step here, under this scope's owner. `flush` there throws
    // FLUSH_IN_ACTION first (SC2006's claim), so that kind is not this walk's.
    if !call.construct
        && let Some(generator) =
            crate::owners::named_action_generator(call_file, call.callee, lookup)
                .and_then(|span| call_file.ast.function_at(span))
    {
        let mut step = Vec::new();
        let complete = function_forbidden_operations(
            resolution,
            call_file,
            generator,
            &mut step,
            visited,
            depth,
            None,
            Entry::ActionStep { guaranteed },
        );
        for (kind, guaranteed, identity_unproven, sites) in step {
            if kind != crate::LeafOwnerOperationKind::Flush {
                record(kinds, kind, guaranteed, identity_unproven, sites);
            }
        }
        return complete;
    }
    // An immediate literal call selects its implementation without a symbol
    // or a transported binding. Use the ordinary helper walk so parameter
    // defaults, conditional completion and the async synchronous prefix keep
    // exactly the same proof requirements as named functions. Construction
    // does not enter an arrow/function through this call path.
    if !call.construct
        && let Some(helper) =
            callback_argument_literal(call_file, call_file.ast.peel_ts_sugar_span(call.callee))
    {
        // ADR 0516: stepping the iterator a generator called in place returns
        // (`(function* () { … })().next()`) runs the body there, which no call
        // edge follows: a call in the body leaves the obligation open.
        if crate::owners::generator_iterator_used(call_file, helper)
            .is_some_and(|created| created.span == call.span)
            && call_file
                .ast
                .calls
                .iter()
                .any(|inner| helper.body.contains(inner.span))
        {
            return false;
        }
        let values = crate::argument_values::parameter_values_for_literal_call(
            call_file, call, call_file, helper, lookup,
        );
        let entry = match values.as_deref() {
            Some(values) if !values.is_empty() => Entry::CalledWith { guaranteed, values },
            _ => Entry::Called { guaranteed },
        };
        return function_forbidden_operations(
            resolution, call_file, helper, kinds, visited, depth, None, entry,
        );
    }
    // This map is the source analyzer's exact, stability-checked transport
    // result, including the shared literal-receiver escape census. Reuse
    // identity, never a signature's possible target or a read-only summary.
    match resolution
        .tables
        .source_call_target(&location(call_file.path.shared(), call.callee))
    {
        Some(target @ crate::interproc::accessor_provenance::Callable::Function { path, span }) => {
            let Some((helper_file, helper)) = lookup
                .file_by_path(path)
                .and_then(|file| file.ast.function_at(*span).map(|function| (file, function)))
            else {
                return false;
            };
            let values = crate::argument_values::parameter_values_for_transported_call(
                call_file,
                call,
                helper_file,
                helper,
                target,
                lookup,
            );
            let entry = match values.as_deref() {
                Some(values) if !values.is_empty() => Entry::CalledWith { guaranteed, values },
                _ => Entry::Called { guaranteed },
            };
            return function_forbidden_operations(
                resolution,
                helper_file,
                helper,
                kinds,
                visited,
                depth,
                None,
                entry,
            );
        }
        Some(crate::interproc::accessor_provenance::Callable::Accessor(symbol))
            if resolution.tables.safe_call(symbol) =>
        {
            // ADR 0550: an action whose generator no `const` names runs a
            // step nothing here walks.
            return !resolution.tables.action(symbol);
        }
        _ => {}
    }
    if let Some((method_file, method, method_this)) =
        exact_instance_method(lookup, call_file, call, &exact_this)
    {
        return function_forbidden_operations(
            resolution,
            method_file,
            method,
            kinds,
            visited,
            depth,
            method_this,
            Entry::Called { guaranteed },
        );
    }
    let callee = call_file.ast.peel_ts_sugar_span(call.callee);
    if lookup.is_member_span(call_file, callee)
        || call_file
            .ast
            .computed_members
            .binary_search(&callee)
            .is_ok()
    {
        return member_call_operations(
            resolution, call_file, call, callee, kinds, visited, depth, guaranteed,
        );
    }
    // The independent binding fact already identifies this exact global.
    // A transparent wrapper need not also carry an identifier entity at its
    // peeled span in the demand table.
    if let Some(resolved) = lookup
        .resolved_callee_call(call_file, call.callee)
        .filter(|call| crate::runtime_semantics::augmented_global_runtime_call(call))
    {
        if dispatches_dom_event(resolved) {
            return false;
        }
        return standard_library_argument_operations(
            resolution, call_file, call, resolved, kinds, visited, depth, guaranteed,
        );
    }
    let Some(symbol) = lookup
        .entities()
        .at(call_file.path.as_str(), callee)
        .or_else(|| lookup.entities().at(call_file.path.as_str(), call.callee))
    else {
        return false;
    };
    if resolution.tables.setter(symbol) {
        // A setter runs a function argument -- the updater -- before it
        // returns (ADR 0210).
        return setter_updater_operations(
            resolution, call_file, call, kinds, visited, depth, guaranteed,
        );
    }
    if resolution.tables.safe_call(symbol) {
        // ADR 0550: as above, an unwalked action step is open.
        return !resolution.tables.action(symbol);
    }
    let Some((helper_file, helper)) = lookup
        .function_for_symbol(symbol)
        .filter(|(helper_file, helper)| lookup.function_value_is_current(helper_file, helper))
    else {
        let Some(resolved) =
            lookup
                .resolved_callee_call(call_file, call.callee)
                .filter(|resolved| {
                    resolved
                        .declaration
                        .as_ref()
                        .is_some_and(|declaration| declaration.standard_library)
                })
        else {
            return false;
        };
        if dispatches_dom_event(resolved) {
            return false;
        }
        return standard_library_argument_operations(
            resolution, call_file, call, resolved, kinds, visited, depth, guaranteed,
        );
    };
    // ADR 0280: the values this call's arguments prove decide the helper's
    // own parameter tests.
    let values =
        crate::argument_values::parameter_values(call_file, call, helper_file, helper, lookup);
    let entry = match values.as_deref() {
        Some(values) if !values.is_empty() => Entry::CalledWith { guaranteed, values },
        _ => Entry::Called { guaranteed },
    };
    function_forbidden_operations(
        resolution,
        helper_file,
        helper,
        kinds,
        visited,
        depth,
        None,
        entry,
    )
}

/// A member call that is not an exact instance's method (ADR 0210).
///
/// The compiler's entity at a member callee's complete span names the
/// receiver's root binding, not the member: `items().forEach` answers `items`
/// and `register.bind` answers `register`. Read as the callee, that made an
/// accessor's array method a safe accessor call and `register.bind(null)` a
/// call of `register`. An exact namespace export is followed by its property
/// symbol. Otherwise a standard-library member's arguments are followed by
/// [`standard_library_argument_operations`], and the receiver itself when the
/// member is `call` or `apply`, which run it before they return.
#[allow(clippy::too_many_arguments)]
fn member_call_operations(
    resolution: LeafScopeResolution<'_, '_>,
    file: &FileFacts,
    call: &solid_facts::ast::CallFact,
    callee: Span,
    kinds: &mut Reached,
    visited: &mut Visited,
    depth: usize,
    guaranteed: bool,
) -> bool {
    if !call.construct
        && let Some((helper_file, helper)) =
            resolution.lookup.namespace_member_function(file, callee)
    {
        return function_forbidden_operations(
            resolution,
            helper_file,
            helper,
            kinds,
            visited,
            depth,
            None,
            Entry::Called { guaranteed },
        );
    }
    let Some((resolved, declaration)) = resolution
        .lookup
        .resolved_callee_call(file, call.callee)
        .and_then(|resolved| Some((resolved, resolved.declaration.as_ref()?)))
        .filter(|(_, declaration)| declaration.standard_library)
    else {
        return false;
    };
    // ADR 0439: an optional call (`el?.scrollIntoView()`) is read like the
    // non-optional form -- the same exact `lib.dom` declaration, the same
    // premises. ADR 0329: a member that dispatches an event synchronously
    // (`el.focus()`, `el.click()`) re-enters registered listeners in either
    // form, and the one member whose optional form was refused for its own
    // synchronous event (`getContext`) keeps that refusal.
    let dom = std::path::Path::new(declaration.source_file.as_ref())
        .file_name()
        .is_some_and(|name| name == "lib.dom.d.ts" || name == "lib.dom.iterable.d.ts");
    if dom
        && (synchronously_dispatching_dom_member(declaration.qualified_name.as_ref())
            || (refused_when_optional(declaration.qualified_name.as_ref())
                && (file
                    .ast
                    .optional_members
                    .iter()
                    .any(|member| callee.contains(*member))
                    || file
                        .source_text(Span::new(call.callee.end, call.span.end))
                        .is_some_and(|suffix| suffix.trim_start().starts_with("?.")))))
    {
        return false;
    }
    let mut complete = standard_library_argument_operations(
        resolution, file, call, resolved, kinds, visited, depth, guaranteed,
    );
    let invokes_receiver = matches!(
        declaration.qualified_name.as_ref(),
        "Function.call"
            | "Function.apply"
            | "CallableFunction.call"
            | "CallableFunction.apply"
            | "NewableFunction.call"
            | "NewableFunction.apply"
    );
    if invokes_receiver {
        complete &= file.ast.member_at(callee).is_some_and(|member| {
            argument_body_operations(
                resolution,
                file,
                member.object,
                kinds,
                visited,
                depth,
                guaranteed,
            )
        });
    }
    complete
}

/// Whether `resolved` is a `lib.dom` global (`dispatchEvent(…)`,
/// `focus()`) that dispatches an event synchronously (ADR 0329).
fn dispatches_dom_event(resolved: &typefacts::ResolvedCall) -> bool {
    resolved.declaration.as_ref().is_some_and(|declaration| {
        std::path::Path::new(declaration.source_file.as_ref())
            .file_name()
            .is_some_and(|name| name == "lib.dom.d.ts" || name == "lib.dom.iterable.d.ts")
            && synchronously_dispatching_dom_member(declaration.qualified_name.as_ref())
    })
}

/// ADR 0329: `lib.dom` members that dispatch an event synchronously, before
/// they return, and so run listeners the program registered: focus moves
/// (`focus`, `blur`), synthetic activation and dispatch (`click`,
/// `dispatchEvent`), and constraint validation (`invalid`) by member name on
/// any interface (every one that declares them dispatches); and, by exact
/// owner, the members whose name is shared with non-dispatching ones: form
/// `reset`/`requestSubmit`, popover toggles (`beforetoggle`), dialog
/// `show`/`showModal`/`close`/`requestClose`, `AbortController.abort`
/// (`abort` on its signal), `XMLHttpRequest.open`/`abort`/`send`
/// (`readystatechange`, `abort`, `loadstart`), `FileReader.abort` (`abort`,
/// `loadend`), `Document.execCommand` (`input`) and, measured against ADR 0329's
/// "tree mutation stays trusted", what moves focus without naming it (ADR 0642):
/// `remove`/`removeChild` of a focused element (`blur`) and `select` (`focus`).
/// `lib.dom` also declares `dispatchEvent`, `focus` and `blur` as globals for
/// `window`.
fn synchronously_dispatching_dom_member(qualified_name: &str) -> bool {
    let (owner, member) = qualified_name
        .rsplit_once('.')
        .unwrap_or(("", qualified_name));
    matches!(
        member,
        "focus" | "blur" | "click" | "dispatchEvent" | "checkValidity" | "reportValidity"
    ) || matches!(
        (owner, member),
        // ADR 0705: `submit` delivers `formdata`; `replaceWith`,
        // `replaceChild` and `replaceChildren` blur a focused element they
        // remove; `reportError` dispatches `error` at the global; a
        // navigation fires `beforeunload`. Measured in Chrome where the page
        // survives it (not `History`/`Location`); a state requirement
        // (focus, a navigation) is unknown, so each is taken to deliver.
        ("HTMLFormElement", "reset" | "requestSubmit" | "submit")
            | ("ChildNode", "remove" | "replaceWith")
            | ("Node", "removeChild" | "replaceChild")
            | ("ParentNode", "replaceChildren")
            | ("" | "WindowOrWorkerGlobalScope", "reportError")
            | ("History", "go")
            | ("Location", "assign" | "reload" | "replace")
            | ("HTMLInputElement" | "HTMLTextAreaElement", "select")
            | (
                "HTMLElement",
                "showPopover" | "hidePopover" | "togglePopover"
            )
            | (
                "HTMLDialogElement",
                "show" | "showModal" | "close" | "requestClose"
            )
            | ("AbortController", "abort")
            | ("XMLHttpRequest", "open" | "abort" | "send")
            | ("FileReader", "abort")
            | ("Document", "execCommand")
            | ("ElementInternals", "checkValidity" | "reportValidity")
            | ("HTMLSelectElement", "remove")
    )
}

/// ADR 0439: `lib.dom` members whose optional form stays refused although the
/// non-optional form is trusted: `getContext` creates a WebGL context, which
/// fires `webglcontextcreationerror` at the canvas synchronously on failure
/// (ADR 0318). The non-optional form keeps the trust it has always had.
fn refused_when_optional(qualified_name: &str) -> bool {
    matches!(
        qualified_name,
        "HTMLCanvasElement.getContext" | "OffscreenCanvas.getContext"
    )
}

/// Whether the function arguments a standard-library call may run in the
/// leaf scope are proven free of forbidden operations (ADR 0210).
///
/// The host runs no Solid code of its own, but it runs the functions it is
/// handed, and the audited timing table
/// ([`crate::runtime_semantics::argument_behavior`]) says when:
///
/// - an inline callback (`list.forEach(register)`) runs before the call
///   returns, so its body is walked like a helper's and a forbidden operation
///   there is a violation of this call;
/// - a fresh-stack callback (`setTimeout`, `queueMicrotask`) runs from a host
///   queue, after the leaf scope is gone;
/// - a deferred callback (`addEventListener`'s listener, `bind`'s bound
///   arguments) runs after the call returns. A synchronous dispatch in the
///   leaf scope (`el.click()`, `el.focus()`) would run a listener there, but
///   dispatch is not modeled: the same `focus()` runs listeners registered
///   anywhere, and the walk has always read it as a host call that runs only
///   what it is handed;
/// - a `PromiseLike.then` call leaves the obligation open: the thenable is
///   any object with a `then`, and that `then` is user code, whatever it is
///   handed;
/// - a value the host only reads or keeps is not run;
/// - any other argument the host may call -- a callable parameter with no
///   audited timing (`new Promise(executor)`) -- leaves the obligation open.
///
/// A parameter typed `any` or `unknown` (`console.log(...data)`) is read as a
/// value. Implicit invocation through getters, `toString`, `valueOf`, an
/// iterator or a thenable is not modeled, as it is not for the rest of the
/// standard-library trust here.
///
/// An inline callback inherits `guaranteed` from the call that hands it over,
/// as ADR 0210 reads it, where the carrier provably runs it (ADR 0563); ADR
/// 0275 changes only what a body's own control flow decides.
#[allow(clippy::too_many_arguments)]
fn standard_library_argument_operations(
    resolution: LeafScopeResolution<'_, '_>,
    file: &FileFacts,
    call: &solid_facts::ast::CallFact,
    resolved: &typefacts::ResolvedCall,
    kinds: &mut Reached,
    visited: &mut Visited,
    depth: usize,
    guaranteed: bool,
) -> bool {
    use crate::runtime_semantics::RuntimeArgumentBehavior;
    use typefacts::Callability;
    let lookup = resolution.lookup;
    // A `PromiseLike` may be any object with a `then`: its implementation is
    // not the host's and may call back before it returns.
    let thenable = resolved
        .declaration
        .as_ref()
        .is_some_and(|declaration| declaration.qualified_name.as_ref() == "PromiseLike.then");
    // The thenable's own `then` is user code, whatever it is handed.
    if thenable {
        return false;
    }
    let mut complete = true;
    for (index, argument) in call.arguments.iter().enumerate() {
        // An object literal's accessors run when the host reads them, whether
        // it is written here or bound to a `const` and named here.
        if argument.value == solid_facts::ast::ArgumentValueKind::Identifier
            && resolution
                .lookup
                .binding_at_reference(
                    file.path.as_str(),
                    file.ast.peel_ts_sugar_span(argument.span),
                )
                .is_some_and(|(binding_file, binding, _)| {
                    binding.initializer_value_kind == solid_facts::ast::RuntimeValueKind::Object
                        && binding.initializer.is_some_and(|initializer| {
                            object_literal_has_accessor(binding_file, initializer)
                        })
                })
        {
            complete = false;
            continue;
        }
        if crate::runtime_semantics::literal_argument_is_not_callable(argument.runtime_value_kind) {
            if argument.runtime_value_kind == solid_facts::ast::RuntimeValueKind::Object
                && object_literal_has_accessor(file, argument.span)
            {
                complete = false;
            }
            continue;
        }
        let callability = lookup
            .entity_at(file.path.as_str(), argument.span)
            .and_then(|entity| entity.callability);
        if callability == Some(Callability::NonCallable) {
            continue;
        }
        if argument.spread {
            // A spread hands over values at indices the mapping does not name.
            complete = false;
            continue;
        }
        // Every default-library listener slot, whichever DOM interface
        // redeclares `addEventListener`, is a deferred callback.
        let listener_slot =
            crate::runtime_semantics::runs_on_invoker_stack(resolved, callability, index);
        match crate::runtime_semantics::argument_behavior(resolved, callability, index) {
            // ADR 0563: a carrier that may run its callback zero times (an
            // empty or unknown-length receiver) makes what it reaches
            // possible, never guaranteed.
            Some(RuntimeArgumentBehavior::InlineCallback) => {
                complete &= argument_body_operations(
                    resolution,
                    file,
                    argument.span,
                    kinds,
                    visited,
                    depth,
                    guaranteed
                        && crate::carrier_runs::carrier_provably_runs(file, call.span, lookup),
                );
            }
            Some(
                RuntimeArgumentBehavior::DeferredCallback
                | RuntimeArgumentBehavior::FreshStackCallback
                | RuntimeArgumentBehavior::RetainedValue
                | RuntimeArgumentBehavior::ValueOnly,
            ) => {}
            None if listener_slot => {}
            None => {
                let parameter_callable = crate::runtime_semantics::resolved_parameter(
                    resolved, index,
                )
                .is_none_or(|parameter| {
                    !matches!(
                        parameter.callability,
                        Callability::NonCallable | Callability::Unknown
                    )
                });
                complete &= !parameter_callable;
            }
        }
    }
    complete
}

/// Whether the object literal at `span`, or one nested in it, defines a
/// property that is not plain data: a getter, a setter or a method.
fn object_literal_has_accessor(file: &FileFacts, span: Span) -> bool {
    file.ast
        .object_properties
        .iter()
        .any(|property| span.contains(property.span) && !property.data)
}

/// A setter's function argument is its updater, which the setter runs before
/// it returns (ADR 0210). Any other argument is the new value.
fn setter_updater_operations(
    resolution: LeafScopeResolution<'_, '_>,
    file: &FileFacts,
    call: &solid_facts::ast::CallFact,
    kinds: &mut Reached,
    visited: &mut Visited,
    depth: usize,
    guaranteed: bool,
) -> bool {
    let mut complete = true;
    for argument in &call.arguments {
        // An object literal's accessors run when the host reads them, whether
        // it is written here or bound to a `const` and named here.
        if argument.value == solid_facts::ast::ArgumentValueKind::Identifier
            && resolution
                .lookup
                .binding_at_reference(
                    file.path.as_str(),
                    file.ast.peel_ts_sugar_span(argument.span),
                )
                .is_some_and(|(binding_file, binding, _)| {
                    binding.initializer_value_kind == solid_facts::ast::RuntimeValueKind::Object
                        && binding.initializer.is_some_and(|initializer| {
                            object_literal_has_accessor(binding_file, initializer)
                        })
                })
        {
            complete = false;
            continue;
        }
        if crate::runtime_semantics::literal_argument_is_not_callable(argument.runtime_value_kind) {
            if argument.runtime_value_kind == solid_facts::ast::RuntimeValueKind::Object
                && object_literal_has_accessor(file, argument.span)
            {
                complete = false;
            }
            continue;
        }
        let callability = resolution
            .lookup
            .entity_at(file.path.as_str(), argument.span)
            .and_then(|entity| entity.callability);
        if callability == Some(typefacts::Callability::NonCallable) {
            continue;
        }
        complete &= argument_body_operations(
            resolution,
            file,
            argument.span,
            kinds,
            visited,
            depth,
            guaranteed,
        );
    }
    complete
}

/// The forbidden operations of the function an argument evaluates to, when
/// that function is exactly known: a function literal written as the
/// argument, or an identifier bound to a project function. Anything else --
/// a call's result, a member, a parameter -- is not followed.
fn argument_body_operations(
    resolution: LeafScopeResolution<'_, '_>,
    file: &FileFacts,
    argument: Span,
    kinds: &mut Reached,
    visited: &mut Visited,
    depth: usize,
    guaranteed: bool,
) -> bool {
    if let Some(literal) = callback_argument_literal(file, argument) {
        return function_forbidden_operations(
            resolution,
            file,
            literal,
            kinds,
            visited,
            depth - 1,
            None,
            Entry::Called { guaranteed },
        );
    }
    let lookup = resolution.lookup;
    let peeled = file.ast.peel_ts_sugar_span(argument);
    // ADR 0538: a position the demand table does not carry (a template's tag)
    // is read through its declaration, the same exact symbol.
    let symbol = lookup
        .entities()
        .at(file.path.as_str(), peeled)
        .or_else(|| {
            file.ast
                .reference_declaration(peeled)
                .and_then(|declaration| lookup.entities().at(file.path.as_str(), declaration))
        });
    let Some((function_file, function)) = symbol
        .and_then(|symbol| lookup.function_for_symbol(symbol))
        .filter(|(function_file, function)| {
            lookup.function_value_is_current(function_file, function)
        })
    else {
        return false;
    };
    function_forbidden_operations(
        resolution,
        function_file,
        function,
        kinds,
        visited,
        depth - 1,
        None,
        Entry::Called { guaranteed },
    )
}

#[allow(clippy::too_many_arguments)]
fn function_forbidden_operations(
    resolution: LeafScopeResolution<'_, '_>,
    helper_file: &FileFacts,
    helper: &solid_facts::ast::FunctionFact,
    kinds: &mut Reached,
    visited: &mut Visited,
    depth: usize,
    exact_this: ExactThis,
    entry: Entry<'_>,
) -> bool {
    let LeafScopeResolution {
        lookup,
        symbol_names,
        ..
    } = resolution;
    if depth == 0 {
        return false;
    }
    let key = (
        helper_file.path.as_str().to_owned(),
        helper.span,
        entry_key(entry, &exact_this),
    );
    if visited.contains(&key) {
        return true;
    }
    visited.push(key);
    let dialect = lookup.dialect;
    let entities = lookup.entities();
    // The parameter list runs on entry: its defaults, top-level or nested in
    // a destructuring pattern, and its computed keys. Whether a default runs
    // depends on the argument, which is not decided here, so an operation
    // there, or a call there the walk cannot follow, leaves the obligation
    // open.
    let parameter_spans = helper
        .parameters
        .iter()
        .flat_map(|parameter| std::iter::once(parameter.pattern).chain(parameter.initializer))
        .collect::<Vec<_>>();
    let mut parameter_calls = helper_file
        .ast
        .calls
        .iter()
        .filter(|inner| {
            parameter_spans.iter().any(|span| span.contains(inner.span))
                && !helper_file.ast.functions.iter().any(|nested| {
                    nested.span != helper.span
                        && helper.span.contains(nested.span)
                        && nested.span.contains(inner.span)
                })
        })
        .peekable();
    // Calling a generator runs none of its body, only its parameter list;
    // stepping an action runs its first step too (ADR 0550).
    let action_step = matches!(entry, Entry::ActionStep { .. });
    if helper.generator && !action_step {
        return parameter_calls.peek().is_none();
    }
    let mut complete = true;
    for inner in parameter_calls {
        let mut possible = Vec::new();
        let mut scratch = visited.clone();
        complete &= crate::call_primitive_name_cached(
            lookup,
            helper_file,
            inner,
            entities,
            symbol_names,
            dialect,
        )
        .is_none()
            && helper_forbidden_operations(
                resolution,
                helper_file,
                inner,
                &mut possible,
                &mut scratch,
                depth - 1,
                exact_this.clone(),
                false,
            )
            && possible.is_empty();
    }
    // ADR 0280: each direct call's standing on this call, when its arguments
    // prove anything about the helper's parameters.
    let standings = match entry {
        Entry::CalledWith { values, .. } => {
            let sites = helper_file
                .ast
                .calls_within(helper.body)
                .filter(|inner| {
                    crate::owners::written_directly_in(&helper_file.ast, helper, inner.span)
                })
                .map(|inner| inner.span)
                .collect::<Vec<_>>();
            solid_facts::ast::argument_site_standings(
                std::path::Path::new(helper_file.path.as_str()),
                &helper_file.source,
                helper.span,
                values,
                &lookup.never_returning_calls_within(helper_file, helper),
                &sites,
            )
            .map(|standings| sites.into_iter().zip(standings).collect::<HashMap<_, _>>())
        }
        _ => None,
    };
    // ADR 0883: JSX written in the helper's body creates on this stack too. Past
    // an await every run reaches first it has left the leaf (`jsx-effect`'s).
    if helper_file
        .ast
        .jsx_elements
        .iter()
        .map(|element| &element.span)
        .chain(&helper_file.ast.jsx_fragments)
        .any(|jsx| helper.body.contains(*jsx))
    {
        for site in crate::owners::jsx_sites(helper_file, lookup) {
            if site.kind == crate::owners::JsxSiteKind::MaybeRenderEffect
                || site.in_getter
                || !helper.body.contains(site.span)
                || !crate::owners::written_directly_in(&helper_file.ast, helper, site.span)
                || helper_file
                    .ast
                    .unconditional_awaits
                    .iter()
                    .any(|await_span| await_span.end <= site.span.start)
            {
                continue;
            }
            let guaranteed =
                reached_on_every_call(resolution, helper_file, helper, site.span, entry);
            record(
                kinds,
                crate::LeafOwnerOperationKind::Primitive("JSX".into()),
                guaranteed,
                false,
                vec![location(helper_file.path.shared(), site.span)],
            );
        }
    }
    for inner in helper_file.ast.calls_within(helper.body) {
        // A call inside a nested function -- its body or its parameter list
        // (ADR 0204) -- is not executed by calling the helper; it belongs to
        // whatever later invokes that function.
        if !crate::owners::written_directly_in(&helper_file.ast, helper, inner.span) {
            continue;
        }
        // An instance field initializer of a class written here runs when an
        // instance is constructed, which this walk does not place (review
        // round 2): the scope is not proven free, nor a proven forbidden call.
        if crate::execution_role::inside_instance_field(helper_file, inner.span) {
            return false;
        }
        // After an `await` that every run of the helper reaches first, the
        // call executes from a promise continuation: the leaf scope that
        // invoked the helper is long gone and no owner exists at all. That is
        // a different claim (an ownerless operation, `missing-owner`'s), not a
        // forbidden call in the leaf scope.
        if helper_file
            .ast
            .unconditional_awaits
            .iter()
            .any(|await_span| {
                await_span.end <= inner.span.start
                    && containing_ast_function(&helper_file.ast, *await_span)
                        .is_some_and(|function| function.span == helper.span)
            })
        {
            continue;
        }
        // ADR 0550: a site a yielded promise's continuation runs has left the
        // scope (no owner: SC4001's claim); one that may follow a thenable
        // `yield` may run there or not.
        let resumption = action_step
            .then(|| crate::owners::action_step_resumption(helper_file, inner.span, lookup))
            .flatten();
        if resumption == Some(crate::owners::StepResumption::Proven) {
            continue;
        }
        let guaranteed = resumption.is_none()
            && match standings
                .as_ref()
                .and_then(|standings| standings.get(&inner.span))
            {
                // Not on this call: nothing to report, nothing left open.
                Some(solid_facts::ast::SiteStanding::Unreachable) => continue,
                Some(solid_facts::ast::SiteStanding::Guaranteed) => {
                    // The walk read this body's `never`/`asserts` calls as exits,
                    // and skipped the ones in arms the arguments rule out.
                    matches!(
                        entry,
                        Entry::CalledWith {
                            guaranteed: true,
                            ..
                        }
                    )
                }
                Some(solid_facts::ast::SiteStanding::Possible) => false,
                None => reached_on_every_call(resolution, helper_file, helper, inner.span, entry),
            };
        let primitive = crate::call_primitive_name_cached(
            lookup,
            helper_file,
            inner,
            entities,
            symbol_names,
            dialect,
        );
        let Some(primitive) = primitive else {
            complete &= helper_forbidden_operations(
                resolution,
                helper_file,
                inner,
                kinds,
                visited,
                depth - 1,
                exact_this.clone(),
                guaranteed,
            );
            continue;
        };
        // ADR 0944: the callbacks a primitive runs in place are part of the helper's synchronous extent.
        complete &= slot_callback_operations(
            resolution,
            helper_file,
            inner,
            &primitive,
            kinds,
            visited,
            depth,
            guaranteed,
            false,
        );
        // One kind per call site, however many helper calls or transitive
        // hops reach it: the finding names the operation, not each way of
        // arriving at it. Kinds arrive interleaved across hops, so an
        // adjacent-only `dedup` would leave byte-identical operations in the
        // serialized IR — reject at the push instead.
        if let Some(kind) = forbidden_operation_kind(dialect, helper_file, inner, &primitive) {
            // ADR 0711: an attribution the binder does not confirm is
            // possible only.
            let proven = primitive_identity_proven(dialect, helper_file, inner, &primitive);
            record(
                kinds,
                kind,
                guaranteed && proven,
                !proven,
                vec![location(helper_file.path.shared(), inner.span)],
            );
        }
    }
    // ADR 0537: a tagged template written in the body calls its tag there.
    // It is in no completion census, so what its tag does is possible only.
    for template in &helper_file.ast.tagged_templates {
        if !helper.body.contains(template.span)
            || !crate::owners::written_directly_in(&helper_file.ast, helper, template.span)
        {
            continue;
        }
        if crate::execution_role::inside_instance_field(helper_file, template.span) {
            return false;
        }
        if helper_file
            .ast
            .unconditional_awaits
            .iter()
            .any(|await_span| {
                await_span.end <= template.span.start
                    && containing_ast_function(&helper_file.ast, *await_span)
                        .is_some_and(|function| function.span == helper.span)
            })
        {
            continue;
        }
        complete &= tag_forbidden_operations(
            resolution,
            helper_file,
            template,
            kinds,
            visited,
            depth - 1,
            matches!(entry, Entry::LeafBody),
        );
    }
    complete
}

/// ADR 0537: what the tag of a tagged template runs where the template is
/// written. The tag is called there with the strings and substitutions, as a
/// call of it is: an exact, current project function (or a literal tag) runs
/// its synchronous extent in the scope. Any other tag, a member's or a
/// package's, is not followed and leaves the scope unproven.
fn tag_forbidden_operations(
    resolution: LeafScopeResolution<'_, '_>,
    file: &FileFacts,
    template: &solid_facts::ast::TaggedTemplateFact,
    kinds: &mut Reached,
    visited: &mut Visited,
    depth: usize,
    guaranteed: bool,
) -> bool {
    let lookup = resolution.lookup;
    if depth == 0 {
        return false;
    }
    let tag = file.ast.peel_ts_sugar_span(template.tag);
    let helper = callback_argument_literal(file, tag)
        .map(|helper| (file, helper))
        .or_else(|| {
            // The tag's binding, through the exact reference resolution: a
            // tag position carries no value entity of its own.
            let symbol = lookup
                .entities()
                .value_symbol(file, template.tag)
                .cloned()
                .or_else(|| {
                    lookup
                        .binding_at_reference(file.path.as_str(), tag)
                        .map(|(_, _, symbol)| symbol)
                })?;
            if resolution.tables.safe_call(&symbol) {
                return None;
            }
            lookup
                .function_for_symbol(&symbol)
                .filter(|(helper_file, helper)| {
                    lookup.function_value_is_current(helper_file, helper)
                })
        });
    let Some((helper_file, helper)) = helper else {
        return false;
    };
    function_forbidden_operations(
        resolution,
        helper_file,
        helper,
        kinds,
        visited,
        depth,
        None,
        Entry::Called { guaranteed },
    )
}

/// What one `return` in a cleanup-accepting callback proves about the value
/// the owner receives.
///
/// Only `ValidFunction` is load-bearing now: it is the ownership fact
/// `function_returns_cleanup` (SC4001) asks for — "does this hand the
/// owner an actual cleanup function". The other three outcomes are the ways
/// that proof can fail, and they are kept apart because they fail for
/// materially different reasons; collapsing them would hide that
/// `return nothing` where `nothing: undefined` is *legal* and merely hands the
/// owner nothing, which is not the same as a value the owner cannot use.
///
/// None is a cleanup-legality finding. Legality is TypeScript's:
/// `EffectFunction` returns `(() => void) | void`, so an unusable value is a
/// type error and reporting it again would duplicate `tsc`. The owner pass may
/// still turn `Unresolved` into an uncertifiable SC4001 when a possibly
/// returned cleanup would need an owner.
enum CleanupReturnStatus {
    /// Proven to be a function: an owner that reads returned cleanups
    /// registers it.
    ValidFunction,
    /// Proven legal and unable to contain a function — `undefined` or `void`.
    ValidNonFunction,
    /// Proven legal and may be either a cleanup function or `undefined`.
    /// Legality is settled, but owner registration is conditional.
    OptionalFunction,
    /// Proven to be a value an owner cannot use as cleanup — which is exactly
    /// the domain `tsc` rejects, so it only means "no cleanup here".
    Invalid,
    /// Neither proven; no cleanup may be assumed.
    Unresolved,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CleanupReturnProof {
    Function,
    OptionalFunction,
    NoFunction,
    Unresolved,
}

fn terminal_cleanup_fix(
    file: &solid_facts::FileFacts,
    owner_region: Span,
    call: &solid_facts::ast::CallFact,
) -> Option<Fix> {
    let callback = file
        .ast
        .functions
        .iter()
        .filter(|function| owner_region.contains(function.span))
        .max_by_key(|function| function.span.end - function.span.start)?;
    let body_end = usize::try_from(callback.body.end).ok()?.checked_sub(1)?;
    let call_end = usize::try_from(call.span.end).ok()?;
    if call_end > body_end || body_end > file.source.len() {
        return None;
    }
    if !file.source.as_bytes()[call_end..body_end]
        .iter()
        .all(|byte| byte.is_ascii_whitespace() || *byte == b';')
    {
        return None;
    }
    let [argument] = call.arguments.as_slice() else {
        return None;
    };
    let start = usize::try_from(argument.span.start).ok()?;
    let end = usize::try_from(argument.span.end).ok()?;
    let argument = file.source.get(start..end)?.trim();
    if argument.is_empty() {
        return None;
    }
    Some(Fix {
        message: "Return the cleanup function instead of calling onCleanup".into(),
        applicability: "safe".into(),
        edits: vec![TextEdit {
            location: location(file.path.shared(), call.span),
            new_text: format!("return {argument}"),
        }],
    })
}

fn cleanup_return_status(
    lookup: &SemanticLookup<'_>,
    file: &solid_facts::FileFacts,
    returned: &solid_facts::ast::ReturnFact,
) -> CleanupReturnStatus {
    let entities = lookup.entities();
    match returned.value {
        solid_facts::ast::ReturnValueKind::Undefined => CleanupReturnStatus::ValidNonFunction,
        solid_facts::ast::ReturnValueKind::Function => CleanupReturnStatus::ValidFunction,
        solid_facts::ast::ReturnValueKind::Member => {
            // Computed dispatch is not an exact property proof: the key may
            // select any member at runtime. Static member expressions have a
            // complete-expression value-domain fact at this exact return
            // span, so classify those from the same evidence as identifiers.
            if file
                .ast
                .computed_members
                .binary_search(&returned.span)
                .is_ok()
            {
                CleanupReturnStatus::Unresolved
            } else {
                domain_cleanup_return_status(
                    lookup
                        .entity_at(file.path.as_str(), returned.span)
                        .and_then(|entity| entity.runtime_value_domain.as_ref()),
                )
            }
        }
        solid_facts::ast::ReturnValueKind::Other => domain_cleanup_return_status(
            lookup
                .entity_at(file.path.as_str(), returned.span)
                .and_then(|entity| entity.runtime_value_domain.as_ref()),
        ),
        solid_facts::ast::ReturnValueKind::Call => {
            let Some(callee) = returned.callee else {
                return CleanupReturnStatus::Unresolved;
            };
            let resolved = lookup
                .resolved_callee_call(file, callee)
                .is_some_and(|call| call.validity == ResolvedCallValidity::Valid);
            if !resolved {
                return CleanupReturnStatus::Unresolved;
            }
            // The *result* of the call, never its callee: `callResultDomain` is
            // matched by the producer against a call-like node occupying exactly
            // the demanded span, so `makeCount()` where `makeCount(): number`
            // classifies as the number it produces rather than the callable
            // `makeCount`. An absent field (no exact call-like node) and an
            // `unknown` domain (a checker error or recovery type) both stay
            // fail-closed in `domain_cleanup_return_status`.
            domain_cleanup_return_status(returned_call_domain(lookup, file, callee))
        }
        solid_facts::ast::ReturnValueKind::Identifier => {
            let Some(symbol) = entities.exact(&location(file.path.shared(), returned.span)) else {
                return CleanupReturnStatus::Unresolved;
            };
            if identifier_return_is_exact_function(lookup, file, symbol) {
                CleanupReturnStatus::ValidFunction
            } else {
                domain_cleanup_return_status(
                    lookup
                        .entity_at(file.path.as_str(), returned.span)
                        .and_then(|entity| entity.runtime_value_domain.as_ref()),
                )
            }
        }
    }
}

fn identifier_return_is_exact_function(
    lookup: &SemanticLookup<'_>,
    file: &solid_facts::FileFacts,
    symbol: &SymbolId,
) -> bool {
    let entities = lookup.entities();
    file.ast.functions.iter().any(|function| {
        function.name.as_ref().is_some_and(|name| {
            entities.get(&location(file.path.shared(), name.span)) == Some(symbol)
        })
    }) || file.ast.bindings.iter().any(|binding| {
        binding.initializer_function
            && binding
                .names
                .iter()
                .any(|name| entities.get(&location(file.path.shared(), name.span)) == Some(symbol))
    })
}

/// Classifies a cleanup return from the compiler's runtime value domain.
///
/// The producer derives this from checker types, flags, constraints,
/// assignability, union constituents, and call signatures — never from
/// rendered type text — so aliases, `any`, `unknown`, and recovery types
/// arrive as `unknown` and stay fail-closed here instead of being guessed from
/// spelling. A missing fact (the demand was not planned, or the compiler had
/// no answer) is likewise unresolved.
fn domain_cleanup_return_status(domain: Option<&RuntimeValueDomain>) -> CleanupReturnStatus {
    let Some(domain) = domain.filter(|domain| !domain.unknown()) else {
        return CleanupReturnStatus::Unresolved;
    };
    match (
        domain.may_be_callable(),
        domain.may_be_other(),
        domain.may_be_undefined(),
    ) {
        // Only ever a function, or only ever a function or `undefined`: legal
        // either way, but the optional form keeps owner registration open.
        (true, false, false) => CleanupReturnStatus::ValidFunction,
        (true, false, true) => CleanupReturnStatus::OptionalFunction,
        // Never a function and never voidish: the owner is handed a value it
        // cannot use, on every execution that reaches this return.
        (false, true, false) => CleanupReturnStatus::Invalid,
        // Only `undefined`/`void`.
        (false, false, true) => CleanupReturnStatus::ValidNonFunction,
        // A domain that admits both a legal and an illegal value proves
        // neither, and `never` (a known empty domain) describes a value this
        // return never produces.
        (true, true, _) | (false, true, true) => CleanupReturnStatus::Invalid,
        (false, false, false) => CleanupReturnStatus::ValidNonFunction,
    }
}

pub(super) fn function_cleanup_return_proof(
    lookup: &SemanticLookup<'_>,
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
) -> CleanupReturnProof {
    let status = |returned: &solid_facts::ast::ReturnFact| {
        match cleanup_return_status(lookup, file, returned) {
            CleanupReturnStatus::ValidFunction => match returned.value {
                solid_facts::ast::ReturnValueKind::Function => CleanupReturnProof::Function,
                solid_facts::ast::ReturnValueKind::Identifier => lookup
                    .entities()
                    .exact(&location(file.path.shared(), returned.span))
                    .filter(|symbol| identifier_return_is_exact_function(lookup, file, symbol))
                    .map_or(CleanupReturnProof::OptionalFunction, |_| {
                        CleanupReturnProof::Function
                    }),
                // A callable checker type alone does not prove the runtime
                // value is non-nullish when strictNullChecks is disabled.
                solid_facts::ast::ReturnValueKind::Call
                | solid_facts::ast::ReturnValueKind::Member
                | solid_facts::ast::ReturnValueKind::Other
                | solid_facts::ast::ReturnValueKind::Undefined => {
                    CleanupReturnProof::OptionalFunction
                }
            },
            CleanupReturnStatus::ValidNonFunction | CleanupReturnStatus::Invalid => {
                CleanupReturnProof::NoFunction
            }
            CleanupReturnStatus::OptionalFunction => CleanupReturnProof::OptionalFunction,
            CleanupReturnStatus::Unresolved => CleanupReturnProof::Unresolved,
        }
    };
    if let Some(returned) = function.expression_return.as_ref() {
        return status(returned);
    }
    let returned = file
        .ast
        .returns
        .iter()
        .filter(|returned| {
            containing_ast_function(&file.ast, returned.span)
                .is_some_and(|owner| owner.span == function.span)
        })
        .collect::<Vec<_>>();
    if returned.is_empty() {
        return CleanupReturnProof::NoFunction;
    }
    if let [only] = returned.as_slice()
        && !only.conditional
    {
        return status(only);
    }
    let statuses = returned.into_iter().map(status).collect::<Vec<_>>();
    if statuses.contains(&CleanupReturnProof::Unresolved) {
        return CleanupReturnProof::Unresolved;
    }
    if statuses
        .iter()
        .all(|status| *status == CleanupReturnProof::NoFunction)
    {
        CleanupReturnProof::NoFunction
    } else {
        // Multiple/conditional exits do not prove that a cleanup is returned
        // on every execution, even when one arm visibly returns a function.
        CleanupReturnProof::OptionalFunction
    }
}

/// The runtime value domain of what a returned call *produces*.
///
/// Resolved at the call's own span, where the producer answers only for a
/// call-like node matching that span exactly. A callee-shaped fact can
/// therefore never be substituted, which is what made the older callability
/// probe classify `makeCount()` from `makeCount`.
fn returned_call_domain<'a>(
    lookup: &'a SemanticLookup<'_>,
    file: &solid_facts::FileFacts,
    callee: Span,
) -> Option<&'a RuntimeValueDomain> {
    let call = lookup.call_by_callee(file, callee)?;
    lookup
        .entity_at(file.path.as_str(), call.span)
        .and_then(|entity| entity.call_result_domain.as_ref())
}

#[cfg(test)]
mod dom_dispatch_tests {
    use super::{refused_when_optional, synchronously_dispatching_dom_member};

    /// ADR 0329 and ADR 0439: the members refused in both forms, and the
    /// ones an optional call reads like its non-optional form.
    #[test]
    fn only_dispatching_dom_members_are_refused() {
        for dispatching in [
            "HTMLOrSVGElement.focus",
            "HTMLOrSVGElement.blur",
            "HTMLElement.click",
            "EventTarget.dispatchEvent",
            "HTMLFormElement.requestSubmit",
            "HTMLFormElement.reset",
            "HTMLDialogElement.show",
            "HTMLDialogElement.showModal",
            "HTMLDialogElement.close",
            "AbortController.abort",
            "HTMLInputElement.select",
            "ChildNode.remove",
            "Node.removeChild",
            // ADR 0705.
            "HTMLFormElement.submit",
            "ChildNode.replaceWith",
            "Node.replaceChild",
            "ParentNode.replaceChildren",
            "reportError",
            "WindowOrWorkerGlobalScope.reportError",
            "History.go",
            "Location.assign",
            "Location.reload",
            "Location.replace",
        ] {
            assert!(
                synchronously_dispatching_dom_member(dispatching),
                "{dispatching}"
            );
        }
        for inert in [
            "HTMLElement.addEventListener",
            "ParentNode.querySelector",
            "Element.getBoundingClientRect",
            "Element.getAttribute",
            "Element.scrollIntoView",
            "Element.scrollTo",
            "Element.setAttribute",
            "History.pushState",
            "ParentNode.append",
            "Node.appendChild",
            "Animatable.animate",
            "HTMLCanvasElement.getContext",
        ] {
            assert!(!synchronously_dispatching_dom_member(inert), "{inert}");
        }
        assert!(refused_when_optional("HTMLCanvasElement.getContext"));
        assert!(refused_when_optional("OffscreenCanvas.getContext"));
        assert!(!refused_when_optional("Element.scrollIntoView"));
    }
}
