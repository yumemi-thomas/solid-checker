//! The consumer end of a source-edge refusal: which accepted contracts it
//! withholds, what it leaves alone, and how a retained build answers a change
//! of record. Written against synthetic facts for two files that both import
//! `run` from `pkg`, observed through `contract_binding.bound` and
//! `static_defects`.

use std::{borrow::Cow, collections::BTreeSet, sync::Arc};

use super::tests::{boundary, project, project_at, project_grounded, range, site};
use crate::contract_semantics::{
    AcceptedContractIndex, AcceptedContractInput, accepted_contract_fixture as accepted,
    accepted_exports, admitted_index,
};
use crate::contracts::{SOURCE_BOUNDARY_WITHHELD_CONTEXT, UNACCEPTED_IMPORT_CONTEXT};
use crate::hosts::BrowserViewSelection;
use crate::package_boundary::{
    ImportKey, PackageBoundary,
    SourceEdgeForm::{DynamicImport, ExportFrom, Import},
    SourceEdgeMismatch,
};
use crate::{
    AnalysisInputs, BuildError, IncrementalBuilder, Program, RuleOptions, StaticDefect,
    StaticDefectKind,
};
use solid_facts::ProjectFacts;

const A: &str = "/p/a.ts";
const B: &str = "/p/b.ts";
const SRC: &str = r#"import { run } from "pkg"; run();"#;
const STMT: &str = r#"import { run } from "pkg";"#;

fn two_files() -> ProjectFacts {
    project(&[(A, SRC), (B, SRC)])
}

fn record_a() -> PackageBoundary {
    boundary(A, "pkg", [site(Import, SRC, STMT)])
}

fn analyze(
    facts: &ProjectFacts,
    contracts: &AcceptedContractIndex,
    boundary: &PackageBoundary,
) -> Program {
    try_analyze(facts, contracts, boundary).unwrap()
}

fn try_analyze(
    facts: &ProjectFacts,
    contracts: &AcceptedContractIndex,
    boundary: &PackageBoundary,
) -> Result<Program, BuildError> {
    crate::build_with_analysis_inputs_measured(
        facts,
        &solid_dialect::Solid2,
        AnalysisInputs {
            boundary,
            contracts,
            hosts: None,
        },
        &RuleOptions::default(),
    )
    .map(|(program, _)| program)
}

fn neutral(program: &Program) -> Vec<&StaticDefect> {
    program
        .static_defects
        .iter()
        .filter(|defect| {
            defect
                .analysis_context
                .starts_with(SOURCE_BOUNDARY_WITHHELD_CONTEXT)
        })
        .collect()
}

fn input(importer: &str, artifact: Option<&str>) -> AcceptedContractInput {
    AcceptedContractInput {
        importer: importer.into(),
        specifier: "pkg".into(),
        contract: accepted(),
        artifact_identity: artifact.map(str::to_owned),
    }
}

fn owned(importer: &str) -> (String, String, String) {
    (importer.into(), "pkg".into(), "art".into())
}

fn browser() -> BrowserViewSelection {
    BrowserViewSelection::of_importers([A])
}

/// One index per tier, each answering `pkg` for both files through that tier
/// alone, so a denial that reaches the tier is the only way `a.ts` stops
/// being bound.
fn tiers() -> Vec<(&'static str, AcceptedContractIndex)> {
    vec![
        (
            "importer-keyed",
            AcceptedContractIndex::new([input(A, None), input(B, None)]).unwrap(),
        ),
        ("artifact-admitted", admitted_index('e')),
        (
            "per-importer admitted",
            AcceptedContractIndex::from_artifact_acceptances([("art".to_owned(), accepted())])
                .with_admitted_artifacts_for([owned(A), owned(B)]),
        ),
        (
            "scoped",
            AcceptedContractIndex::default().with_scoped("/p", admitted_index('f')),
        ),
        (
            // `a.ts` is answered by the view, `b.ts` by the outer tier. The
            // outer tier never answers for `a.ts` instead of a denied view.
            "browser view",
            admitted_index('e').with_browser_view(browser(), admitted_index('f')),
        ),
    ]
}

