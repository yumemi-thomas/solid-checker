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
    // Go's `OpenProject` creates the checker to seed the exported identities.
    assert!(generation.has_checker(), "opening seeds export identities");
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
fn a_replaced_file_version_is_freed_and_an_abandoned_one_is_too() {
    let scratch = Scratch::new(&[
        ("tsconfig.json", SMALL_CONFIG),
        ("a.ts", "export const a = 1;\n"),
        ("b.ts", "export const b = 2;\n"),
    ]);
    let a = scratch.path("a.ts");
    let mut project = scratch.open();
    let edited = overlay();
    let host = || compiler_host(&scratch.root.to_string_lossy(), &edited);
    let generation = project.current_mut_for_test();
    let base = generation.file_region_count();
    assert!(base >= 3, "a, b and the default library files: {base}");
    for round in 0..6 {
        edited.set(&a, &format!("export const a = {round};\n"));
        let candidate = generation.prepare_update(&a, host());
        assert_eq!(candidate.kind(), UpdateKind::Reused);
        // The candidate's new version is held beside the base one and, from
        // the second round, the accepted head's.
        assert_eq!(
            generation.file_region_count(),
            base + 1 + usize::from(round > 0),
            "round {round}"
        );
        generation.commit(candidate);
        // The base file stays (the chain's shared data refers to it) and the
        // head's version is new; the version the previous head read is gone.
        assert_eq!(generation.file_region_count(), base + 1, "round {round}");
    }
    // An abandoned candidate's version goes at the next sweep.
    edited.set(&a, "export const a = 99;\n");
    drop(generation.prepare_update(&a, host()));
    generation.sweep_dead_files();
    assert_eq!(generation.file_region_count(), base + 1);
    assert_eq!(export_names(generation, &a), ["a"]);
}

#[test]
fn dropping_a_generation_on_a_foreign_thread_leaks_instead_of_freeing() {
    let scratch = Scratch::new(&[
        ("tsconfig.json", SMALL_CONFIG),
        ("a.ts", "export const a = 1;\n"),
    ]);
    let overlay = overlay();
    let directory = scratch.root.to_string_lossy().into_owned();
    let project = scratch.open();
    // The project keeps the configuration's region alive past the drop below.
    let generation = project
        .current()
        .unwrap()
        .rebuilt(compiler_host(&directory, &overlay));
    // tsrs arenas are thread-local: a generation built here must not be freed
    // from another thread. It is leaked, and nothing crashes.
    let outcome = std::thread::spawn(move || drop(generation)).join();
    assert!(outcome.is_ok());
}

fn scratch_with_a_call() -> Scratch {
    Scratch::new(&[
        ("tsconfig.json", SMALL_CONFIG),
        (
            "a.ts",
            "declare function f(a: number): void;\nf(\"x\");\nexport const one = 1;\n",
        ),
    ])
}

/// Module invariant 7: a tsrs entry point that acquires the checker while a
/// lease is held would alias `&mut Checker`; the pool refuses.
#[test]
#[should_panic(expected = "unsafe_boundary invariant 7")]
fn a_program_call_that_acquires_the_checker_under_a_lease_is_refused() {
    let scratch = scratch_with_a_call();
    let a = scratch.path("a.ts");
    let project = scratch.open();
    let generation = project.current().unwrap();
    let program = generation.program();
    let file = program.get_source_file(&a).unwrap();
    generation.with_checker(|_checker| {
        let _ = program.get_semantic_diagnostics(&Context::default(), Some(file));
    });
}

#[test]
#[should_panic(expected = "unsafe_boundary invariant 7")]
fn a_second_lease_is_refused() {
    let scratch = scratch_with_a_call();
    let project = scratch.open();
    let generation = project.current().unwrap();
    generation.with_checker(|_outer| generation.with_checker(|_inner| ()));
}

/// The sanctioned way: the lease is released around the call and reacquired,
/// and handles taken before the suspension stay valid after it.
#[test]
fn a_suspended_lease_reports_diagnostics_and_keeps_earlier_handles_valid() {
    let scratch = scratch_with_a_call();
    let a = scratch.path("a.ts");
    let project = scratch.open();
    let generation = project.current().unwrap();
    let program = generation.program();
    let file = program.get_source_file(&a).unwrap();
    let (before, after, diagnostics) = generation.with_checker(|checker| {
        let at = u32::try_from(file.text().find("\"x\"").unwrap()).unwrap();
        let call = crate::nodes::node_at(file, at);
        let before = checker.get_type_at_location(call);
        let before_flags = before.flags();
        let diagnostics = checker.semantic_diagnostics(file);
        // The same type object, read through the reacquired lease.
        let again = checker.get_type_at_location(call);
        (
            before_flags,
            (again.flags(), again.addr() == before.addr()),
            diagnostics,
        )
    });
    assert_eq!(before, after.0);
    assert!(after.1, "the checker is the same one across the suspension");
    assert!(
        diagnostics.iter().any(|d| d.code == 2345),
        "{diagnostics:?}"
    );
}
