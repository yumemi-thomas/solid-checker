//! A table's facts do not depend on the order demands, batches and files are
//! asked in, nor on what the checker computed before (ADR 0367, STABLE-IDS
//! 14.3): check-first, location-pure descriptors and tsrs's canonical history.
//! The program's root order is held fixed: global merge order is a program
//! input (mechanism 5), not checker history. Counter ids are compared by
//! first appearance in table order (`Renamer`); declaration-hash ids exactly.

use super::project_support::Scratch;
use super::randomized_incremental::{Renamer, describe};
use super::symbol_integration::{STRICT_CONFIG, groups, is_ident, open, paths};
use crate::backend::ClosureBackend;
use crate::closure::DemandClosure;
use crate::context::Context;
use crate::gocompat::{absolute, clean};
use crate::model::{SemanticDemandRun, SemanticScope};
use crate::project::Project;
use std::collections::{BTreeMap, HashSet};
use typefacts::Location;
use typefacts::v3::EntityDemand;

/// Shapes of the measured history mechanisms: nested generic calls whose inner
/// call is contextually typed by the outer one (mechanism 1), methods whose
/// `this` is assigned by contextual checking (2), one symbol with different
/// types at its references (first-wins descriptor sharing), and normalized
/// object-literal unions (the `undefinedProperties` cache). `warm.ts` is never
/// demanded; warm-up checks it.
const FILES: [(&str, &str); 6] = [
    ("tsconfig.json", STRICT_CONFIG),
    (
        "calls.ts",
        "declare function parse<T extends Date | number>(value: string, fallback?: T): T | Date;\n\
declare function range<A>(from: A, to: A, step?: (value: A) => A): A[];\n\
declare function pick<K extends string>(keys: K[]): Record<K, number>;\n\
export const dates = range(parse(\"a\"), parse(\"b\", 1), (value) => value);\n\
export const picked = pick([\"x\", \"y\"]).x;\n\
export const mapped = [1, 2].map((n) => range(n, n + 1)).flat();\n",
    ),
    (
        "plugin.ts",
        "interface Context { addWatchFile(file: string): void; meta: { id: string } }\n\
interface Plugin { load?(this: Context, id: string): string | void; name: string }\n\
export const plugin: Plugin = {\n  name: \"p\",\n  load(id) {\n    this.addWatchFile(id);\n    return this.meta.id;\n  },\n};\n\
export const plugins: Plugin[] = [{ name: \"q\", load(id) { this.addWatchFile(id); } }];\n",
    ),
    (
        "narrow.ts",
        "declare const v: string | number | Date;\n\
export const s = typeof v === \"string\" ? v.length : 0;\n\
export const d = v instanceof Date ? v.getTime() : v;\n\
export const w = v;\n",
    ),
    (
        "literals.ts",
        "export const a = [{ groups: \"g\" }, { cols: \"x\" }];\n\
export const b = [{ nodes: \"r\" }, { cols: \"x\" }];\n\
export const c = a.concat(b.map((item) => ({ groups: \"h\", ...item })));\n",
    ),
    (
        "warm.ts",
        "export const early = [{ cols: 1 }, { nodes: 2 }, { groups: 3 }];\n\
import { plugin } from \"./plugin\";\n\
export const loaded = plugin.load?.call({ addWatchFile() {}, meta: { id: \"\" } }, \"x\");\n",
    ),
];

const DEMANDED: [&str; 4] = ["calls.ts", "literals.ts", "narrow.ts", "plugin.ts"];

/// Every identifier (and `this`) of the demanded files, with every family a
/// rule may ask for at an expression.
fn all_demands(scratch: &Scratch) -> Vec<EntityDemand> {
    let mut demands = Vec::new();
    for name in DEMANDED {
        let source = FILES.iter().find(|(file, _)| *file == name).unwrap().1;
        let path = clean(&scratch.path(name));
        let bytes = source.as_bytes();
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
                references: true,
                type_descriptor: true,
                resolved_call: true,
                callability: true,
                constructability: true,
                runtime_value_domain: true,
                primitive_value_domain: true,
                primitive_literal_candidates: true,
                reference_space: true,
                runtime_identity: true,
                library_types: true,
                array_shape: true,
                ..EntityDemand::default()
            });
        }
    }
    demands
}

