//! Reused per-file passes (`passes`, ADR 03XX-p6), at binder-level
//! dependencies: each dependency a kept pass reads (a file's text, exports, a
//! global script, an alias target, a merged namespace) is changed by one edit,
//! and the next generation must equal a fresh one over the same program,
//! while unaffected files are still reused.

use super::project_support::Scratch;
use super::randomized_incremental::{first_difference, probe_with};
use super::symbol_integration::{STRICT_CONFIG, is_ident, open_closure};
use crate::closure::DemandClosure;
use crate::context::Context;
use crate::references::BinderRetention;
use typefacts::Location;
use typefacts::v3::{EntityDemand, FileChange};

/// Every word of every `.ts` source: its symbol, references and reference
/// space, so a stale binder-level answer anywhere shows; every third also its
/// runtime identity (which takes the live symbol) and type.
fn word_demands(closure: &mut DemandClosure) -> Vec<EntityDemand> {
    let mut demands = Vec::new();
    for source in closure.source_files(&Context::new()).unwrap() {
        if !source.path.ends_with(".ts") || source.path.ends_with(".d.ts") {
            continue;
        }
        let path = crate::gocompat::clean(&source.path);
        let bytes = &source.source;
        let mut index = 0;
        while index < bytes.len() {
            if !is_ident(bytes[index]) || (index > 0 && is_ident(bytes[index - 1])) {
                index += 1;
                continue;
            }
            let start = index;
            while index < bytes.len() && is_ident(bytes[index]) {
                index += 1;
            }
            if bytes[start].is_ascii_digit() {
                continue;
            }
            let full = demands.len() % 3 == 0;
            demands.push(EntityDemand {
                location: Location {
                    path: path.as_str().into(),
                    start_byte: start as u64,
                    end_byte: index as u64,
                },
                symbol: true,
                references: true,
                reference_space: true,
                runtime_identity: full,
                type_descriptor: full,
                ..EntityDemand::default()
            });
        }
    }
    demands
}

fn change(scratch: &Scratch, name: &str, version: u64, source: Option<&str>) -> FileChange {
    FileChange {
        path: scratch.path(name),
        source: source.unwrap_or_default().as_bytes().to_vec(),
        deleted: source.is_none(),
        version,
    }
}

/// Applies `steps` (one update each) to a closure that keeps its retained
/// answers, comparing every generation with a closure that replays the same
/// updates and analyzes once; returns what each step reused.
fn retained_matches_fresh(
    files: &[(&str, &str)],
    steps: &[&[(&str, Option<&str>)]],
) -> Vec<BinderRetention> {
    let ctx = Context::new();
    let mut files = files.to_vec();
    files.push(("warm.ts", "export const warm = 0;\n"));
    let scratch = Scratch::new(&files);
    let config = scratch.config();
    let mut incremental = open_closure(&config);
    drop(probe_with(&mut incremental, 1, false, word_demands));
    let _ = incremental.backend_mut().binder_retention();
    let mut versions = std::collections::HashMap::<&str, u64>::new();
    let mut history: Vec<Vec<FileChange>> = Vec::new();
    let mut reuse = Vec::new();
    // A warm-up update before the steps; it is compared like a step.
    let warm: &[(&str, Option<&str>)] = &[("warm.ts", Some("export const warm = 1;\n"))];
    for (step, edits) in std::iter::once(&warm).chain(steps).enumerate() {
        let changes: Vec<FileChange> = edits
            .iter()
            .map(|(name, source)| {
                let version = versions.entry(name).or_default();
                *version += 1;
                change(&scratch, name, *version, *source)
            })
            .collect();
        incremental.update(&ctx, &changes).unwrap();
        history.push(changes);
        let generation = step as u64 + 2;
        let retained = probe_with(&mut incremental, generation, true, word_demands);
        let mut counts = incremental.stats().retention;
        let late = incremental.backend_mut().binder_retention();
        reuse.push(BinderRetention {
            reused_reference_files: counts.cached_reference_facts + late.reused_reference_files,
            scanned_reference_files: counts.recomputed_references + late.scanned_reference_files,
            reused_symbols: counts.cached_symbol_facts,
            resolved_symbols: std::mem::take(&mut counts.recomputed_symbol_facts),
            reused_symbol_facts: late.reused_symbol_facts,
            resolved_symbol_facts: late.resolved_symbol_facts,
            ..BinderRetention::default()
        });
        let mut fresh = open_closure(&config);
        for changes in &history {
            fresh.update(&ctx, changes).unwrap();
        }
        let whole = probe_with(&mut fresh, generation, false, word_demands);
        let mut failures = Vec::new();
        failures.extend(first_difference(
            "entity rows",
            &retained.lines,
            &whole.lines,
        ));
        failures.extend(first_difference(
            "symbol evidence",
            &retained.symbols,
            &whole.symbols,
        ));
        failures.extend(first_difference("file rows", &retained.files, &whole.files));
        assert!(
            failures.is_empty(),
            "step {step}: retained generation differs from fresh\n{}",
            failures.join("\n")
        );
    }
    reuse.remove(0);
    reuse
}

