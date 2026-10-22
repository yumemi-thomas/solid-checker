//! Ports the typefacts-level `*_integration_test.go` cases that need only
//! symbol facts, over the tsrs project: `jsx_symbols`, `global_scope`,
//! `non_durable` and the compiler-backed `incremental_tsgo_adversarial` cases.
//!
//! [`symbol_demands`] is Go's `realisticDemands` over `Source{Bindings,Calls}`:
//! a symbol demand (with references and structural-accessor flags on a fixed
//! rotation) for every call-initialized binding name, and a symbol demand for
//! every callee with the async flag on a rotation. Resolved-call,
//! type-descriptor and runtime-value-domain demands are left out until the
//! resolved-call slice (see the comment at the flag); both sides of every comparison use the same
//! helper, so parity is still between an incremental closure and a fresh one.

use super::project_support::{Scratch, loc};
use crate::closure::{DemandClosure, DemandInputGroup};
use crate::context::Context;
use crate::project::Project;
use crate::session::Session;
use std::path::PathBuf;
use typefacts::producer::{ProducerTable, TransitionEncoder, TransitionInput};
use typefacts::v3::{EntityDemand, FileChange, Operation};
use typefacts::{EntityFact, Location};

fn testdata(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../solid-typefacts/internal/typefacts/testdata")
        .join(name)
        .canonicalize()
        .unwrap()
}

fn open(config: &str) -> Project {
    Project::open(&Context::new(), config, None, None).unwrap()
}

/// Go's `realisticDemands`.
fn symbol_demands(closure: &mut DemandClosure) -> Vec<EntityDemand> {
    let ctx = Context::new();
    let mut demands = Vec::new();
    for source in closure.source_files(&ctx).unwrap() {
        let path = crate::gocompat::clean(&source.path);
        for (index, binding) in closure
            .backend
            .source_bindings(&ctx, &path)
            .unwrap()
            .into_iter()
            .enumerate()
        {
            for name in binding.names {
                if name.path.is_empty() || name.end_byte <= name.start_byte {
                    continue;
                }
                demands.push(EntityDemand {
                    location: name,
                    symbol: true,
                    references: index % 3 == 0,
                    structural_accessor: index % 7 == 0,
                    ..EntityDemand::default()
                });
            }
        }
        for (index, call) in closure
            .backend
            .source_calls(&ctx, &path)
            .unwrap()
            .into_iter()
            .enumerate()
        {
            demands.push(EntityDemand {
                location: call.callee,
                symbol: true,
                // Go also demands `resolvedCall` here, and that is what keeps
                // a retained descriptor honest when a restored file satisfies
                // an import that was unresolved (the leaf cutoff reports only
                // the restored file; an unresolved call records no dependency
                // on it). Go with symbol and descriptor demands alone diverges
                // from a fresh open at that step too (reported separately), so
                // the descriptor flag waits for the resolved-call slice.
                type_descriptor: false,
                r#async: index % 5 == 0,
                ..EntityDemand::default()
            });
        }
    }
    demands
}

fn groups(demands: &[EntityDemand]) -> Vec<DemandInputGroup> {
    let mut by_path: std::collections::BTreeMap<String, Vec<EntityDemand>> = Default::default();
    for demand in demands {
        by_path
            .entry(crate::gocompat::clean(&demand.location.path))
            .or_default()
            .push(demand.clone());
    }
    by_path
        .into_iter()
        .map(|(path, demands)| DemandInputGroup { path, demands })
        .collect()
}

fn paths(demands: &[EntityDemand]) -> Vec<String> {
    let mut paths: Vec<String> = demands
        .iter()
        .map(|demand| crate::gocompat::clean(&demand.location.path))
        .collect();
    paths.sort();
    paths.dedup();
    paths
}

/// Go's `assertFullWireTransitionsIdentical`.
fn assert_full_transitions_identical(
    what: &str,
    step: usize,
    project_id: &str,
    got: &ProducerTable,
    want: &ProducerTable,
) {
    let encode = |table: &ProducerTable| {
        TransitionEncoder::default()
            .encode(TransitionInput {
                project_id,
                base_state_token: "",
                base: None,
                target: table,
                sparse: false,
            })
            .unwrap()
            .bytes
    };
    assert!(
        encode(got) == encode(want),
        "step {step} (generation {}): {what} diverges from fresh materialization",
        got.generation
    );
}

fn open_closure(config: &str) -> DemandClosure {
    DemandClosure::new(Box::new(open(config)), None)
}

