//! Property tests of the incremental base: after any sequence of edits, the
//! updated indexes equal a refold from scratch, and each update's Δ is
//! exactly the set of keys whose values changed.

use std::sync::{Arc, OnceLock};

use solid_facts::compiler::{COMPILER_FACTS_PROTOCOL, ExecutionMap};
use solid_facts::core::{Generation, SourceHash, SourcePath, Span};
use solid_facts::{FileFacts, ProjectFacts, TypeScriptTable};
use typefacts::{EntityFact, SymbolFact};

use super::file_table::FileId;
use super::index::{BaseIndex, FileContribution};
use super::retained::{self, RetainedBase};
use super::{BuildConstants, ViewSources, Views};
use crate::indexes::{CachedAstFileIndex, EntitySymbols};
use crate::{HashMap, SymbolId};

/// Mulberry32, as the edit harness uses: a failing run replays from its seed.
struct Random(u32);

impl Random {
    fn below(&mut self, n: usize) -> usize {
        self.0 = self.0.wrapping_add(0x6d2b_79f5);
        let mut t = self.0;
        t = (t ^ (t >> 15)).wrapping_mul(t | 1);
        t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
        let unit = f64::from(t ^ (t >> 14)) / 4_294_967_296.0;
        // Truncation is the point: a uniform index below `n`.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let index = (unit * n as f64) as usize;
        index.min(n.saturating_sub(1))
    }
}

// ---------------------------------------------------------------------------
// BaseIndex alone: random contributions, random replacements.

fn random_contribution(random: &mut Random) -> FileContribution<u8, u8> {
    let slots = 1 + usize::from(random.below(5) == 0);
    FileContribution::from_slots((0..slots).map(|_| {
        (0..random.below(6))
            .map(|_| {
                (
                    u8::try_from(random.below(8)).unwrap(),
                    u8::try_from(random.below(3)).unwrap(),
                )
            })
            .collect()
    }))
}

