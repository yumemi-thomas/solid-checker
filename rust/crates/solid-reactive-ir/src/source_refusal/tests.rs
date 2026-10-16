use solid_facts::{
    FileFacts, TypeScriptTable,
    compiler::{COMPILER_FACTS_PROTOCOL, ExecutionMap},
    core::Generation,
};

use super::*;
use crate::package_boundary::ImportKey;
use SourceEdgeForm::{DynamicImport, ExportFrom, Import, ImportEquals, Require};

/// Synthetic facts for `files` at one generation: real AST facts from the
/// real extractor, an empty TypeScript table.
pub(crate) fn project_at(generation: u64, files: &[(&str, &str)]) -> ProjectFacts {
    project_with(generation, files, Vec::new())
}

fn project_with(
    generation: u64,
    files: &[(&str, &str)],
    entities: Vec<typefacts::EntityFact>,
) -> ProjectFacts {
    let generation = Generation::new(generation).unwrap();
    let files = files
        .iter()
        .map(|(path, source)| {
            let ast = solid_facts::ast::extract(*path, source).unwrap();
            let compiler = ExecutionMap {
                compiler_facts_protocol: COMPILER_FACTS_PROTOCOL,
                source_hash: ast.source.hash.clone(),
                semantic_model: Default::default(),
                tracked_regions: vec![],
                untracked_regions: vec![],
                discarded_regions: vec![],
                ownership_regions: vec![],
                callback_roles: vec![],
                jsx_operations: vec![],
            };
            FileFacts::new(generation, *source, ast, compiler).unwrap()
        })
        .collect();
    ProjectFacts {
        generation,
        project_id: "refusal".into(),
        files,
        typescript: TypeScriptTable::from_parts(
            3,
            generation.get(),
            "refusal",
            vec![],
            entities,
            vec![],
            vec![],
        ),
        typescript_changes: None,
        resolved_imports: None,
        runtime_resolutions: None,
        runtime_symbol_redirects: Default::default(),
        dependency_source_roots: vec![],
    }
}

pub(crate) fn project(files: &[(&str, &str)]) -> ProjectFacts {
    project_at(1, files)
}

/// The byte range of the first `needle` in `source`, found by text and never
/// by asking the extractor, so an expectation is independent of the join.
pub(crate) fn range(source: &str, needle: &str) -> (u64, u64) {
    let start = source
        .find(needle)
        .unwrap_or_else(|| panic!("{needle:?} not in {source:?}"));
    (start as u64, (start + needle.len()) as u64)
}

pub(crate) fn site(form: SourceEdgeForm, source: &str, needle: &str) -> SourceEdgeSite {
    let (start_byte, end_byte) = range(source, needle);
    SourceEdgeSite {
        form,
        start_byte,
        end_byte,
    }
}

pub(crate) fn boundary(
    importer: &str,
    specifier: &str,
    sites: impl IntoIterator<Item = SourceEdgeSite>,
) -> PackageBoundary {
    let mut boundary = PackageBoundary::default();
    for site in sites {
        boundary.record_source_edge_refusal(ImportKey::new(importer, specifier), site);
    }
    boundary
}

const A: &str = "/p/a.ts";

fn verdict(
    source: &str,
    specifier: &str,
    sites: impl IntoIterator<Item = SourceEdgeSite>,
) -> Result<(), SourceEdgeMismatch> {
    check(&boundary(A, specifier, sites), &project(&[(A, source)]))
}

fn edges(source: &str, specifier: &str) -> FileEdges {
    runtime_edges(&solid_facts::ast::extract(A, source).unwrap(), specifier)
}

fn labels_of(edges: &FileEdges) -> Vec<(SourceEdgeForm, Vec<String>)> {
    edges
        .occurrences
        .iter()
        .map(|edge| (edge.form, edge.labels.clone()))
        .collect()
}

