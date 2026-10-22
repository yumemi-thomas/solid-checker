//! Ports the symbol and semantic-run cases of the Go compiler-backed tests:
//! `symbol_covers_span_test.go`, `semantic_runs_test.go`,
//! `durable_sweep_test.go`, the reference-space cases of
//! `compiler_facts_test.go`, and `TestReleaseAnalysisStateRehydratesDurableIdentity`.
//!
//! The assertions that need a family S4a leaves unavailable (resolved calls,
//! callability, runtime domains, runtime identity) are not ported here; the
//! tests below state what the entity carries instead.

use super::project_support::{
    BUNDLER_CONFIG, SMALL_CONFIG, Scratch, change, deletion, entities, find, find_from, find_last,
    loc, symbol_demand,
};
use crate::backend::ClosureBackend;
use crate::context::Context;
use crate::model::{SemanticDemandRun, SemanticScope, durable_symbol_id};
use std::collections::{HashMap, HashSet};
use typefacts::v3::EntityDemand;
use typefacts::{ReferenceSpace, SymbolClaim};

const STRICT: &str = r#"{"compilerOptions":{"strict":true,"module":"esnext","moduleResolution":"bundler","target":"esnext"},"include":["*.ts"]}"#;

/// A demanded span's symbol is the node at its first byte. It describes the
/// whole span only when that node is the whole span: `f(x)` and `o.p` start at
/// their callee and object, so their symbols name a token inside the span.
#[test]
fn entity_symbol_covers_only_the_whole_demanded_span() {
    let source = "declare function makeHandler(cb: () => void): () => void;\n\
const count = 1;\n\
const obj = { handler() {} };\n\
makeHandler(() => {});\n\
count;\n\
obj.handler;\n";
    let scratch = Scratch::new(&[("tsconfig.json", STRICT), ("calls.ts", source)]);
    let path = scratch.path("calls.ts");
    let mut project = scratch.open();
    // The token is located inside the anchor so each demand names exactly one
    // span of the source, whatever else the anchor's text shares with it.
    let cases = [
        (
            "call names its callee",
            "makeHandler(() => {})",
            "makeHandler(() => {})",
            SymbolClaim::Partial,
        ),
        (
            "identifier covers itself",
            "\ncount;",
            "count",
            SymbolClaim::Covering,
        ),
        (
            "binding covers itself",
            "const count",
            "count",
            SymbolClaim::Covering,
        ),
        (
            "member names its object",
            "\nobj.handler;",
            "obj.handler",
            SymbolClaim::Partial,
        ),
    ];
    let demands: Vec<EntityDemand> = cases
        .iter()
        .map(|(_, anchor, token, _)| {
            let start = source.find(anchor).unwrap() + anchor.find(token).unwrap();
            symbol_demand(loc(&path, start, start + token.len()))
        })
        .collect();
    let facts = entities(&mut project, &path, &demands);
    assert_eq!(facts.len(), cases.len());
    for ((name, _, _, claim), entity) in cases.iter().zip(&facts) {
        assert!(!entity.symbol.is_empty(), "{name}: no symbol");
        assert_eq!(entity.symbol_claim, *claim, "{name}");
        assert_eq!(entity.symbol_covers_span(), *claim == SymbolClaim::Covering);
    }
}

