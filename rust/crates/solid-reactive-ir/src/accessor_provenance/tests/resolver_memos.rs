//! The resolver's memo build (S7b) through edits: the retained store's tables
//! equal a fresh store's at every step, verify-reused mode finds no reused
//! answer that missed a read, and the store's own check (shadow mode: the
//! memo build against the lookup's fresh construction, a mismatch reported
//! as a panic in tests) passes. A child of the base property tests, for
//! their synthetic Type Facts and edit kinds.

use super::*;

/// The dialect primitives the fixture calls, by their spelling.
const PRIMITIVES: &[&str] = &["createContext", "createMemo", "createSignal", "useContext"];

/// A handoff chain (`forward` hands `model` to `relay`, which hands it to
/// `finish`), a destructuring handoff, a context with one provider whose
/// value is an object, an accessor and a closed holder; and a module between
/// them in path order that the edits add and remove.
const RESOLVER_FILES: &[(&str, &str)] = &[
    (
        "/fixture/resolver/a.ts",
        "import { createContext, createMemo, useContext } from \"solid-js\";\nexport const Theme = createContext();\nexport const count = createMemo(() => 1);\nconst model = { read() { return 1; }, label: \"x\" };\nfunction forward(value) {\n  relay(value);\n}\nfunction relay(next) {\n  finish(next);\n}\nfunction finish(last) {\n  last.read();\n}\nforward(model);\nconst { read } = model;\nread();\nexport function useTheme() {\n  const theme = useContext(Theme);\n  return theme.color();\n}\nexport function use() {\n  return count();\n}\n",
    ),
    (
        "/fixture/resolver/c.tsx",
        "import { Theme, count } from \"./a\";\nconst theme = { color() { return \"x\"; } };\nexport function App() {\n  return <Theme value={theme}><div>{count()}</div></Theme>;\n}\nconst holder = { run: () => 1 };\nholder.run();\n",
    ),
];

/// The module the edits add between the other two in path order: it
/// declares a binding whose synthetic symbol `a.ts` declares too, so the
/// last-wins fold changes its answer.
const MIDDLE: (&str, &str) = (
    "/fixture/resolver/b.ts",
    "export const shared = { read() { return 2; } };\nconst model = 2;\nexport const label = \"x\";\n",
);

/// The fixture, with the middle module. The synthetic Type Facts list every
/// identifier as a reference of its spelling, declarations included, so a
/// parameter channel's reference census (exactly its one callee) holds only
/// where they list none: in `a.ts`.
fn resolver_project() -> Project {
    let mut files = RESOLVER_FILES
        .iter()
        .chain([&MIDDLE])
        .map(|(path, source)| ((*path).to_owned(), (*source).to_owned()))
        .collect::<Vec<_>>();
    files.sort();
    Project {
        files,
        schemes: HashMap::default(),
        unreferenced: vec!["/fixture/resolver/a.ts".to_owned()],
    }
}

