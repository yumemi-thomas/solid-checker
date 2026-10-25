use super::*;
use typefacts::{Declaration, SymbolFact};

fn table(path: &str, start: u64, end: u64) -> TypeScriptTable {
    TypeScriptTable::from_parts(
        2,
        1,
        "premise",
        vec![],
        vec![],
        vec![
            SymbolFact {
                id: "alias".into(),
                alias_target: "export".into(),
                declarations: Vec::new().into(),
                references: Vec::new().into(),
            },
            SymbolFact {
                id: "export".into(),
                alias_target: "".into(),
                declarations: vec![Declaration {
                    name: "untrusted-name".into(),
                    kind: "variable".into(),
                    location: Location {
                        path: path.into(),
                        start_byte: start,
                        end_byte: end,
                    },
                }]
                .into(),
                references: Vec::new().into(),
            },
        ],
        vec![],
    )
}

#[test]
fn only_positive_exact_alias_resolution_identifies_configuration() {
    let dialect = solid_dialect::Solid2;
    let original = table("/p/node_modules/solid-js/types/index.d.ts", 2531, 2534);
    assert_eq!(
        configuration_api(&original, "alias", &dialect, &[], &mut Vec::new()),
        Some(RuntimeConfigurationApi::DevelopmentHooks)
    );
    assert_eq!(
        configuration_api(&original, "missing", &dialect, &[], &mut Vec::new()),
        None
    );
    for path in [
        "/p/src/index.d.ts",
        "/p/node_modules/ordinary/types/index.d.ts",
    ] {
        assert_eq!(
            configuration_api(
                &table(path, 2531, 2534),
                "alias",
                &dialect,
                &[],
                &mut Vec::new()
            ),
            None
        );
    }
    for target in ["alias", "export", "missing"] {
        let mut encoded = serde_json::to_value(&original).unwrap();
        encoded["symbols"][1]["aliasTarget"] = target.into();
        let cyclic_or_missing: TypeScriptTable = serde_json::from_value(encoded).unwrap();
        assert_eq!(
            configuration_api(&cyclic_or_missing, "alias", &dialect, &[], &mut Vec::new()),
            None
        );
    }
    let mut encoded = serde_json::to_value(&original).unwrap();
    let extra = encoded["symbols"][1]["declarations"][0].clone();
    encoded["symbols"][1]["declarations"]
        .as_array_mut()
        .unwrap()
        .push(extra);
    let merged: TypeScriptTable = serde_json::from_value(encoded).unwrap();
    assert_eq!(
        configuration_api(&merged, "alias", &dialect, &[], &mut Vec::new()),
        None
    );
    let roots = [("/p/node_modules/alias".into(), "solid-js".into())];
    assert_eq!(
        configuration_api(
            &table("/p/node_modules/alias/types/index.d.ts", 2531, 2534),
            "alias",
            &dialect,
            &roots,
            &mut Vec::new()
        ),
        Some(RuntimeConfigurationApi::DevelopmentHooks)
    );
    let roots = [("/p/node_modules/solid-js".into(), "ordinary".into())];
    assert_eq!(
        configuration_api(&original, "alias", &dialect, &roots, &mut Vec::new()),
        None
    );
}

#[test]
fn assumed_is_default_and_projection_is_byte_identical() {
    let mut finding = crate::Finding::new(
        crate::RuleMetadata {
            code: "SC1001",
            name: "strict-read-untracked",
            severity: "error",
            uncertifiable: false,
            default_enabled: true,
            presets: &[],
        },
        "original wording".into(),
        Span::new(1, 2).location("App.tsx"),
    );
    let before = serde_json::to_vec(&finding).unwrap();
    RuntimeConfigurationPremise::default().apply(&mut finding);
    assert_eq!(serde_json::to_vec(&finding).unwrap(), before);
    let site = Span::new(10, 13).location("setup.ts");
    RuntimeConfigurationPremise::Vetoed {
        location: site.clone(),
    }
    .apply(&mut finding);
    assert_eq!(finding.kind, "uncertifiable");
    assert_eq!(finding.related_locations.last(), Some(&site));
    assert_eq!(finding.evidence[0].location.as_ref(), Some(&site));
    assert!(finding.fixes.is_empty());
    assert!(RuntimeConfigurationPremise::default().permits_proof());
}

