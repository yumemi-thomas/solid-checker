//! ADR 1052: a check Solid makes earlier at the same call throws first.
//!
//! A call can meet two of the runtime's checks, and only the first one to throw reports. Where another of this
//! program's own rows proves (or leaves open) that an earlier check throws at the very call a row stands at, that
//! row's code is not proven to be the one Solid reports, and the run may never reach its check at all. The row is an
//! obligation then, never a violation (the 2026-11-17 ruling, ADR 1018: a check no run reaches gives no violation).
//!
//! The earlier checks, by the order of the pinned rc.21 source (`@solidjs/signals` `dev.js`):
//! - creating a primitive: the effect-callback guard (SC6002's `PRIMITIVE_IN_EFFECT_CALLBACK`) and the leaf owner's
//!   refusal (SC3001's `PRIMITIVE_IN_FORBIDDEN_SCOPE`, `CLEANUP_IN_FORBIDDEN_SCOPE`) throw before the owner check
//!   (`NO_OWNER_EFFECT` and the other owner codes, SC4001) is reached; on a leaf owner's caller's run the refusal throws
//!   first, and nothing orders that run against another caller's (a halt does not stop a later creation: rc.21 `halted`
//!   gates only `schedule` and `flush`), so the owner row is an obligation;
//! - `createEffect` with no callable effect function throws `MISSING_EFFECT_FN` (SC7001) before the leaf owner's
//!   refusal (SC3001);
//! - `useContext` throws `NO_OWNER_CONTEXT` (SC4001's context read) before it looks for a provider
//!   (`CONTEXT_NOT_FOUND`, SC4002).
//!
//! Only a proven earlier check counts (an obligation of it proves no throw), only rows that would be violations are touched, and a call is matched by its path and first byte (a call and its
//! callee start at the same byte; a call nested in another's arguments does not).

use crate::indexes::SemanticLookup;
use crate::{Location, OwnerRequirementOperation, Program, StaticDefectKind};

/// The blocker an uncertain static violation is reported under: the one its context names, else the rule's own
/// (`blockers::for_rule`, what projection gives it, ADR 0680).
fn violation_blocker(violation: &crate::StaticViolation) -> &str {
    crate::blocked_by::blocked_kind(&violation.analysis_context)
        .unwrap_or_else(|| crate::blockers::for_rule(&violation.id))
}

fn same_call(left: &Location, right: &Location) -> bool {
    left.path == right.path && left.start_byte == right.start_byte
}

/// Where a leaf operation is written: its own call, or the sites of the helper it is reached through (ADR 0942).
fn operation_sites(operation: &crate::LeafOwnerOperation) -> &[Location] {
    if operation.sites.is_empty() {
        std::slice::from_ref(&operation.location)
    } else {
        &operation.sites
    }
}

