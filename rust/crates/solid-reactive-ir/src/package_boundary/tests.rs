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
