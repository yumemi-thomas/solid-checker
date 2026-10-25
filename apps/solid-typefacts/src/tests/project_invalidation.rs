//! Ports the compiler-only parts of `tsgo/project_invalidation_test.go` and
//! `tsgo/project_memo_test.go`: the affected set, the leaf and semantic
//! cutoffs, and which updates take the full-rebuild path. The assertions that
//! need symbol ids (`SymbolAt`, `ResolveAlias`) or the source-fact memo belong
//! to S4; the export-identity assertions are made on the declaration shape
//! the cutoff itself compares, which is what keeps ids stable.

use super::project_support::{SMALL_CONFIG, Scratch, change, deletion};
use crate::backend::ClosureBackend;
use crate::context::Context;
use crate::shape::declaration_shape_for;
use crate::unsafe_boundary::UpdateKind;

fn update(
    project: &mut crate::project::Project,
    changes: &[typefacts::v3::FileChange],
) -> Vec<String> {
    project.update(&Context::new(), changes).unwrap().affected
}

#[test]
fn leaf_update_skips_declaration_proof_until_it_gains_an_importer() {
    let scratch = Scratch::new(&[
        ("tsconfig.json", SMALL_CONFIG),
        ("leaf.ts", "export const value = 1;\n"),
        ("consumer.ts", "export const local = true;\n"),
    ]);
    let (leaf, consumer) = (scratch.path("leaf.ts"), scratch.path("consumer.ts"));
    let mut project = scratch.open();

    let affected = update(
        &mut project,
        &[change(
            &leaf,
            1,
            "export const value = \"changed\";\nexport const added = 1;\n",
        )],
    );
    assert_eq!(affected, std::slice::from_ref(&leaf), "only the leaf");
    assert!(
        project.last_update.leaf_cutoff,
        "the leaf update did not take the no-importer cutoff"
    );
    assert_eq!(project.last_update.kind, Some(UpdateKind::Reused));

    update(
        &mut project,
        &[change(
            &consumer,
            1,
            "import { value } from \"./leaf\";\nexport const local = value;\n",
        )],
    );
    let affected = update(
        &mut project,
        &[change(&leaf, 2, "export const value = 2;\n")],
    );
    assert!(
        !project.last_update.leaf_cutoff,
        "an imported leaf must not take the no-importer cutoff"
    );
    assert!(
        affected.contains(&consumer),
        "shape-changing leaf update did not affect its importer: {affected:?}"
    );
}

#[test]
fn semantic_affected_set_cutoff() {
    struct Case {
        name: &'static str,
        before: &'static str,
        after: &'static str,
        importer_affected: bool,
        preserve_export_id: bool,
    }
    let cases = [
        Case {
            name: "annotated body edit stops at the edited file",
            before: "export function value(): number {\n  return 1;\n}\n",
            after: "export function value(): number {\n  return 2;\n}\n",
            importer_affected: false,
            preserve_export_id: false,
        },
        Case {
            name: "inferred exported return change propagates",
            before: "export function value() {\n  return 1;\n}\n",
            after: "export function value() {\n  return \"changed\";\n}\n",
            importer_affected: true,
            preserve_export_id: false,
        },
        Case {
            name: "span shift above an unchanged export preserves identity",
            before: "const local = 1;\nexport function value(): number {\n  return 1;\n}\n",
            after: "const local = 100000;\nexport function value(): number {\n  return 1;\n}\n",
            importer_affected: false,
            preserve_export_id: true,
        },
    ];
    for case in cases {
        let scratch = Scratch::new(&[
            ("tsconfig.json", SMALL_CONFIG),
            ("dependency.ts", case.before),
            (
                "importer.ts",
                "import { value } from \"./dependency\";\nexport const result = value();\n",
            ),
        ]);
        let (dependency, importer) = (scratch.path("dependency.ts"), scratch.path("importer.ts"));
        let mut project = scratch.open();
        let before_ids: Vec<String> =
            declaration_shape_for(project.current().unwrap(), &dependency)
                .expect("shape before")
                .exports
                .iter()
                .map(|export| export.id.clone())
                .collect();

        let affected = update(&mut project, &[change(&dependency, 1, case.after)]);
        assert_eq!(
            affected.contains(&importer),
            case.importer_affected,
            "{}: importer affected, affected set {affected:?}",
            case.name
        );
        assert_eq!(
            project.last_update.semantic_cutoff, !case.importer_affected,
            "{}",
            case.name
        );
        if case.preserve_export_id {
            // The export keeps its identity because the declaration contract
            // is unchanged, though its declaration moved.
            let after_ids: Vec<String> =
                declaration_shape_for(project.current().unwrap(), &dependency)
                    .expect("shape after")
                    .exports
                    .iter()
                    .map(|export| export.id.clone())
                    .collect();
            assert_eq!(after_ids, before_ids, "{}", case.name);
            // The retained shape is the new generation's.
            assert_eq!(
                project.cached_shape(&dependency).expect("cached").exports[0].id,
                before_ids[0]
            );
            // A second span shift (back) is also absorbed.
            let affected = update(&mut project, &[change(&dependency, 2, case.before)]);
            assert!(
                !affected.contains(&importer),
                "second span shift unexpectedly affected the importer: {affected:?}"
            );
            assert!(
                project.last_update.old_shape_cached,
                "the shape was retained"
            );
        }
    }
}

