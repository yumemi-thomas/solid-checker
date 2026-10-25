use std::process::Command;
use std::{env, fs, path::PathBuf};

use crate::process_support::{decode_findings, temporary_directory};
use solid_facts::compiler::CompilerOptions;
use solid_facts_backend::{
    CacheStats, NativeIncrementalSession, SourceChange, SourceFile, TypeFactsSession,
    build_project, dialect,
};

#[test]
fn nested_member_callee_edits_refresh_unchanged_callers() {
    let Ok(typefacts) = env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../fixtures/reactive-ir/literal-argument-member-dispatch");
    let directory = temporary_directory("nested-member-callee-edit");
    for name in [
        "tsconfig.json",
        "solid-js.d.ts",
        "Nested.tsx",
        "nested-source.ts",
    ] {
        fs::copy(fixture.join(name), directory.join(name)).unwrap();
    }
    let stub = directory.join("node_modules/solid-js");
    fs::create_dir_all(&stub).unwrap();
    fs::write(
        stub.join("package.json"),
        r#"{"name":"solid-js","version":"2.0.0-rc.13"}"#,
    )
    .unwrap();
    let veto = directory.join("veto.ts");
    fs::write(&veto, "export {};\n").unwrap();
    let veto = veto.canonicalize().unwrap().to_string_lossy().into_owned();
    let project = directory
        .join("tsconfig.json")
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let caller = directory
        .join("Nested.tsx")
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let helper = directory
        .join("nested-source.ts")
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let source = fs::read_to_string(&helper).unwrap();
    let escaped = source.replacen(
        "return input.source.read();",
        "retain(input.source);\n  return input.source.read();",
        1,
    );
    let caller_source = fs::read_to_string(&caller).unwrap();
    let site =
        caller_source.find("readNested({ source:").unwrap() as u64 + "readNested(".len() as u64;
    let typescript = TypeFactsSession::open(&typefacts, &project, &[]).unwrap();
    let (mut warm, _) =
        NativeIncrementalSession::open_pipelined(dialect::default_dialect(), project, typescript)
            .unwrap();
    let mut incremental = solid_reactive_ir::IncrementalBuilder::default();
    let selected = dialect::default_dialect();
    let has_obligation = |program: &solid_reactive_ir::Program| {
        selected.solve(program).iter().any(|finding| {
            finding.id == "SC9012"
                && finding.kind == "uncertifiable"
                && finding.primary_location.path.as_ref() == caller
                && finding.primary_location.start_byte == site
        })
    };
    let first = warm.analyze().unwrap();
    let (initial, _) = incremental.build(&first, selected.vocabulary).unwrap();
    assert!(!has_obligation(&initial));
    for (version, path, text, expected) in [
        (1, &helper, escaped.as_str(), true),
        (2, &helper, source.as_str(), false),
        (3, &veto, "export function veto() { eval(''); }\n", true),
        (4, &veto, "export {};\n", false),
    ] {
        let facts = warm
            .edit(
                vec![SourceChange {
                    path: path.clone(),
                    version,
                    source: Some(text.into()),
                    compiler_options: CompilerOptions::default(),
                }],
                None,
            )
            .unwrap();
        let cold = solid_reactive_ir::build(&facts, selected.vocabulary).unwrap();
        let (retained, _) = incremental.build(&facts, selected.vocabulary).unwrap();
        assert_eq!(retained, cold, "generation {version}");
        assert_eq!(has_obligation(&retained), expected, "generation {version}");
        assert!(warm.last_build_timings().source_files_reused > 0);
    }
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn native_scheduler_writes_refresh_retained_eager_read_summaries() {
    let Ok(typefacts) = env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let fixture = root.join("fixtures/reactive-ir/accessor-provenance");
    let directory = temporary_directory("native-scheduler-read-edit");
    for name in [
        "tsconfig.json",
        "package.json",
        "solid-js.d.ts",
        "factory.ts",
        "RuntimeStack.tsx",
    ] {
        fs::copy(fixture.join(name), directory.join(name)).unwrap();
    }
    let stub = directory.join("node_modules/solid-js");
    fs::create_dir_all(&stub).unwrap();
    fs::write(
        stub.join("package.json"),
        r#"{"name":"solid-js","version":"2.0.0-rc.13"}"#,
    )
    .unwrap();
    let writes = directory.join("writes.ts");
    fs::write(&writes, "export {};\n").unwrap();
    let project = directory
        .join("tsconfig.json")
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let caller = directory
        .join("RuntimeStack.tsx")
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let writes = writes
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let source = fs::read_to_string(&caller).unwrap();
    let site = source.find("helper();").unwrap() as u64;
    let typescript = TypeFactsSession::open(&typefacts, &project, &[]).unwrap();
    let (mut warm, _) =
        NativeIncrementalSession::open_pipelined(dialect::default_dialect(), project, typescript)
            .unwrap();
    let mut incremental = solid_reactive_ir::IncrementalBuilder::default();
    let first = warm.analyze().unwrap();
    let (initial, _) = incremental
        .build(&first, dialect::default_dialect().vocabulary)
        .unwrap();
    let has_read = |program: &solid_reactive_ir::Program| {
        program
            .reads
            .iter()
            .any(|read| read.location.path.as_ref() == caller && read.location.start_byte == site)
    };
    assert!(!has_read(&initial));
    for (version, source, expected) in [
        (1, "queueMicrotask = callback => callback();\n", true),
        (2, "export {};\n", false),
        (
            3,
            "globalThis.queueMicrotask = callback => callback();\n",
            true,
        ),
        (4, "export {};\n", false),
    ] {
        let facts = warm
            .edit(
                vec![SourceChange {
                    path: writes.clone(),
                    version,
                    source: Some(source.into()),
                    compiler_options: CompilerOptions::default(),
                }],
                None,
            )
            .unwrap();
        let cold = solid_reactive_ir::build(&facts, dialect::default_dialect().vocabulary).unwrap();
        let (retained, _) = incremental
            .build(&facts, dialect::default_dialect().vocabulary)
            .unwrap();
        assert_eq!(has_read(&retained), expected, "generation {version}");
        assert_eq!(retained, cold, "generation {version}");
        assert!(warm.last_build_timings().source_files_reused > 0);
    }
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn augmented_timer_writes_in_another_file_refresh_retained_facts() {
    let Ok(typefacts) = env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let fixture = root.join("fixtures/reactive-ir/leaf-scope-host-callbacks");
    let directory = temporary_directory("augmented-timer-edit");
    for name in [
        "tsconfig.json",
        "solid-js.d.ts",
        "timers.d.ts",
        "AugmentedTimers.tsx",
    ] {
        fs::copy(fixture.join(name), directory.join(name)).unwrap();
    }
    let stub = directory.join("node_modules/solid-js");
    fs::create_dir_all(&stub).unwrap();
    fs::write(
        stub.join("package.json"),
        r#"{"name":"solid-js","version":"2.0.0-rc.13"}"#,
    )
    .unwrap();
    let writes = directory.join("writes.ts");
    fs::write(&writes, "export {};\n").unwrap();
    let project = directory
        .join("tsconfig.json")
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let caller = directory
        .join("AugmentedTimers.tsx")
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let writes = writes
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let source = fs::read_to_string(&caller).unwrap();
    let site = source.find("    setInterval(").unwrap() as u64 + 4;
    let typescript = TypeFactsSession::open(&typefacts, &project, &[]).unwrap();
    let (mut warm, _) =
        NativeIncrementalSession::open_pipelined(dialect::default_dialect(), project, typescript)
            .unwrap();
    let mut incremental = solid_reactive_ir::IncrementalBuilder::default();
    let first = warm.analyze().unwrap();
    let (initial, _) = incremental
        .build(&first, dialect::default_dialect().vocabulary)
        .unwrap();
    let unresolved = |program: &solid_reactive_ir::Program| {
        program.leaf_operations.iter().any(|operation| {
            operation.location.path.as_ref() == caller
                && operation.location.start_byte == site
                && operation.uncertain
        })
    };
    assert!(!unresolved(&initial));
    for (version, source, expected) in [
        (1, "setInterval = (() => 0) as typeof setInterval;\n", true),
        (2, "export {};\n", false),
        (
            3,
            "window.setInterval = (() => 0) as typeof window.setInterval;\n",
            true,
        ),
        (4, "export {};\n", false),
        (
            5,
            "window[\"setInterval\"] = (() => 0) as typeof window.setInterval;\n",
            true,
        ),
        (6, "export {};\n", false),
    ] {
        let facts = warm
            .edit(
                vec![SourceChange {
                    path: writes.clone(),
                    version,
                    source: Some(source.into()),
                    compiler_options: CompilerOptions::default(),
                }],
                None,
            )
            .unwrap();
        let cold = solid_reactive_ir::build(&facts, dialect::default_dialect().vocabulary).unwrap();
        let (retained, _) = incremental
            .build(&facts, dialect::default_dialect().vocabulary)
            .unwrap();
        assert_eq!(unresolved(&retained), expected, "generation {version}");
        assert_eq!(retained, cold, "generation {version}");
        assert!(warm.last_build_timings().source_files_reused > 0);
    }
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn leaf_source_target_edits_match_cold_analysis() {
    let Ok(typefacts) = env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let fixture = root.join("fixtures/reactive-ir/leaf-source-call-targets");
    let directory = temporary_directory("leaf-source-target-edit");
    for name in ["tsconfig.json", "solid-js.d.ts", "service.ts", "App.tsx"] {
        fs::copy(fixture.join(name), directory.join(name)).unwrap();
    }
    let stub = directory.join("node_modules/solid-js");
    fs::create_dir_all(&stub).unwrap();
    fs::copy(
        fixture.join("node_modules/solid-js/package.json"),
        stub.join("package.json"),
    )
    .unwrap();
    let project = directory
        .join("tsconfig.json")
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let service = directory
        .join("service.ts")
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let caller = directory
        .join("App.tsx")
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let source = fs::read_to_string(&service).unwrap();
    let caller_source = fs::read_to_string(&caller).unwrap();
    let site = caller_source.find("service.cleanLeaf()").unwrap() as u64;
    let typescript = TypeFactsSession::open(&typefacts, &project, &[]).unwrap();
    let (mut warm, _) =
        NativeIncrementalSession::open_pipelined(dialect::default_dialect(), project, typescript)
            .unwrap();
    let mut incremental = solid_reactive_ir::IncrementalBuilder::default();
    let first = warm.analyze().unwrap();
    let (initial, _) = incremental
        .build(&first, dialect::default_dialect().vocabulary)
        .unwrap();
    let operations = |program: &solid_reactive_ir::Program| {
        program
            .leaf_operations
            .iter()
            .filter(|operation| {
                operation.location.path.as_ref() == caller && operation.location.start_byte == site
            })
            .map(|operation| (operation.uncertain, operation.possible))
            .collect::<Vec<_>>()
    };
    assert!(operations(&initial).is_empty());
    for (version, edited_source, expected) in [
        (
            1,
            source.replacen(
                "cleanLeaf() { void 0; }",
                "cleanLeaf() { onCleanup(() => {}); }",
                1,
            ),
            vec![(false, false)],
        ),
        (2, source.clone(), vec![]),
        (
            3,
            source.replacen("void 0;", "this;  ", 1),
            vec![(true, false)],
        ),
        (4, source, vec![]),
    ] {
        let edited = warm
            .edit(
                vec![SourceChange {
                    path: service.clone(),
                    version,
                    source: Some(edited_source),
                    compiler_options: CompilerOptions::default(),
                }],
                None,
            )
            .unwrap();
        assert!(warm.last_build_timings().source_files_reused > 0);
        let cold =
            solid_reactive_ir::build(&edited, dialect::default_dialect().vocabulary).unwrap();
        let (retained, _) = incremental
            .build(&edited, dialect::default_dialect().vocabulary)
            .unwrap();
        assert_eq!(operations(&retained), expected);
        assert_eq!(retained, cold, "version {version}");
    }
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn routed_callback_consumer_edits_refresh_retained_dispatch_contexts() {
    let Ok(typefacts) = env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let fixture = root.join("fixtures/reactive-ir/forwarded-tracked-compute");
    let directory = temporary_directory("routed-member-consumer-edit");
    for name in [
        "tsconfig.json",
        "solid-js.d.ts",
        "member-consumers.ts",
        "member-helpers.ts",
        "RoutedMembers.tsx",
    ] {
        fs::copy(fixture.join(name), directory.join(name)).unwrap();
    }
    // Every other file reads the consumer it calls, so its graph chunk reruns
    // on a consumer edit (ADR 0408); this one reads nothing of it.
    fs::write(
        directory.join("unrelated.ts"),
        "export function unrelated() {\n  return 1;\n}\n",
    )
    .unwrap();
    let stub = directory.join("node_modules/solid-js");
    fs::create_dir_all(&stub).unwrap();
    fs::copy(
        fixture.join("node_modules/solid-js/package.json"),
        stub.join("package.json"),
    )
    .unwrap();
    let project = directory
        .join("tsconfig.json")
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let consumer = directory
        .join("member-consumers.ts")
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let caller = directory
        .join("RoutedMembers.tsx")
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let source = fs::read_to_string(&consumer).unwrap();
    let caller_source = fs::read_to_string(&caller).unwrap();
    let argument = caller_source.find("routedMember({").unwrap() + "routedMember(".len();
    let read_site = argument + "{ read: () => ".len();
    let typescript = TypeFactsSession::open(&typefacts, &project, &[]).unwrap();
    let (mut warm, _) =
        NativeIncrementalSession::open_pipelined(dialect::default_dialect(), project, typescript)
            .unwrap();
    let first = warm.analyze().unwrap();
    let mut incremental = solid_reactive_ir::IncrementalBuilder::default();
    let (initial, _) = incremental
        .build(&first, dialect::default_dialect().vocabulary)
        .unwrap();
    let dispatch = |program: &solid_reactive_ir::Program| {
        program.static_defects.iter().any(|defect| {
            defect.location.path.as_ref() == caller
                && defect.location.start_byte == argument as u64
                && defect.analysis_context == "parameter-member-targets-diverge"
        })
    };
    let non_strict = |program: &solid_reactive_ir::Program| {
        program.reads.iter().any(|read| {
            read.location.path.as_ref() == caller
                && read.location.start_byte == read_site as u64
                && read.project_consumer_non_strict
        })
    };
    assert!(!dispatch(&initial));
    assert!(non_strict(&initial));
    for (version, edited_source, excluded) in [
        (
            1,
            source.replacen(
                "return createMemo(() => options.compute());",
                "options.compute();\n  return createMemo(() => options.compute());",
                1,
            ),
            false,
        ),
        (2, source, true),
    ] {
        let edited = warm
            .edit(
                vec![SourceChange {
                    path: consumer.clone(),
                    version,
                    source: Some(edited_source),
                    compiler_options: CompilerOptions::default(),
                }],
                None,
            )
            .unwrap();
        assert!(warm.last_build_timings().source_files_reused > 0);
        let cold =
            solid_reactive_ir::build(&edited, dialect::default_dialect().vocabulary).unwrap();
        let (retained, timings) = incremental
            .build(&edited, dialect::default_dialect().vocabulary)
            .unwrap();
        assert!(timings.interprocedural_graph_reused_files > 0);
        // ADR 0321: the dispatch resolves to the argument literal's arrow in
        // every version; the arrow's read carries the consumer context.
        assert!(!dispatch(&retained), "version {version}");
        assert_eq!(non_strict(&retained), excluded, "version {version}");
        assert_eq!(
            retained, cold,
            "consumer-only edit {version} must refresh callback and dispatch proofs"
        );
    }
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn returned_anonymous_member_caller_edit_matches_cold_analysis() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let fixture = root.join("fixtures/reactive-ir/accessor-provenance");
    let directory = temporary_directory("provenance-caller-edit");
    for name in [
        "tsconfig.json",
        "solid-js.d.ts",
        "warm-factory.ts",
        "Warm.tsx",
    ] {
        fs::copy(fixture.join(name), directory.join(name)).unwrap();
    }
    let stub = directory.join("node_modules/solid-js");
    fs::create_dir_all(&stub).unwrap();
    fs::copy(
        fixture.join("node_modules/solid-js/package.json"),
        stub.join("package.json"),
    )
    .unwrap();
    let project = directory
        .join("tsconfig.json")
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let caller = directory
        .join("Warm.tsx")
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let typescript = TypeFactsSession::open(&typefacts, &project, &[]).unwrap();
    let (mut warm, _) = NativeIncrementalSession::open_pipelined(
        dialect::default_dialect(),
        project.clone(),
        typescript,
    )
    .unwrap();
    let first = warm.analyze().unwrap();
    let mut incremental = solid_reactive_ir::IncrementalBuilder::default();
    incremental
        .build(&first, dialect::default_dialect().vocabulary)
        .unwrap();
    let source = fs::read_to_string(&caller)
        .unwrap()
        .replace("store.other()", "store.value()");
    let edited = warm
        .edit(
            vec![SourceChange {
                path: caller.clone(),
                version: 1,
                source: Some(source.clone()),
                compiler_options: CompilerOptions::default(),
            }],
            None,
        )
        .unwrap();
    assert!(warm.last_build_timings().source_files_reused > 0);
    let fresh = solid_reactive_ir::build(&edited, dialect::default_dialect().vocabulary).unwrap();
    let (retained, timings) = incremental
        .build(&edited, dialect::default_dialect().vocabulary)
        .unwrap();
    assert_eq!(
        timings.interprocedural_graph_reused_files,
        timings.source_discovery_unchanged_files
    );
    assert!(!retained.static_defects.iter().any(|defect| {
        defect.location.path.as_ref() == caller
            && defect.analysis_context == "returned-callable-outside-source-universe"
            && matches!(
                defect.kind,
                solid_reactive_ir::StaticDefectKind::ReactiveDispatchUnresolved { .. }
            )
    }));
    assert!(
        retained
            .reads
            .iter()
            .any(|read| read.location.path.as_ref() == caller)
    );
    assert_eq!(
        retained, fresh,
        "returned callable body facts must be stable across caller-only edits"
    );
    let reverted_source = source.replace("store.value()", "store.other()");
    let reverted = warm
        .edit(
            vec![SourceChange {
                path: caller.clone(),
                version: 2,
                source: Some(reverted_source),
                compiler_options: CompilerOptions::default(),
            }],
            None,
        )
        .unwrap();
    let fresh_reverted =
        solid_reactive_ir::build(&reverted, dialect::default_dialect().vocabulary).unwrap();
    let (retained_reverted, _) = incremental
        .build(&reverted, dialect::default_dialect().vocabulary)
        .unwrap();
    assert!(
        !retained_reverted
            .reads
            .iter()
            .any(|read| read.location.path.as_ref() == caller)
    );
    assert_eq!(
        retained_reverted, fresh_reverted,
        "returning to the inert member must remove the read in retained and cold IR"
    );
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn native_incremental_session_reuses_oxc_and_solid_facts() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let fixture = root.join("fixtures/reactive-ir/tracer");
    let project = fixture.join("tsconfig.json").canonicalize().unwrap();
    let project_id = project.to_string_lossy().into_owned();
    let paths = [fixture.join("App.tsx"), fixture.join("source.ts")];
    let sources = paths
        .iter()
        .map(|path| SourceFile {
            path: path.canonicalize().unwrap().to_string_lossy().into_owned(),
            source: fs::read_to_string(path).unwrap().into(),
            compiler_options: CompilerOptions::default(),
        })
        .collect();
    let typescript = TypeFactsSession::open(&typefacts, &project_id, &[]).unwrap();
    let mut session =
        NativeIncrementalSession::open(dialect::default_dialect(), project_id, sources, typescript)
            .unwrap();
    session.analyze().unwrap();
    let first_timings = session.last_build_timings();
    assert!(first_timings.exchange.server_materialized);
    assert!(!first_timings.exchange.roundtrip.is_zero());
    assert!(!first_timings.exchange.response_decode.is_zero());
    assert!(first_timings.exchange.response_bytes > 0);
    assert_eq!(
        session.cache_stats(),
        CacheStats {
            ast_entries: 2,
            compiler_entries: 2
        }
    );
    // A no-op analysis must reuse both native fact domains without growing
    // either cache.
    session.analyze().unwrap();
    assert!(
        session.last_build_timings().source_analysis.is_zero(),
        "an unchanged generation should reuse its completed source facts"
    );
    assert!(!session.last_build_timings().exchange.server_materialized);
    assert_eq!(
        session.cache_stats(),
        CacheStats {
            ast_entries: 2,
            compiler_entries: 2
        }
    );
    let app = paths[0].canonicalize().unwrap();
    // One edit exchange: the update and the analysis of the new generation
    // travel as a single session call; the changed file's cache entries are
    // invalidated and repopulated within it.
    let before_recovery = session
        .edit(
            vec![SourceChange {
                path: app.to_string_lossy().into_owned(),
                version: 1,
                source: Some(format!("// edit\n{}", fs::read_to_string(&app).unwrap())),
                compiler_options: CompilerOptions::default(),
            }],
            None,
        )
        .unwrap();
    assert_eq!(session.last_build_timings().source_files_reused, 1);
    assert_eq!(session.last_build_timings().source_files_recomputed, 1);
    assert_eq!(session.generation(), 2);
    assert_eq!(
        session.cache_stats(),
        CacheStats {
            ast_entries: 2,
            compiler_entries: 2
        }
    );
    session.recover_typefacts().unwrap();
    let after_recovery = session.analyze().unwrap();
    assert_eq!(
        serde_json::to_vec(&after_recovery).unwrap(),
        serde_json::to_vec(&before_recovery).unwrap(),
        "restarted TypeFacts service must replay the retained generation exactly"
    );
}

#[test]
fn pipelined_open_matches_sequential_sources_and_analyzes() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let fixture = root.join("fixtures/reactive-ir/tracer");
    let project = fixture.join("tsconfig.json").canonicalize().unwrap();
    let project_id = project.to_string_lossy().into_owned();

    // The sequential baseline: fetch sources, then open, in two round trips.
    let mut sequential = TypeFactsSession::open(&typefacts, &project_id, &[]).unwrap();
    let expected = sequential.configured_sources().unwrap();

    // The reordered handshake lets the client pipeline open+sources before the
    // program build completes; the pipelined path must resolve the same source
    // set and produce facts.
    let pipelined = TypeFactsSession::open(&typefacts, &project_id, &[]).unwrap();
    let (mut session, sources) =
        NativeIncrementalSession::open_pipelined(dialect::default_dialect(), project_id, pipelined)
            .unwrap();
    assert_eq!(
        sources
            .iter()
            .map(|source| (source.path.clone(), source.source.clone()))
            .collect::<Vec<_>>(),
        expected
            .iter()
            .map(|source| (source.path.clone(), source.source.clone()))
            .collect::<Vec<_>>(),
        "pipelined open+sources must resolve the same configured source set"
    );
    session.analyze().unwrap();
    assert_eq!(session.generation(), 1);
}
#[test]
fn rust_cli_covers_reactivity_v2_semantic_migration_matrix() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let output = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
        .env("SOLID_TYPEFACTS_BIN", typefacts)
        .args([
            "--format",
            "json",
            "--project",
            &root
                .join("fixtures/engine/eslint-reactivity-v2/tsconfig.json")
                .to_string_lossy(),
        ])
        .output()
        .expect("run Rust diagnostic CLI");
    assert!(
        output.status.success(),
        "stderr = {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let findings = decode_findings(&output.stdout);
    let expected = [
        ("leaf-reexport-flush.tsx", "leaf-owner-forbidden-call"),
        ("leaf-reexport-cleanup.tsx", "leaf-owner-forbidden-call"),
        ("owned-reexport-memo.tsx", "reactive-write-in-owned-scope"),
        (
            "owned-reexport-refresh.tsx",
            "reactive-write-in-owned-scope",
        ),
        ("owned-reexport-action.tsx", "action-called-in-owned-scope"),
        // ADR 0294: an exported function's `Accessor<T>` parameter is a
        // typed candidate, so its reads are obligations, not violations.
        ("effect-apply-parameter.tsx", "reactive-dispatch-unresolved"),
        ("effect-apply-member.tsx", "reactive-dispatch-unresolved"),
        ("after-await-member.tsx", "reactive-dispatch-unresolved"),
        ("after-await-parameter.tsx", "reactive-dispatch-unresolved"),
        ("after-await-namespace.tsx", "reactive-read-after-await"),
        (
            "after-await-callback-before-read.tsx",
            "reactive-read-after-await",
        ),
        (
            "after-await-named-callback.tsx",
            "reactive-read-after-await",
        ),
        (
            "after-await-aliased-callback.tsx",
            "reactive-read-after-await",
        ),
        (
            "after-await-same-expression.tsx",
            "reactive-read-after-await",
        ),
        ("after-await-both-branches.tsx", "reactive-read-after-await"),
        ("after-await-try-finally.tsx", "reactive-read-after-await"),
        (
            "imported-after-await-definition.ts",
            "reactive-read-after-await",
        ),
        ("component-props-read.tsx", "strict-read-untracked"),
        ("component-props-alias.tsx", "strict-read-untracked"),
        ("component-props-merge-alias.tsx", "strict-read-untracked"),
        (
            "component-props-parameter-destructure.tsx",
            "no-destructure",
        ),
        ("component-props-body-destructure.tsx", "no-destructure"),
        (
            "derived-signal-in-effect.tsx",
            "reactive-write-in-owned-scope",
        ),
    ];
    for (file, rule) in expected {
        assert!(
            findings.iter().any(|finding| {
                finding["rule"] == rule
                    && finding["primaryLocation"]["path"]
                        .as_str()
                        .is_some_and(|path| path.ends_with(file))
            }),
            "missing {file} / {rule}"
        );
    }
    let negatives = [
        ("effect-apply-plain-function.tsx", "strict-read-untracked"),
        ("effect-apply-structural-store.tsx", "strict-read-untracked"),
        (
            "after-await-plain-function.tsx",
            "reactive-read-after-await",
        ),
        (
            "after-await-local-accessor.tsx",
            "reactive-read-after-await",
        ),
        ("before-await-accessor.tsx", "reactive-read-after-await"),
        (
            "conditional-await-accessor.tsx",
            "reactive-read-after-await",
        ),
        (
            "nested-after-await-accessor.tsx",
            "reactive-read-after-await",
        ),
        ("loop-await-accessor.tsx", "reactive-read-after-await"),
        ("component-props-tracked.tsx", "strict-read-untracked"),
        ("component-props-tracked-destructure.tsx", "no-destructure"),
        ("noncomponent-object-read.ts", "strict-read-untracked"),
        ("noncomponent-object-destructure.ts", "no-destructure"),
        ("component-props-passthrough.tsx", "strict-read-untracked"),
        ("component-props-local-merge.tsx", "strict-read-untracked"),
        (
            "component-props-unknown-callback.tsx",
            "strict-read-untracked",
        ),
        ("component-static-early-return.tsx", "strict-read-untracked"),
        (
            "signal-write-in-effect-apply.tsx",
            "reactive-write-in-owned-scope",
        ),
    ];
    for (file, rule) in negatives {
        assert!(
            !findings.iter().any(|finding| {
                finding["rule"] == rule
                    && finding["primaryLocation"]["path"]
                        .as_str()
                        .is_some_and(|path| path.ends_with(file))
            }),
            "unexpected {file} / {rule}"
        );
    }
    assert_eq!(
        findings
            .iter()
            .filter(|finding| {
                finding["rule"] == "no-destructure"
                    && finding["primaryLocation"]["path"]
                        .as_str()
                        .is_some_and(|path| {
                            path.ends_with("component-props-parameter-complex-destructure.tsx")
                        })
            })
            .count(),
        3
    );
}
#[test]
fn rust_cli_emits_snapshot_text_and_certification_exit_codes() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let tracer = root.join("fixtures/reactive-ir/tracer/tsconfig.json");
    let output = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
        .env("SOLID_TYPEFACTS_BIN", &typefacts)
        .args(["--format", "text", "--project", &tracer.to_string_lossy()])
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains(": violation\n"));
    assert!(text.contains("SC1001 [violation]"));

    let output = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
        .env("SOLID_TYPEFACTS_BIN", &typefacts)
        .args([
            "--format",
            "json",
            "--certify",
            "--project",
            &tracer.to_string_lossy(),
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let snapshot: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(snapshot["status"], "violation");
    assert_eq!(snapshot["findings"][0]["primaryLocation"]["line"], 8);
    assert!(
        snapshot["findings"][0]["primaryLocation"]["column"]
            .as_u64()
            .is_some_and(|column| column > 0)
    );
    assert_eq!(snapshot["metrics"]["filesAnalyzed"], 1);

    let corrected = root.join("fixtures/reactive-ir/tracer-corrected/tsconfig.json");
    let output = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
        .env("SOLID_TYPEFACTS_BIN", typefacts)
        .args([
            "--format",
            "json",
            "--certify",
            "--project",
            &corrected.to_string_lossy(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let snapshot: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(snapshot["status"], "certified");
    assert_eq!(snapshot["findings"].as_array().unwrap().len(), 0);
}

#[cfg(unix)]
#[test]
fn daemon_and_one_shot_share_snapshot_emission() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let project = root.join("fixtures/reactive-ir/tracer/tsconfig.json");
    let command = || {
        let mut command = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"));
        command
            .env("SOLID_TYPEFACTS_BIN", &typefacts)
            .env("SOLID_CHECKER_DAEMON", "1")
            .env("SOLID_CHECKER_DAEMON_IDLE_SECS", "1");
        command
    };

    let text = command()
        .args(["--format", "text", "--project", &project.to_string_lossy()])
        .output()
        .unwrap();
    assert_eq!(text.status.code(), Some(0));
    assert!(
        String::from_utf8(text.stdout)
            .unwrap()
            .contains("SC1001 [violation]")
    );

    let json = command()
        .args([
            "--format",
            "json",
            "--certify",
            "--project",
            &project.to_string_lossy(),
        ])
        .output()
        .unwrap();
    assert_eq!(json.status.code(), Some(1));
    let snapshot: serde_json::Value = serde_json::from_slice(&json.stdout).unwrap();
    assert_eq!(snapshot["status"], "violation");
}

#[test]
fn joins_real_oxc_compiler_and_tsgo_facts() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let fixture = root.join("fixtures/reactive-ir/tracer");
    let project = fixture
        .join("tsconfig.json")
        .canonicalize()
        .expect("canonical tsconfig");
    let source_paths = [fixture.join("App.tsx"), fixture.join("source.ts")];
    let sources = source_paths
        .into_iter()
        .map(|path| SourceFile {
            path: path
                .canonicalize()
                .expect("canonical source")
                .to_string_lossy()
                .into_owned(),
            source: std::fs::read_to_string(path).expect("read source").into(),
            compiler_options: CompilerOptions::default(),
        })
        .collect();
    let selected = dialect::default_dialect();
    let mut compiler = (selected.compiler)();
    let mut typescript = TypeFactsSession::open(&typefacts, &project.to_string_lossy(), &[])
        .expect("open the TypeFacts session");
    let facts = build_project(
        selected,
        project.to_string_lossy(),
        1,
        sources,
        compiler.as_mut(),
        &mut typescript,
    )
    .expect("join real facts");
    assert_eq!(facts.files.len(), 2);
    assert!(facts.typescript.entities().next().is_some());
    assert!(
        facts
            .typescript
            .entities()
            .any(|entity| entity.callability == Some(typefacts::Callability::Callable)),
        "exported functions should carry compiler-derived callability"
    );
    assert!(
        facts.typescript.entities().any(|entity| {
            matches!(
                entity.reference_space,
                Some(typefacts::ReferenceSpace::Value | typefacts::ReferenceSpace::Both)
            )
        }),
        "runtime imports should carry value-space reference facts"
    );
    assert!(
        facts
            .typescript
            .entities()
            .any(|entity| !entity.runtime_identity.is_empty()),
        "runtime imports and exports should carry canonical runtime identities"
    );
    let program = solid_reactive_ir::build(&facts, dialect::default_dialect().vocabulary)
        .expect("build Rust Reactive IR");
    let (incremental_program, incremental_timings) =
        solid_reactive_ir::IncrementalBuilder::default()
            .build(&facts, dialect::default_dialect().vocabulary)
            .expect("build incremental Rust Reactive IR");
    assert_eq!(
        incremental_program, program,
        "initial incremental fragments must match the fresh builder"
    );
    assert_eq!(incremental_timings.source_discovery_unchanged_files, 0);
    let findings = selected.solve(&program);
    assert_eq!(findings.len(), 1, "findings = {findings:#?}");
    // Catalog-relative name: unprefixed in 2.0, `v1/`-prefixed in a
    // dialect-v1-only build.
    assert!(
        findings[0].rule.ends_with("strict-read-untracked"),
        "rule = {}",
        findings[0].rule
    );
    assert!(findings[0].primary_location.path.ends_with("App.tsx"));
}
/// Opens the producer, absorbing the Linux `ETXTBSY` race: exec'ing a
/// just-written wrapper script can collide with another test thread's fork
/// inheriting the writer's file descriptor for the instant before its own
/// exec. The window is microscopic, so a short retry is enough.
fn open_type_facts_session(executable: &str, project_id: &str) -> TypeFactsSession {
    let mut last = None;
    for _ in 0..50 {
        match TypeFactsSession::open(executable, project_id, &[]) {
            Err(error) if format!("{error:?}").contains("ExecutableFileBusy") => {
                std::thread::sleep(std::time::Duration::from_millis(10));
                last = Some(error);
            }
            result => return result.unwrap(),
        }
    }
    panic!("the producer executable stayed busy: {last:?}");
}

fn tracer_fixture_session(typefacts_executable: &str) -> (NativeIncrementalSession, Vec<PathBuf>) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let fixture = root.join("fixtures/reactive-ir/tracer");
    let project = fixture.join("tsconfig.json").canonicalize().unwrap();
    let project_id = project.to_string_lossy().into_owned();
    let paths = vec![
        fixture.join("App.tsx").canonicalize().unwrap(),
        fixture.join("source.ts").canonicalize().unwrap(),
    ];
    let sources = paths
        .iter()
        .map(|path| SourceFile {
            path: path.to_string_lossy().into_owned(),
            source: fs::read_to_string(path).unwrap().into(),
            compiler_options: CompilerOptions::default(),
        })
        .collect();
    let typescript = open_type_facts_session(typefacts_executable, &project_id);
    let session =
        NativeIncrementalSession::open(dialect::default_dialect(), project_id, sources, typescript)
            .unwrap();
    (session, paths)
}

fn app_edit(paths: &[PathBuf]) -> SourceChange {
    SourceChange {
        path: paths[0].to_string_lossy().into_owned(),
        version: 1,
        source: Some(format!(
            "// edit exchange test\n{}",
            fs::read_to_string(&paths[0]).unwrap()
        )),
        compiler_options: CompilerOptions::default(),
    }
}

#[test]
fn incremental_reactive_ir_matches_fresh_after_an_edit() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let (mut session, paths) = tracer_fixture_session(&typefacts);
    let first = session.analyze().unwrap();
    let mut incremental = solid_reactive_ir::IncrementalBuilder::default();
    incremental
        .build(&first, dialect::default_dialect().vocabulary)
        .unwrap();

    let edited = session.edit(vec![app_edit(&paths)], None).unwrap();
    let fresh = solid_reactive_ir::build(&edited, dialect::default_dialect().vocabulary).unwrap();
    let (retained, timings) = incremental
        .build(&edited, dialect::default_dialect().vocabulary)
        .unwrap();

    assert_eq!(retained, fresh);
    assert!(!timings.reused);
    assert!(timings.typescript_indexes_reused);
    assert!(!timings.reachability_reused);
    assert_eq!(
        timings.reachability_reused_files + timings.reachability_recomputed_files,
        u64::try_from(edited.files.len()).unwrap()
    );
    assert_eq!(
        timings.local_access_reused_files + timings.local_access_recomputed_files,
        u64::try_from(edited.files.len()).unwrap()
    );
    assert_eq!(
        timings.owner_reused_files + timings.owner_recomputed_files,
        u64::try_from(edited.files.len()).unwrap()
    );
    // A graph chunk is reused only for an unchanged file, and only
    // while what it read still answers as it did (ADR 0397): the prefix
    // edit moves the edited file's component keys, which the project passes
    // whose tables the graph reads have read.
    assert!(
        timings.interprocedural_graph_reused_files <= timings.source_discovery_unchanged_files,
        "{timings:?}"
    );
    assert_eq!(
        timings.interprocedural_graph_reused_files + timings.interprocedural_graph_recomputed_files,
        u64::try_from(edited.files.len()).unwrap()
    );
    assert_eq!(
        timings.interprocedural_result_reused_files
            + timings.interprocedural_result_recomputed_files,
        u64::try_from(edited.files.len()).unwrap()
    );
}

#[test]
fn incremental_result_cache_preserves_dispatch_obligations() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let directory = temporary_directory("dispatch-obligation-cache");
    let project = directory.join("tsconfig.json");
    let app = directory.join("App.tsx");
    let source = directory.join("source.ts");
    let other = directory.join("other.ts");
    let declarations = directory.join("solid-js.d.ts");
    fs::write(
        &project,
        r#"{"compilerOptions":{"jsx":"preserve","module":"ESNext","moduleResolution":"Bundler","strict":true,"target":"ES2022"},"files":["App.tsx","source.ts","other.ts","solid-js.d.ts"]}"#,
    )
    .unwrap();
    fs::write(
        &app,
        r#"import { invoke, reactive, quiet } from "./source";
export function App() {
  const value = invoke(Math.random() > 0.5 ? reactive : quiet);
  return <div>{value}</div>;
}
"#,
    )
    .unwrap();
    fs::write(
        &source,
        r#"import { createSignal } from "solid-js";
const [count] = createSignal(0);
export const reactive = { read: () => count() };
export const quiet = { read: () => 0 };
export function invoke(reader: { read(): number }) { return reader.read(); }
"#,
    )
    .unwrap();
    fs::write(&other, "export const unrelated = 1;\n").unwrap();
    fs::write(
        &declarations,
        r#"declare module "solid-js" {
  export function createSignal<T>(value: T): [() => T, (value: T) => void];
}
declare namespace JSX {
  interface IntrinsicElements { div: Record<string, unknown>; }
  interface Element {}
}
"#,
    )
    .unwrap();

    let project_id = project.to_string_lossy().into_owned();
    let paths = [&app, &source, &other];
    let sources = paths
        .iter()
        .map(|path| SourceFile {
            path: path.to_string_lossy().into_owned(),
            source: fs::read_to_string(path).unwrap().into(),
            compiler_options: CompilerOptions::default(),
        })
        .collect();
    let selected = dialect::by_id("solid-v2").unwrap();
    let typescript = open_type_facts_session(&typefacts, &project_id);
    let mut session =
        NativeIncrementalSession::open(selected, project_id, sources, typescript).unwrap();
    let first = session.analyze().unwrap();
    let mut incremental = solid_reactive_ir::IncrementalBuilder::default();
    let (initial, _) = incremental.build(&first, selected.vocabulary).unwrap();
    assert!(selected.solve(&initial).iter().any(|finding| {
        finding.rule == "reactive-dispatch-unresolved" && finding.kind == "uncertifiable"
    }));

    let edited = session
        .edit(
            vec![SourceChange {
                path: other.to_string_lossy().into_owned(),
                version: 1,
                source: Some("export const unrelated = 2;\n".into()),
                compiler_options: CompilerOptions::default(),
            }],
            None,
        )
        .unwrap();
    let fresh = solid_reactive_ir::build(&edited, selected.vocabulary).unwrap();
    let (retained, timings) = incremental.build(&edited, selected.vocabulary).unwrap();
    assert!(
        timings.interprocedural_result_reused_files > 0,
        "the unchanged files should exercise the per-file result cache"
    );
    assert_eq!(retained, fresh);
    assert!(selected.solve(&retained).iter().any(|finding| {
        finding.rule == "reactive-dispatch-unresolved" && finding.kind == "uncertifiable"
    }));
}

