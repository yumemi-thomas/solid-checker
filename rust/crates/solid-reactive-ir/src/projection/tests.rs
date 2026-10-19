use std::sync::Arc;

use super::*;
use crate::{ActionInvocation, AsyncRead, ExecutionRole};

#[test]
fn a_package_owned_read_never_promises_a_runtime_warning() {
    let read = crate::ReactiveRead {
        package_internal: true,
        summary_attributed: false,
        transported: false,
        kind: "accessor".into(),
        accessor: "package.read".into(),
        location: location(20),
        declaration: location(10),
        execution: ExecutionRole::UntrackedRendering,
        context: "Panel".into(),
        via: "package.read".into(),
        origin: None,
        origin_context: "".into(),
        uncertain: false,
        missing_jsx_census: false,
        host_callback_timing: false,
        project_consumer_non_strict: false,
        callback_invocation_unproven: false,
        callee_callback_timing: false,
    };
    let finding = project_finding(
        FindingSeed::StrictRead {
            read: &read,
            compute_reads_source: false,
        },
        &RecordingCatalog(CatalogCapabilities::SOLID_2),
    );
    assert_eq!(finding.kind, "uncertifiable");
    assert!(
        finding
            .message
            .contains("whether Solid emits STRICT_READ_UNTRACKED remains uncertifiable")
    );
    assert!(!finding.message.contains("for every use"));
    assert!(!finding.message.contains("outside any tracking scope"));
}

struct RecordingCatalog(CatalogCapabilities);

impl CatalogWording for RecordingCatalog {
    fn capabilities(&self) -> CatalogCapabilities {
        self.0
    }

    fn wording(&self, seed: FindingSeed<'_>) -> FindingWording {
        let message = match seed {
            FindingSeed::StrictRead { .. } => "read",
            FindingSeed::OwnedWrite(_) => "write",
            FindingSeed::Action(_) => "action",
            FindingSeed::LeafOperation(_) => "leaf",
            FindingSeed::StaticViolation(_) => "static-violation",
            FindingSeed::StaticDefect(_) => "static-defect",
            FindingSeed::DirectiveCreation(_) => "directive",
            FindingSeed::OwnerRequirement(_) => "owner",
            FindingSeed::AsyncRead(_) => "async",
        };
        FindingWording::new(
            RuleMetadata {
                code: "TEST00",
                name: "test",
                severity: "warning",
                uncertifiable: false,
                default_enabled: true,
                presets: &[],
            },
            message,
            "",
        )
    }
}

fn location(index: u64) -> Location {
    Location {
        path: "projection.tsx".into(),
        start_byte: index,
        end_byte: index + 1,
    }
}

fn finding(code: &'static str, start: u64, end: u64) -> Finding {
    Finding::new(
        RuleMetadata {
            code,
            name: "test",
            severity: "warning",
            uncertifiable: false,
            default_enabled: true,
            presets: &[],
        },
        "test".into(),
        Location {
            path: "projection.tsx".into(),
            start_byte: start,
            end_byte: end,
        },
    )
}

/// A catalog whose every rule is an uncertifiable error, as SC9005 is.
struct ErrorCatalog;

impl CatalogWording for ErrorCatalog {
    fn capabilities(&self) -> CatalogCapabilities {
        CatalogCapabilities::SOLID_2
    }

    fn wording(&self, _seed: FindingSeed<'_>) -> FindingWording {
        FindingWording::new(
            RuleMetadata {
                code: "SC9005",
                name: "package-contract-incomplete",
                severity: "error",
                uncertifiable: true,
                default_enabled: true,
                presets: &[],
            },
            "contract",
            "",
        )
    }
}

fn contract_defect(context: &str, site: crate::ContractDefectSite, at: u64) -> StaticDefect {
    StaticDefect {
        kind: StaticDefectKind::PackageContractExportMissing {
            module: "pkg".into(),
            export: "access".into(),
            reexported: false,
            site,
            admission_refusal: None,
        },
        location: location(at),
        analysis_context: context.into(),
        fixes: vec![],
        uncertain: false,
    }
}

