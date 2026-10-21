use crate::HashSet;

use super::cache::{
    CachedTypeScriptIndexes, InterproceduralResultDependency, InterproceduralResultDependencyState,
    SourceDiscoveryIdentity, SourceDiscoveryTypeScriptDelta,
};
use solid_facts::TypeScriptTable;
use solid_facts::core::SourceHash;
use typefacts::{Declaration, EntityFact, FileFact, SourceDigest, SymbolFact};

use super::interproc::InterproceduralResultView;
use super::pipeline::{AnalysisWorkerLimit, parallel_slice_results};
use super::source_discovery::source_discovery_identity_matches;
use super::symbols::{
    alias_roots_and_source_declarations, entity_symbols, patch_typescript_indexes,
    references_for_sources, source_discovery_symbol_semantics, symbol_alias_targets, symbol_names,
    symbols_by_root,
};
use super::*;

#[test]
fn ordered_parallel_maps_respect_the_worker_budget() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    let items = (0..512).collect::<Vec<_>>();
    let active = AtomicUsize::new(0);
    let peak = AtomicUsize::new(0);
    let parallel = {
        let _worker_limit = AnalysisWorkerLimit::enter(2);
        parallel_slice_results(&items, |item| {
            let current = active.fetch_add(1, Ordering::SeqCst) + 1;
            peak.fetch_max(current, Ordering::SeqCst);
            for _ in 0..32 {
                std::thread::yield_now();
            }
            active.fetch_sub(1, Ordering::SeqCst);
            item * 2
        })
    };
    let sequential = {
        let _worker_limit = AnalysisWorkerLimit::enter(1);
        parallel_slice_results(&items, |item| item * 2)
    };

    assert_eq!(parallel, sequential);
    assert!(peak.load(Ordering::SeqCst) <= 2);
    assert_eq!(active.load(Ordering::SeqCst), 0);
}

#[test]
fn primitive_names_resolve_through_the_dialect() {
    assert!(matches!(
        PrimitiveName::new("createEffect", &solid_dialect::Solid2),
        PrimitiveName::Known(Primitive::CreateEffect, "createEffect")
    ));
    // A name outside the dialect's vocabulary keeps its own spelling and
    // answers no vocabulary question.
    assert!(matches!(
        PrimitiveName::new("projectSpecificHelper", &solid_dialect::Solid2),
        PrimitiveName::Other(_)
    ));
    // `flush` is 2.0 vocabulary; `batch` is 1.x's and 2.0 does not have
    // it, so the same spelling resolves in one vocabulary and not another.
    // This used to be shown against the 1.x dialect directly; with one
    // vocabulary it is shown the way a consumer would meet it.
    assert!(matches!(
        PrimitiveName::new("flush", &solid_dialect::Solid2),
        PrimitiveName::Known(Primitive::Flush, "flush")
    ));
    assert!(matches!(
        PrimitiveName::new("batch", &solid_dialect::Solid2),
        PrimitiveName::Other(_)
    ));
}

#[test]
fn runtime_environment_requires_exact_noncontradictory_selection() {
    let mut environment = RuntimeEnvironment {
        target: Some(RuntimeTarget::Browser),
        build: Some(RuntimeBuild::Production),
        rendering: Some(RuntimeRendering::Csr),
        conditions: BTreeSet::from(["import".into()]),
        framework_transforms: BTreeSet::from(["use-server".into()]),
        program_boundary: None,
        router_install: router_params::RouterInstallEvidence::default(),
        package_instances: PackageInstanceEvidence::default(),
    };
    assert!(environment.validate().is_ok());
    assert_eq!(
        environment.selected_conditions(),
        BTreeSet::from([
            "browser".into(),
            "import".into(),
            "production".into(),
            "csr".into(),
            "use-server".into()
        ])
    );
    // The program boundary is a build-wide premise, not a package export
    // condition. Artifact/guard selection happens before the normalized
    // accepted index reaches the analyzer.
    environment.program_boundary = Some(ProgramBoundary::Closed);
    assert!(environment.validate().is_ok());
    assert!(!environment.selected_conditions().contains("closed"));
    assert!(environment.program_is_closed());
    environment.program_boundary = Some(ProgramBoundary::Open);
    assert!(!environment.program_is_closed());
    environment.program_boundary = None;
    assert!(!environment.program_is_closed());
    // Resolver-only condition labels do not get synthesized here.
    assert!(!environment.selected_conditions().contains("default"));

    environment.target = Some(RuntimeTarget::Node);
    assert!(environment.validate().is_err());
    environment.target = Some(RuntimeTarget::Browser);
    environment.conditions.insert(String::new());
    assert!(environment.validate().is_err());
    environment.conditions.remove("");
    environment.conditions.insert("node".into());
    assert!(environment.validate().is_err());
    environment.conditions.remove("node");
    environment.conditions.insert("development".into());
    assert!(environment.validate().is_err());
}