fn shuffled<T: Clone>(items: &[T], seed: u64) -> Vec<T> {
    let mut state = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
    let mut items = items.to_vec();
    for index in (1..items.len()).rev() {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        items.swap(index, (state % (index as u64 + 1)) as usize);
    }
    items
}

fn text(table: &typefacts::producer::ProducerTable) -> String {
    Renamer(Default::default()).apply(&describe(table).join("\n"))
}

fn first_difference(left: &str, right: &str) -> String {
    let Some((l, r)) = left.lines().zip(right.lines()).find(|(l, r)| l != r) else {
        return "row counts differ".to_owned();
    };
    let at = l
        .bytes()
        .zip(r.bytes())
        .position(|(a, b)| a != b)
        .unwrap_or(l.len().min(r.len()));
    let from = l[..at].rfind("Location {").unwrap_or(0);
    let window = |line: &str| line[at.saturating_sub(160)..(at + 160).min(line.len())].to_owned();
    format!(
        "\n  at:      {}\n  base:    {}\n  variant: {}",
        &l[from..(from + 120).min(l.len())],
        window(l),
        window(r)
    )
}

/// One table over every demand, on a fresh program.
fn baseline(scratch: &Scratch) -> String {
    let all = all_demands(scratch);
    let mut closure = open_closure(scratch, |_| {});
    text(
        &closure
            .demand_table_for_groups(&Context::new(), 1, &groups(&all), &[])
            .unwrap(),
    )
}

fn open_closure(scratch: &Scratch, warm: impl FnOnce(&mut Project)) -> DemandClosure {
    let mut project = open(&scratch.config());
    warm(&mut project);
    DemandClosure::new(Box::new(project), None)
}

/// Batch and file order: earlier tables of the generation answer one file at
/// a time, in several file orders, before the table over every demand. Each
/// earlier table leaves the checker with a different history.
#[test]
fn facts_do_not_depend_on_earlier_tables_or_their_file_order() {
    let ctx = Context::new();
    let scratch = Scratch::new(&FILES);
    let base = baseline(&scratch);
    let all = all_demands(&scratch);
    let every_path = paths(&all);
    let forward: Vec<&str> = DEMANDED.to_vec();
    let mut orders = vec![forward.iter().rev().copied().collect::<Vec<_>>(), forward];
    orders.extend((1..=3).map(|seed| shuffled(&DEMANDED, seed)));
    for order in &orders {
        let mut closure = open_closure(&scratch, |_| {});
        for name in order {
            let path = clean(&scratch.path(name));
            let one: Vec<EntityDemand> = all
                .iter()
                .filter(|demand| *demand.location.path == *path)
                .cloned()
                .collect();
            closure
                .demand_table_for_groups(&ctx, 1, &groups(&one), &every_path)
                .unwrap();
        }
        let got = text(
            &closure
                .demand_table_for_groups(&ctx, 1, &groups(&all), &every_path)
                .unwrap(),
        );
        assert!(
            got == base,
            "file order {order:?}: {}",
            first_difference(&base, &got)
        );
    }
}