/// ADR 0221: moving an `await` across a read in an async callee changes
/// whether that read runs during the call, while the callee's read summary
/// can stay the same. A warm session must answer what a cold build does.
#[test]
fn incremental_result_cache_follows_a_moved_await() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let directory = temporary_directory("moved-await-cache");
    let project = directory.join("tsconfig.json");
    let app = directory.join("App.tsx");
    let helper = directory.join("helper.ts");
    let declarations = directory.join("solid-js.d.ts");
    fs::write(
        &project,
        r#"{"compilerOptions":{"jsx":"preserve","module":"ESNext","moduleResolution":"Bundler","strict":true,"target":"ES2022"},"files":["App.tsx","helper.ts","solid-js.d.ts"]}"#,
    )
    .unwrap();
    fs::write(
        &app,
        r#"import { load } from "./helper";
export function App() {
  void load();
  return <div />;
}
"#,
    )
    .unwrap();
    let before = r#"import { createSignal } from "solid-js";
const [count] = createSignal(0);
export async function load() {
  const value = count();
  await Promise.resolve();
  return value;
}
"#;
    let after = r#"import { createSignal } from "solid-js";
const [count] = createSignal(0);
export async function load() {
  await Promise.resolve();
  const value = count();
  return value;
}
"#;
    fs::write(&helper, before).unwrap();
    fs::write(
        &declarations,
        r#"declare module "solid-js" {
  export function createSignal<T>(value: T): [() => T, (value: T) => void];
}
declare namespace JSX {
  interface IntrinsicElements { div: Record<string, unknown>; }
  interface Element {}
}
"#,
    )
    .unwrap();

    let project_id = project.to_string_lossy().into_owned();
    let sources = [&app, &helper]
        .iter()
        .map(|path| SourceFile {
            path: path.to_string_lossy().into_owned(),
            source: fs::read_to_string(path).unwrap().into(),
            compiler_options: CompilerOptions::default(),
        })
        .collect();
    let selected = dialect::by_id("solid-v2").unwrap();
    let typescript = open_type_facts_session(&typefacts, &project_id);
    let mut session =
        NativeIncrementalSession::open(selected, project_id, sources, typescript).unwrap();
    let first = session.analyze().unwrap();
    let mut incremental = solid_reactive_ir::IncrementalBuilder::default();
    let (initial, _) = incremental.build(&first, selected.vocabulary).unwrap();
    let violations = |program: &_| {
        selected
            .solve(program)
            .iter()
            .filter(|finding| {
                finding.rule == "strict-read-untracked" && finding.kind == "violation"
            })
            .count()
    };
    assert_eq!(
        violations(&initial),
        1,
        "the read before the await runs during the call"
    );

    for (version, source) in [(1, after), (2, before), (3, after)] {
        let edited = session
            .edit(
                vec![SourceChange {
                    path: helper.to_string_lossy().into_owned(),
                    version,
                    source: Some(source.into()),
                    compiler_options: CompilerOptions::default(),
                }],
                None,
            )
            .unwrap();
        let fresh = solid_reactive_ir::build(&edited, selected.vocabulary).unwrap();
        let (retained, _) = incremental.build(&edited, selected.vocabulary).unwrap();
        assert_eq!(retained, fresh, "version {version}");
        assert_eq!(
            violations(&retained),
            usize::from(source == before),
            "version {version}"
        );
    }
}

