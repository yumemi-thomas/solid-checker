//! The lifecycle contract of `unsafe_boundary::Generation`: lazy checker,
//! checker disposal and recreation, lease refusal, abandoned candidates,
//! committed candidates, and drop on a foreign thread.

use super::project_support::{SMALL_CONFIG, Scratch};
use crate::overlay::OverlayFs;
use crate::project::compiler_host;
use crate::unsafe_boundary::UpdateKind;
use std::sync::Arc;
use tsrs_compiler::Context;
use tsrs_vfs::{FS, bundled, osvfs};

fn overlay() -> Arc<OverlayFs> {
    let base: Arc<dyn FS> = Arc::new(bundled::wrap_fs(osvfs::fs()));
    Arc::new(OverlayFs::new(base))
}

fn export_names(generation: &crate::unsafe_boundary::Generation, path: &str) -> Vec<String> {
    crate::shape::declaration_shape_for(generation, path)
        .expect("shape")
        .exports
        .into_iter()
        .map(|export| export.name)
        .collect()
}

#[test]
fn the_checker_is_lazy_disposable_and_recreated_on_demand() {
    let scratch = Scratch::new(&[
        ("tsconfig.json", SMALL_CONFIG),
        ("m.ts", "export const one = 1;\nexport const two = 2;\n"),
    ]);
    let m = scratch.path("m.ts");
    let mut project = scratch.open();
    let generation = project.current().unwrap();
    assert!(!generation.has_checker(), "opening needs no checker");
    assert_eq!(export_names(generation, &m), ["one", "two"]);
    assert!(generation.has_checker());
    // `with_checker` did not leave a lease behind.
    project.release_analysis_state_for_test();
    assert!(!project.current().unwrap().has_checker());
    assert_eq!(export_names(project.current().unwrap(), &m), ["one", "two"]);
}

#[test]
fn a_checker_with_an_outstanding_lease_is_not_disposed() {
    let scratch = Scratch::new(&[
        ("tsconfig.json", SMALL_CONFIG),
        ("m.ts", "export const one = 1;\n"),
    ]);
    let mut project = scratch.open();
    let generation = project.current_mut_for_test();
    let handle = generation.program().get_type_checker(&Context::default());
    assert!(
        !generation.release_checker(),
        "a live lease blocks disposal"
    );
    assert!(generation.has_checker());
    drop(handle);
    assert!(generation.release_checker());
    assert!(!generation.has_checker());
}

#[test]
fn an_abandoned_candidate_frees_only_itself() {
    let scratch = Scratch::new(&[
        ("tsconfig.json", SMALL_CONFIG),
        ("a.ts", "export const a = 1;\n"),
        ("b.ts", "export const b = 2;\n"),
    ]);
    let (a, b) = (scratch.path("a.ts"), scratch.path("b.ts"));
    let project = scratch.open();
    let generation = project.current().unwrap();
    let edited = overlay();
    edited.set(&a, "export const a = 10;\nexport const extra = 1;\n");
    let host = || compiler_host(&scratch.root.to_string_lossy(), &edited);

    let reused = generation.prepare_update(&a, host());
    assert_eq!(reused.kind(), UpdateKind::Reused);
    assert_eq!(export_names(&reused, &a), ["a", "extra"]);
    drop(reused);

    // A change of the import list cannot be absorbed in place: the candidate
    // is a whole fresh chain, and abandoning it frees that chain only.
    edited.set(&a, "import { b } from \"./b\";\nexport const a = b;\n");
    let rebuilt = generation.prepare_update(&a, host());
    assert_eq!(rebuilt.kind(), UpdateKind::Rebuilt);
    assert_eq!(export_names(&rebuilt, &a), ["a"]);
    drop(rebuilt);

    // The accepted generation still answers from the original text.
    assert_eq!(export_names(project.current().unwrap(), &a), ["a"]);
    assert_eq!(export_names(project.current().unwrap(), &b), ["b"]);
    let text = project
        .current()
        .unwrap()
        .program()
        .get_source_file(&a)
        .unwrap()
        .text();
    assert_eq!(text, "export const a = 1;\n");
}

#[test]
fn committing_replaces_the_head_and_keeps_the_chain_alive() {
    let scratch = Scratch::new(&[
        ("tsconfig.json", SMALL_CONFIG),
        ("a.ts", "export const a = 1;\n"),
        ("b.ts", "export const b = 2;\n"),
    ]);
    let (a, b) = (scratch.path("a.ts"), scratch.path("b.ts"));
    let mut project = scratch.open();
    let edited = overlay();
    let host = || compiler_host(&scratch.root.to_string_lossy(), &edited);
    for round in 0..5 {
        edited.set(
            &a,
            &format!("export const a = {round};\nexport const r{round} = 1;\n"),
        );
        let generation = project.current_mut_for_test();
        let candidate = generation.prepare_update(&a, host());
        assert_eq!(candidate.kind(), UpdateKind::Reused);
        generation.commit(candidate);
        assert_eq!(generation.updates_since_build(), round + 1);
        assert_eq!(
            export_names(generation, &a),
            ["a".to_owned(), format!("r{round}")]
        );
        // The untouched file shares the chain's processed-file data.
        assert_eq!(export_names(generation, &b), ["b"]);
    }
}

#[test]
fn dropping_a_generation_on_a_foreign_thread_leaks_instead_of_freeing() {
    let scratch = Scratch::new(&[
        ("tsconfig.json", SMALL_CONFIG),
        ("a.ts", "export const a = 1;\n"),
    ]);
    let (config, host) = {
        let overlay = overlay();
        let directory = scratch.root.to_string_lossy().into_owned();
        let project = scratch.open();
        let config = project.current().unwrap().command_line();
        (config, compiler_host(&directory, &overlay))
    };
    // tsrs arenas are thread-local: a generation built here must not be freed
    // from another thread. It is leaked, and nothing crashes.
    let generation = crate::unsafe_boundary::Generation::build(config, host);
    let outcome = std::thread::spawn(move || drop(generation)).join();
    assert!(outcome.is_ok());
}
