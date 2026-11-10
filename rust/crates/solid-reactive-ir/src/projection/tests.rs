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
            multiplicity: 1,
        }],
        async_reads: vec![AsyncRead {
            accessor: Arc::from("user()"),
            location: location(3),
            declaration: location(4),
            execution: ExecutionRole::TrackedJsx,
            leaf_call_site_gates: Vec::new(),
            leaf_owner: None,
            under_loading: false,
            async_provenance: true,
            declared_loading: false,
            options_opaque: false,
            provenance_open: false,
            caught: false,
            component_prop: false,
            ssr_client_hole: false,
            server_rendering_unresolved: false,
            host_callback_timing: false,
            callee_callback_timing: false,
            invocation_context_unproven: false,
            mount_unresolved: false,
            settle_unproven: false,
            call_attributed: false,
            creation_pass_unproven: false,
            root_pass_unproven: false,
            carrier_run_unproven: false,
            apply_first_run_pending: false,
            pending_origin: None,
            pending_apply: None,
            origin: None,
            unaccounted: false,
            read_unresolved: false,
            source_module_unresolved: false,
            walk_cut: false,
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
            leaf_call_site_gates: Vec::new(),
            leaf_owner: Some(Arc::from("onSettled")),
            under_loading: false,
            async_provenance: true,
            declared_loading: false,
            options_opaque: false,
            provenance_open: false,
            caught: false,
            component_prop: false,
            ssr_client_hole: false,
            server_rendering_unresolved: false,
            host_callback_timing: false,
            callee_callback_timing: false,
            invocation_context_unproven: false,
            mount_unresolved: false,
            settle_unproven: false,
            call_attributed: false,
            creation_pass_unproven: false,
            root_pass_unproven: false,
            carrier_run_unproven: false,
            apply_first_run_pending: false,
            pending_origin: None,
            pending_apply: None,
            origin: None,
            unaccounted: false,
            read_unresolved: false,
            source_module_unresolved: false,
            walk_cut: false,
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
            multiplicity: 1,
        }],
        async_reads: vec![AsyncRead {
            accessor: Arc::from("user()"),
            location: location(3),
            declaration: location(4),
            execution: ExecutionRole::TrackedJsx,
            leaf_call_site_gates: Vec::new(),
            leaf_owner: None,
            under_loading: true,
            async_provenance: true,
            declared_loading: false,
            options_opaque: false,
            provenance_open: false,
            caught: false,
            component_prop: false,
            ssr_client_hole: false,
            server_rendering_unresolved: false,
            host_callback_timing: false,
            callee_callback_timing: false,
            invocation_context_unproven: false,
            mount_unresolved: false,
            settle_unproven: false,
            call_attributed: false,
            creation_pass_unproven: false,
            root_pass_unproven: false,
            carrier_run_unproven: false,
            apply_first_run_pending: false,
            pending_origin: None,
            pending_apply: None,
            origin: None,
            unaccounted: false,
            read_unresolved: false,
            source_module_unresolved: false,
            walk_cut: false,
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

/// A row listed once per reachable path projects as it did listed once: the
/// copies are adjacent, so their findings were adjacent and the final
/// `dedup` dropped them. A row held once with its path count
/// ([`ActionInvocation::multiplicity`]) projects as its copies did, and so
/// do adjacent rows that differ only in their counts.
#[test]
fn a_run_of_equal_rows_projects_as_one_row() {
    let action = |at: u64, multiplicity: u64| ActionInvocation {
        action: "save".into(),
        location: location(at),
        declaration: location(2),
        execution: ExecutionRole::TrackedJsx,
        context: "memo".into(),
        multiplicity,
    };
    let project = |actions: Vec<ActionInvocation>| {
        project_findings(
            &Program {
                actions,
                ..Program::default()
            },
            &RecordingCatalog(CatalogCapabilities::SOLID_2),
        )
        .0
    };
    let repeated = project(vec![
        action(1, 1),
        action(1, 1),
        action(1, 1),
        action(5, 1),
        action(1, 1),
    ]);
    assert_eq!(
        repeated,
        project(vec![action(1, 1), action(5, 1), action(1, 1)])
    );
    assert_eq!(
        repeated,
        project(vec![action(1, 3), action(5, 1), action(1, 1)])
    );
    assert_eq!(
        repeated,
        project(vec![action(1, 2), action(1, 1), action(5, 1), action(1, 1)])
    );
    assert_eq!(repeated.len(), 2);
}

// ADR 0933: `*_reported` are `*_silence().is_none()`. These restate the
// expressions they replaced and hold the two equal over every role, flag and
// capability, so the refactor moved no verdict.

const ROLES: [ExecutionRole; 12] = [
    ExecutionRole::Unknown,
    ExecutionRole::Unreported,
    ExecutionRole::ModuleInitialization,
    ExecutionRole::TrackedJsx,
    ExecutionRole::DeferredCallback,
    ExecutionRole::UntrackedCallback,
    ExecutionRole::UntrackedUnwindowed,
    ExecutionRole::EffectApply,
    ExecutionRole::EventCallback,
    ExecutionRole::DirectiveApply,
    ExecutionRole::UntrackedRendering,
    ExecutionRole::DiscardedRendering,
];

fn capabilities(untracked_writes: bool, module_scope_reads: bool) -> CatalogCapabilities {
    CatalogCapabilities {
        untracked_rendering_writes: untracked_writes,
        module_scope_strict_reads: module_scope_reads,
        ..CatalogCapabilities::SOLID_2
    }
}

fn read_row(execution: ExecutionRole, non_strict: bool) -> crate::ReactiveRead {
    crate::ReactiveRead {
        withdrawn_slot: false,
        summary_attributed: false,
        transported: false,
        kind: "accessor".into(),
        accessor: "a".into(),
        location: location(1),
        declaration: location(0),
        execution,
        context: "".into(),
        via: "".into(),
        origin: None,
        origin_context: "".into(),
        uncertain: false,
        pending_open: false,
        missing_jsx_census: false,
        host_callback_timing: false,
        project_consumer_non_strict: non_strict,
        callback_invocation_unproven: false,
        callee_callback_timing: false,
        reported_at_origin: false,
    }
}

#[test]
fn read_reported_is_the_expression_it_replaced_over_every_role_flag_and_capability() {
    for execution in ROLES {
        for non_strict in [false, true] {
            for module_scope in [false, true] {
                let capabilities = capabilities(true, module_scope);
                let read = read_row(execution, non_strict);
                let original = (execution.reports_untracked_read() || execution.reports_unplaced())
                    && !non_strict
                    && (capabilities.module_scope_strict_reads
                        || execution != ExecutionRole::ModuleInitialization);
                assert_eq!(read_reported(&read, capabilities), original);
                assert_eq!(read_silence(&read, capabilities).is_none(), original);
            }
        }
    }
}

#[test]
fn a_read_names_the_first_reason_it_is_withheld() {
    let caps = capabilities(true, false);
    assert_eq!(
        read_silence(&read_row(ExecutionRole::TrackedJsx, true), caps),
        Some(ReadSilence::RoleDoesNotReport)
    );
    assert_eq!(
        read_silence(&read_row(ExecutionRole::UntrackedRendering, true), caps),
        Some(ReadSilence::ProjectConsumerNonStrict)
    );
    assert_eq!(
        read_silence(&read_row(ExecutionRole::ModuleInitialization, false), caps),
        Some(ReadSilence::ModuleScopeNotReported)
    );
    assert_eq!(
        read_silence(&read_row(ExecutionRole::Unknown, false), caps),
        None
    );
}

#[test]
fn write_and_action_reported_are_the_expressions_they_replaced() {
    for execution in ROLES {
        for owned_write in [
            crate::OwnedWriteOption::Off,
            crate::OwnedWriteOption::On,
            crate::OwnedWriteOption::Unknown,
        ] {
            for untracked_writes in [false, true] {
                let capabilities = capabilities(untracked_writes, true);
                let write = crate::ReactiveWrite {
                    setter: Arc::from("set"),
                    operation: crate::ReactiveWriteOperation::Setter,
                    source_kind: crate::ReactiveSourceKind::Accessor,
                    location: location(1),
                    declaration: location(0),
                    execution,
                    owned_write,
                    context: Arc::from(""),
                    multiplicity: 1,
                };
                let original = owned_write != crate::OwnedWriteOption::On
                    && (execution.reports_disallowed_write() || execution.reports_unplaced())
                    && (capabilities.untracked_rendering_writes
                        || execution != ExecutionRole::UntrackedRendering);
                assert_eq!(write_reported(&write, capabilities), original);
                assert_eq!(write_silence(&write, capabilities).is_none(), original);
            }
        }
        let action = ActionInvocation {
            action: Arc::from("go"),
            location: location(1),
            declaration: location(0),
            execution,
            context: Arc::from(""),
            multiplicity: 1,
        };
        let original = execution.reports_disallowed_write() || execution.reports_unplaced();
        assert_eq!(action_reported(&action), original);
        assert_eq!(action_silence(&action).is_none(), original);
    }
}

#[test]
fn a_write_names_the_first_reason_it_is_withheld() {
    let write = |execution, owned_write| crate::ReactiveWrite {
        setter: Arc::from("set"),
        operation: crate::ReactiveWriteOperation::Setter,
        source_kind: crate::ReactiveSourceKind::Accessor,
        location: location(1),
        declaration: location(0),
        execution,
        owned_write,
        context: Arc::from(""),
        multiplicity: 1,
    };
    let caps = capabilities(false, true);
    assert_eq!(
        write_silence(
            &write(
                ExecutionRole::UntrackedRendering,
                crate::OwnedWriteOption::On
            ),
            caps
        ),
        Some(WriteSilence::OwnedWriteOn)
    );
    assert_eq!(
        write_silence(
            &write(
                ExecutionRole::DeferredCallback,
                crate::OwnedWriteOption::Off
            ),
            caps
        ),
        Some(WriteSilence::RoleDoesNotReport)
    );
    assert_eq!(
        write_silence(
            &write(
                ExecutionRole::UntrackedRendering,
                crate::OwnedWriteOption::Off
            ),
            caps
        ),
        Some(WriteSilence::UntrackedRenderingNotReported)
    );
    assert_eq!(
        write_silence(
            &write(ExecutionRole::TrackedJsx, crate::OwnedWriteOption::Unknown),
            caps
        ),
        None
    );
}

fn async_row(execution: ExecutionRole) -> AsyncRead {
    AsyncRead {
        accessor: Arc::from("user()"),
        location: location(3),
        declaration: location(4),
        execution,
        leaf_call_site_gates: Vec::new(),
        leaf_owner: None,
        under_loading: false,
        async_provenance: false,
        declared_loading: false,
        options_opaque: false,
        provenance_open: false,
        caught: false,
        component_prop: false,
        ssr_client_hole: false,
        server_rendering_unresolved: false,
        host_callback_timing: false,
        callee_callback_timing: false,
        invocation_context_unproven: false,
        mount_unresolved: false,
        settle_unproven: false,
        call_attributed: false,
        creation_pass_unproven: false,
        root_pass_unproven: false,
        carrier_run_unproven: false,
        apply_first_run_pending: false,
        pending_origin: None,
        pending_apply: None,
        origin: None,
        unaccounted: false,
        read_unresolved: false,
        source_module_unresolved: false,
        walk_cut: false,
    }
}

#[test]
fn async_read_reported_is_the_expression_it_replaced_over_every_role_and_flag() {
    // The clauses of ADR 0504, 0509 and 0662, in the order they were written.
    fn original(read: &AsyncRead) -> bool {
        if read.execution == ExecutionRole::DiscardedRendering {
            return false;
        }
        if read.apply_first_run_pending {
            return true;
        }
        if read.async_provenance
            && (read.leaf_owner.is_some()
                || read.execution == ExecutionRole::ModuleInitialization
                || read.execution == ExecutionRole::UntrackedRendering
                || (read.execution == ExecutionRole::Unknown && read.invocation_context_unproven))
        {
            return true;
        }
        if read.leaf_owner.is_none()
            && read.execution == ExecutionRole::UntrackedUnwindowed
            && !read.under_loading
            && read.async_provenance
            && !read.declared_loading
        {
            return true;
        }
        read.leaf_owner.is_none()
            && read.execution == ExecutionRole::TrackedJsx
            && !read.under_loading
            && (read.ssr_client_hole
                || read.server_rendering_unresolved
                || (read.async_provenance && !read.declared_loading))
    }
    let mut checked = 0;
    for execution in ROLES {
        for bits in 0_u32..128 {
            let flag = |bit: u32| bits & (1 << bit) != 0;
            let mut read = async_row(execution);
            read.apply_first_run_pending = flag(0);
            read.async_provenance = flag(1);
            read.leaf_owner = flag(2).then(|| Arc::from("onSettled"));
            read.under_loading = flag(3);
            read.declared_loading = flag(4);
            read.ssr_client_hole = flag(5);
            read.invocation_context_unproven = flag(6);
            read.server_rendering_unresolved = flag(5) != flag(6);
            assert_eq!(
                async_read_reported(&read),
                original(&read),
                "{bits:b} {execution:?}"
            );
            assert_eq!(async_read_clause(&read).reports(), original(&read));
            checked += 1;
        }
    }
    assert_eq!(checked, 12 * 128);
}