fn lookup_project(source: &str, declaration_path: &str, start: u64, end: u64) -> ProjectFacts {
    let ast = solid_facts::ast::extract("setup.tsx", source).unwrap();
    let span = if let Some(member) = ast.members.first() {
        member.property
    } else {
        ast.imports[0].bindings[0].local.span
    };
    let mut encoded = serde_json::to_value(table(declaration_path, start, end)).unwrap();
    encoded["entities"] = serde_json::json!([{
        "location": { "path": "setup.tsx", "startByte": span.start, "endByte": span.end },
        "symbol": "alias"
    }]);
    let compiler = solid_facts::compiler::ExecutionMap {
        compiler_facts_protocol: solid_facts::compiler::COMPILER_FACTS_PROTOCOL,
        source_hash: ast.source.hash.clone(),
        semantic_model: Default::default(),
        tracked_regions: vec![],
        untracked_regions: vec![],
        discarded_regions: vec![],
        ownership_regions: vec![],
        callback_roles: vec![],
        jsx_operations: vec![],
    };
    let generation = solid_facts::core::Generation::new(1).unwrap();
    ProjectFacts {
        generation,
        project_id: "lookup".into(),
        files: vec![solid_facts::FileFacts::new(generation, source, ast, compiler).unwrap()],
        typescript: serde_json::from_value(encoded).unwrap(),
        typescript_changes: None,
        resolved_imports: None,
        runtime_resolutions: None,
        runtime_symbol_redirects: std::collections::HashMap::new(),
        dependency_source_roots: Vec::new(),
        dependency_source_call_errors: Vec::new(),
    }
}

fn hydration_project(source: &str) -> ProjectFacts {
    let mut facts = lookup_project(
        source,
        "/p/node_modules/solid-js/types/internal.d.ts",
        7472,
        7484,
    );
    let ast = &facts.files[0].ast;
    let binding = &ast.imports[0].bindings[0];
    let span = if binding.kind == ImportKind::Namespace {
        ast.members
            .iter()
            .find(|member| {
                ast.reference_declaration(ast.peel_ts_sugar_span(member.object))
                    == Some(binding.local.span)
            })
            .map_or(binding.local.span, |member| member.property)
    } else {
        binding.local.span
    };
    let mut encoded = serde_json::to_value(&facts.typescript).unwrap();
    encoded["entities"] = serde_json::json!([{
        "location": { "path": "setup.tsx", "startByte": span.start, "endByte": span.end },
        "symbol": "alias"
    }]);
    facts.typescript = serde_json::from_value(encoded).unwrap();
    facts
}

