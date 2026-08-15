//! Caller-proven props classification across incremental generations.
//!
//! A prop's reactivity is a fact about the component's *call sites*
//! (probed on `solid-js@2.0.0-rc.0`: `devComponent`'s strict-read window
//! only warns when a prop getter reads reactive state), so an edit at a call
//! site must move the findings inside the component — including through the
//! retained incremental caches, whose prop-source fingerprints carry the
//! classification for exactly this reason.

use std::{env, fs, path::PathBuf};

use solid_facts::compiler::CompilerOptions;
use solid_facts_backend::{
    NativeIncrementalSession, SourceChange, SourceFile, TypeFactsSession, dialect,
};

fn solid_v2() -> &'static dialect::Dialect {
    dialect::by_id("solid-v2").expect("the 2.0 dialect is registered")
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

/// A static caller keeps the component silent; editing that caller to pass a
/// signal read makes the same body read a proven strict-read violation, and
/// the retained incremental program must agree with a from-scratch build on
/// both sides of the edit.
#[test]
fn caller_edit_flips_props_classification_through_the_incremental_caches() {
    let Ok(typefacts) = env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let fixture = fixture("props-caller-invalidation");
    let project = fixture.join("tsconfig.json").canonicalize().unwrap();
    let project_id = project.to_string_lossy().into_owned();
    let app = fixture.join("App.tsx");
    let sources = vec![source_file(&app)];

    let typescript = TypeFactsSession::open(&typefacts, &project_id, &[]).unwrap();
    let mut session =
        NativeIncrementalSession::open(solid_v2(), project_id, sources, typescript).unwrap();
    let first = session.analyze().unwrap();
    let mut incremental = solid_reactive_ir::IncrementalBuilder::default();
    let (retained_first, _) = incremental.build(&first, solid_v2().vocabulary).unwrap();
    let strict_reads = |findings: &[solid_reactive_ir::Finding]| {
        findings
            .iter()
            .filter(|finding| finding.id == "SC1001")
            .map(|finding| finding.kind.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        strict_reads(&solid_v2().solve(&retained_first)),
        Vec::<String>::new(),
        "a static-only caller proves the prop is a plain property; the body read is silent"
    );

    // The caller now passes a signal read: the prop becomes proven
    // signal-backed and the identical body read is a violation.
    let original = fs::read_to_string(&app).unwrap();
    let edited_source = original.replace("title=\"static\"", "title={label()}");
    assert_ne!(original, edited_source, "the edit must change the caller");
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

    let fresh = solid_reactive_ir::build(&edited, solid_v2().vocabulary).unwrap();
    let (retained, _) = incremental.build(&edited, solid_v2().vocabulary).unwrap();
    assert_eq!(
        retained, fresh,
        "the retained program must match the from-scratch one after the caller edit"
    );
    assert_eq!(
        strict_reads(&solid_v2().solve(&retained)),
        vec!["violation".to_owned()],
        "a reactive caller proves the prop is signal-backed; the body read is a violation"
    );
}
