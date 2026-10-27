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
                configurable: true,
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
            configurable: true,
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
                name: "package-behavior-unavailable",
                severity: "error",
                uncertifiable: true,
                default_enabled: true,
                configurable: true,
                presets: &[],
            },
            "unavailable",
            "",
        )
    }
}

fn unavailable_defect(at: u64) -> StaticDefect {
    StaticDefect {
        kind: StaticDefectKind::PackageBehaviorUnavailable {
            module: "pkg".into(),
            export: "access".into(),
            reexported: false,
        },
        location: location(at),
        analysis_context: crate::boundary_defects::UNANALYZED_IMPORT_CONTEXT.into(),
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

/// Unavailable package behavior collapses to one finding per package whatever
/// the export, and stays the rule's error.
#[test]
fn unavailable_package_behavior_collapses_per_package() {
    let other_package = {
        let mut defect = unavailable_defect(2);
        let StaticDefectKind::PackageBehaviorUnavailable { module, .. } = &mut defect.kind else {
            unreachable!()
        };
        *module = "other".into();
        defect
    };
    let defects = vec![
        unavailable_defect(1),
        other_package.clone(),
        unavailable_defect(3),
        {
            let mut defect = other_package;
            defect.location = location(4);
            defect
        },
    ];
    let reversed = defects.iter().cloned().rev().collect::<Vec<_>>();
    let collapsed = collapse_unanalyzed_package_defects(&defects, &ErrorCatalog);
    let starts = |findings: &[Finding]| {
        findings
            .iter()
            .map(|finding| finding.primary_location.start_byte)
            .collect::<Vec<_>>()
    };
    // First-seen order: each package group is anchored at its first site.
    assert_eq!(starts(&collapsed), [1, 2]);
    let package = &collapsed[0];
    assert_eq!(package.subject_kind, "package");
    assert_eq!(package.related_locations, [location(3)]);
    assert_eq!(package.severity, "error");
    assert_eq!(package.kind, "uncertifiable");
    // The same defects in the opposite order group the same way.
    let backward = collapse_unanalyzed_package_defects(&reversed, &ErrorCatalog);
    assert_eq!(starts(&backward), [4, 3]);

    // A group of one keeps its exact wording.
    let alone = collapse_unanalyzed_package_defects(&[unavailable_defect(9)], &ErrorCatalog);
    assert_eq!(alone[0].message, "unavailable");
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
            call_attributed: false,
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
            call_attributed: false,
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
            call_attributed: false,
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
fn the_import_sites_of_one_package_collapse_into_one_finding() {
    assert_eq!(
        collapse_unanalyzed_package_defects(
            &[unavailable_defect(1), unavailable_defect(2)],
            &ErrorCatalog
        )
        .len(),
        1
    );
}

#[test]
fn an_unanalyzed_import_is_filed_under_a_coverage_family() {
    assert!(super::coverage_group(FindingSeed::StaticDefect(&unavailable_defect(3))).is_some());
}
