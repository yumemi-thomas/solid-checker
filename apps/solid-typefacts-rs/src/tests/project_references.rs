//! Ports `tsgo/project_references_test.go`, the symbol-id parts of
//! `tsgo/project_memo_test.go` and the `SymbolAt`/`ResolveAlias` parts of
//! `tsgo/project_invalidation_test.go` (deferred from S3).
//!
//! The call target is the `Target` of `Project::source_calls`, as in Go. The
//! source-fact memo assertions live in `source_facts`, except the departed-file
//! eviction, which belongs to `TestReferencesDropFileThatLeftProgram`.

use super::project_support::{SMALL_CONFIG, Scratch, change, deletion, find, find_last};
use crate::backend::ClosureBackend;
use crate::context::Context;
use crate::model::SymbolId;
use crate::project::Project;
use std::collections::HashSet;
use typefacts::Location;

const A_SOURCE: &str = "import { makeThing } from \"./b\";\n\nexport const value = makeThing();\n";

fn target_of(project: &mut Project, path: &str, _source: &str) -> SymbolId {
    let calls = project.source_calls(&Context::new(), path).unwrap();
    assert_eq!(calls.len(), 1, "expected one source call in {path}");
    calls[0].target.clone()
}

fn references_sorted(locations: &[Location]) -> bool {
    locations
        .windows(2)
        .all(|pair| (&*pair[0].path, pair[0].start_byte) <= (&*pair[1].path, pair[1].start_byte))
}

fn base(path: &str) -> &str {
    path.rsplit('/').next().unwrap()
}

#[test]
fn reference_index_across_generations() {
    let b_source = "export function makeThing(): number {\n  return 1;\n}\nexport const local = makeThing();\n";
    let scratch = Scratch::new(&[
        ("tsconfig.json", SMALL_CONFIG),
        ("a.ts", A_SOURCE),
        ("b.ts", b_source),
        ("c.ts", "export const unrelated = 1;\n"),
    ]);
    let (a, b, c) = (
        scratch.path("a.ts"),
        scratch.path("b.ts"),
        scratch.path("c.ts"),
    );
    let ctx = Context::new();
    let mut project = scratch.open();

    let target = target_of(&mut project, &a, A_SOURCE);
    assert!(target.starts_with("symbol:h:"));
    let references = project.references(&ctx, &target).unwrap();
    assert_eq!(
        references.len(),
        2,
        "references in a.ts and b.ts: {references:?}"
    );
    assert!(base(&references[0].path) == "a.ts" && base(&references[1].path) == "b.ts");
    assert!(references_sorted(&references), "{references:?}");
    let serial = |project: &Project, path: &str| {
        project
            .reference_index()
            .files()
            .get(path)
            .map(|entry| entry.serial)
    };
    let (entry_a, entry_b) = (serial(&project, &a).unwrap(), serial(&project, &b).unwrap());
    for (path, entry) in project.reference_index().files() {
        assert!(
            entry.refs.iter().all(|group| group.spans.is_empty()),
            "merged index still duplicates retained spans for {path}"
        );
    }

    // An unrelated update keeps contributions and answers.
    project
        .update(&ctx, &[change(&c, 1, "export const unrelated = 2;\n")])
        .unwrap();
    assert_eq!(
        serial(&project, &a),
        Some(entry_a),
        "a.ts contribution replaced"
    );
    assert_eq!(
        serial(&project, &b),
        Some(entry_b),
        "b.ts contribution replaced"
    );
    assert_eq!(project.references(&ctx, &target).unwrap(), references);

    // A shape-preserving declaring-file edit retains its importers.
    let edited_b = "export function makeThing(): number {\n  return 2;\n}\nexport const local = makeThing();\n";
    project.update(&ctx, &[change(&b, 1, edited_b)]).unwrap();
    assert_eq!(
        serial(&project, &a),
        Some(entry_a),
        "a.ts did not survive a shape-preserving edit"
    );
    assert!(
        serial(&project, &b).is_none(),
        "b.ts contribution survived its own edit"
    );
    let recomputed = project.references(&ctx, &target).unwrap();
    assert_eq!(
        recomputed, references,
        "the edit moves neither declaration nor references"
    );
    let refreshed = serial(&project, &b).expect("b.ts contribution is lazily replaced");
    assert_ne!(refreshed, entry_b);
    assert!(
        serial(&project, &c).is_some(),
        "the unaffected c.ts contribution is retained"
    );

    // A fresh project over the same tree answers the same durable id with
    // byte-identical locations: the on-disk b.ts is unedited, so apply the
    // same edit through an update.
    let mut fresh = scratch.open();
    fresh.update(&ctx, &[change(&b, 1, edited_b)]).unwrap();
    fresh.source_calls(&ctx, &a).unwrap();
    assert_eq!(
        fresh.references(&ctx, &target).unwrap(),
        recomputed,
        "memoized index diverged from a fresh scan"
    );

    // A multi-file update clears the contributions wholesale.
    assert!(!project.reference_index().files().is_empty());
    project
        .update(
            &ctx,
            &[
                change(&c, 2, "export const unrelated = 3;\n"),
                change(&b, 2, "export function makeThing(): number {\n  return 3;\n}\nexport const local = makeThing();\n"),
            ],
        )
        .unwrap();
    assert!(
        project.reference_index().files().is_empty(),
        "no contributions after a full rebuild"
    );
    assert_eq!(project.references(&ctx, &target).unwrap(), references);
}

