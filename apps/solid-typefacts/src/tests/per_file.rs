//! Per-file checkers (`per_file`, `unsafe_boundary` invariant 8): the file
//! checker's lifecycle, and the facts of a small multi-file project: the
//! pooled passes against the sequential shared-identity run, and the
//! `symbols` answers before and after a release.

use super::project_support::{BUNDLER_CONFIG, Scratch};
use super::symbol_integration::{groups, paths, symbol_demands};
use crate::closure::DemandClosure;
use crate::context::Context;
use crate::project::Project;
use typefacts::EntityFact;
use typefacts::producer::{EntityRows, ProducerTable};
use typefacts::v3::SymbolQuery;

const A: &str = "export interface Shape { size: number }
export function area(shape: Shape): number { return shape.size * 2; }
export class Box { width = 1; grow(by: number) { this.width += by; return this; } }
export const make = <T,>(value: T) => ({ value, twice: [value, value] as const });
";

const B: &str = "import { area, Box, make, type Shape } from \"./a\";
const shape: Shape = { size: 3 };
const result = area(shape);
const box = new Box().grow(2);
const made = make(5);
const parsed = parseInt(\"42\");
const checked = isNaN(parsed);
const keys = Object.keys(made);
export { result, box, made, parsed, checked, keys };
";

const C: &str = "import { box, made } from \"./b\";
// @ts-ignore
parseInt = (() => 0) as any;
export const width = box.width + made.value;
export const pair = made.twice;
";

fn fixture() -> Scratch {
    Scratch::new(&[
        ("tsconfig.json", BUNDLER_CONFIG),
        ("a.ts", A),
        ("b.ts", B),
        ("c.ts", C),
    ])
}

fn open(scratch: &Scratch) -> Project {
    scratch.open()
}

/// Whether `text` holds a generation-scoped counter id (`symbol:<g>:<n>`).
fn has_counter(text: &str) -> bool {
    text.match_indices("symbol:").any(|(at, _)| {
        text[at + "symbol:".len()..]
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_digit())
    })
}

fn entity_rows(table: &ProducerTable) -> Vec<EntityFact> {
    let EntityRows::Runs(runs) = &table.entities else {
        panic!("entity rows are not per-file runs");
    };
    runs.iter().flat_map(|run| run.iter().cloned()).collect()
}

fn analyze(project: Project) -> (DemandClosure, ProducerTable) {
    let mut closure = DemandClosure::new(Box::new(project), None);
    let mut demands = symbol_demands(&mut closure);
    for demand in &mut demands {
        demand.reference_space = true;
        demand.callability = true;
    }
    let table = closure
        .demand_table_for_groups(&Context::new(), 1, &groups(&demands), &paths(&demands))
        .unwrap();
    (closure, table)
}

/// Every id the table's rows name, in row order, once.
fn table_ids(rows: &[EntityFact]) -> Vec<std::sync::Arc<str>> {
    let mut seen = std::collections::HashSet::new();
    let mut ids = Vec::new();
    for row in rows {
        let mut push = |id: &std::sync::Arc<str>| {
            if !id.is_empty() && seen.insert(id.clone()) {
                ids.push(id.clone());
            }
        };
        push(&row.symbol);
        if let Some(call) = &row.resolved_call {
            push(&call.target);
        }
    }
    ids
}

fn evidence(closure: &mut DemandClosure, ids: &[std::sync::Arc<str>]) -> String {
    let queries: Vec<SymbolQuery> = ids
        .iter()
        .map(|id| SymbolQuery {
            id: id.clone(),
            references: true,
            references_only: false,
        })
        .collect();
    let facts = closure
        .backend_mut()
        .symbol_evidence(&Context::new(), &queries)
        .unwrap();
    format!("{facts:#?}")
}

