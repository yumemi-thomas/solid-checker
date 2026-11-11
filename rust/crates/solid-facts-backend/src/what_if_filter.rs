//! ADR 0965: what a what-if run does with a typed cause it assumed away.
//!
//! `--assume Premise(<kind>)` asks the seam ([`solid_reactive_ir`]'s `hazard_present`) once the rows are final, which records the kinds the run assumed in
//! `Program::assumed_premises`. This module is the one place the what-if build takes findings out of the report for them: every uncertifiable finding whose
//! blocker kind (`blockedBy.kind`, the head of its `analysisContext`) was assumed away. It is compiled only with the cargo feature `what-if`, so a default
//! build has no such drop, and the report it feeds is tainted (a top-level `assumed` and an `assumed` premise on every finding that stays), which every
//! consumer of a checker report refuses (`scripts/lib/assumed.mjs`). A finding of another kind, and a violation, is never touched.

use solid_reactive_ir::{Finding, Program, blocked_by::blocked_kind};

/// Takes out of `findings` every uncertifiable finding that a cause `program` assumed away blocks.
pub(crate) fn drop_assumed_causes(program: &Program, findings: &mut Vec<Finding>) {
    if program.assumed_premises.is_empty() {
        return;
    }
    let assumed = |finding: &Finding| {
        finding.kind == "uncertifiable"
            && blocked_kind(&finding.analysis_context).is_some_and(|kind| {
                program
                    .assumed_premises
                    .iter()
                    .any(|premise| premise == kind)
            })
    };
    findings.retain(|finding| !assumed(finding));
}