#[test]
fn a_coverage_subject_names_the_package_not_the_subpath() {
    assert_eq!(
        super::package_of_specifier("@scope/name/sub/path"),
        "@scope/name"
    );
    assert_eq!(super::package_of_specifier("@scope/name"), "@scope/name");
    assert_eq!(super::package_of_specifier("name/sub"), "name");
    assert_eq!(super::package_of_specifier("name"), "name");
}

#[test]
fn a_refused_catalog_entry_adds_one_evidence_step_to_the_acceptance_gate() {
    let note = "a project catalog entry exists for this package and was not admitted: \
                    its receipt states no dependency environment, so it is admitted nowhere";
    let refused = |at: u64| {
        let mut defect = contract_defect(
            crate::contracts::UNACCEPTED_IMPORT_CONTEXT,
            crate::ContractDefectSite::Import,
            at,
        );
        let StaticDefectKind::PackageContractExportMissing {
            admission_refusal, ..
        } = &mut defect.kind
        else {
            unreachable!()
        };
        *admission_refusal = Some(note.into());
        defect
    };
    let plain = contract_defect(
        crate::contracts::UNACCEPTED_IMPORT_CONTEXT,
        crate::ContractDefectSite::Import,
        1,
    );
    let without = project_finding(FindingSeed::StaticDefect(&plain), &ErrorCatalog);
    let with = project_finding(FindingSeed::StaticDefect(&refused(1)), &ErrorCatalog);
    assert_eq!(
        with.message, without.message,
        "the message is the gate's own"
    );
    assert_eq!(with.severity, without.severity);
    assert_eq!(
        with.evidence[..without.evidence.len()],
        without.evidence[..]
    );
    assert_eq!(
        with.evidence[without.evidence.len()..]
            .iter()
            .map(|step| step.message.as_str())
            .collect::<Vec<_>>(),
        [note]
    );
    // Collapsed over a package's import sites, the note is stated once.
    let collapsed = collapse_unaccepted_contract_defects(&[refused(1), refused(2)], &ErrorCatalog);
    assert_eq!(collapsed.len(), 1);
    assert_eq!(
        collapsed[0]
            .evidence
            .iter()
            .filter(|step| step.message == note)
            .count(),
        1
    );
    assert_eq!(collapsed[0].related_locations.len(), 1);
}

