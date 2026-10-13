use super::*;
use solid_facts::{AttestedImport, AttestedImportIndex, ImportResolution, TypeScriptTable};

fn fixture() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../fixtures/package-contracts/standard-router-integration")
}

fn premise() -> IntegrationPremise {
    serde_json::from_slice(&std::fs::read(fixture().join("premise.json")).unwrap()).unwrap()
}

// Explicit normalized Type Facts evidence, independent of source spellings.
// These unit inputs test the consumer's joins; published-typing/runtime pairs
// and the native reproducer independently test the production producer.
fn facts(source: &str, resolved: bool) -> ProjectFacts {
    let ast = solid_facts::ast::extract("setup.ts", source).unwrap();
    let premise = premise();
    let identities = std::collections::BTreeMap::from([
        ("RouterContext", premise.value_references[0].clone()),
        ("createRouter", premise.config_calls[0].declaration.clone()),
        (
            "browserHistory",
            premise.config_calls[0].factories[0].clone(),
        ),
        ("hashHistory", premise.config_calls[0].factories[1].clone()),
        (
            "memoryHistory",
            premise.config_calls[0].factories[2].clone(),
        ),
    ]);
    let mut spans = Vec::new();
    for import in &ast.imports {
        for binding in &import.bindings {
            if let Some(name) = binding
                .imported
                .as_deref()
                .filter(|name| identities.contains_key(name))
            {
                spans.push((binding.local.span, name));
                spans.extend(
                    ast.identifiers
                        .iter()
                        .filter(|id| ast.reference_declaration(id.span) == Some(binding.local.span))
                        .map(|id| (id.span, name)),
                );
            }
        }
    }
    for member in &ast.members {
        let name = &source[member.property.start as usize..member.property.end as usize];
        if identities.contains_key(name) {
            spans.push((member.property, name));
        }
    }
    for export in &ast.exports {
        for specifier in &export.specifiers {
            let span = specifier.local.span;
            let name = &source[span.start as usize..span.end as usize];
            if identities.contains_key(name) {
                spans.push((span, name));
            }
        }
    }
    spans.sort();
    spans.dedup();
    let entities = if resolved {
        spans
            .iter()
            .map(|(span, name)| {
                serde_json::from_value(serde_json::json!({
        "location":{"path":"setup.ts","startByte":span.start,"endByte":span.end},"symbol":name
    })).unwrap()
            })
            .collect()
    } else {
        vec![]
    };
    let symbols = identities
        .iter()
        .map(|(name, id)| typefacts::SymbolFact {
            id: (*name).into(),
            alias_target: "".into(),
            references: vec![].into(),
            declarations: vec![typefacts::Declaration {
                name: "identity-only".into(),
                kind: "variable".into(),
                location: Location {
                    path: format!("/p/node_modules/installed-alias/{}", id.path).into(),
                    start_byte: id.start,
                    end_byte: id.end,
                },
            }]
            .into(),
        })
        .collect();
    let generation = solid_facts::core::Generation::new(1).unwrap();
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
    let mut imports = AttestedImportIndex::default();
    imports.insert_file(
        "setup.ts",
        vec![AttestedImport {
            span: Span::new(0, 1),
            text: "arbitrary".into(),
            resolution: ImportResolution::NodeModules,
            resolved_path: "/p/node_modules/installed-alias/dist/index.d.ts".into(),
            included_path: "".into(),
            symlink_path: "".into(),
            extension: ".d.ts".into(),
            package_name: Some("@solidjs/router".into()),
            package_version: Some("2.0.0-next.26".into()),
            package_manifest: Some("/p/node_modules/installed-alias/package.json".into()),
            resolver_package_name: Some("@solidjs/router".into()),
            resolver_package_version: Some("2.0.0-next.26".into()),
        }],
    );
    ProjectFacts {
        generation,
        project_id: "integration".into(),
        files: vec![FileFacts::new(generation, source, ast, compiler).unwrap()],
        typescript: TypeScriptTable::from_parts(
            2,
            1,
            "integration",
            vec![],
            entities,
            symbols,
            vec![],
        ),
        typescript_changes: None,
        resolved_imports: Some(imports),
        runtime_resolutions: None,
        runtime_symbol_redirects: HashMap::new(),
        dependency_source_roots: Vec::new(),
    }
}

