use super::{Finding, RuleMetadata, strict_read_evidence, strict_read_message};
use crate::{ExecutionRole, OwnerRequirement, ReactiveRead};
use typefacts::Location;

fn location(start: u64) -> Location {
    Location {
        path: "app.tsx".into(),
        start_byte: start,
        end_byte: start + 1,
    }
}

fn read(missing_jsx_census: bool) -> ReactiveRead {
    ReactiveRead {
        summary_attributed: false,
        transported: false,
        kind: "accessor".into(),
        accessor: "count".into(),
        location: location(20),
        declaration: location(10),
        execution: ExecutionRole::UntrackedRendering,
        context: "Panel".into(),
        via: "".into(),
        origin: None,
        origin_context: "".into(),
        uncertain: false,
        missing_jsx_census,
        host_callback_timing: false,
        project_consumer_non_strict: false,
        callback_invocation_unproven: false,
        callee_callback_timing: false,
        blocker: None,
    }
}

#[test]
fn a_paired_compute_read_changes_only_the_apply_message() {
    let mut apply = read(false);
    apply.execution = ExecutionRole::EffectApply;
    apply.context = "createEffect apply callback".into();
    let evidence = strict_read_evidence(&apply);
    let message = super::strict_read_message_with_compute(&apply, true);
    assert!(message.contains("compute also reads the same source"));
    assert!(message.contains("apply read is untracked and Solid warns"));
    assert!(!message.contains("never updates"));
    assert!(strict_read_message(&apply).contains("never updates"));
    assert!(!apply.is_uncertifiable());
    assert_eq!(strict_read_evidence(&apply), evidence);
    apply.summary_attributed = true;
    assert_eq!(
        super::strict_read_message_with_compute(&apply, true),
        strict_read_message(&apply)
    );
    apply.summary_attributed = false;
    apply.missing_jsx_census = true;
    assert_eq!(
        super::strict_read_message_with_compute(&apply, true),
        strict_read_message(&apply)
    );
}

/// The census-gap wording *is* the precision claim of the escalation: a
/// finding that says "outside every compiler-tracked JSX region" asserts a
/// completed search of facts that were never collected. The two census-gap
/// fixtures pin the message through the snapshot (they are in coverage's
/// `KEEPS_WORDING` set), but nothing in the snapshot carries evidence
/// steps, so the last evidence sentence is pinned here or nowhere.
#[test]
fn a_census_gap_never_claims_a_completed_search_of_compiler_facts() {
    let overstatement = "outside every compiler-tracked JSX region";

    let ordinary = read(false);
    assert!(strict_read_message(&ordinary).contains("which does not track"));
    assert!(
        strict_read_evidence(&ordinary)
            .last()
            .unwrap()
            .message
            .contains(overstatement),
        "a read with a census entry keeps the completed-search evidence"
    );

    let gap = read(true);
    let message = strict_read_message(&gap);
    assert!(
        message.contains("execution census does not cover"),
        "the census gap must be named in the message: {message}"
    );
    assert!(
        !message.contains("which does not track") && !message.contains("never updates when"),
        "the message must not claim the read never updates: {message}"
    );
    let evidence = strict_read_evidence(&gap);
    let last = &evidence.last().unwrap().message;
    assert!(
        !last.contains(overstatement),
        "the evidence must not claim a completed search: {last}"
    );
    assert!(
        last.contains("no compiler fact places it inside or outside a tracked region"),
        "the evidence must state the missing fact: {last}"
    );
}