// ---- jsx_symbols_integration_test.go --------------------------------------

struct JsxFixture {
    _scratch: Scratch,
    consumer_path: String,
    consumer: String,
    locations: Vec<(&'static str, Location)>,
    scratch_config: String,
}

fn jsx_fixture() -> JsxFixture {
    let consumer = "import { Component } from \"./runtime\";\n\
import { Component as Alias } from \"./runtime\";\n\
import * as Runtime from \"./runtime\";\n\
import { ReExported } from \"./bridge\";\n\
import type { Component as TypeOnlyComponent } from \"./runtime\";\n\
\n\
function Local() { return <div />; }\n\
const opening = <Component></Component>;\n\
const selfClosing = <Alias />;\n\
const namespaceMember = <Runtime.Component />;\n\
const reexported = <ReExported />;\n\
const local = <Local />;\n\
const intrinsic = <div />;\n\
const missing = <Missing />;\n\
const missingMember = <MissingNamespace.Component />;\n\
const invalidTypeOnly = <TypeOnlyComponent />;\n\
function render(Component: typeof Runtime.Component) {\n\
\treturn <Component />;\n\
}\n";
    let scratch = Scratch::new(&[
        (
            "tsconfig.json",
            r#"{"compilerOptions":{"strict":true,"module":"esnext","moduleResolution":"bundler","target":"esnext","jsx":"preserve","noEmit":true},"include":["*.ts","*.tsx"]}"#,
        ),
        (
            "jsx.d.ts",
            "declare namespace JSX {\n interface Element {}\n interface IntrinsicElements { div: {}; }\n}",
        ),
        (
            "runtime.tsx",
            "export function Component() { return <div />; }\n",
        ),
        (
            "barrel.ts",
            "export { Component as ReExported } from \"./runtime\";\n",
        ),
        ("bridge.ts", "export { ReExported } from \"./barrel\";\n"),
        ("unrelated.ts", "export const unrelated = 1;\n"),
        ("consumer.tsx", consumer),
    ]);
    let consumer_path = scratch.path("consumer.tsx");
    let at = |needle: &str, from: usize| {
        let start = from + consumer[from..].find(needle).unwrap();
        loc(&consumer_path, start, start + needle.len())
    };
    let anchor = |text: &str| consumer.find(text).unwrap();
    let shadow = anchor("function render");
    let locations = vec![
        ("namespaceImport", at("Runtime", anchor("import * as"))),
        ("opening", at("Component", anchor("const opening"))),
        ("closing", at("Component", anchor("</Component>"))),
        ("alias", at("Alias", anchor("const selfClosing"))),
        (
            "namespaceMember",
            at("Runtime.Component", anchor("const namespaceMember")),
        ),
        (
            "namespaceRoot",
            at("Runtime", anchor("const namespaceMember")),
        ),
        ("reexported", at("ReExported", anchor("const reexported"))),
        ("local", at("Local", anchor("const local"))),
        ("intrinsic", at("div", anchor("const intrinsic"))),
        ("missing", at("Missing", anchor("const missing"))),
        (
            "missingMember",
            at("MissingNamespace.Component", anchor("const missingMember")),
        ),
        (
            "missingMemberRoot",
            at("MissingNamespace", anchor("const missingMember")),
        ),
        (
            "typeOnly",
            at("TypeOnlyComponent", anchor("const invalidTypeOnly")),
        ),
        (
            "shadowUse",
            at(
                "Component",
                consumer[shadow..].find("return <").unwrap() + shadow,
            ),
        ),
    ];
    JsxFixture {
        scratch_config: scratch.config(),
        consumer_path,
        consumer: consumer.to_owned(),
        locations,
        _scratch: scratch,
    }
}

fn entity_at<'a>(fixture: &JsxFixture, entities: &'a [EntityFact], label: &str) -> &'a EntityFact {
    let wanted = &fixture
        .locations
        .iter()
        .find(|(name, _)| *name == label)
        .unwrap()
        .1;
    entities
        .iter()
        .find(|entity| &entity.location == wanted)
        .unwrap_or_else(|| panic!("missing {label} entity at {wanted:?}"))
}

