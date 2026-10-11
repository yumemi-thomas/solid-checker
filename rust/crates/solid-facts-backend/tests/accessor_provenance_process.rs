//! Transported source-discovered summaries must follow cross-file edits.
use std::{env, fs, path::PathBuf};

use crate::process_support::temporary_directory;
use solid_facts::compiler::CompilerOptions;
use solid_facts_backend::{NativeIncrementalSession, SourceChange, TypeFactsSession, dialect};

fn copied_fixture(label: &str, sources: &[&str]) -> PathBuf {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let fixture = root.join("fixtures/reactive-ir/accessor-provenance");
    let directory = temporary_directory(label);
    for name in ["tsconfig.json", "solid-js.d.ts"]
        .into_iter()
        .chain(sources.iter().copied())
    {
        fs::copy(fixture.join(name), directory.join(name)).unwrap();
    }
    let stub = directory.join("node_modules/solid-js");
    fs::create_dir_all(&stub).unwrap();
    fs::copy(
        fixture.join("node_modules/solid-js/package.json"),
        stub.join("package.json"),
    )
    .unwrap();
    directory
}

#[test]
fn transported_member_helper_edits_match_cold_in_both_directions() {
    let typefacts = env::var("SOLID_TYPEFACTS_BIN").expect("arm the real Type Facts process");
    let directory = copied_fixture(
        "transport-summary-edit",
        &["cache-helper.ts", "cache-factory.ts", "Cache.tsx"],
    );
    let project = directory
        .join("tsconfig.json")
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let helper = directory
        .join("cache-helper.ts")
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let caller = directory
        .join("Cache.tsx")
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let original = fs::read_to_string(&helper).unwrap();
    let without_read = original.replace("void count()", "void plain()");
    assert_ne!(without_read, original);
    assert_eq!(
        without_read.len(),
        original.len(),
        "hold declaration and caller spans stable"
    );
    let vocabulary = dialect::default_dialect().vocabulary;
    let typescript = TypeFactsSession::open(&typefacts, &project, &[]).unwrap();
    let (mut session, _) =
        NativeIncrementalSession::open_pipelined(dialect::default_dialect(), project, typescript)
            .unwrap();
    let first = session.analyze().unwrap();
    let mut incremental = solid_reactive_ir::IncrementalBuilder::default();
    let (program, _) = incremental.build(&first, vocabulary).unwrap();
    let consumer_read = |program: &solid_reactive_ir::Program| {
        program.reads.iter().any(|read| {
            read.location.path.as_ref() == caller
                && read.transported
                && !read.summary_attributed
                && read.execution == solid_reactive_ir::ExecutionRole::UntrackedRendering
        })
    };
    assert!(
        consumer_read(&program),
        "the member must be admitted and carry a proven body read"
    );
    let unrelated_write =
        format!("{original}\nconst other = {{ ready: () => 0 }}; other.ready = () => 0;\n");
    let unknown_assign =
        format!("{original}\nconst other = {{ plain: 0 }}; Object.assign({{}}, other);\n");
    let spread_assign =
        format!("{original}\nconst other = {{ plain: 0 }}; Object.assign({{}}, {{ ...other }});\n");
    for (version, source, reads) in [
        (1, without_read, false),
        (2, original.clone(), true),
        (3, unrelated_write, false),
        (4, original.clone(), true),
        (5, unknown_assign, false),
        (6, original.clone(), true),
        (7, spread_assign, false),
        (8, original, true),
    ] {
        let facts = session
            .edit(
                vec![SourceChange {
                    path: helper.clone(),
                    version,
                    source: Some(source),
                    compiler_options: CompilerOptions::default(),
                }],
                None,
            )
            .unwrap();
        assert!(session.last_build_timings().source_files_reused > 0);
        let cold = solid_reactive_ir::build(&facts, vocabulary).unwrap();
        let (warm, timings) = incremental.build(&facts, vocabulary).unwrap();
        assert!(
            timings.source_discovery_reused_files > 0,
            "exercise reuse of unchanged caller/factory facts"
        );
        assert_eq!(
            consumer_read(&cold),
            reads,
            "the helper summary or unrelated member-name write must change transport"
        );
        assert_eq!(
            warm, cold,
            "transported target dependency must follow the helper summary, edit {version}"
        );
    }
    drop(session);
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn context_receiver_census_edits_match_cold_in_both_directions() {
    let typefacts = env::var("SOLID_TYPEFACTS_BIN").expect("arm the real Type Facts process");
    let directory = copied_fixture(
        "transport-census-edit",
        &["ContextStability.tsx", "context-mutation.ts"],
    );
    let project = directory
        .join("tsconfig.json")
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let mutation = directory
        .join("context-mutation.ts")
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let controls = directory.join("ContextStability.tsx");
    let source = fs::read_to_string(&controls).unwrap();
    fs::write(
        &controls,
        source
            .replace("replaced.stale = () => 0;", "void replaced.stale();")
            .replace(
                "useContext(Unbound).stale = () => 0;",
                "void useContext(Unbound).stale();",
            ),
    )
    .unwrap();
    let original = fs::read_to_string(&mutation).unwrap();
    let closed = original.replace("replaced.stale = () => 0;", "void replaced.stale();");
    assert_ne!(closed, original);
    let vocabulary = dialect::default_dialect().vocabulary;
    let typescript = TypeFactsSession::open(&typefacts, &project, &[]).unwrap();
    let (mut session, _) =
        NativeIncrementalSession::open_pipelined(dialect::default_dialect(), project, typescript)
            .unwrap();
    let first = session.analyze().unwrap();
    let mut incremental = solid_reactive_ir::IncrementalBuilder::default();
    let (program, _) = incremental.build(&first, vocabulary).unwrap();
    let consumer_read = |program: &solid_reactive_ir::Program| {
        program.reads.iter().any(|read| {
            read.context.as_ref() == "CrossConsumerMutation"
                && read.transported
                && !read.summary_attributed
                && !read.is_uncertifiable()
        })
    };
    assert!(!consumer_read(&program));
    for (version, source, reads) in [(1, closed, true), (2, original, false)] {
        let facts = session
            .edit(
                vec![SourceChange {
                    path: mutation.clone(),
                    version,
                    source: Some(source),
                    compiler_options: CompilerOptions::default(),
                }],
                None,
            )
            .unwrap();
        let cold = solid_reactive_ir::build(&facts, vocabulary).unwrap();
        let (warm, _) = incremental.build(&facts, vocabulary).unwrap();
        assert_eq!(
            consumer_read(&cold),
            reads,
            "the shared receiver census must change transport"
        );
        assert_eq!(
            warm, cold,
            "result reuse must follow the complete context census, edit {version}"
        );
    }
    drop(session);
    fs::remove_dir_all(directory).unwrap();
}
