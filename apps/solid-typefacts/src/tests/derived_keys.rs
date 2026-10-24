//! Exact derived-symbol keys (`derived_key.rs`, instant-edits P2): with
//! `Identities::derived_keys`, a derived symbol's id is a function of what it
//! is derived from and its exact type context, the same in every checker and
//! every pass, never shared by two symbols of different types; a symbol with
//! no exact key carries no id at all (no counter).

use super::project_support::Scratch;
use super::symbol_integration::{STRICT_CONFIG, groups, open};
use crate::closure::DemandClosure;
use crate::context::Context;
use crate::gocompat::{absolute, clean};
use crate::project::Project;
use std::collections::BTreeMap;
use std::sync::Arc;
use typefacts::producer::{EntityRows, ProducerTable};
use typefacts::v3::EntityDemand;
use typefacts::{EntityFact, Location, SymbolClaim};

const LIB: &str = "export interface Box<T> {\n  value: T;\n  get size(): number;\n  set size(next: number);\n}\n\
export interface Dict { [key: string]: number }\n\
export class Base<T> {\n  item!: T;\n  get label(): string { return \"\"; }\n  set label(next: string) {}\n}\n";

const A: &str = "import type { Box, Dict } from \"./lib\";\nimport { Base } from \"./lib\";\n\
declare const n: Box<number>;\ndeclare const s: Box<string>;\ndeclare const d: Dict;\ndeclare const b: Base<boolean>;\n\
export const nv = n.value;\nexport const sv = s.value;\nexport const ns = n.size;\nexport const dv = d.anything;\nexport const bl = b.label;\n\
export const meta = import.meta;\n";

const B: &str = "import type { Box } from \"./lib\";\nimport { Base } from \"./lib\";\n\
declare const m: Box<number>;\ndeclare const c: Base<boolean>;\n\
export const mv = m.value;\nexport const cl = c.label;\nexport const other = import.meta;\n";

fn fixture() -> Scratch {
    Scratch::new(&[
        ("tsconfig.json", STRICT_CONFIG),
        ("lib.ts", LIB),
        ("a.ts", A),
        ("b.ts", B),
    ])
}

/// Named demand sites: (name, file, text before the identifier, identifier).
const SITES: [(&str, &str, &str, &str); 11] = [
    ("nv", "a.ts", "nv = n.", "value"),
    ("sv", "a.ts", "sv = s.", "value"),
    ("ns", "a.ts", "ns = n.", "size"),
    ("dv", "a.ts", "dv = d.", "anything"),
    ("bl", "a.ts", "bl = b.", "label"),
    ("meta", "a.ts", "meta = import.", "meta"),
    ("mv", "b.ts", "mv = m.", "value"),
    ("cl", "b.ts", "cl = c.", "label"),
    ("other", "b.ts", "other = import.", "meta"),
    ("size-declaration", "lib.ts", "get ", "size"),
    ("label-declaration", "lib.ts", "get ", "label"),
];

fn source(name: &str) -> &'static str {
    match name {
        "lib.ts" => LIB,
        "a.ts" => A,
        "b.ts" => B,
        _ => unreachable!(),
    }
}

fn demands(scratch: &Scratch) -> Vec<(&'static str, EntityDemand)> {
    SITES
        .iter()
        .map(|(name, file, before, identifier)| {
            let text = source(file);
            let at = text.find(before).unwrap_or_else(|| panic!("{before}"));
            let start = if *file == "lib.ts" {
                // The second `get ` names `label`.
                let mut at = at;
                while !text[at + before.len()..].starts_with(identifier) {
                    at = at + 1 + text[at + 1..].find(before).unwrap();
                }
                at + before.len()
            } else {
                at + before.len()
            };
            assert!(text[start..].starts_with(identifier), "{name}");
            let demand = EntityDemand {
                location: Location {
                    path: clean(&scratch.path(file)).into(),
                    start_byte: start as u64,
                    end_byte: (start + identifier.len()) as u64,
                },
                symbol: true,
                references: true,
                resolved_call: true,
                type_descriptor: true,
                reference_space: true,
                ..EntityDemand::default()
            };
            (*name, demand)
        })
        .collect()
}

fn rows(table: &ProducerTable) -> Vec<EntityFact> {
    let EntityRows::Runs(runs) = &table.entities else {
        panic!("entity rows are not per-file runs");
    };
    runs.iter().flat_map(|run| run.iter().cloned()).collect()
}