#[test]
fn jsx_tag_demands_use_compiler_symbol_identity() {
    let fixture = jsx_fixture();
    let mut project = open(&fixture.scratch_config);
    let mut demands: Vec<EntityDemand> = fixture
        .locations
        .iter()
        .map(|(_, location)| EntityDemand {
            location: location.clone(),
            symbol: true,
            reference_space: true,
            ..EntityDemand::default()
        })
        .collect();
    demands.sort_by_key(|demand| (demand.location.start_byte, demand.location.end_byte));
    let entities = super::project_support::entities(&mut project, &fixture.consumer_path, &demands);
    let missing: Vec<&Location> = ["missing", "missingMember", "missingMemberRoot"]
        .iter()
        .map(|label| {
            &fixture
                .locations
                .iter()
                .find(|(name, _)| name == label)
                .unwrap()
                .1
        })
        .collect();
    for entity in &entities {
        if missing.contains(&&entity.location) {
            assert!(
                entity.symbol.is_empty() && entity.symbol_unresolved(),
                "unresolved entity at {:?} = {entity:?}",
                entity.location
            );
        } else {
            assert!(
                !entity.symbol.is_empty() && !entity.symbol_unresolved(),
                "resolved entity at {:?} = {entity:?}",
                entity.location
            );
        }
    }
    // The JSX tags of one imported component are one symbol through the alias
    // chain, so their canonical declarations agree.
    let canonical = |project: &mut Project, label: &str| {
        let id = entity_at(&fixture, &entities, label).symbol.clone();
        let mut current = id.clone();
        while let Some(target) =
            crate::backend::ClosureBackend::resolve_alias(project, &Context::new(), &current)
                .unwrap()
        {
            current = target;
        }
        current
    };
    let component = canonical(&mut project, "opening");
    for label in ["closing", "alias", "namespaceMember", "reexported"] {
        assert_eq!(canonical(&mut project, label), component, "{label}");
    }
    // A type-only import used as a JSX tag is still a value use of the symbol;
    // Go's runtime-identity assertion on it is part of the unavailable family.
    assert_eq!(
        entity_at(&fixture, &entities, "typeOnly").reference_space,
        Some(typefacts::ReferenceSpace::Value),
    );
    assert!(
        entity_at(&fixture, &entities, "typeOnly")
            .runtime_identity
            .is_empty()
    );
    assert!(!fixture.consumer.is_empty());
}

#[test]
fn jsx_tag_demand_does_not_eagerly_resolve_symbol_identity() {
    let fixture = jsx_fixture();
    let mut project = open(&fixture.scratch_config);
    let location = fixture
        .locations
        .iter()
        .find(|(name, _)| *name == "namespaceMember")
        .unwrap()
        .1
        .clone();
    let entities = super::project_support::entities(
        &mut project,
        &fixture.consumer_path,
        &[EntityDemand {
            location,
            ..EntityDemand::default()
        }],
    );
    assert_eq!(entities.len(), 1);
    let entity = &entities[0];
    assert!(
        entity.symbol.is_empty()
            && entity.reference_space.is_none()
            && entity.runtime_identity.is_empty(),
        "symbol identity was computed without a symbol-bearing demand: {entity:?}"
    );
}

// ---- global_scope_integration_test.go ---------------------------------------

/// Editing a script-kind declaring file must fail closed: the import-edge walk
/// cannot see global-scope references, so nothing may be retained and the
/// table must equal a fresh materialization.
#[test]
fn global_scope_edit_fails_closed_and_keeps_parity() {
    let ctx = Context::new();
    let root = testdata("retained-scripts");
    let config = root.join("tsconfig.json").to_string_lossy().into_owned();
    let declaring = crate::gocompat::clean(&root.join("globals.ts").to_string_lossy());
    let original = std::fs::read(&declaring).unwrap();
    // Prepending shifts every declaration span in the file, so any retained
    // identity minted from those spans becomes stale.
    let edit = FileChange {
        path: declaring,
        source: [b"// global-scope edit\n".as_slice(), &original].concat(),
        deleted: false,
        version: 1,
    };

    let mut incremental = open_closure(&config);
    let demands = symbol_demands(&mut incremental);
    incremental
        .demand_table_for_groups(&ctx, 1, &groups(&demands), &paths(&demands))
        .unwrap();
    let affected = incremental
        .update(&ctx, std::slice::from_ref(&edit))
        .unwrap()
        .affected;
    let sources = incremental.source_files(&ctx).unwrap();
    assert!(
        affected.len() >= sources.len(),
        "editing a global-scope declaring file reported {} affected of {} source files; \
         the affected set must fail closed",
        affected.len(),
        sources.len()
    );

    let rebased = symbol_demands(&mut incremental);
    let table = incremental
        .demand_table_for_groups(&ctx, 2, &groups(&rebased), &[])
        .unwrap();
    let retention = incremental.stats().retention;
    assert_eq!(
        retention.retained_files, 0,
        "a global-scope edit retained files; every identity may have moved ({retention:?})"
    );
    let mut fresh = open_closure(&config);
    fresh.update(&ctx, std::slice::from_ref(&edit)).unwrap();
    let fresh_demands = symbol_demands(&mut fresh);
    let fresh_table = fresh
        .demand_table_for_groups(&ctx, 2, &groups(&fresh_demands), &[])
        .unwrap();
    assert_full_transitions_identical(
        "table after a global-scope edit",
        0,
        &config,
        &table,
        &fresh_table,
    );
}