/// ADR 0283: a caller's getter read through a helper's own parameter Get
/// rests on the helper's body, which no read summary states. The helper file
/// holds no call-role syntax, so only the `FunctionSource` dependency can
/// invalidate the caller's retained result when the Get becomes an alias.
#[test]
fn incremental_result_cache_follows_a_helper_parameter_get() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let directory = temporary_directory("helper-parameter-get-cache");
    let project = directory.join("tsconfig.json");
    let app = directory.join("App.tsx");
    let helper = directory.join("helper.ts");
    let declarations = directory.join("solid-js.d.ts");
    fs::write(
        &project,
        r#"{"compilerOptions":{"jsx":"preserve","module":"ESNext","moduleResolution":"Bundler","strict":true,"target":"ES2022"},"files":["App.tsx","helper.ts","solid-js.d.ts"]}"#,
    )
    .unwrap();
    fs::write(
        &app,
        r#"import { createSignal } from "solid-js";
import { read } from "./helper";
export function App() {
  const [count] = createSignal(0);
  const value = read({ get a() { return count(); } });
  return <div>{value}</div>;
}
"#,
    )
    .unwrap();
    let before = r#"export function read(o: { a: number }) {
  return o.a;
}
"#;
    let after = r#"export function read(o: { a: number }) {
  const alias = o;
  return alias.a;
}
"#;
    fs::write(&helper, before).unwrap();
    fs::write(
        &declarations,
        r#"declare module "solid-js" {
  export function createSignal<T>(value: T): [() => T, (value: T) => void];
}
declare namespace JSX {
  interface IntrinsicElements { div: Record<string, unknown>; }
  interface Element {}
}
"#,
    )
    .unwrap();

    let project_id = project.to_string_lossy().into_owned();
    let sources = [&app, &helper]
        .iter()
        .map(|path| SourceFile {
            path: path.to_string_lossy().into_owned(),
            source: fs::read_to_string(path).unwrap().into(),
            compiler_options: CompilerOptions::default(),
        })
        .collect();
    let selected = dialect::by_id("solid-v2").unwrap();
    let typescript = open_type_facts_session(&typefacts, &project_id);
    let mut session =
        NativeIncrementalSession::open(selected, project_id, sources, typescript).unwrap();
    let first = session.analyze().unwrap();
    let mut incremental = solid_reactive_ir::IncrementalBuilder::default();
    let (initial, _) = incremental.build(&first, selected.vocabulary).unwrap();
    let violations = |program: &_| {
        selected
            .solve(program)
            .iter()
            .filter(|finding| {
                finding.rule == "strict-read-untracked" && finding.kind == "violation"
            })
            .count()
    };
    assert_eq!(
        violations(&initial),
        1,
        "the helper's Get runs the getter in the component body"
    );

    for (version, source) in [(1, after), (2, before), (3, after)] {
        let edited = session
            .edit(
                vec![SourceChange {
                    path: helper.to_string_lossy().into_owned(),
                    version,
                    source: Some(source.into()),
                    compiler_options: CompilerOptions::default(),
                }],
                None,
            )
            .unwrap();
        let fresh = solid_reactive_ir::build(&edited, selected.vocabulary).unwrap();
        let (retained, _) = incremental.build(&edited, selected.vocabulary).unwrap();
        assert_eq!(retained, fresh, "version {version}");
        assert_eq!(
            violations(&retained),
            usize::from(source == before),
            "version {version}"
        );
    }
}