/// Go builds the index struct directly; so does this.
#[test]
fn broad_invalidation_keeps_exact_symbol_evidence() {
    const FILES: usize = 65;
    let mut index = crate::references::ReferenceIndex::default();
    let mut affected = Vec::new();
    for file in 0..FILES {
        let path = format!("/project/file-{file:02}.ts");
        index.insert_for_test(&path, &format!("symbol-{file:02}"), true);
        affected.push(path);
    }
    index.invalidate(&affected, &|_| false, &|_| false);
    assert!(
        !index.is_merged(),
        "broad invalidation selects a full merged-index rebuild"
    );
    assert!(
        index.delta_exact(),
        "broad rebuild discarded exact changed-symbol evidence"
    );
    assert_eq!(index.changed_symbol_set().len(), FILES);
}

/// A durable id whose declaration disappears must fail closed even though
/// retained contributions may still mention it.
#[test]
fn references_fail_closed_when_the_declaration_disappears() {
    let scratch = Scratch::new(&[
        ("tsconfig.json", SMALL_CONFIG),
        ("a.ts", A_SOURCE),
        (
            "b.ts",
            "export function makeThing(): number {\n  return 1;\n}\n",
        ),
    ]);
    let (a, b) = (scratch.path("a.ts"), scratch.path("b.ts"));
    let ctx = Context::new();
    let mut project = scratch.open();
    let target = target_of(&mut project, &a, A_SOURCE);
    project.references(&ctx, &target).unwrap();
    project
        .update(
            &ctx,
            &[change(
                &b,
                1,
                "export function makeOther(): number {\n  return 1;\n}\n",
            )],
        )
        .unwrap();
    let error = project.references(&ctx, &target).unwrap_err();
    assert!(
        error.0.starts_with("not found"),
        "vanished declaration: {error}"
    );
}

/// A file that leaves the program on an incremental update must stop
/// contributing references even when it is not in the affected set.
#[test]
fn references_drop_a_file_that_left_the_program() {
    let main_source = "import { helper } from \"./lib\";\nimport { fromDep } from \"./dep\";\nexport const value = helper() + fromDep;\n";
    let scratch = Scratch::new(&[
        (
            "tsconfig.json",
            r#"{"compilerOptions":{"module":"esnext","moduleResolution":"bundler","target":"esnext"},"include":["main.ts"]}"#,
        ),
        (
            "lib.ts",
            "export function helper(): number {\n  return 1;\n}\n",
        ),
        (
            "dep.ts",
            "import { helper } from \"./lib\";\nexport const fromDep = helper();\n",
        ),
        ("main.ts", main_source),
    ]);
    let (main, dep) = (scratch.path("main.ts"), scratch.path("dep.ts"));
    let ctx = Context::new();
    let mut project = scratch.open();
    let calls = project.source_calls(&ctx, &main).unwrap();
    assert_eq!(calls.len(), 1, "expected one call in main.ts");
    let target = calls[0].target.clone();
    let before = project.references(&ctx, &target).unwrap();
    assert!(
        before
            .iter()
            .any(|location| base(&location.path) == "dep.ts"),
        "a dep.ts reference before the import is removed: {before:?}"
    );

    project.source_calls(&ctx, &dep).unwrap();

    // Dropping main's dep import removes dep.ts from the program without
    // putting dep.ts in the affected set.
    project
        .update(
            &ctx,
            &[change(
                &main,
                1,
                "import { helper } from \"./lib\";\nexport const value = helper();\n",
            )],
        )
        .unwrap();
    assert!(
        !project.reference_index().files().contains_key(&dep),
        "reference contribution of a departed file survived the update"
    );
    // The source-fact memo evicts departed files the same way: dep.ts must
    // answer like a file the program does not contain, not from the memo.
    let error = project.source_calls(&ctx, &dep).unwrap_err();
    assert!(
        error.0.starts_with("not found"),
        "expected not-found for source calls of a departed file, got {error}"
    );
    let after = project.references(&ctx, &target).unwrap();
    assert!(
        after
            .iter()
            .all(|location| base(&location.path) != "dep.ts"),
        "reference in departed file survived: {after:?}"
    );
    assert!(!after.is_empty(), "the main.ts reference remains");
}