fn summary_node(path: &str, span: Span, body: Span) -> SummaryNode {
    SummaryNode {
        path: path.into(),
        span,
        body,
        name: None,
        symbol: None,
        runtime_identity: String::new(),
        parameters: Vec::new(),
        exported: false,
        r#async: false,
        source_hash: solid_facts::core::SourceHash::of(""),
    }
}

fn summary_read(symbol: &str, display: &str, start: u64) -> SummaryRead {
    SummaryRead {
        transported: false,
        symbol: symbol.into(),
        display: display.into(),
        kind: Some("accessor".into()),
        declaration: Location {
            path: "fixture.tsx".into(),
            start_byte: start,
            end_byte: start + 1,
        },
        origin: Location {
            path: "fixture.tsx".into(),
            start_byte: start + 10,
            end_byte: start + 11,
        },
        origin_context: symbol.into(),
        owner: None,
        local: false,
        conditional: false,
        undecided: false,
    }
}

fn declaration(name: &str, path: &str, start: u64) -> Declaration {
    Declaration {
        name: name.into(),
        kind: "const".into(),
        location: Location {
            path: path.into(),
            start_byte: start,
            end_byte: start + 1,
        },
    }
}

fn typescript_table(
    generation: u64,
    sources: Vec<SourceDigest>,
    entities: Vec<EntityFact>,
    symbols: Vec<SymbolFact>,
    files: Vec<FileFact>,
) -> TypeScriptTable {
    TypeScriptTable::from_parts(3, generation, "fixture", sources, entities, symbols, files)
}

fn empty_project(generation: u64) -> ProjectFacts {
    ProjectFacts {
        generation: solid_facts::core::Generation::new(generation).unwrap(),
        project_id: "fixture".into(),
        files: Vec::new(),
        typescript: typescript_table(generation, Vec::new(), Vec::new(), Vec::new(), Vec::new()),
        typescript_changes: None,
        resolved_imports: None,
        runtime_resolutions: None,
        runtime_symbol_redirects: std::collections::HashMap::default(),
        dependency_source_roots: Vec::new(),
    }
}

#[test]
fn projected_references_include_every_alias_member_without_retaining_unrelated_symbols() {
    let reference = |start| Location {
        path: "fixture.ts".into(),
        start_byte: start,
        end_byte: start + 1,
    };
    let table = typescript_table(
        1,
        Vec::new(),
        Vec::new(),
        vec![
            SymbolFact {
                id: "root".into(),
                alias_target: "".into(),
                declarations: Vec::new().into(),
                references: vec![reference(30)].into(),
            },
            SymbolFact {
                id: "alias".into(),
                alias_target: "root".into(),
                declarations: Vec::new().into(),
                references: vec![reference(10)].into(),
            },
            SymbolFact {
                id: "unrelated".into(),
                alias_target: "".into(),
                declarations: Vec::new().into(),
                references: vec![reference(20)].into(),
            },
        ],
        Vec::new(),
    );
    let interner = SymbolInterner::from_table(&table);
    let (aliases, _) = alias_roots_and_source_declarations(
        &table,
        &interner,
        &std::collections::HashMap::default(),
    );
    let roots = symbols_by_root(&table, &aliases, &interner);
    let source = SymbolId::from("root");
    let projected = references_for_sources(&table, &roots, std::iter::once(&source));

    assert_eq!(
        projected[&source]
            .iter()
            .map(|location| location.start_byte)
            .collect::<Vec<_>>(),
        vec![10, 30]
    );
    assert_eq!(projected.len(), 1);
}

#[test]
fn exact_runtime_redirects_replace_declaration_alias_roots() {
    let table = typescript_table(
        1,
        Vec::new(),
        Vec::new(),
        vec![
            SymbolFact {
                id: "import-alias".into(),
                alias_target: "declaration".into(),
                declarations: Vec::new().into(),
                references: Vec::new().into(),
            },
            SymbolFact {
                id: "declaration".into(),
                alias_target: "".into(),
                declarations: Vec::new().into(),
                references: Vec::new().into(),
            },
            SymbolFact {
                id: "runtime".into(),
                alias_target: "".into(),
                declarations: Vec::new().into(),
                references: Vec::new().into(),
            },
        ],
        Vec::new(),
    );
    let interner = SymbolInterner::from_table(&table);
    let redirects = std::collections::HashMap::from([("declaration".into(), "runtime".into())]);
    let (aliases, _) = alias_roots_and_source_declarations(&table, &interner, &redirects);

    assert_eq!(aliases["import-alias"].as_str(), "runtime");
    assert_eq!(aliases["declaration"].as_str(), "runtime");
    assert_eq!(aliases["runtime"].as_str(), "runtime");
}