#[test]
fn an_optional_inline_accessor_invocation_is_not_a_proven_read() {
    let mut optional = read(false);
    optional.via = "access".into();
    optional.callback_invocation_unproven = true;
    assert!(optional.is_uncertifiable());
    let message = strict_read_message(&optional);
    assert!(message.contains("may be read through access"));
    assert!(message.contains("does not prove this call invokes the accessor"));
    assert!(!message.contains("never updates"));
    optional.origin = Some(optional.location.clone());
    let evidence = strict_read_evidence(&optional);
    assert!(evidence[1].message.contains("may read"));
    assert!(evidence[2].message.contains("possible read"));
    optional.missing_jsx_census = true;
    let message = strict_read_message(&optional);
    assert!(message.contains("tracking context is also unresolved"));
    assert!(!message.contains("outside a tracking scope"));
    optional.missing_jsx_census = false;
    assert!(
        strict_read_evidence(&optional)
            .last()
            .unwrap()
            .message
            .contains("does not guarantee an invocation")
    );
}

#[test]
fn deferred_apply_does_not_claim_an_inline_invocation_or_a_completed_read() {
    let mut deferred = read(false);
    deferred.execution = crate::ExecutionRole::EffectApply;
    deferred.callee_callback_timing = true;
    assert!(deferred.is_uncertifiable());
    let message = strict_read_message(&deferred);
    assert!(message.contains("options and compute dependencies"));
    assert!(!message.contains("states an inline invocation") && !message.contains("never updates"));
    let evidence = strict_read_evidence(&deferred);
    assert!(
        evidence
            .last()
            .unwrap()
            .message
            .contains("invocation is not proven")
    );
}

/// A read in a host-retained callback names the hole it rests on: the
/// host may run the callback inside the strict-read window or after it,
/// so neither "does not track" nor a completed search is claimed.
#[test]
fn a_host_callback_read_never_claims_the_window_either_way() {
    let mut host = read(false);
    host.host_callback_timing = true;
    assert!(host.is_uncertifiable());
    let message = strict_read_message(&host);
    assert!(
        message.contains("inside the component body's strict-read window, or after it"),
        "the host-timing hole must be named in the message: {message}"
    );
    assert!(
        !message.contains("which does not track") && !message.contains("never updates when"),
        "the message must not claim the read never updates: {message}"
    );
    let evidence = strict_read_evidence(&host);
    let last = &evidence.last().unwrap().message;
    assert!(
        !last.contains("outside every compiler-tracked JSX region"),
        "the evidence must not claim a completed search: {last}"
    );
    assert!(
        last.contains("no fact places its execution inside or after"),
        "the evidence must state the missing fact: {last}"
    );
}

/// A read in a literal handed to a project function that is not proven to
/// invoke it names that hole, not the host one, and claims neither
/// "does not track" nor a completed search.
#[test]
fn a_callee_callback_read_never_claims_the_window_either_way() {
    let mut callee = read(false);
    callee.callee_callback_timing = true;
    assert!(callee.is_uncertifiable());
    let message = strict_read_message(&callee);
    assert!(
        message.contains("not proven to invoke it during the call"),
        "the callee-timing hole must be named in the message: {message}"
    );
    assert!(
        !message.contains("host API")
            && !message.contains("which does not track")
            && !message.contains("never updates when"),
        "the message must not claim the read never updates: {message}"
    );
    let evidence = strict_read_evidence(&callee);
    let last = &evidence.last().unwrap().message;
    assert!(
        !last.contains("outside every compiler-tracked JSX region")
            && last.contains("not proven to invoke it during the call"),
        "the evidence must state the missing fact: {last}"
    );
}

/// The interprocedural arm reaches the same sentence through a different
/// subject, so it gets its own assertion rather than riding on the direct
/// one.
#[test]
fn the_census_gap_evidence_covers_the_propagated_read_too() {
    let mut gap = read(true);
    gap.via = "title".into();
    gap.origin = Some(location(4));
    let evidence = strict_read_evidence(&gap);
    let last = &evidence.last().unwrap().message;
    assert!(
        last.starts_with(
            "the call sits inside a JSX expression the compiler's execution census does not cover"
        ),
        "unexpected evidence for the propagated read: {last}"
    );
    assert!(strict_read_message(&gap).contains("read through title"));
}