/// Each site's entity, by name.
fn answer(
    scratch: &Scratch,
    per_file: bool,
    warm: impl FnOnce(&mut Project),
) -> BTreeMap<&'static str, EntityFact> {
    let mut project = open(&scratch.config());
    project.select_derived_keys_for_test();
    if per_file {
        project.select_per_file_checkers_for_test();
    }
    warm(&mut project);
    let named = demands(scratch);
    let all: Vec<EntityDemand> = named.iter().map(|(_, demand)| demand.clone()).collect();
    let mut closure = DemandClosure::new(Box::new(project), None);
    let table = closure
        .demand_table_for_groups(&Context::new(), 1, &groups(&all), &[])
        .unwrap();
    let rows = rows(&table);
    named
        .into_iter()
        .map(|(name, demand)| {
            let row = rows
                .iter()
                .find(|row| row.location == demand.location)
                .unwrap_or_else(|| panic!("no row for {name}"))
                .clone();
            (name, row)
        })
        .collect()
}

fn id<'a>(facts: &'a BTreeMap<&'static str, EntityFact>, name: &str) -> &'a Arc<str> {
    &facts[name].symbol
}

fn assert_keyed(facts: &BTreeMap<&'static str, EntityFact>, name: &str) {
    let id = id(facts, name);
    assert!(id.starts_with("symbol:h:"), "{name}: {id:?}");
}

#[test]
fn instantiations_keep_their_type_context_and_share_it_across_files() {
    let scratch = fixture();
    for per_file in [false, true] {
        let facts = answer(&scratch, per_file, |_| {});
        for name in ["nv", "sv", "mv", "bl", "cl", "ns", "meta", "other"] {
            assert_keyed(&facts, name);
        }
        // `Box<number>.value` and `Box<string>.value` are two symbols.
        assert_ne!(id(&facts, "nv"), id(&facts, "sv"), "per-file {per_file}");
        // One instantiation reached from two files is one symbol.
        assert_eq!(id(&facts, "nv"), id(&facts, "mv"), "per-file {per_file}");
        assert_eq!(id(&facts, "bl"), id(&facts, "cl"), "per-file {per_file}");
        // `import.meta`'s `meta` is a checker singleton.
        assert_eq!(
            id(&facts, "meta"),
            id(&facts, "other"),
            "per-file {per_file}"
        );
    }
}

#[test]
fn an_accessor_instantiation_takes_its_root_id_when_no_type_variable_reaches_it() {
    let scratch = fixture();
    for per_file in [false, true] {
        let facts = answer(&scratch, per_file, |_| {});
        // `size` and `label` read and write a type with no type variable:
        // the checker answers their instantiations with the root itself once
        // both types are resolved, so the id is the declaration's.
        assert_keyed(&facts, "size-declaration");
        assert_eq!(id(&facts, "ns"), id(&facts, "size-declaration"));
        assert_eq!(id(&facts, "bl"), id(&facts, "label-declaration"));
    }
}

#[test]
fn an_index_signature_symbol_has_no_id() {
    let scratch = fixture();
    for per_file in [false, true] {
        let facts = answer(&scratch, per_file, |_| {});
        let row = &facts["dv"];
        assert!(
            row.symbol.is_empty(),
            "per-file {per_file}: {:?}",
            row.symbol
        );
        assert_eq!(row.symbol_claim, SymbolClaim::Unresolved);
    }
}

#[test]
fn ids_do_not_depend_on_the_mode_or_the_checker_history() {
    let scratch = fixture();
    let base = answer(&scratch, false, |_| {});
    let text = |facts: &BTreeMap<&'static str, EntityFact>| format!("{facts:?}");
    assert!(!text(&base).contains("symbol:1:"), "a counter id: {base:?}");
    let per_file = answer(&scratch, true, |_| {});
    assert_eq!(text(&base), text(&per_file));
    // Warm the canonical checker on the other file, and resolve the types at
    // the sites in reverse, before the table.
    let warmed = answer(&scratch, false, |project| {
        let generation = project.live().unwrap();
        let program = generation.program();
        let file = program
            .get_source_file(&absolute(&scratch.path("b.ts")).unwrap())
            .unwrap();
        let _ = generation.semantic_diagnostics(file);
        let sites = demands(&scratch);
        project
            .with_env(|env, _| {
                for (_, demand) in sites.iter().rev() {
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
    assert_eq!(text(&base), text(&warmed));
}
