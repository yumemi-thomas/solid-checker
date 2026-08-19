#[path = "support/process.rs"]
mod support;

use std::process::Command;
use std::{env, fs, path::PathBuf};

use solid_facts::compiler::CompilerOptions;
use solid_facts_backend::{
    CacheStats, NativeIncrementalSession, SourceChange, SourceFile, TypeFactsSession,
    build_project, dialect,
};
use support::{decode_findings, temporary_directory};

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
        ("leaf-reexport-flush.tsx", "flush-in-forbidden-scope"),
        ("leaf-reexport-cleanup.tsx", "cleanup-in-forbidden-scope"),
        ("owned-reexport-memo.tsx", "reactive-write-in-owned-scope"),
        (
            "owned-reexport-refresh.tsx",
            "reactive-write-in-owned-scope",
        ),
        ("owned-reexport-action.tsx", "action-called-in-owned-scope"),
        ("effect-apply-parameter.tsx", "strict-read-untracked"),
        ("effect-apply-member.tsx", "strict-read-untracked"),
        ("after-await-member.tsx", "reactive-read-after-await"),
        ("after-await-parameter.tsx", "reactive-read-after-await"),
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
            "component-reactive-early-return.tsx",
            "strict-read-untracked",
        ),
        (
            "component-reactive-conditional-return.tsx",
            "strict-read-untracked",
        ),
        (
            "component-props-parameter-destructure.tsx",
            "component-props-destructure",
        ),
        (
            "component-props-body-destructure.tsx",
            "component-props-destructure",
        ),
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
        (
            "component-props-tracked-destructure.tsx",
            "component-props-destructure",
        ),
        ("noncomponent-object-read.ts", "strict-read-untracked"),
        (
            "noncomponent-object-destructure.ts",
            "component-props-destructure",
        ),
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
                finding["rule"] == "component-props-destructure"
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
    assert_eq!(incremental_timings.source_discovery_recomputed_files, 2);
    assert_eq!(incremental_timings.typed_accessor_recomputed_files, 2);
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
    assert!(!timings.local_accesses_reused);
    assert!(!timings.interprocedural_reused);
    assert!(!timings.owner_fixed_point_reused);
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
    assert_eq!(
        timings.source_discovery_reused_files + timings.source_discovery_recomputed_files,
        u64::try_from(edited.files.len()).unwrap()
    );
    assert_eq!(
        timings.typed_accessor_reused_files + timings.typed_accessor_recomputed_files,
        u64::try_from(edited.files.len()).unwrap()
    );
    assert_eq!(
        timings.interprocedural_graph_reused_files,
        timings.source_discovery_reused_files
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
    let (retained, timings) = incremental
        .build(&edited, dialect::default_dialect().vocabulary)
        .unwrap();
    assert_eq!(retained, fresh);
    assert!(!timings.owner_fixed_point_reused);
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
            .all(|finding| finding.rule != "no-owner-effect"),
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
            finding.rule == "no-owner-effect" && finding.primary_location.path.ends_with("Card.tsx")
        }),
        "without a JSX use, the exported untyped factory has no proven owner: {fresh_findings:#?}"
    );
    assert_eq!(
        retained, fresh,
        "an edit in a reference file must invalidate component-dependent artifacts in Card.tsx"
    );
}

#[test]
fn incremental_contract_exports_drop_removed_names() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let (mut session, paths) = tracer_fixture_session(&typefacts);
    let first = session.analyze().unwrap();
    let mut incremental = solid_reactive_ir::IncrementalBuilder::default();
    let (initial, _) = incremental
        .build(&first, dialect::default_dialect().vocabulary)
        .unwrap();
    assert!(initial.contract_exports.contains_key("Bad"));

    let original = fs::read_to_string(&paths[0]).unwrap();
    let changed = original.replacen("export function Bad", "function Bad", 1);
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
    let (retained, _) = incremental
        .build(&edited, dialect::default_dialect().vocabulary)
        .unwrap();

    assert_eq!(retained, fresh);
    assert!(!retained.contract_exports.contains_key("Bad"));
}