/// `export import` in another module makes a namespace member a module
/// export, which changes its key (`Decl` to `Export`): `Exports(a.ts)`.
#[test]
fn an_export_added_in_another_module_changes_a_retained_id() {
    let a = "export namespace N {\n  export const inner = 1;\n  export const again = inner;\n}\n";
    let g = "import { N } from \"./a\";\nexport const g = N;\n";
    let g_exporting =
        "import { N } from \"./a\";\nexport const g = N;\nexport import inner = N.inner;\n";
    let reuse = retained_matches_fresh(
        &[
            ("tsconfig.json", STRICT_CONFIG),
            ("a.ts", a),
            ("g.ts", g),
            ("z.ts", "export const z = 1;\n"),
        ],
        &[
            &[("g.ts", Some(g_exporting))],
            &[("z.ts", Some("export const z = 2;\n"))],
            &[("g.ts", Some(g))],
        ],
    );
    // Per-file passes keep whole passes keyed by their demands, which this
    // test's word demands shift in every file after an edited one: retained
    // == fresh above is what holds.
    let _ = reuse;
}

/// A global removed from a script leaves a module's identifier unresolved,
/// and one added back resolves it: `Ambient`.
#[test]
fn a_global_script_edit_changes_retained_resolutions() {
    let script = "declare function G(): number;\ndeclare var gv: number;\n";
    let user = "export const u = G() + 1;\nexport function f() { return G() + gv; }\n";
    let reuse = retained_matches_fresh(
        &[
            ("tsconfig.json", STRICT_CONFIG),
            ("g.ts", script),
            ("m.ts", user),
            ("z.ts", "export const z = 1;\n"),
        ],
        &[
            &[("g.ts", Some("declare var gv: number;\n"))],
            &[("z.ts", Some("export const z = 2;\n"))],
            &[("g.ts", Some(script))],
            &[("z.ts", Some("export const z = 3;\n"))],
        ],
    );
    assert!(reuse[1].reused_reference_files > 0, "{reuse:?}");
}

/// A module augmentation from another module merges into a retained file's
/// interface: `Ambient` covers augmentations as well as scripts.
#[test]
fn a_module_augmentation_changes_retained_declarations() {
    let a = "export interface Box { a: number }\nexport const make = (): Box => ({ a: 1 });\nconst local: Box = make();\nexport const read = local.a;\n";
    let augment = "import { Box } from \"./a\";\ndeclare module \"./a\" {\n  interface Box { b?: string }\n}\nexport const use = (box: Box) => box.b;\n";
    retained_matches_fresh(
        &[
            ("tsconfig.json", STRICT_CONFIG),
            ("a.ts", a),
            ("aug.ts", "export const nothing = 0;\n"),
            ("z.ts", "export const z = 1;\n"),
        ],
        &[
            &[("aug.ts", Some(augment))],
            &[("z.ts", Some("export const z = 2;\n"))],
            &[("aug.ts", Some("export const nothing = 0;\n"))],
        ],
    );
}

/// An import's target moves behind a re-export, then to another module, then
/// the module that provided it leaves and returns: the alias sights catch
/// every move (exports, resolution, files entering the program).
#[test]
fn alias_targets_are_rechecked_on_reuse() {
    let user = "import { x } from \"./mid\";\nexport const u = x + x;\n";
    let mid_local = "export const x = 1;\n";
    let mid_reexport = "export { x } from \"./low\";\n";
    let mid_star = "export * from \"./other\";\n";
    retained_matches_fresh(
        &[
            ("tsconfig.json", STRICT_CONFIG),
            ("user.ts", user),
            ("mid.ts", mid_local),
            ("low.ts", "export const x = 2;\n"),
            ("other.ts", "export const x = 3;\n"),
        ],
        &[
            &[("mid.ts", Some(mid_reexport))],
            &[(
                "low.ts",
                Some("export const pad = 0;\nexport const x = 2;\n"),
            )],
            &[("mid.ts", Some(mid_star))],
            &[("other.ts", None)],
            &[("other.ts", Some("export const x = 3;\n"))],
            &[("mid.ts", Some(mid_local))],
        ],
    );
}

