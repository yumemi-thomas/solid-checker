use super::*;

fn reason(kind: UnanalyzableReason, detail: &str) -> BoundaryReason {
    BoundaryReason::new(kind, detail)
}

#[test]
fn causes_are_kept_in_one_order_whatever_order_they_arrive_in() {
    let key = ImportKey::new("/p/src/a.ts", "pkg");
    let first = reason(
        UnanalyzableReason::ClosureLimit,
        "closure exceeds 4000 files",
    );
    let second = reason(
        UnanalyzableReason::UnsupportedBinding(BindingForm::NamespaceValue),
        "namespace import at /p/src/a.ts:3",
    );
    let mut forward = PackageBoundary::default();
    forward.record(key.clone(), Some("pkg"), None, first.clone());
    forward.record(key.clone(), Some("pkg"), None, second.clone());
    let mut backward = PackageBoundary::default();
    backward.record(key.clone(), Some("pkg"), None, second.clone());
    backward.record(key.clone(), Some("pkg"), None, first.clone());
    backward.record(key.clone(), Some("pkg"), None, first);
    assert_eq!(forward, backward);
    assert_eq!(forward.digest(), backward.digest());
    let import = forward.import("/p/src/a.ts", "pkg").expect("recorded");
    assert_eq!(import.reasons().len(), 2, "every cause, each once");
    assert_eq!(import.package_name.as_deref(), Some("pkg"));
}

#[test]
fn copies_and_subpaths_are_separate_imports() {
    let mut boundary = PackageBoundary::default();
    boundary.record(
        ImportKey::new("/p/a/src/x.ts", "pkg"),
        Some("pkg"),
        Some("/p/a/node_modules/pkg"),
        reason(
            UnanalyzableReason::ManifestUnavailable,
            "unreadable package.json",
        ),
    );
    boundary.record(
        ImportKey::new("/p/a/src/x.ts", "pkg/sub"),
        Some("pkg"),
        Some("/p/a/node_modules/pkg"),
        reason(
            UnanalyzableReason::RuntimeEntryUnavailable,
            "no runtime entry for ./sub",
        ),
    );
    // Another copy of the same name is not blamed for the first's reason.
    assert!(boundary.import("/p/b/src/y.ts", "pkg").is_none());
    assert_eq!(
        boundary
            .import("/p/a/src/x.ts", "pkg")
            .expect("copy")
            .reasons()[0]
            .kind,
        UnanalyzableReason::ManifestUnavailable
    );
    assert_eq!(
        boundary
            .import("/p/a/src/x.ts", "pkg/sub")
            .expect("subpath")
            .reasons()[0]
            .kind,
        UnanalyzableReason::RuntimeEntryUnavailable
    );
}

#[test]
fn a_conflicting_identity_becomes_unknown_rather_than_either() {
    let key = ImportKey::new("/p/src/a.ts", "alias");
    let mut boundary = PackageBoundary::default();
    let cause = reason(UnanalyzableReason::LegacyContractFallback, "x");
    boundary.record(key.clone(), Some("one"), Some("/r/one"), cause.clone());
    boundary.record(key.clone(), None, None, cause.clone());
    assert_eq!(
        boundary
            .import("/p/src/a.ts", "alias")
            .unwrap()
            .package_name
            .as_deref(),
        Some("one")
    );
    boundary.record(key.clone(), Some("two"), Some("/r/one"), cause.clone());
    boundary.record(key, Some("one"), None, cause);
    let import = boundary.import("/p/src/a.ts", "alias").unwrap();
    assert_eq!(import.package_name, None);
    assert_eq!(import.installed_root.as_deref(), Some("/r/one"));
}