#[test]
fn incremental_contract_exports_refresh_changed_summaries() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let (mut session, paths) = tracer_fixture_session(&typefacts);
    let first = session.analyze().unwrap();
    let mut incremental = solid_reactive_ir::IncrementalBuilder::default();
    let (initial, _) = incremental
        .build(&first, dialect::default_dialect().vocabulary)
        .unwrap();
    let initial_good = initial.contract_exports.get("Good").cloned().unwrap();

    let original = fs::read_to_string(&paths[0]).unwrap();
    let changed = original.replacen(
        "return <div>{count()}</div>;",
        "return <div>static</div>;",
        1,
    );
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
    let (retained, _) = incremental
        .build(&edited, dialect::default_dialect().vocabulary)
        .unwrap();

    assert_eq!(retained, fresh);
    assert_ne!(
        retained.contract_exports.get("Good"),
        Some(&initial_good),
        "a changed inferred function contract must invalidate its fragment"
    );
}

#[test]
fn incremental_contract_exports_refresh_cross_file_reexports() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let directory = temporary_directory("contract-reexport-invalidation");
    let project = directory.join("tsconfig.json");
    let source = directory.join("source.ts");
    let entrypoint = directory.join("index.ts");
    fs::write(
        &project,
        r#"{"compilerOptions":{"allowImportingTsExtensions":true,"noEmit":true},"files":["source.ts","index.ts"]}"#,
    )
    .unwrap();
    let original = "export function source() { return 1; }\n";
    fs::write(&source, original).unwrap();
    fs::write(
        &entrypoint,
        "export { source as publicSource } from \"./source.ts\";\n",
    )
    .unwrap();
    let project_id = project.to_string_lossy().into_owned();
    let sources = [&source, &entrypoint]
        .into_iter()
        .map(|path| SourceFile {
            path: path.to_string_lossy().into_owned(),
            source: fs::read_to_string(path).unwrap().into(),
            compiler_options: CompilerOptions::default(),
        })
        .collect();
    let typescript = TypeFactsSession::open(&typefacts, &project_id, &[]).unwrap();
    let mut session =
        NativeIncrementalSession::open(dialect::default_dialect(), project_id, sources, typescript)
            .unwrap();
    let first = session.analyze().unwrap();
    let mut incremental = solid_reactive_ir::IncrementalBuilder::default();
    let (initial, _) = incremental
        .build(&first, dialect::default_dialect().vocabulary)
        .unwrap();
    assert_eq!(
        initial
            .contract_exports
            .get("publicSource")
            .map(|summary| summary.async_behavior.as_str()),
        Some("")
    );

    let changed = "export async function source() { return 1; }\n";
    fs::write(&source, changed).unwrap();
    let edited = session
        .edit(
            vec![SourceChange {
                path: source.to_string_lossy().into_owned(),
                version: 1,
                source: Some(changed.into()),
                compiler_options: CompilerOptions::default(),
            }],
            None,
        )
        .unwrap();
    let fresh = solid_reactive_ir::build(&edited, dialect::default_dialect().vocabulary).unwrap();
    let (retained, _) = incremental
        .build(&edited, dialect::default_dialect().vocabulary)
        .unwrap();

    assert_eq!(retained, fresh);
    assert_eq!(
        retained
            .contract_exports
            .get("publicSource")
            .map(|summary| summary.async_behavior.as_str()),
        Some("promise"),
        "an unchanged re-export fragment must follow its changed source contract"
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

    let original = fs::read_to_string(&paths[1]).unwrap();
    let changed = original.replacen("createSignal(0)", "createSignal(1)", 1);
    assert_ne!(changed, original);
    let edited = session
        .edit(
            vec![SourceChange {
                path: paths[1].to_string_lossy().into_owned(),
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
    assert!(timings.local_accesses_reused);
    assert_eq!(
        timings.local_access_reused_files,
        u64::try_from(edited.files.len()).unwrap()
    );
    assert_eq!(timings.local_access_recomputed_files, 0);
    assert!(timings.interprocedural_reused);
    assert!(timings.owner_fixed_point_reused);
    assert_eq!(
        timings.owner_reused_files,
        u64::try_from(edited.files.len()).unwrap()
    );
    assert_eq!(timings.owner_recomputed_files, 0);
}

#[test]
fn incremental_owner_fragments_match_fresh_owner_fixtures() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    for fixture_name in ["owner-presence", "leaf-owner"] {
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