/// Marks each row whose call meets an earlier check that throws (see the module docs).
pub(crate) fn mark(
    program: &mut Program,
    settled_gates: &crate::owners::SettledGateDecisions,
    lookup: &SemanticLookup<'_>,
) {
    // Calls an earlier check is proven to refuse, and so never reach the owner check (an obligation of either proves no
    // throw: where it does not throw, the owner check is reached, and Chrome reports NO_OWNER_* at such calls):
    // - the effect-callback guard (SC6002's violations) throws when an effect is created, before `createEffect` reports
    //   `NO_OWNER_EFFECT` at its end; a boundary reports `NO_OWNER_BOUNDARY` before it creates anything, so the guard is
    //   earlier for an effect only;
    // - the leaf owner's refusal (SC3001's violations, at the operation's own call) throws on the leaf caller's run, and
    //   nothing orders that run against an unowned caller's (a halt does not stop a later creation), so the owner row is
    //   an obligation: every operation.
    let effect_guarded: Vec<Location> = program
        .static_violations
        .iter()
        .filter(|violation| violation.id == "SC6002" && !violation.uncertain)
        .map(|violation| violation.location.clone())
        .collect();
    // ADR 1111: a read written after a creation the effect-callback guard refuses or may refuse (an SC6002 violation, or its
    // `effect-owner-unproven` obligation: an apply runs the creation, its owner unproven; not a creation no row places, which
    // proves no throw: unknown counts as reachable) in the same function, or in a function that function calls by name after the creation, may never run on
    // that run: the guard throws first (`const f = () => s.v; createMemo(f); f();` in an effect's apply; the row stands at
    // the call into the helper, the read at its origin). The row is an obligation. A read the pending rule holds open
    // (ADR 0509) is one already, and its pending row hangs on the open read: it is left as it is.
    let may_refuse: Vec<Location> = program
        .static_violations
        .iter()
        .filter(|violation| {
            violation.id == "SC6002"
                && (!violation.uncertain
                    || violation_blocker(violation) == crate::blockers::EFFECT_OWNER_UNPROVEN)
        })
        .map(|violation| violation.location.clone())
        .collect();
    let pending_open: std::collections::HashSet<Location> = program
        .async_reads
        .iter()
        .filter(|read| read.async_provenance && !read.apply_first_run_pending && !read.unaccounted)
        .map(|read| read.location.clone())
        .collect();
    for read in &mut program.reads {
        if read.uncertain
            || pending_open.contains(&read.location)
            || !read_after_refused_creation(
                read.origin.as_ref().unwrap_or(&read.location),
                &may_refuse,
                lookup,
            )
        {
            continue;
        }
        read.uncertain = true;
        read.context =
            crate::blockers::with_detail(crate::blockers::CREATION_THROWS_FIRST, &read.context)
                .into();
    }
    // ADR 1052, part 3: SC6002's `effect-owner-unproven` obligation names a caller that runs the creation in an effect's
    // apply (a helper the apply and a handler or a render both call). On that run the guard throws first and halts; the
    // run the owner row proves unowned may be that one, and no fact orders the others' runs against the halt: the owner
    // row is an obligation. The accounting obligation (`effect-creation-unaccounted`) names no caller and is not this.
    let effect_may_guard: Vec<Location> = program
        .static_violations
        .iter()
        .filter(|violation| {
            violation.id == "SC6002"
                && violation.uncertain
                && violation_blocker(violation) == crate::blockers::EFFECT_OWNER_UNPROVEN
        })
        .map(|violation| violation.location.clone())
        .chain(effect_guarded.iter().cloned())
        .collect();
    let leaf_refused: Vec<Location> = program
        .leaf_operations
        .iter()
        .filter(|operation| !operation.uncertain && !operation.possible)
        .flat_map(|operation| operation_sites(operation).iter().cloned())
        .collect();
    let missing_effect_function: Vec<Location> = program
        .static_defects
        .iter()
        .filter(|defect| {
            !defect.uncertain && matches!(defect.kind, StaticDefectKind::MissingEffectFunction)
        })
        .map(|defect| defect.location.clone())
        .collect();
    // Context reads proven to run with no owner: a missing-owner violation of the read. Its obligation is no proof: a
    // read whose owner varies by caller is reported `CONTEXT_NOT_FOUND` by Chrome as often as `NO_OWNER_CONTEXT`.
    let unowned_reads: Vec<Location> = program
        .missing_owners
        .iter()
        .filter(|requirement| {
            requirement.report
                && !requirement.uncertain
                && requirement.operation == OwnerRequirementOperation::ContextRead
        })
        .map(|requirement| requirement.location.clone())
        .collect();
    // ADR 1139: the code a JSX handler may run, computed once and only where an apply may deliver into a shadow tree.
    let shadow_reach_memo = std::cell::OnceCell::new();
    for requirement in &mut program.missing_owners {
        if requirement.uncertain || !requirement.report {
            continue;
        }
        let earlier = match requirement.operation {
            OwnerRequirementOperation::ContextProvider => {
                unowned_reads
                    .iter()
                    .any(|read| same_call(read, &requirement.location))
                    || later_render_apply_run_only(&requirement.location, lookup)
                    || in_handler_the_veto_may_leave_unowned(&requirement.location, lookup)
            }
            OwnerRequirementOperation::ContextRead => false,
            operation => {
                (operation == OwnerRequirementOperation::Effect
                    && (effect_may_guard
                        .iter()
                        .any(|guarded| same_call(guarded, &requirement.location))
                        || in_slot_called_only_in_applies(&requirement.location, true, lookup)
                        // ADR 1115 part 2: the apply's delivery to a shadow-tree listener runs the creation in the effect-callback window.
                        || creation_an_apply_may_deliver_in_shadow(&requirement.location, &shadow_reach_memo, lookup)))
                    || leaf_refused
                        .iter()
                        .any(|refused| same_call(refused, &requirement.location))
            }
        };
        if earlier {
            requirement.uncertain = true;
            requirement.earlier_check_throws = true;
        }
    }
    // SC7002 claims the creation reaches its result probe (SYNC_NODE_RECEIVED_ASYNC), which comes after every creation guard.
    // An earlier check proven to throw at the creation (`effect_guarded`, `leaf_refused` at the call itself) ends the call
    // first. No obligation counts here: the accounting ones (`effect-creation-unaccounted`, `leaf-operation-unaccounted`)
    // name no caller (taking them cost 964 smoke programs Chrome reports SYNC_NODE_RECEIVED_ASYNC at), and
    // `effect-owner-unproven` names an apply caller while another caller (a render, a rerun) may reach the probe first
    // (14 such smoke programs, ADR 1052 part 3).
    let creation_may_throw: Vec<Location> = effect_guarded
        .iter()
        .cloned()
        // A leaf refusal counts only at a call written in the leaf callback itself, whose every run it refuses: one reached
        // through a helper refuses that caller's run alone, and another caller (a render) may reach the probe first.
        .chain(
            program
                .leaf_operations
                .iter()
                .filter(|operation| {
                    !operation.uncertain && !operation.possible && operation.sites.is_empty()
                })
                .map(|operation| operation.location.clone()),
        )
        .collect();
    // ADR 1052 part 5: a refusal reached through a helper counts when it refuses every run of the creation: the helper holding
    // it is a named function nothing exports or reassigns, and every reference to it is the exact call of a proven refusal of
    // that creation (the anchor of an SC3001 violation whose `sites` hold it). Another caller would be a run nothing refuses.
    let refused_through: Vec<(Location, Location)> = program
        .leaf_operations
        .iter()
        .filter(|operation| !operation.uncertain && !operation.possible)
        .flat_map(|operation| {
            operation
                .sites
                .iter()
                .map(|site| (site.clone(), operation.location.clone()))
        })
        .collect();
    for violation in &mut program.static_violations {
        if violation.id != "SC7002" || violation.uncertain {
            continue;
        }
        let anchors: Vec<&Location> = refused_through
            .iter()
            .filter(|(site, _)| same_call(site, &violation.location))
            .map(|(_, anchor)| anchor)
            .collect();
        if creation_may_throw
            .iter()
            .any(|refused| same_call(refused, &violation.location))
            || (!anchors.is_empty() && every_run_refused(&violation.location, &anchors, lookup))
            || later_render_apply_run_only(&violation.location, lookup)
            || in_rerun_cleanup(&violation.location, lookup)
            || forwarded_into_leaf(&violation.location, lookup)
            // ADR 1052 part 8: a render effect's apply is no such run here, since its first run is owned and reaches the probe.
            || in_slot_called_only_in_applies(&violation.location, false, lookup)
            || prop_runs_only_in_owned_leaf(&violation.location, settled_gates, lookup)
            || refused_at_mount_else_handlers(&violation.location, settled_gates, lookup)
            || listener_detached_in_owned_leaf(&violation.location, settled_gates, lookup)
            || creation_an_apply_may_deliver_in_shadow(&violation.location, &shadow_reach_memo, lookup)
        {
            violation.uncertain = true;
            violation.analysis_context = crate::blockers::with_detail(
                crate::blockers::CREATION_THROWS_FIRST,
                &violation.analysis_context,
            );
            violation.message.push_str("; but a check Solid makes earlier at this creation (the effect-callback guard or a leaf owner's refusal) may throw first, so the result probe is not proven to be reached");
        }
    }
    for operation in &mut program.leaf_operations {
        if operation.uncertain || operation.possible {
            continue;
        }
        if operation_sites(operation).iter().any(|site| {
            missing_effect_function
                .iter()
                .any(|refused| same_call(refused, site))
        }) {
            operation.uncertain = true;
            operation.earlier_check_throws = true;
        }
    }
}