#[test]
fn an_explicit_none_override_is_an_answer_and_absence_is_not() {
    let mut boundary = PackageBoundary::default();
    boundary.override_refusal(ImportKey::new("/p/a.ts", "pkg"), None);
    boundary.override_refusal(ImportKey::new("/p/a.ts", "pkg"), Some("later".into()));
    boundary.override_refusal(ImportKey::new("/p/b.ts", "pkg"), Some("refused".into()));
    assert_eq!(boundary.refusal_override("/p/a.ts", "pkg"), Some(None));
    assert_eq!(
        boundary.refusal_override("/p/b.ts", "pkg"),
        Some(Some("refused"))
    );
    assert_eq!(boundary.refusal_override("/p/c.ts", "pkg"), None);
    let mut other = PackageBoundary::default();
    other.override_refusal(ImportKey::new("/p/a.ts", "pkg"), Some("x".into()));
    assert_ne!(boundary.digest(), other.digest());
}

#[test]
fn the_legacy_projection_is_exactly_the_old_requirement_index() {
    let mut boundary = PackageBoundary::default();
    let keys = [("/p/a.ts", "pkg"), ("/p/b.ts", "@s/x/sub")];
    for (importer, specifier) in keys {
        boundary.require_legacy(
            ImportKey::new(importer, specifier),
            UncertifiableImportReason::Unspecified,
        );
    }
    let old = AcceptedContractIndex::default().with_uncertifiable_imports(
        keys.map(|(importer, specifier)| (importer.to_owned(), specifier.to_owned())),
    );
    assert_eq!(boundary.legacy_requirement_index(), old);
    assert_eq!(
        boundary.legacy_reason("/p/a.ts", "pkg"),
        Some(UncertifiableImportReason::Unspecified)
    );
    assert_eq!(boundary.legacy_reason("/p/c.ts", "pkg"), None);
    // A recorded cause alone is no requirement: it moves no finding.
    let mut explained = PackageBoundary::default();
    explained.record(
        ImportKey::new("/p/a.ts", "pkg"),
        Some("pkg"),
        None,
        reason(
            UnanalyzableReason::NotSelected(SelectionFailure::NoDemand),
            "x",
        ),
    );
    assert_eq!(
        explained.legacy_requirement_index(),
        AcceptedContractIndex::default()
    );
}

#[test]
fn export_causes_stay_apart_from_import_wide_ones() {
    let key = ImportKey::new("/p/a.ts", "pkg");
    let site = BoundarySite {
        path: "/p/a.ts".into(),
        start_byte: 9,
        end_byte: 12,
    };
    let mut boundary = PackageBoundary::default();
    boundary.record_export(
        key.clone(),
        ExportDemandKey {
            export: "f".into(),
            declaration: None,
            use_site: site.clone(),
        },
        reason(
            UnanalyzableReason::ExportJoinUnavailable(ExportJoinFailure::AmbiguousProvider),
            "two star providers",
        )
        .at(site),
    );
    let import = boundary.import("/p/a.ts", "pkg").unwrap();
    assert!(import.reasons().is_empty());
    assert_eq!(import.exports().len(), 1);
    let mut merged = PackageBoundary::default();
    merged.extend(boundary.clone());
    assert_eq!(merged, boundary);
}