#[test]
fn semantic_demand_runs_align_results_and_report_durability() {
    let types = "export type Handler = (value: number) => string;";
    let library = "import type { Handler } from \"./types\";\n\
export function invoke(callback: Handler): string { return callback(1); }\n";
    let alpha = "import { invoke } from \"./library\";\n\
export const alpha = invoke(value => String(value));\n\
export const alphaAgain = invoke(value => String(value));\n";
    let beta = "type Calls<T extends string> = { [K in T]: (value: number) => number };\n\
declare const mapped: Calls<\"run\">;\n\
export const beta = mapped.run(1);\n";
    let scratch = Scratch::new(&[
        ("tsconfig.json", STRICT),
        ("types.ts", types),
        ("library.ts", library),
        ("alpha.ts", alpha),
        ("beta.ts", beta),
    ]);
    let (alpha_path, beta_path) = (scratch.path("alpha.ts"), scratch.path("beta.ts"));
    let mut project = scratch.open();

    // The Go test demands resolved calls and reads their declarations; the
    // symbol at the callee carries the same durability evidence (the mapped
    // type's `run` is synthesized, so it has no declaration to hash).
    let beta_demands = vec![symbol_demand(find(&beta_path, beta, "run(1)"))];
    let alpha_demands = vec![
        symbol_demand(find(&alpha_path, alpha, "invoke(value")),
        symbol_demand(find_last(&alpha_path, alpha, "invoke(value")),
    ];
    let runs = [
        SemanticDemandRun {
            path: &beta_path,
            demands: &beta_demands,
        },
        SemanticDemandRun {
            path: &alpha_path,
            demands: &alpha_demands,
        },
    ];
    let (suppression, seed) = (HashSet::new(), HashMap::new());
    let scope = SemanticScope {
        suppression: &suppression,
        descriptor_seed: &seed,
    };
    let results = project
        .semantic_demand_runs(&Context::new(), &runs, &scope)
        .unwrap();
    assert_eq!(results.len(), runs.len());
    for (run, result) in runs.iter().zip(&results) {
        assert_eq!(result.entities.len(), run.demands.len());
        assert_eq!(result.structural.len(), run.demands.len());
        for (demand, entity) in run.demands.iter().zip(&result.entities) {
            assert_eq!(entity.location, demand.location);
        }
    }
    for (index, result) in results.iter().enumerate() {
        let ids: Vec<&str> = result
            .structural
            .iter()
            .map(|id| &**id)
            .chain(result.entities.iter().map(|entity| &*entity.symbol))
            .collect();
        let want_durable = ids.iter().all(|id| durable_symbol_id(id));
        assert_eq!(result.durable, want_durable, "result {index}: {ids:?}");
        if index == 0 {
            assert!(
                !want_durable,
                "the mapped call embedded only durable ids {ids:?}; non-durable coverage is vacuous"
            );
        } else {
            assert!(
                want_durable,
                "the source-backed call embedded non-durable ids {ids:?}"
            );
        }
    }
}

#[test]
fn semantic_demand_runs_reject_a_cross_file_query_location() {
    let scratch = Scratch::new(&[
        (
            "tsconfig.json",
            r#"{"compilerOptions":{"strict":true},"include":["*.ts"]}"#,
        ),
        ("run.ts", "export const value = 1;"),
        ("other.ts", "export const other = 2;"),
    ]);
    let (run_path, other_path) = (scratch.path("run.ts"), scratch.path("other.ts"));
    let mut project = scratch.open();
    let demands = [EntityDemand {
        location: loc(&run_path, 0, 0),
        query_location: Some(loc(&other_path, 0, 0)),
        symbol: true,
        ..EntityDemand::default()
    }];
    let (suppression, seed) = (HashSet::new(), HashMap::new());
    let scope = SemanticScope {
        suppression: &suppression,
        descriptor_seed: &seed,
    };
    let error = project
        .semantic_demand_runs(
            &Context::new(),
            &[SemanticDemandRun {
                path: &run_path,
                demands: &demands,
            }],
            &scope,
        )
        .unwrap_err();
    assert!(
        error.0.contains("query location")
            && error.0.contains(&run_path)
            && error.0.contains(&other_path),
        "semantic_demand_runs error = {error}"
    );
}