#[test]
fn compact_source_identity_reuses_only_exact_typefacts_manifests() {
    let source_hash = SourceHash::of("source");
    let cached = SourceDiscoveryIdentity {
        source_hash: source_hash.clone(),
        symbols: vec![SymbolId::from("source-symbol")],
    };
    let exact = SourceDiscoveryTypeScriptDelta {
        entity_paths: HashSet::default(),
        file_paths: HashSet::default(),
        semantic_symbol_ids: HashSet::default(),
    };
    assert!(source_discovery_identity_matches(
        &cached,
        "fixture.ts",
        &source_hash,
        false,
        Some(&exact),
    ));
    assert!(!source_discovery_identity_matches(
        &cached,
        "fixture.ts",
        &source_hash,
        false,
        None,
    ));

    let affected = SourceDiscoveryTypeScriptDelta {
        entity_paths: HashSet::from_iter(["fixture.ts".into()]),
        file_paths: HashSet::default(),
        semantic_symbol_ids: HashSet::default(),
    };
    assert!(!source_discovery_identity_matches(
        &cached,
        "fixture.ts",
        &source_hash,
        false,
        Some(&affected),
    ));
    let changed_symbol = SourceDiscoveryTypeScriptDelta {
        entity_paths: HashSet::default(),
        file_paths: HashSet::default(),
        semantic_symbol_ids: HashSet::from_iter([SymbolId::from("source-symbol")]),
    };
    assert!(!source_discovery_identity_matches(
        &cached,
        "fixture.ts",
        &source_hash,
        false,
        Some(&changed_symbol),
    ));
}

fn typescript_index_cache(table: &TypeScriptTable) -> CachedTypeScriptIndexes {
    let interner = SymbolInterner::from_table(table);
    let (aliases, source_declarations) = alias_roots_and_source_declarations(
        table,
        &interner,
        &std::collections::HashMap::default(),
    );
    CachedTypeScriptIndexes {
        symbol_alias_targets: symbol_alias_targets(table, &interner),
        symbols_by_root: symbols_by_root(table, &aliases, &interner),
        entities: entity_symbols(table, &aliases, &interner),
        symbol_names: symbol_names(table, &aliases, &interner, &solid_dialect::Solid2),
        source_discovery_symbol_semantics: source_discovery_symbol_semantics(table, &interner),
        source_discovery_delta: None,
        aliases,
        source_declarations,
        interner,
    }
}

#[test]
fn incremental_builder_reuses_only_the_same_coherent_generation() {
    let first = empty_project(1);
    let fresh = build(&first, &solid_dialect::Solid2).unwrap();
    let mut incremental = IncrementalBuilder::default();

    let (initial, initial_timings) = incremental.build(&first, &solid_dialect::Solid2).unwrap();
    let (reused, reused_timings) = incremental.build(&first, &solid_dialect::Solid2).unwrap();
    let mut next_facts = empty_project(2);
    next_facts.typescript_changes = Some(solid_facts::TypeScriptChanges {
        unchanged: true,
        ..solid_facts::TypeScriptChanges::default()
    });
    let (next, next_timings) = incremental
        .build(&next_facts, &solid_dialect::Solid2)
        .unwrap();

    assert_eq!(initial, fresh);
    assert_eq!(reused, fresh);
    assert_eq!(next, fresh);
    assert!(!initial_timings.reused);
    assert!(reused_timings.reused);
    assert!(!next_timings.reused);
    assert!(next_timings.typescript_indexes_reused);
}

#[test]
fn idle_retention_preserves_results_and_rebuilds_released_indexes() {
    let first = empty_project(1);
    let fresh = build(&first, &solid_dialect::Solid2).unwrap();
    let mut incremental = IncrementalBuilder::default();

    let (initial, _) = incremental.build(&first, &solid_dialect::Solid2).unwrap();
    incremental.retain_for_idle(CacheRetention::Balanced);
    let (same_generation, same_timings) =
        incremental.build(&first, &solid_dialect::Solid2).unwrap();
    incremental.retain_for_idle(CacheRetention::Compact);

    let mut next_facts = empty_project(2);
    next_facts.typescript_changes = Some(solid_facts::TypeScriptChanges {
        unchanged: true,
        ..solid_facts::TypeScriptChanges::default()
    });
    let (next, next_timings) = incremental
        .build(&next_facts, &solid_dialect::Solid2)
        .unwrap();

    assert_eq!(initial, fresh);
    assert_eq!(same_generation, fresh);
    assert_eq!(next, fresh);
    assert!(same_timings.reused);
    assert!(!next_timings.reused);
    assert!(!next_timings.typescript_indexes_reused);
}