/// ADR 1052 part 5: whether every run of the code at `site` is one of `anchors`: the function written directly around it is a
/// synchronous, non-generator function bound to a name (`function_binding_name`) that no module exports and nothing rebinds,
/// with at least one reference, each of them the callee of a call that starts where an anchor does (the same file). `false`
/// for anything else (a method, a slot, an alias, a handler, an argument, a reference in another file).
fn every_run_refused(site: &Location, anchors: &[&Location], lookup: &SemanticLookup<'_>) -> bool {
    let Some(file) = lookup.file_by_path(site.path.as_ref()) else {
        return false;
    };
    let (Ok(start), Ok(end)) = (u32::try_from(site.start_byte), u32::try_from(site.end_byte))
    else {
        return false;
    };
    let Some(function) =
        crate::owners::owning_function(&file.ast, solid_facts::core::Span::new(start, end))
    else {
        return false;
    };
    // The name every run goes through: the function's own (a declaration or the `const` it initializes), or, for a generator,
    // the `const` the `action(...)` it is handed to initializes (ADR 1052 part 6: each call of the action steps the generator,
    // and its first step runs on that call's stack).
    let binding = if function.r#async {
        None
    } else if function.generator {
        action_binding(file, function, lookup)
    } else if crate::execution_role::module_exports(file, function, lookup)
        || !lookup.function_value_is_current(file, function)
    {
        None
    } else {
        crate::owners::function_binding_name(file, function).map(|name| name.span)
    };
    let Some(binding) = binding else {
        return false;
    };
    let binding = solid_facts::ast::NamedSpan { span: binding };
    let Some(symbol) = lookup.entities().at(file.path.as_str(), binding.span) else {
        return false;
    };
    let references: Vec<_> = lookup
        .symbol_references(symbol.as_str())
        .into_iter()
        .filter(|reference| {
            !(reference.path.as_ref() == file.path.as_str()
                && reference.start_byte == u64::from(binding.span.start))
        })
        .collect();
    !references.is_empty()
        && references.iter().all(|reference| {
            reference.path.as_ref() == file.path.as_str()
                && file.ast.calls.iter().any(|call| {
                    !call.construct
                        && u64::from(file.ast.peel_ts_sugar_span(call.callee).start)
                            == reference.start_byte
                        && anchors.iter().any(|anchor| {
                            anchor.path.as_ref() == file.path.as_str()
                                && anchor.start_byte == u64::from(call.span.start)
                        })
                })
        })
}

/// ADR 1052 part 5: `site` is written directly in the apply of a primitive whose apply runs first at creation under the
/// caller's owner and on every later run in the effect phase with no owner (`Dialect::callback_reruns_in_effect_phase`:
/// `createRenderEffect`'s), and it is no completion site of that apply (`completion_sites`: an earlier `return`, `throw`,
/// `break` or `continue`, or a branch, may skip it). The owned first run may never reach it; every later run meets the
/// effect-callback guard (a creation) or the owner check (a context read) first. `false` for anything else.
/// ADR 1115 part 2, widened by ADR 1139: an effect's apply may deliver an event to a listener in a shadow tree. That delivery runs a
/// JSX handler inside the apply's effect-callback window, where the guard throws PRIMITIVE_IN_EFFECT_CALLBACK at a creation first,
/// and nothing orders another delivery's run before it. The creation need not be written in the handler: a helper, a closure or an
/// IIFE the handler runs creates there too ([`ShadowReach`]). ADR 1125 left the owner bit to cover these; the owner bit now leaves
/// delivery alone, so the check is this one.
fn creation_an_apply_may_deliver_in_shadow(
    site: &Location,
    reach: &std::cell::OnceCell<ShadowReach>,
    lookup: &SemanticLookup<'_>,
) -> bool {
    if !lookup.apply_delivery_in_shadow_possible() {
        return false;
    }
    // Bundle j smoke (ADR 1139): the reach is the code a shadow delivery from an apply may run, so it is asked only where such a delivery is
    // possible; elsewhere (the delivery premise broken with no apply dispatch, a listener object, an `eval`) the creation counts only when it is
    // written in a JSX handler, as before ADR 1139.
    if !lookup.apply_dispatch_into_shadow_possible() {
        return in_jsx_handler(site, lookup);
    }
    match reach.get_or_init(|| shadow_reach(lookup)) {
        ShadowReach::Unbounded => true,
        ShadowReach::Functions(functions) => {
            let (Ok(start), Ok(end)) =
                (u32::try_from(site.start_byte), u32::try_from(site.end_byte))
            else {
                return true;
            };
            let span = solid_facts::core::Span::new(start, end);
            functions.iter().any(|(path, function)| {
                path.as_str() == site.path.as_ref() && function.contains(span)
            })
        }
    }
}

/// The code a JSX handler may run on its own stack: the handler attribute values (every function written in one, and the function a value
/// names: a value nothing resolves may be any function), the exact handlers an intrinsic element's spread hands it (an unresolved spread
/// may hand any), and the functions they hand on or call, transitively. Per call in the reach:
/// - an argument that is no function literal (those are inside the reach already) and not proven non-callable is followed to the
///   function it names; one nothing names may be any function;
/// - a Solid primitive's call adds nothing more (its callbacks are its arguments);
/// - a callee that is a value a primitive returned (an accessor, a setter: a `const` bound to the primitive's call, its tuple slots)
///   may run the callbacks handed to that call (a memo's compute runs where its accessor is read), so their code joins the reach;
/// - any other callee is followed to the function it resolves to; one that resolves to none may run any function.
///
/// "Any function" is [`ShadowReach::Unbounded`].
enum ShadowReach {
    Unbounded,
    Functions(Vec<(String, solid_facts::core::Span)>),
}

fn shadow_reach(lookup: &SemanticLookup<'_>) -> ShadowReach {
    let mut reached: Vec<(String, solid_facts::core::Span)> = Vec::new();
    for file in lookup.files() {
        for element in &file.ast.jsx_elements {
            for attribute in &element.attributes {
                let handler = attribute.namespace.is_some()
                    || file
                        .source_text(attribute.local_name)
                        .is_some_and(|name| name.len() > 2 && name.starts_with("on"));
                if let (true, Some(value)) = (handler, attribute.value) {
                    reached.push((file.path.to_string(), value));
                    // ADR 1139 r1 (F1): a handler named by its value (`onScroll={handle}`, `obj.m`, `props.h`) runs the function the value
                    // names; one nothing resolves may be any function.
                    if let Some(expression) = attribute.expression {
                        let expression = file.ast.peel_ts_sugar_span(expression);
                        let literal = file
                            .ast
                            .function_at(expression)
                            .is_some_and(|function| function.span == expression);
                        if !literal {
                            match lookup
                                .function_called_at(file.path.as_str(), expression)
                                .or_else(|| lookup.object_member_function(file, expression))
                            {
                                Some((target_file, function)) => {
                                    reached.push((target_file.path.to_string(), function.span));
                                }
                                None => return ShadowReach::Unbounded,
                            }
                        }
                    }
                }
            }
            // ADR 1139 r1 (F2): the handlers an intrinsic element's spread hands it (ADR 0358's exact targets); an unresolved spread may hand any
            // function. A component's spread is its props: where the component spreads them onto an element, that spread is asked there.
            if !crate::object_members::intrinsic_element(file, element) {
                continue;
            }
            for spread in &element.spreads {
                match lookup
                    .spread_handler_targets()
                    .get(&crate::location(file.path.shared(), spread.argument))
                {
                    Some(Some(targets)) => {
                        reached
                            .extend(targets.iter().map(|(path, span)| (path.to_string(), *span)));
                    }
                    Some(None) => return ShadowReach::Unbounded,
                    None => return ShadowReach::Unbounded,
                }
            }
        }
    }
    let mut next = 0;
    while next < reached.len() {
        let (path, region) = reached[next].clone();
        next += 1;
        let Some(file) = lookup.file_by_path(&path) else {
            return ShadowReach::Unbounded;
        };
        for call in file
            .ast
            .calls
            .iter()
            .filter(|call| region.contains(call.span))
        {
            let mut found = Vec::new();
            if !handed_functions(file, &call.arguments, lookup, &mut found) {
                return ShadowReach::Unbounded;
            }
            if lookup.primitive_at_call(file, call.span).is_none() {
                if let Some((target_file, function)) = lookup
                    .function_called_at(file.path.as_str(), call.callee)
                    .or_else(|| lookup.object_member_function(file, call.callee))
                {
                    found.push((target_file.path.to_string(), function.span));
                } else if let Some((binding_file, initializer)) =
                    primitive_value_initializer(file, call.callee, lookup)
                {
                    found.push((binding_file.path.to_string(), initializer.span));
                    if !handed_functions(binding_file, &initializer.arguments, lookup, &mut found) {
                        return ShadowReach::Unbounded;
                    }
                } else {
                    return ShadowReach::Unbounded;
                }
            }
            for target in found {
                if !reached.contains(&target) {
                    reached.push(target);
                }
            }
        }
    }
    ShadowReach::Functions(reached)
}