/// A shorthand property `{ makeThing }` reads the value binding: it is a
/// reference of the function, in its own file and through an import.
#[test]
fn shorthand_property_is_a_reference_of_its_value() {
    let a_source = "import { makeThing } from \"./b\";\n\nexport const value = makeThing();\nexport const held = { makeThing };\n";
    let b_source = "export function makeThing(): number {\n  return 1;\n}\nexport const local = { makeThing };\n";
    let scratch = Scratch::new(&[
        ("tsconfig.json", SMALL_CONFIG),
        ("a.ts", a_source),
        ("b.ts", b_source),
    ]);
    let (a, b) = (scratch.path("a.ts"), scratch.path("b.ts"));
    let mut project = scratch.open();
    let target = target_of(&mut project, &a, a_source);
    let references = project.references(&Context::new(), &target).unwrap();
    for (path, source) in [(&a, a_source), (&b, b_source)] {
        let start = find_last(path, source, "makeThing").start_byte;
        assert!(
            references
                .iter()
                .any(|reference| *reference.path == **path && reference.start_byte == start),
            "shorthand at {path}:{start} is not a reference: {references:?}"
        );
    }
}

/// `TestSourceFactsMemoAcrossGenerations`, without the memo: held ids resolve
/// in later generations and a full rebuild re-resolves them.
#[test]
fn held_ids_resolve_across_generations() {
    let scratch = Scratch::new(&[
        ("tsconfig.json", SMALL_CONFIG),
        ("a.ts", A_SOURCE),
        (
            "b.ts",
            "export function makeThing(): number {\n  return 1;\n}\n",
        ),
        ("c.ts", "export const unrelated = 1;\n"),
    ]);
    let (a, b, c) = (
        scratch.path("a.ts"),
        scratch.path("b.ts"),
        scratch.path("c.ts"),
    );
    let ctx = Context::new();
    let mut project = scratch.open();
    let target = target_of(&mut project, &a, A_SOURCE);
    assert!(
        target.starts_with("symbol:h:"),
        "a durable symbol id, got {target}"
    );
    let declarations = project.declarations(&ctx, &target).unwrap().unwrap();
    assert!(declarations.len() == 1 && base(&declarations[0].location.path) == "b.ts");

    // An unrelated update keeps a.ts out of the affected set and resolves held ids.
    let affected = project
        .update(&ctx, &[change(&c, 1, "export const unrelated = 2;\n")])
        .unwrap()
        .affected;
    assert!(
        affected.iter().all(|path| base(path) != "a.ts"),
        "{affected:?}"
    );
    assert_eq!(
        project.declarations(&ctx, &target).unwrap().unwrap(),
        declarations
    );

    // A shape-preserving imported body edit retains the importer, and the
    // declaration's name span is unchanged, so the durable identity is too.
    let affected = project
        .update(
            &ctx,
            &[change(
                &b,
                1,
                "export function makeThing(): number {\n  return 2;\n}\n",
            )],
        )
        .unwrap()
        .affected;
    assert!(
        affected.iter().all(|path| base(path) != "a.ts"),
        "{affected:?}"
    );
    assert_eq!(
        target_of(&mut project, &a, A_SOURCE),
        target,
        "durable id changed for an unmoved declaration"
    );

    // An exported signature change puts the importer in the affected set.
    let affected = project
        .update(
            &ctx,
            &[change(
                &b,
                2,
                "export function makeThing(): string {\n  return \"changed\";\n}\n",
            )],
        )
        .unwrap()
        .affected;
    assert!(
        affected.iter().any(|path| base(path) == "a.ts"),
        "{affected:?}"
    );

    // A multi-file update is a full rebuild; the held id re-resolves after it.
    project
        .update(
            &ctx,
            &[
                change(&c, 3, "export const unrelated = 3;\n"),
                change(
                    &b,
                    3,
                    "export function makeThing(): number {\n  return 3;\n}\n",
                ),
            ],
        )
        .unwrap();
    assert!(
        project.declarations(&ctx, &target).unwrap().is_some(),
        "durable id failed to re-resolve after a full rebuild"
    );
    let _ = (deletion, HashSet::<u8>::new());
}

