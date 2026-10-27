#[test]
fn inferred_projection_is_monotone_for_existing_violations() {
    use super::{SnapshotFinding, merge_host_findings};
    let finding = |kind: &str| {
        serde_json::from_value::<SnapshotFinding>(serde_json::json!({
            "id":"SC4001", "rule":"missing-owner", "kind":kind, "severity":"error", "message":"baseline",
            "primaryLocation":{"path":"app.ts", "startByte":0, "endByte":1, "line":1, "column":1}
        })).unwrap()
    };
    let baseline = [finding("violation"), finding("uncertifiable")];
    let kept = merge_host_findings(&baseline, &[], &|_| true, "inferred browser: root");
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].message, "baseline");
    let selected = merge_host_findings(
        &baseline,
        &[finding("violation")],
        &|_| true,
        "inferred browser: root",
    );
    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].evidence.len(), 1);
    let refused = merge_host_findings(
        &baseline,
        &[finding("violation")],
        &|_| false,
        "inferred browser: root",
    );
    assert_eq!(refused.len(), 2);
    assert!(refused.iter().all(|finding| finding.evidence.is_empty()));
}

use std::{path::Path, sync::Arc};

use solid_facts::core::Generation;
use solid_facts::{ProjectFacts, TypeScriptTable};
use solid_reactive_ir::RuntimeEnvironment;

use super::{DiagnosticSession, retain_enabled};

#[test]
fn unknown_finding_identities_fail_closed() {
    let mut findings = vec![solid_reactive_ir::Finding::new(
        solid_reactive_ir::RuleMetadata {
            code: "TEST00",
            name: "not-in-the-catalog",
            severity: "error",
            uncertifiable: false,
            default_enabled: true,
            configurable: true,
            presets: &[],
        },
        "synthetic".into(),
        typefacts::Location {
            path: "synthetic.tsx".into(),
            start_byte: 0,
            end_byte: 1,
        },
    )];
    let error = retain_enabled(
        crate::dialect::default_dialect(),
        &solid_reactive_ir::RuleOptions::default(),
        &mut findings,
    )
    .unwrap_err();
    assert!(findings.is_empty());
    assert!(matches!(
        error,
        crate::BackendError::UnknownRuleIdentity { rules, .. }
            if rules == ["not-in-the-catalog"]
    ));
}

#[test]
fn a_disabled_coverage_rule_returns_its_withheld_obligations_and_an_obligation_cannot_be_disabled()
{
    let finding = |code: &'static str, name: &'static str, uncertifiable: bool| {
        let mut finding = solid_reactive_ir::Finding::new(
            solid_reactive_ir::RuleMetadata {
                code,
                name,
                severity: "warning",
                uncertifiable,
                default_enabled: true,
                configurable: true,
                presets: &[],
            },
            "synthetic".into(),
            typefacts::Location {
                path: "synthetic.tsx".into(),
                start_byte: 0,
                end_byte: 1,
            },
        );
        if uncertifiable {
            finding.kind = "uncertifiable".into();
        }
        finding
    };
    let mut findings = vec![
        finding("SC7005", "http-response-after-flush", true),
        finding("SC1001", "strict-read-untracked", false),
        // An obligation has no key: the options below cannot disable it.
        finding("SC9012", "reactive-dispatch-unresolved", true),
        finding("SC9005", "package-behavior-unavailable", true),
    ];
    let options = solid_reactive_ir::RuleOptions::parse(
        r#"{ "schemaVersion": 1, "rules": {
          "http-response-after-flush": { "enabled": false },
          "reactive-dispatch-unresolved": { "enabled": false },
          "package-behavior-unavailable": { "enabled": false },
          "strict-read-untracked": { "enabled": false } } }"#,
        |_| true,
    )
    .unwrap();
    let withheld =
        retain_enabled(crate::dialect::default_dialect(), &options, &mut findings).unwrap();
    // The obligations stay (ADR 0440); the disabled rules go.
    assert_eq!(
        findings
            .iter()
            .map(|finding| finding.id.as_str())
            .collect::<Vec<_>>(),
        ["SC9012", "SC9005"]
    );
    // Only the coverage rule's obligation keeps the result from certifying;
    // a disabled defect rule is a rule the project chose not to check.
    assert_eq!(
        withheld
            .iter()
            .map(|finding| finding.id.as_str())
            .collect::<Vec<_>>(),
        ["SC7005"]
    );
}