#[test]
fn a_file_checker_is_the_threads_checker_and_the_program_checker_is_never_made() {
    let scratch = fixture();
    let mut project = scratch.open();
    let generation = project.current_mut_for_test();
    assert!(generation.release_checker());
    assert!(!generation.has_checker());
    let first = {
        let scope = generation.file_checker();
        assert!(generation.in_file_checker());
        let address = generation.with_checker(|checker| std::ptr::from_mut(&mut **checker).addr());
        // A suspended lease reacquires the same (file) checker.
        let file = generation.program().source_files()[0];
        let _ = generation.semantic_diagnostics(file);
        let again = generation.with_checker(|checker| std::ptr::from_mut(&mut **checker).addr());
        assert_eq!(address, again);
        drop(scope);
        address
    };
    assert!(!generation.in_file_checker());
    assert!(
        !generation.has_checker(),
        "the program's single checker was made inside a file checker"
    );
    // Outside a file checker the program's single checker answers again.
    let single = generation.with_checker(|checker| std::ptr::from_mut(&mut **checker).addr());
    assert!(generation.has_checker());
    let _ = (first, single);
}

#[test]
#[should_panic(expected = "do not nest")]
fn file_checkers_do_not_nest() {
    let scratch = fixture();
    let project = scratch.open();
    let generation = project.current().unwrap();
    let _outer = generation.file_checker();
    let _inner = generation.file_checker();
}

/// The pool's passes (an identity state each) and the sequential run (one
/// identity state, path order: the key-collision answer) give one table and
/// one set of `symbols` answers: an id is a function of the program, never
/// of which pass minted it first. No counter id reaches either.
#[test]
fn pooled_and_sequential_passes_answer_the_same_facts() {
    let scratch = fixture();
    let (mut pooled, pooled_table) = analyze(open(&scratch));
    let mut sequential_project = open(&scratch);
    sequential_project.select_sequential_for_test();
    let (mut sequential, sequential_table) = analyze(sequential_project);
    let pooled_rows = entity_rows(&pooled_table);
    let sequential_rows = entity_rows(&sequential_table);
    assert!(!pooled_rows.is_empty());
    let text = format!("{pooled_rows:#?}");
    assert!(!has_counter(&text), "a counter id: {text}");
    assert_eq!(text, format!("{sequential_rows:#?}"));
    // The cross-file census withdrew `parseInt`'s binding (c.ts writes it)
    // and kept `isNaN`'s.
    let binding = |rows: &[EntityFact], name: &str| {
        let at = B.find(name).unwrap() as u64;
        rows.iter()
            .find(|row| row.location.path.ends_with("/b.ts") && row.location.start_byte == at)
            .and_then(|row| row.resolved_call.as_ref())
            .map(|call| call.default_library_binding.is_some())
    };
    assert_eq!(binding(&pooled_rows, "parseInt(\""), Some(false));
    assert_eq!(binding(&pooled_rows, "isNaN("), Some(true));

    // `symbols` answers, references included, from the precomputed rows.
    let ids = table_ids(&pooled_rows);
    let answers = evidence(&mut pooled, &ids);
    assert_eq!(answers, evidence(&mut sequential, &ids));

    // After a release every id is asked in a scratch pass: a binder id
    // re-resolves through its key and answers the same; a derived id no
    // position names fails closed to an id-only row, never to another
    // symbol's answer.
    pooled.release_backend_analysis_state();
    for id in &ids {
        let query = SymbolQuery {
            id: id.clone(),
            references: true,
            references_only: false,
        };
        let before = &sequential
            .backend_mut()
            .symbol_evidence(&Context::new(), std::slice::from_ref(&query))
            .unwrap()[0];
        let after = &pooled
            .backend_mut()
            .symbol_evidence(&Context::new(), std::slice::from_ref(&query))
            .unwrap()[0];
        assert!(
            after == before || (after.declarations.is_empty() && after.references.is_empty()),
            "{id}: {after:?} vs {before:?}"
        );
    }
}