fn names(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

/// The rejection for an unreferenced non-type binding at `needle` in `source`.
fn collateral(source: &str, needle: &str) -> SourceEdgeMismatch {
    let (start_byte, end_byte) = range(source, needle);
    SourceEdgeMismatch::UnreferencedCollateral {
        importer: A.into(),
        specifier: "m".into(),
        start_byte,
        end_byte,
    }
}

#[test]
fn every_supported_form_joins_at_its_exact_statement_or_expression_with_its_labels() {
    // (source, expected occurrences: form, text of its exact range, labels)
    /// One expected occurrence: its form, the text of its range, its labels.
    type Expected<'a> = (SourceEdgeForm, &'a str, &'a [&'a str]);
    let rows: &[(&str, &[Expected])] = &[
        (
            r#"import { a as b, c } from "m"; b(); c();"#,
            &[(Import, r#"import { a as b, c } from "m";"#, &["a", "c"])],
        ),
        (
            r#"import d from "m"; d();"#,
            &[(Import, r#"import d from "m";"#, &["default"])],
        ),
        (
            r#"export { x as y, type T } from "m"; export { default as z } from "m";"#,
            &[
                (ExportFrom, r#"export { x as y, type T } from "m";"#, &["y"]),
                (ExportFrom, r#"export { default as z } from "m";"#, &["z"]),
            ],
        ),
        (
            r#"export * from "m"; export * as ns from "m";"#,
            &[
                (ExportFrom, r#"export * from "m";"#, &["*"]),
                (ExportFrom, r#"export * as ns from "m";"#, &["* as ns"]),
            ],
        ),
        (
            r#"import S = require("m"); export import T = require("m"); S; T;"#,
            &[
                (ImportEquals, r#"import S = require("m");"#, &["*"]),
                (ImportEquals, r#"import T = require("m");"#, &["*"]),
            ],
        ),
        (
            r#"const x = import("m"); const y = require("m");"#,
            &[
                (DynamicImport, r#"import("m")"#, &["*"]),
                (Require, r#"require("m")"#, &["*"]),
            ],
        ),
    ];
    for (source, expected) in rows {
        let found = edges(source, "m");
        assert!(found.unsupported.is_empty(), "{source}");
        assert_eq!(found.occurrences.len(), expected.len(), "{source}");
        let mut sites = Vec::new();
        for (occurrence, (form, needle, labels)) in found.occurrences.iter().zip(*expected) {
            let (start, end) = range(source, needle);
            assert_eq!(occurrence.form, *form, "{source}");
            assert_eq!(
                (
                    u64::from(occurrence.span.start),
                    u64::from(occurrence.span.end)
                ),
                (start, end),
                "{source}: {needle}"
            );
            assert_eq!(occurrence.labels, names(labels), "{source}");
            sites.push(SourceEdgeSite {
                form: *form,
                start_byte: start,
                end_byte: end,
            });
        }
        assert_eq!(verdict(source, "m", sites), Ok(()), "{source}");
    }
}

#[test]
fn only_a_load_the_extractor_records_a_literal_specifier_for_is_an_occurrence() {
    // Pinned against the extractor as it is: a call with an options argument
    // records no specifier, so it has no textual identity to record; a
    // substitution-free template literal does. Neither is inferred here.
    let source = r#"const a = import("m", { with: { type: "json" } }); const b = import(`m`); const c = import(spec);"#;
    let found = edges(source, "m");
    assert_eq!(found.occurrences.len(), 1, "{:?}", found.occurrences);
    assert_eq!(found.occurrences[0].form, DynamicImport);
    assert_eq!(
        (
            u64::from(found.occurrences[0].span.start),
            u64::from(found.occurrences[0].span.end)
        ),
        range(source, "import(`m`)")
    );
    for needle in [r#"import("m", { with: { type: "json" } })"#, "import(spec)"] {
        assert!(
            matches!(
                verdict(source, "m", [site(DynamicImport, source, needle)]),
                Err(SourceEdgeMismatch::NoSuchOccurrence { .. })
            ),
            "{needle}"
        );
    }
}

#[test]
fn each_rejection_is_its_own_variant_and_never_matches_a_neighbour() {
    let import = r#"import { a } from "m"; a();"#;
    let stmt = r#"import { a } from "m";"#;
    let (start, end) = range(import, stmt);
    let none = |form, start_byte, end_byte| SourceEdgeMismatch::NoSuchOccurrence {
        importer: A.into(),
        specifier: "m".into(),
        form,
        start_byte,
        end_byte,
    };
    let exact = SourceEdgeSite {
        form: Import,
        start_byte: start,
        end_byte: end,
    };
    assert_eq!(verdict(import, "m", [exact]), Ok(()));
    // Shifted by one byte either way: no neighbouring statement or symbol.
    for (shifted_start, shifted_end) in [(start + 1, end), (start, end - 1), (start, end + 1)] {
        assert_eq!(
            verdict(
                import,
                "m",
                [SourceEdgeSite {
                    form: Import,
                    start_byte: shifted_start,
                    end_byte: shifted_end
                }]
            ),
            Err(none(Import, shifted_start, shifted_end))
        );
    }
    // The right range under another form, and another specifier.
    assert_eq!(
        verdict(
            import,
            "m",
            [SourceEdgeSite {
                form: ExportFrom,
                ..exact
            }]
        ),
        Err(none(ExportFrom, start, end))
    );
    assert_eq!(
        check(&boundary(A, "other", [exact]), &project(&[(A, import)])),
        Err(SourceEdgeMismatch::NoSuchOccurrence {
            importer: A.into(),
            specifier: "other".into(),
            form: Import,
            start_byte: start,
            end_byte: end,
        })
    );
    // An unknown importer, and one inside a dependency source root.
    assert_eq!(
        check(
            &boundary("/p/zzz.ts", "m", [exact]),
            &project(&[(A, import)])
        ),
        Err(SourceEdgeMismatch::UnknownImporter {
            importer: "/p/zzz.ts".into()
        })
    );
    let mut dependency = project(&[("/p/dep/a.ts", import)]);
    dependency.dependency_source_roots = vec!["/p/dep".into()];
    assert_eq!(
        check(&boundary("/p/dep/a.ts", "m", [exact]), &dependency),
        Err(SourceEdgeMismatch::ImporterInDependencySource {
            importer: "/p/dep/a.ts".into()
        })
    );
    // Excluded from this API's supported occurrence set (which says nothing
    // about whether a module is loaded), so nothing can be recorded against
    // them. Each is `NoSuchOccurrence`, never silently ok.
    let not_occurrences: &[(&str, SourceEdgeForm, &str)] = &[
        (
            r#"import type { A } from "m"; let x: A;"#,
            Import,
            r#"import type { A } from "m";"#,
        ),
        (
            r#"import { type A } from "m"; let x: A;"#,
            Import,
            r#"import { type A } from "m";"#,
        ),
        (r#"import "m";"#, Import, r#"import "m";"#),
        (
            r#"import { a } from "m";"#,
            Import,
            r#"import { a } from "m";"#,
        ),
        (
            r#"export type { A } from "m";"#,
            ExportFrom,
            r#"export type { A } from "m";"#,
        ),
        (
            r#"export { type A } from "m";"#,
            ExportFrom,
            r#"export { type A } from "m";"#,
        ),
        (
            r#"export {} from "m";"#,
            ExportFrom,
            r#"export {} from "m";"#,
        ),
        (
            r#"type T = import("m").A;"#,
            DynamicImport,
            r#"import("m")"#,
        ),
        (
            r#"import type S = require("m");"#,
            ImportEquals,
            r#"import type S = require("m");"#,
        ),
        (
            r#"export function f(require: (m: string) => void) { require("m"); }"#,
            Require,
            r#"require("m")"#,
        ),
    ];
    for (source, form, needle) in not_occurrences {
        let (start_byte, end_byte) = range(source, needle);
        assert_eq!(
            verdict(
                source,
                "m",
                [SourceEdgeSite {
                    form: *form,
                    start_byte,
                    end_byte
                }]
            ),
            Err(none(*form, start_byte, end_byte)),
            "{source}"
        );
        assert!(
            edges(source, "m").occurrences.is_empty(),
            "{source}: {:?}",
            edges(source, "m").occurrences
        );
    }
}

#[test]
fn a_runtime_namespace_import_is_unsupported_alone_and_beside_a_recorded_import() {
    let unsupported = |start_byte, end_byte| SourceEdgeMismatch::UnsupportedOccurrence {
        importer: A.into(),
        specifier: "m".into(),
        start_byte,
        end_byte,
    };
    let namespace = r#"import * as ns from "m"; ns.a;"#;
    let stmt = r#"import * as ns from "m";"#;
    let (start, end) = range(namespace, stmt);
    assert_eq!(
        verdict(namespace, "m", [site(Import, namespace, stmt)]),
        Err(unsupported(start, end))
    );
    assert!(edges(namespace, "m").occurrences.is_empty());
    // A mixed default-and-namespace statement is the same unsupported thing.
    let mixed = r#"import d, * as ns from "m"; d(); ns.a;"#;
    assert_eq!(edges(mixed, "m").unsupported.len(), 1);
    // Beside a recorded import of the same key: the denial is key-wide, so
    // it would silence the namespace import too.
    let both = r#"import { a } from "m"; import * as ns from "m"; a(); ns.b;"#;
    let (start, end) = range(both, r#"import * as ns from "m";"#);
    assert_eq!(
        verdict(both, "m", [site(Import, both, r#"import { a } from "m";"#)]),
        Err(unsupported(start, end))
    );
    // An unreferenced namespace import is no supported occurrence either,
    // and it is not inert: it is rejected with the other unreferenced
    // bindings (see the collateral test below).
    let elided = r#"import { a } from "m"; import * as ns from "m"; a();"#;
    assert_eq!(
        verdict(
            elided,
            "m",
            [site(Import, elided, r#"import { a } from "m";"#)]
        ),
        Err(collateral(elided, r#"import * as ns from "m";"#))
    );
}

#[test]
fn a_key_with_a_contract_bound_record_must_record_every_import_and_export_from_of_the_key() {
    let unrecorded = |form, source: &str, needle| {
        let (start_byte, end_byte) = range(source, needle);
        SourceEdgeMismatch::UnrecordedOccurrence {
            importer: A.into(),
            specifier: "m".into(),
            form,
            start_byte,
            end_byte,
        }
    };
    let two_imports = r#"import { a } from "m"; import { b } from "m"; a(); b();"#;
    let first = r#"import { a } from "m";"#;
    let second = r#"import { b } from "m";"#;
    assert_eq!(
        verdict(two_imports, "m", [site(Import, two_imports, first)]),
        Err(unrecorded(Import, two_imports, second))
    );
    assert_eq!(
        verdict(
            two_imports,
            "m",
            [
                site(Import, two_imports, first),
                site(Import, two_imports, second)
            ]
        ),
        Ok(())
    );
    let two_exports = r#"export { a } from "m"; export { b } from "m";"#;
    assert_eq!(
        verdict(
            two_exports,
            "m",
            [site(ExportFrom, two_exports, r#"export { a } from "m";"#)]
        ),
        Err(unrecorded(
            ExportFrom,
            two_exports,
            r#"export { b } from "m";"#
        ))
    );
    // Mixed: an import and an export-from of one key. Two imports, or two
    // exports, are not the only shape of incompleteness.
    let mixed = r#"import { a } from "m"; a(); export { b } from "m"; export * from "m";"#;
    let import = site(Import, mixed, r#"import { a } from "m";"#);
    let named = site(ExportFrom, mixed, r#"export { b } from "m";"#);
    let star = site(ExportFrom, mixed, r#"export * from "m";"#);
    assert_eq!(
        verdict(mixed, "m", [import]),
        Err(unrecorded(ExportFrom, mixed, r#"export { b } from "m";"#))
    );
    assert_eq!(
        verdict(mixed, "m", [import, named]),
        Err(unrecorded(ExportFrom, mixed, r#"export * from "m";"#))
    );
    assert_eq!(
        verdict(mixed, "m", [named, star]),
        Err(unrecorded(Import, mixed, r#"import { a } from "m";"#))
    );
    assert_eq!(verdict(mixed, "m", [import, named, star]), Ok(()));
}

#[test]
fn inert_type_only_and_side_effect_collateral_beside_a_denied_occurrence_stays_supported() {
    let source = r#"import { a } from "m"; import type { T } from "m"; import { type U } from "m"; import "m"; export type { V } from "m"; a();"#;
    let found = edges(source, "m");
    assert_eq!(found.occurrences.len(), 1);
    assert!(found.unreferenced.is_empty() && found.unsupported.is_empty());
    assert_eq!(
        (
            u64::from(found.occurrences[0].span.start),
            u64::from(found.occurrences[0].span.end)
        ),
        range(source, r#"import { a } from "m";"#)
    );
    assert_eq!(
        verdict(
            source,
            "m",
            [site(Import, source, r#"import { a } from "m";"#)]
        ),
        Ok(()),
        "these hold no binding the contract projection answers"
    );
}

/// R1-B1: the contract projection answers every non-type binding, referenced
/// or not, so a key-wide denial recorded for one occurrence must not silently
/// withhold what the analysis could report at another, unrecordable one.
#[test]
fn an_unreferenced_non_type_binding_of_a_denied_key_is_rejected_not_preserved() {
    let first = r#"import { run } from "m";"#;
    let idle = r#"import { idle } from "m";"#;
    let source = format!("{first} {idle} run();");
    let record = site(Import, &source, first);
    assert_eq!(
        verdict(&source, "m", [record]),
        Err(collateral(&source, idle))
    );
    // Named, default, aliased and mixed-with-a-type-only specifier alike.
    for unreferenced in [
        r#"import dflt from "m";"#,
        r#"import { idle as other } from "m";"#,
        r#"import { type T, idle } from "m";"#,
    ] {
        let source = format!("{first} {unreferenced} run();");
        let record = site(Import, &source, first);
        assert_eq!(
            verdict(&source, "m", [record]),
            Err(collateral(&source, unreferenced)),
            "{unreferenced}"
        );
    }
    // An unrecorded referenced statement is still reported as that, first.
    let both = format!("{first} {idle} import {{ extra }} from \"m\"; idle(); run(); extra();");
    assert!(matches!(
        verdict(&both, "m", [site(Import, &both, first)]),
        Err(SourceEdgeMismatch::UnrecordedOccurrence { .. })
    ));
    // The same statement holding the unreferenced binding is the recorded
    // occurrence itself: the record names it, so nothing is another's.
    let together = r#"import { run, idle } from "m"; run();"#;
    assert_eq!(
        verdict(
            together,
            "m",
            [site(Import, together, r#"import { run, idle } from "m";"#)]
        ),
        Ok(())
    );
    // Other keys are unaffected, and a literal-only record denies nothing, so
    // it needs no collateral rejection.
    let other = r#"import { run } from "m"; import { idle } from "other"; run();"#;
    assert_eq!(verdict(other, "m", [site(Import, other, first)]), Ok(()));
    let literal =
        r#"import { run } from "m"; import { idle } from "m"; run(); const x = import("m");"#;
    assert_eq!(
        verdict(
            literal,
            "m",
            [site(DynamicImport, literal, r#"import("m")"#)]
        ),
        Ok(())
    );
}

#[test]
fn duplicate_literals_are_two_occurrences_and_a_literal_only_record_denies_nothing() {
    let source = r#"import { a } from "m"; a(); const x = import("m"); const y = import("m"); import * as ns from "m"; ns.b;"#;
    let found = edges(source, "m");
    assert_eq!(
        found
            .occurrences
            .iter()
            .filter(|edge| edge.form == DynamicImport)
            .count(),
        2
    );
    let first = SourceEdgeSite {
        form: DynamicImport,
        start_byte: source.find(r#"import("m")"#).unwrap() as u64,
        end_byte: (source.find(r#"import("m")"#).unwrap() + r#"import("m")"#.len()) as u64,
    };
    // One of two equal literals recorded: fine, it is a record of one.
    let one = boundary(A, "m", [first]);
    assert_eq!(check(&one, &project(&[(A, source)])), Ok(()));
    // The same site recorded twice is one site.
    let mut twice = one.clone();
    twice.record_source_edge_refusal(ImportKey::new(A, "m"), first);
    assert_eq!(twice, one);
}

#[test]
fn an_import_equals_record_denies_nothing_and_an_exported_one_is_the_inner_declaration() {
    let source = r#"export import S = require("m"); S;"#;
    let found = edges(source, "m");
    assert_eq!(labels_of(&found), vec![(ImportEquals, names(&["*"]))]);
    let recorded = boundary(
        A,
        "m",
        [site(ImportEquals, source, r#"import S = require("m");"#)],
    );
    assert_eq!(check(&recorded, &project(&[(A, source)])), Ok(()));
}

#[test]
fn a_record_made_against_one_generation_joins_by_range_and_membership_only() {
    let v1 = r#"import { run } from "m"; run();"#;
    let record = boundary(A, "m", [site(Import, v1, r#"import { run } from "m";"#)]);
    // A leading comment shifts every range: no nearby statement matches.
    let shifted = format!("// c\n{v1}");
    assert!(matches!(
        check(&record, &project_at(2, &[(A, shifted.as_str())])),
        Err(SourceEdgeMismatch::NoSuchOccurrence { .. })
    ));
    // Equal text and range in a changed generation joins exactly as before.
    // This is the documented limit, not a guarantee: a record carries no
    // generation, so a caller reacquires its records for every coherent
    // facts generation. Here the code after the import changed, the record
    // did not, and the join cannot tell.
    let v2 = r#"import { run } from "m"; run(); run(() => 1);"#;
    assert_eq!(check(&record, &project_at(2, &[(A, v2)])), Ok(()));
}

#[test]
fn recorded_occurrences_come_back_in_source_order_across_forms_one_per_recorded_site() {
    let source = r#"const r = require("m"); import { a } from "m"; a(); export { b } from "m"; const d = import("m");"#;
    let recorded = boundary(
        A,
        "m",
        [
            site(Require, source, r#"require("m")"#),
            site(Import, source, r#"import { a } from "m";"#),
            site(ExportFrom, source, r#"export { b } from "m";"#),
        ],
    );
    let facts = project(&[(A, source)]);
    let forms = recorded_occurrences(&facts.files[0], &recorded)
        .into_iter()
        .map(|(specifier, edge)| (specifier, edge.form))
        .collect::<Vec<_>>();
    // The unrecorded dynamic import is not part of it; the order is the
    // source's, not the record's.
    assert_eq!(
        forms,
        vec![
            ("m".to_owned(), Require),
            ("m".to_owned(), Import),
            ("m".to_owned(), ExportFrom),
        ]
    );
    assert!(recorded_occurrences(&facts.files[0], &PackageBoundary::default()).is_empty());
}