/// ADR 0428: the `reactivity` key reaches the findings of its four members
/// through the same enablement as any member key, and a member the document
/// names keeps its own setting.
#[test]
fn the_reactivity_group_key_configures_its_members() {
    let finding = |code: &'static str, name: &'static str| {
        solid_reactive_ir::Finding::new(
            solid_reactive_ir::RuleMetadata {
                code,
                name,
                severity: "error",
                uncertifiable: false,
                default_enabled: true,
                configurable: true,
                presets: &[],
            },
            "synthetic".into(),
            typefacts::Location {
                path: "synthetic.tsx".into(),
                start_byte: 0,
                end_byte: 1,
            },
        )
    };
    let retained = |rules: &str| {
        let directory = std::env::temp_dir().join(format!(
            "solid-checker-group-key-{}-{}",
            std::process::id(),
            rules.len()
        ));
        std::fs::create_dir_all(directory.join(".solid-checker")).unwrap();
        std::fs::write(
            directory.join(".solid-checker").join("rule-options.json"),
            format!(r#"{{ "schemaVersion": 1, "rules": {{ {rules} }} }}"#),
        )
        .unwrap();
        let options = super::discover_rule_options(&directory).unwrap();
        std::fs::remove_dir_all(&directory).unwrap();
        let mut findings = vec![
            finding("SC1001", "strict-read-untracked"),
            finding("SC1002", "reactive-read-after-await"),
            finding("SC1003", "no-destructure"),
            finding("SC1005", "uncalled-accessor"),
            finding("SC1007", "reactive-handler-frozen"),
        ];
        retain_enabled(crate::dialect::default_dialect(), &options, &mut findings).unwrap();
        findings
            .iter()
            .map(|finding| finding.id.clone())
            .collect::<Vec<_>>()
    };
    // SC1003 is not a member: the group never reaches it.
    assert_eq!(
        retained(r#""reactivity": { "enabled": false }"#),
        ["SC1003"]
    );
    assert_eq!(
        retained(
            r#""reactivity": { "enabled": false },
               "reactive-handler-frozen": { "enabled": true },
               "strict-read-untracked": { "enabled": true }"#
        ),
        ["SC1001", "SC1003", "SC1007"]
    );
    assert_eq!(
        retained(r#""reactivity": { "enabled": true }, "uncalled-accessor": { "enabled": false }"#),
        ["SC1001", "SC1002", "SC1003", "SC1007"]
    );
}

/// ADR 0426: `resolve-in-tracked-scope` and `until-in-tracked-scope` disable
/// their own code of the merged rule only; the merged name disables both.
#[test]
fn a_former_name_of_a_merged_rule_disables_its_own_code_only() {
    let finding = |code: &'static str| {
        solid_reactive_ir::Finding::new(
            solid_reactive_ir::RuleMetadata {
                code,
                name: "no-await-primitive-in-tracked-scope",
                severity: "error",
                uncertifiable: false,
                default_enabled: true,
                configurable: true,
                presets: &[],
            },
            "synthetic".into(),
            typefacts::Location {
                path: "synthetic.tsx".into(),
                start_byte: 0,
                end_byte: 1,
            },
        )
    };
    let retained = |disabled: &str| {
        let options = solid_reactive_ir::RuleOptions::parse_with_scoped_aliases(
            &format!(
                r#"{{ "schemaVersion": 1, "rules": {{ {disabled:?}: {{ "enabled": false }} }} }}"#
            ),
            |rule| {
                crate::dialect::ALL
                    .iter()
                    .any(|dialect| (dialect.has_rule)(rule))
            },
            crate::dialect::rule_alias,
            crate::dialect::rule_alias_code,
        )
        .unwrap();
        let mut findings = vec![finding("SC2004"), finding("SC2005")];
        retain_enabled(crate::dialect::default_dialect(), &options, &mut findings).unwrap();
        findings
            .iter()
            .map(|finding| finding.id.as_str().to_owned())
            .collect::<Vec<_>>()
    };
    assert_eq!(retained("resolve-in-tracked-scope"), ["SC2005"]);
    assert_eq!(retained("resolve-in-reactive-scope"), ["SC2005"]);
    assert_eq!(retained("until-in-tracked-scope"), ["SC2004"]);
    assert!(retained("no-await-primitive-in-tracked-scope").is_empty());
}

#[test]
fn diagnostic_session_reuses_the_complete_result() {
    let facts = ProjectFacts {
        generation: Generation::new(1).unwrap(),
        project_id: "/virtual/tsconfig.json".into(),
        files: Vec::new(),
        typescript: TypeScriptTable::from_parts(
            3,
            1,
            "/virtual/tsconfig.json",
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        ),
        typescript_changes: None,
        resolved_imports: None,
        runtime_resolutions: None,
        runtime_symbol_redirects: Default::default(),
        dependency_source_roots: Vec::new(),
        dependency_source_call_errors: Vec::new(),
    };
    let mut session = DiagnosticSession::default();

    let (initial, initial_timings) = session
        .analyze_inputs_measured_with_enablement(
            Path::new(&facts.project_id),
            &[],
            &facts,
            solid_reactive_ir::AnalysisInputs::empty(),
            super::RequestedRuleEnablement::default(),
        )
        .unwrap();
    let (reused, reused_timings) = session
        .analyze_inputs_measured_with_enablement(
            Path::new(&facts.project_id),
            &[],
            &facts,
            solid_reactive_ir::AnalysisInputs::empty(),
            super::RequestedRuleEnablement::default(),
        )
        .unwrap();

    assert!(Arc::ptr_eq(&initial, &reused));
    assert!(!initial_timings.reused);
    assert!(reused_timings.reused);
}

#[test]
fn diagnostic_session_keys_retention_on_requested_enablement() {
    let facts = ProjectFacts {
        generation: Generation::new(1).unwrap(),
        project_id: "/virtual/tsconfig.json".into(),
        files: Vec::new(),
        typescript: TypeScriptTable::from_parts(
            3,
            1,
            "/virtual/tsconfig.json",
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        ),
        typescript_changes: None,
        resolved_imports: None,
        runtime_resolutions: None,
        runtime_symbol_redirects: Default::default(),
        dependency_source_roots: Vec::new(),
        dependency_source_call_errors: Vec::new(),
    };
    let mut session = DiagnosticSession::default();

    let (_, baseline) = session
        .analyze_inputs_measured_with_enablement(
            Path::new(&facts.project_id),
            &[],
            &facts,
            solid_reactive_ir::AnalysisInputs::empty(),
            super::RequestedRuleEnablement::default(),
        )
        .unwrap();
    let (preset_analysis, preset_miss) = session
        .analyze_inputs_measured_with_enablement(
            Path::new(&facts.project_id),
            &[],
            &facts,
            solid_reactive_ir::AnalysisInputs::empty(),
            super::RequestedRuleEnablement {
                presets: &["preferences".into()],
                rules: &[],
                runtime: RuntimeEnvironment::default(),
                feedback_facts: false,
                host_suppressions: &[],
                default_program_boundary: None,
            },
        )
        .unwrap();
    let (preset_reused, preset_hit) = session
        .analyze_inputs_measured_with_enablement(
            Path::new(&facts.project_id),
            &[],
            &facts,
            solid_reactive_ir::AnalysisInputs::empty(),
            super::RequestedRuleEnablement {
                presets: &["preferences".into(), "preferences".into()],
                rules: &[],
                runtime: RuntimeEnvironment::default(),
                feedback_facts: false,
                host_suppressions: &[],
                default_program_boundary: None,
            },
        )
        .unwrap();
    let (_, rule_miss) = session
        .analyze_inputs_measured_with_enablement(
            Path::new(&facts.project_id),
            &[],
            &facts,
            solid_reactive_ir::AnalysisInputs::empty(),
            super::RequestedRuleEnablement {
                presets: &[],
                rules: &["prefer-show".into()],
                runtime: RuntimeEnvironment::default(),
                feedback_facts: false,
                host_suppressions: &[],
                default_program_boundary: None,
            },
        )
        .unwrap();

    assert!(!baseline.reused);
    assert!(
        !preset_miss.reused,
        "a preset change must miss retained analysis"
    );
    assert!(
        preset_hit.reused,
        "duplicate preset values must normalize to one identity"
    );
    assert!(Arc::ptr_eq(&preset_analysis, &preset_reused));
    assert!(
        !rule_miss.reused,
        "an enabled-rule change must miss retained analysis"
    );
}
/// A rule-options document naming a *removed* rule must load, and one
/// naming a rule that never existed must still fail. The first half is the
/// migration path for a project that had disabled a rule this checker went
/// on to delete; the second is what keeps a typo from silently changing
/// policy, which is the reason the validation exists.
#[test]
fn compatibility_rule_identities_are_tolerated_and_typos_are_not() {
    let directory = std::env::temp_dir().join(format!(
        "solid-checker-retired-rules-{}",
        std::process::id()
    ));
    let options_directory = directory.join(".solid-checker");
    std::fs::create_dir_all(&options_directory).unwrap();
    let document = options_directory.join("rule-options.json");

    for retired in crate::dialect::RETIRED_RULES {
        std::fs::write(
            &document,
            format!(
                r#"{{ "schemaVersion": 1, "rules": {{ {:?}: {{ "enabled": false }} }} }}"#,
                retired.0
            ),
        )
        .unwrap();
        let loaded = super::discover_rule_options(&directory);
        assert!(
            loaded.is_ok(),
            "a document disabling the retired {:?} must still load: {loaded:?}",
            retired.0
        );
        // And the identity really is gone: nothing in either catalog
        // declares it, so the disable is a no-op rather than a demotion.
        assert!(
            !crate::dialect::ALL
                .iter()
                .any(|dialect| (dialect.has_rule)(retired.0)),
            "{:?} is retired but still declared by a catalog",
            retired.0
        );
    }

    for (old, current) in crate::dialect::RULE_ALIASES {
        // An alias names a rule in one dialect's catalog, and a
        // single-dialect build (`--no-default-features --features
        // dialect-v2`, which `scripts/verify.sh` checks and which retiring
        // 1.x makes the only build) compiles only one of them. An alias
        // whose target no compiled-in catalog declares is not loadable
        // here and is not this test's subject; the build that carries the
        // target is where it is asserted.
        if !crate::dialect::ALL
            .iter()
            .any(|dialect| (dialect.has_rule)(current))
        {
            continue;
        }
        std::fs::write(
            &document,
            format!(r#"{{ "schemaVersion": 1, "rules": {{ {old:?}: {{ "enabled": false }} }} }}"#),
        )
        .unwrap();
        let loaded = super::discover_rule_options(&directory)
            .unwrap_or_else(|error| panic!("alias {old:?} must load: {error}"));
        if let Some(code) = crate::dialect::rule_alias_code(old) {
            // ADR 0426: a name limited to one code of a merged rule disables
            // that code alone.
            assert!(
                !loaded.is_enabled_for_code(current, code, true, &[])
                    && loaded.is_enabled(current, true, &[]),
                "disabling alias {old:?} must disable {code} and not {current:?}"
            );
        } else {
            assert!(
                !loaded.is_enabled(current, true, &[]),
                "disabling alias {old:?} did not disable {current:?}"
            );
        }
        assert!(
            crate::dialect::ALL
                .iter()
                .any(|dialect| (dialect.has_rule)(current)),
            "alias target {current:?} is absent from every catalog"
        );
        assert!(
            !crate::dialect::ALL
                .iter()
                .any(|dialect| (dialect.has_rule)(old)),
            "alias source {old:?} is still declared by a catalog"
        );
    }

    std::fs::write(
        &document,
        r#"{ "schemaVersion": 1, "rules": { "v1/no-such-rule": { "enabled": false } } }"#,
    )
    .unwrap();
    assert!(super::discover_rule_options(&directory).is_err());

    std::fs::remove_dir_all(&directory).ok();
}

/// ADR 0193: an application is closed-world, a published library is not,
/// and a project with no readable manifest keeps the open boundary.
#[test]
fn the_default_program_boundary_follows_the_nearest_manifest() {
    use solid_reactive_ir::ProgramBoundary::{Closed, Open};
    let root = std::env::temp_dir().join(format!(
        "solid-checker-program-boundary-{}",
        std::process::id()
    ));
    let boundary = |manifest: Option<&str>| {
        let _ = std::fs::remove_dir_all(&root);
        let project = root.join("app/src");
        std::fs::create_dir_all(&project).unwrap();
        if let Some(manifest) = manifest {
            std::fs::write(root.join("app/package.json"), manifest).unwrap();
        }
        super::default_program_boundary(&project.join("tsconfig.json"))
    };
    assert_eq!(
        boundary(Some(r#"{"private": true, "exports": "./x.js"}"#)),
        Closed
    );
    assert_eq!(
        boundary(Some(r#"{"name": "app"}"#)),
        Closed,
        "publishes nothing"
    );
    assert_eq!(
        boundary(Some(r#"{"name": "lib", "exports": "./x.js"}"#)),
        Open
    );
    assert_eq!(boundary(Some(r#"{"name": "lib", "main": "x.js"}"#)), Open);
    assert_eq!(boundary(Some(r#"{"name": "lib", "bin": "x.js"}"#)), Open);
    assert_eq!(boundary(Some("not json")), Open, "an unreadable manifest");
    assert_eq!(boundary(None), Open, "no manifest above the project");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn dispatch_premises_name_what_each_site_withholds_or_why_it_is_excluded() {
    use super::dispatch_premise;
    let site = |receiver: &str, caller: Option<&str>, owner: bool, read: bool| {
        solid_reactive_ir::EventDispatchSite {
            path: "src/a.ts".into(),
            line: 7,
            receiver: receiver.into(),
            caller: caller.map(str::to_owned),
            withholds_owner_proofs: owner,
            withholds_read_proofs: read,
        }
    };
    assert_eq!(
        dispatch_premise(&site("unknown", None, true, true)),
        "synchronous dispatch (ADR 0435) at src/a.ts:7 withholds event handler owner and read proofs: receiver unknown"
    );
    assert_eq!(
        dispatch_premise(&site("element", Some("runWithOwner(null)"), false, true)),
        "synchronous dispatch (ADR 0435) at src/a.ts:7 withholds event handler read proofs: receiver element, caller runWithOwner(null)"
    );
    assert_eq!(
        dispatch_premise(&site("window", None, false, false)),
        "synchronous dispatch (ADR 0435) at src/a.ts:7 excluded: receiver window"
    );
    assert_eq!(
        dispatch_premise(&site("element", Some("scheduled callback"), false, false)),
        "synchronous dispatch (ADR 0435) at src/a.ts:7 excluded: caller scheduled callback"
    );
}

#[test]
fn the_line_index_places_every_offset_where_the_scan_does() {
    use crate::SourceFile;

    use super::{SourcePlacer, source_location};
    let texts = [
        "",
        "one",
        "a\nb\r\nc\n",
        "\n\n\nx",
        "caf\u{e9} \u{1f600} \n\u{1f600}\u{1f600}x\ny",
        "no newline \u{2028} here",
    ];
    let sources = texts
        .iter()
        .enumerate()
        .map(|(index, text)| SourceFile {
            path: format!("/p/{index}.ts"),
            source: (*text).into(),
            compiler_options: Default::default(),
        })
        .collect::<Vec<_>>();
    let mut placer = SourcePlacer::new(&sources);
    for (index, text) in texts.iter().enumerate() {
        for offset in 0..=text.len() + 2 {
            let location = typefacts::Location {
                path: format!("/p/{index}.ts").into(),
                start_byte: offset as u64,
                end_byte: offset as u64,
            };
            let placed = placer.place(&location);
            let scanned = source_location(&location, &sources);
            assert_eq!(
                (placed.line, placed.column),
                (scanned.line, scanned.column),
                "{text:?} at {offset}"
            );
        }
    }
    let unknown = typefacts::Location {
        path: "/p/missing.ts".into(),
        start_byte: 3,
        end_byte: 4,
    };
    let placed = placer.place(&unknown);
    assert_eq!((placed.line, placed.column), (1, 1));
}