/// A missing-census owner requirement must not claim a completed search for
/// a live, unowned operation.
#[test]
fn a_census_gap_makes_an_owner_requirement_uncertifiable_not_a_violation() {
    let metadata = RuleMetadata {
        code: "SC4001",
        name: "missing-owner",
        severity: "error",
        uncertifiable: false,
        default_enabled: true,
        presets: &[],
    };
    let requirement = || OwnerRequirement {
        operation: crate::OwnerRequirementOperation::Cleanup,
        location: location(20),
        uncertain: false,
        runtime_uncertain: false,
        caller_uncertain: false,
        conditional_owner: false,
        later_run_unowned: false,
        component_uncertain: false,
        after_await: false,
        event_path: None,
        missing_jsx_census: false,
        through_dependency: None,
        report: true,
    };

    let ordinary = Finding::for_owner_requirement(
        metadata,
        &requirement(),
        "onCleanup is called without a reactive owner",
        "Register it under a component or root.",
    );
    assert_eq!(ordinary.kind, "violation");
    assert!(
        ordinary.evidence[0]
            .message
            .contains("no containing component, computation, or root owner dominates"),
        "the ordinary requirement keeps its completed-search evidence: {:?}",
        ordinary.evidence[0].message
    );

    let mut missing = requirement();
    missing.uncertain = true;
    missing.missing_jsx_census = true;
    let missing = Finding::for_owner_requirement(
        metadata,
        &missing,
        "onCleanup is called without a reactive owner",
        "Register it under a component or root.",
    );
    assert_eq!(missing.kind, "uncertifiable");
    assert!(missing.message.contains("execution census does not cover"));
    assert!(
        !missing.evidence[0]
            .message
            .contains("no containing component, computation, or root owner dominates")
    );

    // A later run of a first-run-owned callback names its own reason, not
    // a nullable `runWithOwner` owner the source never wrote.
    let mut later = requirement();
    later.uncertain = true;
    later.later_run_unowned = true;
    let later = Finding::for_owner_requirement(
        metadata,
        &later,
        "onCleanup is called without a reactive owner",
        "Register it under a component or root.",
    );
    assert_eq!(later.kind, "uncertifiable");
    assert!(later.message.contains("owned only on its first run"));
    assert!(later.evidence[0].message.contains("only on its first run"));
    assert!(!later.message.contains("runWithOwner"));
    assert!(!later.message.contains("is exported"));
}

/// ADR 0363: a violation of a mixed-invocation callback's event run names the
/// run, and never says the owner is unproven.
#[test]
fn a_mixed_invocation_violation_names_its_event_run() {
    let metadata = RuleMetadata {
        code: "SC4001",
        name: "missing-owner",
        severity: "error",
        uncertifiable: false,
        default_enabled: true,
        presets: &[],
    };
    let requirement = OwnerRequirement {
        operation: crate::OwnerRequirementOperation::Cleanup,
        location: location(20),
        uncertain: false,
        runtime_uncertain: false,
        caller_uncertain: false,
        conditional_owner: false,
        later_run_unowned: false,
        component_uncertain: false,
        after_await: false,
        event_path: Some(
            crate::execution_role::EventEdge {
                attribute: "onClick".into(),
                element: "button".into(),
            }
            .describe(),
        ),
        missing_jsx_census: false,
        through_dependency: None,
        report: true,
    };
    let finding = Finding::for_owner_requirement(
        metadata,
        &requirement,
        "onCleanup is called without a reactive owner",
        "Register it under a component or root.",
    );
    assert_eq!(finding.kind, "violation");
    assert!(
        finding
            .message
            .contains("when run as the `onClick` handler of `<button>`"),
        "{}",
        finding.message
    );
    assert!(
        finding.evidence[0]
            .message
            .contains("the `onClick` handler of `<button>`")
    );
    assert!(
        crate::execution_role::EventEdge::default()
            .describe()
            .contains("an event handler")
    );
}