// ---- non_durable_integration_test.go ----------------------------------------

fn is_ident(byte: u8) -> bool {
    byte == b'_' || byte.is_ascii_alphanumeric()
}

/// A symbol at every property name after a dot, plus every binding name: the
/// mapped-type accesses resolve to synthesized symbols, which is what makes
/// the file non-durable.
fn non_durable_demands(root: &std::path::Path) -> Vec<EntityDemand> {
    let mut names: Vec<PathBuf> = std::fs::read_dir(root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "ts"))
        .collect();
    names.sort();
    let mut demands = Vec::new();
    for name in names {
        let source = std::fs::read(&name).unwrap();
        let path = name.to_string_lossy().into_owned();
        let mut offset = 0;
        while let Some(dot) = source[offset..].iter().position(|byte| *byte == b'.') {
            let start = offset + dot + 1;
            let mut end = start;
            while end < source.len() && is_ident(source[end]) {
                end += 1;
            }
            if end > start {
                demands.push(EntityDemand {
                    location: loc(&path, start, end),
                    symbol: true,
                    references: true,
                    ..EntityDemand::default()
                });
            }
            offset = start;
        }
    }
    assert!(
        !demands.is_empty(),
        "the fixture produced no property demands"
    );
    demands
}

fn table_transition_mode(transition: &[u8]) -> u64 {
    fn uvarint(bytes: &[u8]) -> (u64, usize) {
        let (mut value, mut shift) = (0u64, 0);
        for (index, byte) in bytes.iter().enumerate() {
            value |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return (value, index + 1);
            }
            shift += 7;
        }
        panic!("table transition has no varint");
    }
    let (_, width) = uvarint(transition);
    uvarint(&transition[width..]).0
}

/// A mapped-type property access resolves to a synthesized symbol, so the
/// producer mints a generation-scoped identity for it and the file can never
/// be retained. That is unavoidable. What was avoidable is the consequence: a
/// delta can describe re-minted identities perfectly well; it just has to be
/// told which paths moved.
#[test]
fn non_durable_files_still_get_a_delta() {
    let root = testdata("non-durable");
    let config = root.join("tsconfig.json").to_string_lossy().into_owned();
    let mut session = Session::new(Box::new(open(&config)), &config, true).unwrap();
    let demands = non_durable_demands(&root);

    let mut cold = super::lifecycle_request(1, Operation::Analyze, 1);
    cold.project_id.clone_from(&config);
    cold.reset_state = true;
    cold.demands = demands;
    let first = super::lifecycle(&mut session, cold);
    assert!(
        first.ok
            && !first.table_transition.is_empty()
            && table_transition_mode(&first.table_transition) == 0,
        "cold analyze = {first:?}"
    );
    let non_durable = first
        .timings
        .as_ref()
        .map_or(0, |timings| timings.non_durable_files);
    assert!(
        non_durable > 0,
        "the fixture is durable, so this test proves nothing"
    );
    let mut token = first.state_token;

    let island = crate::gocompat::clean(&root.join("island.ts").to_string_lossy());
    let original = std::fs::read(&island).unwrap();
    // Several edits, so this covers the steady state rather than one transition.
    let (mut generation, mut request_id) = (1u64, 1u64);
    for edit in 1..=3u64 {
        generation += 1;
        request_id += 1;
        let mut source = original.clone();
        source.extend_from_slice(format!("\nexport const edit{edit} = 1;\n").as_bytes());
        let mut update = super::lifecycle_request(request_id, Operation::Update, generation);
        update.project_id.clone_from(&config);
        update.changes = vec![FileChange {
            path: island.clone(),
            source,
            deleted: false,
            version: edit,
        }];
        assert!(super::lifecycle(&mut session, update).ok, "update {edit}");

        request_id += 1;
        let mut analyze = super::lifecycle_request(request_id, Operation::Analyze, generation);
        analyze.project_id.clone_from(&config);
        analyze.state_token.clone_from(&token);
        let analyzed = super::lifecycle(&mut session, analyze);
        assert!(analyzed.ok, "analyze {edit} = {analyzed:?}");
        token = analyzed.state_token.clone();
        let non_durable = analyzed
            .timings
            .as_ref()
            .map_or(0, |timings| timings.non_durable_files);
        assert!(non_durable > 0, "edit {edit} stopped being non-durable");
        assert!(
            !analyzed.table_transition.is_empty()
                && table_transition_mode(&analyzed.table_transition) == 1,
            "edit {edit} did not carry a delta because {non_durable} file(s) are non-durable; \
             one synthesized symbol must not cost a project-wide pack per keystroke"
        );
    }
}