/// `facts` with a valid resolved call at every identifier callee: the
/// synthetic table gives only member callees one, and a parameter channel
/// needs its call's.
fn with_identifier_calls(mut facts: ProjectFacts, project: &Project) -> ProjectFacts {
    let callees = facts
        .files
        .iter()
        .flat_map(|file| {
            file.ast
                .calls
                .iter()
                .filter(|call| {
                    file.ast
                        .identifiers
                        .binary_search_by_key(&call.callee, |id| id.span)
                        .is_ok()
                })
                .map(|call| (file.path.to_string(), call.callee))
        })
        .collect::<std::collections::BTreeSet<_>>();
    let resolved = Arc::new(
        serde_json::from_value::<typefacts::ResolvedCall>(serde_json::json!({
            "validity": "valid",
            "kind": "call",
        }))
        .expect("a resolved call"),
    );
    let mut entities = facts.typescript.entities().cloned().collect::<Vec<_>>();
    for entity in &mut entities {
        let (Ok(start), Ok(end)) = (
            u32::try_from(entity.location.start_byte),
            u32::try_from(entity.location.end_byte),
        ) else {
            continue;
        };
        if entity.resolved_call.is_none()
            && callees.contains(&(entity.location.path.to_string(), Span::new(start, end)))
        {
            entity.resolved_call = Some(Arc::clone(&resolved));
        }
    }
    let mut symbols = std::collections::BTreeMap::<String, Vec<serde_json::Value>>::new();
    for entity in &entities {
        if entity.symbol.is_empty() {
            continue;
        }
        let references = symbols.entry(entity.symbol.to_string()).or_default();
        let path = entity.location.path.as_ref();
        if !project.unreferenced.iter().any(|hidden| hidden == path) {
            references.push(serde_json::json!({
                "path": path,
                "startByte": entity.location.start_byte,
                "endByte": entity.location.end_byte,
            }));
        }
    }
    let symbols = symbols
        .into_iter()
        .map(|(id, references)| {
            serde_json::from_value::<typefacts::SymbolFact>(serde_json::json!({
                "id": id,
                "references": references,
            }))
            .expect("symbol")
        })
        .collect();
    facts.typescript =
        TypeScriptTable::from_parts(3, 1, "fixture", Vec::new(), entities, symbols, Vec::new());
    facts
}

/// One build's memo stats, by table.
type Stats = Vec<(&'static str, super::super::memo::TableStats)>;

/// The resolver's tables over `project` with `store`, and the memo stats.
fn resolver_build(
    project: &Project,
    store: &super::super::memo::build::MemoStore,
) -> Option<(Vec<String>, Stats)> {
    with_resolver_lookup(project, store, |facts, lookup, accessors| {
        let tables =
            crate::interproc::accessor_provenance::resolver_tables(facts, lookup, accessors);
        let stats = lookup
            .end_memos()
            .map(|report| report.tables)
            .unwrap_or_default();
        (tables, stats)
    })
}

/// Runs `f` on the fixture's facts, a lookup over `store`, and an accessor
/// per binding a `createMemo` call initializes.
fn with_resolver_lookup<R>(
    project: &Project,
    store: &super::super::memo::build::MemoStore,
    f: impl FnOnce(
        &ProjectFacts,
        &crate::indexes::SemanticLookup<'_>,
        &HashMap<SymbolId, (SymbolId, typefacts::Location)>,
    ) -> R,
) -> Option<R> {
    let facts = with_identifier_calls(facts(project)?, project);
    let ast_indexes = facts
        .files
        .iter()
        .map(|file| (file.path.clone(), CachedAstFileIndex::new(file)))
        .collect::<HashMap<_, _>>();
    let symbols = entity_symbols(&facts);
    // Every scheme's spelling of a primitive names it.
    let names = symbols
        .by_path
        .values()
        .flat_map(|rows| rows.values())
        .filter_map(|symbol| {
            let name = symbol.as_str().split_once(':')?.1;
            PRIMITIVES
                .contains(&name)
                .then(|| (symbol.clone(), SymbolId::from(name)))
        })
        .collect::<HashMap<_, _>>();
    let lookup = crate::indexes::SemanticLookup::new(
        &facts,
        &ast_indexes,
        &symbols,
        &names,
        &solid_dialect::Solid2,
        true,
    )
    .with_memo_store(store);
    // An accessor per binding a `createMemo` call initializes.
    let mut accessors = HashMap::default();
    for file in &facts.files {
        for binding in &file.ast.bindings {
            let Some(call) = binding
                .call_initializer
                .and_then(|span| file.ast.call_at(span))
            else {
                continue;
            };
            if file.source_text(call.callee) != Some("createMemo") {
                continue;
            }
            for name in &binding.names {
                if let Some(symbol) = symbols.at(file.path.as_str(), name.span) {
                    accessors.insert(
                        symbol.clone(),
                        (
                            symbol.clone(),
                            typefacts::Location {
                                path: file.path.shared(),
                                start_byte: u64::from(name.span.start),
                                end_byte: u64::from(name.span.end),
                            },
                        ),
                    );
                }
            }
        }
    }
    Some(f(&facts, &lookup, &accessors))
}

/// One fixture edit the resolver's seeds name: a mid-chain handoff removed
/// or restored, a member veto flipped, the middle module added or removed.
fn resolver_edit(project: &mut Project, kind: &str) -> Option<()> {
    fn file<'p>(project: &'p mut Project, path: &str) -> Option<&'p mut String> {
        project
            .files
            .iter_mut()
            .find(|(candidate, _)| candidate == path)
            .map(|(_, text)| text)
    }
    match kind {
        "remove-mid-edge" => {
            let text = file(project, "/fixture/resolver/a.ts")?;
            *text = if text.contains("finish(next);") {
                text.replace("finish(next);", "finish(nil);")
            } else {
                text.replace("finish(nil);", "finish(next);")
            };
        }
        "flip-veto" => {
            let text = file(project, "/fixture/resolver/c.tsx")?;
            *text = if text.contains("holder.read = ") {
                text.replace("holder.read = () => 2;\n", "")
            } else {
                format!("{text}holder.read = () => 2;\n")
            };
        }
        "flip-escape" => {
            let text = file(project, "/fixture/resolver/a.ts")?;
            *text = if text.contains("keep(model);") {
                text.replace("keep(model);\n", "")
            } else {
                format!("{text}keep(model);\n")
            };
        }
        "toggle-middle" => match project.files.iter().position(|(path, _)| path == MIDDLE.0) {
            Some(at) => {
                project.files.remove(at);
            }
            None => {
                project
                    .files
                    .push((MIDDLE.0.to_owned(), MIDDLE.1.to_owned()));
                project.files.sort();
            }
        },
        _ => return edit(project, kind, &mut Random(kind.len() as u32), 0),
    }
    Some(())
}