/// Demand order inside one batch: the runs and the demands of each run in
/// reverse and shuffled orders, answered after the check-first pass a table
/// makes, give the canonical batch's facts at every location.
#[test]
fn facts_do_not_depend_on_demand_order_inside_a_batch() {
    let ctx = Context::new();
    let scratch = Scratch::new(&FILES);
    let all = all_demands(&scratch);
    let canonical: Vec<(String, Vec<EntityDemand>)> = groups(&all)
        .into_iter()
        .map(|group| (group.path, group.demands))
        .collect();
    let answer = |runs: &[(String, Vec<EntityDemand>)]| -> String {
        let mut project = open(&scratch.config());
        let mut sorted: Vec<&str> = runs.iter().map(|(path, _)| path.as_str()).collect();
        sorted.sort_unstable();
        project.check_demanded_files(&ctx, &sorted).unwrap();
        let suppression = HashSet::new();
        let results = project
            .semantic_demand_runs(
                &ctx,
                &runs
                    .iter()
                    .map(|(path, demands)| SemanticDemandRun { path, demands })
                    .collect::<Vec<_>>(),
                &SemanticScope {
                    suppression: &suppression,
                },
            )
            .unwrap();
        // Back to canonical order before counters are renamed.
        let mut by_location: BTreeMap<(String, u64, u64), String> = BTreeMap::new();
        for result in results {
            for entity in result.entities {
                let key = (
                    entity.location.path.to_string(),
                    entity.location.start_byte,
                    entity.location.end_byte,
                );
                by_location.insert(key, format!("{entity:?}"));
            }
        }
        let lines: Vec<String> = by_location.into_values().collect();
        Renamer(Default::default()).apply(&lines.join("\n"))
    };
    let base = answer(&canonical);
    let reversed: Vec<(String, Vec<EntityDemand>)> = canonical
        .iter()
        .rev()
        .map(|(path, demands)| (path.clone(), demands.iter().rev().cloned().collect()))
        .collect();
    let mut variants = vec![reversed];
    for seed in 1..=4 {
        variants.push(
            shuffled(&canonical, seed)
                .into_iter()
                .map(|(path, demands)| (path, shuffled(&demands, seed + 10)))
                .collect(),
        );
    }
    for (index, variant) in variants.iter().enumerate() {
        let got = answer(variant);
        assert!(
            got == base,
            "demand order {index}: {}",
            first_difference(&base, &got)
        );
    }
}

/// Warm-up: before the table, the checker checks a shuffled subset of files
/// (including one never demanded) and resolves types at shuffled positions.
#[test]
fn facts_do_not_depend_on_checker_warm_up() {
    let ctx = Context::new();
    let scratch = Scratch::new(&FILES);
    let base = baseline(&scratch);
    let all = all_demands(&scratch);
    let mut names: Vec<&str> = DEMANDED.to_vec();
    names.push("warm.ts");
    for seed in 1..=4u64 {
        let files = shuffled(&names, seed);
        let positions = shuffled(&all, seed + 100);
        let mut closure = open_closure(&scratch, |project| {
            let generation = project.live().unwrap();
            let program = generation.program();
            for name in files.iter().take(1 + seed as usize % files.len()) {
                let file = program
                    .get_source_file(&absolute(&scratch.path(name)).unwrap())
                    .unwrap();
                let _ = generation.semantic_diagnostics(file);
            }
            project
                .with_env(|env, _| {
                    for demand in positions.iter().take(positions.len() / 2) {
                        let file = env
                            .program
                            .get_source_file(&absolute(&demand.location.path).unwrap())
                            .unwrap();
                        let node = crate::nodes::node_at(file, demand.location.start_byte as u32);
                        let _ = env.checker.get_type_at_location(node);
                    }
                })
                .unwrap();
        });
        let got = text(
            &closure
                .demand_table_for_groups(&ctx, 1, &groups(&all), &[])
                .unwrap(),
        );
        assert!(
            got == base,
            "warm-up seed {seed} ({files:?}): {}",
            first_difference(&base, &got)
        );
    }
}

/// The same table built again on the same program, its checker now warm,
/// equals the first: what a table answers is canonical, not a first-run
/// artefact.
#[test]
fn a_repeated_table_equals_the_first() {
    let ctx = Context::new();
    let scratch = Scratch::new(&FILES);
    let all = all_demands(&scratch);
    let every_path = paths(&all);
    let mut closure = open_closure(&scratch, |_| {});
    let first = text(
        &closure
            .demand_table_for_groups(&ctx, 1, &groups(&all), &[])
            .unwrap(),
    );
    for _ in 0..2 {
        let again = text(
            &closure
                .demand_table_for_groups(&ctx, 1, &groups(&all), &every_path)
                .unwrap(),
        );
        assert!(
            again == first,
            "repeat: {}",
            first_difference(&first, &again)
        );
    }
    assert_eq!(first, baseline(&scratch));
}