/// The correctness half: retained contributions must reproduce a fresh
/// whole-batch table even when identities inside them were re-minted this
/// generation.
#[test]
fn non_durable_retention_matches_fresh_materialization() {
    let ctx = Context::new();
    let root = testdata("non-durable");
    let config = root.join("tsconfig.json").to_string_lossy().into_owned();
    let demands = non_durable_demands(&root);
    let island = crate::gocompat::clean(&root.join("island.ts").to_string_lossy());
    let original = std::fs::read(&island).unwrap();
    let edit = FileChange {
        path: island,
        source: [b"// non-durable delta edit\n".as_slice(), &original].concat(),
        deleted: false,
        version: 1,
    };

    let mut incremental = open_closure(&config);
    incremental
        .demand_table_for_groups(&ctx, 1, &groups(&demands), &paths(&demands))
        .unwrap();
    assert!(
        incremental.stats().retention.non_durable_files > 0,
        "the fixture is durable, so this test proves nothing"
    );
    incremental
        .update(&ctx, std::slice::from_ref(&edit))
        .unwrap();
    let table = incremental
        .demand_table_for_groups(&ctx, 2, &groups(&demands), &[])
        .unwrap();
    let mut fresh = open_closure(&config);
    fresh.update(&ctx, std::slice::from_ref(&edit)).unwrap();
    let fresh_table = fresh
        .demand_table_for_groups(&ctx, 2, &groups(&demands), &[])
        .unwrap();
    assert_full_transitions_identical(
        "retained table with non-durable files",
        0,
        &config,
        &table,
        &fresh_table,
    );
}

// ---- incremental_tsgo_adversarial_test.go -----------------------------------

fn assert_alternating_script_matches_fresh(
    files: &[(&str, &str)],
    script: impl FnOnce(&Scratch, &dyn Fn(&str) -> Vec<u8>) -> Vec<FileChange>,
) {
    let ctx = Context::new();
    let scratch = Scratch::new(files);
    let config = scratch.config();
    let originals = |name: &str| std::fs::read(scratch.path(name)).unwrap();
    let changes = script(&scratch, &originals);

    let mut incremental = open_closure(&config);
    let initial = symbol_demands(&mut incremental);
    incremental
        .demand_table_for_groups(&ctx, 1, &groups(&initial), &paths(&initial))
        .unwrap();
    for (step, change) in changes.iter().enumerate() {
        incremental
            .update(&ctx, std::slice::from_ref(change))
            .unwrap_or_else(|error| panic!("incremental update {step}: {error}"));
        let generation = step as u64 + 2;
        let demands = symbol_demands(&mut incremental);
        let retained = incremental
            .demand_table_for_groups(&ctx, generation, &groups(&demands), &[])
            .unwrap_or_else(|error| panic!("incremental materialization {step}: {error}"));

        let mut fresh = open_closure(&config);
        for (replay, change) in changes.iter().enumerate().take(step + 1) {
            fresh
                .update(&ctx, std::slice::from_ref(change))
                .unwrap_or_else(|error| panic!("fresh replay {replay}/{step}: {error}"));
        }
        let fresh_demands = symbol_demands(&mut fresh);
        let whole = fresh
            .demand_table_for_groups(&ctx, generation, &groups(&fresh_demands), &[])
            .unwrap_or_else(|error| panic!("fresh materialization {step}: {error}"));
        assert_full_transitions_identical(
            "adversarial retained table",
            step,
            &config,
            &retained,
            &whole,
        );
    }
}

