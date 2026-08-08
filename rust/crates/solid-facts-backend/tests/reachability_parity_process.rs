//! From-scratch and incremental reachability, held to each other.
//!
//! `solid_reactive_ir::build` and `IncrementalBuilder::build` reach the same
//! reachability answer by different routes: the fresh pass walks whole-project
//! facts once, the incremental pass assembles per-file fragments. Every edge
//! rule therefore exists twice, and a rule added to one route only is a
//! divergence that shows up as two different programs for the same facts.

use std::{env, fs, path::PathBuf};

use solid_facts::compiler::CompilerOptions;
use solid_facts_backend::{
    NativeIncrementalSession, SourceChange, SourceFile, TypeFactsSession, dialect,
};

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

/// A function named by a primitive's options object is reachable, and both
/// passes agree that it is.
///
/// `createEffect(compute, { effect: applyValue, error: reportError })` invokes
/// both named functions. The AST records them as the argument's
/// `identifier_properties`, and reachability has to follow those to keep the
/// writes inside them in the IR. The rule lived in the incremental fragment
/// pass only, so the fresh pass dropped both functions -- the same facts
/// produced two different programs depending on which builder ran.
#[test]
fn an_options_object_named_callback_is_reachable_in_both_passes() {
    let Ok(typefacts) = env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let fixture = fixture("solid-effect-options-callback");
    let project = fixture.join("tsconfig.json").canonicalize().unwrap();
    let project_id = project.to_string_lossy().into_owned();
    let app = fixture.join("App.ts");
    let source = fs::read_to_string(&app).unwrap();

    let typescript = TypeFactsSession::open(&typefacts, &project_id, &[]).unwrap();
    let mut session = NativeIncrementalSession::open(
        dialect::default_dialect(),
        project_id,
        vec![source_file(&app)],
        typescript,
    )
    .unwrap();
    let facts = session.analyze().unwrap();
    let fresh = solid_reactive_ir::build(&facts, dialect::default_dialect().vocabulary).unwrap();
    let (retained, _) = solid_reactive_ir::IncrementalBuilder::default()
        .build(&facts, dialect::default_dialect().vocabulary)
        .unwrap();

    // The premise: the options-object functions really are live code, so a pass
    // that drops them loses IR rather than merely labelling it differently.
    let body = u64::try_from(source.find("function applyValue").expect("apply marker")).unwrap();
    // `rfind`: the header comment names the primitive call too.
    let end = u64::try_from(source.rfind("createEffect(").expect("effect marker")).unwrap();
    let inside = |path: &str, start: u64, finish: u64| {
        path.ends_with("App.ts") && start >= body && finish <= end
    };
    assert!(
        fresh.writes.iter().any(|write| inside(
            write.location.path.as_ref(),
            write.location.start_byte,
            write.location.end_byte
        )),
        "the options-object callbacks' writes must be recorded: {:#?}",
        fresh
            .writes
            .iter()
            .map(|write| (write.setter.as_ref(), write.location.start_byte))
            .collect::<Vec<_>>()
    );

    assert_eq!(
        retained, fresh,
        "both reachability passes must follow options-object callbacks"
    );
}

/// The same agreement after an edit, when the incremental pass answers from
/// cached fragments instead of computing every file afresh.
#[test]
fn options_object_reachability_survives_an_incremental_edit() {
    let Ok(typefacts) = env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let fixture = fixture("solid-effect-options-callback");
    let project = fixture.join("tsconfig.json").canonicalize().unwrap();
    let project_id = project.to_string_lossy().into_owned();
    let app = fixture.join("App.ts");
    let source = fs::read_to_string(&app).unwrap();

    let typescript = TypeFactsSession::open(&typefacts, &project_id, &[]).unwrap();
    let mut session = NativeIncrementalSession::open(
        dialect::default_dialect(),
        project_id,
        vec![source_file(&app)],
        typescript,
    )
    .unwrap();
    let first = session.analyze().unwrap();
    let mut incremental = solid_reactive_ir::IncrementalBuilder::default();
    incremental
        .build(&first, dialect::default_dialect().vocabulary)
        .unwrap();

    // Drop `error` from the options object: `reportError` loses its only edge
    // while `applyValue` keeps its own, so the edit moves the answer.
    let edited_source = source.replace(", error: reportError }", " }");
    assert_ne!(edited_source, source, "options-object error marker");
    let edited = session
        .edit(
            vec![SourceChange {
                path: app.canonicalize().unwrap().to_string_lossy().into_owned(),
                version: 1,
                source: Some(edited_source),
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
}

/// A callback position naming a *different* binding of the same spelling does
/// not classify a module-scoped function as that callback.
///
/// `Host` destructures its own `apply` parameter and passes it as the effect's
/// apply callback. The module-scoped `function apply` shares only the spelling.
/// Matching by source text admitted it and reported its read of `count` as
/// running in the effect-apply phase; the read is plain untracked module code.
#[test]
fn a_same_spelled_binding_does_not_classify_an_unrelated_function() {
    let Ok(typefacts) = env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let fixture = fixture("solid-shadowed-named-callback");
    let project = fixture.join("tsconfig.json").canonicalize().unwrap();
    let project_id = project.to_string_lossy().into_owned();
    let app = fixture.join("App.ts");
    let source = fs::read_to_string(&app).unwrap();

    let typescript = TypeFactsSession::open(&typefacts, &project_id, &[]).unwrap();
    let mut session = NativeIncrementalSession::open(
        dialect::default_dialect(),
        project_id,
        vec![source_file(&app)],
        typescript,
    )
    .unwrap();
    let facts = session.analyze().unwrap();
    let program = solid_reactive_ir::build(&facts, dialect::default_dialect().vocabulary).unwrap();

    // The whole read set, so the assertion cannot pass by there being no reads
    // at all: the effect's own tracked read of `count` has to survive, and the
    // module-scoped function's read must not be admitted alongside it.
    let module_body =
        u64::try_from(source.find("function apply(value").expect("apply marker")).unwrap();
    let module_end = u64::try_from(source.find("\napply(1);").expect("call marker")).unwrap();
    let effect_read =
        u64::try_from(source.rfind("() => count()").expect("compute marker")).unwrap();
    let reads = program
        .reads
        .iter()
        .map(|read| {
            let start = read.location.start_byte;
            let region = if start >= module_body && read.location.end_byte <= module_end {
                "module-scoped apply"
            } else if start >= effect_read {
                "effect compute"
            } else {
                "elsewhere"
            };
            (read.accessor.as_ref(), region, read.execution)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        reads,
        [(
            "count",
            "effect compute",
            solid_reactive_ir::ExecutionRole::TrackedJsx
        )],
        "the module-scoped function is not the effect's apply callback"
    );
}
