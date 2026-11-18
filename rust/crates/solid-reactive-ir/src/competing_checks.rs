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
pub(crate) fn mark(program: &mut Program, lookup: &SemanticLookup<'_>) {
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
                    && effect_may_guard
                        .iter()
                        .any(|guarded| same_call(guarded, &requirement.location)))
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