/// Kobalte defect 7: open claims at call arguments were one warning per
/// argument. Two exports, three call sites each, spread over two files and
/// supplied out of order, become one warning for the package (ADR 0224),
/// anchored at the first site in `(path, start)` order, with the other
/// five in `related_locations`, the exports and the count in the message.
/// An unbound claim at the same arguments keeps its own finding per call.
#[test]
fn open_claims_at_call_arguments_collapse_per_package_across_files() {
    let at = |export: &str, path: &str, start: u64, context: &str| StaticDefect {
        kind: StaticDefectKind::PackageContractExportMissing {
            module: "pkg".into(),
            export: export.into(),
            reexported: false,
            site: crate::ContractDefectSite::Argument,
            admission_refusal: None,
        },
        location: Location {
            path: path.into(),
            start_byte: start,
            end_byte: start + 1,
        },
        analysis_context: context.into(),
        fixes: vec![],
        uncertain: false,
    };
    let open = "unknown-contract-claims:callbacks";
    let defects = vec![
        at("second", "b.tsx", 30, open),
        at("first", "b.tsx", 20, open),
        at("first", "a.tsx", 40, open),
        at("second", "a.tsx", 5, open),
        at("first", "a.tsx", 10, open),
        at("second", "b.tsx", 7, open),
        // The same argument twice is one site.
        at("second", "b.tsx", 7, open),
        at(
            "first",
            "a.tsx",
            10,
            "unbound-contract-claims:callback arguments",
        ),
        at(
            "first",
            "b.tsx",
            20,
            "unbound-contract-claims:callback arguments",
        ),
    ];
    let sites = |finding: &Finding| {
        std::iter::once(&finding.primary_location)
            .chain(&finding.related_locations)
            .map(|location| (location.path.to_string(), location.start_byte))
            .collect::<Vec<_>>()
    };
    let site = |path: &str, start: u64| (path.to_owned(), start);
    // Deterministic: every permutation of the input gives the same output.
    let mut reversed = defects.clone();
    reversed.reverse();
    let collapsed = collapse_unaccepted_contract_defects(&defects, &ErrorCatalog);
    let (mut forward, _) = finish_findings(collapsed, Instant::now(), Instant::now());
    let (backward, _) = finish_findings(
        collapse_unaccepted_contract_defects(&reversed, &ErrorCatalog),
        Instant::now(),
        Instant::now(),
    );
    assert_eq!(forward, backward);

    let (open_findings, unbound): (Vec<_>, Vec<_>) = forward
        .drain(..)
        .partition(|finding| finding.analysis_context == open);
    assert_eq!(
        open_findings.len(),
        1,
        "one finding per package and open domains, whatever the export"
    );
    let group = &open_findings[0];
    assert_eq!(
        sites(group),
        [
            site("a.tsx", 5),
            site("a.tsx", 10),
            site("a.tsx", 40),
            site("b.tsx", 7),
            site("b.tsx", 20),
            site("b.tsx", 30)
        ]
    );
    assert_eq!(
        group.message,
        "the reactivity contract for pkg leaves callbacks unknown for 2 called exports: \
             first, second; code whose proof depends on those claims cannot be certified (6 call \
             sites)"
    );
    assert_eq!(group.severity, "warning");
    assert_eq!(group.kind, "uncertifiable");
    assert_eq!(group.subject_kind, "package");
    // An unbound claim is about its call: one finding per site, unchanged.
    assert_eq!(
        unbound.iter().map(sites).collect::<Vec<_>>(),
        [vec![site("a.tsx", 10)], vec![site("b.tsx", 20)]]
    );
    assert!(unbound.iter().all(|finding| finding.message == "contract"
        && finding.severity == "error"
        && finding.subject_kind.is_empty()));

    // A group of one keeps its exact wording.
    let alone =
        collapse_unaccepted_contract_defects(&[at("first", "a.tsx", 10, open)], &ErrorCatalog);
    assert_eq!(alone[0].message, "contract");
    assert!(alone[0].related_locations.is_empty());
}

#[test]
fn only_the_open_claims_gate_lowers_package_contract_severity_to_warning() {
    // ADR 0119: an accepted contract's open claims warn; every other gate
    // keeps the rule's error, and every one stays uncertifiable.
    let cases = [
        (
            "unknown-contract-claims:returns",
            crate::ContractDefectSite::Import,
            "warning",
        ),
        (
            "unknown-contract-claims:callbacks",
            crate::ContractDefectSite::Argument,
            "warning",
        ),
        (
            crate::contracts::UNACCEPTED_IMPORT_CONTEXT,
            crate::ContractDefectSite::Import,
            "error",
        ),
        (
            "obsolete-policy1-receipt: policy 1 cannot authorize analyzer semantics",
            crate::ContractDefectSite::Import,
            "error",
        ),
        (
            "unbound-contract-claims:callback arguments",
            crate::ContractDefectSite::Argument,
            "error",
        ),
        ("", crate::ContractDefectSite::Import, "error"),
    ];
    for (index, (context, site, severity)) in cases.into_iter().enumerate() {
        let defect = contract_defect(context, site, index as u64);
        let finding = project_finding(FindingSeed::StaticDefect(&defect), &ErrorCatalog);
        assert_eq!(finding.severity, severity, "{context:?}");
        assert_eq!(finding.kind, "uncertifiable", "{context:?}");
    }
    // A claim group collapsed over its import sites keeps the lowered
    // severity, and the acceptance gate collapsed over a package keeps
    // the error.
    let defects = vec![
        contract_defect(
            "unknown-contract-claims:returns",
            crate::ContractDefectSite::Import,
            1,
        ),
        contract_defect(
            "unknown-contract-claims:returns",
            crate::ContractDefectSite::Import,
            2,
        ),
        contract_defect(
            crate::contracts::UNACCEPTED_IMPORT_CONTEXT,
            crate::ContractDefectSite::Import,
            3,
        ),
        contract_defect(
            crate::contracts::UNACCEPTED_IMPORT_CONTEXT,
            crate::ContractDefectSite::Import,
            4,
        ),
    ];
    let collapsed = collapse_unaccepted_contract_defects(&defects, &ErrorCatalog);
    let severities = collapsed
        .iter()
        .map(|finding| finding.severity.as_str())
        .collect::<Vec<_>>();
    assert_eq!(severities, ["warning", "error"]);
}