/// ADR 0287: whether a hook returns its store reads the hook's own syntax --
/// the proxy trap, and the same-file forward to the function holding it --
/// which no summary or declared type carries. An edit to either has to
/// invalidate the caller's retained source discovery.
#[test]
fn incremental_result_cache_follows_a_proxy_trap_and_forward() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let directory = temporary_directory("proxy-store-cache");
    let project = directory.join("tsconfig.json");
    let app = directory.join("App.tsx");
    let hook = directory.join("hook.ts");
    let declarations = directory.join("solid-js.d.ts");
    fs::write(
        &project,
        r#"{"compilerOptions":{"jsx":"preserve","module":"ESNext","moduleResolution":"Bundler","strict":true,"target":"ES2022"},"files":["App.tsx","hook.ts","solid-js.d.ts"]}"#,
    )
    .unwrap();
    fs::write(
        &app,
        r#"import { createSignal } from "solid-js";
import { useQuery } from "./hook";
export function App() {
  const query = useQuery();
  const [config] = createSignal(query.search);
  return <div>{config()}</div>;
}
"#,
    )
    .unwrap();
    let hook_source = |trap: &str, forward: &str| {
        format!(
            r#"import {{ createStore }} from "solid-js";
type Query = {{ search: string }};
const other: Query = {{ search: "" }};
function useBase(): Query {{
  const [state] = createStore<Query>({{ search: "" }});
  return new Proxy(state, {{
    get(target, prop, receiver) {{
      return {trap};
    }},
  }});
}}
function usePlain(): Query {{
  return {{ search: "" }};
}}
export function useQuery(): Query {{
  return {forward}();
}}
"#
        )
    };
    let proven = hook_source("Reflect.get(target, prop, receiver)", "useBase");
    let other_trap = hook_source("Reflect.get(other, prop, receiver)", "useBase");
    let other_forward = hook_source("Reflect.get(target, prop, receiver)", "usePlain");
    fs::write(&hook, &proven).unwrap();
    fs::write(
        &declarations,
        r#"declare module "solid-js" {
  export function createSignal<T>(value: T): [() => T, (value: T) => void];
  export function createStore<T extends object>(value: T): [T, (fn: (state: T) => void) => void];
}
declare namespace JSX {
  interface IntrinsicElements { div: Record<string, unknown>; }
  interface Element {}
}
"#,
    )
    .unwrap();

    let project_id = project.to_string_lossy().into_owned();
    let sources = [&app, &hook]
        .iter()
        .map(|path| SourceFile {
            path: path.to_string_lossy().into_owned(),
            source: fs::read_to_string(path).unwrap().into(),
            compiler_options: CompilerOptions::default(),
        })
        .collect();
    let selected = dialect::by_id("solid-v2").unwrap();
    let typescript = open_type_facts_session(&typefacts, &project_id);
    let mut session =
        NativeIncrementalSession::open(selected, project_id, sources, typescript).unwrap();
    let first = session.analyze().unwrap();
    let mut incremental = solid_reactive_ir::IncrementalBuilder::default();
    let (initial, _) = incremental.build(&first, selected.vocabulary).unwrap();
    let violations = |program: &_| {
        selected
            .solve(program)
            .iter()
            .filter(|finding| {
                finding.rule == "strict-read-untracked" && finding.kind == "violation"
            })
            .count()
    };
    assert_eq!(
        violations(&initial),
        1,
        "the forwarded proxy reads its store in the component body"
    );

    for (version, source) in [
        (1, &other_trap),
        (2, &proven),
        (3, &other_forward),
        (4, &proven),
    ] {
        let edited = session
            .edit(
                vec![SourceChange {
                    path: hook.to_string_lossy().into_owned(),
                    version,
                    source: Some(source.clone()),
                    compiler_options: CompilerOptions::default(),
                }],
                None,
            )
            .unwrap();
        let fresh = solid_reactive_ir::build(&edited, selected.vocabulary).unwrap();
        let (retained, _) = incremental.build(&edited, selected.vocabulary).unwrap();
        assert_eq!(retained, fresh, "version {version}");
        assert_eq!(
            violations(&retained),
            usize::from(source == &proven),
            "version {version}"
        );
    }
}