/// `TestSemanticAffectedSetCutoff`'s `preserveExportID` case: an export keeps
/// its id across span shifts, and an importer's alias keeps resolving to it.
#[test]
fn export_ids_survive_span_shifts() {
    let before = "const local = 1;\nexport function value(): number {\n  return 1;\n}\n";
    let after = "const local = 100000;\nexport function value(): number {\n  return 1;\n}\n";
    let importer_source =
        "import { value } from \"./dependency\";\nexport const result = value();\n";
    let scratch = Scratch::new(&[
        ("tsconfig.json", SMALL_CONFIG),
        ("dependency.ts", before),
        ("importer.ts", importer_source),
    ]);
    let (dependency, importer) = (scratch.path("dependency.ts"), scratch.path("importer.ts"));
    let ctx = Context::new();
    let mut project = scratch.open();
    let at = |project: &mut Project, path: &str, source: &str, needle: &str| {
        let location = find(path, source, needle);
        project.symbol_at(&Context::new(), &location).unwrap()
    };
    let before_id = at(&mut project, &dependency, before, "value");
    let alias_id = at(&mut project, &importer, importer_source, "value");
    assert_eq!(
        project.resolve_alias(&ctx, &alias_id).unwrap().as_deref(),
        Some(&*before_id),
        "import target before edit"
    );

    let affected = project
        .update(&ctx, &[change(&dependency, 1, after)])
        .unwrap()
        .affected;
    assert!(
        affected.iter().all(|path| *path != importer),
        "{affected:?}"
    );
    assert_eq!(
        at(&mut project, &dependency, after, "value"),
        before_id,
        "export id after span shift"
    );
    assert_eq!(
        project.resolve_alias(&ctx, &alias_id).unwrap().as_deref(),
        Some(&*before_id),
        "retained import target after span shift"
    );

    let affected = project
        .update(&ctx, &[change(&dependency, 2, before)])
        .unwrap()
        .affected;
    assert!(
        affected.iter().all(|path| *path != importer),
        "second span shift: {affected:?}"
    );
    assert_eq!(
        at(&mut project, &dependency, before, "value"),
        before_id,
        "export id after second span shift"
    );
}

/// `TestLeafUpdateSkipsDeclarationProofUntilItGainsAnImporter`'s `SymbolAt`
/// assertions: a leaf's changed and added exports resolve after the cutoff.
#[test]
fn leaf_exports_resolve_after_the_no_importer_cutoff() {
    let scratch = Scratch::new(&[
        ("tsconfig.json", SMALL_CONFIG),
        ("leaf.ts", "export const value = 1;\n"),
        ("consumer.ts", "export const local = true;\n"),
    ]);
    let leaf = scratch.path("leaf.ts");
    let mut project = scratch.open();
    let ctx = Context::new();
    let source = "export const value = \"changed\";\nexport const added = 1;\n";
    project.update(&ctx, &[change(&leaf, 1, source)]).unwrap();
    assert!(project.last_update.leaf_cutoff);
    for name in ["value", "added"] {
        project
            .symbol_at(&ctx, &find(&leaf, source, name))
            .unwrap_or_else(|error| panic!("resolve leaf export {name}: {error}"));
    }
}
