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
    let cause = reason(UnanalyzableReason::LegacyRequirementFallback, "x");
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
fn a_legacy_marker_answers_its_key() {
    let mut boundary = PackageBoundary::default();
    let keys = [("/p/a.ts", "pkg"), ("/p/b.ts", "@s/x/sub")];
    for (importer, specifier) in keys {
        boundary.require_legacy(ImportKey::new(importer, specifier));
    }
    assert!(boundary.requires_legacy("/p/a.ts", "pkg"));
    assert!(!boundary.requires_legacy("/p/c.ts", "pkg"));
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
    assert!(!explained.requires_legacy("/p/a.ts", "pkg"));
}

fn hex(digest: [u8; 32]) -> String {
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Derived without this code: sha256 of the bytes `fingerprint` writes
/// (`package-boundary-v2`), computed by an independent script that reads only
/// the encoding, for inputs specified here. The encoding changed from v1 when
/// the never-populated export causes and refusal overrides, and the marker
/// reason byte, left it; every vector below is a deliberate re-pin.
#[test]
fn the_digest_follows_the_documented_encoding() {
    assert_eq!(
        hex(PackageBoundary::default().digest()),
        "9ba38701148688c5be5f155deaeff9631a286ab53d1ae13c77d818b7fa33c6ce"
    );
    let mut legacy = PackageBoundary::default();
    legacy.require_legacy(ImportKey::new("/p/a.ts", "pkg"));
    legacy.require_legacy(ImportKey::new("/p/b.ts", "other/sub"));
    assert_eq!(
        hex(legacy.digest()),
        "fe32e5f0ed7af717a307c1a611e72bf23155111c9725776d10a458c31953ab95"
    );
    let (recorded, _) = recorded_boundary();
    assert_eq!(
        hex(recorded.digest()),
        "fe44eb1034cae917296872e54d7c2273d76ad72b6f4040fb7e332c55cb60801f"
    );
    let mut mixed = recorded;
    mixed.require_legacy(ImportKey::new("/p/a.ts", "pkg"));
    assert_eq!(
        hex(mixed.digest()),
        "a19ca90975fda5380f708b74a3b3da9d1177613f1877ace21e6cd8d9a64994fb"
    );
}

#[test]
fn extending_a_boundary_keeps_every_cause_and_marker_of_both() {
    let (recorded, _) = recorded_boundary();
    let mut marked = PackageBoundary::default();
    marked.require_legacy(ImportKey::new("/p/a.ts", "pkg"));
    let mut merged = PackageBoundary::default();
    merged.extend(recorded.clone());
    merged.extend(marked.clone());
    let mut backward = marked;
    backward.extend(recorded);
    assert_eq!(merged, backward);
    assert!(merged.requires_legacy("/p/a.ts", "pkg"));
    assert_eq!(merged.import("/p/a.ts", "pkg").unwrap().reasons().len(), 1);
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
fn the_analysis_identity_vector_of_a_populated_boundary_is_its_digest() {
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
        "fe44eb1034cae917296872e54d7c2273d76ad72b6f4040fb7e332c55cb60801f"
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
