//! Per-file passes with reuse and a pool (instant-edits P3, `passes`): a
//! kept pass output is served only while nothing its read set holds changed,
//! every table equals a fresh project's, and the pool's tables equal the
//! sequential ones.

use super::project_support::Scratch;
use super::symbol_integration::{STRICT_CONFIG, groups, is_ident, paths};
use crate::backend::ClosureBackend;
use crate::closure::DemandClosure;
use crate::context::Context;
use crate::gocompat::clean;
use crate::model::SemanticDemandRun;
use crate::project::Project;
use std::collections::BTreeMap;
use typefacts::Location;
use typefacts::v3::{EntityDemand, FileChange, SymbolQuery};

const A: &str = "export function f(x: number) { return x + 1; }\nexport const g = { value: 2 };\n";
const B: &str = "import { f, g } from \"./a\";\nexport const b = f(g.value);\nexport async function run() { await Promise.resolve(b); return parseInt(\"1\"); }\n";
const C: &str = "export const z = 1;\nexport function zz() { return z; }\n";
const D: &str = "export const dd = helper(G);\n";
const E: &str = "import { z } from \"./c\";\nexport const ez = z;\n";
const GLOB: &str = "declare var G: number;\nfunction helper(n: number) { return n * G; }\n";

fn files() -> Vec<(&'static str, &'static str)> {
    vec![
        ("tsconfig.json", STRICT_CONFIG),
        ("a.ts", A),
        ("b.ts", B),
        ("c.ts", C),
        ("d.ts", D),
        ("e.ts", E),
        ("glob.ts", GLOB),
    ]
}

/// Per-file passes with `workers` workers, reuse (verified: a reused pass
/// that differs from a fresh one panics) or none.
fn open(scratch: &Scratch, workers: usize, reuse: bool) -> Project {
    let mut project = scratch.open();
    project.select_passes_for_test(workers, reuse, reuse);
    project
}

/// Every word of every `.ts` source of `sources`: symbol, reference space,
/// type, a resolved call before `(`, and async facts at `run`.
fn demands(scratch: &Scratch, sources: &BTreeMap<String, String>) -> Vec<EntityDemand> {
    let mut demands = Vec::new();
    for (name, text) in sources {
        if !name.ends_with(".ts") {
            continue;
        }
        let path = clean(&scratch.path(name));
        let bytes = text.as_bytes();
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
            demands.push(EntityDemand {
                location: Location {
                    path: path.as_str().into(),
                    start_byte: start as u64,
                    end_byte: index as u64,
                },
                symbol: true,
                reference_space: true,
                type_descriptor: true,
                resolved_call: bytes.get(index) == Some(&b'('),
                r#async: &text[start..index] == "run",
                ..EntityDemand::default()
            });
        }
    }
    demands
}

/// One table's runs, async facts and the symbol rows of every id they name,
/// as text.
fn table(project: &mut Project, demands: &[EntityDemand]) -> String {
    table_with(project, demands, false)
}