#[test]
fn shared_builder_reuses_the_program_allocation() {
    let facts = empty_project(1);
    let mut incremental = IncrementalBuilder::default();

    let (initial, initial_timings) = incremental
        .build_shared(&facts, &solid_dialect::Solid2)
        .unwrap();
    let (reused, reused_timings) = incremental
        .build_shared(&facts, &solid_dialect::Solid2)
        .unwrap();

    assert!(Arc::ptr_eq(&initial, &reused));
    assert!(!initial_timings.reused);
    assert!(reused_timings.reused);
}

#[test]
fn source_declaration_index_skips_earlier_dts_only_symbols() {
    let table = typescript_table(
        1,
        Vec::new(),
        Vec::new(),
        vec![
            typefacts::SymbolFact {
                id: "early".into(),
                alias_target: "root".into(),
                declarations: (vec![declaration("Accessor", "solid-js.d.ts", 1)]).into(),
                references: (Vec::new()).into(),
            },
            typefacts::SymbolFact {
                id: "later".into(),
                alias_target: "root".into(),
                declarations: (vec![
                    declaration("Accessor", "other.d.ts", 2),
                    declaration("sourceAccessor", "source.ts", 3),
                ])
                .into(),
                references: (Vec::new()).into(),
            },
        ],
        Vec::new(),
    );

    let interner = SymbolInterner::from_table(&table);
    let (_, declarations) = alias_roots_and_source_declarations(
        &table,
        &interner,
        &std::collections::HashMap::default(),
    );

    assert_eq!(declarations["root"].name, ("sourceAccessor").into());
    assert_eq!(declarations["root"].location.path, ("source.ts").into());
}

#[test]
fn exact_index_patch_replaces_local_alias_ids_without_retargeting_the_graph() {
    let symbol = |id: &str, target: &str, declarations: Vec<Declaration>| SymbolFact {
        id: id.into(),
        alias_target: target.into(),
        declarations: declarations.into(),
        references: Vec::new().into(),
    };
    let entity = |symbol: &str, start: u64| EntityFact {
        location: Location {
            path: "fixture.ts".into(),
            start_byte: start,
            end_byte: start + 1,
        },
        symbol: symbol.into(),
        symbol_unresolved: false,
        type_descriptor: None,
        resolved_call: None,
        callability: None,
        constructability: None,
        runtime_binding_kind: None,
        runtime_value_domain: None,
        primitive_value_domain: typefacts::PrimitiveValueDomain::default(),
        primitive_literal_candidates: None,
        call_result_domain: None,
        constant_value: None,
        array_shape: None,
        tuple_shape: None,
        library_types: None,
        reference_space: None,
        runtime_identity: "".into(),
    };
    let old = typescript_table(
        1,
        Vec::new(),
        vec![entity("old-alias", 10)],
        vec![
            symbol("root", "", vec![declaration("root", "fixture.ts", 1)]),
            SymbolFact {
                references: vec![Location {
                    path: "fixture.ts".into(),
                    start_byte: 10,
                    end_byte: 11,
                }]
                .into(),
                ..symbol(
                    "old-alias",
                    "root",
                    vec![declaration("root", "fixture.d.ts", 2)],
                )
            },
        ],
        Vec::new(),
    );
    let current = typescript_table(
        2,
        Vec::new(),
        vec![entity("new-alias", 12)],
        vec![
            symbol("root", "", vec![declaration("root", "fixture.ts", 3)]),
            SymbolFact {
                references: vec![
                    Location {
                        path: "fixture.ts".into(),
                        start_byte: 13,
                        end_byte: 14,
                    },
                    Location {
                        path: "fixture.ts".into(),
                        start_byte: 12,
                        end_byte: 13,
                    },
                    Location {
                        path: "fixture.ts".into(),
                        start_byte: 12,
                        end_byte: 13,
                    },
                ]
                .into(),
                ..symbol(
                    "new-alias",
                    "root",
                    vec![declaration("root", "fixture.d.ts", 4)],
                )
            },
        ],
        Vec::new(),
    );
    let symbols_by_id = current
        .symbols()
        .map(|symbol| (symbol.id(), symbol))
        .collect::<HashMap<_, _>>();
    let mut patched = typescript_index_cache(&old);
    let changes = solid_facts::TypeScriptChanges {
        unchanged: false,
        entity_paths: vec!["fixture.ts".into()],
        symbol_ids: vec!["new-alias".into(), "old-alias".into(), "root".into()],
        file_paths: Vec::new(),
    };

    assert!(
        patch_typescript_indexes(
            &mut patched,
            &current,
            &symbols_by_id,
            &solid_dialect::Solid2,
            &changes
        )
        .is_some()
    );
    let fresh = typescript_index_cache(&current);
    assert_eq!(patched.symbol_alias_targets, fresh.symbol_alias_targets);
    assert_eq!(patched.aliases, fresh.aliases);
    assert_eq!(patched.source_declarations, fresh.source_declarations);
    assert_eq!(patched.entities, fresh.entities);
    assert_eq!(patched.symbol_names, fresh.symbol_names);
    assert_eq!(
        patched.source_discovery_symbol_semantics,
        fresh.source_discovery_symbol_semantics
    );
    assert_eq!(
        patched
            .source_discovery_delta
            .as_ref()
            .unwrap()
            .semantic_symbol_ids,
        ["new-alias", "old-alias"]
            .into_iter()
            .map(SymbolId::from)
            .collect()
    );
}

