use std::sync::Arc;

use super::*;
use crate::{ActionInvocation, AsyncRead, ExecutionRole};

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

/// The acceptance gate collapses to one finding per package whatever the
/// export, and stays the rule's error; an obligation at an exact call argument
/// keeps its own finding, because there the site is the content.
#[test]
fn the_acceptance_gate_collapses_per_package_and_argument_sites_stay_apart() {
    use crate::ContractDefectSite::{Argument, Import};
    let gate = crate::contracts::UNACCEPTED_IMPORT_CONTEXT;
    let defects = vec![
        contract_defect(gate, Import, 1),
        contract_defect("claim-gate", Argument, 2),
        contract_defect(gate, Import, 3),
        contract_defect("claim-gate", Argument, 4),
    ];
    let reversed = defects.iter().cloned().rev().collect::<Vec<_>>();
    let collapsed = collapse_unaccepted_contract_defects(&defects, &ErrorCatalog);
    let starts = |findings: &[Finding]| {
        findings
            .iter()
            .map(|finding| finding.primary_location.start_byte)
            .collect::<Vec<_>>()
    };
    // First-seen order: the package group is anchored at its first site.
    assert_eq!(starts(&collapsed), [2, 4, 1]);
    let package = &collapsed[2];
    assert_eq!(package.subject_kind, "package");
    assert_eq!(package.related_locations, [location(3)]);
    assert_eq!(package.severity, "error");
    assert_eq!(package.kind, "uncertifiable");
    assert!(
        collapsed[..2]
            .iter()
            .all(|finding| finding.subject_kind.is_empty() && finding.related_locations.is_empty())
    );
    // The same defects in the opposite order group the same way.
    let backward = collapse_unaccepted_contract_defects(&reversed, &ErrorCatalog);
    assert_eq!(starts(&backward), [4, 2, 3]);

    // A group of one keeps its exact wording.
    let alone =
        collapse_unaccepted_contract_defects(&[contract_defect(gate, Import, 9)], &ErrorCatalog);
    assert_eq!(alone[0].message, "contract");
    assert!(alone[0].related_locations.is_empty());
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