/// [`table`], through the sequential shared-identity run when `sequential`.
fn table_with(project: &mut Project, demands: &[EntityDemand], sequential: bool) -> String {
    let ctx = Context::new();
    let groups = groups(demands);
    let runs: Vec<SemanticDemandRun<'_>> = groups
        .iter()
        .map(|group| SemanticDemandRun {
            path: &group.path,
            demands: &group.demands,
            shared: None,
        })
        .collect();
    let locations: Vec<Location> = demands
        .iter()
        .filter(|demand| demand.r#async)
        .map(|demand| demand.location.clone())
        .collect();
    let (results, facts) = if sequential {
        let (results, facts) = project
            .sequential_demand_runs(&ctx, &runs, &locations)
            .unwrap();
        (
            results
                .into_iter()
                .map(crate::model::RunAnswer::Result)
                .collect(),
            facts,
        )
    } else {
        project
            .per_file_demand_runs(&ctx, &runs, &locations)
            .unwrap()
    };
    // As the table holds them: one row per location.
    let results = entity_rows(results, &runs);
    let mut ids: Vec<std::sync::Arc<str>> = Vec::new();
    for result in &results {
        for entity in result.iter() {
            ids.push(entity.symbol.clone());
            if let Some(call) = &entity.resolved_call {
                ids.push(call.target.clone());
            }
        }
    }
    ids.retain(|id| !id.is_empty());
    ids.sort();
    ids.dedup();
    let queries: Vec<SymbolQuery> = ids
        .into_iter()
        .map(|id| SymbolQuery {
            id,
            references: true,
            references_only: false,
        })
        .collect();
    let evidence = project.per_file_symbol_evidence(&ctx, &queries).unwrap();
    format!("{results:#?}\n{facts:#?}\n{evidence:#?}")
}

/// Each run's entity rows (and durability), as the table holds them.
fn entity_rows(
    answers: Vec<crate::model::RunAnswer>,
    runs: &[SemanticDemandRun<'_>],
) -> Vec<std::sync::Arc<[typefacts::EntityFact]>> {
    answers
        .into_iter()
        .zip(runs)
        .map(|(answer, run)| answer.into_rows(run.path, run.demands).unwrap().0)
        .collect()
}

fn first_line_difference(left: &str, right: &str) -> Option<String> {
    left.lines()
        .zip(right.lines())
        .enumerate()
        .find(|(_, (a, b))| a != b)
        .map(|(line, (a, b))| format!("line {line}: {a} | {b}"))
        .or_else(|| {
            (left.lines().count() != right.lines().count()).then(|| "line counts differ".to_owned())
        })
}

/// An edit (`None` deletes) and how many passes the next table must run.
struct Step {
    edits: Vec<(&'static str, Option<String>)>,
    ran: usize,
    what: &'static str,
}

/// Applies `steps`, one update each, to a project that keeps its passes,
/// checking every table against a fresh project replaying the same updates.
#[test]
fn kept_passes_rerun_exactly_what_an_edit_can_change() {
    let ctx = Context::new();
    let scratch = Scratch::new(&files());
    let mut sources: BTreeMap<String, String> = files()
        .into_iter()
        .filter(|(name, _)| name.ends_with(".ts"))
        .map(|(name, text)| (name.to_owned(), text.to_owned()))
        .collect();
    let mut incremental = open(&scratch, 2, true);
    let first = table(&mut incremental, &demands(&scratch, &sources));
    let stats = incremental.per_file_stats().reuse;
    assert_eq!((stats.ran, stats.reused), (6, 0), "{stats:?}");
    let mut fresh = open(&scratch, 1, false);
    assert_eq!(first, table(&mut fresh, &demands(&scratch, &sources)));

    let steps = [
        Step {
            edits: vec![("a.ts", Some(A.replace("x + 1", "x + 2")))],
            // a.ts and its importer b.ts.
            ran: 2,
            what: "a body edit in an imported module",
        },
        Step {
            edits: vec![("c.ts", Some(C.replace("= 1", "= 10")))],
            ran: 2,
            what: "an edit that moves an exported declaration",
        },
        Step {
            edits: vec![("d.ts", Some(D.replace("helper(G)", "helper(G + 1)")))],
            ran: 1,
            what: "a leaf module edit",
        },
        Step {
            edits: vec![("glob.ts", Some(GLOB.replace("n * G", "n * G * 2")))],
            ran: 6,
            what: "a global contributor's edit",
        },
        Step {
            edits: vec![("f.ts", Some("export const ff = 1;\n".to_owned()))],
            // Only f.ts: nothing reads it (a rebuilt chain keeps every pass
            // whose read set kept its text, exports and resolutions).
            ran: 1,
            what: "a file entering the program (membership, a rebuilt chain)",
        },
        Step {
            edits: vec![(
                "c.ts",
                Some(format!(
                    "import {{ f }} from \"./a\";\n{}",
                    C.replace("= 1", "= f(1)")
                )),
            )],
            // c.ts and its importer e.ts; a.ts gains a reader, not an input.
            ran: 2,
            what: "a changed import (resolution, a rebuilt chain)",
        },
        Step {
            edits: vec![("f.ts", None)],
            // Nothing read f.ts.
            ran: 0,
            what: "a file leaving the program",
        },
        Step {
            edits: vec![("b.ts", Some(B.replace("g.value", "g.value + 1")))],
            ran: 1,
            what: "an importer's edit",
        },
    ];
    let mut versions: BTreeMap<&str, u64> = BTreeMap::new();
    let mut history: Vec<Vec<FileChange>> = Vec::new();
    for step in &steps {
        let mut changes = Vec::new();
        for (name, text) in &step.edits {
            let version = versions.entry(name).or_default();
            *version += 1;
            match text {
                Some(text) => {
                    sources.insert((*name).to_owned(), text.clone());
                }
                None => {
                    sources.remove(*name);
                }
            }
            changes.push(FileChange {
                path: scratch.path(name),
                source: text.clone().unwrap_or_default().into_bytes(),
                deleted: text.is_none(),
                version: *version,
            });
        }
        ClosureBackend::update(&mut incremental, &ctx, &changes).unwrap();
        history.push(changes);
        let demands = demands(&scratch, &sources);
        let kept = table(&mut incremental, &demands);
        let stats = incremental.per_file_stats().reuse;
        assert_eq!(stats.mismatches, 0, "{}: {stats:?}", step.what);
        assert_eq!(stats.ran, step.ran, "{}: {stats:?}", step.what);
        assert_eq!(
            stats.reused,
            sources.len() - step.ran,
            "{}: {stats:?}",
            step.what
        );
        let mut fresh = open(&scratch, 1, false);
        for changes in &history {
            ClosureBackend::update(&mut fresh, &ctx, changes).unwrap();
        }
        let whole = table(&mut fresh, &demands);
        assert!(
            kept == whole,
            "{}: kept passes differ from a fresh project: {:?}",
            step.what,
            first_line_difference(&kept, &whole)
        );
        // And the shared-identity run of P2, which merges no pass output.
        let mut shared = open(&scratch, 1, false);
        for changes in &history {
            ClosureBackend::update(&mut shared, &ctx, changes).unwrap();
        }
        let legacy = table_with(&mut shared, &demands, true);
        assert!(
            kept == legacy,
            "{}: kept passes differ from the shared-identity run: {:?}",
            step.what,
            first_line_difference(&kept, &legacy)
        );
    }
}

/// The read set: the file's forward closure and every global contributor's,
/// nothing else.
#[test]
fn a_read_set_holds_the_import_closure_and_the_global_scope() {
    let scratch = Scratch::new(&files());
    let mut sources: BTreeMap<String, String> = BTreeMap::new();
    for (name, text) in files() {
        if name.ends_with(".ts") {
            sources.insert(name.to_owned(), text.to_owned());
        }
    }
    let mut project = open(&scratch, 1, true);
    let _ = table(&mut project, &demands(&scratch, &sources));
    let read = |project: &Project, name: &str| -> Vec<String> {
        project
            .per_file_read_set_for_test(&clean(&scratch.path(name)))
            .into_iter()
            .filter(|path| !path.starts_with("bundled:"))
            .map(|path| path.rsplit('/').next().unwrap_or_default().to_owned())
            .collect()
    };
    assert_eq!(read(&project, "b.ts"), ["a.ts", "b.ts", "glob.ts"]);
    assert_eq!(read(&project, "e.ts"), ["c.ts", "e.ts", "glob.ts"]);
    assert_eq!(read(&project, "d.ts"), ["d.ts", "glob.ts"]);
}

/// The pool's passes merge in path order: whatever the schedule, its table
/// equals the sequential one, and both equal the shared-identity run.
#[test]
fn the_pool_answers_what_one_thread_answers() {
    let scratch = Scratch::new(&files());
    let sources: BTreeMap<String, String> = files()
        .into_iter()
        .filter(|(name, _)| name.ends_with(".ts"))
        .map(|(name, text)| (name.to_owned(), text.to_owned()))
        .collect();
    let demands = demands(&scratch, &sources);
    let mut sequential = open(&scratch, 1, false);
    let one = table(&mut sequential, &demands);
    for workers in [2, 4, 8] {
        let mut pooled = open(&scratch, workers, false);
        let many = table(&mut pooled, &demands);
        assert!(
            one == many,
            "{workers} workers: {:?}",
            first_line_difference(&one, &many)
        );
        assert_eq!(
            pooled.per_file_stats().reuse.workers,
            workers.min(sources.len())
        );
    }
    let mut shared = open(&scratch, 1, false);
    let legacy = table_with(&mut shared, &demands, true);
    assert!(
        one == legacy,
        "shared identity: {:?}",
        first_line_difference(&one, &legacy)
    );
}

/// The transport manifest names exactly the paths whose rows or source
/// changed: an edit nothing reads changes one path.
#[test]
fn the_transport_names_only_paths_whose_rows_changed() {
    let ctx = Context::new();
    let scratch = Scratch::new(&files());
    let mut sources: BTreeMap<String, String> = files()
        .into_iter()
        .filter(|(name, _)| name.ends_with(".ts"))
        .map(|(name, text)| (name.to_owned(), text.to_owned()))
        .collect();
    let mut closure = DemandClosure::new(Box::new(open(&scratch, 2, true)), None);
    let first = demands(&scratch, &sources);
    closure
        .demand_table_for_groups(&ctx, 1, &groups(&first), &paths(&first))
        .unwrap();
    let mut previous = closure.table().cloned().expect("the first table");
    let edits = [
        ("d.ts", D.replace("helper(G)", "helper(G + 1)")),
        // A comment moves every row of a.ts; b.ts reads a.ts, runs again,
        // and changes only where its rows hold a.ts's spans.
        ("a.ts", format!("// moved\n{A}")),
        ("c.ts", C.replace("= 1", "= 2")),
    ];
    for (generation, (name, text)) in (2..).zip(edits) {
        sources.insert(name.to_owned(), text.clone());
        closure
            .update(
                &ctx,
                &[FileChange {
                    path: scratch.path(name),
                    source: text.into_bytes(),
                    deleted: false,
                    version: generation,
                }],
            )
            .unwrap();
        let next = demands(&scratch, &sources);
        let table = closure
            .demand_table_for_groups(&ctx, generation, &groups(&next), &[])
            .unwrap();
        let manifest = table.transport.clone().expect("an exact transport");
        let mut named: Vec<String> = manifest.entity_paths.iter().cloned().collect();
        named.sort();
        // What actually differs from the previous table, path by path.
        let mut every: Vec<String> = previous
            .sources
            .iter()
            .chain(&table.sources)
            .map(|source| source.path.to_string())
            .collect();
        every.sort();
        every.dedup();
        let differs: Vec<String> = every
            .into_iter()
            .filter(|path| {
                let rows = |table: &typefacts::producer::ProducerTable| {
                    let typefacts::producer::EntityRows::Runs(runs) = &table.entities else {
                        panic!("entity rows are not per-file runs");
                    };
                    (
                        runs.iter()
                            .flat_map(|run| run.iter())
                            .filter(|entity| &*entity.location.path == path)
                            .cloned()
                            .collect::<Vec<_>>(),
                        table.files.iter().find(|file| &*file.path == path).cloned(),
                        table
                            .sources
                            .iter()
                            .find(|source| &*source.path == path)
                            .cloned(),
                    )
                };
                rows(&previous) != rows(&table)
            })
            .collect();
        assert_eq!(named, differs, "{name}");
        assert!(
            named.iter().any(|path| path.ends_with(name)),
            "{name}: {named:?}"
        );
        if name == "d.ts" {
            assert_eq!(named.len(), 1, "{named:?}");
        }
        previous = table;
    }
}

/// ADR 0389's inexact cases fail closed on the producer side (the reactive
/// IR binds a file's own facts by this delta alone since #97). The first
/// table, a restarted merge (a file demanded for the first time) and a
/// rebuilt reference index (a file leaving the program) do not name their
/// changed `symbols` answers, so the client closes its symbols in full; the
/// sequential run (a key collision's) keeps no per-path versions either, so
/// its next table names every demanded path.
#[test]
fn every_inexact_table_names_no_symbol_changes_and_the_sequential_run_every_path() {
    let ctx = Context::new();
    let scratch = Scratch::new(&files());
    let mut sources: BTreeMap<String, String> = files()
        .into_iter()
        .filter(|(name, _)| name.ends_with(".ts") && *name != "e.ts")
        .map(|(name, text)| (name.to_owned(), text.to_owned()))
        .collect();
    let exact = |closure: &mut DemandClosure| closure.changed_references(&ctx).unwrap().1;
    let edit = |closure: &mut DemandClosure, name: &str, text: Option<&str>, version: u64| {
        closure
            .update(
                &ctx,
                &[FileChange {
                    path: scratch.path(name),
                    source: text.unwrap_or_default().as_bytes().to_vec(),
                    deleted: text.is_none(),
                    version,
                }],
            )
            .unwrap();
    };

    let mut closure = DemandClosure::new(Box::new(open(&scratch, 2, true)), None);
    let first = demands(&scratch, &sources);
    closure
        .demand_table_for_groups(&ctx, 1, &groups(&first), &paths(&first))
        .unwrap();
    assert!(!exact(&mut closure), "the first table");
    // An exact edit, for contrast.
    edit(&mut closure, "c.ts", Some(&C.replace("= 1", "= 2")), 2);
    sources.insert("c.ts".to_owned(), C.replace("= 1", "= 2"));
    let next = demands(&scratch, &sources);
    closure
        .demand_table_for_groups(&ctx, 2, &groups(&next), &[])
        .unwrap();
    assert!(exact(&mut closure), "an edit with the same passes is exact");
    // e.ts demanded for the first time: its work changes, the merge starts over.
    sources.insert("e.ts".to_owned(), E.to_owned());
    let next = demands(&scratch, &sources);
    closure
        .demand_table_for_groups(&ctx, 2, &groups(&next), &paths(&next))
        .unwrap();
    assert!(!exact(&mut closure), "a restarted merge");
    // d.ts leaves the program: the reference index is rebuilt.
    edit(&mut closure, "d.ts", None, 3);
    sources.remove("d.ts");
    let next = demands(&scratch, &sources);
    closure
        .demand_table_for_groups(&ctx, 3, &groups(&next), &[])
        .unwrap();
    assert!(!exact(&mut closure), "a rebuilt reference index");

    // The sequential run: no versions, so the manifest names every path.
    let mut project = open(&scratch, 2, true);
    project.select_sequential_for_test();
    let mut closure = DemandClosure::new(Box::new(project), None);
    let all = demands(&scratch, &sources);
    closure
        .demand_table_for_groups(&ctx, 1, &groups(&all), &paths(&all))
        .unwrap();
    edit(&mut closure, "c.ts", Some(C), 2);
    sources.insert("c.ts".to_owned(), C.to_owned());
    let all = demands(&scratch, &sources);
    let table = closure
        .demand_table_for_groups(&ctx, 2, &groups(&all), &[])
        .unwrap();
    assert!(!exact(&mut closure), "the sequential run");
    let manifest = table.transport.expect("a manifest");
    let named: std::collections::BTreeSet<String> = manifest.entity_paths.into_iter().collect();
    for group in groups(&all) {
        assert!(
            named.contains(&group.path),
            "the sequential run's table must name {} ({named:?})",
            group.path
        );
    }
}

/// Every id a table's rows name (entity symbols, call targets) and each one's
/// `symbols` answer with references, as text.
fn answers(project: &mut Project, demands: &[EntityDemand]) -> BTreeMap<String, String> {
    let ctx = Context::new();
    let groups = groups(demands);
    let runs: Vec<SemanticDemandRun<'_>> = groups
        .iter()
        .map(|group| SemanticDemandRun {
            path: &group.path,
            demands: &group.demands,
            shared: None,
        })
        .collect();
    let (results, _) = project.per_file_demand_runs(&ctx, &runs, &[]).unwrap();
    let results = entity_rows(results, &runs);
    let mut ids: Vec<std::sync::Arc<str>> = results
        .iter()
        .flat_map(|result| result.iter())
        .flat_map(|entity| {
            std::iter::once(entity.symbol.clone())
                .chain(entity.resolved_call.iter().map(|call| call.target.clone()))
        })
        .filter(|id| !id.is_empty())
        .collect();
    ids.sort();
    ids.dedup();
    answers_of(project, &ids)
}

fn answers_of(project: &mut Project, ids: &[std::sync::Arc<str>]) -> BTreeMap<String, String> {
    let queries: Vec<SymbolQuery> = ids
        .iter()
        .map(|id| SymbolQuery {
            id: id.clone(),
            references: true,
            references_only: false,
        })
        .collect();
    project
        .per_file_symbol_evidence(&Context::new(), &queries)
        .unwrap()
        .into_iter()
        .map(|fact| (fact.id.to_string(), format!("{fact:?}")))
        .collect()
}

/// Instant-edits P4: after a table the merged passes name every id whose
/// `symbols` answer (row or references) may differ from the previous table's,
/// so a client patches its closure instead of asking every row again. Each
/// edit's named set must hold every previously answered id whose answer
/// moved; a table that cannot say (a file entering or leaving the program
/// rebuilds the reference index) names nothing.
#[test]
fn the_merged_passes_name_every_changed_symbol_answer() {
    let ctx = Context::new();
    let scratch = Scratch::new(&files());
    let mut sources: BTreeMap<String, String> = files()
        .into_iter()
        .filter(|(name, _)| name.ends_with(".ts"))
        .map(|(name, text)| (name.to_owned(), text.to_owned()))
        .collect();
    let mut project = open(&scratch, 2, true);
    let mut previous = answers(&mut project, &demands(&scratch, &sources));
    assert_eq!(project.per_file_symbol_changes(), None, "the first table");
    let edits: [(&str, Option<String>, bool); 6] = [
        ("a.ts", Some(A.replace("x + 1", "x + 2")), true),
        ("c.ts", Some(format!("// moved\n{C}")), true),
        ("b.ts", Some(B.replace("g.value", "g.value + 1")), true),
        ("f.ts", Some("export const ff = 1;\n".to_owned()), false),
        ("e.ts", Some(E.replace("ez = z", "ez = z + 1")), true),
        ("c.ts", Some(C.to_owned()), true),
    ];
    for (version, (name, text, exact)) in edits.into_iter().enumerate() {
        match &text {
            Some(text) => sources.insert(name.to_owned(), text.clone()),
            None => sources.remove(name),
        };
        let change = FileChange {
            path: scratch.path(name),
            source: text.clone().unwrap_or_default().into_bytes(),
            deleted: text.is_none(),
            version: version as u64 + 1,
        };
        ClosureBackend::update(&mut project, &ctx, &[change]).unwrap();
        let current = answers(&mut project, &demands(&scratch, &sources));
        let named = project.per_file_symbol_changes();
        assert_eq!(named.is_some(), exact, "{name}: exactness");
        if let Some(named) = named {
            let held: Vec<std::sync::Arc<str>> =
                previous.keys().map(|id| id.as_str().into()).collect();
            let now = answers_of(&mut project, &held);
            for (id, answer) in &previous {
                if now.get(id) != Some(answer) {
                    assert!(
                        named.iter().any(|named| named.as_ref() == id),
                        "{name}: {id} moved but is not named ({answer} -> {:?})",
                        now.get(id)
                    );
                }
            }
        }
        previous = current;
    }
}

/// An edit that changes imports rebuilds the program chain (tsrs cannot
/// replace a file whose imports changed), and the kept passes are checked
/// path by path: a pass runs again only if its read set holds a file whose
/// text, exports or resolutions moved, or one that left the program. Each
/// table equals a fresh project's and the sequential run's (and every reused
/// pass is recomputed and compared, `verify`).
#[test]
fn import_edits_rerun_only_the_passes_that_reach_the_edit() {
    const X: &str = "export const v = 1;\n";
    const Y: &str = "export const v = \"s\";\n";
    const W: &str = "import { m } from \"./mod\";\nexport const w = m;\n";
    const U: &str = "import { v } from \"./x\";\nexport const u = v;\n";
    let mut all = files();
    all.extend([
        ("x.ts", X),
        ("y.ts", Y),
        ("u.ts", U),
        ("w.ts", W),
        ("mod.ts", "export const m = 1;\n"),
        // Outside `include`: in the program only while something imports
        // them.
        ("mod/index.ts", "export const m = \"index\";\n"),
        ("lib/extra.ts", "export const extra = 5;\n"),
    ]);
    let ctx = Context::new();
    let scratch = Scratch::new(&all);
    let mut sources: BTreeMap<String, String> = all
        .iter()
        .filter(|(name, _)| name.ends_with(".ts") && !name.contains('/'))
        .map(|(name, text)| ((*name).to_owned(), (*text).to_owned()))
        .collect();
    let mut incremental = open(&scratch, 2, true);
    let _ = table(&mut incremental, &demands(&scratch, &sources));
    let steps = [
        Step {
            edits: vec![(
                "d.ts",
                Some(format!(
                    "import {{ extra }} from \"./lib/extra\";\n{}",
                    D.replace("helper(G)", "helper(G) + extra")
                )),
            )],
            // d.ts, and lib/extra.ts entering the program.
            ran: 2,
            what: "an import of a module new to the program",
        },
        Step {
            edits: vec![("u.ts", Some(U.replace("./x", "./y")))],
            // u.ts only: x.ts loses its reader, y.ts gains one.
            ran: 1,
            what: "a specifier changed to resolve elsewhere",
        },
        Step {
            edits: vec![("mod.ts", None)],
            // w.ts's text is the same, its `./mod` now resolves to
            // mod/index.ts (entering the program): its resolutions moved.
            ran: 2,
            what: "a resolution moved by a deleted file, its importer's text unchanged",
        },
        Step {
            edits: vec![("d.ts", Some(D.to_owned()))],
            // d.ts only; lib/extra.ts leaves the program.
            ran: 1,
            what: "an import removed",
        },
    ];
    let mut versions: BTreeMap<&str, u64> = BTreeMap::new();
    let mut history: Vec<Vec<FileChange>> = Vec::new();
    for step in &steps {
        let mut changes = Vec::new();
        for (name, text) in &step.edits {
            let version = versions.entry(name).or_default();
            *version += 1;
            match text {
                Some(text) => {
                    sources.insert((*name).to_owned(), text.clone());
                }
                None => {
                    sources.remove(*name);
                }
            }
            changes.push(FileChange {
                path: scratch.path(name),
                source: text.clone().unwrap_or_default().into_bytes(),
                deleted: text.is_none(),
                version: *version,
            });
        }
        ClosureBackend::update(&mut incremental, &ctx, &changes).unwrap();
        assert_eq!(
            incremental.last_update.kind,
            Some(crate::unsafe_boundary::UpdateKind::Rebuilt),
            "{}: the case needs a rebuilt chain",
            step.what
        );
        history.push(changes);
        let demands = demands(&scratch, &sources);
        let kept = table(&mut incremental, &demands);
        let stats = incremental.per_file_stats().reuse;
        assert_eq!(stats.mismatches, 0, "{}: {stats:?}", step.what);
        assert_eq!(stats.ran, step.ran, "{}: {stats:?}", step.what);
        assert!(
            stats.reused >= sources.len() - step.ran,
            "{}: {stats:?}",
            step.what
        );
        let mut fresh = open(&scratch, 1, false);
        for changes in &history {
            ClosureBackend::update(&mut fresh, &ctx, changes).unwrap();
        }
        let whole = table(&mut fresh, &demands);
        assert!(
            kept == whole,
            "{}: kept passes differ from a fresh project: {:?}",
            step.what,
            first_line_difference(&kept, &whole)
        );
        let mut shared = open(&scratch, 1, false);
        for changes in &history {
            ClosureBackend::update(&mut shared, &ctx, changes).unwrap();
        }
        let legacy = table_with(&mut shared, &demands, true);
        assert!(
            kept == legacy,
            "{}: kept passes differ from the shared-identity run: {:?}",
            step.what,
            first_line_difference(&kept, &legacy)
        );
    }
}
