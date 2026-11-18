//! ADR 1120: the plain-program premise, scoped by dependency reach, with its per-value answers. Measurement only: no proof asks it
//! yet, so no verdict depends on it. With `SOLID_CHECKER_BLOCKER_CENSUS=1` and `SOLID_CHECKER_PREMISE_CENSUS=1`, the internal JSON's
//! `blockerCensus` table gets one `plain-premise` row per entry module (a project file no analysed file loads) and one
//! `plain-premise-program` row.
//!
//! A hazard is a place that may break the premise in a file the entry's realm runs: the facts' premise sites (Proxy, prototype routes,
//! writes under a standard name, global writes, `Object.assign`, coercion hooks, accessors, Solid's private keys, shadow roots,
//! `with`), code built from strings (`code_sinks`), a computed key nothing proves (`computed_keys`), and a module the build does not
//! read (a value import, export-from or load that resolves outside the analysed files, unless the dialect owns it). The per-value
//! answers count apart, as resolved: a hook or accessor on a contained value (`PremiseSiteFact::contained`), and a computed key
//! proven to be constant texts none of which is a prototype route or a private key.
//!
//! An entry's realm is its closure over static value imports, export-froms and literal loads among the analysed files: test and story
//! files are entries of their own, so the tooling they load does not count against the application's entry (Fable's tier 2).

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use solid_facts::FileFacts;
use solid_facts::ast::{CodeSinkKind, ComputedKeySite, PremiseClause};

use crate::blocker_census::CensusRow;
use crate::indexes::SemanticLookup;

/// Whether the premise census is on (`SOLID_CHECKER_PREMISE_CENSUS=1`); it reports through the blocker census, which must be on too.
pub(crate) fn enabled() -> bool {
    std::env::var_os("SOLID_CHECKER_PREMISE_CENSUS").is_some_and(|value| value == "1")
}

/// One hazard: its clause, its key, and whether a per-value answer resolves it.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct Hazard {
    clause: String,
    key: String,
    resolved: bool,
}

const PROTOTYPE_KEYS: [&str; 3] = ["prototype", "__proto__", "constructor"];

fn clause_name(clause: PremiseClause) -> &'static str {
    match clause {
        PremiseClause::ProxyOrReflect => "proxy",
        PremiseClause::PrototypeRoute => "proto-route",
        PremiseClause::GlobalWrite => "global-write",
        PremiseClause::StandardKeyWrite => "standard-key-write",
        PremiseClause::ObjectAssign => "object-assign",
        PremiseClause::CoercionHook => "coercion-hook",
        PremiseClause::Accessor => "accessor",
        PremiseClause::PrivateKey => "private-key",
        PremiseClause::ShadowRoot => "shadow",
        PremiseClause::With => "with",
    }
}

fn sink_name(kind: CodeSinkKind) -> &'static str {
    match kind {
        CodeSinkKind::Eval => "eval",
        CodeSinkKind::FunctionConstructor => "function-constructor",
        CodeSinkKind::ConstructorCall => "constructor-call",
        CodeSinkKind::StringTimer => "string-timer",
        CodeSinkKind::ScriptElement => "script-element",
        CodeSinkKind::EventAttribute => "event-attribute",
        CodeSinkKind::MarkupWrite => "markup-write",
        CodeSinkKind::DynamicImport => "dynamic-import",
        CodeSinkKind::JavascriptUrl => "javascript-url",
        CodeSinkKind::GlobalLookup => "global-lookup",
    }
}

/// What one load of a file names.
enum Load {
    /// An analysed file.
    Read(String),
    /// A package whose analysed files are these (TypeScript resolved the load to the package's declarations; the build reads its
    /// runtime files). Every analysed file of the package counts as loaded: more reach, never less.
    Package(Vec<String>),
    /// A style sheet (`.css`, `.scss`, …): the bundler injects rules, no code.
    Style(String),
    /// Another asset (`.svg`, `.png`, …): a URL by default, but a bundler plugin may make it code.
    Asset(String),
    /// A module the build does not read: outside the analysed files, external, built in, unknown or unresolved.
    Unread(String),
}

const STYLE_EXTENSIONS: [&str; 6] = [".css", ".scss", ".sass", ".less", ".styl", ".pcss"];
const ASSET_EXTENSIONS: [&str; 12] = [
    ".svg", ".png", ".jpg", ".jpeg", ".gif", ".webp", ".avif", ".ico", ".woff", ".woff2", ".ttf",
    ".json",
];

