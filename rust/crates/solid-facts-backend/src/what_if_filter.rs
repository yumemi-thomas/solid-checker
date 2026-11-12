//! ADR 0965: what a what-if run does with a typed cause it assumed away.
//!
//! `--assume Premise(<kind>)` asks the seam ([`solid_reactive_ir`]'s `hazard_present`) once the rows are final, which records the kinds the run assumed in
//! `Program::assumed_premises`. This module is the one place the what-if build takes findings out of the report for them: every uncertifiable finding whose
//! blocker kind (`blockedBy.kind`, the head of its `analysisContext`) was assumed away. It is compiled only with the cargo feature `what-if`, so a default
//! build has no such drop, and the report it feeds is tainted (a top-level `assumed` and an `assumed` premise on every finding that stays), which every
//! consumer of a checker report refuses (`scripts/lib/assumed.mjs`). A finding of another kind, and a violation, is never touched.

use std::collections::HashMap;

use solid_reactive_ir::{Finding, LEAF_OWNER_UNPROVEN, Program, blocked_by::blocked_kind};

/// Takes out of `findings` every uncertifiable finding that a cause `program` assumed away blocks.
///
/// A leaf operation's `leaf-owner-unproven` row is owed for one or more reasons the program's leaf operations at its site name (ADR 0965, slice 2:
/// one of the two stops of the slot climb, each a kind of its own, or `leaf-owner-unproven` for any other reason): it leaves when every reason it is
/// owed for was assumed, and stands while one stands. Every other finding leaves when its blocker kind was assumed.
pub(crate) fn drop_assumed_causes(program: &Program, findings: &mut Vec<Finding>) {
    if program.assumed_premises.is_empty() {
        return;
    }
    let assumed_kind = |kind: &str| {
        program
            .assumed_premises
            .iter()
            .any(|premise| premise == kind)
    };
    let mut owed = HashMap::new();
    for operation in &program.leaf_operations {
        owed.entry(&operation.location)
            .or_insert_with(Vec::new)
            .push(operation.what_if_cause());
    }
    let assumed = |finding: &Finding| {
        if finding.kind != "uncertifiable" {
            return false;
        }
        let Some(kind) = blocked_kind(&finding.analysis_context) else {
            return false;
        };
        if kind == LEAF_OWNER_UNPROVEN
            && let Some(causes) = owed.get(&finding.primary_location)
        {
            return causes.iter().all(|cause| assumed_kind(cause));
        }
        assumed_kind(kind)
    };
    findings.retain(|finding| !assumed(finding));
}

#[cfg(test)]
mod tests {
    use solid_reactive_ir::{LeafOwnerOperation, LeafOwnerOperationKind, LeafStop};
    use typefacts::Location;

    use super::*;

    fn location(start: u64) -> Location {
        Location {
            path: "/p/App.tsx".into(),
            start_byte: start,
            end_byte: start + 5,
        }
    }

    fn operation(start: u64, stop: Option<LeafStop>) -> LeafOwnerOperation {
        LeafOwnerOperation {
            kind: LeafOwnerOperationKind::UnmodelledSlot,
            owner: "createTrackedEffect".into(),
            location: location(start),
            fix: None,
            call_site_gate: None,
            uncertain: true,
            possible: true,
            via: None,
            identity_unproven: false,
            sites: Vec::new(),
            computation_error_unobserved: false,
            stop,
        }
    }

    fn row(start: u64, blocker: &str) -> Finding {
        Finding {
            id: "SC3001".into(),
            rule: "leaf-owner-forbidden-call".into(),
            kind: "uncertifiable".into(),
            message: "message".into(),
            hint: String::new(),
            severity: "warning".into(),
            analysis_context: blocker.into(),
            subject_kind: String::new(),
            coverage_family: String::new(),
            coverage_subject: String::new(),
            primary_location: location(start),
            related_locations: Vec::new(),
            evidence: Vec::new(),
            fixes: Vec::new(),
        }
    }