/// The families a later slice owns are absent (never a fabricated value),
/// counted, and the ported ones beside them are answered. S4c ported
/// `typeDescriptor`, `callability`, `constructability` and
/// `runtimeValueDomain`; `resolvedCall`, `parameterObjectShape` and
/// `runtimeIdentity` remain.
#[test]
fn unported_families_are_absent_counted_and_never_fabricated() {
    let source = "declare function factory(): any;\nexport const result = factory();\n";
    let scratch = Scratch::new(&[
        (
            "tsconfig.json",
            r#"{"compilerOptions":{"strict":true},"include":["*.ts"]}"#,
        ),
        ("run.ts", source),
    ]);
    let path = scratch.path("run.ts");
    let mut project = scratch.open();
    let start = source.find("factory();").unwrap();
    let demand = |end: usize| EntityDemand {
        location: loc(&path, start, end),
        symbol: true,
        callability: true,
        runtime_value_domain: true,
        constructability: true,
        resolved_call: true,
        reference_space: true,
        type_descriptor: true,
        runtime_identity: true,
        call_result_domain: true,
        ..EntityDemand::default()
    };
    let facts = entities(
        &mut project,
        &path,
        &[
            demand(start + "factory".len()),
            demand(start + "factory()".len()),
        ],
    );
    for entity in &facts {
        assert!(!entity.symbol.is_empty());
        assert!(entity.callability.is_some() && entity.constructability.is_some());
        assert!(entity.runtime_value_domain.is_some());
        assert!(entity.type_descriptor.is_some());
        assert!(entity.resolved_call.is_none() && entity.runtime_identity.is_empty());
        assert!(entity.call_result_domain.is_none());
        assert_eq!(entity.reference_space, Some(ReferenceSpace::Value));
    }
    for family in ["resolvedCall", "runtimeIdentity", "runtimeBindingKind"] {
        assert_eq!(project.unavailable.count(family), 2, "{family}");
    }
    // Only the span that is exactly a call needs the call resolver.
    assert_eq!(project.unavailable.count("callResultDomain"), 1);
    for family in [
        "callability",
        "constructability",
        "runtimeValueDomain",
        "typeDescriptor",
        "referenceSpace",
    ] {
        assert_eq!(project.unavailable.count(family), 0, "{family}");
    }
}

/// The scope's descriptor seed is compiler-independent, so a seeded
/// descriptor answers (and contributes its alias declarations as
/// dependencies) while a suppressed symbol stays absent.
#[test]
fn the_scope_seeds_descriptors_and_suppresses_structural_symbols() {
    let source = "export const value = 1;\nexport const other = 2;\n";
    let scratch = Scratch::new(&[("tsconfig.json", STRICT), ("a.ts", source)]);
    let (path, other_path) = (scratch.path("a.ts"), "/elsewhere/types.ts".to_owned());
    let mut project = scratch.open();
    let value = find(&path, source, "value");
    let other = find(&path, source, "other");
    let value_id = project.symbol_at(&Context::new(), &value).unwrap();
    let other_id = project.symbol_at(&Context::new(), &other).unwrap();
    let descriptor = std::sync::Arc::new(typefacts::TypeDescriptor {
        text: "Seeded".into(),
        origin_module: "".into(),
        alias_declarations: vec![typefacts::Declaration {
            name: "Seeded".into(),
            kind: "declaration".into(),
            location: loc(&other_path, 0, 6),
        }]
        .into(),
    });
    let seed = HashMap::from([
        (value_id.clone(), descriptor.clone()),
        (other_id.clone(), descriptor.clone()),
    ]);
    let suppression = HashSet::from([other_id]);
    let scope = SemanticScope {
        suppression: &suppression,
        descriptor_seed: &seed,
    };
    let demands: Vec<EntityDemand> = [&value, &other]
        .into_iter()
        .map(|location| EntityDemand {
            location: location.clone(),
            symbol: true,
            type_descriptor: true,
            ..EntityDemand::default()
        })
        .collect();
    let results = project
        .semantic_demand_runs(
            &Context::new(),
            &[SemanticDemandRun {
                path: &path,
                demands: &demands,
            }],
            &scope,
        )
        .unwrap();
    let result = &results[0];
    assert_eq!(
        result.entities[0].type_descriptor.as_deref(),
        Some(&*descriptor)
    );
    assert!(
        result.entities[1].type_descriptor.is_none(),
        "a suppressed symbol keeps no descriptor"
    );
    assert_eq!(result.dependencies, [other_path]);
    assert_eq!(project.unavailable.count("typeDescriptor"), 0);
}

#[test]
fn an_unknown_run_file_leaves_bare_entities() {
    let scratch = Scratch::new(&[("tsconfig.json", STRICT), ("a.ts", "export const a = 1;\n")]);
    let missing = scratch.path("missing.ts");
    let mut project = scratch.open();
    let facts = entities(
        &mut project,
        &missing,
        &[symbol_demand(loc(&missing, 0, 1))],
    );
    assert_eq!(facts.len(), 1);
    assert!(facts[0].symbol.is_empty() && facts[0].symbol_claim == SymbolClaim::Absent);
}