fn stacked() -> AcceptedContractIndex {
    AcceptedContractIndex::new([input(A, Some("art")), input(B, Some("art"))])
        .unwrap()
        .with_admitted_artifacts([("pkg".to_owned(), "art".to_owned())])
        .with_admitted_artifacts_for([owned(A), owned(B)])
        .with_scoped("/p", admitted_index('f'))
        .with_browser_view(browser(), admitted_index('9'))
}

#[test]
fn with_no_contract_and_no_marker_absence_is_not_evidence_and_a_record_gives_one_neutral_defect() {
    let facts = two_files();
    let none = AcceptedContractIndex::default();
    assert!(
        analyze(&facts, &none, &PackageBoundary::default())
            .static_defects
            .is_empty(),
        "no contract and no marker is no finding"
    );
    let program = analyze(&facts, &none, &record_a());
    let defects = neutral(&program);
    assert_eq!(defects.len(), 1);
    assert_eq!(program.static_defects.len(), 1);
    let defect = defects[0];
    let (start, end) = range(SRC, STMT);
    assert_eq!(
        (
            defect.location.path.as_ref(),
            defect.location.start_byte,
            defect.location.end_byte
        ),
        (A, start, end)
    );
    assert_eq!(defect.analysis_context, "source-boundary-withheld:import");
    assert!(defect.fixes.is_empty());
    assert!(!defect.uncertain);
    let StaticDefectKind::PackageContractExportMissing {
        module,
        export,
        reexported,
        admission_refusal,
        ..
    } = &defect.kind
    else {
        panic!("{:?}", defect.kind);
    };
    assert_eq!((module.as_str(), export.as_str()), ("pkg", "run"));
    assert!(!reexported && admission_refusal.is_none());
}

#[test]
fn every_ordinary_tier_is_denied_at_the_recorded_pair_and_only_there() {
    let facts = two_files();
    for (tier, contracts) in tiers() {
        let plain = analyze(&facts, &contracts, &PackageBoundary::default());
        assert_eq!(plain.contract_binding.bound, 2, "{tier}: both bound");
        assert!(plain.static_defects.is_empty(), "{tier}");
        let denied = analyze(&facts, &contracts, &record_a());
        assert_eq!(
            denied.contract_binding.bound, 1,
            "{tier}: a.ts loses its acceptance, b.ts keeps its own"
        );
        assert_eq!(neutral(&denied).len(), 1, "{tier}");
        assert_eq!(denied.static_defects.len(), 1, "{tier}");
        // Recording the sibling instead denies the sibling, symmetrically.
        let other = analyze(
            &facts,
            &contracts,
            &boundary(B, "pkg", [site(Import, SRC, STMT)]),
        );
        assert_eq!(other.contract_binding.bound, 1, "{tier}");
        assert_eq!(neutral(&other)[0].location.path.as_ref(), B, "{tier}");
    }
}

#[test]
fn every_tier_stacked_is_denied_together() {
    let facts = two_files();
    let contracts = stacked();
    assert_eq!(
        analyze(&facts, &contracts, &PackageBoundary::default())
            .contract_binding
            .bound,
        2
    );
    let denied = analyze(&facts, &contracts, &record_a());
    assert_eq!(denied.contract_binding.bound, 1);
    assert_eq!(neutral(&denied).len(), 1);
}

