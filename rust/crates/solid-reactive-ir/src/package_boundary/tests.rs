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
fn a_legacy_marker_answers_its_reason() {
    let mut boundary = PackageBoundary::default();
    let keys = [("/p/a.ts", "pkg"), ("/p/b.ts", "@s/x/sub")];
    for (importer, specifier) in keys {
        boundary.require_legacy(
            ImportKey::new(importer, specifier),
            UncertifiableImportReason::Unspecified,
        );
    }
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
    assert_eq!(explained.legacy_reason("/p/a.ts", "pkg"), None);
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
    let inputs = |boundary: &PackageBoundary| {
        crate::AnalysisInputs {
            boundary,
            hosts: None,
        }
        .cache_fingerprints()
    };
    assert!(
        crate::AnalysisInputs::empty()
            .cache_fingerprints()
            .is_empty()
    );
    assert!(inputs(&PackageBoundary::default()).is_empty());
    let (recorded, _) = recorded_boundary();
    let populated = inputs(&recorded);
    assert_eq!(populated.len(), 1);
    assert_eq!(
        hex(populated[0]),
        "e0c08052518b2734066e4fad48587e465b6bf682f1d121a8fe1b8bd20f71e09d"
    );
}

#[test]
fn a_recorded_cause_is_part_of_the_analysis_identity_and_an_empty_boundary_is_not() {
    assert!(
        crate::AnalysisInputs::empty()
            .cache_fingerprints()
            .is_empty()
    );
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
        hosts: None,
    }
    .cache_fingerprints();
    assert_eq!(explained, vec![boundary.digest()]);
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
            hosts: None,
        }
        .cache_fingerprints(),
        "a changed reason invalidates a cached answer although no file moved"
    );
}
