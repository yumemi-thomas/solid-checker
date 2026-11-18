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
            }
            OwnerRequirementOperation::ContextRead => false,
            operation => {
                (operation == OwnerRequirementOperation::Effect
                    && (effect_may_guard
                        .iter()
                        .any(|guarded| same_call(guarded, &requirement.location))
                        || in_slot_called_only_in_applies(&requirement.location, true, lookup)))
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
            // ADR 1052 part 8: a render effect's apply is no such run here, since its first run is owned and reaches the probe.
            || in_slot_called_only_in_applies(&violation.location, false, lookup)
            || prop_runs_only_in_owned_leaf(&violation.location, settled_gates, lookup)
            || refused_at_mount_else_handlers(&violation.location, settled_gates, lookup)
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

/// ADR 1052 part 6: `site` is written directly in the cleanup an effect's apply returns, and the effect may rerun
/// (`directly_in_rerun_effect_cleanup`, ADR 0539's `WindowedCleanup::MayRerun`): a rerun calls the cleanup inside the effect
/// callback, where a creation meets the guard first, and nothing orders that rerun against the disposal that would run it
/// outside. An effect whose compute provably tracks nothing never reruns and is not this.
fn in_rerun_cleanup(site: &Location, lookup: &SemanticLookup<'_>) -> bool {
    let Some(file) = lookup.file_by_path(site.path.as_ref()) else {
        return false;
    };
    let (Ok(start), Ok(end)) = (u32::try_from(site.start_byte), u32::try_from(site.end_byte))
    else {
        return false;
    };
    crate::execution_role::directly_in_rerun_effect_cleanup(
        file,
        solid_facts::core::Span::new(start, end),
        lookup,
    )
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
