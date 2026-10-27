use solid_reactive_ir::{
    AsyncRead, DirectMutationTarget, ExecutionRole, OwnerRequirement, OwnerRequirementOperation,
    ReactiveRead, ReactiveWrite, ReactiveWriteOperation, StaticDefect, StaticDefectKind,
};
use typefacts::Location;

use super::*;

fn location(index: u64) -> Location {
    Location {
        path: "catalog-prose.tsx".into(),
        start_byte: index,
        end_byte: index + 1,
    }
}

/// Materializes every shared static-defect wording branch plus the
/// catalog findings that consumed the old dialect prose helpers: a strict
/// read, an owned write, an ownerless cleanup (the owner arm whose hint
/// diverges most between the versions), and a pending async read (a table
/// only the 2.0 catalog projects). Dynamic subjects use a reserved prefix
/// so the API-name assertion below can distinguish user code quoted by a
/// finding from catalog-owned advice.
fn catalog_prose_program() -> Program {
    let defect_kinds = [
        StaticDefectKind::ReactiveObjectDestructure {
            source: "props".into(),
            component_props: true,
        },
        StaticDefectKind::ReactiveReadAfterAwait {
            accessor: "sampleAccessor".into(),
        },
        StaticDefectKind::ComponentReturnsConditionally,
        StaticDefectKind::PackageBehaviorUnavailable {
            module: "sample-package".into(),
            export: "sampleExport".into(),
            reexported: false,
        },
        StaticDefectKind::MissingEffectFunction,
        StaticDefectKind::ReactiveSourceUncaptured {
            source: "sampleAccessor".into(),
            callee: "sampleCallee".into(),
        },
        StaticDefectKind::ReactiveHandlerRead {
            attribute: "onClick".into(),
            expression: "sampleHandler".into(),
        },
        StaticDefectKind::UncalledAccessor {
            name: "sampleAccessor".into(),
            position: "sample expression".into(),
        },
        StaticDefectKind::DirectMutation {
            name: "sampleProjection".into(),
            target: DirectMutationTarget::Projection,
        },
        StaticDefectKind::DirectMutation {
            name: "sampleStore".into(),
            target: DirectMutationTarget::Store,
        },
        StaticDefectKind::DirectMutation {
            name: "sampleProps".into(),
            target: DirectMutationTarget::Props,
        },
    ];
    Program {
        reads: vec![ReactiveRead {
            summary_attributed: false,
            transported: false,
            kind: "signal".into(),
            accessor: "sampleAccessor".into(),
            location: location(1),
            declaration: location(2),
            execution: ExecutionRole::UntrackedRendering,
            context: "sample component".into(),
            via: "".into(),
            origin: None,
            origin_context: "".into(),
            uncertain: false,
            missing_jsx_census: false,
            host_callback_timing: false,
            project_consumer_non_strict: false,
            callback_invocation_unproven: false,
            callee_callback_timing: false,
        }],
        writes: vec![ReactiveWrite {
            setter: "sampleSetter".into(),
            operation: ReactiveWriteOperation::Setter,
            source_kind: solid_reactive_ir::ReactiveSourceKind::Accessor,
            location: location(3),
            declaration: location(4),
            execution: ExecutionRole::TrackedJsx,
            owned_write: solid_reactive_ir::OwnedWriteOption::Off,
            context: "sample computation".into(),
        }],
        missing_owners: vec![OwnerRequirement {
            operation: OwnerRequirementOperation::Cleanup,
            location: location(5),
            uncertain: false,
            runtime_uncertain: false,
            caller_uncertain: false,
            conditional_owner: false,
            later_run_unowned: false,
            component_uncertain: false,
            after_await: false,
            missing_jsx_census: false,
            through_dependency: None,
            report: true,
        }],
        async_reads: vec![AsyncRead {
            accessor: "sampleAsyncAccessor".into(),
            location: location(6),
            declaration: location(7),
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
        static_defects: defect_kinds
            .into_iter()
            .enumerate()
            .map(|(index, kind)| StaticDefect {
                kind,
                location: location(index as u64 + 10),
                analysis_context: String::new(),
                fixes: vec![],
                uncertain: false,
            })
            .collect(),
        ..Program::default()
    }
}

fn called_names(text: &str) -> impl Iterator<Item = String> + '_ {
    text.match_indices('(').filter_map(|(index, _)| {
        let name = text[..index]
            .chars()
            .rev()
            .take_while(|character| character.is_alphanumeric() || matches!(character, '_' | '$'))
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<String>();
        (!name.is_empty()).then_some(name)
    })
}

fn contains_identifier(text: &str, expected: &str) -> bool {
    text.split(|character: char| !(character.is_alphanumeric() || matches!(character, '_' | '$')))
        .any(|identifier| identifier == expected)
}

/// The default is read from the registry, not named.
///
/// With one dialect this cannot fail, which is exactly why it is written
/// as a property rather than as `assert_eq!(default_dialect().id,
/// "solid-v2")`: the assertion that would have to change on the day a
/// newer dialect is added is the assertion that would be wrong that day.
#[test]
fn the_default_is_the_newest_dialect_the_registry_carries() {
    let newest = ALL
        .iter()
        .map(|dialect| dialect.vocabulary.version())
        .max()
        .expect("ALL is non-empty");
    assert_eq!(default_dialect().vocabulary.version(), newest);
    // Registered: the language itself or one of its release variants,
    // resolvable by id to this very value.
    assert!(
        by_id(default_dialect().id).is_some_and(|found| std::ptr::eq(found, default_dialect())),
        "the default must be a registered dialect, not a value beside the registry"
    );
}

#[test]
fn dialect_ids_are_unique_and_resolvable() {
    for dialect in ALL {
        assert_eq!(by_id(dialect.id).map(|found| found.id), Some(dialect.id));
    }
    assert!(by_id("solid-v3").is_none());
}

#[test]
fn compatibility_registry_counts_match_the_catalog_migration() {
    assert_eq!(
        RULE_ALIASES.len(),
        21,
        "the migration note must list every transferred configuration key"
    );
    assert_eq!(
        RETIRED_RULES.len(),
        64,
        "eight pre-existing TypeScript redundancies, 31 catalog-reduction identities, the 18 the 1.x catalog still declared when ADR 0110 deleted it, and the seven names of SC9005, SC9011 and SC9012 (ADR 0440)"
    );
}

/// ADR 0428: a group is a key over rules, never a rule or a former name, and
/// each member is a real rule of the one catalog that carries the group.
#[test]
fn rule_groups_name_catalog_rules_and_collide_with_no_identity() {
    assert_eq!(
        RULE_GROUPS
            .iter()
            .map(|(name, _)| *name)
            .collect::<Vec<_>>(),
        ["reactivity"]
    );
    let v2 = default_dialect();
    for (group, members) in RULE_GROUPS {
        assert!(!members.is_empty());
        for member in *members {
            assert!((v2.has_rule)(member), "{group}: {member}");
            assert!(rule_group(member).is_none());
        }
        assert!(!(v2.has_rule)(group), "{group} must not also be a rule");
        assert!(rule_alias(group).is_none() && retired_rule(group).is_none());
    }
    assert_eq!(
        rule_group("reactivity"),
        Some(
            &[
                "strict-read-untracked",
                "reactive-read-after-await",
                "uncalled-accessor",
                "reactive-handler-frozen"
            ][..]
        )
    );
    // SC1003 keeps its own key in upstream (ADR 0428), and SC4001 has no
    // upstream counterpart: neither belongs to the group.
    let members = rule_group("reactivity").unwrap();
    assert!(!members.contains(&"no-destructure") && !members.contains(&"missing-owner"));
}

#[test]
fn every_catalog_identity_resolves_to_its_metadata() {
    for dialect in ALL {
        if dialect.id == "solid-v2" {
            for rule in solid_v2_rules::Rule::ALL {
                // A name is the lookup key (ADR 0426): codes sharing one name
                // resolve to the first of them, and agree on all else.
                let found = (dialect.rule_metadata)(rule.metadata().name).unwrap();
                let expected = rule.metadata();
                assert_eq!(
                    (
                        found.name,
                        found.severity,
                        found.uncertifiable,
                        found.default_enabled,
                        found.presets
                    ),
                    (
                        expected.name,
                        expected.severity,
                        expected.uncertifiable,
                        expected.default_enabled,
                        expected.presets
                    )
                );
            }
        }
    }
}

/// Catalogs own user-facing wording, but the generated export index still
/// owns the fact of which APIs exist. Exercise the real findings so moving
/// prose out of [`solid_dialect::Dialect`] cannot also remove the guard
/// that stopped 2.0 advice leaking into 1.x diagnostics (and vice versa).
#[test]
fn rule_catalog_prose_only_names_apis_exported_by_its_dialect() {
    const NON_SOLID_CALLS: &[&str] =
        &["dispose", "log", "queueMicrotask", "setStore", "setTimeout"];
    let program = catalog_prose_program();
    for (version, forbidden) in [
        (
            solid_dialect::Version::V1,
            &["action", "actions", "onSettled", "ownedWrite"][..],
        ),
        (
            solid_dialect::Version::V2,
            &[
                "onMount",
                "mergeProps",
                "splitProps",
                "produce",
                "Suspense",
                "SuspenseList",
            ][..],
        ),
    ] {
        // A single-dialect feature build simply has nothing to check for
        // the absent version.
        let Some(dialect) = by_version(version) else {
            continue;
        };
        let findings = dialect.solve(&program);
        // Beyond the defects: the strict read, the owned write, and the
        // ownerless cleanup. The pending async read joins them only in
        // 2.0 — the 1.x catalog deliberately never reads that table.
        let catalog_findings = match version {
            solid_dialect::Version::V1 => 3,
            solid_dialect::Version::V2 => 4,
        };
        assert_eq!(
            findings.len(),
            program.static_defects.len() + catalog_findings
        );

        let mut checked_calls = 0;
        let mut prose = String::new();
        for finding in &findings {
            for (field, text) in std::iter::once(("message", finding.message.as_str()))
                .chain(std::iter::once(("hint", finding.hint.as_str())))
                .chain(
                    finding
                        .evidence
                        .iter()
                        .map(|step| ("evidence", step.message.as_str())),
                )
            {
                prose.push_str(text);
                prose.push('\n');
                for name in called_names(text) {
                    if name.starts_with("sample") || NON_SOLID_CALLS.contains(&name.as_str()) {
                        continue;
                    }
                    assert!(
                        !dialect
                            .vocabulary
                            .export_modules(&name, solid_dialect::ExportPosition::Value)
                            .is_empty(),
                        "{version:?} catalog {field} names {name}(), which that dialect does not export: {text:?}"
                    );
                    checked_calls += 1;
                }
            }
        }
        assert!(
            checked_calls >= 5,
            "{version:?}: only {checked_calls} API calls checked"
        );
        for name in forbidden {
            assert!(
                !contains_identifier(&prose, name),
                "{version:?} catalog names the other dialect's {name}: {prose}"
            );
        }

        match version {
            solid_dialect::Version::V1 => {
                assert!(prose.contains("createEffect(fn, value?)"));
                assert!(prose.contains("splitProps(props"));
                assert!(prose.contains("mergeProps(defaults"));
                assert_eq!(
                    dialect
                        .vocabulary
                        .callback_positions(solid_dialect::Primitive::CreateEffect),
                    &[0]
                );
            }
            solid_dialect::Version::V2 => {
                assert!(prose.contains("createEffect(compute, apply)"));
                assert!(prose.contains("omit(props"));
                assert!(prose.contains("merge(defaults"));
                assert_eq!(
                    dialect
                        .vocabulary
                        .callback_positions(solid_dialect::Primitive::CreateEffect),
                    &[1]
                );
            }
        }
    }
}

#[test]
fn detection_reads_the_resolved_solid_js_version() {
    let root = std::env::temp_dir().join(format!(
        "solid-checker-dialect-detect-{}",
        std::process::id()
    ));
    let package = root.join("node_modules/solid-js");
    std::fs::create_dir_all(&package).unwrap();
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        package.join("package.json"),
        r#"{"name":"solid-js","version":"1.9.14"}"#,
    )
    .unwrap();
    let project = root.join("src/tsconfig.json");
    std::fs::write(&project, "{}").unwrap();
    // The classification, not the dialect id: a 1.x install resolves to
    // `Version::V1` in every build, and only a build carrying the 1.x
    // dialect can turn that into one.
    assert!(matches!(
        detect_detailed(&project),
        Detection::Installed {
            version: solid_dialect::Version::V1,
            ..
        } | Detection::Unsupported {
            classification: solid_dialect::Classification::Modelled(solid_dialect::Version::V1,),
            ..
        }
    ));

    std::fs::write(
        package.join("package.json"),
        r#"{"name":"solid-js","version":"2.0.0-rc.0"}"#,
    )
    .unwrap();
    assert!(matches!(
        detect_detailed(&project),
        Detection::Installed {
            version: solid_dialect::Version::V2,
            ..
        } | Detection::Unsupported {
            classification: solid_dialect::Classification::Modelled(solid_dialect::Version::V2,),
            ..
        }
    ));

    // No resolvable version answers the default rather than guessing.
    std::fs::write(
        package.join("package.json"),
        r#"{"name":"solid-js","version":"workspace:*"}"#,
    )
    .unwrap();
    assert!(matches!(
        detect_detailed(&project),
        Detection::Defaulted { manifest: Some(_) }
    ));
    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn a_broken_nearer_manifest_does_not_mask_an_installation_higher_up() {
    let root = std::env::temp_dir().join(format!(
        "solid-checker-dialect-broken-stub-{}",
        std::process::id()
    ));
    let outer = root.join("node_modules/solid-js");
    let inner = root.join("workspace/node_modules/solid-js");
    std::fs::create_dir_all(&outer).unwrap();
    std::fs::create_dir_all(&inner).unwrap();
    std::fs::create_dir_all(root.join("workspace/src")).unwrap();
    std::fs::write(
        outer.join("package.json"),
        r#"{"name":"solid-js","version":"1.9.14"}"#,
    )
    .unwrap();
    let project = root.join("workspace/src/tsconfig.json");
    std::fs::write(&project, "{}").unwrap();

    // Unparseable JSON and a version-less manifest are both the walk
    // continuing, exactly like an unreadable file.
    //
    // Asserted as *which manifest the walk selected*, which is the claim.
    // Reading it off a dialect id instead made this test depend on the 1.x
    // dialect being compiled in, and it failed in the
    // `--features dialect-v2` arm for a reason that has nothing to do with
    // the walk.
    let selected = |detection: Detection| match detection {
        Detection::Installed { manifest, .. } | Detection::Unsupported { manifest, .. } => manifest,
        Detection::Defaulted { manifest } => {
            panic!("the outer 1.x install is a resolution, not a default: {manifest:?}")
        }
    };
    std::fs::write(inner.join("package.json"), "{ not json").unwrap();
    assert_eq!(
        selected(detect_detailed(&project)),
        outer.join("package.json")
    );
    std::fs::write(inner.join("package.json"), r#"{"name":"solid-js"}"#).unwrap();
    assert_eq!(
        selected(detect_detailed(&project)),
        outer.join("package.json")
    );

    // A parseable version that classifies stops the walk at the nearest
    // manifest, masking the outer 1.x -- resolution order, not breakage.
    std::fs::write(
        inner.join("package.json"),
        r#"{"name":"solid-js","version":"2.0.0-rc.0"}"#,
    )
    .unwrap();
    assert_eq!(
        selected(detect_detailed(&project)),
        inner.join("package.json")
    );
    std::fs::remove_dir_all(&root).unwrap();
}

/// The three outcomes `detect` collapses, kept apart -- and the manifest
/// path a refusal has to name. The walk is unbounded, so the deciding
/// `package.json` is frequently not the one beside the project, and a
/// refusal that cannot say which file decided it is not actionable.
#[test]
fn detection_separates_an_install_from_a_default_and_names_the_manifest() {
    let root = std::env::temp_dir().join(format!(
        "solid-checker-dialect-detection-{}",
        std::process::id()
    ));
    let package = root.join("node_modules/solid-js");
    std::fs::create_dir_all(&package).unwrap();
    std::fs::create_dir_all(root.join("src")).unwrap();
    let manifest = package.join("package.json");
    let project = root.join("src/tsconfig.json");
    std::fs::write(&project, "{}").unwrap();

    std::fs::write(&manifest, r#"{"name":"solid-js","version":"2.0.0-rc.0"}"#).unwrap();
    match detect_detailed(&project) {
        Detection::Installed {
            dialect,
            version,
            manifest: read,
            ..
        } => {
            assert_eq!(dialect.id, "solid-v2");
            assert_eq!(version, solid_dialect::Version::V2);
            // Named from two directories up, not from beside the project.
            assert_eq!(read, manifest);
        }
        // A build without the 2.0 dialect still resolves the install and
        // still names the file; only the dialect is missing.
        Detection::Unsupported {
            classification: solid_dialect::Classification::Modelled(solid_dialect::Version::V2),
            installed,
            manifest: read,
            ..
        } => {
            assert_eq!(read, manifest);
            assert_eq!(installed, "2.0.0-rc.0");
        }
        other => panic!("an installed 2.0 is a resolution: {other:?}"),
    }

    // Resolvable but unclassifiable: a default that can still say which
    // file it read, which is what separates it from nothing installed.
    std::fs::write(&manifest, r#"{"name":"solid-js","version":"workspace:*"}"#).unwrap();
    match detect_detailed(&project) {
        Detection::Defaulted {
            manifest: Some(read),
        } => assert_eq!(read, manifest),
        other => panic!("an unclassifiable version defaults, and names its file: {other:?}"),
    }

    // Nothing installed anywhere above: a default with no file to name.
    std::fs::remove_dir_all(root.join("node_modules")).unwrap();
    assert!(matches!(
        detect_detailed(&project),
        Detection::Defaulted { manifest: None }
    ));
    std::fs::remove_dir_all(&root).unwrap();
}

/// The refusal's identity is held in two places on purpose; this is what
/// stops them drifting.
///
/// [`UNSUPPORTED_RUNTIME_CODE`] is what the emission actually writes, and
/// it cannot read a catalog because the refusal happens before a dialect
/// is chosen. The Solid 2 catalog is where every *consumer* resolves the
/// identity -- the npm rules manifest, suppression configuration, the
/// docs URL -- so a disagreement between them would publish a code no
/// adapter recognizes while the tool emitted it anyway.
#[cfg(feature = "dialect-v2")]
#[test]
fn the_refusal_identity_is_the_one_the_catalog_publishes() {
    let dialect = by_id("solid-v2").expect("the 2.0 dialect is compiled in");
    let metadata = (dialect.rule_metadata)(UNSUPPORTED_RUNTIME_RULE)
        .unwrap_or_else(|| panic!("the 2.0 catalog must declare {UNSUPPORTED_RUNTIME_RULE}"));
    assert_eq!(metadata.code, UNSUPPORTED_RUNTIME_CODE);
    assert_eq!(metadata.name, UNSUPPORTED_RUNTIME_RULE);
    assert!(
        metadata.uncertifiable,
        "the refusal asserts nothing about the project's source, so it is an \
             uncertifiable result and not a violation"
    );
    assert_eq!(metadata.severity, "error");
}

/// The refusal snapshot is the *whole* result, and its shape is the claim.
#[test]
fn the_refusal_snapshot_carries_one_finding_and_measures_nothing() {
    let snapshot = crate::diagnostics::unsupported_runtime_snapshot(
        "1.9.14",
        Path::new("/tmp/app/node_modules/solid-js/package.json"),
        None,
    );
    assert_eq!(snapshot.status, "uncertifiable");
    assert_eq!(
        snapshot.findings.len(),
        1,
        "a second finding would assert something about source that was \
             never analyzed under the language it runs"
    );
    let finding = &snapshot.findings[0];
    assert_eq!(finding.id, UNSUPPORTED_RUNTIME_CODE);
    assert_eq!(finding.rule, UNSUPPORTED_RUNTIME_RULE);
    assert_eq!(finding.kind, "uncertifiable");
    assert!(
        finding.message.contains("1.9.14"),
        "the refusal quotes the version it read, not the classified major: {}",
        finding.message
    );
    assert_eq!(
        finding.primary_location.path, "/tmp/app/node_modules/solid-js/package.json",
        "the deciding manifest is the location, because it is the file to change"
    );
    assert_eq!(
        snapshot.metrics.files_analyzed, 0,
        "nothing was read; reporting otherwise would overstate what happened"
    );
}

/// A named major with no vocabulary behind it refuses.
///
/// This is the half of the refusal that `Version` can still express:
/// `V1` exists precisely so detection can *recognise* `1.9.14` in order to
/// turn it down. The other half -- a major no variant names at all --
/// classifies as `UnmodelledMajor` and is pinned end to end by
/// `a_major_this_build_does_not_name_is_refused_like_one_it_does` in
/// `tests/dialects_process.rs`, because its whole claim is about what the
/// process emits.
#[test]
fn a_named_major_with_no_dialect_behind_it_is_unsupported() {
    let root = std::env::temp_dir().join(format!(
        "solid-checker-dialect-unsupported-{}",
        std::process::id()
    ));
    let package = root.join("node_modules/solid-js");
    std::fs::create_dir_all(&package).unwrap();
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        package.join("package.json"),
        r#"{"name":"solid-js","version":"1.9.14"}"#,
    )
    .unwrap();
    let project = root.join("src/tsconfig.json");
    std::fs::write(&project, "{}").unwrap();

    let detection = detect_detailed(&project);
    assert!(
        matches!(
            &detection,
            Detection::Unsupported {
                classification: solid_dialect::Classification::Modelled(solid_dialect::Version::V1,),
                ..
            }
        ),
        "a 1.x install is unsupported, not a 2.0 project: {detection:?}"
    );
    std::fs::remove_dir_all(&root).unwrap();
}

/// A scratch project whose nearest `node_modules/solid-js` names `version`,
/// beside no other Solid package.
fn installed_project(tag: &str, version: &str) -> (PathBuf, PathBuf) {
    installed_triple(tag, Some(version), None, None)
}

/// A scratch project with a hoisted `node_modules` holding each of the
/// three Solid 2 packages given a version.
fn installed_triple(
    tag: &str,
    solid_js: Option<&str>,
    signals: Option<&str>,
    web: Option<&str>,
) -> (PathBuf, PathBuf) {
    let root = std::env::temp_dir().join(format!(
        "solid-checker-dialect-release-{tag}-{}",
        std::process::id()
    ));
    for (package, version) in [
        ("solid-js", solid_js),
        ("@solidjs/signals", signals),
        ("@solidjs/web", web),
    ] {
        let Some(version) = version else { continue };
        let directory = root.join("node_modules").join(package);
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(
            directory.join("package.json"),
            format!(r#"{{"name":"{package}","version":"{version}"}}"#),
        )
        .unwrap();
    }
    std::fs::create_dir_all(root.join("src")).unwrap();
    let project = root.join("src/tsconfig.json");
    std::fs::write(&project, "{}").unwrap();
    (root, project)
}

/// The release classification end to end through detection: which dialect
/// analyzes each installation, and whether the analysis carries the
/// notice.
#[cfg(feature = "dialect-v2")]
#[test]
fn detection_classifies_the_installed_triple() {
    const RC3: Option<&str> = Some("2.0.0-rc.3");
    const RC9: Option<&str> = Some("2.0.0-rc.9");
    // (solid-js, signals, web, selected dialect id, notice due)
    let rows = [
        (
            RC3,
            RC3,
            RC3,
            "solid-v2@flush-in-effect-callback+optimistic-setter-guarded+latest-rejects-pending+hydrate-inline+disposal-rejects-writes",
            true,
        ),
        // rc.0's optimistic-store setter meets no owned-scope guard (N5),
        // its one answer that differs from rc.3's: a reviewed variant, so
        // no notice.
        (
            Some("2.0.0-rc.0"),
            Some("2.0.0-rc.0"),
            None,
            "solid-v2@optimistic-store-setter-unguarded+optimistic-setter-guarded+latest-rejects-pending+disposal-rejects-writes",
            true,
        ),
        (
            Some("2.0.0-rc.1"),
            Some("2.0.0-rc.1"),
            Some("2.0.0-rc.1"),
            "solid-v2@flush-in-effect-callback+optimistic-setter-guarded+latest-rejects-pending+hydrate-inline+disposal-rejects-writes",
            true,
        ),
        (
            Some("2.0.0-rc.2"),
            Some("2.0.0-rc.2"),
            Some("2.0.0-rc.2"),
            "solid-v2@flush-in-effect-callback+optimistic-setter-guarded+latest-rejects-pending+hydrate-inline+disposal-rejects-writes",
            true,
        ),
        (
            RC3,
            RC3,
            None,
            "solid-v2@flush-in-effect-callback+optimistic-setter-guarded+latest-rejects-pending+disposal-rejects-writes",
            true,
        ),
        (
            RC9,
            RC9,
            RC9,
            "solid-v2@store-root-mutable+omit-predicate+until+flush-in-action+flush-in-effect-callback+dynamic-static+store-setter-guards-roots+optimistic-setter-guarded+latest-rejects-pending+hydrate-inline+disposal-rejects-writes",
            true,
        ),
        // A fresh install of the audited solid-js today (the rc.1-rc.8
        // review § 5): the store answer is signals rc.9's.
        (
            RC3,
            RC9,
            RC3,
            "solid-v2@store-root-mutable+omit-predicate+flush-in-action+flush-in-effect-callback+store-setter-guards-roots+optimistic-setter-guarded+latest-rejects-pending+hydrate-inline+disposal-rejects-writes",
            true,
        ),
        (
            Some("2.0.0-rc.4"),
            Some("2.0.0-rc.4"),
            Some("2.0.0-rc.4"),
            "solid-v2@flush-in-effect-callback+optimistic-setter-guarded+latest-rejects-pending+hydrate-inline+disposal-rejects-writes",
            true,
        ),
        (
            Some("2.0.0-rc.6"),
            Some("2.0.0-rc.6"),
            Some("2.0.0-rc.6"),
            "solid-v2@until+flush-in-effect-callback+optimistic-setter-guarded+latest-rejects-pending+hydrate-inline+disposal-rejects-writes",
            true,
        ),
        (
            Some("2.0.0-rc.7"),
            Some("2.0.0-rc.7"),
            Some("2.0.0-rc.7"),
            "solid-v2@store-root-mutable+until+flush-in-effect-callback+optimistic-setter-guarded+latest-rejects-pending+hydrate-inline+disposal-rejects-writes",
            true,
        ),
        // No signals beside solid-js: the conservative answers, and the
        // notice.
        (
            RC9,
            None,
            RC9,
            "solid-v2@dynamic-static+hydrate-inline",
            true,
        ),
        (RC3, None, RC3, "solid-v2@hydrate-inline", true),
        (
            Some("2.0.0-rc.10"),
            Some("2.0.0-rc.10"),
            Some("2.0.0-rc.10"),
            "solid-v2@dynamic-options-unread",
            true,
        ),
        (
            Some("2.0.0-beta.19"),
            RC3,
            RC3,
            "solid-v2@flush-in-effect-callback+optimistic-setter-guarded+latest-rejects-pending+hydrate-inline+disposal-rejects-writes",
            true,
        ),
        (
            Some("2.0.0"),
            RC3,
            RC3,
            "solid-v2@flush-in-effect-callback+optimistic-setter-guarded+latest-rejects-pending+hydrate-inline+disposal-rejects-writes",
            true,
        ),
    ];
    for (index, (solid_js, signals, web, id, notice_due)) in rows.into_iter().enumerate() {
        let (root, project) = installed_triple(&format!("row{index}"), solid_js, signals, web);
        let detection = detect_detailed(&project);
        let Detection::Installed {
            dialect,
            installed: read,
            releases,
            ..
        } = &detection
        else {
            panic!("{solid_js:?} is analyzed: {detection:?}");
        };
        let row = format!("{solid_js:?}/{signals:?}/{web:?}");
        assert_eq!(dialect.id, id, "{row}");
        assert_eq!(Some(read.as_str()), solid_js);
        assert_eq!(
            releases
                .iter()
                .map(|release| release.version.as_deref())
                .collect::<Vec<_>>(),
            vec![solid_js, signals, web],
            "{row}"
        );
        let notice = release_notice(dialect, &project);
        assert_eq!(notice.is_some(), notice_due, "{row}: {notice:?}");
        if let Some(notice) = notice {
            assert_eq!(Some(notice.installed.as_str()), solid_js);
            assert_eq!(
                notice.manifest,
                root.join("node_modules/solid-js/package.json")
            );
            assert!(!notice.gaps.is_empty());
            assert_eq!(notice.audited.len(), 3);
        }
        // A dialect detection did not pick is a decision: no notice. The
        // audited language under an rc.9 install is the `--dialect
        // solid-v2` escape hatch.
        if dialect.id != SOLID_V2.id {
            assert_eq!(release_notice(&SOLID_V2, &project), None, "{row}");
        }
        std::fs::remove_dir_all(&root).unwrap();
    }
}

/// `@solidjs/signals` resolves from the installed `solid-js`, the way that
/// package's own re-export does: a copy nested under `solid-js` wins over
/// the hoisted one.
#[cfg(feature = "dialect-v2")]
#[test]
fn signals_resolves_from_the_installed_solid_js() {
    let (root, project) = installed_triple(
        "nested-signals",
        Some("2.0.0-rc.3"),
        Some("2.0.0-rc.3"),
        Some("2.0.0-rc.3"),
    );
    // The rc.3 triple is reviewed, and older than the audited release:
    // that is its one gap.
    let Detection::Installed {
        dialect: reviewed, ..
    } = detect_detailed(&project)
    else {
        panic!("analyzed");
    };
    assert_eq!(
        reviewed.id,
        "solid-v2@flush-in-effect-callback+optimistic-setter-guarded+latest-rejects-pending+hydrate-inline+disposal-rejects-writes"
    );
    let notice = release_notice(reviewed, &project).expect("rc.3 is older than audited");
    assert_eq!(notice.gaps.len(), 1, "{notice:?}");
    let nested = root.join("node_modules/solid-js/node_modules/@solidjs/signals");
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::write(
        nested.join("package.json"),
        r#"{"name":"@solidjs/signals","version":"2.0.0-rc.9"}"#,
    )
    .unwrap();
    let Detection::Installed {
        dialect, releases, ..
    } = detect_detailed(&project)
    else {
        panic!("analyzed");
    };
    assert_eq!(
        dialect.id,
        "solid-v2@store-root-mutable+omit-predicate+flush-in-action+flush-in-effect-callback+store-setter-guards-roots+optimistic-setter-guarded+latest-rejects-pending+hydrate-inline+disposal-rejects-writes"
    );
    assert_eq!(releases[1].version.as_deref(), Some("2.0.0-rc.9"));
    let nested_manifest = "node_modules/solid-js/node_modules/@solidjs/signals/package.json";
    assert!(
        releases[1]
            .manifest
            .as_ref()
            .is_some_and(|manifest| manifest.ends_with(nested_manifest)),
        "{releases:?}"
    );
    // Every manifest the review read is a watched input, and an owner
    // that did not resolve contributes the path an install would create.
    let watched = release_manifests(&project);
    assert!(
        watched
            .iter()
            .any(|path| path.ends_with("node_modules/solid-js/package.json"))
    );
    assert!(
        watched.iter().any(|path| path.ends_with(nested_manifest)),
        "{watched:?}"
    );
    std::fs::remove_dir_all(root.join("node_modules/@solidjs/web")).unwrap();
    assert!(
        release_manifests(&project)
            .iter()
            .any(|path| path.ends_with("node_modules/@solidjs/web/package.json"))
    );
    std::fs::remove_dir_all(&root).unwrap();
}

/// The pre-beta experiment is refused although its major is carried, and
/// the refusal carries the vocabulary's reason.
#[cfg(feature = "dialect-v2")]
#[test]
fn the_pre_beta_experiment_is_refused_with_its_reason() {
    let (root, project) = installed_project("experimental", "2.0.0-experimental.1");
    let detection = detect_detailed(&project);
    let Detection::Unsupported {
        classification,
        installed,
        refusal: Some(refusal),
        manifest,
    } = &detection
    else {
        panic!("2.0.0-experimental.1 is refused: {detection:?}");
    };
    assert_eq!(
        *classification,
        solid_dialect::Classification::Modelled(solid_dialect::Version::V2)
    );
    assert_eq!(installed, "2.0.0-experimental.1");
    assert_eq!(refusal.line, "2.0.0-experimental.x");
    let snapshot =
        crate::diagnostics::unsupported_runtime_snapshot(installed, manifest, Some(refusal));
    assert_eq!(snapshot.status, "uncertifiable");
    assert_eq!(snapshot.findings.len(), 1);
    let message = &snapshot.findings[0].message;
    assert!(
        message.contains("2.0.0-experimental.1")
            && message.contains("argument 2")
            && !message.contains("carries no dialect"),
        "the refusal states the vocabulary's reason, not an absent dialect: {message}"
    );
    // The release it points to is the audited one, read from the
    // vocabulary rather than spelled in the hint.
    let audited = by_version(solid_dialect::Version::V2)
        .expect("2.0 is carried")
        .vocabulary
        .audited_installation();
    assert_eq!(refusal.audited, audited);
    let hint = &snapshot.findings[0].hint;
    assert!(
        hint.contains(&format!("(the audited one is {})", audited[0].1)),
        "{hint}"
    );
    assert_eq!(snapshot.findings[0].id, UNSUPPORTED_RUNTIME_CODE);
    std::fs::remove_dir_all(&root).unwrap();
}

/// Every variant is one its language lists, named after its key,
/// resolvable by id, absent from the language registry, and the same
/// catalog as its language.
#[cfg(feature = "dialect-v2")]
#[test]
fn every_release_variant_is_one_its_language_lists() {
    let mut ids = Vec::new();
    for language in ALL.iter().copied() {
        for vocabulary in language.vocabulary.variants() {
            let variant = for_vocabulary(language, Some(*vocabulary));
            let key = variant
                .vocabulary
                .variant_key()
                .expect("a variant names its key");
            assert_eq!(variant.id, format!("{}@{key}", language.id));
            assert!(std::ptr::eq(
                variant.vocabulary as *const dyn solid_dialect::Dialect as *const (),
                *vocabulary as *const dyn solid_dialect::Dialect as *const ()
            ));
            assert!(by_id(variant.id).is_some_and(|found| std::ptr::eq(found, variant)));
            assert!(std::ptr::eq(
                for_vocabulary(language, Some(*vocabulary)),
                variant
            ));
            assert!(!ALL.iter().any(|registered| registered.id == variant.id));
            // The variant is the same catalog; only the vocabulary moves.
            assert_eq!(variant.rule_count, language.rule_count);
            assert_eq!(
                variant.compiler_facts_identity,
                language.compiler_facts_identity
            );
            ids.push(variant.id);
        }
        // An id whose key no variant spells resolves to nothing.
        for malformed in ["", "nope", "until+until"] {
            assert!(by_id(&format!("{}@{malformed}", language.id)).is_none());
        }
    }
    assert!(!ids.is_empty());
    let count = ids.len();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), count, "variant ids are unique");
    // B1, as the dialects detection selects for the rc.3 and rc.9 triples
    // answer it.
    assert!(SOLID_V2.vocabulary.store_root_properties_are_readonly());
    assert!(!SOLID_V2_RC9.vocabulary.store_root_properties_are_readonly());
    // B3: only rc.9's `omit` can invoke its lone second argument.
    let omit = solid_dialect::Primitive::Omit;
    assert!(
        !SOLID_V2
            .vocabulary
            .callback_runs_on_result_access(omit, 1, 2)
    );
    assert!(
        SOLID_V2_RC9
            .vocabulary
            .callback_runs_on_result_access(omit, 1, 2)
    );
    assert!(std::ptr::eq(for_vocabulary(&SOLID_V2, None), &SOLID_V2));
    // The language is the vocabulary ids are spelled against, and a
    // project with nothing installed gets the one the dialect names for
    // it, whichever entry that is.
    assert_eq!(SOLID_V2.vocabulary.variant_key(), None);
    assert!(std::ptr::eq(
        default_dialect(),
        for_vocabulary(&SOLID_V2, Some(&solid_dialect::Solid2::DEFAULTED))
    ));
}

/// ADR 0360, 0441: the refused-file finding's identity is the catalog's. A
/// proven refusal is the catalog's error, one per file; any other refusal is
/// the project-level notice quoting the compiler's first line.
#[cfg(feature = "dialect-v2")]
#[test]
fn the_compiler_refusal_identity_is_the_one_the_catalog_publishes() {
    use solid_facts::compiler::RefusalVerdict;
    let dialect = by_id("solid-v2").expect("the 2.0 dialect is compiled in");
    let metadata = (dialect.rule_metadata)(COMPILER_REFUSED_RULE)
        .unwrap_or_else(|| panic!("the 2.0 catalog must declare {COMPILER_REFUSED_RULE}"));
    assert_eq!(metadata.code, COMPILER_REFUSED_CODE);
    assert!(!metadata.uncertifiable);
    assert_eq!(metadata.severity, "error");
    let proven = crate::diagnostics::compiler_refused_finding(
        "/app/View.tsx",
        "/app/View.tsx: `attr:data-z` on <p> uses the `attr:` namespace, which was removed in Solid 2.0. Attributes are the default in 2.0. (3:5)",
        RefusalVerdict::ProvenDefect,
    );
    assert_eq!(proven.id, metadata.code);
    assert_eq!(proven.rule, metadata.name);
    assert_eq!(proven.severity, metadata.severity);
    assert_eq!(proven.kind, "violation");
    assert_eq!(proven.subject_kind, "file");
    assert_eq!(proven.primary_location.path.as_ref(), "/app/View.tsx");
    assert!(
        proven.message.starts_with(
            "the pinned Solid 2 compiler refuses /app/View.tsx: `attr:data-z` on <p> uses the `attr:` namespace"
        ),
        "{}",
        proven.message
    );
    for verdict in [RefusalVerdict::Unproven, RefusalVerdict::TypeScriptOwned] {
        let finding = crate::diagnostics::compiler_refused_finding(
            "/app/View.tsx",
            "/app/View.tsx: The HTML provided is malformed. (3:5)\nUser HTML:\n <p><p></p></p>",
            verdict,
        );
        assert_eq!(finding.id, metadata.code);
        assert_eq!(finding.rule, metadata.name);
        assert_eq!(finding.severity, "warning");
        assert_eq!(finding.kind, "uncertifiable");
        assert_eq!(finding.subject_kind, "project");
        assert!(
            finding.message.starts_with(
                "the Solid compiler refused /app/View.tsx: The HTML provided is malformed. (3:5);"
            ),
            "{}",
            finding.message
        );
        assert!(finding.evidence[0].message.contains("User HTML"));
    }
}

/// The notice's identity is held here and published by the catalog; this
/// keeps them from drifting, as for the refusal.
#[cfg(feature = "dialect-v2")]
#[test]
fn the_notice_identity_is_the_one_the_catalog_publishes() {
    let dialect = by_id("solid-v2").expect("the 2.0 dialect is compiled in");
    let metadata = (dialect.rule_metadata)(UNAUDITED_RELEASE_RULE)
        .unwrap_or_else(|| panic!("the 2.0 catalog must declare {UNAUDITED_RELEASE_RULE}"));
    assert_eq!(metadata.code, UNAUDITED_RELEASE_CODE);
    assert!(metadata.uncertifiable);
    let (root, project) = installed_triple(
        "notice-wording",
        Some("2.0.0-rc.3"),
        Some("2.0.0-rc.9"),
        Some("2.0.0-rc.3"),
    );
    let detected = match detect_detailed(&project) {
        Detection::Installed { dialect, .. } => dialect,
        other => panic!("analyzed: {other:?}"),
    };
    let notice = release_notice(detected, &project).expect("a mixed triple has gaps");
    let finding = crate::diagnostics::unaudited_release_finding(&notice);
    assert_eq!(finding.id, metadata.code);
    assert_eq!(finding.rule, metadata.name);
    assert_eq!(finding.severity, metadata.severity);
    assert_eq!(finding.kind, "uncertifiable");
    assert_eq!(finding.subject_kind, "project");
    let found = "solid-js 2.0.0-rc.3, @solidjs/signals 2.0.0-rc.9 and @solidjs/web 2.0.0-rc.3";
    assert!(finding.message.contains(found), "{}", finding.message);
    // The advice pins all three, and names what it found.
    assert!(
            finding.hint.contains(
                "pin solid-js, @solidjs/signals and @solidjs/web to 2.0.0-rc.17, the audited release of each"
            ) && finding
                .hint
                .contains(&format!("this project resolves {found}")),
            "{}",
            finding.hint
        );
    assert!(
        !finding.message.contains("  ") && !finding.hint.contains("  "),
        "wording is one line of prose: {:?} / {:?}",
        finding.message,
        finding.hint
    );
    assert!(
        // signals rc.9 carries no gap of its own any more; the mixed
        // triple's is the rc.1-rc.8 review's.
        finding.hint.contains(
            "The review is docs/release-audits/2026-09-26-solid-2-rc1-rc8-release-review.md."
        ),
        "the notice points at the reviews: {}",
        finding.hint
    );
    let gaps = finding
        .evidence
        .iter()
        .filter(|step| step.message.starts_with("known gap "))
        .count();
    assert_eq!(gaps, notice.gaps.len(), "every gap is named");
    assert_eq!(
        finding
            .evidence
            .iter()
            .filter(|step| step.message.contains("resolves here"))
            .count(),
        3,
        "every owner's manifest is evidence"
    );
    assert_eq!(
        &*finding.primary_location.path,
        root.join("node_modules/solid-js/package.json")
            .display()
            .to_string()
    );
    std::fs::remove_dir_all(&root).unwrap();
}
