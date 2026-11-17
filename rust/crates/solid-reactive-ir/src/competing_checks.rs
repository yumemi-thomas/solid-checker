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
//!   (`NO_OWNER_EFFECT` and the other owner codes, SC4001) is reached; a leaf owner's caller throws and halts the
//!   system, so a caller of the same helper that runs later never reports (the order is not proven: an obligation);
//! - `createEffect` with no callable effect function throws `MISSING_EFFECT_FN` (SC7001) before the leaf owner's
//!   refusal (SC3001);
//! - `useContext` throws `NO_OWNER_CONTEXT` (SC4001's context read) before it looks for a provider
//!   (`CONTEXT_NOT_FOUND`, SC4002).
//!
//! Only a proven earlier check counts (an obligation of it proves no throw), only rows that would be violations are touched, and a call is matched by its path and first byte (a call and its
//! callee start at the same byte; a call nested in another's arguments does not).

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
pub(crate) fn mark(program: &mut Program) {
    // Calls an earlier check is proven to refuse, and so never reach the owner check (an obligation of either proves no
    // throw: where it does not throw, the owner check is reached, and Chrome reports NO_OWNER_* at such calls):
    // - the effect-callback guard (SC6002's violations) throws when an effect is created, before `createEffect` reports
    //   `NO_OWNER_EFFECT` at its end; a boundary reports `NO_OWNER_BOUNDARY` before it creates anything, so the guard is
    //   earlier for an effect only;
    // - the leaf owner's refusal (SC3001's violations, at the operation's own call) throws on the leaf caller's run and
    //   halts the reactive system, so an unowned caller that runs later never reports; which runs first is not proven
    //   in general, so the owner row is an obligation either way: every operation.
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
            OwnerRequirementOperation::ContextProvider => unowned_reads
                .iter()
                .any(|read| same_call(read, &requirement.location)),
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
    for violation in &mut program.static_violations {
        if violation.id != "SC7002" || violation.uncertain {
            continue;
        }
        if creation_may_throw
            .iter()
            .any(|refused| same_call(refused, &violation.location))
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
