//! Granularity of the cross-file returned-callback proof digest.
//!
//! Solid 1.x routes some callbacks through a function the primitive returns, so
//! answers cached for the factory's file depend on facts in every other file.
//! The digest that guards those fragments has to change whenever such a fact
//! moves -- and must not change when nothing a proof can read has moved.
//! `cross_file_callbacks_process.rs` holds the first half; this holds the
//! second.

use std::{env, fs, path::PathBuf};

use solid_facts::compiler::CompilerOptions;
use solid_facts_backend::{
    NativeIncrementalSession, SourceChange, SourceFile, TypeFactsSession, dialect,
};

/// The 1.x dialect, chosen explicitly: these fixtures carry no `node_modules`
/// for detection to read.
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

/// Editing a module no proof can read keeps every other file's fragments.
///
/// `shapes.ts` declares an interface and a type alias: no calls, no JSX, no
/// member expressions, no bindings. The cross-file machinery's every loop over
/// a file other than the one it was asked about iterates one of those four
/// tables, so `shapes.ts` cannot contribute an invocation site, an alias, or a
/// factory seed. A digest over every file's source hash nevertheless changed
/// when it was touched, discarding the reachability, owner and local-access
/// fragments of `adapter.ts` and `consumer.ts` along with it.
#[test]
fn editing_a_module_no_proof_can_read_keeps_the_other_fragments() {
    let Ok(typefacts) = env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let fixture = fixture("solid-1x-adapter-with-declarations");
    let project = fixture.join("tsconfig.json").canonicalize().unwrap();
    let project_id = project.to_string_lossy().into_owned();
    let shapes = fixture.join("shapes.ts");
    let sources = vec![
        source_file(&fixture.join("adapter.ts")),
        source_file(&fixture.join("consumer.ts")),
        source_file(&shapes),
    ];

    let typescript = TypeFactsSession::open(&typefacts, &project_id, &[]).unwrap();
    let mut session =
        NativeIncrementalSession::open(solid_v1(), project_id, sources, typescript).unwrap();
    let first = session.analyze().unwrap();
    let mut incremental = solid_reactive_ir::IncrementalBuilder::default();
    incremental.build(&first, solid_v1().vocabulary).unwrap();

    // Add a declaration to `shapes.ts`: its own facts change, and no other
    // file's do.
    let original = fs::read_to_string(&shapes).unwrap();
    let edited_source = format!("{original}\nexport type Count = number;\n");
    let edited = session
        .edit(
            vec![SourceChange {
                path: shapes
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
    let (retained, timings) = incremental.build(&edited, solid_v1().vocabulary).unwrap();

    // Correct first: a finer digest that dropped a real dependency would show
    // up here.
    assert_eq!(
        retained, fresh,
        "the retained program must still match the from-scratch one"
    );

    // Then finer: only the edited file is recomputed, in each of the three
    // caches whose fragments carry the digest.
    let files = u64::try_from(edited.files.len()).unwrap();
    assert_eq!(files, 3);
    for (stage, reused, recomputed) in [
        (
            "reachability",
            timings.reachability_reused_files,
            timings.reachability_recomputed_files,
        ),
        (
            "local access",
            timings.local_access_reused_files,
            timings.local_access_recomputed_files,
        ),
        (
            "owner",
            timings.owner_reused_files,
            timings.owner_recomputed_files,
        ),
    ] {
        assert_eq!(
            reused + recomputed,
            files,
            "{stage} must account for every file"
        );
        assert_eq!(
            (stage, reused),
            (stage, files - 1),
            "{stage} must reuse every file but the edited one"
        );
    }
}
