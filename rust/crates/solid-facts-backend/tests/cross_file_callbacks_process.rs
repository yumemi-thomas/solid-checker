//! Cross-file returned-adapter proofs, observed end to end.
//!
//! Solid 1.x routes some callbacks through a function the primitive returns
//! (`mapArray`, `indexArray`, `on`, `createSelector`, `createReaction`,
//! `lazy`, `produce`). Whether those callbacks run at all is a fact about the
//! *whole project*: the factory call lives in one file and the invocation of its
//! result can live in any other. The engine's per-file cache fragments must
//! therefore not survive a change to a file they never mention.
//!
//! The second test here guards the opposite failure: narrowing reachability to
//! the arguments that carry a modelled callback fact, which loses every function
//! the runtime reaches through an options object.

use std::{env, fs, path::PathBuf};

use solid_facts::compiler::CompilerOptions;
use solid_facts_backend::{
    NativeIncrementalSession, SourceChange, SourceFile, TypeFactsSession, dialect,
};

/// The 1.x dialect, chosen explicitly: these fixtures carry no `node_modules`
/// for detection to read, and a session is always opened with a dialect anyway.
fn solid_v1() -> &'static dialect::Dialect {
    dialect::by_id("solid-v1").expect("the 1.x dialect is registered")
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("tests/fixtures/{name}"))
}

fn source_file(path: &PathBuf) -> SourceFile {
    SourceFile {
        path: path.canonicalize().unwrap().to_string_lossy().into_owned(),
        source: fs::read_to_string(path).unwrap().into(),
        compiler_options: CompilerOptions::default(),
    }
}

/// Editing the *only* file that invokes an adapter must invalidate the answers
/// cached for the file that created it.
///
/// `adapter.ts` calls `mapArray`, whose list and mapper callbacks run only when
/// the returned function runs; `consumer.ts` holds the project's single
/// invocation. Removing that invocation turns the read inside the mapper from a
/// live reactive read into dormant code -- a change to `adapter.ts`'s facts made
/// by touching a file whose name never appears in `adapter.ts`.
///
/// Held to the from-scratch answer rather than to a hand-listed finding set, so
/// the test cannot pass by agreeing with a wrong baseline.
#[test]
fn editing_the_only_invocation_site_invalidates_the_factory_file_fragments() {
    let Ok(typefacts) = env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let fixture = fixture("solid-1x-cross-file-adapter");
    let project = fixture.join("tsconfig.json").canonicalize().unwrap();
    let project_id = project.to_string_lossy().into_owned();
    let adapter = fixture.join("adapter.ts");
    let consumer = fixture.join("consumer.ts");
    let sources = vec![source_file(&adapter), source_file(&consumer)];

    let typescript = TypeFactsSession::open(&typefacts, &project_id, &[]).unwrap();
    let mut session =
        NativeIncrementalSession::open(solid_v1(), project_id, sources, typescript).unwrap();
    let first = session.analyze().unwrap();
    let mut incremental = solid_reactive_ir::IncrementalBuilder::default();
    let first_program = incremental.build(&first, solid_v1().vocabulary).unwrap().0;

    // The premise: with the invocation in place, the mapper's read of `scale`
    // is a live reactive read attributed to `adapter.ts`.
    let adapter_path = adapter
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let adapter_reads = |program: &solid_reactive_ir::Program| {
        program
            .reads
            .iter()
            .filter(|read| read.location.path.as_ref() == adapter_path)
            .count()
    };
    assert!(
        adapter_reads(&first_program) > 0,
        "the invoked mapper must contribute at least one read from adapter.ts"
    );

    // Remove the invocation and nothing else: `scaled()` becomes `scaled`, so
    // every symbol reference in `consumer.ts` survives and only the call shape
    // changes.
    let original = fs::read_to_string(&consumer).unwrap();
    let edited_source = original.replace("return scaled();", "return scaled;");
    assert_ne!(edited_source, original, "consumer.ts invocation marker");
    let edited = session
        .edit(
            vec![SourceChange {
                path: consumer
                    .canonicalize()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned(),
                version: 1,
                source: Some(edited_source),
                compiler_options: CompilerOptions::default(),
            }],
            None,
        )
        .unwrap();

    let fresh = solid_reactive_ir::build(&edited, solid_v1().vocabulary).unwrap();
    let (retained, _) = incremental.build(&edited, solid_v1().vocabulary).unwrap();

    // The edit really does change what is true about the *other* file, so a
    // cache that ignores it is observably stale rather than merely coarse.
    assert_eq!(
        adapter_reads(&fresh),
        0,
        "a discarded adapter's mapper reads nothing"
    );
    assert_eq!(
        retained, fresh,
        "adapter.ts fragments must follow the cross-file invocation proof"
    );
}

/// A function the runtime reaches only through a primitive's options object
/// stays reachable, and the reads and writes inside it stay in the IR.
///
/// `createMemo(fn, value, { equals })` invokes the comparator on every
/// recompute. The dialect models positional callbacks, so there is no
/// callback-execution fact at the options argument; reachability that demanded
/// one drops the comparator, and with it the read of `tolerance` and the write
/// through `setTolerance` -- see the fixture for why no other reachability path
/// covers for it.
#[test]
fn an_options_object_comparator_stays_reachable() {
    let Ok(typefacts) = env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let fixture = fixture("solid-1x-options-comparator");
    let project = fixture.join("tsconfig.json").canonicalize().unwrap();
    let project_id = project.to_string_lossy().into_owned();
    let app = fixture.join("App.ts");
    let source = fs::read_to_string(&app).unwrap();

    let typescript = TypeFactsSession::open(&typefacts, &project_id, &[]).unwrap();
    let mut session =
        NativeIncrementalSession::open(solid_v1(), project_id, vec![source_file(&app)], typescript)
            .unwrap();
    let facts = session.analyze().unwrap();
    let program = solid_reactive_ir::build(&facts, solid_v1().vocabulary).unwrap();

    // The comparator's body, located from the source text so the assertions
    // cannot drift onto the memo's own callback.
    let body = source
        .find("equals: (previous: number, next: number) => {")
        .expect("comparator marker") as u64;
    // Ends before `Widget`, whose own `rounded()` read would otherwise satisfy
    // the assertions from a completely different reachability path.
    let end = source
        .find("});\n\nexport function Widget")
        .expect("memo close marker") as u64;
    let within = |path: &str, start: u64, finish: u64| {
        path.ends_with("App.ts") && start >= body && finish <= end
    };
    assert!(
        program.reads.iter().any(|read| within(
            read.location.path.as_ref(),
            read.location.start_byte,
            read.location.end_byte
        )),
        "the comparator's read of `tolerance` must be recorded: {:#?}",
        program
            .reads
            .iter()
            .map(|read| (read.accessor.as_ref(), read.location.start_byte))
            .collect::<Vec<_>>()
    );
    assert!(
        program.writes.iter().any(|write| within(
            write.location.path.as_ref(),
            write.location.start_byte,
            write.location.end_byte
        )),
        "the comparator's write through `setTolerance` must be recorded: {:#?}",
        program
            .writes
            .iter()
            .map(|write| (write.setter.as_ref(), write.location.start_byte))
            .collect::<Vec<_>>()
    );

    // The incremental reachability pass assembles the same graph from cached
    // per-file fragments and answers the same question. The two passes each keep
    // their own copy of the argument rule; held to each other, neither can be
    // narrowed alone.
    let (retained, _) = solid_reactive_ir::IncrementalBuilder::default()
        .build(&facts, solid_v1().vocabulary)
        .unwrap();
    assert_eq!(retained, program);
}