fn asset_kind(specifier: &str) -> Option<bool> {
    let bare = specifier
        .split(['?', '#'])
        .next()
        .unwrap_or(specifier)
        .to_ascii_lowercase();
    if STYLE_EXTENSIONS
        .iter()
        .any(|extension| bare.ends_with(extension))
    {
        return Some(true);
    }
    ASSET_EXTENSIONS
        .iter()
        .any(|extension| bare.ends_with(extension))
        .then_some(false)
}

/// The analysed files of the package a resolved path lies in (`…/node_modules/<name>/…`, a scoped name being two segments), matched
/// by `/node_modules/<name>/` anywhere in their path (a store layout's real path included); `None` when the build reads none.
fn package_files(resolved: &str, program: &BTreeSet<&str>) -> Option<Vec<String>> {
    let (_, rest) = resolved.rsplit_once("/node_modules/")?;
    let mut segments = rest.split('/');
    let first = segments.next()?;
    let name = if first.starts_with('@') {
        format!("{first}/{}", segments.next()?)
    } else {
        first.to_owned()
    };
    let marker = format!("/node_modules/{name}/");
    let mut files: Vec<String> = program
        .iter()
        .filter(|path| path.contains(&marker))
        .map(|path| (*path).to_owned())
        .collect();
    files.sort_unstable();
    (!files.is_empty()).then_some(files)
}

/// The loads of a file. The bundler's own resolution answers when the host obtained it (ADR 0220; a missing row is unknown, never
/// TypeScript's); without it, TypeScript's attested resolution, where a declaration file is no runtime module.
fn loads(lookup: &SemanticLookup<'_>, file: &FileFacts, program: &BTreeSet<&str>) -> Vec<Load> {
    let runtime = lookup.runtime_resolutions();
    let imports = lookup.resolved_imports();
    let resolve = |declaration: solid_facts::core::Span,
                   at: Option<solid_facts::core::Span>,
                   specifier: &str|
     -> Option<Load> {
        if lookup.dialect.owns_module(specifier) {
            return None;
        }
        let read = |path: &str| {
            if program.contains(path) {
                Some(Load::Read(path.to_owned()))
            } else {
                package_files(path, program).map(Load::Package)
            }
        };
        let found = if let Some(runtime) = runtime {
            at.and_then(
                |at| match runtime.outcome(file.path.as_str(), at, specifier) {
                    solid_facts::runtime_resolution::RuntimeOutcome::File {
                        path,
                        physical_path,
                    } => read(path).or_else(|| read(physical_path)),
                    _ => None,
                },
            )
        } else {
            imports.and_then(|imports| {
                match imports.specifier(file.path.as_str(), declaration, specifier) {
                    solid_facts::SpecifierAttestation::Attested(row)
                        if row.resolution != solid_facts::ImportResolution::Unresolved =>
                    {
                        read(row.resolved_path.as_ref())
                    }
                    _ => None,
                }
            })
        };
        Some(found.unwrap_or_else(|| match asset_kind(specifier) {
            Some(true) => Load::Style(specifier.to_owned()),
            Some(false) => Load::Asset(specifier.to_owned()),
            None => Load::Unread(specifier.to_owned()),
        }))
    };
    let ast = &file.ast;
    let mut out = Vec::new();
    for import in ast.imports.iter().filter(|import| !import.type_only) {
        out.extend(resolve(
            import.span,
            import.specifier_span,
            import.module.as_str(),
        ));
    }
    for export in ast.exports.iter().filter(|export| !export.type_only) {
        if let Some(module) = export.module.as_deref() {
            out.extend(resolve(export.span, export.specifier_span, module));
        }
    }
    for load in &ast.module_loads {
        match load.specifier.as_deref() {
            Some(specifier) => out.extend(resolve(load.span, load.specifier_span, specifier)),
            None => out.push(Load::Unread("*computed".into())),
        }
    }
    out
}