/// The functions `arguments` hand on that are not written in them (see [`ShadowReach`]), pushed to `found`; false when one may be a
/// function nothing names.
fn handed_functions(
    file: &solid_facts::FileFacts,
    arguments: &[solid_facts::ast::ArgumentFact],
    lookup: &SemanticLookup<'_>,
    found: &mut Vec<(String, solid_facts::core::Span)>,
) -> bool {
    use solid_facts::ast::ArgumentValueKind;
    for argument in arguments {
        if matches!(
            argument.value,
            ArgumentValueKind::Function
                | ArgumentValueKind::AsyncFunction
                | ArgumentValueKind::Undefined
                | ArgumentValueKind::Null
        ) || (!argument.spread && argument.runtime_value_kind.is_proven_noncallable())
        {
            continue;
        }
        match lookup
            .function_called_at(file.path.as_str(), argument.span)
            .or_else(|| lookup.object_member_function(file, argument.span))
        {
            Some((target_file, function)) => {
                found.push((target_file.path.to_string(), function.span))
            }
            None => return false,
        }
    }
    true
}

/// The primitive call a `const` called at `callee` is bound to (a `createSignal` tuple, a `createMemo` accessor): the callee is a value
/// that primitive returned.
fn primitive_value_initializer<'a>(
    file: &solid_facts::FileFacts,
    callee: solid_facts::core::Span,
    lookup: &SemanticLookup<'a>,
) -> Option<(&'a solid_facts::FileFacts, &'a solid_facts::ast::CallFact)> {
    let callee = file.ast.peel_ts_sugar_span(callee);
    let (binding_file, binding, _) = lookup.binding_at_reference(file.path.as_str(), callee)?;
    if !binding.immutable {
        return None;
    }
    let initializer = binding_file
        .ast
        .peel_ts_sugar_span(binding.call_initializer?);
    let call = binding_file
        .ast
        .calls
        .iter()
        .find(|call| call.span == initializer)?;
    lookup.primitive_at_call(binding_file, call.span)?;
    Some((binding_file, call))
}

/// ADR 1125 (#296 smoke census): a context read in a JSX handler whose ownerless proof the delivery premise withholds. A delivery runs
/// the handler with no owner, where Solid throws `NO_OWNER_CONTEXT` before it looks for a provider; an owned run is only the veto's
/// possibility (an apply under an owned flush, a call the scan cannot rule out), so `ContextNotFoundError` is not proven.
fn in_handler_the_veto_may_leave_unowned(site: &Location, lookup: &SemanticLookup<'_>) -> bool {
    // #296 re-run: only where the veto comes from the delivery premise (code may run the handler with no event and no owner) or
    // from an apply's delivery into a shadow tree; a visible dispatch on an owned stack runs it owned, where CONTEXT_NOT_FOUND holds.
    (!lookup.event_delivery_intact() || lookup.apply_delivery_in_shadow_possible())
        && in_jsx_handler(site, lookup)
}

/// Whether `site` sits in the value of a JSX handler attribute (`onScroll`, `onclick`, `on:scroll`).
fn in_jsx_handler(site: &Location, lookup: &SemanticLookup<'_>) -> bool {
    let Some(file) = lookup.file_by_path(site.path.as_ref()) else {
        return false;
    };
    let (Ok(start), Ok(end)) = (u32::try_from(site.start_byte), u32::try_from(site.end_byte))
    else {
        return false;
    };
    let span = solid_facts::core::Span::new(start, end);
    file.ast.jsx_elements.iter().any(|element| {
        element.attributes.iter().any(|attribute| {
            // `onScroll`, `onclick`, and `on:scroll` (a namespaced listener); a name that is no handler only widens the answer.
            attribute.value.is_some_and(|value| value.contains(span))
                && (attribute.namespace.is_some()
                    || file
                        .source_text(attribute.local_name)
                        .is_some_and(|name| name.len() > 2 && name.starts_with("on")))
        })
    })
}

fn later_render_apply_run_only(site: &Location, lookup: &SemanticLookup<'_>) -> bool {
    let Some(file) = lookup.file_by_path(site.path.as_ref()) else {
        return false;
    };
    let (Ok(start), Ok(end)) = (u32::try_from(site.start_byte), u32::try_from(site.end_byte))
    else {
        return false;
    };
    let span = solid_facts::core::Span::new(start, end);
    let Some(apply) = crate::owners::owning_function(&file.ast, span) else {
        return false;
    };
    let ast = &file.ast;
    let rerunning_apply = ast.arguments_containing(apply.span).any(|(call, index)| {
        ast.peel_ts_sugar_span(call.arguments[index].span) == apply.span
            && call.arguments[..=index]
                .iter()
                .all(|argument| !argument.spread)
            && lookup
                .primitive_at_call(file, call.span)
                .is_some_and(|primitive| {
                    lookup
                        .dialect
                        .callback_reruns_in_effect_phase(primitive, index)
                })
    });
    rerunning_apply
        && !ast
            .completion_sites
            .iter()
            .any(|completion| completion.start == span.start && apply.body.contains(*completion))
}