/// The compiler options change what resolves (`noUncheckedIndexedAccess` is
/// not enough: `module` changes resolution): `Options`.
#[test]
fn a_tsconfig_change_drops_every_retained_answer() {
    let classic = r#"{"compilerOptions":{"module":"commonjs","moduleResolution":"node10","target":"esnext","strict":true},"include":["*.ts"]}"#;
    let reuse = retained_matches_fresh(
        &[
            ("tsconfig.json", STRICT_CONFIG),
            ("a.ts", "export const a = 1;\n"),
            ("b.ts", "import { a } from \"./a\";\nexport const b = a;\n"),
        ],
        &[
            &[("tsconfig.json", Some(classic))],
            &[("a.ts", Some("export const a = 2;\n"))],
        ],
    );
    assert_eq!(reuse[0].reused_reference_files, 0, "{reuse:?}");
    assert!(reuse[1].reused_reference_files > 0, "{reuse:?}");
}

/// A span shift in a declaring file moves `Decl` ids but not `Export` ids;
/// every pass whose read set holds the edited file runs again.
#[test]
fn a_prefix_edit_rescans_only_the_edited_file() {
    let a = "export function f() { return 1; }\nfunction hidden() { return f(); }\nexport const h = hidden();\n";
    let reuse = retained_matches_fresh(
        &[
            ("tsconfig.json", STRICT_CONFIG),
            ("a.ts", a),
            (
                "b.ts",
                "import { f, h } from \"./a\";\nexport const b = f() + h;\n",
            ),
            ("c.ts", "import { b } from \"./b\";\nexport const c = b;\n"),
        ],
        &[&[("a.ts", Some(&format!("// shifted\n{a}")))]],
    );
    // b.ts and c.ts read a.ts (their read sets hold it): only warm.ts's
    // pass is kept, and its word demands moved with a.ts's words.
    assert_eq!(reuse[0].scanned_reference_files, 4, "{reuse:?}");
}

/// Scope lookup resolves an alias to test its meaning: an import of an
/// interface does not answer a value use, so the use is unresolved (or a
/// global of that name answers it), until the import's module leaves the
/// program (an unknown target answers every meaning) or exports a value.
/// The answer depends on another module with no change to this file's text
/// (V4, seed 87): such a lookup is replayed and compared on every reuse.
#[test]
fn a_lookup_an_alias_answers_follows_the_alias_target() {
    let user = "import { v } from \"./m\";\nexport const u = v(\"s\");\nexport function f() { return v; }\n";
    let script = "declare function v(s: string): number;\n";
    let reuse = retained_matches_fresh(
        &[
            ("tsconfig.json", STRICT_CONFIG),
            ("user.ts", user),
            ("m.ts", "export interface v { plain: number }\n"),
            ("z.ts", "export const z = 1;\n"),
        ],
        &[
            &[("m.ts", None)],
            &[("z.ts", Some("export const z = 2;\n"))],
            &[("m.ts", Some("export interface v { plain: number }\n"))],
            &[("g.ts", Some(script))],
            &[("m.ts", Some("export const v = (s: string) => s.length;\n"))],
            &[("m.ts", Some("export interface v { plain: number }\n"))],
        ],
    );
    assert!(
        reuse.iter().any(|step| step.reused_reference_files > 0),
        "{reuse:?}"
    );
}

/// Pass reuse needs no update to start: a table scans every file once, and
/// the next table over an edit runs only the passes whose read set holds the
/// edited file. `a.ts` reads nothing `b.ts` declares, so its pass is kept.
#[test]
fn a_table_scans_every_file_and_an_edit_reruns_only_its_readers() {
    let ctx = Context::new();
    let scratch = Scratch::new(&[
        ("tsconfig.json", STRICT_CONFIG),
        ("a.ts", "export const a = 1;\n"),
        ("b.ts", "import { a } from \"./a\";\nexport const b = a;\n"),
    ]);
    let mut closure = open_closure(&scratch.config());
    drop(probe_with(&mut closure, 1, false, word_demands));
    let first = closure.stats().retention;
    assert_eq!(
        (first.cached_reference_facts, first.recomputed_references),
        (0, 2),
        "{first:?}"
    );
    let edit = change(
        &scratch,
        "b.ts",
        1,
        Some("import { a } from \"./a\";\nexport const b = a + 1;\n"),
    );
    closure.update(&ctx, std::slice::from_ref(&edit)).unwrap();
    drop(probe_with(&mut closure, 2, false, word_demands));
    let again = closure.stats().retention;
    assert_eq!(
        (again.cached_reference_facts, again.recomputed_references),
        (1, 1),
        "{again:?}"
    );
}