#[test]
fn enabled_specific_defects_own_their_strict_reads() {
    let mut findings = vec![
        finding("SC1001", 10, 11),
        finding("SC5001", 10, 11),
        finding("SC1001", 21, 22),
        finding("SC1004", 20, 25),
        finding("SC1001", 27, 28),
        finding("SC1007", 27, 28),
        finding("SC1001", 30, 31),
    ];

    suppress_findings_owned_by_enabled_rules(&mut findings, CatalogCapabilities::SOLID_2);

    assert_eq!(
        findings
            .iter()
            .map(|finding| (finding.id.as_str(), finding.primary_location.start_byte))
            .collect::<Vec<_>>(),
        [
            ("SC5001", 10),
            ("SC1004", 20),
            ("SC1007", 27),
            ("SC1001", 30),
        ]
    );
}

#[test]
fn solid_one_keeps_the_handler_rule_split() {
    let mut findings = vec![finding("SC1001", 10, 11), finding("SC1007", 10, 11)];

    suppress_findings_owned_by_enabled_rules(&mut findings, CatalogCapabilities::SOLID_1);

    assert_eq!(findings.len(), 2);
}

#[test]
fn catalog_capabilities_gate_whole_program_tables() {
    let program = Program {
        actions: vec![ActionInvocation {
            action: "save".into(),
            location: location(1),
            declaration: location(2),
            execution: ExecutionRole::TrackedJsx,
            context: "App".into(),
        }],
        async_reads: vec![AsyncRead {
            accessor: Arc::from("user()"),
            location: location(3),
            declaration: location(4),
            execution: ExecutionRole::TrackedJsx,
            leaf_owner: None,
            under_loading: false,
            async_provenance: true,
            declared_loading: false,
            options_opaque: false,
            ssr_client_hole: false,
            server_rendering_unresolved: false,
            host_callback_timing: false,
            callee_callback_timing: false,
            invocation_context_unproven: false,
            mount_unresolved: false,
        }],
        ..Program::default()
    };

    let (solid_one, _) =
        project_findings(&program, &RecordingCatalog(CatalogCapabilities::SOLID_1));
    assert!(solid_one.is_empty());

    let (solid_two, _) =
        project_findings(&program, &RecordingCatalog(CatalogCapabilities::SOLID_2));
    assert_eq!(
        solid_two
            .iter()
            .map(|finding| finding.message.as_str())
            .collect::<Vec<_>>(),
        ["action", "async"]
    );
}

/// A deleted async read is not an async read. The leaf-owner clause
/// short-circuits ahead of every role test, so before the discarded-role
/// exclusion was ordered in front of it a leaf-owned read inside a value
/// the compiler removed would have been reported as a pending read that
/// throws at runtime -- about code neither compiler emits.
#[test]
fn a_discarded_async_read_is_excluded_ahead_of_the_leaf_owner_short_circuit() {
    let program = Program {
        async_reads: vec![AsyncRead {
            accessor: Arc::from("user()"),
            location: location(3),
            declaration: location(4),
            execution: ExecutionRole::DiscardedRendering,
            leaf_owner: Some(Arc::from("onSettled")),
            under_loading: false,
            async_provenance: true,
            declared_loading: false,
            options_opaque: false,
            ssr_client_hole: false,
            server_rendering_unresolved: false,
            host_callback_timing: false,
            callee_callback_timing: false,
            invocation_context_unproven: false,
            mount_unresolved: false,
        }],
        ..Program::default()
    };
    let (findings, _) = project_findings(&program, &RecordingCatalog(CatalogCapabilities::SOLID_2));
    assert!(findings.is_empty());

    // The control: the same row with a live role is still reported, so the
    // exclusion cannot pass by disabling the clause.
    let program = Program {
        async_reads: vec![AsyncRead {
            execution: ExecutionRole::UntrackedRendering,
            ..program.async_reads.into_iter().next().unwrap()
        }],
        ..Program::default()
    };
    let (findings, _) = project_findings(&program, &RecordingCatalog(CatalogCapabilities::SOLID_2));
    assert_eq!(
        findings
            .iter()
            .map(|finding| finding.message.as_str())
            .collect::<Vec<_>>(),
        ["async"]
    );
}