#[test]
fn a_base_index_update_equals_a_refold_and_reports_exactly_the_changed_keys() {
    for seed in 1..=300 {
        let mut random = Random(seed);
        let mut files = HashMap::<FileId, FileContribution<u8, u8>>::default();
        let mut interner = super::file_table::FileInterner::default();
        let ids = (0..8)
            .map(|index| interner.intern(&format!("f{index}.ts")))
            .collect::<Vec<_>>();
        for id in &ids {
            if random.below(3) > 0 {
                files.insert(*id, random_contribution(&mut random));
            }
        }
        let mut index = BaseIndex::fold(files.clone());
        for _ in 0..20 {
            let mut changes = Vec::new();
            for _ in 0..=random.below(3) {
                let id = ids[random.below(ids.len())];
                let next = (random.below(4) > 0).then(|| random_contribution(&mut random));
                match &next {
                    Some(contribution) => files.insert(id, contribution.clone()),
                    None => files.remove(&id),
                };
                changes.push((id, next));
            }
            let before = index.clone();
            let delta = index.update(changes);
            let refold = BaseIndex::fold(files.clone());
            assert!(index == refold, "seed {seed}: update differs from refold");
            assert_eq!(
                delta,
                before.changed_keys(&refold),
                "seed {seed}: inexact delta"
            );
            for key in 0..8u8 {
                assert!(index.values(&key).eq(refold.values(&key)));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// The whole base over fixture projects: random source edits, file adds and
// removes, and TypeFacts-only changes, checked through the same path shadow
// mode takes ([`retained::check`]).

/// The fixture projects the edit harness edits (scripts/ir-edit-fuzz.mjs),
/// and a few more with module and member traffic.
const FIXTURES: &[&str] = &[
    "reactive-ir/callback-forwarding",
    "reactive-ir/accessor-provenance",
    "reactive-ir/async-boundary",
    "reactive-ir/deferred-effect-apply",
    "reactive-ir/call-valued-callee-identity",
    "reactive-ir/callback-props-reads",
];

fn fixture_sources(fixture: &str) -> Vec<(String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../fixtures")
        .join(fixture);
    let mut found = Vec::new();
    let mut stack = vec![root.clone()];
    while let Some(directory) = stack.pop() {
        for entry in std::fs::read_dir(&directory).expect("fixture directory") {
            let path = entry.expect("fixture entry").path();
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            if path.is_dir() {
                if !name.starts_with('.') && name != "node_modules" && name != "variants" {
                    stack.push(path);
                }
            } else if (name.ends_with(".ts") || name.ends_with(".tsx")) && !name.ends_with(".d.ts")
            {
                let relative = path.strip_prefix(&root).unwrap().to_string_lossy();
                found.push((
                    format!("/fixture/{relative}"),
                    std::fs::read_to_string(&path).expect("fixture source"),
                ));
            }
        }
    }
    found.sort();
    found
}

/// A project under edit: its sources, and per path the spelling of the
/// synthetic TypeFacts symbol ids (a TypeFacts-only change re-spells them).
#[derive(Clone)]
struct Project {
    files: Vec<(String, String)>,
    schemes: HashMap<String, u32>,
    /// Paths whose references TypeFacts does not list (a references-only
    /// change: the entities stay).
    unreferenced: Vec<String>,
}

fn location(path: &str, span: Span) -> serde_json::Value {
    serde_json::json!({
        "path": path,
        "startByte": span.start,
        "endByte": span.end,
    })
}

/// A synthetic TypeFacts table: every identifier is an entity whose symbol
/// is its spelling (so references cross files), and member callees spelled
/// like the hazards' library calls carry a standard-library resolved call.
fn typescript(
    files: &[FileFacts],
    schemes: &HashMap<String, u32>,
    unreferenced: &[String],
) -> TypeScriptTable {
    let mut entities = Vec::<EntityFact>::new();
    let mut symbols = std::collections::BTreeMap::<String, Vec<serde_json::Value>>::new();
    for file in files {
        let path = file.path.as_str();
        let scheme = schemes.get(path).copied().unwrap_or(0);
        for identifier in &file.ast.identifiers {
            let Some(text) = file.source_text(identifier.span) else {
                continue;
            };
            let symbol = format!("s{scheme}:{text}");
            entities.push(
                serde_json::from_value(serde_json::json!({
                    "location": location(path, identifier.span),
                    "symbol": symbol,
                    "symbolClaim": "covering",
                }))
                .expect("entity"),
            );
            let references = symbols.entry(symbol).or_default();
            if !unreferenced.iter().any(|hidden| hidden == path) {
                references.push(location(path, identifier.span));
            }
        }
        for call in &file.ast.calls {
            let Some(member) = file.ast.member_at(call.callee) else {
                continue;
            };
            let qualified = match file.source_text(member.property) {
                Some("dispatchEvent") => "EventTarget.dispatchEvent",
                Some("assign") => "ObjectConstructor.assign",
                Some("defineProperty") => "ObjectConstructor.defineProperty",
                _ => continue,
            };
            entities.push(
                serde_json::from_value(serde_json::json!({
                    "location": location(path, call.callee),
                    "resolvedCall": {
                        "validity": "valid",
                        "kind": "call",
                        "declaration": {
                            "kind": "method",
                            "location": location("lib.d.ts", Span::new(0, 1)),
                            "qualifiedName": qualified,
                            "sourceFile": "lib.d.ts",
                            "standardLibrary": true,
                        },
                    },
                }))
                .expect("entity with a resolved call"),
            );
        }
    }
    entities.sort_by(|left, right| {
        (
            &left.location.path,
            left.location.start_byte,
            left.location.end_byte,
        )
            .cmp(&(
                &right.location.path,
                right.location.start_byte,
                right.location.end_byte,
            ))
    });
    let symbols = symbols
        .into_iter()
        .map(|(id, references)| {
            serde_json::from_value::<SymbolFact>(serde_json::json!({
                "id": id,
                "references": references,
            }))
            .expect("symbol")
        })
        .collect();
    TypeScriptTable::from_parts(3, 1, "fixture", Vec::new(), entities, symbols, Vec::new())
}

fn facts(project: &Project) -> Option<ProjectFacts> {
    let generation = Generation::new(1).unwrap();
    let mut files = Vec::new();
    for (path, source) in &project.files {
        let ast = solid_facts::ast::extract(path, source).ok()?;
        files.push(FileFacts {
            generation,
            path: SourcePath::new(path).unwrap(),
            source_hash: SourceHash::of(source),
            source: Arc::from(source.as_str()),
            ast: Arc::new(ast),
            compiler: Arc::new(ExecutionMap {
                compiler_facts_protocol: COMPILER_FACTS_PROTOCOL,
                source_hash: SourceHash::of(source),
                semantic_model: Default::default(),
                tracked_regions: Vec::new(),
                untracked_regions: Vec::new(),
                discarded_regions: Vec::new(),
                ownership_regions: Vec::new(),
                callback_roles: Vec::new(),
                jsx_operations: Vec::new(),
            }),
        });
    }
    let typescript = typescript(&files, &project.schemes, &project.unreferenced);
    Some(ProjectFacts {
        generation,
        project_id: "fixture".into(),
        files,
        typescript,
        typescript_changes: None,
        resolved_imports: None,
        runtime_resolutions: None,
        runtime_symbol_redirects: std::collections::HashMap::default(),
        dependency_source_roots: Vec::new(),
        dependency_source_call_errors: Vec::new(),
    })
}

fn entity_symbols(facts: &ProjectFacts) -> EntitySymbols {
    let mut symbols = EntitySymbols {
        by_path: HashMap::default(),
        exact_by_path: HashMap::default(),
    };
    for entity in facts.typescript.entities() {
        if entity.symbol.is_empty() {
            continue;
        }
        let key = (entity.location.start_byte, entity.location.end_byte);
        let path = entity.location.path.to_string();
        let symbol = SymbolId::from(entity.symbol.as_ref());
        symbols
            .exact_by_path
            .entry(path.clone())
            .or_default()
            .insert(key, symbol.clone());
        symbols.by_path.entry(path).or_default().insert(key, symbol);
    }
    symbols
}

/// One shadow check of `project` against `retained`: the update must equal
/// the refold, with an exact Δ. Returns how many files changed.
fn check(project: &Project, retained: &mut Option<RetainedBase>) -> Option<usize> {
    let facts = facts(project)?;
    let ast_indexes = facts
        .files
        .iter()
        .map(|file| (file.path.clone(), CachedAstFileIndex::new(file)))
        .collect::<HashMap<_, _>>();
    let symbols = entity_symbols(&facts);
    let entities = crate::indexes::first_entity_by_location(facts.typescript.entities());
    let references = OnceLock::new();
    let table = retained::table(retained, &facts.files);
    let views = Views {
        table: &table,
        files: &facts.files,
        sources: ViewSources {
            ast_indexes: &ast_indexes,
            symbols: &symbols,
            entities: &entities,
            typescript: &facts.typescript,
            references: &references,
        },
        constants: BuildConstants {
            dialect: &solid_dialect::Solid2,
        },
    };
    let found = retained::check(retained, &views);
    if let Some(found) = &found {
        assert!(
            found.mismatch.is_none(),
            "{}",
            found.mismatch.as_deref().unwrap_or("")
        );
    }
    Some(found.map_or(0, |found| found.changed_files + found.removed_files))
}

const KINDS: &[&str] = &[
    "body",
    "prefix",
    "delete-line",
    "add-import",
    "remove-import",
    "rename-export",
    "add-component",
    "add-eval",
    "add-prototype-write",
    "add-dispatch",
    "add-assign",
    "add-member-write",
    "add-file",
    "remove-file",
    "typefacts-only",
    "typefacts-references",
];

/// The edit harness's edit kinds (scripts/ir-edit-fuzz.mjs) plus file adds
/// and removes, hazard-flipping additions, and a TypeFacts-only change.
/// Returns `None` when the kind has no site in the chosen file.
fn edit(project: &mut Project, kind: &str, random: &mut Random, serial: usize) -> Option<()> {
    if kind == "add-file" {
        project.files.push((
            format!("/fixture/added{serial}.ts"),
            format!(
                "import {{ helper{serial} }} from \"./missing\";\nexport const value{serial} = helper{serial}();\n"
            ),
        ));
        return Some(());
    }
    if project.files.is_empty() {
        return None;
    }
    let index = random.below(project.files.len());
    if kind == "remove-file" {
        (project.files.len() > 1).then(|| {
            project.files.remove(index);
        })?;
        return Some(());
    }
    let others = project
        .files
        .iter()
        .enumerate()
        .filter(|(other, _)| *other != index)
        .map(|(_, (path, _))| path.clone())
        .collect::<Vec<_>>();
    let (path, text) = &mut project.files[index];
    if kind == "typefacts-references" {
        match project
            .unreferenced
            .iter()
            .position(|hidden| hidden == path)
        {
            Some(at) => {
                project.unreferenced.remove(at);
            }
            None => project.unreferenced.push(path.clone()),
        }
        return Some(());
    }
    if kind == "typefacts-only" {
        *project.schemes.entry(path.clone()).or_default() += 1;
        return Some(());
    }
    let next = match kind {
        "body" => {
            let quote = text.find('"')?;
            let end = quote + 1 + text[quote + 1..].find('"')?;
            (end > quote + 1).then_some(())?;
            let mut bytes = text.clone().into_bytes();
            bytes[quote + 1] = if bytes[quote + 1] == b'x' { b'y' } else { b'x' };
            String::from_utf8(bytes).ok()?
        }
        "prefix" => format!("// fuzz {}\n{text}", random.below(1_000_000)),
        "delete-line" => {
            let lines = text.split('\n').collect::<Vec<_>>();
            let candidates = (0..lines.len())
                .filter(|line| !lines[*line].trim().is_empty())
                .collect::<Vec<_>>();
            (!candidates.is_empty()).then_some(())?;
            let removed = candidates[random.below(candidates.len())];
            lines
                .iter()
                .enumerate()
                .filter(|(line, _)| *line != removed)
                .map(|(_, line)| *line)
                .collect::<Vec<_>>()
                .join("\n")
        }
        "add-import" => {
            let target = others.get(random.below(others.len().max(1)))?;
            let specifier = format!(
                "./{}",
                target
                    .trim_start_matches("/fixture/")
                    .trim_end_matches(".tsx")
                    .trim_end_matches(".ts")
            );
            if random.below(2) == 0 {
                format!(
                    "import * as __fz{serial} from \"{specifier}\";\nvoid __fz{serial};\n{text}"
                )
            } else {
                format!("import \"{specifier}\";\n{text}")
            }
        }
        "remove-import" => {
            let lines = text.split('\n').collect::<Vec<_>>();
            let imports = (0..lines.len())
                .filter(|line| {
                    lines[*line].starts_with("import ") && lines[*line].contains(" from ")
                })
                .collect::<Vec<_>>();
            (!imports.is_empty()).then_some(())?;
            let removed = imports[random.below(imports.len())];
            lines
                .iter()
                .enumerate()
                .filter(|(line, _)| *line != removed)
                .map(|(_, line)| *line)
                .collect::<Vec<_>>()
                .join("\n")
        }
        "rename-export" => {
            let at = text.find("export function ")? + "export function ".len();
            let name_end = at + text[at..].find(|c: char| !c.is_alphanumeric() && c != '_')?;
            format!(
                "{}{}_fz{serial}{}",
                &text[..at],
                &text[at..name_end],
                &text[name_end..]
            )
        }
        "add-component" => {
            if path.ends_with(".tsx") {
                format!(
                    "{text}\nexport function FzComp{serial}(props: {{ v: number }}) {{\n  return <div>{{props.v}}</div>;\n}}\n"
                )
            } else {
                format!(
                    "{text}\nexport function fzHelper{serial}(value: number) {{\n  return value + 1;\n}}\n"
                )
            }
        }
        "add-eval" => {
            format!("{text}\nexport const __fzEval{serial} = (code: string) => eval(code);\n")
        }
        "add-prototype-write" => format!("{text}\n(Object.prototype as any).__fz{serial} = 1;\n"),
        "add-dispatch" => {
            format!("{text}\ndocument.body.dispatchEvent(new Event(\"fz{serial}\"));\n")
        }
        "add-assign" => format!("{text}\nObject.assign(Object.prototype, {{ fz{serial}: 1 }});\n"),
        "add-member-write" => format!(
            "{text}\nconst fzHolder{serial}: any = {{}};\nfzHolder{serial}.run{serial} = () => 1;\ndelete fzHolder{serial}.gone;\n"
        ),
        _ => unreachable!("edit kind {kind}"),
    };
    *text = next;
    Some(())
}

#[test]
fn base_indexes_updated_through_random_edits_equal_a_refold() {
    let mut steps = 0;
    let mut changed_steps = 0;
    for fixture in FIXTURES {
        let original = Project {
            files: fixture_sources(fixture),
            schemes: HashMap::default(),
            unreferenced: Vec::new(),
        };
        assert!(!original.files.is_empty(), "{fixture} has sources");
        for seed in 1..=4 {
            let mut random = Random(seed);
            let mut project = original.clone();
            let mut retained = None;
            check(&project, &mut retained).expect("the fixture parses");
            for serial in 0..25 {
                let kind = KINDS[random.below(KINDS.len())];
                let mut next = project.clone();
                if edit(&mut next, kind, &mut random, serial).is_none() {
                    continue;
                }
                // An unparseable edit is skipped, as the harness skips it.
                let mut candidate = retained.take();
                let Some(changed) = check(&next, &mut candidate) else {
                    retained = candidate;
                    continue;
                };
                retained = candidate;
                project = next;
                steps += 1;
                if changed > 0 {
                    changed_steps += 1;
                }
            }
        }
    }
    eprintln!("base property test: {steps} edits applied, {changed_steps} changed some file");
    assert!(steps > 200, "only {steps} edits applied");
    assert!(
        changed_steps * 10 > steps * 8,
        "{changed_steps} of {steps} edits changed a file"
    );
}

#[test]
fn an_unchanged_project_updates_nothing() {
    let project = Project {
        files: fixture_sources(FIXTURES[1]),
        schemes: HashMap::default(),
        unreferenced: Vec::new(),
    };
    let mut retained = None;
    check(&project, &mut retained);
    assert_eq!(check(&project, &mut retained), Some(0));
    let mut retyped = project.clone();
    retyped.schemes.insert(project.files[0].0.clone(), 7);
    assert_eq!(check(&retyped, &mut retained), Some(1));
}

#[test]
fn a_contribution_reads_only_its_own_file() {
    // The same file in two projects contributes the same entries: nothing
    // another file holds can reach a contribution.
    let alone = Project {
        files: fixture_sources(FIXTURES[1])[..1].to_vec(),
        schemes: HashMap::default(),
        unreferenced: Vec::new(),
    };
    let together = Project {
        files: fixture_sources(FIXTURES[1]),
        schemes: HashMap::default(),
        unreferenced: Vec::new(),
    };
    let first = |project: &Project| {
        let facts = facts(project).unwrap();
        let ast_indexes = facts
            .files
            .iter()
            .map(|file| (file.path.clone(), CachedAstFileIndex::new(file)))
            .collect::<HashMap<_, _>>();
        let symbols = entity_symbols(&facts);
        let entities = crate::indexes::first_entity_by_location(facts.typescript.entities());
        let references = OnceLock::new();
        let table = super::FileTable::new(&facts.files);
        let views = Views {
            table: &table,
            files: &facts.files,
            sources: ViewSources {
                ast_indexes: &ast_indexes,
                symbols: &symbols,
                entities: &entities,
                typescript: &facts.typescript,
                references: &references,
            },
            constants: BuildConstants {
                dialect: &solid_dialect::Solid2,
            },
        };
        let id = table.slot_id(0);
        (
            format!(
                "{:?}",
                views.contribution(id, super::contributions::modules)
            ),
            format!(
                "{:?}",
                views.contribution(id, super::contributions::members)
            ),
            format!(
                "{:?}",
                views.contribution(id, super::contributions::hazards)
            ),
            format!(
                "{:?}",
                views.contribution(id, super::contributions::references)
            ),
            format!(
                "{:?}",
                views.contribution(id, super::contributions::callees)
            ),
            views.view(0).fingerprint(),
        )
    };
    assert_eq!(first(&alone), first(&together));
}

// ---------------------------------------------------------------------------
// L2: the identity-wrapper memo over a retained store ([`super::memo`]).

/// Wrappers across files: `wrap` and `keep` return their parameter, the
/// others do not; `use.ts` calls them from another file.
const WRAPPER_FILES: &[(&str, &str)] = &[
    (
        "/fixture/wrap.ts",
        "export function wrap(cb: () => void) {\n  return cb;\n}\nexport const keep = (value: number) => value;\nexport function notWrapper(cb: () => void) {\n  cb();\n  return cb;\n}\nexport async function later(cb: () => void) {\n  return cb;\n}\n",
    ),
    (
        "/fixture/use.ts",
        "import { wrap, keep } from \"./wrap\";\nexport const run = wrap(() => keep(1));\nexport const label = \"x\";\n",
    ),
];

/// What one memo build answered and found.
struct MemoBuild {
    answers: Vec<(Arc<str>, Option<usize>)>,
    /// The per-symbol table's stats.
    stats: super::memo::TableStats,
    /// The per-function table's stats.
    functions: super::memo::TableStats,
    begin: super::memo::build::BeginReport,
}

/// One build of `project` over the retained `store`: every candidate's
/// answer (plus one absent symbol), and the build's table stats.
fn memo_build(project: &Project, store: &super::memo::build::MemoStore) -> Option<MemoBuild> {
    use crate::indexes::IdentityWrapper;
    let facts = facts(project)?;
    let ast_indexes = facts
        .files
        .iter()
        .map(|file| (file.path.clone(), CachedAstFileIndex::new(file)))
        .collect::<HashMap<_, _>>();
    let symbols = entity_symbols(&facts);
    let entities = crate::indexes::first_entity_by_location(facts.typescript.entities());
    let references = OnceLock::new();
    let table = store.table(&super::FileTable::new(&facts.files));
    let views = Views {
        table: &table,
        files: &facts.files,
        sources: ViewSources {
            ast_indexes: &ast_indexes,
            symbols: &symbols,
            entities: &entities,
            typescript: &facts.typescript,
            references: &references,
        },
        constants: BuildConstants {
            dialect: &solid_dialect::Solid2,
        },
    };
    let begin = store.begin(&views, false, &super::memo::probes::NoProbes);
    let world =
        super::memo::build::BuildWorld::new(views, store, false, &super::memo::probes::NoProbes);
    let mut keys = store.wrapper_candidate_keys();
    keys.push(Arc::from("s0:absent"));
    let answers = keys
        .into_iter()
        .map(|key| {
            let answer = world.answer::<IdentityWrapper>(&key);
            (key, answer)
        })
        .collect();
    let tables = store.end();
    let stats = |table: &str| {
        tables
            .iter()
            .find(|(name, _)| *name == table)
            .map(|(_, stats)| *stats)
            .unwrap_or_default()
    };
    Some(MemoBuild {
        answers,
        stats: stats("identity_wrappers"),
        functions: stats("identity_wrapper_functions"),
        begin,
    })
}

fn wrapper_project(fixture: &str) -> Project {
    let mut files = fixture_sources(fixture);
    files.extend(
        WRAPPER_FILES
            .iter()
            .map(|(path, source)| ((*path).to_owned(), (*source).to_owned())),
    );
    files.sort();
    Project {
        files,
        schemes: HashMap::default(),
        unreferenced: Vec::new(),
    }
}

#[test]
fn identity_wrappers_are_found_across_files() {
    let store = super::memo::build::MemoStore::new(false, true);
    let build = memo_build(&wrapper_project(FIXTURES[0]), &store).unwrap();
    let answer = |symbol: &str| {
        build
            .answers
            .iter()
            .find(|(key, _)| key.as_ref() == symbol)
            .map(|(_, answer)| *answer)
    };
    assert_eq!(answer("s0:wrap"), Some(Some(0)));
    assert_eq!(answer("s0:keep"), Some(Some(0)));
    assert_eq!(answer("s0:notWrapper"), Some(None));
    // Async: not a candidate at all.
    assert_eq!(answer("s0:later"), None);
    assert_eq!(answer("s0:absent"), Some(None));
    assert_eq!(build.stats.reused + build.stats.verified, 0);
}

/// With stable symbol ids (the synthetic table spells a symbol by its text),
/// an edit that leaves a wrapper's file and index entries alone reuses its
/// answer; an edit that moves the wrapper's spans recomputes its function's
/// answer to the same value, and the symbol's answer is verified unrun.
#[test]
fn identity_wrapper_answers_are_reused_when_ids_are_stable() {
    let store = super::memo::build::MemoStore::new(false, true);
    let mut project = wrapper_project(FIXTURES[0]);
    let first = memo_build(&project, &store).unwrap();
    assert!(first.stats.fresh > 2);
    // A body edit of the caller: the same byte length, no span moves.
    let caller = project
        .files
        .iter()
        .position(|(path, _)| path == "/fixture/use.ts")
        .unwrap();
    project.files[caller].1 = project.files[caller].1.replace("\"x\"", "\"y\"");
    let second = memo_build(&project, &store).unwrap();
    assert_eq!(first.answers, second.answers);
    assert_eq!(second.begin.changed_files, 1);
    let stats = second.stats;
    assert_eq!(
        (
            stats.reused,
            stats.fresh + stats.changed + stats.unchanged + stats.verified
        ),
        (first.answers.len() as u64, 0),
        "{stats:?}"
    );
    // A prefix edit of the wrappers' file moves every span in it: the L1
    // entries (symbol, ordinal) stay, each function's answer is recomputed
    // to the same value, and each symbol's answer is verified (cutoff).
    let wrappers = project
        .files
        .iter()
        .position(|(path, _)| path == "/fixture/wrap.ts")
        .unwrap();
    project.files[wrappers].1 = format!("// moved\n{}", project.files[wrappers].1);
    let third = memo_build(&project, &store).unwrap();
    assert_eq!(first.answers, third.answers);
    assert_eq!(third.begin.delta_keys, 0);
    let (stats, functions) = (third.stats, third.functions);
    assert!(functions.unchanged >= 3, "{functions:?}");
    assert_eq!(functions.changed + functions.fresh, 0, "{functions:?}");
    assert!(stats.verified >= 3, "{stats:?}");
    assert_eq!(
        stats.changed + stats.unchanged + stats.fresh,
        0,
        "{stats:?}"
    );
}

/// Through random edits (the base property test's kinds, TypeFacts-only
/// changes included), the retained store answers exactly as a fresh store,
/// with verify-reused mode on: no reused answer missed a read.
#[test]
fn identity_wrapper_memos_through_random_edits_equal_a_fresh_store() {
    let mut steps = 0;
    let (mut reused, mut computed) = (0, 0);
    for fixture in FIXTURES {
        let original = wrapper_project(fixture);
        for seed in 1..=3 {
            let mut random = Random(seed);
            let mut project = original.clone();
            let store = super::memo::build::MemoStore::new(true, true);
            memo_build(&project, &store).expect("the fixture parses");
            for serial in 0..20 {
                let kind = KINDS[random.below(KINDS.len())];
                let mut next = project.clone();
                if edit(&mut next, kind, &mut random, serial).is_none() || facts(&next).is_none() {
                    continue;
                }
                project = next;
                let build = memo_build(&project, &store).unwrap();
                let fresh = super::memo::build::MemoStore::new(false, false);
                let expected = memo_build(&project, &fresh).unwrap();
                assert_eq!(
                    build.answers, expected.answers,
                    "{fixture} seed {seed} step {serial} ({kind})"
                );
                assert_eq!(
                    build.stats.mismatches + build.functions.mismatches,
                    0,
                    "{fixture} seed {seed} step {serial}"
                );
                steps += 1;
                for stats in [build.stats, build.functions] {
                    reused += stats.reused + stats.verified;
                    computed += stats.fresh + stats.changed + stats.unchanged;
                }
            }
        }
    }
    eprintln!(
        "identity-wrapper memos: {steps} edits, {reused} answers reused, {computed} computed"
    );
    assert!(steps > 150, "only {steps} edits applied");
    assert!(reused > computed, "{reused} reused, {computed} computed");
}

// ---------------------------------------------------------------------------
// L2: the component census over a retained store (study S5c,
// [`crate::indexes::ComponentStatusAt`]).

/// A function's `(path, span)` and its call sites' `(caller, span)`.
type FunctionSites = (String, Span, Vec<(String, Span)>);

/// Every census answer of one build, by path and span, read through the
/// lookup as its consumers read them.
#[derive(Debug, PartialEq)]
struct CensusAnswers {
    /// Each function's status, and whether it is an escaped component.
    statuses: Vec<(String, Span, crate::indexes::ComponentStatus, bool)>,
    /// Each function's call-site references, as `(caller, span)`.
    sites: Vec<FunctionSites>,
    /// The functions some tag renders.
    rendered: Vec<(String, Span)>,
    /// Each file's lazy `default` witnesses that hold.
    witnesses: Vec<(String, Span, Span)>,
    /// The result-access forwarded arguments, by caller.
    forwarded: Vec<(String, Vec<Span>)>,
}

/// One build of `project` over `store` through a lookup: every census
/// answer, and the build's memo stats by table.
fn census_build(
    project: &Project,
    store: &super::memo::build::MemoStore,
    closed: bool,
) -> Option<(CensusAnswers, Vec<(&'static str, super::memo::TableStats)>)> {
    let facts = facts(project)?;
    let ast_indexes = facts
        .files
        .iter()
        .map(|file| (file.path.clone(), CachedAstFileIndex::new(file)))
        .collect::<HashMap<_, _>>();
    let symbols = entity_symbols(&facts);
    let names = HashMap::default();
    let lookup = crate::indexes::SemanticLookup::new(
        &facts,
        &ast_indexes,
        &symbols,
        &names,
        &solid_dialect::Solid2,
        closed,
    )
    .with_memo_store(store);
    let mut answers = CensusAnswers {
        statuses: Vec::new(),
        sites: Vec::new(),
        rendered: Vec::new(),
        witnesses: Vec::new(),
        forwarded: Vec::new(),
    };
    for file in lookup.files() {
        let path = file.path.as_str();
        for function in &file.ast.functions {
            answers.statuses.push((
                path.to_owned(),
                function.span,
                lookup.function_component_status(file, function),
                lookup
                    .escaped_component_functions()
                    .contains(&(path, function.span)),
            ));
            answers.sites.push((
                path.to_owned(),
                function.span,
                lookup
                    .function_call_site_references(path, function.span)
                    .into_iter()
                    .map(|(caller, span)| (caller.path.to_string(), span))
                    .collect(),
            ));
            if lookup.jsx_call_site_loading(path, function.span).any {
                answers.rendered.push((path.to_owned(), function.span));
            }
            for property in &file.ast.object_properties {
                if lookup.is_lazy_default_render_witness(file, function.span, property.span) {
                    answers
                        .witnesses
                        .push((path.to_owned(), function.span, property.span));
                }
            }
        }
        answers.forwarded.push((
            path.to_owned(),
            lookup
                .result_access_forwarded_arguments(path)
                .iter()
                .map(|entry| entry.argument)
                .collect(),
        ));
    }
    let report = lookup.end_memos()?;
    Some((answers, report.tables))
}

fn table_stats(
    tables: &[(&'static str, super::memo::TableStats)],
    name: &str,
) -> super::memo::TableStats {
    tables
        .iter()
        .find(|(table, _)| *table == name)
        .map(|(_, stats)| *stats)
        .unwrap_or_default()
}

/// A component, the module that renders it, and a module unrelated to both.
const CENSUS_FILES: &[(&str, &str)] = &[
    (
        "/fixture/census/panel.tsx",
        "export function Panel() {\n  return <div>panel</div>;\n}\nexport function Row() {\n  return <p>row</p>;\n}\n",
    ),
    (
        "/fixture/census/app.tsx",
        "import { Panel, Row } from \"./panel\";\nexport function App() {\n  const rows = [1].map(Row);\n  return <Panel>{rows}</Panel>;\n}\n",
    ),
    (
        "/fixture/census/other.ts",
        "export const label = \"x\";\nexport function helper(value: number) {\n  return value + 1;\n}\n",
    ),
];

fn census_project() -> Project {
    Project {
        files: CENSUS_FILES
            .iter()
            .map(|(path, source)| ((*path).to_owned(), (*source).to_owned()))
            .collect(),
        schemes: HashMap::default(),
        unreferenced: Vec::new(),
    }
}

/// With stable symbol ids, an edit of a module that neither references,
/// calls nor renders a function leaves its identity unrun: its reads are
/// verified (the adapters answer as before) and nothing in the other files
/// is recomputed. An edit of the rendering module recomputes the answers that
/// read it.
#[test]
fn component_identities_are_reused_across_an_unrelated_edit() {
    let store = super::memo::build::MemoStore::new(false, true);
    let mut project = census_project();
    let (first, tables) = census_build(&project, &store, false).unwrap();
    // `Panel` is rendered by a tag in another file; `Row` is handed to
    // `map`, an escape; `helper` returns no JSX.
    let status = |answers: &CensusAnswers, index: usize| {
        let (_, _, status, escaped) = &answers.statuses[index];
        (*status, *escaped)
    };
    use crate::indexes::ComponentStatus;
    assert_eq!(status(&first, 0), (ComponentStatus::Proven, false));
    assert_eq!(status(&first, 1), (ComponentStatus::Proven, true));
    assert_eq!(status(&first, 3), (ComponentStatus::No, false));
    assert_eq!(first.rendered.len(), 1);
    assert_eq!(first.sites[0].2.len(), 2, "the tag and its closing name");
    // One per-file answer each; a census of its references for `Row` and
    // `App`, whose own file does not decide them.
    assert_eq!(table_stats(&tables, "component_functions").fresh, 3);
    assert_eq!(table_stats(&tables, "component_census").fresh, 2);
    // A body edit of the unrelated module: the same length, no span moves.
    project.files[2].1 = project.files[2].1.replace("\"x\"", "\"y\"");
    let (second, tables) = census_build(&project, &store, false).unwrap();
    assert_eq!(first, second);
    let files = table_stats(&tables, "component_functions");
    // Only the edited file's answer is recomputed.
    assert_eq!(
        (files.fresh, files.changed, files.unchanged),
        (0, 0, 1),
        "{files:?}"
    );
    assert_eq!(files.reused + files.verified, 2, "{files:?}");
    let census = table_stats(&tables, "component_census");
    assert_eq!(
        census.fresh + census.changed + census.unchanged,
        0,
        "{census:?}"
    );
    assert_eq!(census.reused + census.verified, 2, "{census:?}");
    let sites = table_stats(&tables, "function_call_sites");
    assert_eq!(sites.changed + sites.fresh, 0, "{sites:?}");
    // A prefix edit of the rendering module moves every span in it: the
    // answers that read it are recomputed, and agree with a fresh store.
    project.files[1].1 = format!("// moved\n{}", project.files[1].1);
    let (third, tables) = census_build(&project, &store, false).unwrap();
    let fresh = super::memo::build::MemoStore::new(false, false);
    let (expected, _) = census_build(&project, &fresh, false).unwrap();
    assert_eq!(third, expected);
    let census = table_stats(&tables, "component_census");
    assert!(census.unchanged + census.changed >= 1, "{census:?}");
}

/// Through random edits (the base property test's kinds, TypeFacts-only
/// changes included), the retained store's census answers exactly as a
/// fresh store's, with verify-reused mode on: no reused answer missed a read.
#[test]
fn census_memos_through_random_edits_equal_a_fresh_store() {
    let mut steps = 0;
    let (mut reused, mut computed) = (0, 0);
    let fixtures = FIXTURES.iter().map(|fixture| {
        let mut project = Project {
            files: fixture_sources(fixture),
            schemes: HashMap::default(),
            unreferenced: Vec::new(),
        };
        project.files.extend(census_project().files);
        project.files.sort();
        (*fixture, project)
    });
    for (fixture, original) in fixtures {
        for seed in 1..=2 {
            let closed = seed == 2;
            let mut random = Random(seed);
            let mut project = original.clone();
            let store = super::memo::build::MemoStore::new(true, true);
            census_build(&project, &store, closed).expect("the fixture parses");
            for serial in 0..15 {
                let kind = KINDS[random.below(KINDS.len())];
                let mut next = project.clone();
                if edit(&mut next, kind, &mut random, serial).is_none() || facts(&next).is_none() {
                    continue;
                }
                project = next;
                let (answers, tables) = census_build(&project, &store, closed).unwrap();
                let fresh = super::memo::build::MemoStore::new(false, false);
                let (expected, _) = census_build(&project, &fresh, closed).unwrap();
                assert_eq!(
                    answers, expected,
                    "{fixture} seed {seed} step {serial} ({kind})"
                );
                for (table, stats) in &tables {
                    assert_eq!(
                        stats.mismatches, 0,
                        "{fixture} seed {seed} step {serial}: {table}"
                    );
                    reused += stats.reused + stats.verified;
                    computed += stats.fresh + stats.changed + stats.unchanged;
                }
                steps += 1;
            }
        }
    }
    eprintln!("census memos: {steps} edits, {reused} answers reused, {computed} computed");
    assert!(steps > 120, "only {steps} edits applied");
    assert!(reused > 0, "{reused} reused, {computed} computed");
}