/// A deleted file's identities can never re-resolve, and re-entry re-mints
/// them through location scans -- so the full-rebuild update path must drop
/// them instead of letting a long session accrete every file it ever saw.
/// Identities of files still in the program must survive the same sweep.
#[test]
fn full_rebuild_sweeps_departed_files_durable_identities() {
    let scratch = Scratch::new(&[
        ("tsconfig.json", SMALL_CONFIG),
        ("kept.ts", "export const kept = 1\n"),
        ("doomed.ts", "export const doomed = 2\n"),
    ]);
    let (kept, doomed) = (scratch.path("kept.ts"), scratch.path("doomed.ts"));
    let mut project = scratch.open();
    let ctx = Context::new();
    for location in [loc(&kept, 13, 17), loc(&doomed, 13, 19)] {
        project.symbol_at(&ctx, &location).unwrap();
    }
    let ref_paths = |project: &crate::project::Project| -> HashSet<String> {
        project
            .identities()
            .durable_refs()
            .values()
            .map(|reference| reference.path.clone())
            .collect()
    };
    let before = ref_paths(&project);
    assert!(
        before.contains(&kept) && before.contains(&doomed),
        "identities were not minted for both files: {before:?}"
    );
    // A delete forces the full-rebuild update path, which sweeps.
    project.update(&ctx, &[deletion(&doomed, 1)]).unwrap();
    let after = ref_paths(&project);
    assert!(
        !after.contains(&doomed),
        "the deleted file's durable identities survived the sweep"
    );
    assert!(
        after.contains(&kept),
        "a living file's identities were swept"
    );
}

#[test]
fn release_analysis_state_rehydrates_durable_identity() {
    let source = "export function target() { return 1 }\nexport const value = target();\n";
    let scratch = Scratch::new(&[("tsconfig.json", SMALL_CONFIG), ("source.ts", source)]);
    let path = scratch.path("source.ts");
    let mut project = scratch.open();
    let ctx = Context::new();
    let location = find_last(&path, source, "target");

    let before = project.symbol_at(&ctx, &location).unwrap();
    assert!(durable_symbol_id(&before) && before.starts_with("symbol:h:"));
    project.release_analysis_state_for_test();
    assert!(
        !project.current().unwrap().has_checker() && !project.identities().holds_symbols(),
        "checker-owned state survived ReleaseAnalysisState"
    );
    let after = project.symbol_at(&ctx, &location).unwrap();
    assert_eq!(after, before, "rehydrated symbol");
    // The id minted before the release still resolves to its declaration.
    let declarations = project.declarations(&ctx, &before).unwrap().unwrap();
    assert_eq!(declarations.len(), 1);
    assert_eq!(&*declarations[0].kind, "function");
}

/// Reference space follows the symbol's meanings and every use of it
/// (`TestReferenceSpaceAndCanonicalRuntimeIdentity`, without the runtime
/// identity assertions).
#[test]
fn reference_space_follows_uses_and_aliases() {
    let consumer = "import { JSX, Portal, Portal as Unused, Both } from \"./runtime\";\n\
import type { JSX as TypeOnlyJSX } from \"./runtime\";\n\
import { Shared } from \"./runtime\";\n\
import { NamedPortal } from \"./star\";\n\
import { LocalPortal } from \"./local\";\n\
import { SubpathPortal } from \"runtime-package/subpath\";\n\
type Element = TypeOnlyJSX;\n\
Portal();\n\
type BothType = Both;\n\
new Both();\n\
NamedPortal();\n\
LocalPortal();\n\
SubpathPortal();\n\
type SharedType = Shared;\n\
Shared();\n";
    let scratch = Scratch::new(&[
        ("tsconfig.json", STRICT),
        (
            "runtime.ts",
            "export interface JSX { node: unknown }\nexport function Portal() {}\nexport class Both {}\nexport type Shared = { typeOnly: true };\nexport const Shared = () => 1;\n",
        ),
        (
            "named.ts",
            "export { Portal as NamedPortal } from \"./runtime\";\n",
        ),
        ("star.ts", "export * from \"./named\";\n"),
        (
            "local.ts",
            "import { Portal } from \"./runtime\";\nexport { Portal as LocalPortal };\n",
        ),
        (
            "node_modules/runtime-package/package.json",
            r#"{"name":"runtime-package","exports":{"./subpath":"./subpath.d.ts"}}"#,
        ),
        (
            "node_modules/runtime-package/subpath.d.ts",
            "export { Portal as SubpathPortal } from \"../../runtime\";",
        ),
        ("consumer.ts", consumer),
    ]);
    let path = scratch.path("consumer.ts");
    let mut project = scratch.open();
    let names = [
        "JSX",
        "Portal",
        "Both",
        "NamedPortal",
        "LocalPortal",
        "SubpathPortal",
        "TypeOnlyJSX",
        "Unused",
        "Shared",
    ];
    let demands: Vec<EntityDemand> = names
        .iter()
        .map(|name| EntityDemand {
            location: find(&path, consumer, name),
            symbol: true,
            reference_space: true,
            ..EntityDemand::default()
        })
        .collect();
    let facts = entities(&mut project, &path, &demands);
    use ReferenceSpace::{Both, Neither, Type, Value};
    let want = [
        Neither, Value, Both, Value, Value, Value, Type, Neither, Both,
    ];
    for ((name, want), entity) in names.iter().zip(want).zip(&facts) {
        assert_eq!(entity.reference_space, Some(want), "{name}");
    }
}