/// `TestSourceFactsMemoAcrossGenerations`, the compiler-only half.
#[test]
fn affected_sets_across_generations() {
    let scratch = Scratch::new(&[
        ("tsconfig.json", SMALL_CONFIG),
        (
            "a.ts",
            "import { makeThing } from \"./b\";\n\nexport const value = makeThing();\n",
        ),
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
    let mut project = scratch.open();

    let affected = update(
        &mut project,
        &[change(&c, 1, "export const unrelated = 2;\n")],
    );
    assert!(
        !affected.contains(&a),
        "unrelated update affected a.ts: {affected:?}"
    );

    let affected = update(
        &mut project,
        &[change(
            &b,
            1,
            "export function makeThing(): number {\n  return 2;\n}\n",
        )],
    );
    assert!(
        !affected.contains(&a),
        "shape-preserving imported body edit: {affected:?}"
    );

    let affected = update(
        &mut project,
        &[change(
            &b,
            2,
            "export function makeThing(): string {\n  return \"changed\";\n}\n",
        )],
    );
    assert!(
        affected.contains(&a),
        "exported signature change must affect the importer: {affected:?}"
    );
    assert!(
        project.cached_shape(&a).is_none(),
        "the importer's retained shape is evicted"
    );

    // A multi-file update takes the full rebuild and clears retained shapes.
    assert!(project.cached_shape(&b).is_some());
    update(
        &mut project,
        &[
            change(&c, 3, "export const unrelated = 3;\n"),
            change(
                &b,
                3,
                "export function makeThing(): number {\n  return 3;\n}\n",
            ),
        ],
    );
    assert_eq!(project.last_update.kind, Some(UpdateKind::Rebuilt));
    assert!(
        project.cached_shape(&b).is_none(),
        "a full rebuild clears retained shapes"
    );
}

#[test]
fn updates_that_cannot_reuse_the_program_rebuild_it() {
    let scratch = Scratch::new(&[
        ("tsconfig.json", SMALL_CONFIG),
        ("a.ts", "export const a = 1;\n"),
        ("b.ts", "export const b = 2;\n"),
    ]);
    let (a, b) = (scratch.path("a.ts"), scratch.path("b.ts"));
    let mut project = scratch.open();
    // Adding an import changes the file's import list, which in-place
    // replacement does not support.
    let affected = update(
        &mut project,
        &[change(
            &a,
            1,
            "import { b } from \"./b\";\nexport const a = b;\n",
        )],
    );
    assert_eq!(project.last_update.kind, Some(UpdateKind::Rebuilt));
    assert!(affected.contains(&a));
    assert_eq!(
        project.forward_dependencies(&a),
        std::slice::from_ref(&b),
        "the graph advanced"
    );
    // The program is usable after the chain swap.
    let inventory = crate::backend::ModuleGraph::module_graph(
        &mut project,
        &Context::new(),
        &typefacts::v3::ModuleGraphRequest {
            imports: true,
            import_paths: vec![a.clone()],
            packages: false,
            distinct_bare_imports: false,
            omit_modules: false,
        },
    )
    .unwrap();
    assert_eq!(&*inventory.imports[0].resolved_path, b);
}

#[test]
fn deletes_and_config_edits_rebuild_and_unknown_files_join_the_program() {
    let scratch = Scratch::new(&[
        ("tsconfig.json", SMALL_CONFIG),
        ("a.ts", "export const a = 1;\n"),
        ("gone.ts", "export const gone = 1;\n"),
    ]);
    let (a, gone, fresh) = (
        scratch.path("a.ts"),
        scratch.path("gone.ts"),
        scratch.path("fresh.ts"),
    );
    let mut project = scratch.open();
    let paths = |project: &mut crate::project::Project| -> Vec<String> {
        project
            .source_files(&Context::new())
            .unwrap()
            .into_iter()
            .map(|file| file.path)
            .collect()
    };
    assert_eq!(paths(&mut project), [a.clone(), gone.clone()]);

    let affected = update(&mut project, &[deletion(&gone, 1)]);
    assert_eq!(project.last_update.kind, Some(UpdateKind::Rebuilt));
    assert!(affected.contains(&gone), "{affected:?}");
    assert_eq!(paths(&mut project), std::slice::from_ref(&a));

    // A file that exists only in the overlay joins the program through the
    // configuration's include pattern.
    update(
        &mut project,
        &[change(&fresh, 1, "export const fresh = 1;\n")],
    );
    assert_eq!(project.last_update.kind, Some(UpdateKind::Rebuilt));
    assert_eq!(paths(&mut project), [a.clone(), fresh.clone()]);
    assert!(
        !std::path::Path::new(&fresh).exists(),
        "nothing is written to disk"
    );

    // Editing the configuration rebuilds from the new bytes.
    update(
        &mut project,
        &[change(
            &scratch.config(),
            1,
            r#"{"compilerOptions":{"module":"esnext"},"files":["a.ts"]}"#,
        )],
    );
    assert_eq!(paths(&mut project), [a]);
}

#[test]
fn stale_and_empty_updates_change_nothing() {
    let scratch = Scratch::new(&[
        ("tsconfig.json", SMALL_CONFIG),
        ("a.ts", "export const a = 1;\n"),
    ]);
    let a = scratch.path("a.ts");
    let mut project = scratch.open();
    assert!(update(&mut project, &[]).is_empty());
    update(&mut project, &[change(&a, 5, "export const a = 5;\n")]);
    let number = project.generation_number();
    // Versions at or below the retained one are ignored.
    assert!(update(&mut project, &[change(&a, 5, "export const a = 6;\n")]).is_empty());
    assert!(update(&mut project, &[change(&a, 2, "export const a = 7;\n")]).is_empty());
    assert_eq!(project.generation_number(), number);
    let sources = project.source_files(&Context::new()).unwrap();
    assert_eq!(sources[0].source, b"export const a = 5;\n");
}

#[test]
fn a_cancelled_update_leaves_the_accepted_generation_intact() {
    let scratch = Scratch::new(&[
        ("tsconfig.json", SMALL_CONFIG),
        ("a.ts", "export function a(): number { return 1; }\n"),
        (
            "b.ts",
            "import { a } from \"./a\";\nexport const b = a();\n",
        ),
    ]);
    let a = scratch.path("a.ts");
    let mut project = scratch.open();
    let ctx = Context::new();
    ctx.cancel();
    let error = project
        .update(&ctx, &[change(&a, 1, "export const a = 2;\n")])
        .unwrap_err();
    assert_eq!(error.0, "context canceled");
    assert_eq!(project.generation_number(), 1);
    let sources = project.source_files(&Context::new()).unwrap();
    assert!(String::from_utf8_lossy(&sources[0].source).contains("return 1"));
    // And the project keeps working afterwards.
    let affected = update(
        &mut project,
        &[change(&a, 1, "export function a(): number { return 3; }\n")],
    );
    assert!(affected.contains(&a));
    assert_eq!(project.generation_number(), 2);
}

#[test]
fn a_failed_rebuild_leaves_the_accepted_generation_intact() {
    let scratch = Scratch::new(&[
        ("tsconfig.json", SMALL_CONFIG),
        ("a.ts", "export const a = 1;\n"),
    ]);
    let mut project = scratch.open();
    let error = project
        .update(
            &Context::new(),
            &[change(&scratch.config(), 1, "{ not json")],
        )
        .unwrap_err();
    assert!(error.0.starts_with("parse tsconfig: "), "{error}");
    assert_eq!(project.generation_number(), 1);
    assert_eq!(project.source_files(&Context::new()).unwrap().len(), 1);
    // A later valid edit applies on top of the old overlay, not the bad one.
    update(
        &mut project,
        &[change(&scratch.path("a.ts"), 1, "export const a = 2;\n")],
    );
    assert_eq!(project.generation_number(), 2);
}

#[test]
fn the_rebase_policy_replaces_the_chain_without_changing_the_answers() {
    let scratch = Scratch::new(&[
        ("tsconfig.json", SMALL_CONFIG),
        ("a.ts", "export function a(): number { return 0; }\n"),
        (
            "b.ts",
            "import { a } from \"./a\";\nexport const b = a();\n",
        ),
    ]);
    let a = scratch.path("a.ts");
    let mut project = scratch.open();
    let mut rebased_at = None;
    for version in 1..=u64::from(crate::project::REBASE_AFTER_UPDATES) + 2 {
        let outcome = project
            .update(
                &Context::new(),
                &[change(
                    &a,
                    version,
                    &format!("export function a(): number {{ return {version}; }}\n"),
                )],
            )
            .unwrap();
        assert!(
            !outcome.affected.contains(&scratch.path("b.ts")),
            "version {version}: {:?}",
            outcome.affected
        );
        // Every in-place update keeps the module graph, except the one a
        // rebase follows: that chain was built again from the disk as it is.
        assert_eq!(
            outcome.graph_kept, !project.last_update.rebased,
            "version {version}"
        );
        if project.last_update.rebased && rebased_at.is_none() {
            rebased_at = Some(version);
        }
    }
    assert_eq!(
        rebased_at,
        Some(u64::from(crate::project::REBASE_AFTER_UPDATES))
    );
    // After the rebase the chain starts counting again, and the accepted
    // content is the last edit.
    assert_eq!(project.current().unwrap().updates_since_build(), 2);
    let sources = project.source_files(&Context::new()).unwrap();
    let expected = format!(
        "export function a(): number {{ return {}; }}\n",
        u64::from(crate::project::REBASE_AFTER_UPDATES) + 2
    );
    assert_eq!(String::from_utf8_lossy(&sources[0].source), expected);
}

/// A rebuilt chain reuses the previous parsed configuration, which holds the
/// resolved project references the old chain's loader produced. Freeing the old
/// chain must not free anything the new one reads (run under
/// `TSRS_ARENA_POISON=1` to make a stale read visible).
#[test]
fn a_rebuilt_chain_over_project_references_keeps_the_reference_mapping() {
    let scratch = Scratch::new(&[
        (
            "tsconfig.json",
            r#"{
  "compilerOptions": { "strict": true, "module": "esnext", "target": "esnext", "moduleResolution": "bundler" },
  "include": ["app/**/*.ts"],
  "references": [{ "path": "./lib" }]
}"#,
        ),
        (
            "lib/tsconfig.json",
            r#"{
  "compilerOptions": {
    "strict": true, "module": "esnext", "target": "esnext", "moduleResolution": "bundler",
    "composite": true, "declaration": true, "outDir": "./dist", "rootDir": "./src"
  },
  "include": ["src/**/*.ts"]
}"#,
        ),
        (
            "lib/src/channel.ts",
            "export function channelFor() { return { live: true }; }\n",
        ),
        (
            "lib/dist/channel.d.ts",
            "export declare function channelFor(): { live: boolean };\n",
        ),
        (
            "app/index.ts",
            "import { channelFor } from \"../lib/dist/channel.js\";\nexport const channel = channelFor();\n",
        ),
        ("app/other.ts", "export const other = 1;\n"),
    ]);
    let (index, other) = (scratch.path("app/index.ts"), scratch.path("app/other.ts"));
    let source = scratch.path("lib/src/channel.ts");
    let mut project = scratch.open();
    let mapping = |project: &mut crate::project::Project| {
        let inventory = crate::backend::ModuleGraph::module_graph(
            project,
            &Context::new(),
            &typefacts::v3::ModuleGraphRequest {
                imports: true,
                import_paths: Vec::new(),
                packages: false,
                distinct_bare_imports: false,
                omit_modules: false,
            },
        )
        .unwrap();
        let module = inventory
            .modules
            .iter()
            .find(|module| *module.path == *source)
            .unwrap();
        (
            module
                .project_reference
                .clone()
                .expect("project reference")
                .output_dts
                .to_string(),
            inventory
                .imports
                .iter()
                .map(|fact| fact.included_path.to_string())
                .collect::<Vec<_>>(),
        )
    };
    let before = mapping(&mut project);
    // Adding an import cannot be absorbed in place: a fresh chain is built from
    // the same configuration and the old chain is freed.
    update(
        &mut project,
        &[change(
            &other,
            1,
            "import { channelFor } from \"../lib/dist/channel.js\";\nexport const other = channelFor();\n",
        )],
    );
    assert_eq!(project.last_update.kind, Some(UpdateKind::Rebuilt));
    let after = mapping(&mut project);
    assert_eq!(after.0, before.0);
    assert_eq!(after.1.iter().filter(|path| **path == source).count(), 2);
    // And an in-place edit afterwards still works on the new chain.
    let affected = update(
        &mut project,
        &[change(
            &index,
            1,
            "import { channelFor } from \"../lib/dist/channel.js\";\nexport const channel = channelFor();\n// edit\n",
        )],
    );
    assert_eq!(project.last_update.kind, Some(UpdateKind::Reused));
    assert!(affected.contains(&index));
}