#[test]
fn hydration_host_vetoes_transport_and_writes_but_keeps_data_reads() {
    for (usage, permits) in [
        ("host.load = id => id;", false),
        ("host.has = () => true;", false),
        ("host.gather = () => {};", false),
        ("const alias = host;", false),
        ("export { host };", false),
        ("const { load } = host;", false),
        ("Object.assign(host, {});", false),
        ("Object.defineProperty(host, 'load', {});", false),
        ("consume(host);", false),
        ("consume(host.context);", false),
        ("host.context.serialize('id', 1);", false),
        ("host.hydrating = true;", false),
        ("if (host.hydrating = true) consume(1);", false),
        ("const { hydrating } = host;", false),
        ("host.registry = new Map();", false),
        ("host.context = undefined;", false),
        ("out[host.hydrating ? 1 : 0] = 1;", true),
        ("out[host.done ? 1 : 0]++;", true),
        ("delete out[host.done ? 1 : 0];", true),
        ("for (out[host.done ? 1 : 0] of values) {}", true),
        ("({ a: local = host.hydrating } = value);", true),
        ("({ [host.done ? 'a' : 'b']: local } = value);", true),
        ("out[host.done = true] = 1;", false),
        ("(host.hydrating as boolean) = true;", false),
        ("host.done ||= true;", false),
        ("delete host.done;", false),
        ("({ x: host.done } = value);", false),
        ("for (host.done of values) {}", false),
        ("host['load'] = () => 1;", false),
        ("host[key] = value;", false),
        ("host.load?.('id');", false),
        ("const value = host.hydrating;", true),
        ("consume(!host.hydrating);", true),
        ("const value = host.done;", true),
        ("if (host.context) consume(1);", true),
        ("const absent = !(host.context);", true),
        ("host.context && consume(1);", true),
        ("const flag = host.context ? 1 : 0;", true),
        ("if (host) consume(1);", true),
        ("if (host.context) host.load = () => 1;", false),
        ("const value = (host as any).hydrating;", true),
    ] {
        let source = format!("import {{ sharedConfig as host }} from 'solid-js/internal'; {usage}");
        assert_eq!(
            scan(&hydration_project(&source), &solid_dialect::Solid2).permits_proof(),
            permits,
            "{usage}"
        );
    }
    for (usage, permits) in [
        ("H.sharedConfig.load = () => 1;", false),
        ("const alias = H.sharedConfig;", false),
        ("const flag = H.sharedConfig.hydrating;", true),
        ("if ((H.sharedConfig as any).context) consume(1);", true),
        ("type Host = typeof H.sharedConfig;", true),
    ] {
        let source = format!("import * as H from 'solid-js/internal'; {usage}");
        assert_eq!(
            scan(&hydration_project(&source), &solid_dialect::Solid2).permits_proof(),
            permits,
            "{usage}"
        );
    }
    for source in [
        "import type { sharedConfig as host } from 'solid-js/internal'; type T = typeof host;",
        "import { sharedConfig as host } from 'solid-js/internal'; type T = typeof host;",
        "import { sharedConfig as host } from 'solid-js/internal'; function f(host: any) { host.load = () => 1; }",
    ] {
        assert!(scan(&hydration_project(source), &solid_dialect::Solid2).permits_proof());
    }
    let mut unresolved = hydration_project(
        "import { sharedConfig as host } from 'solid-js/internal'; host.load = () => 1;",
    );
    unresolved.typescript = table("/p/src/local.d.ts", 7472, 7484);
    assert!(scan(&unresolved, &solid_dialect::Solid2).permits_proof());
}

#[test]
fn a_process_free_project_without_resolution_evidence_keeps_the_premise() {
    let facts = ProjectFacts {
        generation: solid_facts::core::Generation::new(1).unwrap(),
        project_id: "premise".into(),
        files: vec![],
        typescript: table("/p/src/local.d.ts", 1, 4),
        typescript_changes: None,
        resolved_imports: None,
        runtime_resolutions: None,
        runtime_symbol_redirects: std::collections::HashMap::new(),
        dependency_source_roots: Vec::new(),
        dependency_source_call_errors: Vec::new(),
    };
    assert_eq!(
        scan(&facts, &solid_dialect::Solid2),
        RuntimeConfigurationPremise::Assumed
    );
}

fn at_generation(mut facts: ProjectFacts, generation: u64) -> ProjectFacts {
    let generation = solid_facts::core::Generation::new(generation).unwrap();
    facts.generation = generation;
    for file in &mut facts.files {
        file.generation = generation;
    }
    facts
}