/// The fixture's tables: the chain hands `model` through to its receiver,
/// the context's one provider is found, and the accessor is a runtime origin.
#[test]
fn the_resolver_finds_the_fixture_tables() {
    let store = super::super::memo::build::MemoStore::new(true, true);
    let (tables, _) = resolver_build(&resolver_project(), &store).unwrap();
    let has = |needle: &str| tables.iter().any(|line| line.contains(needle));
    assert!(has("parameter"), "{tables:#?}");
    assert!(has("providers"), "{tables:#?}");
    assert!(has("runtime origin"), "{tables:#?}");
}

/// Through the seeds' edits, interleaved with the base harness's random
/// kinds, the retained store's resolver equals a fresh store's.
#[test]
fn resolver_memos_through_edits_equal_a_fresh_store() {
    let seeds = [
        "remove-mid-edge",
        "flip-veto",
        "toggle-middle",
        "flip-escape",
    ];
    let mut steps = 0;
    let (mut reused, mut computed) = (0, 0);
    for seed in 1..=3 {
        let mut random = Random(seed);
        let mut project = resolver_project();
        let store = super::super::memo::build::MemoStore::new(true, true);
        resolver_build(&project, &store).expect("the fixture parses");
        for serial in 0..24 {
            let kind = if random.below(2) == 0 {
                seeds[random.below(seeds.len())]
            } else {
                KINDS[random.below(KINDS.len())]
            };
            let mut next = project.clone();
            let applied = if seeds.contains(&kind) {
                resolver_edit(&mut next, kind)
            } else {
                edit(&mut next, kind, &mut random, serial)
            };
            if applied.is_none() || facts(&next).is_none() {
                continue;
            }
            project = next;
            let (tables, stats) = resolver_build(&project, &store).unwrap();
            let fresh = super::super::memo::build::MemoStore::new(false, false);
            let (expected, _) = resolver_build(&project, &fresh).unwrap();
            assert_eq!(tables, expected, "seed {seed} step {serial} ({kind})");
            for (table, stats) in &stats {
                assert_eq!(stats.mismatches, 0, "seed {seed} step {serial}: {table}");
                if table.starts_with("resolver_") {
                    reused += stats.reused + stats.verified;
                    computed += stats.fresh + stats.changed + stats.unchanged;
                }
            }
            steps += 1;
        }
    }
    eprintln!("resolver memos: {steps} edits, {reused} answers reused, {computed} computed");
    assert!(steps > 40, "only {steps} edits applied");
    assert!(reused > 0, "{reused} reused, {computed} computed");
}