fn vetoed(source: &str, resolved: bool) -> bool {
    matches!(
        scan(
            &facts(source, resolved),
            "@solidjs/router",
            "2.0.0-next.26",
            &premise()
        ),
        PremiseState::Vetoed(_)
    )
}

#[test]
fn authored_integration_fixture_cases_have_only_positive_visible_vetoes() {
    for (case, expected) in [
        ("context", true),
        ("namespace", true),
        ("reexport", true),
        ("custom", true),
        ("nonliteral", true),
        ("spread", true),
        ("computed", true),
        ("standard", false),
        ("unresolved", false),
        ("type-query", false),
    ] {
        let source = std::fs::read_to_string(fixture().join(format!("cases/{case}.ts"))).unwrap();
        assert_eq!(vetoed(&source, true), expected, "{case}");
        assert!(
            !vetoed(&source, false),
            "unresolved {case} must remain outside the premise"
        );
    }
}

#[test]
fn exact_factory_calls_preserve_wrappers_aliases_and_nested_config_properties() {
    for (config, expected) in [
        ("{routes:[]}", false),
        ("{routes:[],history: memoryHistory()}", false),
        ("({routes:[],history: (memoryHistory())})", false),
        ("{routes:[{history: {}}]}", false),
        ("{routes:[], 'history': memoryHistory()}", false),
        ("{routes:[],history: {get:()=>'/',set:()=>{}}}", true),
        ("{routes:[],history}", true),
        ("{routes:[],history: make()}", true),
        ("{routes:[],history: undefined}", true),
        ("{routes:[],get history(){return memoryHistory();}}", true),
        ("{routes:[],...extra}", true),
        ("{routes:[],['history']:memoryHistory()}", true),
        ("config", true),
    ] {
        let source = format!(
            "import {{ createRouter as build, memoryHistory }} from 'anything'; build({config});"
        );
        assert_eq!(vetoed(&source, true), expected, "{config}");
    }
    assert!(vetoed(
        "import {createRouter} from 'anything'; const build=createRouter; build(config);",
        true
    ));
    assert!(!vetoed(
        "import {createRouter} from 'anything'; let build=createRouter; build(config);",
        true
    ));
    assert!(!vetoed(
        "import * as R from 'anything'; R['createRouter'](config);",
        true
    ));
    assert!(vetoed(
        "import * as R from 'anything'; R.createRouter(config);",
        true
    ));
}

#[test]
fn missing_owning_evidence_wrong_versions_and_ambiguous_spans_do_not_veto() {
    let source = "import {RouterContext} from 'anything'; consume(RouterContext);";
    let p = premise();
    let mut missing = facts(source, true);
    missing.resolved_imports = None;
    assert_eq!(
        scan(&missing, "@solidjs/router", "2.0.0-next.26", &p),
        PremiseState::Assumed
    );
    assert_eq!(
        scan(&facts(source, true), "other", "2.0.0-next.26", &p),
        PremiseState::Assumed
    );
    assert_eq!(
        scan(&facts(source, true), "@solidjs/router", "2.0.0-next.25", &p),
        PremiseState::Assumed
    );
    let mut wrong = p.clone();
    wrong.value_references[0].start += 1;
    assert_eq!(
        scan(
            &facts(source, true),
            "@solidjs/router",
            "2.0.0-next.26",
            &wrong
        ),
        PremiseState::Assumed
    );
    let mut duplicate = facts(source, true);
    let mut table = serde_json::to_value(&duplicate.typescript).unwrap();
    let rows = table["entities"].as_array_mut().unwrap();
    rows.extend(rows.clone());
    duplicate.typescript = serde_json::from_value(table).unwrap();
    assert_eq!(
        scan(&duplicate, "@solidjs/router", "2.0.0-next.26", &p),
        PremiseState::Assumed
    );
    let mut conflict = facts(source, true);
    let mut wrong_owner = conflict
        .resolved_imports
        .as_ref()
        .unwrap()
        .iter()
        .next()
        .unwrap()
        .1
        .clone();
    wrong_owner.package_name = Some("unrelated".into());
    conflict
        .resolved_imports
        .as_mut()
        .unwrap()
        .insert_file("other.ts", vec![wrong_owner]);
    assert_eq!(
        scan(&conflict, "@solidjs/router", "2.0.0-next.26", &p),
        PremiseState::Assumed
    );
}