fn hex(digest: [u8; 32]) -> String {
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Derived without this code: sha256 of the bytes `fingerprint` wrote at
/// e340233ba, computed by an independent script that reads only the
/// encoding (`legacy_pins.py` in the S report), for inputs specified here.
/// A boundary with no source-edge records must keep every one of these.
#[test]
fn a_boundary_with_no_source_edge_records_keeps_the_digest_it_had_before_they_existed() {
    assert_eq!(
        hex(PackageBoundary::default().digest()),
        "0c4320bbd149c02aa47cb7aba0fb5d4ba7157523934b91952974f48f77c3f2bd"
    );
    let mut legacy = PackageBoundary::default();
    legacy.require_legacy(
        ImportKey::new("/p/a.ts", "pkg"),
        UncertifiableImportReason::Unspecified,
    );
    legacy.require_legacy(
        ImportKey::new("/p/b.ts", "other/sub"),
        UncertifiableImportReason::ObsoletePolicy1,
    );
    legacy.override_refusal(ImportKey::new("/p/a.ts", "pkg"), Some("because".into()));
    legacy.override_refusal(ImportKey::new("/p/c.ts", "pkg"), None);
    assert_eq!(
        hex(legacy.digest()),
        "862e27b69d64dd5ab10a9d806a8efc213a840729def4c87602dcc68b325044c1"
    );
    let (recorded, _) = recorded_boundary();
    assert_eq!(
        hex(recorded.digest()),
        "e0c08052518b2734066e4fad48587e465b6bf682f1d121a8fe1b8bd20f71e09d"
    );
    let mut mixed = recorded;
    mixed.require_legacy(
        ImportKey::new("/p/a.ts", "pkg"),
        UncertifiableImportReason::Unspecified,
    );
    mixed.override_refusal(ImportKey::new("/p/a.ts", "pkg"), Some("because".into()));
    assert_eq!(
        hex(mixed.digest()),
        "77b26dc883d2aeebe5c16f24f3e1f73687e7223d34f287ec29637c451339c144"
    );
}

fn recorded_boundary() -> (PackageBoundary, ImportKey) {
    let key = ImportKey::new("/p/a.ts", "pkg");
    let mut boundary = PackageBoundary::default();
    boundary.record(
        key.clone(),
        Some("pkg"),
        Some("/p/node_modules/pkg"),
        reason(
            UnanalyzableReason::ClosureLimit,
            "closure exceeds 4000 files",
        )
        .at(BoundarySite {
            path: "/p/a.ts".into(),
            start_byte: 3,
            end_byte: 9,
        }),
    );
    boundary.record_export(
        key.clone(),
        ExportDemandKey {
            export: "run".into(),
            declaration: Some(BoundarySite {
                path: "/p/node_modules/pkg/index.js".into(),
                start_byte: 1,
                end_byte: 2,
            }),
            use_site: BoundarySite {
                path: "/p/a.ts".into(),
                start_byte: 10,
                end_byte: 20,
            },
        },
        reason(UnanalyzableReason::ManifestUnavailable, "m"),
    );
    boundary.record(
        ImportKey::new("/p/b.ts", "other"),
        None,
        None,
        reason(
            UnanalyzableReason::NotSelected(SelectionFailure::NoDemand),
            "d",
        ),
    );
    (boundary, key)
}

#[test]
fn the_analysis_identity_vectors_of_a_boundary_with_no_source_edges_are_unchanged() {
    let contracts = AcceptedContractIndex::default();
    let index = contracts.cache_fingerprint();
    let inputs = |boundary: &PackageBoundary| {
        crate::AnalysisInputs {
            boundary,
            contracts: &contracts,
            hosts: None,
        }
        .cache_fingerprints()
    };
    assert_eq!(
        crate::AnalysisInputs::contracts_only(&contracts).cache_fingerprints(),
        vec![index]
    );
    assert_eq!(inputs(&PackageBoundary::default()), vec![index]);
    let (recorded, _) = recorded_boundary();
    let populated = inputs(&recorded);
    assert_eq!(populated.len(), 2);
    assert_eq!(populated[0], index);
    assert_eq!(
        hex(populated[1]),
        "e0c08052518b2734066e4fad48587e465b6bf682f1d121a8fe1b8bd20f71e09d"
    );
}

#[test]
fn a_recorded_cause_is_part_of_the_analysis_identity_and_an_empty_boundary_is_not() {
    let contracts = AcceptedContractIndex::default();
    let plain = crate::AnalysisInputs::contracts_only(&contracts).cache_fingerprints();
    assert_eq!(plain, vec![contracts.cache_fingerprint()]);
    let mut boundary = PackageBoundary::default();
    boundary.record(
        ImportKey::new("/p/a.ts", "pkg"),
        Some("pkg"),
        None,
        reason(
            UnanalyzableReason::ClosureLimit,
            "closure exceeds 4000 files",
        ),
    );
    let explained = crate::AnalysisInputs {
        boundary: &boundary,
        contracts: &contracts,
        hosts: None,
    }
    .cache_fingerprints();
    assert_eq!(explained.len(), 2);
    assert_eq!(explained[0], plain[0]);
    let mut reworded = PackageBoundary::default();
    reworded.record(
        ImportKey::new("/p/a.ts", "pkg"),
        Some("pkg"),
        None,
        reason(
            UnanalyzableReason::ClosureLimit,
            "closure exceeds 4001 files",
        ),
    );
    assert_ne!(
        explained,
        crate::AnalysisInputs {
            boundary: &reworded,
            contracts: &contracts,
            hosts: None,
        }
        .cache_fingerprints(),
        "a changed reason invalidates a cached answer although no file moved"
    );
}

fn edge(form: SourceEdgeForm, start_byte: u64, end_byte: u64) -> SourceEdgeSite {
    SourceEdgeSite {
        form,
        start_byte,
        end_byte,
    }
}

fn with_edges(sites: &[(&str, &str, SourceEdgeSite)]) -> PackageBoundary {
    let mut boundary = PackageBoundary::default();
    for (importer, specifier, site) in sites {
        boundary.record_source_edge_refusal(ImportKey::new(*importer, *specifier), *site);
    }
    boundary
}

#[test]
fn source_edge_sites_are_one_ordered_set_whatever_order_and_repetition_they_arrive_in() {
    use SourceEdgeForm::{ExportFrom, Import};
    let one = edge(Import, 3, 20);
    let two = edge(ExportFrom, 30, 45);
    let forward = with_edges(&[("/p/a.ts", "pkg", one), ("/p/a.ts", "pkg", two)]);
    let backward = with_edges(&[
        ("/p/a.ts", "pkg", two),
        ("/p/a.ts", "pkg", one),
        ("/p/a.ts", "pkg", two),
    ]);
    assert_eq!(forward, backward);
    assert_eq!(forward.digest(), backward.digest());
    let sites = forward.source_edge_sites("/p/a.ts", "pkg").unwrap();
    assert_eq!(sites.iter().copied().collect::<Vec<_>>(), vec![one, two]);
    assert_eq!(forward.source_edge_sites("/p/b.ts", "pkg"), None);
    assert_eq!(forward.source_edge_sites("/p/a.ts", "pkg/sub"), None);
}

#[test]
fn two_occurrences_stay_two_sites_and_the_same_range_under_two_forms_stays_two() {
    use SourceEdgeForm::{DynamicImport, Import, Require};
    let boundary = with_edges(&[
        ("/p/a.ts", "pkg", edge(Import, 3, 20)),
        ("/p/a.ts", "pkg", edge(Import, 30, 47)),
        ("/p/a.ts", "pkg", edge(DynamicImport, 50, 62)),
        ("/p/a.ts", "pkg", edge(Require, 50, 62)),
    ]);
    assert_eq!(
        boundary.source_edge_sites("/p/a.ts", "pkg").unwrap().len(),
        4
    );
    assert_eq!(boundary.source_edge_refusals().count(), 1);
}

#[test]
fn every_part_of_a_source_edge_is_part_of_the_boundary_identity() {
    use SourceEdgeForm::{ExportFrom, Import};
    let base = with_edges(&[("/p/a.ts", "pkg", edge(Import, 3, 20))]);
    let variants = [
        with_edges(&[("/p/b.ts", "pkg", edge(Import, 3, 20))]),
        with_edges(&[("/p/a.ts", "pkg2", edge(Import, 3, 20))]),
        with_edges(&[("/p/a.ts", "pkg", edge(ExportFrom, 3, 20))]),
        with_edges(&[("/p/a.ts", "pkg", edge(Import, 4, 20))]),
        with_edges(&[("/p/a.ts", "pkg", edge(Import, 3, 21))]),
        with_edges(&[
            ("/p/a.ts", "pkg", edge(Import, 3, 20)),
            ("/p/a.ts", "pkg", edge(Import, 30, 47)),
        ]),
        PackageBoundary::default(),
    ];
    for variant in &variants {
        assert_ne!(variant.digest(), base.digest());
    }
    // Only source edges: not empty, and no package identity is invented.
    assert!(!base.is_empty());
    assert!(base.import("/p/a.ts", "pkg").is_none());
    assert_eq!(base.imports().count(), 0);
    assert_eq!(
        base.legacy_requirement_index(),
        AcceptedContractIndex::default()
    );
}

/// Derived without this code: the same independent script as above, from the
/// reviewed design of the added section.
#[test]
fn the_source_edge_section_has_the_bytes_the_design_states() {
    use SourceEdgeForm::{ExportFrom, Import, Require};
    let boundary = with_edges(&[
        ("/p/b.ts", "pkg", edge(Require, 5, 17)),
        ("/p/a.ts", "pkg", edge(ExportFrom, 30, 45)),
        ("/p/a.ts", "pkg", edge(Import, 3, 20)),
    ]);
    assert_eq!(
        hex(boundary.digest()),
        "cf50e2953da8461a100631cd7fa162f0b05dcf935de06e357d7f7410421dcd34"
    );
}

#[test]
fn extending_unions_sites_and_leaves_reasons_markers_and_overrides_alone() {
    use SourceEdgeForm::{ExportFrom, Import};
    let (recorded, key) = recorded_boundary();
    let mut left = recorded.clone();
    left.record_source_edge_refusal(key.clone(), edge(Import, 3, 20));
    let mut right = PackageBoundary::default();
    right.record_source_edge_refusal(key.clone(), edge(ExportFrom, 30, 45));
    right.record_source_edge_refusal(key.clone(), edge(Import, 3, 20));
    let mut folded = left.clone();
    folded.extend(right);
    assert_eq!(folded.source_edge_sites("/p/a.ts", "pkg").unwrap().len(), 2);
    // Everything that is not a source edge is what `left` already held.
    assert_eq!(
        folded.import("/p/a.ts", "pkg"),
        left.import("/p/a.ts", "pkg")
    );
    assert_eq!(
        folded.imports().collect::<Vec<_>>(),
        left.imports().collect::<Vec<_>>()
    );
    assert_eq!(
        folded.legacy_requirement_index(),
        left.legacy_requirement_index()
    );
    // A key holding only source edges never becomes a package identity.
    let mut only = PackageBoundary::default();
    only.record_source_edge_refusal(ImportKey::new("/p/c.ts", "x"), edge(Import, 0, 9));
    folded.extend(only);
    assert!(folded.import("/p/c.ts", "x").is_none());
    assert_eq!(folded.source_edge_refusals().count(), 2);
    assert!(PackageBoundary::default().is_empty());
}

#[test]
fn a_source_edge_alone_makes_the_analysis_identity_carry_the_boundary() {
    use SourceEdgeForm::Import;
    let contracts = AcceptedContractIndex::default();
    let boundary = with_edges(&[("/p/a.ts", "pkg", edge(Import, 3, 20))]);
    let vector = crate::AnalysisInputs {
        boundary: &boundary,
        contracts: &contracts,
        hosts: None,
    }
    .cache_fingerprints();
    assert_eq!(vector.len(), 2);
    assert_eq!(vector[0], contracts.cache_fingerprint());
    assert_eq!(vector[1], boundary.digest());
}

#[test]
fn the_form_spellings_are_the_stable_context_suffixes() {
    use SourceEdgeForm::{DynamicImport, ExportFrom, Import, ImportEquals, Require};
    let forms = [Import, ExportFrom, ImportEquals, DynamicImport, Require];
    assert_eq!(
        forms.map(SourceEdgeForm::code),
        [
            "import",
            "export-from",
            "import-equals",
            "dynamic-import",
            "require"
        ]
    );
    assert_eq!(
        forms.map(SourceEdgeForm::is_contract_bound),
        [true, true, false, false, false]
    );
}
