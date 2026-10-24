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
                Some(property) => {
                    // A project member call: its resolved declaration's
                    // symbol is spelled under the caller's declaration
                    // scheme, which a TypeFacts-only edit of another file
                    // re-spells (ids keyed by declaration position, study
                    // N5) while this file's source and entity rows stay.
                    let declared = schemes.get(&format!("decl:{path}")).copied().unwrap_or(0);
                    // A TypeFacts-only flip of the call's validity (another
                    // file's edit can make an overload unresolvable).
                    let validity = if schemes
                        .get(&format!("validity:{path}"))
                        .copied()
                        .unwrap_or(0)
                        % 2
                        == 1
                    {
                        "recovery"
                    } else {
                        "valid"
                    };
                    entities.push(
                        serde_json::from_value(serde_json::json!({
                            "location": location(path, call.callee),
                            "resolvedCall": {
                                "validity": validity,
                                "kind": "call",
                                "declaration": {
                                    "symbol": format!("d{declared}:{property}"),
                                    "kind": "method",
                                    "location": location("/fixture/declared.ts", Span::new(0, 1)),
                                    "qualifiedName": property,
                                },
                            },
                        }))
                        .expect("entity with a project resolved call"),
                    );
                    continue;
                }
                None => continue,
            };
            let validity = if schemes
                .get(&format!("validity:{path}"))
                .copied()
                .unwrap_or(0)
                % 2
                == 1
            {
                "recovery"
            } else {
                "valid"
            };
            entities.push(
                serde_json::from_value(serde_json::json!({
                    "location": location(path, call.callee),
                    "resolvedCall": {
                        "validity": validity,
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
        Arc::make_mut(symbols.exact_by_path.entry(path.clone()).or_default())
            .insert(key, symbol.clone());
        Arc::make_mut(symbols.by_path.entry(path).or_default()).insert(key, symbol);
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
    "typefacts-validity",
    "typefacts-references",
    "typefacts-declarations",
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
    if kind == "typefacts-validity" {
        *project
            .schemes
            .entry(format!("validity:{path}"))
            .or_default() += 1;
        return Some(());
    }
    if kind == "typefacts-declarations" {
        *project.schemes.entry(format!("decl:{path}")).or_default() += 1;
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

/// `FileRead::resolved_callee_call` hands a memo the whole resolved call
/// (the flow memos read its validity), so the Signatures section must move
/// when only a call's validity does: an edit elsewhere can make an overload
/// unresolvable while this file's source and entity rows stay.
#[test]
fn a_resolved_call_validity_flip_changes_the_calling_file() {
    let store = super::memo::build::MemoStore::new(false, true);
    let mut project = wrapper_project(FIXTURES[0]);
    project.files.push((
        "/fixture/calls.ts".to_owned(),
        "export const box = { run() { return 1; } };\nexport const ran = box.run();\n".to_owned(),
    ));
    memo_build(&project, &store).unwrap();
    let unchanged = memo_build(&project, &store).unwrap();
    assert_eq!(unchanged.begin.changed_files, 0);
    *project
        .schemes
        .entry("validity:/fixture/calls.ts".to_owned())
        .or_default() += 1;
    let flipped = memo_build(&project, &store).unwrap();
    assert_eq!(flipped.begin.changed_files, 1);
}

/// One memo begin over `facts` with `symbols` as the build's entity rows.
fn begin_over(
    facts: &ProjectFacts,
    symbols: &EntitySymbols,
    store: &super::memo::build::MemoStore,
) -> super::memo::build::BeginReport {
    let ast_indexes = facts
        .files
        .iter()
        .map(|file| (file.path.clone(), CachedAstFileIndex::new(file)))
        .collect::<HashMap<_, _>>();
    let entities = crate::indexes::first_entity_by_location(facts.typescript.entities());
    let references = OnceLock::new();
    let table = store.table(&super::FileTable::new(&facts.files));
    let views = Views {
        table: &table,
        files: &facts.files,
        sources: ViewSources {
            ast_indexes: &ast_indexes,
            symbols,
            entities: &entities,
            typescript: &facts.typescript,
            references: &references,
        },
        constants: BuildConstants {
            dialect: &solid_dialect::Solid2,
        },
    };
    // The producer says nothing changed: the store must not need its word.
    let begin = store.begin_with(&views, false, &super::memo::probes::NoProbes, true, false);
    store.end();
    begin
}

/// ADR 0406: a file's TypeFacts parts are digested again exactly
/// when its rows are not the rows the store digested last build, whatever
/// the producer names. The same table and rows: nothing is digested again.
/// One path's entities replaced (no change list names it): that file alone
/// is digested again and found changed. The same content in a new table (a
/// full replace): every file is digested again and none is found changed.
/// One path's entity-symbol rows copied on write: that file alone.
#[test]
fn fingerprint_parts_are_reused_exactly_for_the_rows_they_digested() {
    // Shadow mode on: every reused part is also compared with a fresh digest.
    let store = super::memo::build::MemoStore::new(false, true);
    let project = wrapper_project(FIXTURES[0]);
    let original = facts(&project).unwrap();
    let files = original.files.len();
    let symbols = entity_symbols(&original);
    let first = begin_over(&original, &symbols, &store);
    assert_eq!((first.entities_reused, first.rows_reused), (0, 0));

    let same = begin_over(&original, &symbols, &store);
    assert_eq!((same.entities_reused, same.rows_reused), (files, files));
    assert_eq!(same.changed_files, 0);

    let path = "/fixture/use.ts";
    let mut entities = original.typescript.entities_for_path(path).to_vec();
    assert!(!entities.is_empty(), "the caller has entities");
    entities.pop();
    let mut edited = original.clone();
    edited.typescript = edited
        .typescript
        .clone()
        .with_entity_overrides([(path.to_owned(), entities.clone())].into_iter().collect());
    let unnamed = begin_over(&edited, &symbols, &store);
    assert_eq!(unnamed.entities_reused, files - 1);
    assert_eq!(unnamed.changed_files, 1);
    assert!(!store.poisoned());

    let mut replaced = facts(&project).unwrap();
    replaced.typescript = replaced
        .typescript
        .clone()
        .with_entity_overrides([(path.to_owned(), entities)].into_iter().collect());
    let full = begin_over(&replaced, &symbols, &store);
    assert_eq!(full.entities_reused, 0);
    assert_eq!(full.changed_files, 0);

    let mut rows = symbols.clone();
    let row = Arc::make_mut(rows.by_path.get_mut(path).expect("the caller's rows"));
    let key = *row.keys().next().expect("a row");
    row.insert(key, SymbolId::from("s0:moved"));
    let copied = begin_over(&replaced, &rows, &store);
    assert_eq!(copied.rows_reused, files - 1);
    assert_eq!(copied.changed_files, 1);
    assert!(!store.poisoned());
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

/// A question about one function's component identity reads its file's
/// part of the census (`component_spans`), not the whole census: a prefix
/// edit of another file, which moves every span there, leaves the proof of
/// a question about this file holding, and moves the proof of a question
/// about the edited file. The answers are the whole sets'.
#[test]
fn a_component_question_reads_only_its_files_part_of_the_census() {
    use crate::base::reads::{ReadProof, Section, record};
    const PATHS: [&str; 2] = ["/fixture/census/app.tsx", "/fixture/census/panel.tsx"];
    type Asked = (
        Vec<crate::indexes::ComponentStatus>,
        Vec<ReadProof>,
        Vec<bool>,
    );
    let store = super::memo::build::MemoStore::new(false, true);
    // Each path's answers, their proofs, and whether `kept`'s proofs hold.
    let ask = |project: &Project, kept: &[Asked]| -> Vec<Asked> {
        let facts = facts(project).expect("the project parses");
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
            false,
        )
        .with_memo_store(&store);
        lookup.begin_memos();
        let asked = PATHS
            .iter()
            .enumerate()
            .map(|(index, path)| {
                let held = kept.get(index).map_or_else(Vec::new, |(_, proofs, _)| {
                    proofs.iter().map(|proof| proof.holds(&lookup)).collect()
                });
                let file = lookup.file_by_path(path).expect("the file is analyzed");
                let (mut answers, mut proofs) = (Vec::new(), Vec::new());
                for function in &file.ast.functions {
                    let (status, reads) =
                        record(|| lookup.function_component_status(file, function));
                    assert!(!reads.reads_section(Section::Components), "{path}");
                    let proven = lookup.function_is_component(file, function)
                        || lookup
                            .escaped_component_functions()
                            .contains(&(*path, function.span));
                    assert_eq!(
                        status == crate::indexes::ComponentStatus::Proven,
                        proven,
                        "{path}"
                    );
                    answers.push(status);
                    proofs.push(ReadProof::new(reads, &lookup));
                }
                (answers, proofs, held)
            })
            .collect();
        let _ = lookup.end_memos();
        asked
    };
    let mut project = census_project();
    let first = ask(&project, &[]);
    project.files[0].1 = format!("// moved\n{}", project.files[0].1);
    let second = ask(&project, &first);
    assert!(
        second[0].2.iter().all(|held| *held),
        "app reads none of panel's spans"
    );
    assert!(second[1].2.iter().all(|held| !*held), "panel's spans moved");
    assert_eq!(second[1].0, first[1].0);
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

// ---------------------------------------------------------------------------
// L2: returned-callback use sites over a retained store (study S5e,
// [`crate::indexes::ReturnedUses`]).

/// One factory call's uses, as `(factory file, factory span, [(file, span,
/// kind)])`.
type FactoryUses = (String, Span, Vec<(String, Span, crate::indexes::UseKind)>);

type MemoTables = Vec<(&'static str, super::memo::TableStats)>;

/// One build of `project` over `store`: the uses of every call's result, and
/// the build's memo stats by table.
fn returned_build(
    project: &Project,
    store: &super::memo::build::MemoStore,
) -> Option<(Vec<FactoryUses>, MemoTables)> {
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
        false,
    )
    .with_memo_store(store);
    let mut answers = Vec::new();
    for file in lookup.files() {
        let path = file.path.as_str();
        for call in &file.ast.calls {
            let uses = lookup
                .returned_uses(path, call.span)
                .iter()
                .map(|used| {
                    (
                        lookup.returned_use_file(used).unwrap().path.to_string(),
                        used.span,
                        used.kind,
                    )
                })
                .collect();
            answers.push((path.to_owned(), call.span, uses));
        }
    }
    let report = lookup.end_memos()?;
    Some((answers, report.tables))
}

/// A factory whose result is bound, destructured, called and handed to a
/// callback in its own file, aliased and rendered in a second file, and
/// `preload`ed in a third; and a module unrelated to all of them.
const RETURNED_FILES: &[(&str, &str)] = &[
    (
        "/fixture/returned/factory.ts",
        "export const load = make(() => 1);\nexport const [track, again] = pair(() => 2);\nmake(() => 3)();\nmake(() => 4).preload();\ntake(make(() => 5));\nmake(() => 6).other();\nexport const unused = make(() => 7);\n",
    ),
    (
        "/fixture/returned/alias.tsx",
        "import { load, track } from \"./factory\";\nconst Alias = load;\nexport const Again = Alias;\nAlias();\ntrack(() => 1);\nexport const view = <Again />;\n",
    ),
    (
        "/fixture/returned/preloader.ts",
        "import { Again } from \"./alias\";\nimport { load } from \"./factory\";\nAgain.preload();\nload.other();\nschedule(load);\nexport const label = \"x\";\n",
    ),
    (
        "/fixture/returned/other.ts",
        "export const note = \"x\";\nexport function helper(value: number) {\n  return value + 1;\n}\n",
    ),
];

fn returned_project() -> Project {
    Project {
        files: RETURNED_FILES
            .iter()
            .map(|(path, source)| ((*path).to_owned(), (*source).to_owned()))
            .collect(),
        schemes: HashMap::default(),
        unreferenced: Vec::new(),
    }
}

fn returned_table(tables: &[(&'static str, super::memo::TableStats)]) -> super::memo::TableStats {
    table_stats(tables, "returned_callback_uses")
}

#[test]
fn returned_uses_follow_the_result_through_aliases_and_files() {
    use crate::indexes::UseKind;
    let store = super::memo::build::MemoStore::new(false, true);
    let project = returned_project();
    let (answers, _) = returned_build(&project, &store).unwrap();
    let text = |path: &str| {
        project
            .files
            .iter()
            .find(|(candidate, _)| candidate == path)
            .map(|(_, source)| source.as_str())
            .unwrap()
    };
    // The uses of the call that starts with `needle` in `path`, as
    // `(file, text at the use, kind)`.
    let uses_of = |path: &str, needle: &str| {
        let source = text(path);
        let start = source.find(needle).expect("the factory call");
        let (_, _, uses) = answers
            .iter()
            .find(|(file, span, _)| file == path && usize::try_from(span.start).unwrap() == start)
            .expect("a call at the span");
        uses.iter()
            .map(|(file, span, kind)| {
                let source = text(file);
                (
                    file.rsplit('/').next().unwrap().to_owned(),
                    source
                        [usize::try_from(span.start).unwrap()..usize::try_from(span.end).unwrap()]
                        .to_owned(),
                    *kind,
                )
            })
            .collect::<Vec<_>>()
    };
    let factory = "/fixture/returned/factory.ts";
    // `load`: aliased (`Alias`, `Again`) and used in two other files.
    let load = uses_of(factory, "make(() => 1)");
    assert!(
        load.contains(&("alias.tsx".into(), "Alias()".into(), UseKind::Call)),
        "{load:?}"
    );
    assert!(
        load.contains(&("alias.tsx".into(), "<Again />".into(), UseKind::Jsx)),
        "{load:?}"
    );
    assert!(
        load.contains(&(
            "preloader.ts".into(),
            "Again.preload()".into(),
            UseKind::Preload
        )),
        "{load:?}"
    );
    assert!(
        load.contains(&(
            "preloader.ts".into(),
            "schedule(load)".into(),
            UseKind::Argument(0)
        )),
        "{load:?}"
    );
    // `load.other()` invokes a member of the result, not the result.
    assert!(
        !load.iter().any(|(_, text, _)| text == "load.other()"),
        "{load:?}"
    );
    // Both names of a destructured result hold it.
    let pair = uses_of(factory, "pair(() => 2)");
    assert!(
        pair.contains(&("alias.tsx".into(), "track(() => 1)".into(), UseKind::Call)),
        "{pair:?}"
    );
    // The written-around forms.
    let direct = uses_of(factory, "make(() => 3)");
    assert_eq!(direct.len(), 1, "{direct:?}");
    assert_eq!(direct[0].2, UseKind::Call);
    let preload = uses_of(factory, "make(() => 4)");
    assert_eq!(preload.len(), 1, "{preload:?}");
    let handed = uses_of(factory, "make(() => 5)");
    assert_eq!(handed.len(), 1, "{handed:?}");
    assert_eq!(handed[0].2, UseKind::Argument(0));
    // A member of the result is not a use of it, and an unused result has
    // none.
    assert!(uses_of(factory, "make(() => 6)").is_empty());
    assert!(uses_of(factory, "make(() => 7)").is_empty());
}

/// With stable symbol ids, an edit of a module that neither holds the
/// factory nor mentions its result leaves every answer unrun; an edit of a
/// user recomputes only what read it.
#[test]
fn returned_uses_are_reused_across_an_unrelated_edit() {
    let store = super::memo::build::MemoStore::new(false, true);
    let mut project = returned_project();
    let (first, tables) = returned_build(&project, &store).unwrap();
    assert!(returned_table(&tables).fresh > 0);
    // A body edit of the unrelated module: the same length, no span moves.
    project.files[3].1 = project.files[3].1.replace("\"x\"", "\"y\"");
    let (second, tables) = returned_build(&project, &store).unwrap();
    assert_eq!(first, second);
    let uses = returned_table(&tables);
    assert_eq!(uses.fresh + uses.changed + uses.unchanged, 0, "{uses:?}");
    assert!(uses.reused + uses.verified > 0, "{uses:?}");
    // A body edit of a user: its readers re-run, to the same answers.
    project.files[2].1 = project.files[2].1.replace("\"x\"", "\"z\"");
    let (third, tables) = returned_build(&project, &store).unwrap();
    assert_eq!(first, third);
    let uses = returned_table(&tables);
    assert_eq!(uses.changed, 0, "{uses:?}");
    assert!(uses.unchanged > 0, "{uses:?}");
    // Dropping the aliasing import moves the answers of the factory's calls.
    project.files[1].1 = project.files[1].1.replace("Alias();", "");
    let (fourth, tables) = returned_build(&project, &store).unwrap();
    assert_ne!(first, fourth);
    assert!(returned_table(&tables).changed > 0);
    let fresh = super::memo::build::MemoStore::new(false, false);
    let (expected, _) = returned_build(&project, &fresh).unwrap();
    assert_eq!(fourth, expected);
}

/// Through random edits, the retained store's use sites equal a fresh
/// store's, with verify-reused mode on.
#[test]
fn returned_uses_through_random_edits_equal_a_fresh_store() {
    let (mut steps, mut reused, mut computed) = (0, 0, 0);
    let fixtures = FIXTURES.iter().map(|fixture| {
        let mut project = Project {
            files: fixture_sources(fixture),
            schemes: HashMap::default(),
            unreferenced: Vec::new(),
        };
        project.files.extend(returned_project().files);
        project.files.sort();
        (*fixture, project)
    });
    for (fixture, original) in fixtures {
        for seed in 1..=2 {
            let mut random = Random(seed);
            let mut project = original.clone();
            let store = super::memo::build::MemoStore::new(true, true);
            returned_build(&project, &store).expect("the fixture parses");
            for serial in 0..15 {
                let kind = KINDS[random.below(KINDS.len())];
                let mut next = project.clone();
                if edit(&mut next, kind, &mut random, serial).is_none() || facts(&next).is_none() {
                    continue;
                }
                project = next;
                let (answers, tables) = returned_build(&project, &store).unwrap();
                let fresh = super::memo::build::MemoStore::new(false, false);
                let (expected, _) = returned_build(&project, &fresh).unwrap();
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
    eprintln!("returned uses: {steps} edits, {reused} answers reused, {computed} computed");
    assert!(steps > 120, "only {steps} edits applied");
    assert!(reused > 0, "{reused} reused, {computed} computed");
}

/// Every memo table of a build has its own id and name, and the schema's
/// dispatch by id finds that table: two slices that pick the same id (or a
/// range dispatch that shadows another's) fail here, not as a stats or
/// invalidation mix-up at run time.
#[test]
fn every_build_table_has_its_own_id() {
    use super::memo::TableSet;
    let tables = super::memo::build::BuildTables::default();
    let all = tables.tables();
    let mut ids = std::collections::BTreeMap::new();
    let mut names = std::collections::BTreeSet::new();
    for table in &all {
        if let Some(other) = ids.insert(table.id(), table.name()) {
            panic!(
                "tables `{other}` and `{}` share the id {:?}",
                table.name(),
                table.id()
            );
        }
        assert!(
            names.insert(table.name()),
            "two tables are named `{}`",
            table.name()
        );
    }
    for table in &all {
        assert_eq!(
            tables.table(table.id()).name(),
            table.name(),
            "the id {:?} dispatches to another table",
            table.id()
        );
    }
}

/// A file's callee index entries name the symbol its member calls' resolved
/// declarations carry. An edit elsewhere can re-spell that symbol (ids keyed
/// by declaration position, study N5) while the file's source and entity
/// rows stay: the file's view must count as changed, or the retained index
/// keeps the old entries (the groove seed-11 shadow mismatch).
#[test]
fn a_respelled_call_declaration_changes_the_calling_file() {
    let source = "export const box = { run() { return 1; } };\nexport const ran = box.run();\n";
    let mut project = Project {
        files: vec![("/fixture/calls.ts".to_owned(), source.to_owned())],
        schemes: HashMap::default(),
        unreferenced: Vec::new(),
    };
    let mut retained = None;
    check(&project, &mut retained).expect("the source parses");
    *project
        .schemes
        .entry("decl:/fixture/calls.ts".to_owned())
        .or_default() += 1;
    assert_eq!(check(&project, &mut retained), Some(1));
}

// ---------------------------------------------------------------------------
// L2: the invocation-flow answers over a retained store (study S5a,
// [`crate::execution_role::InvocationFlowAnswer`]).

/// Every literal's flow answer and event-only census of one build, by path
/// and span, read through the lookup as its consumers read them.
type FlowAnswers = Vec<(String, Span, Option<crate::ExecutionRole>, bool)>;

/// One build of `project` over `store` through a lookup: every literal's
/// answers, and the build's memo stats by table.
fn flow_build(
    project: &Project,
    store: &super::memo::build::MemoStore,
    closed: bool,
) -> Option<(FlowAnswers, Vec<(&'static str, super::memo::TableStats)>)> {
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
    let roles = lookup.invocation_flow_answers();
    let events = lookup.event_invoked_literals();
    let mut answers = Vec::new();
    for file in lookup.files() {
        for literal in &file.ast.functions {
            let key = (file.path.to_string(), literal.span);
            answers.push((
                key.0.clone(),
                key.1,
                roles.get(&key).copied(),
                events.contains(&key),
            ));
        }
    }
    // A build that asked no memo (an open program) reports none.
    let tables = lookup
        .end_memos()
        .map(|report| report.tables)
        .unwrap_or_default();
    Some((answers, tables))
}

/// A handler bound in one module, handed through a component's props to an
/// intrinsic element's event attribute in another, and a module unrelated to
/// both.
const FLOW_FILES: &[(&str, &str)] = &[
    (
        "/fixture/flow/button.tsx",
        "export function Button(props: { onPress: () => void }) {\n  return <button onClick={props.onPress}>go</button>;\n}\n",
    ),
    (
        "/fixture/flow/app.tsx",
        "import { Button } from \"./button\";\nexport function App() {\n  const press = () => console.log(\"x\");\n  return <Button onPress={press} />;\n}\n",
    ),
    ("/fixture/flow/other.ts", "export const label = \"x\";\n"),
];

fn flow_project() -> Project {
    Project {
        files: FLOW_FILES
            .iter()
            .map(|(path, source)| ((*path).to_owned(), (*source).to_owned()))
            .collect(),
        schemes: HashMap::default(),
        unreferenced: Vec::new(),
    }
}

/// The walk follows the handler across files and finds one event delivery;
/// an edit of a module the walk never visits leaves every answer unrun (its
/// reads are verified), and an edit of a module it visits recomputes the
/// answers that read it, agreeing with a fresh store.
#[test]
fn flow_answers_follow_a_handler_across_files_and_are_reused_across_an_unrelated_edit() {
    let store = super::memo::build::MemoStore::new(false, true);
    let mut project = flow_project();
    let (first, tables) = flow_build(&project, &store, true).unwrap();
    // The arrow bound to `press` (the module's shorter function).
    let press = first
        .iter()
        .filter(|(path, ..)| path == "/fixture/flow/app.tsx")
        .min_by_key(|(_, span, ..)| span.end - span.start)
        .expect("the handler literal");
    assert_eq!(
        (press.2, press.3),
        (Some(crate::ExecutionRole::EventCallback), true),
        "{first:?}"
    );
    // One answer per file.
    let files = project.files.len() as u64;
    assert_eq!(table_stats(&tables, "invocation_flow_answers").fresh, files);
    assert_eq!(table_stats(&tables, "event_invoked_literals").fresh, files);
    // An open program asks nothing.
    let open = super::memo::build::MemoStore::new(false, true);
    let (unproven, tables) = flow_build(&project, &open, false).unwrap();
    assert!(
        unproven
            .iter()
            .all(|(.., role, event)| role.is_none() && !event)
    );
    assert_eq!(table_stats(&tables, "invocation_flow_answers").fresh, 0);
    // A body edit of the unrelated module: the same length, no span moves.
    // Only its own answer is recomputed (to the same value): the other
    // modules' walks never visited it.
    project.files[2].1 = project.files[2].1.replace("\"x\"", "\"y\"");
    let (second, tables) = flow_build(&project, &store, true).unwrap();
    assert_eq!(first, second);
    for table in ["invocation_flow_answers", "event_invoked_literals"] {
        let stats = table_stats(&tables, table);
        assert_eq!(
            (stats.fresh, stats.changed, stats.unchanged),
            (0, 0, 1),
            "{table}: {stats:?}"
        );
        assert_eq!(
            stats.reused + stats.verified,
            files - 1,
            "{table}: {stats:?}"
        );
    }
    // A prefix edit of the component's module moves every span in it: the
    // walks that visited it run again, and agree with a fresh store.
    project.files[1].1 = format!("// moved\n{}", project.files[1].1);
    let (third, tables) = flow_build(&project, &store, true).unwrap();
    let fresh = super::memo::build::MemoStore::new(false, false);
    let (expected, _) = flow_build(&project, &fresh, true).unwrap();
    assert_eq!(third, expected);
    let stats = table_stats(&tables, "invocation_flow_answers");
    assert!(
        stats.fresh + stats.unchanged + stats.changed >= 1,
        "{stats:?}"
    );
}

/// Handlers handed to project callees, which give them to an intrinsic
/// element's event attribute: an imported function, a local object literal's
/// method and a namespace import's member; a module whose members
/// are reassigned, reflectively and by assignment; and a module unrelated to
/// all of them.
const CALLEE_FILES: &[(&str, &str)] = &[
    (
        "/fixture/callee/bind.tsx",
        "export function bind(callback: () => void) {\n  return <button onClick={callback}>go</button>;\n}\nexport function give(callback: () => void) {\n  return <button onClick={callback}>go</button>;\n}\nexport function call(callback: () => void) {\n  callback();\n}\n",
    ),
    (
        "/fixture/callee/app.tsx",
        "import { bind, call } from \"./bind\";\nimport * as ns from \"./bind\";\nconst box = {\n  hand(callback: () => void) {\n    return <button onClick={callback}>go</button>;\n  },\n};\nexport function App() {\n  const press = () => console.log(\"x\");\n  const hold = () => console.log(\"y\");\n  const tap = () => console.log(\"z\");\n  const ping = () => console.log(\"w\");\n  call(ping);\n  return [bind(press), box.hand(hold), ns.give(tap)];\n}\n",
    ),
    (
        "/fixture/callee/patch.ts",
        "export const target = { size: 1 };\ntarget.size = 2;\nObject.defineProperty(target, \"name\", { value: 1 });\n",
    ),
    ("/fixture/callee/other.ts", "export const label = \"x\";\n"),
];

fn callee_project() -> Project {
    Project {
        files: CALLEE_FILES
            .iter()
            .map(|(path, source)| ((*path).to_owned(), (*source).to_owned()))
            .collect(),
        schemes: HashMap::default(),
        unreferenced: Vec::new(),
    }
}

/// A walk that resolves a project callee at a parameter, an object member's
/// method or a namespace member reads the callee's file, the caller's member
/// targets and the assigned-member census, not the whole program; one that
/// finds a call invoking the value reads that site's role
/// ([`crate::execution_role::FlowSiteRole`]). An edit of a module it never
/// visits leaves its answers unrun: the site roles run again (they read the
/// whole program) and answer the same. An edit of the callee's module
/// recomputes them, as a fresh store answers.
#[test]
fn flow_answers_through_project_callees_are_reused_across_an_unrelated_edit() {
    let store = super::memo::build::MemoStore::new(true, true);
    let mut project = callee_project();
    let (first, _) = flow_build(&project, &store, true).unwrap();
    // The synthetic facts resolve the imported function; the member calls
    // resolve no project callee (their declarations are synthetic), so only
    // the first handler is proven, the others ask and find nothing.
    let delivered = first
        .iter()
        .filter(|(path, .., event)| path == "/fixture/callee/app.tsx" && *event)
        .count();
    assert_eq!(delivered, 1, "{first:?}");
    let files = project.files.len() as u64;
    // A body edit of the unrelated module: only its own answers run.
    project.files[3].1 = project.files[3].1.replace("\"x\"", "\"y\"");
    let (second, tables) = flow_build(&project, &store, true).unwrap();
    assert_eq!(first, second);
    for table in ["invocation_flow_answers", "event_invoked_literals"] {
        let stats = table_stats(&tables, table);
        assert_eq!(
            (
                stats.fresh,
                stats.changed,
                stats.unchanged,
                stats.mismatches
            ),
            (0, 0, 1, 0),
            "{table}: {stats:?}"
        );
        assert_eq!(
            stats.reused + stats.verified,
            files - 1,
            "{table}: {stats:?}"
        );
    }
    let sites = table_stats(&tables, "flow_site_role");
    assert!(
        sites.unchanged >= 1 && sites.changed == 0 && sites.fresh == 0,
        "{sites:?}"
    );
    // The callee's module, edited so that the function no longer hands its
    // callback to an event: the walks that visited it run again,
    // and agree with a fresh store.
    project.files[0].1 = project.files[0].1.replacen(
        "bind(callback: () => void) {\n  return <button onClick={callback}>go</button>;",
        "bind(callback: () => void) {\n  return <button title={callback}>go</button>;",
        1,
    );
    let (third, tables) = flow_build(&project, &store, true).unwrap();
    let fresh = super::memo::build::MemoStore::new(false, false);
    let (expected, _) = flow_build(&project, &fresh, true).unwrap();
    assert_eq!(third, expected);
    assert_ne!(third, first);
    let stats = table_stats(&tables, "event_invoked_literals");
    assert!(stats.changed >= 1 && stats.mismatches == 0, "{stats:?}");
    // A module whose members are written: the census moves, and the answers
    // that read it agree with a fresh store.
    project.files[2].1 = project.files[2]
        .1
        .replace("target.size = 2;", "target.bind = 2;");
    let (fourth, tables) = flow_build(&project, &store, true).unwrap();
    let fresh = super::memo::build::MemoStore::new(false, false);
    let (expected, _) = flow_build(&project, &fresh, true).unwrap();
    assert_eq!(fourth, expected);
    for (table, stats) in &tables {
        assert_eq!(stats.mismatches, 0, "{table}: {stats:?}");
    }
}

/// Through random edits (the base property test's kinds, TypeFacts-only
/// changes and hazard flips included), the retained store's flow answers
/// equal a fresh store's, with verify-reused mode on: no reused answer missed
/// a read.
#[test]
fn flow_memos_through_random_edits_equal_a_fresh_store() {
    let mut steps = 0;
    let (mut reused, mut computed) = (0, 0);
    let fixtures = FIXTURES.iter().map(|fixture| {
        let mut project = Project {
            files: fixture_sources(fixture),
            schemes: HashMap::default(),
            unreferenced: Vec::new(),
        };
        project.files.extend(flow_project().files);
        project.files.extend(callee_project().files);
        project.files.sort();
        (*fixture, project)
    });
    for (fixture, original) in fixtures {
        for seed in 1..=2 {
            let mut random = Random(seed);
            let mut project = original.clone();
            let store = super::memo::build::MemoStore::new(true, true);
            flow_build(&project, &store, true).expect("the fixture parses");
            for serial in 0..15 {
                let kind = KINDS[random.below(KINDS.len())];
                let mut next = project.clone();
                if edit(&mut next, kind, &mut random, serial).is_none() || facts(&next).is_none() {
                    continue;
                }
                project = next;
                let (answers, tables) = flow_build(&project, &store, true).unwrap();
                let fresh = super::memo::build::MemoStore::new(false, false);
                let (expected, _) = flow_build(&project, &fresh, true).unwrap();
                assert_eq!(
                    answers, expected,
                    "{fixture} seed {seed} step {serial} ({kind})"
                );
                for (table, stats) in &tables {
                    assert_eq!(
                        stats.mismatches, 0,
                        "{fixture} seed {seed} step {serial}: {table}"
                    );
                    if table.contains("invocation_flow") || table.contains("event_invoked") {
                        reused += stats.reused + stats.verified;
                        computed += stats.fresh + stats.changed + stats.unchanged;
                    }
                }
                steps += 1;
            }
        }
    }
    eprintln!("flow memos: {steps} edits, {reused} answers reused, {computed} computed");
    assert!(steps > 120, "only {steps} edits applied");
    assert!(reused > 0, "{reused} reused, {computed} computed");
}

/// A tracker and a tuple bound in one file, aliased, re-exported and called
/// (plainly and through TypeScript wrappers) in a second; a hook returning a
/// fresh signal and its caller; and a module unrelated to all of them.
const INVOCATION_FILES: &[(&str, &str)] = &[
    (
        "/fixture/invocations/factory.ts",
        "export const load = make(() => 1);\nexport const [first, second] = pair(0);\nload(() => 2);\nexport function useCounter() {\n  const count = memo(() => 0);\n  return count;\n}\n",
    ),
    (
        "/fixture/invocations/user.ts",
        "import { load, second } from \"./factory\";\nimport { useCounter } from \"./factory\";\nconst Alias = load;\nAlias(() => 3);\n(load as any)(() => 4);\nload!(() => 5);\nsecond(1);\nother(() => 6);\nexport const counter = useCounter();\n",
    ),
    (
        "/fixture/invocations/other.ts",
        "export const note = \"x\";\nexport function helper(value: number) {\n  return String(value);\n}\n",
    ),
];

fn invocation_project() -> Project {
    Project {
        files: INVOCATION_FILES
            .iter()
            .map(|(path, source)| ((*path).to_owned(), (*source).to_owned()))
            .collect(),
        schemes: HashMap::default(),
        unreferenced: Vec::new(),
    }
}

/// Per call: `(file, call text, primitive, slot)` for every positive
/// returned-invocation answer; per binding-initializer call: `(file, call
/// text, created call text, primitive)` for every returned source.
type InvocationAnswers = (
    Vec<(String, String, solid_dialect::Primitive, Option<usize>)>,
    Vec<(String, String, String, solid_dialect::Primitive)>,
);

fn invocation_build(
    project: &Project,
    store: &super::memo::build::MemoStore,
) -> Option<(InvocationAnswers, MemoTables)> {
    let facts = facts(project)?;
    let ast_indexes = facts
        .files
        .iter()
        .map(|file| (file.path.clone(), CachedAstFileIndex::new(file)))
        .collect::<HashMap<_, _>>();
    let symbols = entity_symbols(&facts);
    // The synthetic table names every identifier by its spelling (scheme 0):
    // `make`, `pair` and `memo` are the primitives.
    let names = [
        ("make", "createReaction"),
        ("pair", "createSignal"),
        ("memo", "createMemo"),
    ]
    .into_iter()
    .map(|(symbol, name)| {
        (
            crate::SymbolId::from(format!("s0:{symbol}").as_str()),
            crate::SymbolId::from(name),
        )
    })
    .collect::<HashMap<_, _>>();
    let lookup = crate::indexes::SemanticLookup::new(
        &facts,
        &ast_indexes,
        &symbols,
        &names,
        &solid_dialect::Solid2,
        false,
    )
    .with_memo_store(store);
    let text = |file: &FileFacts, span: Span| file.source_text(span).unwrap_or_default().to_owned();
    let mut invocations = Vec::new();
    let mut sources = Vec::new();
    for file in lookup.files() {
        let path = file.path.as_str();
        for call in &file.ast.calls {
            if let Some((primitive, slot)) = lookup.returned_primitive_at(file, call) {
                invocations.push((path.to_owned(), text(file, call.span), primitive, slot));
            }
            if let Some((origin, created, primitive)) = lookup.returned_source_at(file, call.span) {
                sources.push((
                    path.to_owned(),
                    text(file, call.span),
                    text(origin, created.span),
                    primitive,
                ));
            }
        }
    }
    let report = lookup.end_memos()?;
    Some(((invocations, sources), report.tables))
}

fn invocation_table(tables: &[(&'static str, super::memo::TableStats)]) -> super::memo::TableStats {
    table_stats(tables, "returned_primitive_invocations")
}

#[test]
fn returned_invocations_follow_aliases_slots_and_wrapped_callees() {
    use solid_dialect::Primitive;
    let store = super::memo::build::MemoStore::new(false, true);
    let ((invocations, sources), _) = invocation_build(&invocation_project(), &store).unwrap();
    let of = |path: &str, call: &str| {
        invocations
            .iter()
            .find(|(file, text, ..)| file.ends_with(path) && text == call)
            .map(|(.., primitive, slot)| (*primitive, *slot))
    };
    assert_eq!(
        of("factory.ts", "load(() => 2)"),
        Some((Primitive::CreateReaction, None))
    );
    // Through a local alias of an imported binding.
    assert_eq!(
        of("user.ts", "Alias(() => 3)"),
        Some((Primitive::CreateReaction, None))
    );
    // A callee wrapped in `as` or `!` resolves as the identifier it peels to.
    assert_eq!(
        of("user.ts", "(load as any)(() => 4)"),
        Some((Primitive::CreateReaction, None))
    );
    assert_eq!(
        of("user.ts", "load!(() => 5)"),
        Some((Primitive::CreateReaction, None))
    );
    // The destructured slot holding the symbol.
    assert_eq!(
        of("user.ts", "second(1)"),
        Some((Primitive::CreateSignal, Some(1)))
    );
    // A callee bound to nothing proves nothing.
    assert_eq!(of("user.ts", "other(() => 6)"), None);
    assert_eq!(
        sources
            .iter()
            .map(|(file, call, created, primitive)| (
                file.rsplit('/').next().unwrap(),
                call.as_str(),
                created.as_str(),
                *primitive
            ))
            .collect::<Vec<_>>(),
        [(
            "user.ts",
            "useCounter()",
            "memo(() => 0)",
            Primitive::CreateMemo
        )]
    );
}

/// With stable symbol ids, an edit of a module no chain reads leaves every
/// other file's answer unrun; an edit of the factory reruns exactly the files
/// whose chains read it.
#[test]
fn returned_invocations_rerun_exactly_the_files_that_read_an_edit() {
    let store = super::memo::build::MemoStore::new(false, true);
    let mut project = invocation_project();
    let (first, tables) = invocation_build(&project, &store).unwrap();
    assert_eq!(invocation_table(&tables).fresh, 3);
    assert!(project.files[2].0.ends_with("other.ts"));
    project.files[2].1 = project.files[2].1.replace("\"x\"", "\"y\"");
    let (second, tables) = invocation_build(&project, &store).unwrap();
    assert_eq!(first, second);
    let calls = invocation_table(&tables);
    assert_eq!(
        (
            calls.reused + calls.verified,
            calls.unchanged,
            calls.changed
        ),
        (2, 1, 0),
        "{calls:?}"
    );
    // The tracker stops being one: the factory's own answer and the user's
    // change; the unrelated module's is reused.
    project.files[0].1 = project.files[0].1.replace("make(() => 1)", "keep(() => 1)");
    let (third, tables) = invocation_build(&project, &store).unwrap();
    assert_ne!(first, third);
    let calls = invocation_table(&tables);
    assert_eq!(
        (calls.reused + calls.verified, calls.changed),
        (1, 2),
        "{calls:?}"
    );
    let fresh = super::memo::build::MemoStore::new(false, false);
    let (expected, _) = invocation_build(&project, &fresh).unwrap();
    assert_eq!(third, expected);
}

/// Through random edits, the retained store's answers equal a fresh store's,
/// with verify-reused mode on.
#[test]
fn returned_invocations_through_random_edits_equal_a_fresh_store() {
    let (mut steps, mut reused) = (0, 0);
    for fixture in FIXTURES {
        let mut original = Project {
            files: fixture_sources(fixture),
            schemes: HashMap::default(),
            unreferenced: Vec::new(),
        };
        original.files.extend(invocation_project().files);
        original.files.sort();
        for seed in 1..=2 {
            let mut random = Random(seed);
            let mut project = original.clone();
            let store = super::memo::build::MemoStore::new(true, true);
            invocation_build(&project, &store).expect("the fixture parses");
            for serial in 0..15 {
                let kind = KINDS[random.below(KINDS.len())];
                let mut next = project.clone();
                if edit(&mut next, kind, &mut random, serial).is_none() || facts(&next).is_none() {
                    continue;
                }
                project = next;
                let (answers, tables) = invocation_build(&project, &store).unwrap();
                let fresh = super::memo::build::MemoStore::new(false, false);
                let (expected, _) = invocation_build(&project, &fresh).unwrap();
                assert_eq!(
                    answers, expected,
                    "{fixture} seed {seed} step {serial} ({kind})"
                );
                for (table, stats) in &tables {
                    assert_eq!(
                        stats.mismatches, 0,
                        "{fixture} seed {seed} step {serial}: {table}"
                    );
                }
                let calls = invocation_table(&tables);
                reused += calls.reused + calls.verified;
                steps += 1;
            }
        }
    }
    assert!(steps > 120, "only {steps} edits applied");
    assert!(reused > 0, "{reused} reused");
}

/// The accessor-provenance resolver's memo build (S7b), over this harness.
#[path = "../accessor_provenance/tests/resolver_memos.rs"]
mod resolver_memos;