/// A program build asks the disk through lookups that last one build
/// (`OverlayFs::build_scope`). A file created on disk after a build that
/// probed for it and found nothing is found by the next build, whichever way
/// that build is made: nothing a build learned about the disk is kept.
#[test]
fn a_file_created_after_a_failed_probe_is_found_by_the_next_build() {
    let scratch = Scratch::new(&[
        ("tsconfig.json", SMALL_CONFIG),
        (
            "a.ts",
            "import { x } from \"./x\";\nexport const a = typeof x;\n",
        ),
        ("b.ts", "export const b = 2;\n"),
    ]);
    let (a, x) = (scratch.path("a.ts"), scratch.path("x.ts"));
    let mut project = scratch.open();
    let resolution_of_x = |project: &mut crate::project::Project| -> String {
        let inventory = crate::backend::ModuleGraph::module_graph(
            project,
            &Context::new(),
            &typefacts::v3::ModuleGraphRequest {
                imports: true,
                import_paths: vec![a.clone()],
                packages: false,
                distinct_bare_imports: false,
                omit_modules: false,
            },
        )
        .unwrap();
        inventory.imports[0].resolved_path.to_string()
    };
    assert_eq!(resolution_of_x(&mut project), "", "x.ts does not exist yet");

    scratch.write_all(&[("x.ts", "export const x = 1;\n")]);
    // An import edit rebuilds the chain from the disk as it is now.
    update(
        &mut project,
        &[change(
            &a,
            1,
            "import { x } from \"./x\";\nimport { b } from \"./b\";\nexport const a = [x, b];\n",
        )],
    );
    assert_eq!(project.last_update.kind, Some(UpdateKind::Rebuilt));
    assert_eq!(resolution_of_x(&mut project), x);

    // And again: a second missing file, created after the build that missed
    // it.
    scratch.write_all(&[("y.ts", "export const y = 1;\n")]);
    update(
        &mut project,
        &[change(
            &a,
            2,
            "import { x } from \"./x\";\nimport { y } from \"./y\";\nexport const a = [x, y];\n",
        )],
    );
    assert_eq!(project.last_update.kind, Some(UpdateKind::Rebuilt));
    assert_eq!(
        project.forward_dependencies(&a),
        [x.clone(), scratch.path("y.ts")],
        "the second rebuild saw y.ts, created after the first one"
    );
}