/// ADR 1052 part 6: the name of the `const` that `action(generator)` initializes, `generator` written as the action's only
/// callback argument (no spread), the binding a single identifier that is a local or no export statement holds, and nothing
/// writes. `None` for anything else.
fn action_binding(
    file: &solid_facts::FileFacts,
    generator: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
) -> Option<solid_facts::core::Span> {
    let ast = &file.ast;
    // The exact recognition the leaf walk steps the action with (`named_action_generator`).
    let (call, _) = crate::owners::stepped_action_generator(file, generator, lookup)?;
    let binding = ast.bindings.iter().find(|binding| {
        binding.immutable
            && binding.shape == solid_facts::ast::BindingShape::Identifier
            && binding.names.len() == 1
            && binding
                .initializer
                .is_some_and(|initializer| ast.peel_ts_sugar_span(initializer) == call.span)
    })?;
    if ast
        .exports
        .iter()
        .any(|export| export.span.contains(binding.declaration))
        // A binding written in a function is a local: an export statement around that function exports the function.
        && crate::owners::owning_function(ast, binding.declaration).is_none()
        || crate::indexes::binding_written(file, binding.names[0].span)
    {
        return None;
    }
    Some(binding.names[0].span)
}

/// ADR 1155 r3: the creation at `site` runs only in a leaf callback ADR 0200's proof forwards it to
/// ([`crate::execution_role::forwarded_only_into_leaf`]): the leaf refuses it before the result probe.
fn forwarded_into_leaf(site: &Location, lookup: &SemanticLookup<'_>) -> bool {
    site_in_file(site, lookup).is_some_and(|(file, span)| {
        crate::execution_role::forwarded_only_into_leaf(file, span, lookup)
    })
}

/// The analyzed file and span of `site`: one read site for [`in_rerun_cleanup`] and [`forwarded_into_leaf`].
fn site_in_file<'a>(
    site: &Location,
    lookup: &SemanticLookup<'a>,
) -> Option<(&'a solid_facts::FileFacts, solid_facts::core::Span)> {
    let file = lookup.file_by_path(site.path.as_ref())?;
    let (Ok(start), Ok(end)) = (u32::try_from(site.start_byte), u32::try_from(site.end_byte))
    else {
        return None;
    };
    Some((file, solid_facts::core::Span::new(start, end)))
}

/// ADR 1052 part 6: `site` is written directly in the cleanup an effect's apply returns, and the effect may rerun
/// (`directly_in_rerun_effect_cleanup`, ADR 0539's `WindowedCleanup::MayRerun`): a rerun calls the cleanup inside the effect
/// callback, where a creation meets the guard first, and nothing orders that rerun against the disposal that would run it
/// outside. An effect whose compute provably tracks nothing never reruns and is not this.
fn in_rerun_cleanup(site: &Location, lookup: &SemanticLookup<'_>) -> bool {
    site_in_file(site, lookup).is_some_and(|(file, span)| {
        crate::execution_role::directly_in_rerun_effect_cleanup(file, span, lookup)
    })
}

/// ADR 1052 part 7: `site` is written directly in a function that is the value of a data property of an object literal a
/// module-scope `const` holds (nothing exports or writes that binding), and every reference to the binding is the object of a
/// member that resolves exactly to that function (`object_member_function`) and is the callee of a call written directly in an
/// apply the effect phase runs (`Dialect::callback_runs_in_effect_phase`, an effect's apply, or `callback_reruns_in_effect_phase`,
/// a render effect's, whose first run is owned). Every unowned run of the function is then on an apply's stack, where creating
/// an effect throws PRIMITIVE_IN_EFFECT_CALLBACK before `NO_OWNER_EFFECT` is reported. (Under ADR 1089's closed module entry the
/// owner graph places `h1.go()` there, while SC6002's walk leaves the slot `function-run-unplaced`.) `false` for anything else:
/// another reference (a handler, a hand-off, an export), a computed member, a slot of a literal no `const` holds. Part 8: SC7002
/// asks with `rerunning_applies` false, since a render effect's owned first run reaches the result probe (no guard there).
fn in_slot_called_only_in_applies(
    site: &Location,
    rerunning_applies: bool,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let Some(file) = lookup.file_by_path(site.path.as_ref()) else {
        return false;
    };
    let (Ok(start), Ok(end)) = (u32::try_from(site.start_byte), u32::try_from(site.end_byte))
    else {
        return false;
    };
    let ast = &file.ast;
    let Some(function) =
        crate::owners::owning_function(ast, solid_facts::core::Span::new(start, end))
    else {
        return false;
    };
    if function.r#async || function.generator {
        return false;
    }
    if !ast.object_properties.iter().any(|property| {
        property.data
            && !property.computed
            && ast.peel_ts_sugar_span(property.value) == function.span
    }) {
        return false;
    }
    // The module `const` whose initializer is the literal holding the slot.
    let Some(binding) = ast.bindings.iter().find(|binding| {
        binding.immutable
            && binding.shape == solid_facts::ast::BindingShape::Identifier
            && binding.names.len() == 1
            && binding.initializer.is_some_and(|initializer| {
                initializer.contains(function.span)
                    && ast.object_properties.iter().any(|property| {
                        ast.peel_ts_sugar_span(property.value) == function.span
                            && ast.peel_ts_sugar_span(initializer).contains(property.span)
                    })
            })
    }) else {
        return false;
    };
    let name = binding.names[0].span;
    if crate::owners::owning_function(ast, binding.declaration).is_some()
        || ast
            .exports
            .iter()
            .any(|export| export.span.contains(binding.declaration))
        || crate::indexes::binding_written(file, name)
    {
        return false;
    }
    let Some(symbol) = lookup.entities().at(file.path.as_str(), name) else {
        return false;
    };
    let references: Vec<_> = lookup
        .symbol_references(symbol.as_str())
        .into_iter()
        .filter(|reference| {
            !(reference.path.as_ref() == file.path.as_str()
                && reference.start_byte == u64::from(name.start))
        })
        .collect();
    let in_apply = |call: &solid_facts::ast::CallFact| {
        crate::owners::owning_function(ast, call.span).is_some_and(|apply| {
            ast.arguments_containing(apply.span).any(|(holder, index)| {
                ast.peel_ts_sugar_span(holder.arguments[index].span) == apply.span
                    && holder.arguments[..=index]
                        .iter()
                        .all(|argument| !argument.spread)
                    && lookup
                        .primitive_at_call(file, holder.span)
                        .is_some_and(|primitive| {
                            lookup
                                .dialect
                                .callback_runs_in_effect_phase(primitive, index)
                                || rerunning_applies
                                    && lookup
                                        .dialect
                                        .callback_reruns_in_effect_phase(primitive, index)
                        })
            })
        })
    };
    !references.is_empty()
        && references.iter().all(|reference| {
            reference.path.as_ref() == file.path.as_str()
                && ast.members.iter().any(|member| {
                    u64::from(ast.peel_ts_sugar_span(member.object).start) == reference.start_byte
                        && ast.computed_members.binary_search(&member.span).is_err()
                        && lookup
                            .object_member_function(file, member.span)
                            .is_some_and(|(target, slot)| {
                                target.path.as_str() == file.path.as_str()
                                    && slot.span == function.span
                            })
                        && ast.calls.iter().any(|call| {
                            !call.construct
                                && ast.peel_ts_sugar_span(call.callee) == member.span
                                && in_apply(call)
                        })
                })
        })
}