/// A retained per-file answer is the full scan's whenever it is reused, and
/// is withdrawn by anything that could move it: the file's source, its
/// entities, or the configuration identity of a symbol it asked about. No
/// change set is trusted: each input is checked against the current facts.
#[test]
fn a_retained_scan_answers_as_a_full_scan_and_withdraws_on_any_input_change() {
    let dialect = &solid_dialect::Solid2;
    let veto = "import { sharedConfig as host } from 'solid-js/internal'; host.load = () => 1;";
    let keep =
        "import { sharedConfig as host } from 'solid-js/internal'; const value = host.hydrating;";
    // Reuse agrees with a full scan, case by case, when nothing moved.
    for source in [veto, keep] {
        let mut cache = ScanCache::default();
        let facts = hydration_project(source);
        let first = scan_retained(&at_generation(facts.clone(), 1), dialect, &mut cache);
        let reused = scan_retained(&at_generation(facts, 2), dialect, &mut cache);
        assert_eq!(first, scan(&hydration_project(source), dialect), "{source}");
        assert_eq!(reused, first, "{source}");
    }
    // A changed source is rescanned.
    let mut cache = ScanCache::default();
    assert!(
        !scan_retained(
            &at_generation(hydration_project(veto), 1),
            dialect,
            &mut cache
        )
        .permits_proof()
    );
    assert!(
        scan_retained(
            &at_generation(hydration_project(keep), 2),
            dialect,
            &mut cache
        )
        .permits_proof()
    );
    // The same source whose symbol no longer resolves to the hydration host
    // (a different table) is rescanned, whatever any change set says.
    let mut cache = ScanCache::default();
    let facts = hydration_project(veto);
    assert!(!scan_retained(&at_generation(facts.clone(), 1), dialect, &mut cache).permits_proof());
    let mut moved = facts;
    moved.typescript = table("/p/src/local.d.ts", 7472, 7484);
    assert!(scan_retained(&at_generation(moved, 2), dialect, &mut cache).permits_proof());
}

/// The table of `facts` with `edit` applied to its encoded symbols.
fn with_symbols(
    mut facts: ProjectFacts,
    edit: impl FnOnce(&mut Vec<serde_json::Value>),
) -> ProjectFacts {
    let mut encoded = serde_json::to_value(&facts.typescript).unwrap();
    let mut symbols = encoded["symbols"].as_array().cloned().unwrap_or_default();
    edit(&mut symbols);
    encoded["symbols"] = serde_json::Value::Array(symbols);
    facts.typescript = serde_json::from_value(encoded).unwrap();
    facts
}

/// A kept identity is asked again only when a row its alias chain read
/// moved: a symbol no chain read moves nothing, and a moved chain member
/// rescans exactly the files that asked, with the full scan's answer.
#[test]
fn a_kept_identity_is_asked_again_only_when_its_chain_moved() {
    let dialect = &solid_dialect::Solid2;
    let veto = "import { sharedConfig as host } from 'solid-js/internal'; host.load = () => 1;";
    let facts = hydration_project(veto);
    let mut cache = ScanCache::default();
    let first = scan_retained(&at_generation(facts.clone(), 1), dialect, &mut cache);
    assert!(!first.permits_proof());
    assert_eq!(cache.last_counts, (0, 1));
    // A symbol nothing asked about appears: the file is kept.
    let unrelated = with_symbols(facts.clone(), |symbols| {
        symbols.push(serde_json::json!({ "id": "unrelated" }));
    });
    let kept = scan_retained(&at_generation(unrelated.clone(), 2), dialect, &mut cache);
    assert_eq!(kept, first);
    assert_eq!(cache.last_counts, (1, 0));
    // The alias's target, a row its chain read, now declares elsewhere: the
    // file is rescanned and agrees with a full scan.
    let moved = with_symbols(unrelated, |symbols| {
        for symbol in symbols.iter_mut() {
            if symbol["id"] == "export" {
                symbol["declarations"][0]["location"]["path"] = "/p/src/local.d.ts".into();
            }
        }
    });
    let rescanned = scan_retained(&at_generation(moved.clone(), 3), dialect, &mut cache);
    assert_eq!(cache.last_counts, (0, 1));
    assert_eq!(rescanned, scan(&moved, dialect));
    assert!(rescanned.permits_proof());
}