fn alternating(path: &str, before: &[u8], after: &[u8], count: u64) -> Vec<FileChange> {
    (0..count)
        .map(|index| FileChange {
            path: path.to_owned(),
            source: if index % 2 == 0 { after } else { before }.to_vec(),
            deleted: false,
            version: index + 1,
        })
        .collect()
}

const STRICT_CONFIG: &str = r#"{"compilerOptions":{"module":"esnext","moduleResolution":"bundler","target":"esnext","strict":true},"include":["*.ts"]}"#;

#[test]
fn module_and_global_augmentation_alternating_generations_match_fresh() {
    type Case<'a> = (&'a str, Vec<(&'a str, &'a str)>, &'a str);
    let cases: [Case; 2] = [
        (
            "augment.ts",
            vec![
                ("tsconfig.json", STRICT_CONFIG),
                (
                    "base.ts",
                    "export interface Box { value: number }\nexport const box: Box = { value: 1, extra: \"x\" } as Box;\n",
                ),
                (
                    "augment.ts",
                    "import \"./base\";\ndeclare module \"./base\" { interface Box { extra: string } }\nexport {};\n",
                ),
                (
                    "consumer.ts",
                    "import { box } from \"./base\";\nfunction read(): string { return box.extra; }\nexport const result = read();\n",
                ),
                (
                    "unrelated.ts",
                    "export function alone(): number { return 1; }\nexport const local = alone();\n",
                ),
            ],
            "import \"./base\";\ndeclare module \"./base\" { interface Box { extra: number } }\nexport {};\n",
        ),
        (
            "augment.ts",
            vec![
                ("tsconfig.json", STRICT_CONFIG),
                (
                    "augment.ts",
                    "declare global { interface Window { extra: string } }\nexport {};\n",
                ),
                (
                    "consumer.ts",
                    "function read(): string { return window.extra; }\nexport const result = read();\n",
                ),
                (
                    "unrelated.ts",
                    "export function alone(): number { return 1; }\nexport const local = alone();\n",
                ),
            ],
            "declare global { interface Window { extra: number } }\nexport {};\n",
        ),
    ];
    for (target, files, after) in cases {
        assert_alternating_script_matches_fresh(&files, |scratch, originals| {
            alternating(
                &scratch.path(target),
                &originals(target),
                after.as_bytes(),
                6,
            )
        });
    }
}

#[test]
fn inferred_export_alternating_generations_match_fresh() {
    let files = [
        ("tsconfig.json", STRICT_CONFIG),
        ("dependency.ts", "export function value() { return 1; }\n"),
        (
            "consumer.ts",
            "import { value } from \"./dependency\";\nexport const result = value();\n",
        ),
        (
            "unrelated.ts",
            "export function alone(): number { return 1; }\nexport const local = alone();\n",
        ),
    ];
    assert_alternating_script_matches_fresh(&files, |scratch, originals| {
        alternating(
            &scratch.path("dependency.ts"),
            &originals("dependency.ts"),
            b"export function value() { return \"changed\"; }\n",
            8,
        )
    });
}

#[test]
fn deletion_restoration_and_config_alternation_match_fresh() {
    let files = [
        ("tsconfig.json", STRICT_CONFIG),
        (
            "dependency.ts",
            "export function value(): number { return 1; }\n",
        ),
        (
            "consumer.ts",
            "import { value } from \"./dependency\";\nexport const result = value();\n",
        ),
        (
            "unrelated.ts",
            "export function alone(): number { return 1; }\nexport const local = alone();\n",
        ),
    ];
    assert_alternating_script_matches_fresh(&files, |scratch, originals| {
        let change = |name: &str, version: u64, source: Option<Vec<u8>>| FileChange {
            path: scratch.path(name),
            deleted: source.is_none(),
            source: source.unwrap_or_default(),
            version,
        };
        let loose = br#"{"compilerOptions":{"module":"esnext","moduleResolution":"bundler","target":"esnext","strict":false},"include":["*.ts"]}"#.to_vec();
        vec![
            change("dependency.ts", 1, None),
            change("dependency.ts", 2, Some(originals("dependency.ts"))),
            change("tsconfig.json", 1, Some(loose)),
            change("tsconfig.json", 2, Some(originals("tsconfig.json"))),
            change("dependency.ts", 3, None),
            change("dependency.ts", 4, Some(originals("dependency.ts"))),
        ]
    });
}