/// ADR 1052 part 8: `site` is a creation the leaf owner refuses (`creates_forbidden_primitive`), written directly in a
/// synchronous function that a project component's prop holds, and every call that runs it
/// (`prop_callback_invocations`: through a named function, a forwarded prop or a spread props object) is written directly
/// in the callback of a leaf owner (`CallbackOwner::Leaf`, `onSettled`'s), whose own call the owner fixed point decided
/// runs under a live owner (`LeafGateDecision::Owned`; a gate with no decision proves nothing here). Every run of the
/// creation is then on that leaf owner's stack, where it throws PRIMITIVE_IN_FORBIDDEN_SCOPE before the result probe.
/// `false` for anything else: another call of the function, a call in a nested function, an out-of-band or undecided
/// `onSettled`.
fn prop_runs_only_in_owned_leaf(
    site: &Location,
    settled_gates: &crate::owners::SettledGateDecisions,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let Some(file) = lookup.file_by_path(site.path.as_ref()) else {
        return false;
    };
    let (Ok(start), Ok(end)) = (u32::try_from(site.start_byte), u32::try_from(site.end_byte))
    else {
        return false;
    };
    let span = solid_facts::core::Span::new(start, end);
    let Some(creation) = file.ast.calls.iter().find(|call| call.span.start == start) else {
        return false;
    };
    if !lookup
        .primitive_at_call(file, creation.span)
        .is_some_and(|primitive| {
            crate::cleanup::creates_forbidden_primitive(lookup.dialect, file, creation, primitive)
        })
    {
        return false;
    }
    let Some(function) = crate::owners::owning_function(&file.ast, span) else {
        return false;
    };
    if function.r#async || function.generator {
        return false;
    }
    let Some(calls) = crate::execution_role::prop_callback_invocations(file, function, lookup)
    else {
        return false;
    };
    !calls.is_empty()
        && calls.iter().all(|(call_file, call)| {
            let ast = &call_file.ast;
            let Some(callback) = crate::owners::owning_function(ast, *call) else {
                return false;
            };
            ast.arguments_containing(callback.span)
                .any(|(owner_call, index)| {
                    ast.peel_ts_sugar_span(owner_call.arguments[index].span) == callback.span
                        && owner_call.arguments[..=index]
                            .iter()
                            .all(|argument| !argument.spread)
                        && lookup
                            .primitive_at_call(call_file, owner_call.span)
                            .is_some_and(|primitive| {
                                crate::owners::callback_owner_at_call(
                                    call_file, owner_call, primitive, index, lookup,
                                ) == Some(solid_dialect::CallbackOwner::Leaf)
                                    && (!lookup
                                        .dialect
                                        .leaf_owner_requires_owned_call_site(primitive)
                                        || settled_gates.get(&(
                                            call_file.path.to_string(),
                                            u64::from(owner_call.span.start),
                                            u64::from(owner_call.span.end),
                                        )) == Some(&crate::owners::LeafGateDecision::Owned))
                            })
                })
        })
}

/// ADR 1052 part 9: whether `site`, a creation the leaf owner and the effect-callback guard refuse
/// (`creates_forbidden_primitive`), is written directly in a synchronous function bound to a local name (unexported, its value
/// current) whose every reference is the callee of a call of one of two kinds, with at least one of the first:
/// - refused at mount: written directly in an effect's apply (`callback_runs_in_effect_phase`) or in a leaf owner's callback
///   (`CallbackOwner::Leaf`, its call decided `Owned` where the dialect gates it), the primitive's call a completion site of the
///   component `K` around it (it runs on every mount, `completion_sites`): that run throws PRIMITIVE_IN_EFFECT_CALLBACK or
///   PRIMITIVE_IN_FORBIDDEN_SCOPE before the result probe;
/// - an event handler of the same mount: written directly in the value of an `on*` attribute of an intrinsic element in `K`'s
///   JSX. `render` runs the mount and its first flush synchronously and, on a throw, disposes the root and unregisters its
///   delegated events (`@solidjs/web` `render`, rc.21): no event reaches the handler before the refusal, and none after it.
///   Chrome: the handler never runs (`NO_OWNER_CLEANUP` written first in it is never reported).
///
/// `false` for anything else (another caller, a conditional or nested primitive call, two components).
fn refused_at_mount_else_handlers(
    site: &Location,
    settled_gates: &crate::owners::SettledGateDecisions,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let Some(file) = lookup.file_by_path(site.path.as_ref()) else {
        return false;
    };
    let (Ok(start), Ok(end)) = (u32::try_from(site.start_byte), u32::try_from(site.end_byte))
    else {
        return false;
    };
    let ast = &file.ast;
    let Some(creation) = ast.calls.iter().find(|call| call.span.start == start) else {
        return false;
    };
    if !lookup
        .primitive_at_call(file, creation.span)
        .is_some_and(|primitive| {
            crate::cleanup::creates_forbidden_primitive(lookup.dialect, file, creation, primitive)
        })
    {
        return false;
    }
    let Some(function) =
        crate::owners::owning_function(ast, solid_facts::core::Span::new(start, end))
    else {
        return false;
    };
    if function.r#async
        || function.generator
        || crate::execution_role::module_exports(file, function, lookup)
        || !lookup.function_value_is_current(file, function)
    {
        return false;
    }
    let Some(name) = crate::owners::function_binding_name(file, function).map(|name| name.span)
    else {
        return false;
    };
    let Some(symbol) = lookup.entities().at(file.path.as_str(), name) else {
        return false;
    };
    // The component whose mount runs a refusing call, and whether one was seen.
    let mut component: Option<solid_facts::core::Span> = None;
    let mut handlers = Vec::new();
    for reference in lookup.symbol_references(symbol.as_str()) {
        if reference.path.as_ref() != file.path.as_str() {
            return false;
        }
        if reference.start_byte == u64::from(name.start) {
            continue;
        }
        let Some(call) = ast.calls.iter().find(|call| {
            !call.construct
                && u64::from(ast.peel_ts_sugar_span(call.callee).start) == reference.start_byte
        }) else {
            return false;
        };
        let Some(callback) = crate::owners::owning_function(ast, call.span) else {
            return false;
        };
        let refusing = ast
            .arguments_containing(callback.span)
            .find(|(owner_call, index)| {
                ast.peel_ts_sugar_span(owner_call.arguments[*index].span) == callback.span
                    && owner_call.arguments[..=*index]
                        .iter()
                        .all(|argument| !argument.spread)
                    && lookup
                        .primitive_at_call(file, owner_call.span)
                        .is_some_and(|primitive| {
                            lookup
                                .dialect
                                .callback_runs_in_effect_phase(primitive, *index)
                                && apply_runs_at_mount(file, owner_call, primitive, *index, lookup)
                                || crate::owners::callback_owner_at_call(
                                    file, owner_call, primitive, *index, lookup,
                                ) == Some(solid_dialect::CallbackOwner::Leaf)
                                    && (!lookup
                                        .dialect
                                        .leaf_owner_requires_owned_call_site(primitive)
                                        || settled_gates.get(&(
                                            file.path.to_string(),
                                            u64::from(owner_call.span.start),
                                            u64::from(owner_call.span.end),
                                        )) == Some(&crate::owners::LeafGateDecision::Owned))
                        })
            })
            .map(|(owner_call, _)| owner_call.span);
        if let Some(owner_call) = refusing {
            let Some(mounted) = crate::owners::owning_function(ast, owner_call) else {
                return false;
            };
            if !ast.completion_sites.contains(&owner_call)
                || component.is_some_and(|known| known != mounted.span)
            {
                return false;
            }
            component = Some(mounted.span);
            continue;
        }
        // An event handler: the whole value of an `on*` attribute of an intrinsic element.
        let handler = ast.jsx_elements.iter().find(|element| {
            crate::execution_role::intrinsic_element(file, element)
                && element.attributes.iter().any(|attribute| {
                    attribute.namespace.is_none()
                        && file
                            .source_text(attribute.name)
                            .is_some_and(|attribute_name| attribute_name.starts_with("on"))
                        && attribute.expression.is_some_and(|expression| {
                            ast.peel_ts_sugar_span(expression) == callback.span
                        })
                })
        });
        let Some(element) = handler else {
            return false;
        };
        handlers.push(element.span);
    }
    component.is_some_and(|mounted| {
        handlers.iter().all(|element| {
            crate::owners::owning_function(ast, *element)
                .is_some_and(|holder| holder.span == mounted)
        })
    })
}