#[test]
fn exact_index_patch_does_not_treat_references_as_source_discovery_semantics() {
    let table = |reference_start| {
        typescript_table(
            1,
            Vec::new(),
            Vec::new(),
            vec![SymbolFact {
                id: "root".into(),
                alias_target: (String::new()).into(),
                declarations: (vec![declaration("root", "fixture.ts", 1)]).into(),
                references: (vec![Location {
                    path: "fixture.ts".into(),
                    start_byte: reference_start,
                    end_byte: reference_start + 1,
                }])
                .into(),
            }],
            Vec::new(),
        )
    };
    let old = table(10);
    let current = table(20);
    let symbols_by_id = current
        .symbols()
        .map(|symbol| (symbol.id(), symbol))
        .collect::<HashMap<_, _>>();
    let mut patched = typescript_index_cache(&old);
    let changes = solid_facts::TypeScriptChanges {
        unchanged: false,
        entity_paths: Vec::new(),
        symbol_ids: vec!["root".into()],
        file_paths: Vec::new(),
    };

    assert!(
        patch_typescript_indexes(
            &mut patched,
            &current,
            &symbols_by_id,
            &solid_dialect::Solid2,
            &changes
        )
        .is_some()
    );
    assert!(
        patched
            .source_discovery_delta
            .as_ref()
            .unwrap()
            .semantic_symbol_ids
            .is_empty()
    );
}

#[test]
fn exact_index_patch_does_not_treat_declaration_offsets_as_source_semantics() {
    let table = |start| {
        typescript_table(
            1,
            Vec::new(),
            Vec::new(),
            vec![SymbolFact {
                id: "root".into(),
                alias_target: (String::new()).into(),
                declarations: (vec![declaration("root", "fixture.ts", start)]).into(),
                references: (Vec::new()).into(),
            }],
            Vec::new(),
        )
    };
    let old = table(10);
    let current = table(30);
    let symbols_by_id = current
        .symbols()
        .map(|symbol| (symbol.id(), symbol))
        .collect::<HashMap<_, _>>();
    let mut patched = typescript_index_cache(&old);
    let changes = solid_facts::TypeScriptChanges {
        unchanged: false,
        entity_paths: Vec::new(),
        symbol_ids: vec!["root".into()],
        file_paths: Vec::new(),
    };

    assert!(
        patch_typescript_indexes(
            &mut patched,
            &current,
            &symbols_by_id,
            &solid_dialect::Solid2,
            &changes
        )
        .is_some()
    );
    assert_eq!(
        patched.source_declarations,
        typescript_index_cache(&current).source_declarations
    );
    assert!(
        patched
            .source_discovery_delta
            .as_ref()
            .unwrap()
            .semantic_symbol_ids
            .is_empty(),
        "moving a declaration without changing its source semantics must not invalidate importers"
    );
}

#[test]
fn exact_index_patch_does_not_invalidate_when_a_runtime_representative_moves_files() {
    let table = |path| {
        typescript_table(
            1,
            Vec::new(),
            Vec::new(),
            vec![SymbolFact {
                id: "root".into(),
                alias_target: (String::new()).into(),
                declarations: (vec![declaration("createSignal", path, 10)]).into(),
                references: (Vec::new()).into(),
            }],
            Vec::new(),
        )
    };
    let old = table("a.ts");
    let current = table("b.ts");
    let symbols_by_id = current
        .symbols()
        .map(|symbol| (symbol.id(), symbol))
        .collect::<HashMap<_, _>>();
    let mut patched = typescript_index_cache(&old);
    let changes = solid_facts::TypeScriptChanges {
        unchanged: false,
        entity_paths: Vec::new(),
        symbol_ids: vec!["root".into()],
        file_paths: Vec::new(),
    };

    assert!(
        patch_typescript_indexes(
            &mut patched,
            &current,
            &symbols_by_id,
            &solid_dialect::Solid2,
            &changes
        )
        .is_some()
    );
    assert_eq!(
        patched.source_declarations["root"].location.path,
        ("b.ts").into(),
        "the current representative location must still be patched"
    );
    assert!(
        patched
            .source_discovery_delta
            .as_ref()
            .unwrap()
            .semantic_symbol_ids
            .is_empty(),
        "choosing another runtime declaration for the same root must not invalidate importers"
    );
}