/// The hazards of one file, unread loads included.
fn file_hazards(
    lookup: &SemanticLookup<'_>,
    file: &FileFacts,
    program: &BTreeSet<&str>,
) -> Vec<Hazard> {
    let mut hazards = Vec::new();
    for site in &file.ast.premise_sites {
        hazards.push(Hazard {
            clause: clause_name(site.clause).into(),
            key: site.key.to_string(),
            resolved: site.contained,
        });
    }
    for sink in &file.ast.code_sinks {
        hazards.push(Hazard {
            clause: "code-from-string".into(),
            key: sink_name(sink.kind).into(),
            resolved: false,
        });
    }
    for key in &file.ast.computed_keys {
        let site = match key.site {
            ComputedKeySite::Member => "member",
            ComputedKeySite::Pattern => "pattern",
            ComputedKeySite::ReflectGet => "reflect-get",
        };
        let dangerous = key.texts.iter().any(|text| {
            PROTOTYPE_KEYS.contains(&text.as_str()) || text.contains("$$") || text.starts_with("_$")
        });
        hazards.push(Hazard {
            clause: "computed-key".into(),
            key: site.into(),
            resolved: key.proven && !dangerous,
        });
    }
    for load in loads(lookup, file, program) {
        let (clause, key, resolved) = match load {
            Load::Read(_) | Load::Package(_) => continue,
            Load::Style(specifier) => ("style-module", specifier, true),
            Load::Asset(specifier) => ("asset-module", specifier, false),
            Load::Unread(specifier) => ("unread-module", specifier, false),
        };
        hazards.push(Hazard {
            clause: clause.into(),
            key,
            resolved,
        });
    }
    hazards
}

/// The package a file of `node_modules` belongs to (a scoped name being two segments).
fn package_name(path: &str) -> Option<&str> {
    let (_, rest) = path.rsplit_once("/node_modules/")?;
    let first = rest.split('/').next()?;
    if first.starts_with('@') {
        let end = rest
            .match_indices('/')
            .nth(1)
            .map_or(rest.len(), |(at, _)| at);
        Some(&rest[..end])
    } else {
        Some(first)
    }
}

fn is_package(path: &str) -> bool {
    path.contains("/node_modules/")
}