#[test]
fn per_file_runs_never_make_the_program_checker_and_purge_every_handle() {
    let scratch = fixture();
    let mut project = open(&scratch);
    assert!(project.current_mut_for_test().release_checker());
    let path = scratch.path("b.ts");
    let demands: Vec<_> = ["area(", "make(", "parseInt("]
        .iter()
        .map(|needle| typefacts::v3::EntityDemand {
            location: super::project_support::find(&path, B, needle),
            symbol: true,
            resolved_call: true,
            type_descriptor: true,
            reference_space: true,
            ..typefacts::v3::EntityDemand::default()
        })
        .collect();
    let run = crate::model::SemanticDemandRun {
        path: &path,
        demands: &demands,
    };
    let (results, _) = project
        .per_file_demand_runs(&Context::new(), &[run], &[])
        .unwrap();
    assert_eq!(results[0].entities.len(), 3);
    assert!(!project.current().unwrap().has_checker());
    assert!(
        !project.identities().holds_symbols(),
        "a handle into a freed file checker survived its pass"
    );
    let stats = project.per_file_stats();
    assert_eq!(stats.demanded_passes, 1);
    // b.ts in its own pass; a.ts and c.ts for their reference contributions.
    assert_eq!(stats.reference_passes, 2);
    assert!(stats.passes >= 3);
}

/// The program fills its include processor's diagnostics lazily, wherever the
/// first `get_semantic_diagnostics` allocates (rc17 corpus, kui: a read of
/// freed memory in the second pass). They are primed outside every file
/// checker, so no pass's region escapes and every pass's checker is freed.
#[test]
fn program_caches_never_land_in_a_file_checkers_region() {
    let a = "/// <reference path=\"./missing.ts\" />\nexport const one = 1;\n";
    let b = "import { one } from \"./a\";\nexport const two = one + 1;\n";
    let scratch = Scratch::new(&[
        ("tsconfig.json", super::project_support::SMALL_CONFIG),
        ("a.ts", a),
        ("b.ts", b),
    ]);
    let mut project = open(&scratch);
    let leaked = crate::unsafe_boundary::leaked_file_checkers();
    let (a_path, b_path) = (scratch.path("a.ts"), scratch.path("b.ts"));
    let demand = |path: &str, source: &str, needle: &str| typefacts::v3::EntityDemand {
        location: super::project_support::find(path, source, needle),
        symbol: true,
        type_descriptor: true,
        ..typefacts::v3::EntityDemand::default()
    };
    let a_demands = vec![demand(&a_path, a, "one")];
    let b_demands = vec![demand(&b_path, b, "two")];
    let runs = [
        crate::model::SemanticDemandRun {
            path: &a_path,
            demands: &a_demands,
        },
        crate::model::SemanticDemandRun {
            path: &b_path,
            demands: &b_demands,
        },
    ];
    let (results, _) = project
        .per_file_demand_runs(&Context::new(), &runs, &[])
        .unwrap();
    assert_eq!(results.len(), 2);
    // The case proves something only with a processing diagnostic (TS6053,
    // the missing referenced file): asked after the passes.
    let generation = project.current().unwrap();
    let file = generation.program().get_source_file(&a_path).unwrap();
    assert!(
        generation
            .semantic_diagnostics(file)
            .iter()
            .any(|diagnostic| diagnostic.code == 6053),
        "the case needs a processing diagnostic"
    );
    assert_eq!(
        crate::unsafe_boundary::leaked_file_checkers(),
        leaked,
        "a file checker's region escaped into a program cache"
    );
}

/// The same cache filled by the program's single checker (a program-level
/// question outside any file checker): its first `get_semantic_diagnostics`
/// must not land in that checker's region, which `release_checker` frees
/// while the program (and the cache) stays.
#[test]
fn program_caches_survive_a_program_checker_release() {
    let a = "/// <reference path=\"./missing.ts\" />\nexport const one = 1;\n";
    let scratch = Scratch::new(&[
        ("tsconfig.json", super::project_support::SMALL_CONFIG),
        ("a.ts", a),
    ]);
    let mut project = open(&scratch);
    let leaked = crate::unsafe_boundary::leaked_file_checkers();
    let a_path = scratch.path("a.ts");
    let has_6053 = |project: &Project| {
        let generation = project.current().unwrap();
        let file = generation.program().get_source_file(&a_path).unwrap();
        generation
            .semantic_diagnostics(file)
            .iter()
            .any(|diagnostic| diagnostic.code == 6053)
    };
    assert!(has_6053(&project), "the case needs a processing diagnostic");
    project.release_analysis_state_for_test();
    assert!(
        has_6053(&project),
        "the cache is read again after the release"
    );
    assert_eq!(
        crate::unsafe_boundary::leaked_file_checkers(),
        leaked,
        "the slot checker's region escaped into a program cache"
    );
}