    /// The starts of the findings that stay when `assumed` is assumed away, over `operations` and one row per start.
    fn left(operations: Vec<LeafOwnerOperation>, starts: &[u64], assumed: &[&str]) -> Vec<u64> {
        let program = Program {
            leaf_operations: operations,
            assumed_premises: assumed.iter().map(|kind| (*kind).to_owned()).collect(),
            ..Program::default()
        };
        let mut findings: Vec<Finding> = starts
            .iter()
            .map(|start| row(*start, LEAF_OWNER_UNPROVEN))
            .collect();
        drop_assumed_causes(&program, &mut findings);
        findings
            .iter()
            .map(|finding| finding.primary_location.start_byte)
            .collect()
    }

    // ADR 0965, slice 2: a stop is a premise of its own; the other reasons a leaf operation is owed are `leaf-owner-unproven`.
    #[test]
    fn what_if_a_stop_lifts_its_own_rows_and_leaf_owner_unproven_the_rest() {
        let operations = || {
            vec![
                operation(10, Some(LeafStop::UnmodelledInPlace)),
                operation(20, Some(LeafStop::Bound)),
                operation(30, None),
            ]
        };
        let rows = [10, 20, 30];
        assert_eq!(left(operations(), &rows, &[]), rows);
        assert_eq!(
            left(
                operations(),
                &rows,
                &["leaf-owner-climb-unmodelled-in-place"]
            ),
            [20, 30]
        );
        assert_eq!(
            left(operations(), &rows, &["leaf-owner-climb-bound"]),
            [10, 30]
        );
        assert_eq!(
            left(operations(), &rows, &[LEAF_OWNER_UNPROVEN]),
            [10, 20],
            "the rest lifts no stop's row"
        );
        assert_eq!(
            left(
                operations(),
                &rows,
                &[
                    LEAF_OWNER_UNPROVEN,
                    "leaf-owner-climb-unmodelled-in-place",
                    "leaf-owner-climb-bound"
                ]
            ),
            Vec::<u64>::new()
        );
    }

    // A row owed for several reasons (two operations at one site) stands while one of them does.
    #[test]
    fn what_if_a_row_owed_for_several_reasons_stands_while_one_stands() {
        let operations = || {
            vec![
                operation(10, Some(LeafStop::UnmodelledInPlace)),
                operation(10, None),
            ]
        };
        assert_eq!(
            left(
                operations(),
                &[10],
                &["leaf-owner-climb-unmodelled-in-place"]
            ),
            [10]
        );
        assert_eq!(left(operations(), &[10], &[LEAF_OWNER_UNPROVEN]), [10]);
        assert_eq!(
            left(
                operations(),
                &[10],
                &[LEAF_OWNER_UNPROVEN, "leaf-owner-climb-unmodelled-in-place"]
            ),
            Vec::<u64>::new()
        );
    }

    // Another kind, a site no leaf operation holds, a violation and a row of another kind are the slice-1 rule's: the kind assumed.
    #[test]
    fn what_if_every_other_finding_leaves_by_its_blocker_kind() {
        let program = Program {
            leaf_operations: vec![operation(10, Some(LeafStop::Bound))],
            assumed_premises: vec![
                LEAF_OWNER_UNPROVEN.to_owned(),
                "leaf-callback-opaque".to_owned(),
            ],
            ..Program::default()
        };
        let mut violation = row(30, LEAF_OWNER_UNPROVEN);
        violation.kind = "violation".into();
        let mut findings = vec![
            row(10, LEAF_OWNER_UNPROVEN),
            row(20, LEAF_OWNER_UNPROVEN),
            violation,
            row(10, "leaf-callback-opaque"),
            row(40, "function-run-unplaced"),
        ];
        drop_assumed_causes(&program, &mut findings);
        let kept: Vec<(u64, &str)> = findings
            .iter()
            .map(|finding| {
                (
                    finding.primary_location.start_byte,
                    blocked_kind(&finding.analysis_context).unwrap_or(""),
                )
            })
            .collect();
        assert_eq!(
            kept,
            [
                (10, LEAF_OWNER_UNPROVEN),
                (30, LEAF_OWNER_UNPROVEN),
                (40, "function-run-unplaced")
            ],
            "the stop's row stands under `leaf-owner-unproven` alone; a row no operation owes leaves by its kind; the callback row of the same site leaves by its own kind"
        );
    }
}