/// Each seed kind changes the tables when applied and restores them when
/// applied again, and the retained store follows both ways.
#[test]
fn each_resolver_seed_moves_the_tables_and_back() {
    for kind in [
        "remove-mid-edge",
        "flip-veto",
        "toggle-middle",
        "flip-escape",
    ] {
        let store = super::super::memo::build::MemoStore::new(true, true);
        let mut project = resolver_project();
        let (before, _) = resolver_build(&project, &store).unwrap();
        resolver_edit(&mut project, kind).unwrap();
        let (during, _) = resolver_build(&project, &store).unwrap();
        let fresh = super::super::memo::build::MemoStore::new(false, false);
        assert_eq!(
            during,
            resolver_build(&project, &fresh).unwrap().0,
            "{kind}"
        );
        resolver_edit(&mut project, kind).unwrap();
        let (after, _) = resolver_build(&project, &store).unwrap();
        assert_eq!(before, after, "{kind}");
        if kind != "flip-veto" {
            assert_ne!(before, during, "{kind} moves no table");
        }
    }
}

/// A body edit of the module no walk visits reruns only its own stage
/// memos: the other modules' bindings, census and origins are reused or
/// verified. (An edit of `c.tsx` reruns `a.ts`'s origins too: its context
/// read walks to the provider's value there.)
#[test]
fn a_body_edit_reuses_the_other_modules_stages() {
    let store = super::super::memo::build::MemoStore::new(false, true);
    let mut project = resolver_project();
    resolver_build(&project, &store).unwrap();
    let (_, text) = project
        .files
        .iter_mut()
        .find(|(path, _)| path == MIDDLE.0)
        .unwrap();
    *text = text.replacen("\"x\"", "\"y\"", 1);
    let (tables, stats) = resolver_build(&project, &store).unwrap();
    let fresh = super::super::memo::build::MemoStore::new(false, false);
    assert_eq!(tables, resolver_build(&project, &fresh).unwrap().0);
    for table in ["resolver_census", "resolver_origins", "resolver_bindings"] {
        let found = table_stats(&stats, table);
        assert_eq!(found.reused + found.verified, 2, "{table}: {found:?}");
        assert_eq!(
            found.fresh + found.changed + found.unchanged,
            1,
            "{table}: {found:?}"
        );
    }
}

/// A proof that read the resolver's memos is undecided until this build's
/// memo construction serves: those memos read derived tables, and one clean
/// by its stamp would answer before the pass that may move its keys
/// publishes (final review R2, finding 6a). Once served, it holds.
#[test]
fn a_proof_of_resolver_reads_is_undecided_until_the_resolver_serves() {
    use crate::base::reads::{ReadProof, Verdict};
    let store = super::super::memo::build::MemoStore::new(false, true);
    let project = resolver_project();
    let proof = with_resolver_lookup(&project, &store, |facts, lookup, accessors| {
        let (_, reads) = crate::base::reads::record(|| {
            crate::interproc::accessor_provenance::resolver_tables(facts, lookup, accessors)
        });
        assert!(
            reads.memos().iter().any(|read| {
                crate::interproc::accessor_provenance::RESOLVER_TABLES.contains(&read.table.0)
            }),
            "the construction answers resolver memos"
        );
        let proof = ReadProof::new(reads, lookup);
        lookup.end_memos();
        proof
    })
    .unwrap();
    with_resolver_lookup(&project, &store, |facts, lookup, accessors| {
        lookup.begin_memos();
        assert!(
            matches!(proof.verdict(lookup), Verdict::Undecided(_)),
            "checked before the resolver's passes publish"
        );
        let _ = crate::interproc::accessor_provenance::resolver_tables(facts, lookup, accessors);
        assert_eq!(proof.verdict(lookup), Verdict::Holds, "after they publish");
        lookup.end_memos();
    })
    .unwrap();
}