/// ADR 1052 part 9, the mount premise: an effect's apply runs on the mount's first flush only when its `defer` option is absent or the
/// literal `false`. A deferred apply (`{ defer: true }`, or an option no literal decides) first runs on a later flush (a handler's
/// `flush()`), where no PRIMITIVE_IN_EFFECT_CALLBACK precedes the creation's own check: Chrome reports SYNC_NODE_RECEIVED_ASYNC there
/// (mx-recall `effect-apply-defer-flush-rerun`), so such a call is no refusal at mount.
fn apply_runs_at_mount(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    primitive: solid_dialect::Primitive,
    argument: usize,
    lookup: &SemanticLookup<'_>,
) -> bool {
    lookup
        .dialect
        .effect_apply_defer_options(primitive, argument)
        .is_none_or(|options| {
            matches!(
                crate::call_option_literal(file, call, options, "defer"),
                solid_dialect::OptionLiteral::Absent | solid_dialect::OptionLiteral::False
            )
        })
}

/// Whether `call` is written directly in the callback of a leaf owner (`CallbackOwner::Leaf`) whose own call the owner fixed
/// point decided `Owned` where the dialect gates it: that callback's span.
fn owned_leaf_callback(
    file: &solid_facts::FileFacts,
    call: solid_facts::core::Span,
    settled_gates: &crate::owners::SettledGateDecisions,
    lookup: &SemanticLookup<'_>,
) -> Option<solid_facts::core::Span> {
    let ast = &file.ast;
    let callback = crate::owners::owning_function(ast, call)?;
    ast.arguments_containing(callback.span)
        .any(|(owner_call, index)| {
            ast.peel_ts_sugar_span(owner_call.arguments[index].span) == callback.span
                && owner_call.arguments[..=index]
                    .iter()
                    .all(|argument| !argument.spread)
                && lookup
                    .primitive_at_call(file, owner_call.span)
                    .is_some_and(|primitive| {
                        crate::owners::callback_owner_at_call(
                            file, owner_call, primitive, index, lookup,
                        ) == Some(solid_dialect::CallbackOwner::Leaf)
                            && (!lookup
                                .dialect
                                .leaf_owner_requires_owned_call_site(primitive)
                                || settled_gates.get(&(
                                    file.path.to_string(),
                                    u64::from(owner_call.span.start),
                                    u64::from(owner_call.span.end),
                                )) == Some(&crate::owners::LeafGateDecision::Owned))
                    })
        })
        .then_some(callback.span)
}

