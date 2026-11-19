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
        PremiseClause::KeyReader => "key-reader",
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
    // ADR 1125: the first reason the program's event delivery is not proven intact.
    if let Some((path, why, key)) = crate::indexes::event_delivery::delivery_hazard_reason(lookup) {
        rows.push(CensusRow {
            path,
            start: 0,
            end: 0,
            kind: "event-delivery-hazard".into(),
            blockers: vec![format!("{why}|{key}")],
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