#[test]
fn the_index_withholds_contract_semantics_and_changes_no_enumeration() {
    let edges = BTreeSet::from([(A.to_owned(), "pkg".to_owned())]);
    let open = stacked().with_uncertifiable_imports([("/p/c.ts".to_owned(), "other".to_owned())]);
    let denied = open.clone().without_import_edges(&edges);
    let export = open
        .contract(B, "pkg")
        .unwrap()
        .export("run")
        .unwrap()
        .identity
        .clone();
    for importer in [A, B] {
        let denies = importer == A;
        assert_eq!(denied.contract(importer, "pkg").is_err(), denies);
        assert_eq!(denied.resolve_name(importer, "pkg", "run").is_err(), denies);
        assert_eq!(denied.resolve(importer, "pkg", &export).is_err(), denies);
        assert_eq!(denied.public_export_names(importer, "pkg").is_err(), denies);
    }
    // What the project accepted, and why something was not, is reported as it
    // was: the denial is of semantics at one pair, not of the record.
    assert_eq!(denied.semantic_identity(), open.semantic_identity());
    assert_eq!(
        denied.admitted_contracts().collect::<Vec<_>>(),
        open.admitted_contracts().collect::<Vec<_>>()
    );
    assert_eq!(
        denied.uncertifiable_reason("/p/c.ts", "other"),
        open.uncertifiable_reason("/p/c.ts", "other")
    );
    assert_eq!(
        denied.admission_refusal_at(A, "pkg"),
        open.admission_refusal_at(A, "pkg")
    );
    // No pair denied is no change at all, in identity too.
    assert_eq!(open.clone().without_import_edges(&BTreeSet::new()), open);
    assert_eq!(
        open.clone()
            .without_import_edges(&BTreeSet::new())
            .cache_fingerprint(),
        open.cache_fingerprint()
    );
    assert_ne!(denied.cache_fingerprint(), open.cache_fingerprint());
    // Folding in another index keeps the denial.
    let folded = denied
        .clone()
        .with_fallback(AcceptedContractIndex::new([input(A, None)]).unwrap());
    assert!(folded.contract(A, "pkg").is_err());
    // A scope added after the denial is still denied at this index.
    let scoped = denied.with_scoped("/p", admitted_index('c'));
    assert!(scoped.contract(A, "pkg").is_err());
    assert!(scoped.contract(B, "pkg").is_ok());
}

#[test]
fn a_legacy_marker_on_a_recorded_edge_gives_only_the_neutral_defect() {
    let facts = two_files();
    let marked = AcceptedContractIndex::default()
        .with_uncertifiable_imports([(A.to_owned(), "pkg".to_owned())]);
    let legacy = analyze(&facts, &marked, &PackageBoundary::default());
    assert_eq!(legacy.static_defects.len(), 1);
    assert_eq!(
        legacy.static_defects[0].analysis_context,
        UNACCEPTED_IMPORT_CONTEXT
    );
    let recorded = analyze(&facts, &marked, &record_a());
    assert_eq!(recorded.static_defects.len(), 1, "one story at one edge");
    assert_eq!(neutral(&recorded).len(), 1);
    assert!(
        recorded
            .static_defects
            .iter()
            .all(|defect| defect.analysis_context != UNACCEPTED_IMPORT_CONTEXT)
    );
}

