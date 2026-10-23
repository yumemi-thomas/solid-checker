//! Opening a project: the exact Go error texts, the `-project-config`
//! overlay, `sources` and source digests, and the closed-project behavior.

use super::project_support::{SMALL_CONFIG, Scratch};
use crate::backend::ClosureBackend;
use crate::context::Context;
use crate::project::{ERR_CLOSED, Project};
use sha2::{Digest, Sha256};

fn open_error(config: &str, source: Option<&str>) -> String {
    match Project::open(&Context::new(), config, source, None) {
        Ok(_) => panic!("opening {config} unexpectedly succeeded"),
        Err(error) => error,
    }
}

#[test]
fn a_missing_tsconfig_reports_ts5083_with_the_go_text() {
    let scratch = Scratch::new(&[]);
    let missing = scratch.path("tsconfig.json");
    assert_eq!(
        open_error(&missing, None),
        format!("parse tsconfig: TS5083: Cannot read file '{missing}'.")
    );
}

#[test]
fn a_relative_config_path_is_resolved_against_the_working_directory() {
    let error = open_error("definitely-missing/tsconfig.json", None);
    let expected = std::env::current_dir()
        .unwrap()
        .join("definitely-missing/tsconfig.json");
    assert_eq!(
        error,
        format!(
            "parse tsconfig: TS5083: Cannot read file '{}'.",
            expected.display()
        )
    );
}

#[test]
fn configuration_errors_are_counted_not_listed() {
    // No input files: TS18003 is a configuration error, not a read failure.
    let scratch = Scratch::new(&[(
        "tsconfig.json",
        r#"{"compilerOptions":{"module":"esnext"},"include":["nothing-here/*.ts"]}"#,
    )]);
    assert_eq!(
        open_error(&scratch.config(), None),
        "parse tsconfig: 1 configuration error(s)"
    );
}

#[test]
fn project_config_bytes_stand_in_for_the_file_at_its_logical_path() {
    let scratch = Scratch::new(&[
        ("a.ts", "export const a = 1;\n"),
        ("skipped.ts", "export const s = 1;\n"),
    ]);
    assert!(!std::path::Path::new(&scratch.config()).exists());
    let mut project = Project::open(
        &Context::new(),
        &scratch.config(),
        Some(r#"{"compilerOptions":{"module":"esnext"},"files":["a.ts"]}"#),
        None,
    )
    .unwrap();
    let files = project.source_files(&Context::new()).unwrap();
    assert_eq!(
        files
            .iter()
            .map(|file| file.path.as_str())
            .collect::<Vec<_>>(),
        [scratch.path("a.ts")]
    );
    assert!(
        !std::path::Path::new(&scratch.config()).exists(),
        "the configuration overlay is never written"
    );
    // A rebuild reads the same bytes.
    project
        .update(
            &Context::new(),
            &[super::project_support::deletion(
                &scratch.path("skipped.ts"),
                1,
            )],
        )
        .unwrap();
    assert_eq!(project.source_files(&Context::new()).unwrap().len(), 1);
}

#[test]
fn sources_exclude_declaration_files_and_digests_hash_their_bytes() {
    let scratch = Scratch::new(&[
        (
            "tsconfig.json",
            r#"{"compilerOptions":{"module":"esnext"},"include":["*.ts"]}"#,
        ),
        ("b.ts", "export const b = 2;\n"),
        ("a.ts", "export const a = 1; // é\n"),
        ("types.d.ts", "export declare const t: number;\n"),
    ]);
    let mut project = scratch.open();
    let ctx = Context::new();
    let files = project.source_files(&ctx).unwrap();
    assert_eq!(
        files
            .iter()
            .map(|file| file.path.as_str())
            .collect::<Vec<_>>(),
        [scratch.path("a.ts"), scratch.path("b.ts")],
        "path-ordered, no declaration files"
    );
    assert!(files.iter().all(|file| !file.local));
    assert_eq!(files[0].source, "export const a = 1; // é\n".as_bytes());
    crate::backend::validate_canonical_source_files(&files).unwrap();

    let digests = project.source_digests(&ctx).unwrap();
    assert_eq!(digests.len(), 2);
    for (file, digest) in files.iter().zip(&digests) {
        assert_eq!(&*digest.path, file.path);
        assert_eq!(
            digest.sha256.to_string(),
            format!("sha256:{:x}", Sha256::digest(&file.source))
        );
    }
}

#[test]
fn a_closed_project_refuses_every_operation_and_a_second_close() {
    let scratch = Scratch::new(&[
        ("tsconfig.json", SMALL_CONFIG),
        ("a.ts", "export const a = 1;\n"),
    ]);
    let mut project = scratch.open();
    project.close().unwrap();
    let ctx = Context::new();
    assert_eq!(project.source_files(&ctx).unwrap_err().0, ERR_CLOSED);
    assert_eq!(project.source_digests(&ctx).unwrap_err().0, ERR_CLOSED);
    assert_eq!(project.update(&ctx, &[]).unwrap_err().0, ERR_CLOSED);
    assert_eq!(project.close().unwrap_err().0, ERR_CLOSED);
}

#[test]
fn the_fact_families_answer_or_fail_never_empty() {
    use crate::model::SemanticScope;
    let scratch = Scratch::new(&[
        ("tsconfig.json", SMALL_CONFIG),
        ("a.ts", "export const a = 1;\n"),
    ]);
    let mut project = scratch.open();
    let ctx = Context::new();
    let name = |error: crate::backend::BackendError| error.0;
    let suppression = Default::default();
    let scope = SemanticScope {
        suppression: &suppression,
    };
    assert!(project.async_functions_at(&ctx, &[]).unwrap().is_empty());
    assert!(
        name(
            project
                .async_functions_at(&ctx, &[typefacts::Location::default()])
                .unwrap_err()
        )
        .starts_with("not found: source file")
    );
    // The symbol families answer: an unknown id is "not found", which is
    // `None` or an id-only row, never an error and never a made-up fact.
    assert_eq!(project.resolve_alias(&ctx, "symbol:h:x").unwrap(), None);
    assert_eq!(project.declarations(&ctx, "symbol:h:x").unwrap(), None);
    assert!(project.references_batch(&ctx, &[]).unwrap().is_empty());
    assert_eq!(
        project.changed_references(&ctx).unwrap(),
        (Vec::new(), false)
    );
    assert!(
        project
            .semantic_demand_runs(&ctx, &[], &scope)
            .unwrap()
            .is_empty()
    );
    let query = typefacts::v3::SymbolQuery {
        id: "symbol:h:x".into(),
        references: true,
        references_only: false,
    };
    let facts =
        crate::backend::SymbolEvidence::symbol_evidence(&mut project, &ctx, &[query]).unwrap();
    assert_eq!(facts.len(), 1);
    assert_eq!(&*facts[0].id, "symbol:h:x");
    assert!(facts[0].alias_target.is_empty() && facts[0].declarations.is_empty());
}