const JSX_CONFIG: &str = r#"{"compilerOptions":{"module":"esnext","moduleResolution":"bundler","target":"esnext","jsx":"react-jsx","jsxImportSource":"solid-js"},"include":["*.ts","*.tsx"]}"#;

/// What an update says about the module graph (`UpdateOutcome::graph_kept`):
/// true exactly when the compiler replaced the one changed file in place and
/// nothing was rebuilt, and then every other file's import rows, the files the
/// program holds and their facts are the previous generation's. Every edit
/// that can move another file's module resolution (an import, a reference
/// directive, a module augmentation, a JSX runtime pragma) says false; whatever
/// an update says, the graph it leaves equals a fresh program's.
#[test]
fn an_update_keeps_the_module_graph_only_when_the_compiler_replaced_the_file_in_place() {
    use crate::backend::{ModuleGraph, ModuleInventory};
    use typefacts::v3::ModuleGraphRequest;

    let originals = [
        ("a.ts", "import { b } from \"./b\";\nexport const a = b;\n"),
        ("b.ts", "export const b = 1;\n"),
        ("script.ts", "var script = 1;\n"),
        (
            "view.tsx",
            "export const View = () => <div class=\"x\" />;\n",
        ),
    ];
    let mut files = vec![("tsconfig.json", JSX_CONFIG)];
    files.extend(originals);
    let scratch = Scratch::new(&files);
    let mut project = scratch.open();
    let inventory = |project: &mut crate::project::Project| -> ModuleInventory {
        project
            .module_graph(
                &Context::new(),
                &ModuleGraphRequest {
                    imports: true,
                    packages: true,
                    ..ModuleGraphRequest::default()
                },
            )
            .unwrap()
    };
    let mut version = 0_u64;
    // (name, file, edited text, whether the compiler replaces in place)
    let steps: [(&str, &str, &str, Option<bool>); 13] = [
        ("body edit", "b.ts", "export const b = 2;\n", Some(true)),
        (
            "a JSX body edit under the same runtime import",
            "view.tsx",
            "export const View = () => <div class=\"y\" />;\n",
            Some(true),
        ),
        (
            "an added import",
            "a.ts",
            "import { b } from \"./b\";\nimport { script } from \"./script\";\nexport const a = b;\n",
            Some(false),
        ),
        (
            "the import removed again",
            "a.ts",
            "import { b } from \"./b\";\nexport const a = b;\n",
            Some(false),
        ),
        (
            "a changed specifier",
            "a.ts",
            "import { b } from \"./b.ts\";\nexport const a = b;\n",
            Some(false),
        ),
        (
            "an added reference path directive",
            "a.ts",
            "/// <reference path=\"./script.ts\" />\nimport { b } from \"./b.ts\";\nexport const a = b;\n",
            Some(false),
        ),
        (
            "an added reference types directive",
            "a.ts",
            "/// <reference types=\"solid-js\" />\nimport { b } from \"./b.ts\";\nexport const a = b;\n",
            Some(false),
        ),
        (
            "an added JSX runtime pragma",
            "view.tsx",
            "/** @jsxImportSource preact */\nexport const View = () => <div class=\"y\" />;\n",
            Some(false),
        ),
        (
            "an added global augmentation in a module",
            "b.ts",
            "export const b = 2;\ndeclare global { interface Window { w: number } }\n",
            Some(false),
        ),
        (
            "an added module augmentation in a module",
            "b.ts",
            "export const b = 2;\ndeclare module \"x\" { export const y: number; }\n",
            Some(false),
        ),
        (
            "an added ambient module in a script",
            "script.ts",
            "var script = 1;\ndeclare module \"x\" { export const y: number; }\n",
            Some(false),
        ),
        (
            "the edit that removes the augmentation",
            "b.ts",
            "export const b = 3;\n",
            Some(false),
        ),
        (
            "a body edit on the rebuilt chain",
            "b.ts",
            "export const b = 4;\n",
            Some(true),
        ),
    ];
    let mut kept_updates = 0;
    for (name, file, text, expected) in steps {
        let before = inventory(&mut project);
        version += 1;
        let path = scratch.path(file);
        let outcome = project
            .update(&Context::new(), &[change(&path, version, text)])
            .unwrap();
        let kind = project.last_update.kind;
        let rebased = project.last_update.rebased;
        assert_eq!(
            outcome.graph_kept,
            kind == Some(UpdateKind::Reused) && !rebased,
            "{name}: {kind:?}"
        );
        if let Some(expected) = expected {
            assert_eq!(outcome.graph_kept, expected, "{name}: {kind:?}");
        }
        let after = inventory(&mut project);
        if outcome.graph_kept {
            kept_updates += 1;
            // The files the program holds, and every other file's rows, are
            // what they were: only the edited file's own rows can differ.
            assert_eq!(before.modules, after.modules, "{name}");
            assert_eq!(before.unmapped_roots, after.unmapped_roots, "{name}");
            let others = |inventory: &ModuleInventory| {
                inventory
                    .imports
                    .iter()
                    .filter(|fact| *fact.specifier.path != path)
                    .cloned()
                    .collect::<Vec<_>>()
            };
            assert_eq!(others(&before), others(&after), "{name}");
        }
        // Whatever the update said, the graph is a fresh program's: the same
        // edit written to disk and opened from scratch.
        std::fs::write(&path, text).unwrap();
        let mut fresh = scratch.open();
        assert_eq!(inventory(&mut fresh), after, "{name}");
    }
    assert!(kept_updates >= 2, "{kept_updates}");
}