#[test]
fn reference_space_classifies_qualified_type_names() {
    let source = "\n\t\timport { JSX, Portal, Namespace, Shared } from \"./runtime\";\n\n\
\t\ttype Style = JSX.CSSProperties;\n\
\t\ttype Element = JSX.Element;\n\
\t\ttype Deep = Namespace.Type.Member;\n\
\t\ttype SharedType = Shared.Type;\n\n\
\t\tShared.runtime;\n\
\t\tPortal();\n\t";
    let scratch = Scratch::new(&[
        (
            "tsconfig.json",
            r#"{"compilerOptions":{"module":"esnext","moduleResolution":"bundler","strict":true},"include":["*.ts"]}"#,
        ),
        (
            "runtime.ts",
            "export namespace JSX {\n export interface CSSProperties { color?: string; }\n export interface Element { node: unknown; }\n}\n\
export function Portal() {}\n\
export namespace Namespace { export namespace Type { export interface Member { value: unknown; } } }\n\
export namespace Shared { export interface Type { value: unknown; } export const runtime = 1; }\n",
        ),
        ("consumer.ts", source),
    ]);
    let path = scratch.path("consumer.ts");
    let mut project = scratch.open();
    let want = [
        ("JSX", ReferenceSpace::Type),
        ("Portal", ReferenceSpace::Value),
        ("Namespace", ReferenceSpace::Type),
        ("Shared", ReferenceSpace::Both),
    ];
    let demands: Vec<EntityDemand> = want
        .iter()
        .map(|(name, _)| EntityDemand {
            location: find(&path, source, name),
            symbol: true,
            reference_space: true,
            ..EntityDemand::default()
        })
        .collect();
    let facts = entities(&mut project, &path, &demands);
    for ((name, space), entity) in want.iter().zip(&facts) {
        assert_eq!(entity.reference_space, Some(*space), "{name}");
    }
}

#[test]
fn a_changed_source_is_reanalyzed_in_the_new_generation() {
    // Not a Go case: the new generation's runs read the accepted program, not
    // the one the identities were first minted in.
    let scratch = Scratch::new(&[
        ("tsconfig.json", SMALL_CONFIG),
        ("a.ts", "export const a = 1;\n"),
    ]);
    let path = scratch.path("a.ts");
    let mut project = scratch.open();
    let before = entities(
        &mut project,
        &path,
        &[symbol_demand(find(&path, "export const a = 1;\n", "a"))],
    );
    project
        .update(
            &Context::new(),
            &[change(&path, 1, "\n\nexport const a = 1;\n")],
        )
        .unwrap();
    let source = "\n\nexport const a = 1;\n";
    let after = entities(
        &mut project,
        &path,
        &[symbol_demand(find_from(&path, source, "a", 15))],
    );
    assert!(!before[0].symbol.is_empty() && !after[0].symbol.is_empty());
    let _ = BUNDLER_CONFIG;
}