/// The census rows (module docs).
pub(crate) fn census(lookup: &SemanticLookup<'_>) -> Vec<CensusRow> {
    let files = lookup.files();
    let program: BTreeSet<&str> = files.iter().map(|file| file.path.as_str()).collect();
    let mut edges: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    let mut loaded: BTreeSet<String> = BTreeSet::new();
    let mut hazards: BTreeMap<&str, Vec<Hazard>> = BTreeMap::new();
    for file in files {
        let targets: Vec<String> = loads(lookup, file, &program)
            .into_iter()
            .flat_map(|load| match load {
                Load::Read(path) => vec![path],
                Load::Package(paths) => paths,
                _ => Vec::new(),
            })
            .filter(|target| target != file.path.as_str())
            .collect();
        loaded.extend(targets.iter().cloned());
        edges.insert(file.path.as_str(), targets);
        hazards.insert(file.path.as_str(), file_hazards(lookup, file, &program));
    }
    let mut roots: Vec<&str> = files
        .iter()
        .map(|file| file.path.as_str())
        .filter(|path| !is_package(path) && !loaded.contains(*path))
        .collect();
    if roots.is_empty() {
        // A cycle with no entry: every project file is one.
        roots = files
            .iter()
            .map(|file| file.path.as_str())
            .filter(|path| !is_package(path))
            .collect();
    }
    roots.sort_unstable();
    let summarize = |reach: &BTreeSet<&str>| -> (Vec<String>, bool) {
        let mut counts: BTreeMap<(String, bool), usize> = BTreeMap::new();
        let mut keys: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        // The distinct open keys of each clause, and the open sites in project (not package) files.
        let mut distinct: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        let mut project_open: BTreeMap<String, usize> = BTreeMap::new();
        for path in reach {
            for hazard in hazards.get(path).into_iter().flatten() {
                *counts
                    .entry((hazard.clause.clone(), hazard.resolved))
                    .or_default() += 1;
                if !hazard.resolved {
                    let set = distinct.entry(hazard.clause.clone()).or_default();
                    if set.len() < 500 {
                        // An any-key site names where it is: `*@project`, or `*@<package>`.
                        let key = if hazard.key == "*" {
                            format!("*@{}", package_name(path).unwrap_or("project"))
                        } else {
                            hazard.key.clone()
                        };
                        set.insert(key.replace(['|', ','], "_"));
                    }
                    if !is_package(path) {
                        *project_open.entry(hazard.clause.clone()).or_default() += 1;
                    }
                }
                if !hazard.resolved {
                    let set = keys.entry(hazard.clause.clone()).or_default();
                    if set.len() < 8 {
                        set.insert(format!(
                            "{}@{}",
                            hazard.key,
                            path.rsplit('/').next().unwrap_or(path)
                        ));
                    }
                }
            }
        }
        let packages = reach.iter().filter(|path| is_package(path)).count();
        let mut blockers = vec![format!("files|{}|packages|{}", reach.len(), packages)];
        let mut plain = true;
        let clauses: BTreeSet<&String> = counts.keys().map(|(clause, _)| clause).collect();
        for clause in clauses {
            let open = counts.get(&(clause.clone(), false)).copied().unwrap_or(0);
            let resolved = counts.get(&(clause.clone(), true)).copied().unwrap_or(0);
            plain &= open == 0;
            let sample = keys
                .get(clause)
                .map(|set| set.iter().cloned().collect::<Vec<_>>().join(","))
                .unwrap_or_default();
            let project = project_open.get(clause).copied().unwrap_or(0);
            blockers.push(format!(
                "clause|{clause}|open|{open}|resolved|{resolved}|{sample}|project|{project}"
            ));
            if let Some(set) = distinct.get(clause) {
                blockers.push(format!(
                    "keys|{clause}|{}",
                    set.iter().cloned().collect::<Vec<_>>().join(",")
                ));
            }
        }
        if plain {
            blockers.push("plain".into());
        }
        (blockers, plain)
    };
    let mut rows = Vec::new();
    let mut plain_roots = 0usize;
    let mut union: BTreeSet<&str> = BTreeSet::new();
    for root in &roots {
        let mut reach: BTreeSet<&str> = BTreeSet::new();
        let mut queue = VecDeque::from([*root]);
        while let Some(path) = queue.pop_front() {
            if !reach.insert(path) {
                continue;
            }
            for target in edges.get(path).into_iter().flatten() {
                if let Some(target) = program.get(target.as_str()) {
                    queue.push_back(target);
                }
            }
        }
        let (blockers, plain) = summarize(&reach);
        plain_roots += usize::from(plain);
        union.extend(reach.iter().copied());
        rows.push(CensusRow {
            path: (*root).to_owned(),
            start: 0,
            end: 0,
            kind: "plain-premise".into(),
            blockers,
        });
    }
    let all: BTreeSet<&str> = files.iter().map(|file| file.path.as_str()).collect();
    let (mut blockers, _) = summarize(&all);
    blockers.insert(
        0,
        format!(
            "roots|{}|plain-roots|{}|unreached-files|{}",
            roots.len(),
            plain_roots,
            all.len() - union.len()
        ),
    );
    rows.push(CensusRow {
        path: String::new(),
        start: 0,
        end: 0,
        kind: "plain-premise-program".into(),
        blockers,
    });
    rows
}

/// ADR 1120 part 4: the per-question API. A proof asks a narrow question of the code that may run in the same realm as a file (the
/// union of the closures of every entry module whose closure holds it), instead of asking whether the whole program is plain: one
/// undecided package elsewhere answers nothing about this file. Entries and closures are those of [`census`] (static value loads,
/// the bundler's resolution or TypeScript's, a package by its analysed files).
pub(crate) struct Realms {
    files: Vec<String>,
    /// Each entry's closure, as indexes into `files`.
    closures: Vec<BTreeSet<usize>>,
}

/// The program's entries and their closures.
pub(crate) fn realms(lookup: &SemanticLookup<'_>) -> Realms {
    let all = lookup.files();
    let program: BTreeSet<&str> = all.iter().map(|file| file.path.as_str()).collect();
    let files: Vec<String> = all.iter().map(|file| file.path.to_string()).collect();
    let index: BTreeMap<&str, usize> = files
        .iter()
        .enumerate()
        .map(|(at, path)| (path.as_str(), at))
        .collect();
    let mut edges: Vec<Vec<usize>> = vec![Vec::new(); files.len()];
    let mut loaded = vec![false; files.len()];
    for (at, file) in all.iter().enumerate() {
        for load in loads(lookup, file, &program) {
            let targets = match load {
                Load::Read(path) => vec![path],
                Load::Package(paths) => paths,
                // Not an analysed file: no edge (its hazard is the file's own, `stored_handler_hazard`).
                Load::Style(_) | Load::Asset(_) | Load::Unread(_) => continue,
            };
            for target in targets {
                if let Some(&to) = index.get(target.as_str())
                    && to != at
                {
                    edges[at].push(to);
                    loaded[to] = true;
                }
            }
        }
    }
    let mut roots: Vec<usize> = (0..files.len())
        .filter(|&at| !is_package(&files[at]) && !loaded[at])
        .collect();
    if roots.is_empty() {
        roots = (0..files.len())
            .filter(|&at| !is_package(&files[at]))
            .collect();
    }
    let closures = roots
        .into_iter()
        .map(|root| {
            let mut reach = BTreeSet::new();
            let mut queue = VecDeque::from([root]);
            while let Some(at) = queue.pop_front() {
                if reach.insert(at) {
                    queue.extend(edges[at].iter().copied());
                }
            }
            reach
        })
        .collect();
    Realms { files, closures }
}