#[test]
fn compiler_option_only_edit_invalidates_cached_ownership() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let (mut session, paths) = tracer_fixture_session(&typefacts);
    let first = session.analyze().unwrap();
    assert!(
        first.files[0]
            .compiler
            .ownership_regions
            .iter()
            .any(|region| region.kind == solid_facts::compiler::OwnershipRegionKind::Owned),
        "the default compiler wrapper should emit an ownership region"
    );
    let mut incremental = solid_reactive_ir::IncrementalBuilder::default();
    incremental
        .build(&first, dialect::default_dialect().vocabulary)
        .unwrap();

    let edited = session
        .edit(
            vec![SourceChange {
                path: paths[0].to_string_lossy().into_owned(),
                version: 1,
                source: Some(fs::read_to_string(&paths[0]).unwrap()),
                compiler_options: CompilerOptions {
                    effect_wrapper: Some("customEffect".into()),
                    ..CompilerOptions::default()
                },
            }],
            None,
        )
        .unwrap();
    assert!(
        edited.files[0].compiler.ownership_regions.is_empty(),
        "a custom wrapper must make no semantic ownership claim"
    );

    let fresh = solid_reactive_ir::build(&edited, dialect::default_dialect().vocabulary).unwrap();
    let (retained, _) = incremental
        .build(&edited, dialect::default_dialect().vocabulary)
        .unwrap();
    assert_eq!(retained, fresh);
}

