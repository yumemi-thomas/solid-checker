//! From-scratch and incremental owner analysis, held to each other.
//!
//! `solid_reactive_ir::build` and `IncrementalBuilder::build` construct the
//! owner graph twice — the fresh pass over whole-project facts, the
//! incremental pass from per-file fragments — so every node-construction rule
//! exists in both. The seam this file guards is the binding-aware function
//! name: an arrow carries no name of its own, so `const helper = () => ...`
//! is nameless unless the builder consults its binding. One pass applying
//! that fallback to the call-edge symbol and the other applying it to the
//! context-seeding name made the same facts produce two different programs.

use std::{env, fs, path::PathBuf};

use solid_facts::compiler::CompilerOptions;
use solid_facts_backend::{NativeIncrementalSession, SourceFile, TypeFactsSession, dialect};

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

/// Arrow-bound functions get the same owner analysis from both passes, and
/// the same analysis their `function` spellings would get.
///
/// The fixture holds the three shapes that told the passes apart: a
/// module-invoked arrow helper (its effects run unowned and must be
/// reported), an arrow with an exact Solid `Component` type (owned, never
/// reported), and an exported lowercase arrow hook (reported as uncertain —
/// analyzed callers are unknown, exactly as its `function` spelling is
/// treated).
#[test]
fn arrow_bound_functions_get_the_same_owner_analysis_in_both_passes() {
    let Ok(typefacts) = env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let fixture = fixture("owner-arrow-parity");
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

    assert_eq!(
        fresh.missing_owners, retained.missing_owners,
        "fresh and incremental owner analysis diverged"
    );
    assert_eq!(retained, fresh, "programs diverged beyond owners");

    // The parity above must not be vacuous: the module-invoked helper's
    // effect and cleanup are reported, the exported hook's are reported as
    // uncertain, and the component's are not reported at all.
    let requirement = |marker: &str| {
        let start = u64::try_from(source.find(marker).expect(marker)).unwrap();
        fresh
            .missing_owners
            .iter()
            .find(|requirement| requirement.location.start_byte == start)
            .expect(marker)
    };
    let orphan_effect = requirement("createEffect(() => 1");
    assert!(orphan_effect.report && !orphan_effect.uncertain);
    let widget_effect = requirement("createEffect(() => 2");
    assert!(!widget_effect.report);
    let hook_effect = requirement("createEffect(() => 3");
    assert!(hook_effect.report && hook_effect.uncertain);
}