/// Queries naming one id twice are merged before they are answered
/// (references if any asks, declarations unless every query is
/// references-only); the passes' rows answer the merged query exactly as a
/// fresh closure does.
#[test]
fn retained_symbol_facts_answer_merged_duplicate_queries() {
    let ctx = Context::new();
    let files = [
        ("tsconfig.json", STRICT_CONFIG),
        (
            "a.ts",
            "export function f() { return 1; }\nexport const a = f();\n",
        ),
        (
            "b.ts",
            "import { f, a } from \"./a\";\nexport const b = f() + a;\n",
        ),
        ("z.ts", "export const z = 1;\n"),
    ];
    let scratch = Scratch::new(&files);
    let config = scratch.config();
    let mut incremental = open_closure(&config);
    let mut lines = probe_with(&mut incremental, 1, false, word_demands).lines;
    for (generation, version) in [(2, 1), (3, 2)] {
        let change = FileChange {
            path: scratch.path("z.ts"),
            source: format!("export const z = {version};\n").into_bytes(),
            deleted: false,
            version,
        };
        incremental
            .update(&ctx, std::slice::from_ref(&change))
            .unwrap();
        lines = probe_with(&mut incremental, generation, true, word_demands).lines;
    }
    let ids = super::randomized_incremental::ids_in(&lines);
    assert!(!ids.is_empty());
    let queries: Vec<typefacts::v3::SymbolQuery> = ids
        .iter()
        .flat_map(|id| {
            [(false, false), (true, true), (true, false)].map(|(references, references_only)| {
                typefacts::v3::SymbolQuery {
                    id: id.clone(),
                    references,
                    references_only,
                }
            })
        })
        .collect();
    let _ = incremental.backend_mut().binder_retention();
    let retained = incremental.resolve_symbol_evidence(&ctx, &queries).unwrap();
    assert!(
        incremental
            .backend_mut()
            .binder_retention()
            .reused_symbol_facts
            > 0,
        "no pass row answered"
    );
    let mut fresh = open_closure(&config);
    for version in [1, 2] {
        let change = FileChange {
            path: scratch.path("z.ts"),
            source: format!("export const z = {version};\n").into_bytes(),
            deleted: false,
            version,
        };
        fresh.update(&ctx, std::slice::from_ref(&change)).unwrap();
    }
    drop(probe_with(&mut fresh, 3, false, word_demands));
    let whole = fresh.resolve_symbol_evidence(&ctx, &queries).unwrap();
    assert_eq!(retained.len(), ids.len());
    assert_eq!(format!("{retained:?}"), format!("{whole:?}"));
}

/// A namespace merged into the global scope from another file can carry an
/// alias (`declare global { namespace N { export import x = M.x } }`): a
/// lookup inside another file's `namespace N` meets it with no import of
/// its own, and its answer follows the alias target (PR #73 review). The
/// target moves behind a re-export, then becomes a type the value use cannot
/// see: reference lists and entity facts must follow.
#[test]
fn an_alias_in_a_namespace_merged_from_another_file_is_rechecked() {
    let g = "import * as M from \"./m\";\ndeclare global { namespace N { export import x = M.x; } }\nexport {};\n";
    retained_matches_fresh(
        &[
            ("tsconfig.json", STRICT_CONFIG),
            ("m.ts", "export const x = 1;\n"),
            ("other.ts", "export const x = 2;\n"),
            ("g.ts", g),
            ("u.ts", "namespace N { export const y = x; }\n"),
        ],
        &[
            &[("m.ts", Some("export { x } from \"./other\";\n"))],
            &[("m.ts", Some("export interface x { a: number }\n"))],
            &[("m.ts", Some("export const x = 1;\n"))],
        ],
    );
}

/// The same alias reaches a script namespace merged across two scripts, and
/// a global enum and class merged across files: every scope a merge spans is
/// recomputed.
#[test]
fn scopes_merged_across_files_are_recomputed() {
    let g = "import * as M from \"./m\";\ndeclare global { namespace N { export import x = M.x; } enum E { A = 1 } }\nexport {};\n";
    retained_matches_fresh(
        &[
            ("tsconfig.json", STRICT_CONFIG),
            ("m.ts", "export const x = 1;\n"),
            ("other.ts", "export const x = 2;\n"),
            ("g.ts", g),
            ("s1.ts", "namespace N { export const z = 3; }\n"),
            (
                "u.ts",
                "namespace N { export const y = x + z; }\nenum E { B = A }\n",
            ),
        ],
        &[
            &[("m.ts", Some("export { x } from \"./other\";\n"))],
            &[("other.ts", Some("export type x = number;\n"))],
            &[("m.ts", Some("export interface x { a: number }\n"))],
        ],
    );
}