#[test]
fn cross_file_jsx_use_invalidates_cached_component_identity() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let directory = temporary_directory("component-identity-invalidation");
    let project = directory.join("tsconfig.json");
    let definition = directory.join("Card.tsx");
    let usage = directory.join("usage.tsx");
    let declarations = directory.join("solid-js.d.ts");
    fs::write(
        &project,
        r#"{"compilerOptions":{"jsx":"preserve","module":"ESNext","moduleResolution":"Bundler","strict":true,"target":"ES2022"},"files":["Card.tsx","usage.tsx","solid-js.d.ts"]}"#,
    )
    .unwrap();
    fs::write(
        &definition,
        r#"import { createEffect } from "solid-js";
export const Card = () => {
  const view = <div />;
  createEffect(() => 1, () => {});
  return view;
};
"#,
    )
    .unwrap();
    let rendered = r#"import { Card } from "./Card";
void <Card />;
"#;
    fs::write(&usage, rendered).unwrap();
    fs::write(
        &declarations,
        r#"declare module "solid-js" {
  export function createEffect<T>(compute: () => T, apply: (value: T) => void): void;
}
declare namespace JSX {
  interface IntrinsicElements { div: Record<string, never>; }
}
"#,
    )
    .unwrap();

    let project_id = project.to_string_lossy().into_owned();
    let sources = [&definition, &usage]
        .into_iter()
        .map(|path| SourceFile {
            path: path.to_string_lossy().into_owned(),
            source: fs::read_to_string(path).unwrap().into(),
            compiler_options: CompilerOptions::default(),
        })
        .collect();
    let typescript = open_type_facts_session(&typefacts, &project_id);
    let selected = dialect::by_id("solid-v2").unwrap();
    let mut session =
        NativeIncrementalSession::open(selected, project_id, sources, typescript).unwrap();
    let first = session.analyze().unwrap();
    let mut incremental = solid_reactive_ir::IncrementalBuilder::default();
    let (initial, _) = incremental.build(&first, selected.vocabulary).unwrap();
    assert!(
        selected
            .solve(&initial)
            .iter()
            .all(|finding| finding.rule != "missing-owner"),
        "the JSX use should prove Card is an owned component"
    );

    let unrendered = r#"import { Card } from "./Card";