#[test]
fn a_literal_only_record_is_reported_and_denies_nothing() {
    let source = r#"import { run } from "pkg"; run(); const x = import("pkg");"#;
    let facts = project(&[(A, source), (B, source)]);
    let contracts = AcceptedContractIndex::new([input(A, None), input(B, None)]).unwrap();
    let literal = boundary(A, "pkg", [site(DynamicImport, source, r#"import("pkg")"#)]);
    let program = analyze(&facts, &contracts, &literal);
    // The static import of the same key keeps its own, separate binding, and
    // the marker stands beside it: the deliberately narrower denial.
    assert_eq!(program.contract_binding.bound, 2);
    assert_eq!(neutral(&program).len(), 1);
    assert_eq!(
        neutral(&program)[0].analysis_context,
        "source-boundary-withheld:dynamic-import"
    );
    assert!(literal.denied_import_edges().is_empty());
}

#[test]
fn the_denial_is_borrowed_when_nothing_bound_is_recorded() {
    let contracts = stacked();
    let cow = || Cow::Borrowed(&contracts);
    let kept = |boundary: &PackageBoundary| {
        matches!(
            crate::pipeline::deny_recorded_edges(cow(), boundary),
            Cow::Borrowed(index) if std::ptr::eq(index, &contracts)
        )
    };
    assert!(kept(&PackageBoundary::default()));
    assert!(kept(&boundary(
        A,
        "pkg",
        [site(DynamicImport, r#"import("pkg")"#, r#"import("pkg")"#)]
    )));
    assert!(!kept(&record_a()));
    let denied = crate::pipeline::deny_recorded_edges(cow(), &record_a());
    assert!(denied.contract(A, "pkg").is_err());
    assert!(denied.contract(B, "pkg").is_ok());
}

#[test]
fn an_export_from_edge_is_denied_and_reported_at_its_own_statement() {
    let barrel = r#"export { run } from "pkg";"#;
    let facts = project(&[(A, barrel)]);
    let contracts = AcceptedContractIndex::new([input(A, None)]).unwrap();
    let record = boundary(A, "pkg", [site(ExportFrom, barrel, barrel)]);
    let program = analyze(&facts, &contracts, &record);
    let defects = neutral(&program);
    assert_eq!(defects.len(), 1);
    assert_eq!(
        defects[0].analysis_context,
        "source-boundary-withheld:export-from"
    );
    let StaticDefectKind::PackageContractExportMissing {
        reexported, export, ..
    } = &defects[0].kind
    else {
        panic!();
    };
    assert!(reexported);
    assert_eq!(export, "run");
}

fn counted(
    builder: &mut IncrementalBuilder,
    facts: &ProjectFacts,
    record: &PackageBoundary,
    contracts: &AcceptedContractIndex,
) -> (Arc<Program>, bool) {
    let (program, timings) = builder
        .build_with_analysis_inputs_shared(
            facts,
            &solid_dialect::Solid2,
            AnalysisInputs {
                boundary: record,
                contracts,
                hosts: None,
            },
            &RuleOptions::default(),
        )
        .unwrap();
    (program, timings.reused)
}

#[test]
fn a_retained_build_answers_only_the_record_it_was_built_from() {
    let source = r#"import { run } from "pkg"; run(); const x = import("pkg");"#;
    let facts = project(&[(A, source)]);
    let contracts = AcceptedContractIndex::new([input(A, None)]).unwrap();
    let import = site(Import, source, STMT);
    let dynamic = site(DynamicImport, source, r#"import("pkg")"#);
    let one = boundary(A, "pkg", [import]);
    let both = boundary(A, "pkg", [import, dynamic]);
    let mut builder = IncrementalBuilder::default();

    let (first, reused) = counted(&mut builder, &facts, &one, &contracts);
    assert!(!reused);
    let (again, reused) = counted(&mut builder, &facts, &one, &contracts);
    assert!(reused && Arc::ptr_eq(&first, &again));

    // Adding a site is a different record: not reused, and one more defect.
    let (added, reused) = counted(&mut builder, &facts, &both, &contracts);
    assert!(!reused);
    assert_eq!(neutral(&added).len(), 2);
    // A different form at the same key is a different record too.
    let swapped = boundary(A, "pkg", [dynamic]);
    let (literal, reused) = counted(&mut builder, &facts, &swapped, &contracts);
    assert!(!reused);
    assert_eq!(
        literal.contract_binding.bound, 1,
        "nothing denied: bound again"
    );
    // Removing the record is a fresh build of the same facts.
    let (removed, reused) = counted(
        &mut builder,
        &facts,
        &PackageBoundary::default(),
        &contracts,
    );
    assert!(!reused);
    let fresh = analyze(&facts, &contracts, &PackageBoundary::default());
    assert_eq!(*removed, fresh);
    assert!(neutral(&removed).is_empty());
}

#[test]
fn an_invalid_record_builds_nothing_and_the_next_valid_call_equals_a_fresh_build() {
    let facts = project(&[(A, SRC)]);
    let contracts = AcceptedContractIndex::new([input(A, None)]).unwrap();
    let (start, end) = range(SRC, STMT);
    let shifted = boundary(
        A,
        "pkg",
        [crate::package_boundary::SourceEdgeSite {
            form: Import,
            start_byte: start + 1,
            end_byte: end,
        }],
    );
    let valid = record_a();
    let mut builder = IncrementalBuilder::default();
    let (before, _) = counted(&mut builder, &facts, &valid, &contracts);
    let error = builder
        .build_with_analysis_inputs_shared(
            &facts,
            &solid_dialect::Solid2,
            AnalysisInputs {
                boundary: &shifted,
                contracts: &contracts,
                hosts: None,
            },
            &RuleOptions::default(),
        )
        .unwrap_err();
    assert!(matches!(
        error,
        BuildError::SourceEdge(SourceEdgeMismatch::NoSuchOccurrence { .. })
    ));
    // Semantic equality only: the retained state may have been cleared, so no
    // pointer or reuse promise is made across the failed call.
    let (after, _) = counted(&mut builder, &facts, &valid, &contracts);
    assert_eq!(*after, *before);
    assert_eq!(*after, analyze(&facts, &contracts, &valid));
}

#[test]
fn an_equal_range_in_a_new_generation_is_accepted_which_is_the_documented_limit() {
    // The record names only text and a range, so it cannot tell that the code
    // after the import changed. A caller reacquires records per generation.
    let v1 = project_at(1, &[(A, SRC)]);
    let v2_source = r#"import { run } from "pkg"; run(); run(() => 1);"#;
    let v2 = project_at(2, &[(A, v2_source)]);
    let contracts = AcceptedContractIndex::new([input(A, None)]).unwrap();
    let record = record_a();
    let mut builder = IncrementalBuilder::default();
    let (first, _) = counted(&mut builder, &v1, &record, &contracts);
    let (second, reused) = counted(&mut builder, &v2, &record, &contracts);
    assert!(!reused, "a new generation is a new build");
    assert_eq!(neutral(&first).len(), 1);
    assert_eq!(neutral(&second).len(), 1);
    assert_eq!(*second, analyze(&v2, &contracts, &record));
}

#[test]
fn the_key_is_the_importer_and_the_specifier_as_written() {
    let key = ImportKey::new(A, "pkg");
    let mut record = PackageBoundary::default();
    record.record_source_edge_refusal(key, site(Import, SRC, STMT));
    assert_eq!(record.denied_import_edges().len(), 1);
    assert_eq!(record, record_a());
}

fn defect_contexts(program: &Program) -> Vec<(String, String)> {
    program
        .static_defects
        .iter()
        .map(|defect| {
            let StaticDefectKind::PackageContractExportMissing { export, .. } = &defect.kind else {
                panic!("{:?}", defect.kind);
            };
            (export.clone(), defect.analysis_context.clone())
        })
        .collect()
}

/// R1-B1, through the ordinary consumer with grounded binding entities and an
/// accepted contract that leaves `idle`'s owner requirements open. Both
/// `run` and `idle` are declared exports; nothing here is a TypeScript
/// defect and no call to `idle` is invented.
#[test]
fn a_key_wide_denial_never_silently_drops_an_unreferenced_bindings_obligation() {
    let first = r#"import { run } from "pkg";"#;
    let second = r#"import { idle } from "pkg";"#;
    let source = format!("{first}\n{second}\nrun();\n");
    let facts = project_grounded(&[(A, &source)], &[(A, first, "run"), (A, second, "idle")]);
    let contracts = AcceptedContractIndex::new([AcceptedContractInput {
        importer: A.into(),
        specifier: "pkg".into(),
        contract: accepted_exports(&[("run", false), ("idle", true)]),
        artifact_identity: None,
    }])
    .unwrap();

    // Baseline: explicit unknowns at both imports (the fixture contract
    // leaves `reactiveReads` open on every export), including the open owner
    // requirements at the second import, which nothing else answers.
    let baseline = analyze(&facts, &contracts, &PackageBoundary::default());
    assert_eq!(baseline.contract_binding.bound, 2);
    assert_eq!(
        defect_contexts(&baseline),
        vec![
            (
                "run".to_owned(),
                "unknown-contract-claims:reactiveReads".to_owned()
            ),
            (
                "idle".to_owned(),
                "unknown-contract-claims:reactiveReads,ownerRequirements".to_owned()
            ),
        ]
    );
    let (start, _) = range(&source, "idle");
    assert_eq!(baseline.static_defects[1].location.start_byte, start);

    // Recording only the first import would erase it, so the build is an
    // explicit, neutral rejection naming the second statement: no program,
    // and no marker standing in for the other occurrence.
    let record = boundary(A, "pkg", [site(Import, &source, first)]);
    let (start_byte, end_byte) = range(&source, second);
    match try_analyze(&facts, &contracts, &record) {
        Err(BuildError::SourceEdge(SourceEdgeMismatch::UnreferencedCollateral {
            importer,
            specifier,
            start_byte: start,
            end_byte: end,
        })) => assert_eq!(
            (importer.as_str(), specifier.as_str(), start, end),
            (A, "pkg", start_byte, end_byte)
        ),
        other => panic!(
            "expected an explicit rejection, got {:?}",
            other.map(|_| ())
        ),
    }

    // The same two imports with `idle` referenced are two occurrences, and
    // recording both is the caller's decision for each: denied key-wide,
    // one neutral defect per statement, no unknown left behind by accident.
    let used = format!("{first}\n{second}\nrun();\nidle();\n");
    let used_facts = project_grounded(&[(A, &used)], &[(A, first, "run"), (A, second, "idle")]);
    let both = boundary(
        A,
        "pkg",
        [site(Import, &used, first), site(Import, &used, second)],
    );
    let recorded = analyze(&used_facts, &contracts, &both);
    assert_eq!(neutral(&recorded).len(), 2);
    assert_eq!(recorded.static_defects.len(), 2);
    // Only one of them recorded: the other is an unrecorded occurrence.
    assert!(matches!(
        try_analyze(
            &used_facts,
            &contracts,
            &boundary(A, "pkg", [site(Import, &used, first)])
        ),
        Err(BuildError::SourceEdge(
            SourceEdgeMismatch::UnrecordedOccurrence { .. }
        ))
    ));

    // Inert collateral stays supported: a type-only and a side-effect import
    // of the key hold no binding the projection answers.
    let inert = format!("{first}\nimport type {{ T }} from \"pkg\";\nimport \"pkg\";\nrun();\n");
    let inert_facts = project_grounded(&[(A, &inert)], &[(A, first, "run")]);
    let run_only = AcceptedContractIndex::new([AcceptedContractInput {
        importer: A.into(),
        specifier: "pkg".into(),
        contract: accepted_exports(&[("run", false)]),
        artifact_identity: None,
    }])
    .unwrap();
    assert_eq!(
        defect_contexts(&analyze(
            &inert_facts,
            &run_only,
            &PackageBoundary::default()
        )),
        vec![(
            "run".to_owned(),
            "unknown-contract-claims:reactiveReads".to_owned()
        )]
    );
    let marked = analyze(
        &inert_facts,
        &run_only,
        &boundary(A, "pkg", [site(Import, &inert, first)]),
    );
    assert_eq!(neutral(&marked).len(), 1);
    assert_eq!(marked.static_defects.len(), 1);

    // A literal-only record denies nothing, so it needs no collateral
    // rejection: both bindings keep their own unknowns beside the marker.
    let literal_source = format!("{first}\n{second}\nrun();\nconst x = import(\"pkg\");\n");
    let literal_facts = project_grounded(
        &[(A, &literal_source)],
        &[(A, first, "run"), (A, second, "idle")],
    );
    let literal = analyze(
        &literal_facts,
        &contracts,
        &boundary(
            A,
            "pkg",
            [site(DynamicImport, &literal_source, r#"import("pkg")"#)],
        ),
    );
    assert_eq!(literal.contract_binding.bound, 2);
    assert_eq!(neutral(&literal).len(), 1);
    assert_eq!(
        literal
            .static_defects
            .iter()
            .filter(|defect| defect
                .analysis_context
                .starts_with("unknown-contract-claims:"))
            .count(),
        2
    );
}