/// ADR 1052 part 10: whether `site`, a creation the leaf owner refuses (`creates_forbidden_primitive`), is written directly in a
/// synchronous function literal registered whole as a listener (a `lib.dom` `addEventListener(type, listener)`, `type` a string
/// literal) on a fresh element: a `const` of a component `K` (unexported, unwritten) that `Document.createElement` initializes,
/// whose every other reference is
/// - written directly in one owned leaf callback (`owned_leaf_callback`) that also holds `el.remove()` (`ChildNode.remove`) as one
///   of its completion sites: whatever that callback dispatches to the element runs on the leaf owner's stack, where the creation
///   throws PRIMITIVE_IN_FORBIDDEN_SCOPE first, and on its every completion the element has left the page; or
/// - written directly in `K`'s body (which runs before the settled callback) as the receiver of a standard-library call that
///   delivers nothing synchronously, or only events that are not `type` to its receiver (`events_delivered_to_receiver`: a
///   `focus()` before a `blur` listener), or as the whole argument of `ParentNode.append` or `Node.appendChild`.
///
/// No later event then reaches the listener: the element is detached and nothing else holds it (Chrome:
/// `evt-settled-fire-remove`, only PRIMITIVE_IN_FORBIDDEN_SCOPE, also with ADR 1101's later events). `false` for anything else:
/// an element that stays attached, another holder, a reference in a nested function, a dispatching call outside the callback.
fn listener_detached_in_owned_leaf(
    site: &Location,
    settled_gates: &crate::owners::SettledGateDecisions,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let Some(file) = lookup.file_by_path(site.path.as_ref()) else {
        return false;
    };
    let (Ok(start), Ok(end)) = (u32::try_from(site.start_byte), u32::try_from(site.end_byte))
    else {
        return false;
    };
    let ast = &file.ast;
    let Some(creation) = ast.calls.iter().find(|call| call.span.start == start) else {
        return false;
    };
    if !lookup
        .primitive_at_call(file, creation.span)
        .is_some_and(|primitive| {
            crate::cleanup::creates_forbidden_primitive(lookup.dialect, file, creation, primitive)
        })
    {
        return false;
    }
    let Some(listener) =
        crate::owners::owning_function(ast, solid_facts::core::Span::new(start, end))
    else {
        return false;
    };
    if listener.r#async || listener.generator {
        return false;
    }
    let standard = |call: &solid_facts::ast::CallFact| {
        lookup
            .resolved_callee_call(file, ast.peel_ts_sugar_span(call.callee))
            .and_then(crate::runtime_semantics::standard_call_name)
            .map(str::to_owned)
    };
    // The registration: `el.addEventListener("type", listener)`.
    let Some(registration) = ast.calls.iter().find(|call| {
        !call.construct
            && call.arguments.len() >= 2
            && call.arguments[..2].iter().all(|argument| !argument.spread)
            && ast.peel_ts_sugar_span(call.arguments[1].span) == listener.span
    }) else {
        return false;
    };
    // lib.dom declares the element's own overload (`HTMLInputElement.addEventListener`), as `EventTarget`'s.
    if !standard(registration).is_some_and(|name| name.ends_with(".addEventListener")) {
        return false;
    }
    let Some(event_type) = file
        .source_text(ast.peel_ts_sugar_span(registration.arguments[0].span))
        .and_then(|text| {
            text.strip_prefix('"')
                .and_then(|rest| rest.strip_suffix('"'))
                .or_else(|| {
                    text.strip_prefix('\'')
                        .and_then(|rest| rest.strip_suffix('\''))
                })
        })
        .filter(|text| text.chars().all(|c| c.is_ascii_alphanumeric()))
        .map(str::to_owned)
    else {
        return false;
    };
    let Some(member) = ast
        .members
        .iter()
        .find(|member| ast.peel_ts_sugar_span(registration.callee) == member.span)
    else {
        return false;
    };
    let receiver = ast.peel_ts_sugar_span(member.object);
    // The element: a `const` of the component a `Document.createElement` call initializes.
    let Some(binding) = ast.bindings.iter().find(|binding| {
        binding.immutable
            && binding.shape == solid_facts::ast::BindingShape::Identifier
            && binding.names.len() == 1
            && lookup
                .entities()
                .at(file.path.as_str(), binding.names[0].span)
                .is_some_and(|declared| {
                    lookup
                        .symbol_references(declared.as_str())
                        .iter()
                        .any(|reference| {
                            reference.path.as_ref() == file.path.as_str()
                                && reference.start_byte == u64::from(receiver.start)
                        })
                })
    }) else {
        return false;
    };
    let name = binding.names[0].span;
    let Some(component) = crate::owners::owning_function(ast, binding.declaration) else {
        return false;
    };
    let created = binding.initializer.is_some_and(|initializer| {
        ast.calls.iter().any(|call| {
            call.span == ast.peel_ts_sugar_span(initializer)
                && standard(call).as_deref() == Some("Document.createElement")
        })
    });
    if !created || crate::indexes::binding_written(file, name) {
        return false;
    }
    let Some(symbol) = lookup.entities().at(file.path.as_str(), name) else {
        return false;
    };
    let mut leaf: Option<solid_facts::core::Span> = None;
    let mut removed = false;
    for reference in lookup.symbol_references(symbol.as_str()) {
        if reference.path.as_ref() != file.path.as_str() {
            return false;
        }
        if reference.start_byte == u64::from(name.start) {
            continue;
        }
        let Ok(at) = u32::try_from(reference.start_byte) else {
            return false;
        };
        // The call the reference is the receiver of (`el.m(...)`), if any.
        let received = ast
            .members
            .iter()
            .find(|member| ast.peel_ts_sugar_span(member.object).start == at)
            .and_then(|member| {
                ast.calls.iter().find(|call| {
                    !call.construct && ast.peel_ts_sugar_span(call.callee) == member.span
                })
            });
        let Some(holder) =
            crate::owners::owning_function(ast, solid_facts::core::Span::new(at, at))
        else {
            return false;
        };
        if let Some(call) = received
            && let Some(callback) = owned_leaf_callback(file, call.span, settled_gates, lookup)
        {
            if leaf.is_some_and(|known| known != callback) {
                return false;
            }
            leaf = Some(callback);
            if standard(call).as_deref() == Some("ChildNode.remove")
                && ast.completion_sites.contains(&call.span)
            {
                removed = true;
            }
            continue;
        }
        if holder.span != component.span {
            return false;
        }
        if let Some(call) = received {
            let Some(name) = standard(call) else {
                return false;
            };
            let quiet = !crate::runtime_semantics::delivers_synchronously(&name)
                || crate::runtime_semantics::events_delivered_to_receiver(&name)
                    .is_some_and(|events| !events.contains(&event_type.as_str()));
            if !quiet {
                return false;
            }
            continue;
        }
        // `document.body.append(el)`: the whole argument of an attaching call, which runs no listener of a fresh element.
        let attached = ast.calls.iter().any(|call| {
            call.arguments.len() == 1
                && !call.arguments[0].spread
                && ast.peel_ts_sugar_span(call.arguments[0].span).start == at
                && matches!(
                    standard(call).as_deref(),
                    Some("ParentNode.append" | "Node.appendChild")
                )
        });
        if !attached {
            return false;
        }
    }
    // The settled callback runs after the component's body, so its leaf owner is the element's last holder.
    removed
        && leaf
            .is_some_and(|callback| component.span.contains(callback) && callback != component.span)
}

/// ADR 1111: whether `at` (a read: a row's origin, else its location) runs after a call in `refused` (a creation the
/// effect-callback guard refuses or may refuse) on a run of the function holding that call: written after it in that
/// function, or in a function that function calls by name after it. A read in the creation's own arguments is not one.
fn read_after_refused_creation(
    at: &Location,
    refused: &[Location],
    lookup: &SemanticLookup<'_>,
) -> bool {
    let Some(file) = lookup.file_by_path(at.path.as_ref()) else {
        return false;
    };
    let (Ok(start), Ok(end)) = (u32::try_from(at.start_byte), u32::try_from(at.end_byte)) else {
        return false;
    };
    let ast = &file.ast;
    let read = solid_facts::core::Span::new(start, end);
    let read_function = crate::owners::owning_function(ast, read);
    let holder_of = |span: solid_facts::core::Span| {
        crate::owners::owning_function(ast, span).map(|function| function.span)
    };
    refused.iter().any(|creation| {
        creation.path == at.path
            && ast.calls.iter().any(|call| {
                if u64::from(call.span.start) != creation.start_byte || call.span.contains(read) {
                    return false;
                }
                let Some(holder) = holder_of(call.span) else {
                    return false;
                };
                if read_function.map(|function| function.span) == Some(holder) {
                    return call.span.end <= read.start;
                }
                read_function.is_some_and(|function| {
                    crate::execution_role::function_called_by_name(file, function, lookup)
                        .into_iter()
                        .any(|run| call.span.end <= run.start && holder_of(run) == Some(holder))
                })
            })
    })
}