void Card;
"#;
    fs::write(&usage, unrendered).unwrap();
    let edited = session
        .edit(
            vec![SourceChange {
                path: usage.to_string_lossy().into_owned(),
                version: 1,
                source: Some(unrendered.into()),
                compiler_options: CompilerOptions::default(),
            }],
            None,
        )
        .unwrap();
    let fresh = solid_reactive_ir::build(&edited, selected.vocabulary).unwrap();
    let (retained, _) = incremental.build(&edited, selected.vocabulary).unwrap();
    let fresh_findings = selected.solve(&fresh);
    assert!(
        fresh_findings.iter().any(|finding| {
            finding.rule == "missing-owner" && finding.primary_location.path.ends_with("Card.tsx")
        }),
        "without a JSX use, the exported untyped factory has no proven owner: {fresh_findings:#?}"
    );
    assert_eq!(
        retained, fresh,
        "an edit in a reference file must invalidate component-dependent artifacts in Card.tsx"
    );
}

#[test]
fn incremental_reactive_ir_reuses_semantic_indexes_for_same_shape_body_edit() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let (mut session, paths) = tracer_fixture_session(&typefacts);
    let first = session.analyze().unwrap();
    let mut incremental = solid_reactive_ir::IncrementalBuilder::default();
    incremental
        .build(&first, dialect::default_dialect().vocabulary)
        .unwrap();

    // A same-length JSX text edit keeps every fact's shape. A call argument's
    // literal is not such an edit (ADR 0252): owner guards read it, so a
    // change there must recompute the owner fixed point.
    let original = fs::read_to_string(&paths[0]).unwrap();
    let changed = original.replacen(">Read</button>", ">Load</button>", 1);
    assert_ne!(changed, original);
    let edited = session
        .edit(
            vec![SourceChange {
                path: paths[0].to_string_lossy().into_owned(),
                version: 1,
                source: Some(changed),
                compiler_options: CompilerOptions::default(),
            }],
            None,
        )
        .unwrap();
    let fresh = solid_reactive_ir::build(&edited, dialect::default_dialect().vocabulary).unwrap();
    let (retained, timings) = incremental
        .build(&edited, dialect::default_dialect().vocabulary)
        .unwrap();

    assert_eq!(retained, fresh);
    assert!(timings.typescript_indexes_reused);
    assert!(timings.reachability_reused);
    // The edited file's chunks rerun; every other file's are kept.
    assert_eq!(
        timings.local_access_reused_files + 1,
        u64::try_from(edited.files.len()).unwrap()
    );
    assert_eq!(timings.local_access_recomputed_files, 1);
    assert_eq!(
        timings.owner_reused_files + 1,
        u64::try_from(edited.files.len()).unwrap()
    );
    assert_eq!(timings.owner_recomputed_files, 1);
}

#[test]
fn incremental_owner_fragments_match_fresh_owner_fixtures() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    for fixture_name in ["owner-presence", "leaf-owner", "directive-owned-effects"] {
        let fixture = root.join(format!("fixtures/reactive-ir/{fixture_name}"));
        let project = fixture.join("tsconfig.json").canonicalize().unwrap();
        let project_id = project.to_string_lossy().into_owned();
        let app = fixture.join("App.tsx").canonicalize().unwrap();
        let sources = vec![SourceFile {
            path: app.to_string_lossy().into_owned(),
            source: fs::read_to_string(&app).unwrap().into(),
            compiler_options: CompilerOptions::default(),
        }];
        let typescript = TypeFactsSession::open(&typefacts, &project_id, &[]).unwrap();
        let mut session = NativeIncrementalSession::open(
            dialect::default_dialect(),
            project_id,
            sources,
            typescript,
        )
        .unwrap();
        let first = session.analyze().unwrap();
        let mut incremental = solid_reactive_ir::IncrementalBuilder::default();
        incremental
            .build(&first, dialect::default_dialect().vocabulary)
            .unwrap();
        let edited = session
            .edit(
                vec![SourceChange {
                    path: app.to_string_lossy().into_owned(),
                    version: 1,
                    source: Some(format!(
                        "// owner fragment edit\n{}",
                        fs::read_to_string(&app).unwrap()
                    )),
                    compiler_options: CompilerOptions::default(),
                }],
                None,
            )
            .unwrap();
        let fresh =
            solid_reactive_ir::build(&edited, dialect::default_dialect().vocabulary).unwrap();
        let (retained, timings) = incremental
            .build(&edited, dialect::default_dialect().vocabulary)
            .unwrap();
        assert_eq!(retained, fresh, "fixture {fixture_name}");
        assert_eq!(
            timings.owner_reused_files + timings.owner_recomputed_files,
            u64::try_from(edited.files.len()).unwrap(),
            "fixture {fixture_name}"
        );
    }
}

#[test]
fn cancelled_edit_before_any_send_leaves_the_session_consistent() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let (mut session, paths) = tracer_fixture_session(&typefacts);
    session.analyze().unwrap();
    let cancelled = std::sync::atomic::AtomicBool::new(true);
    let error = session
        .edit(vec![app_edit(&paths)], Some(&cancelled))
        .unwrap_err();
    assert!(
        matches!(error, solid_facts_backend::BackendError::Cancelled),
        "expected cancellation, got {error}"
    );
    assert_eq!(
        session.generation(),
        1,
        "nothing was sent, so the generation must not advance"
    );
    cancelled.store(false, std::sync::atomic::Ordering::Release);
    let edited = session
        .edit(vec![app_edit(&paths)], Some(&cancelled))
        .unwrap();
    assert_eq!(session.generation(), 2);
    let reanalyzed = session.analyze().unwrap();
    assert_eq!(
        serde_json::to_vec(&edited).unwrap(),
        serde_json::to_vec(&reanalyzed).unwrap(),
        "an edit exchange must answer exactly what a follow-up analysis answers"
    );
}

/// Wraps the real service in a script that arms one crash marker, so the
/// session under test observes a deterministic service death and the
/// restarted service (same wrapper, marker consumed) runs normally.
#[cfg(unix)]
fn crash_armed_service(
    typefacts_executable: &str,
    variable: &str,
    label: &str,
) -> (PathBuf, PathBuf) {
    use std::os::unix::fs::PermissionsExt as _;
    let directory = temporary_directory(label);
    let marker = directory.join("crash-marker");
    fs::write(&marker, b"armed").unwrap();
    let wrapper = directory.join("solid-typefacts-crashing.sh");
    fs::write(
        &wrapper,
        format!(
            "#!/bin/sh\n{variable}={marker} exec {typefacts_executable} \"$@\"\n",
            marker = marker.display(),
        ),
    )
    .unwrap();
    fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o755)).unwrap();
    (wrapper, marker)
}

#[cfg(unix)]
#[test]
fn edit_recovers_when_the_service_dies_before_the_update_lands() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let (wrapper, marker) = crash_armed_service(
        &typefacts,
        "SOLID_TYPEFACTS_CRASH_BEFORE_UPDATE",
        "crash-before-update",
    );
    // Arm the marker only after the session is warm, so the crash hits the
    // edit's update half: the update never lands, recovery replays the
    // pre-edit state, and the retry re-sends both halves.
    fs::remove_file(&marker).unwrap();
    let (mut session, paths) = tracer_fixture_session(&wrapper.to_string_lossy());
    session.analyze().unwrap();
    fs::write(&marker, b"armed").unwrap();
    let edited = session.edit(vec![app_edit(&paths)], None).unwrap();
    assert_eq!(session.generation(), 2);
    assert!(!marker.exists(), "the crash marker must have been consumed");
    let reanalyzed = session.analyze().unwrap();
    assert_eq!(
        serde_json::to_vec(&edited).unwrap(),
        serde_json::to_vec(&reanalyzed).unwrap(),
        "recovery must converge on the same facts"
    );
}