#[test]
fn exact_index_patch_invalidates_a_root_when_an_alias_changes_its_source_declaration() {
    let symbol = |id: &str, target: &str, declarations| SymbolFact {
        id: id.into(),
        alias_target: target.into(),
        declarations,
        references: (Vec::new()).into(),
    };
    let old = typescript_table(
        1,
        Vec::new(),
        Vec::new(),
        vec![
            symbol("root", "", (Vec::new()).into()),
            symbol(
                "old-alias",
                "root",
                (vec![declaration("root", "fixture.d.ts", 1)]).into(),
            ),
        ],
        Vec::new(),
    );
    let current = typescript_table(
        2,
        Vec::new(),
        Vec::new(),
        vec![
            symbol("root", "", (Vec::new()).into()),
            symbol(
                "new-alias",
                "root",
                (vec![declaration("root", "fixture.ts", 1)]).into(),
            ),
        ],
        Vec::new(),
    );
    let symbols_by_id = current
        .symbols()
        .map(|symbol| (symbol.id(), symbol))
        .collect::<HashMap<_, _>>();
    let mut patched = typescript_index_cache(&old);
    let changes = solid_facts::TypeScriptChanges {
        unchanged: false,
        entity_paths: Vec::new(),
        symbol_ids: vec!["new-alias".into(), "old-alias".into()],
        file_paths: Vec::new(),
    };

    assert!(
        patch_typescript_indexes(
            &mut patched,
            &current,
            &symbols_by_id,
            &solid_dialect::Solid2,
            &changes
        )
        .is_some()
    );
    assert!(
        patched
            .source_discovery_delta
            .as_ref()
            .unwrap()
            .semantic_symbol_ids
            .contains("root")
    );
    assert_eq!(
        patched.source_declarations,
        typescript_index_cache(&current).source_declarations
    );
}

#[test]
fn exact_index_patch_rejects_alias_retargeting() {
    let table = |target: &str| {
        typescript_table(
            1,
            Vec::new(),
            Vec::new(),
            vec![
                SymbolFact {
                    id: "root-a".into(),
                    alias_target: (String::new()).into(),
                    declarations: (Vec::new()).into(),
                    references: (Vec::new()).into(),
                },
                SymbolFact {
                    id: "root-b".into(),
                    alias_target: (String::new()).into(),
                    declarations: (Vec::new()).into(),
                    references: (Vec::new()).into(),
                },
                SymbolFact {
                    id: "alias".into(),
                    alias_target: target.into(),
                    declarations: (Vec::new()).into(),
                    references: (Vec::new()).into(),
                },
            ],
            Vec::new(),
        )
    };
    let old = table("root-a");
    let current = table("root-b");
    let symbols_by_id = current
        .symbols()
        .map(|symbol| (symbol.id(), symbol))
        .collect::<HashMap<_, _>>();
    let mut patched = typescript_index_cache(&old);
    let changes = solid_facts::TypeScriptChanges {
        unchanged: false,
        entity_paths: Vec::new(),
        symbol_ids: vec!["alias".into()],
        file_paths: Vec::new(),
    };

    assert!(
        patch_typescript_indexes(
            &mut patched,
            &current,
            &symbols_by_id,
            &solid_dialect::Solid2,
            &changes
        )
        .is_none()
    );
    assert_eq!(patched.aliases, typescript_index_cache(&old).aliases);
}

#[test]
fn summary_containment_selects_innermost_function() {
    let nodes = vec![
        summary_node(
            "fixture.tsx",
            Span { start: 0, end: 100 },
            Span { start: 10, end: 90 },
        ),
        summary_node(
            "fixture.tsx",
            Span { start: 20, end: 60 },
            Span { start: 30, end: 50 },
        ),
    ];
    let by_path = function_indices_by_path(&nodes);

    assert_eq!(
        containing_summary_function_indexed(
            &nodes,
            &by_path,
            "fixture.tsx",
            Span { start: 35, end: 40 },
        ),
        Some(1)
    );
}