/// The cursor's selectors (`exactCallLikeAt`, `resolvedCallLikeAt`,
/// `covering`) that the unported families will use.
#[test]
fn cursor_selects_exact_and_outermost_call_nodes() {
    use crate::nodes::NodeCursor;
    let source = "declare function factory(): (value: number) => number;\nexport const result = factory()(1);\n";
    let scratch = Scratch::new(&[("tsconfig.json", STRICT), ("run.ts", source)]);
    let path = scratch.path("run.ts");
    let project = scratch.open();
    let file = project
        .current()
        .unwrap()
        .program()
        .get_source_file(&path)
        .unwrap();
    let mut cursor = NodeCursor::new(Some(file));
    let start = source.find("factory()(1)").unwrap() as u64;
    let inner_end = start + "factory()".len() as u64;
    let outer_end = start + "factory()(1)".len() as u64;
    // Exact: only the call whose whole span is demanded answers.
    let inner = cursor.exact_call_like_at(start, inner_end).unwrap();
    let outer = cursor.exact_call_like_at(start, outer_end).unwrap();
    assert_ne!(inner, outer);
    assert!(cursor.exact_call_like_at(start, start + 3).is_none());
    // Resolved: the outermost call contained by the demanded span.
    assert_eq!(cursor.resolved_call_like_at(start, outer_end), Some(outer));
    assert_eq!(cursor.resolved_call_like_at(start, inner_end), Some(inner));
    // Covering: the smallest node containing the range.
    assert_eq!(cursor.covering(start, outer_end), Some(outer));
    assert!(cursor.exact_expression_at(start, start).is_none());
}

/// `TestAnalyzeEvidenceEscapesInternalSymbolNames` and the object-shape half
/// of Go's `wireSymbolName` fix. Go spells an internal symbol name `__x`
/// (from its `0xFE` prefix); tsrs marks it with U+007F, which is valid UTF-8
/// and would otherwise reach the wire unnoticed. Every name the producer
/// hands the closure (the source of both the plain-CBOR `symbolEvidence` and
/// `referenceEvidence` sections and of the packed symbol rows) goes through
/// `declarations_for_symbol`, so the evidence of an unnamed class expression
/// (`__class`) carries Go's spelling.
#[test]
fn symbol_evidence_spells_internal_names_like_go() {
    let source = "export const Anonymous = class {};\n";
    let scratch = Scratch::new(&[("tsconfig.json", SMALL_CONFIG), ("a.ts", source)]);
    let path = scratch.path("a.ts");
    let ctx = Context::new();
    let mut project = scratch.open();
    // The checker has no symbol *at* the expression; its declared symbol is
    // the unnamed class's `__class`.
    let start = find(&path, source, "class").start_byte;
    let id = project
        .with_env(|env, _| {
            let file = env.program.get_source_file(&path).unwrap();
            let node = crate::nodes::node_at(file, u32::try_from(start).unwrap());
            env.id_for(node.symbol().expect("class expression symbol"))
        })
        .unwrap();
    let query = typefacts::v3::SymbolQuery {
        id: id.clone(),
        references: false,
        references_only: false,
    };
    let facts =
        crate::backend::SymbolEvidence::symbol_evidence(&mut project, &ctx, &[query]).unwrap();
    assert_eq!(facts.len(), 1);
    let names: Vec<&str> = facts[0]
        .declarations
        .iter()
        .map(|declaration| &*declaration.name)
        .collect();
    assert_eq!(names, ["__class"], "evidence declarations: {facts:?}");
    assert!(
        names.iter().all(|name| !name.contains('\u{7f}')),
        "tsrs's internal-name marker reached the wire"
    );
}

#[test]
fn wire_symbol_name_only_rewrites_the_internal_prefix() {
    use crate::project::wire_symbol_name;
    assert_eq!(wire_symbol_name("\u{7f}index"), "__index");
    assert_eq!(wire_symbol_name("\u{7f}call"), "__call");
    assert_eq!(wire_symbol_name("\u{7f}"), "__");
    assert_eq!(wire_symbol_name("plain"), "plain");
    assert_eq!(wire_symbol_name("__index"), "__index");
    assert_eq!(wire_symbol_name("a\u{7f}b"), "a\u{7f}b");
}