#[cfg(unix)]
#[test]
fn edit_recovers_when_the_service_dies_between_update_and_analyze() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let (wrapper, marker) = crash_armed_service(
        &typefacts,
        "SOLID_TYPEFACTS_CRASH_BEFORE_ANALYZE",
        "crash-before-analyze",
    );
    // Arm the marker only after the session is warm, so the crash hits the
    // edit's analyze half: the update has landed and the generation is
    // committed, recovery replays the committed generation, and the retry
    // re-sends only the analyze.
    fs::remove_file(&marker).unwrap();
    let (mut session, paths) = tracer_fixture_session(&wrapper.to_string_lossy());
    session.analyze().unwrap();
    fs::write(&marker, b"armed").unwrap();
    let edited = session.edit(vec![app_edit(&paths)], None).unwrap();
    assert_eq!(session.generation(), 2);
    assert!(!marker.exists(), "the crash marker must have been consumed");
    let reanalyzed = session.analyze().unwrap();
    assert_eq!(
        serde_json::to_vec(&edited).unwrap(),
        serde_json::to_vec(&reanalyzed).unwrap(),
        "recovery must converge on the same facts"
    );
}

/// A launch-counting wrapper around the real producer. Each launch appends a
/// line to the returned log. `rearm_after` names a file: while it exists every
/// launch re-creates the panic marker, so the producer panics on every launch;
/// otherwise the marker is whatever the test wrote (one shot, consumed).
#[cfg(unix)]
fn panicking_service(
    typefacts_executable: &str,
    label: &str,
) -> (PathBuf, PathBuf, PathBuf, PathBuf) {
    use std::os::unix::fs::PermissionsExt as _;
    let directory = temporary_directory(label);
    let marker = directory.join("panic-marker");
    let launches = directory.join("launches");
    let rearm = directory.join("rearm");
    let wrapper = directory.join("solid-typefacts-panicking.sh");
    fs::write(
        &wrapper,
        format!(
            "#!/bin/sh\necho launch >> '{launches}'\n[ -e '{rearm}' ] && : > '{marker}'\n\
             SOLID_TYPEFACTS_PANIC_BEFORE_UPDATE='{marker}' exec '{typefacts_executable}' \"$@\"\n",
            launches = launches.display(),
            rearm = rearm.display(),
            marker = marker.display(),
        ),
    )
    .unwrap();
    fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o755)).unwrap();
    (wrapper, marker, launches, rearm)
}

#[cfg(unix)]
fn launch_count(launches: &std::path::Path) -> usize {
    fs::read_to_string(launches)
        .map(|text| text.lines().count())
        .unwrap_or(0)
}

fn nth_edit(paths: &[PathBuf], version: u64, label: &str) -> SourceChange {
    SourceChange {
        path: paths[0].to_string_lossy().into_owned(),
        version,
        source: Some(format!(
            "// {label}\n{}",
            fs::read_to_string(&paths[0]).unwrap()
        )),
        compiler_options: CompilerOptions::default(),
    }
}

/// Runs `work` on a thread and fails the test if it does not finish: a client
/// that hangs on a dead producer is the defect under test.
#[cfg(unix)]
fn bounded<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> T {
    let (done_tx, done_rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = done_tx.send(work());
    });
    done_rx
        .recv_timeout(std::time::Duration::from_secs(120))
        .expect("the client hung after a producer panic")
}

/// A panic on the producer's worker thread stops the process (fail-stop hook);
/// the client's crash path relaunches, replays the earlier accepted edit, and
/// re-sends the failed one. The facts must equal a session that never crashed.
#[cfg(unix)]
#[test]
fn a_producer_panic_on_a_later_edit_recovers_by_replay() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let (wrapper, marker, launches, _) = panicking_service(&typefacts, "panic-recovers");
    let wrapper = wrapper.to_string_lossy().into_owned();
    let (recovered, launched) = bounded({
        let launches = launches.clone();
        move || {
            let (mut session, paths) = tracer_fixture_session(&wrapper);
            session.analyze().unwrap();
            session
                .edit(vec![nth_edit(&paths, 1, "first")], None)
                .unwrap();
            assert_eq!(launch_count(&launches), 1, "no relaunch before the panic");
            fs::write(&marker, b"armed").unwrap();
            let facts = session
                .edit(vec![nth_edit(&paths, 2, "second")], None)
                .unwrap();
            assert!(!marker.exists(), "the panic marker must have been consumed");
            (serde_json::to_vec(&facts).unwrap(), launch_count(&launches))
        }
    });
    assert_eq!(launched, 2, "the panic must cost exactly one relaunch");
    let control = bounded(move || {
        let (mut session, paths) = tracer_fixture_session(&typefacts);
        session.analyze().unwrap();
        session
            .edit(vec![nth_edit(&paths, 1, "first")], None)
            .unwrap();
        let facts = session
            .edit(vec![nth_edit(&paths, 2, "second")], None)
            .unwrap();
        serde_json::to_vec(&facts).unwrap()
    });
    assert_eq!(
        recovered, control,
        "replayed overlays must give the facts of a session that never crashed"
    );
}

/// A producer that panics on every launch never certifies: the edit fails
/// within the bounded recovery attempts (no facts, so nothing is certified),
/// and the relaunches stop.
#[cfg(unix)]
#[test]
fn a_producer_that_panics_on_every_launch_fails_the_edit_without_looping() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let (wrapper, marker, launches, rearm) = panicking_service(&typefacts, "panic-always");
    let wrapper = wrapper.to_string_lossy().into_owned();
    let (edited, after) = bounded({
        let launches = launches.clone();
        move || {
            let (mut session, paths) = tracer_fixture_session(&wrapper);
            session.analyze().unwrap();
            session
                .edit(vec![nth_edit(&paths, 1, "first")], None)
                .unwrap();
            fs::write(&rearm, b"").unwrap();
            fs::write(&marker, b"armed").unwrap();
            let edited = session.edit(vec![nth_edit(&paths, 2, "second")], None);
            let count = launch_count(&launches);
            (edited.map(|_| ()), count)
        }
    });
    assert!(edited.is_err(), "a panicking producer must not certify");
    assert!(after > 1, "recovery must have relaunched");
    assert!(
        after <= 8,
        "relaunches must be bounded, got {after} launches"
    );
}

/// A generation that asks for no identity attestation reports no counts.
///
/// The measurement is per generation, and a retained session keeps the last
/// one. An edit that removes the program's only bare specifier asks for nothing
/// -- there is no specifier a package could be matched by -- so leaving the previous
/// generation's counts in place would make `SOLID_CHECKER_TIMINGS` attribute
/// them to a generation that never issued the operation.
#[test]
fn an_identity_measurement_describes_the_current_generation() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let directory = temporary_directory("identity-scope-reset");
    let package = directory.join("node_modules/plain-package");
    fs::create_dir_all(&package).unwrap();
    fs::write(
        package.join("package.json"),
        "{\"name\":\"plain-package\",\"version\":\"1.0.0\",\"types\":\"index.d.ts\"}\n",
    )
    .unwrap();
    fs::write(
        package.join("index.d.ts"),
        "export declare function mapValue(map: (index: number) => unknown): void;\n",
    )
    .unwrap();
    let app = directory.join("App.ts");
    fs::write(
        &app,
        "import { mapValue } from \"plain-package\";\nexport const use = () => mapValue(() => 1);\n",
    )
    .unwrap();
    let project = directory.join("tsconfig.json");
    fs::write(
        &project,
        "{\"compilerOptions\":{\"module\":\"ESNext\",\"moduleResolution\":\"Bundler\",\
         \"strict\":true,\"target\":\"ES2022\"},\"include\":[\"App.ts\"]}\n",
    )
    .unwrap();
    let project_id = project
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let sources = vec![SourceFile {
        path: app.canonicalize().unwrap().to_string_lossy().into_owned(),
        source: fs::read_to_string(&app).unwrap().into(),
        compiler_options: CompilerOptions::default(),
    }];
    let typescript = TypeFactsSession::open(&typefacts, &project_id, &[]).unwrap();
    let mut session =
        NativeIncrementalSession::open(dialect::default_dialect(), project_id, sources, typescript)
            .unwrap();
    session.analyze().unwrap();
    assert_eq!(session.last_import_identity().requested, 1);
    assert_eq!(session.last_import_identity().specifiers, 1);

    session
        .edit(
            vec![SourceChange {
                path: app.canonicalize().unwrap().to_string_lossy().into_owned(),
                version: 1,
                source: Some("export const use = () => 1;\n".to_owned()),
                compiler_options: CompilerOptions::default(),
            }],
            None,
        )
        .unwrap();
    // Every program file is attested (ADR 0219), so the edited file is still
    // asked about; the specifier it no longer has is not carried over from
    // the previous generation.
    let measurement = session.last_import_identity();
    assert_eq!(measurement.requested, 1);
    assert_eq!(measurement.attested, 1);
    assert_eq!(measurement.unknown, 0);
    assert_eq!(measurement.specifiers, 0);
    fs::remove_dir_all(directory).unwrap();
}