#[test]
fn containing_function_buckets_assign_each_item_to_one_innermost_owner() {
    let nodes = vec![
        summary_node(
            "fixture.tsx",
            Span { start: 0, end: 100 },
            Span { start: 10, end: 90 },
        ),
        summary_node(
            "fixture.tsx",
            Span { start: 20, end: 60 },
            Span { start: 30, end: 50 },
        ),
    ];
    let by_path = function_indices_by_path(&nodes);
    let items = [
        ("fixture.tsx", Span { start: 35, end: 40 }),
        ("fixture.tsx", Span { start: 70, end: 75 }),
        (
            "fixture.tsx",
            Span {
                start: 110,
                end: 115,
            },
        ),
    ];

    let buckets = items_by_containing_function(
        &nodes,
        &by_path,
        items.iter().map(|(path, span)| (*path, span)),
        |span| *span,
    );

    assert_eq!(buckets[0], vec![&items[1].1]);
    assert_eq!(buckets[1], vec![&items[0].1]);
}

#[test]
fn function_lookup_preserves_first_symbol_and_parameter_owner_for_a_path() {
    let mut first = summary_node(
        "fixture.tsx",
        Span { start: 0, end: 40 },
        Span { start: 10, end: 30 },
    );
    first.symbol = Some("first-function".into());
    first.parameters = vec!["shared-parameter".into()];
    let mut second = summary_node(
        "fixture.tsx",
        Span { start: 50, end: 90 },
        Span { start: 60, end: 80 },
    );
    second.symbol = Some("second-function".into());
    second.parameters = vec!["shared-parameter".into(), "second-parameter".into()];
    let nodes = vec![first, second];
    let by_path = function_indices_by_path(&nodes);

    let lookup = function_lookup_for_path(&nodes, &by_path, "fixture.tsx");

    assert_eq!(lookup.by_symbol.get("first-function"), Some(&0));
    assert_eq!(lookup.by_symbol.get("second-function"), Some(&1));
    assert_eq!(lookup.by_span.get(&Span { start: 0, end: 40 }), Some(&0));
    assert_eq!(lookup.by_span.get(&Span { start: 50, end: 90 }), Some(&1));
    assert_eq!(
        lookup.parameter_owner.get("shared-parameter"),
        Some(&(0, 0))
    );
    assert_eq!(
        lookup.parameter_owner.get("second-parameter"),
        Some(&(1, 1))
    );
}

#[test]
fn summary_membership_keeps_first_writer() {
    let mut reads = SummaryReads::default();
    let first = summary_read("first", "signal", 1);
    let mut duplicate = first.clone();
    duplicate.symbol = "second".into();
    duplicate.kind = Some("store-path".into());
    duplicate.origin_context = "different".into();

    assert!(reads.push_unique(first));
    assert!(!reads.push_unique(duplicate));

    assert_eq!(reads.len(), 1);
    assert_eq!(reads[0].symbol, "first");
    assert_eq!(reads[0].kind.as_deref(), Some("accessor"));
    assert_eq!(reads[0].origin_context, "first");
}

#[test]
fn returned_summary_deltas_preserve_fixed_edge_order() {
    let mut summaries = vec![
        SummaryReads::default(),
        SummaryReads::default(),
        SummaryReads::default(),
    ];
    summaries[1].push(summary_read("one", "one", 1));
    summaries[2].push(summary_read("two", "two", 2));

    propagate_returned_summary_deltas(&mut summaries, &[(0, 1), (1, 2), (0, 2)]);

    assert_eq!(
        summaries[0]
            .iter()
            .map(|read| read.symbol.as_str())
            .collect::<Vec<_>>(),
        vec!["one", "two"]
    );
    assert_eq!(
        summaries[1]
            .iter()
            .map(|read| read.symbol.as_str())
            .collect::<Vec<_>>(),
        vec!["one", "two"]
    );
}

#[test]
fn transported_summary_edges_keep_provenance_through_ordinary_hops() {
    let mut summaries = vec![SummaryReads::default(); 4];
    summaries[3].push(summary_read("count", "count", 1));
    let reverse_edges = vec![vec![], vec![0], vec![1], vec![2]];
    propagate_summary_deltas(
        &mut summaries,
        &reverse_edges,
        &mut [0; 4],
        &HashSet::from_iter([(2, 3)]),
        &HashSet::from_iter([(0, 1), (1, 2), (2, 3)]),
    );
    assert!(
        !summaries[3][0].transported,
        "the ordinary leaf remains eligible"
    );
    for summary in &summaries[..3] {
        assert!(
            summary[0].transported,
            "ordinary hops must retain the transport boundary"
        );
    }
}