/// For each analysed file, the first hazard `question` finds in the realm that may run it: `(file, why)`. A file in no entry's closure
/// (code nothing loads) is answered as its own realm.
pub(crate) fn ask_by_realm(
    lookup: &SemanticLookup<'_>,
    realms: &Realms,
    question: &dyn Fn(&FileFacts) -> Option<&'static str>,
) -> BTreeMap<String, (String, &'static str)> {
    let all = lookup.files();
    let found: Vec<Option<&'static str>> = all.iter().map(question).collect();
    let mut answer: BTreeMap<String, (String, &'static str)> = BTreeMap::new();
    let mut in_realm = vec![false; realms.files.len()];
    for closure in &realms.closures {
        let hazard = closure
            .iter()
            .find_map(|&at| found[at].map(|why| (realms.files[at].clone(), why)));
        for &at in closure {
            in_realm[at] = true;
            if let Some(hazard) = &hazard {
                answer
                    .entry(realms.files[at].clone())
                    .or_insert_with(|| hazard.clone());
            }
        }
    }
    for (at, seen) in in_realm.iter().enumerate() {
        if !seen && let Some(why) = found[at] {
            answer.insert(realms.files[at].clone(), (realms.files[at].clone(), why));
        }
    }
    answer
}

/// The standard readers of own keys: a call of one may hand out the key Solid stores a handler under.
const KEY_EXPOSING_CALLS: &[&str] = &[
    "ObjectConstructor.keys",
    "ObjectConstructor.values",
    "ObjectConstructor.entries",
    "ObjectConstructor.getOwnPropertyNames",
    "ObjectConstructor.getOwnPropertyDescriptor",
    "ObjectConstructor.getOwnPropertyDescriptors",
    "ObjectConstructor.getOwnPropertySymbols",
    "ObjectConstructor.assign",
    "Reflect.ownKeys",
    "Reflect.get",
    "Reflect.getOwnPropertyDescriptor",
];

/// The listener API whose replacement keeps or runs a handler: a write under one of these names may be a patch of `EventTarget`.
const LISTENER_API: [&str; 3] = ["addEventListener", "removeEventListener", "dispatchEvent"];

/// ADR 1089 part E, round 2: why code of this file may reach a handler rc.21 `@solidjs/web` stored on an element
/// (`node["_$$" + name]`, `node["_$$" + name + "Data"]`, `dist/web.dev.js:480, 790-799`), or run a non-delegated one other than by
/// event delivery, or deliver it with `window.event` unset; `None` when it may do none. Every value may be an element here. Keys are
/// compared by their cooked value (ADR 1120 K3), never by source text:
/// - a premise site: Proxy or `Reflect`, a prototype route (`prototype`, `defineProperty`, `getPrototypeOf`, …), `Object.assign` onto
///   a value not made in place, a key holding `$$` or starting `_$` (read or written), a shadow-root spelling, `with`, a write under a
///   listener-API name or under a key no proof narrows, a global write of a listener-API name;
/// - code built from a string (`code_sinks`);
/// - a computed key nothing proves, or proven to be a text holding `$$` or starting `_$` (`"_$$" + "click"`);
/// - an object pattern with a rest or a key that is no plain name, an assigned object pattern, a spread that copies own keys (an
///   object's or a JSX spread attribute), a `for … in` (a text test, which only refuses more), a standard reader of own keys;
/// - a module the build does not read (and an asset a plugin may make code).
pub(crate) fn stored_handler_hazard(
    lookup: &SemanticLookup<'_>,
    file: &FileFacts,
) -> Option<&'static str> {
    use solid_facts::ast::PremiseClause as C;
    let ast = &file.ast;
    for site in &ast.premise_sites {
        let hit = match site.clause {
            C::ProxyOrReflect => Some("proxy"),
            C::PrototypeRoute => Some("prototype-route"),
            C::ObjectAssign => Some("object-assign"),
            C::PrivateKey => Some("private-key"),
            C::ShadowRoot => Some("shadow-root"),
            C::With => Some("with"),
            C::StandardKeyWrite | C::GlobalWrite
                if site.key == "*" || LISTENER_API.contains(&site.key.as_str()) =>
            {
                Some("listener-api-write")
            }
            C::StandardKeyWrite | C::GlobalWrite | C::CoercionHook | C::Accessor => None,
        };
        if hit.is_some() {
            return hit;
        }
    }
    if !ast.code_sinks.is_empty() {
        return Some("code-from-string");
    }
    if ast.computed_keys.iter().any(|key| {
        !key.proven
            || key
                .texts
                .iter()
                .any(|text| text.contains("$$") || text.starts_with("_$"))
    }) {
        return Some("computed-key");
    }
    let patterns = ast
        .bindings
        .iter()
        .chain(
            ast.functions
                .iter()
                .flat_map(|function| function.parameters.iter()),
        )
        .any(|binding| {
            binding.shape == solid_facts::ast::BindingShape::Object
                && (binding.names.len() != binding.object_slots.len()
                    || binding
                        .object_slots
                        .iter()
                        .any(|slot| !slot.plain_identifier_key))
        });
    if patterns {
        return Some("object-pattern");
    }
    if ast.assignments.iter().any(|assignment| {
        file.source_text(ast.peel_ts_sugar_span(assignment.target))
            .is_some_and(|target| target.trim_start_matches('(').trim_start().starts_with('{'))
    }) {
        return Some("assigned-pattern");
    }
    // An array element's or a call argument's spread iterates its operand (`[...xs]`, `f(...xs)`): it exposes no key. Any other
    // spread copies own keys (an object literal's), and so does a JSX spread attribute.
    let same_spread = |at: solid_facts::core::Span, spread: &solid_facts::ast::SpreadFact| {
        at == spread.span || (spread.span.contains(at) && at.contains(spread.argument))
    };
    let iterated = |spread: &solid_facts::ast::SpreadFact| {
        ast.array_literal_elements.iter().any(|literal| {
            literal.elements.iter().any(|element| {
                element.kind == solid_facts::ast::ArrayElementKind::Spread
                    && same_spread(element.span, spread)
            })
        }) || ast.calls.iter().any(|call| {
            call.arguments
                .iter()
                .any(|argument| argument.spread && same_spread(argument.span, spread))
        })
    };
    if ast.spreads.iter().any(|spread| !iterated(spread))
        || ast
            .jsx_elements
            .iter()
            .any(|element| !element.spreads.is_empty())
    {
        return Some("spread");
    }
    if source_has_for_in(&file.source) {
        return Some("for-in");
    }
    if ast.calls.iter().any(|call| {
        lookup
            .resolved_callee_call(file, call.callee)
            .and_then(|resolved| resolved.declaration.as_ref())
            .is_some_and(|declaration| {
                declaration.standard_library
                    && KEY_EXPOSING_CALLS.contains(&declaration.qualified_name.as_ref())
            })
    }) {
        return Some("key-call");
    }
    let program: BTreeSet<&str> = lookup
        .files()
        .iter()
        .map(|file| file.path.as_str())
        .collect();
    if loads(lookup, file, &program)
        .iter()
        .any(|load| matches!(load, Load::Unread(_) | Load::Asset(_)))
    {
        return Some("unread-module");
    }
    None
}

/// Whether `source` has a `for (` whose head, up to its first `;` or `)`, holds ` in ` (a superset of `for … in`).
fn source_has_for_in(source: &str) -> bool {
    source.match_indices("for").any(|(at, _)| {
        let before = source[..at].chars().next_back();
        let rest = source[at + 3..].trim_start();
        if before.is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '$')
            || !rest.starts_with('(')
        {
            return false;
        }
        let head = &rest[1..];
        let end = head.find([';', ')']).unwrap_or(head.len());
        head[..end].split_whitespace().any(|word| word == "in")
    })
}