#[test]
fn selection_filters_safe_rows_before_the_catalog_seam() {
    let program = Program {
        actions: vec![ActionInvocation {
            action: "save".into(),
            location: location(1),
            declaration: location(2),
            execution: ExecutionRole::EventCallback,
            context: "handler".into(),
        }],
        async_reads: vec![AsyncRead {
            accessor: Arc::from("user()"),
            location: location(3),
            declaration: location(4),
            execution: ExecutionRole::TrackedJsx,
            leaf_owner: None,
            under_loading: true,
            async_provenance: true,
            declared_loading: false,
            options_opaque: false,
            ssr_client_hole: false,
            server_rendering_unresolved: false,
            host_callback_timing: false,
            callee_callback_timing: false,
            invocation_context_unproven: false,
            mount_unresolved: false,
        }],
        ..Program::default()
    };
    let (findings, _) = project_findings(&program, &RecordingCatalog(CatalogCapabilities::SOLID_2));
    assert!(findings.is_empty());
}

fn evidence_terms() -> StaticDefectTerms {
    StaticDefectTerms {
        props_destructure_hint: "",
        reactive_object_destructure_hint: "",
        missing_effect_message: "",
        missing_effect_hint: "",
        store_mutation_hint: |_| String::new(),
        removed_export_hint: |_, _| None,
        rich_argument_transport_throw: "",
        rich_argument_resolved_hint: "",
        rich_argument_nested_hint: "",
        rich_argument_primitive_hint: "",
        rich_argument_unresolved_hint: "",
    }
}

/// The source-argument evidence states what the facts establish and no
/// more: the accessor sits in the call's argument position, and tracking
/// is unproven if the call executes. It claims no invocation, no invalid
/// API and no untracked read, so it holds unchanged for a call TypeScript
/// accepts. The other kinds that shared its sentence keep it; this test
/// pins that scope, not that sentence's truth for them.
#[test]
fn a_source_argument_evidence_is_conditional_and_leaves_other_kinds_alone() {
    let defect = |kind| StaticDefect {
        kind,
        location: location(7),
        analysis_context: String::new(),
        fixes: vec![],
        uncertain: false,
    };
    let terms = evidence_terms();
    let uncaptured = defect(StaticDefectKind::ReactiveSourceUncaptured {
        source: "count".into(),
        callee: "observe".into(),
    });
    assert_eq!(
        static_defect_text(&uncaptured, &terms).evidence,
        "a reference bound to a reactive accessor occupies this call expression's argument position; if the call executes, the available analysis does not establish whether reads through that accessor stay tracked"
    );
    let shared = "the invalid API shape is statically present at this call";
    for kind in [
        StaticDefectKind::MissingEffectFunction,
        StaticDefectKind::UncalledAccessor {
            name: "count".into(),
            position: "argument".into(),
        },
    ] {
        assert_eq!(static_defect_text(&defect(kind), &terms).evidence, shared);
    }
}

#[test]
fn the_acceptance_gate_sites_collapse_into_one_finding() {
    let gate = |at| {
        contract_defect(
            crate::contracts::UNACCEPTED_IMPORT_CONTEXT,
            crate::ContractDefectSite::Import,
            at,
        )
    };
    assert_eq!(
        collapse_unaccepted_contract_defects(&[gate(1), gate(2)], &ErrorCatalog).len(),
        1
    );
}

#[test]
fn an_unaccepted_import_is_filed_under_a_coverage_family() {
    assert!(
        super::coverage_group(FindingSeed::StaticDefect(&contract_defect(
            crate::contracts::UNACCEPTED_IMPORT_CONTEXT,
            crate::ContractDefectSite::Import,
            3
        )))
        .is_some()
    );
}