/// ADR 0276: past an edge, a row's own completion standing becomes a hop
/// below and the edge's standing becomes the holder's; a better-proven
/// row arriving later is kept beside a weaker one.
#[test]
fn summary_edges_carry_completion_standing() {
    let mut summaries = vec![SummaryReads::default(); 4];
    let mut guarded = summary_read("count", "count", 1);
    guarded.local = true;
    summaries[3].push(guarded);
    // 0 -> 1 -> 3 is unconditional except 1 -> 3's own row; 0 -> 2 -> 3
    // calls 3 conditionally in 2.
    let reverse_edges = vec![vec![], vec![0], vec![0], vec![1, 2]];
    propagate_summary_deltas(
        &mut summaries,
        &reverse_edges,
        &mut [0; 4],
        &HashSet::default(),
        &HashSet::from_iter([(0, 1), (0, 2), (1, 3)]),
    );
    let standing = |index: usize| {
        summaries[index]
            .iter()
            .map(|read| (read.local, read.conditional))
            .collect::<Vec<_>>()
    };
    assert_eq!(standing(1), [(false, true)]);
    assert_eq!(standing(2), [(true, true)]);
    assert_eq!(standing(0), [(false, true)]);

    let mut reads = SummaryReads::default();
    let mut weak = summary_read("count", "count", 1);
    weak.conditional = true;
    assert!(reads.push_unique(weak));
    assert!(reads.push_unique(summary_read("count", "count", 1)));
    let mut weaker = summary_read("count", "count", 1);
    weaker.local = true;
    assert!(!reads.push_unique(weaker), "the proven row covers it");
}

#[test]
fn missing_result_dependency_invalidates_when_a_function_appears() {
    let dependency = InterproceduralResultDependency::Symbol("helper".into());
    let mut node = summary_node(
        "fixture.tsx",
        Span { start: 0, end: 10 },
        Span { start: 2, end: 9 },
    );
    node.symbol = Some("helper".into());
    let nodes = vec![node];
    let indexes = HashMap::from_iter([(("fixture.tsx".into(), nodes[0].span), 0)]);
    let summaries = vec![SummaryReads::default()];
    let invoked_parameters = vec![Vec::new()];
    let invoked_parameter_members = vec![Vec::new()];
    let returned_bindings = HashMap::default();
    let missing = InterproceduralResultDependencyState::Missing;

    let missing_by_symbol = HashMap::default();
    let missing_view = InterproceduralResultView {
        nodes: &nodes,
        indexes: &indexes,
        by_symbol: &missing_by_symbol,
        summaries: &summaries,
        invoked_parameters: &invoked_parameters,
        invoked_parameter_members: &invoked_parameter_members,
        returned_bindings: &returned_bindings,
    };
    assert!(missing_view.dependency_matches(&missing, &dependency));

    let by_symbol = HashMap::from_iter([("helper".into(), 0)]);
    let present_view = InterproceduralResultView {
        by_symbol: &by_symbol,
        ..missing_view
    };
    assert!(!present_view.dependency_matches(&missing, &dependency));

    let direct_members = vec![vec![crate::interproc::ParameterMemberInvocation {
        parameter: 0,
        path: vec!["of".into(), "values".into()],
        in_owner_body: true,
        site: Span { start: 3, end: 5 },
        primitive_builtin: false,
        dispatch_read_excluded: false,
        unmodeled: false,
    }]];
    let direct_view = InterproceduralResultView {
        invoked_parameter_members: &direct_members,
        ..present_view
    };
    let direct_state = InterproceduralResultDependencyState::Function {
        name: nodes[0].name.clone(),
        summary: Vec::new(),
        invoked_parameters: Vec::new(),
        invoked_parameter_members: direct_members[0].clone(),
    };
    let mut captured_members = direct_members.clone();
    captured_members[0][0].in_owner_body = false;
    let captured_view = InterproceduralResultView {
        invoked_parameter_members: &captured_members,
        ..direct_view
    };
    assert!(direct_view.dependency_matches(&direct_state, &dependency));
    assert!(!captured_view.dependency_matches(&direct_state, &dependency));
    let inline =
        InterproceduralResultDependency::InlineFunction("fixture.tsx".into(), nodes[0].span);
    assert!(direct_view.dependency_matches(&direct_state, &inline));
    assert!(!captured_view.dependency_matches(&direct_state, &inline));
    let mut tracked_members = direct_members.clone();
    tracked_members[0][0].dispatch_read_excluded = true;
    let tracked_view = InterproceduralResultView {
        invoked_parameter_members: &tracked_members,
        ..direct_view
    };
    assert!(!tracked_view.dependency_matches(&direct_state, &dependency));
    assert!(!tracked_view.dependency_matches(&direct_state, &inline));
}
