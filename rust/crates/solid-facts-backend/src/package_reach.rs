//! ADR 0972: may an installed package the build does not read reach a Solid action? Decided from its source, per package, by walking the
//! closure of every runtime file it can load.
//!
//! The entries are a superset of what any resolver can select: every target of `exports` under any condition (patterns expanded over the
//! files they match), `main`, `module`, the files of an object `browser` field, and `index.js` when none is named. A superset never clears a
//! package the selected entry could not clear, so no host's conditions are assumed. Each runtime file is scanned by
//! [`solid_facts::module_reach::scan_module`] (Oxc: loads and hazards, never text). A load is followed as a bundler follows it: a relative path
//! to the file it names (its extensions, an `index` file, a directory's manifest), `#` to the package's `imports` (every target), a bare
//! name through Node's lookup (and package self-reference) to that package's entries, a Node builtin to nothing. The walk ends at the first:
//!
//! - load of a Solid package (the dialect's runtime and ecosystem packages): the package reaches Solid;
//! - hazard of a scanned file, a load nothing installed answers, a native module, a manifest it does not read, or more than
//!   [`MAX_CLOSURE_FILES`] files: undecided.
//!
//! A package whose whole closure is walked with none of these is clear: no code it can run names a Solid package, so it cannot call an
//! action. A declaration in `@types/<name>` describes the code of `<name>`, looked up from where the types are installed; one in a package of
//! global declarations only (no module) names no code, and is undecided (who provides a global is not said); a declaration in a declaration
//! file of the project itself (`declare module`, `declare function`) describes code no package holds, and is undecided.
//!
//! File scans are kept for the life of the process, keyed by path and checked against the file's length and modification time, so a daemon
//! scans a package once.
//!
//! One computation walks the closure of every installed package, and the closures overlap (a package's dependencies are in the closure of each package that
//! loads it), so what a walk reads is worked out once per computation: the lookups of the file system (a manifest, a resolution, a canonical path, the package a
//! name finds) are answered from one snapshot of the tree, and a file's visit (its hazards, accessors and global writes, and the files and package entries its
//! loads name, as steps in the order the visit meets them) is kept in a [`Memo`] that every walk holding the file replays. A walk finds the same files in the same
//! order and notes the same problems as one that read everything itself. The files one step out of a walk are scanned side by side before the walk visits them.

use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock, PoisonError};
use std::time::SystemTime;

use solid_facts::module_reach::{ModuleHazardKind, ModuleScan, scan_module};
use solid_facts::package_reach::{
    AccessorFacts, PackageReach, PackageReachIndex, ReachBlocker, is_declaration_file,
    package_root_of,
};

use crate::host_constants::{find_package_root, split_bare_specifier};

/// How many files one package's closure walk reads before it stops undecided.
pub const MAX_CLOSURE_FILES: usize = 20_000;

/// Runtime files the walk scans.
const SCRIPT_EXTENSIONS: &[&str] = &[".js", ".mjs", ".cjs", ".jsx", ".ts", ".tsx", ".mts", ".cts"];
/// Files that hold no code the walk does not see: their load runs nothing a package wrote.
const CODE_FREE_EXTENSIONS: &[&str] = &[
    ".json", ".css", ".scss", ".sass", ".less", ".styl", ".svg", ".png", ".jpg", ".jpeg", ".gif",
    ".webp", ".avif", ".ico", ".bmp", ".woff", ".woff2", ".ttf", ".otf", ".eot", ".txt", ".md",
    ".html", ".map",
];
/// A load the walk cannot see into.
const NATIVE_EXTENSIONS: &[&str] = &[".node", ".wasm"];
/// The extensions a bundler tries after a relative specifier.
const RESOLVE_EXTENSIONS: &[&str] = &[
    ".js", ".mjs", ".cjs", ".jsx", ".ts", ".tsx", ".mts", ".cts", ".json",
];

/// Node 22's `module.builtinModules`, unprefixed: a bare load of one that no installed package answers loads a builtin.
const NODE_BUILTINS: &[&str] = &[
    "assert",
    "assert/strict",
    "async_hooks",
    "buffer",
    "child_process",
    "cluster",
    "console",
    "constants",
    "crypto",
    "dgram",
    "diagnostics_channel",
    "dns",
    "dns/promises",
    "domain",
    "events",
    "fs",
    "fs/promises",
    "http",
    "http2",
    "https",
    "inspector",
    "inspector/promises",
    "module",
    "net",
    "os",
    "path",
    "path/posix",
    "path/win32",
    "perf_hooks",
    "process",
    "punycode",
    "querystring",
    "readline",
    "readline/promises",
    "repl",
    "stream",
    "stream/consumers",
    "stream/promises",
    "stream/web",
    "string_decoder",
    "sys",
    "timers",
    "timers/promises",
    "tls",
    "trace_events",
    "tty",
    "url",
    "util",
    "util/types",
    "v8",
    "vm",
    "wasi",
    "worker_threads",
    "zlib",
];

/// Which package names are Solid's: the dialect's primitive-defining packages and its ecosystem scopes.
pub struct SolidPackages<'d> {
    pub names: &'d [&'d str],
    pub scopes: &'d [&'d str],
}

impl SolidPackages<'_> {
    fn contains(&self, name: &str) -> bool {
        self.names.contains(&name) || self.scopes.iter().any(|scope| name.starts_with(scope))
    }
}

/// The reach verdicts for every installed package the project's facts name a declaration in.
///
/// A walk starts from what the program loads: the subpaths of each package its attested imports name (`pkg`, `pkg/sub`), each resolved through the
/// package's `exports` under every condition, so that no host's conditions are assumed and a subpath nothing imports is not walked. A package named
/// by declarations alone (a type another package's declarations mention) has no import to read: every entry stands. A package with no `exports`
/// map can be imported by a deep path no entry reaches, so the runtime files beside the declarations the program names (`dist/util.d.ts` beside
/// `dist/util.js`) are entries too.
#[must_use]
pub fn package_reach_index(
    facts: &solid_facts::ProjectFacts,
    solid: &SolidPackages<'_>,
) -> PackageReachIndex {
    let _reading = Reading::fresh();
    // Each declaration root, with the runtime root that holds its code, the runtime files beside its declarations and the subpaths the program imports.
    struct Named {
        runtime: Result<PathBuf, PackageReach>,
        siblings: BTreeSet<PathBuf>,
        subpaths: BTreeSet<String>,
    }
    let mut roots = BTreeMap::<String, Named>::new();
    let (installed, project_declarations) = declaration_paths(facts);
    for path in installed {
        let Some(root) = package_root_of(&path) else {
            continue;
        };
        let named = roots.entry(root.to_owned()).or_insert_with(|| Named {
            runtime: runtime_root(Path::new(root)),
            siblings: BTreeSet::new(),
            subpaths: BTreeSet::new(),
        });
        named.siblings.extend(runtime_siblings(&path));
    }
    // The subpaths of every attested import of a bare specifier, an analyzed package's own imports included.
    if let Some(imports) = &facts.resolved_imports {
        for (_, import) in imports.iter() {
            if import.resolved_path.is_empty() {
                continue;
            }
            let Some((_, subpath)) = split_bare_specifier(&import.text) else {
                continue;
            };
            let path = if import.symlink_path.is_empty() {
                &import.resolved_path
            } else {
                &import.symlink_path
            };
            if let Some(named) = package_root_of(path).and_then(|root| roots.get_mut(root)) {
                named.subpaths.insert(subpath);
            }
        }
    }
    // One verdict per runtime root, over the entries and the siblings of every declaration root that names it.
    let mut by_runtime = BTreeMap::<PathBuf, (Demand, BTreeSet<PathBuf>)>::new();
    for (root, named) in &roots {
        if let Ok(runtime) = &named.runtime {
            let (demand, siblings) = by_runtime.entry(runtime.clone()).or_default();
            demand.subpaths.extend(named.subpaths.iter().cloned());
            demand.everything |= named.subpaths.is_empty();
            if Path::new(root) == runtime.as_path() {
                siblings.extend(named.siblings.iter().cloned());
            }
        }
    }
    let mut memo = Memo::default();
    let verdicts = by_runtime
        .into_iter()
        .map(|(runtime, (demand, siblings))| {
            let closure = package_reach(&runtime, solid, &demand, &siblings, &mut memo);
            (runtime, closure)
        })
        .collect::<HashMap<_, _>>();
    let mut index = PackageReachIndex::default();
    for (root, named) in roots {
        let (reach, accessors) = match &named.runtime {
            Ok(runtime) => match verdicts.get(runtime) {
                Some(closure) => (closure.reach.clone(), closure.accessors.clone()),
                None => (
                    missing(format!("no verdict for {}", short(runtime))),
                    AccessorFacts::open(),
                ),
            },
            Err(reach) => (reach.clone(), AccessorFacts::open()),
        };
        index.accessors.insert(root.clone(), accessors);
        if let Ok(runtime) = &named.runtime
            && let Some(closure) = verdicts.get(runtime)
        {
            for written in &closure.global_writes.names {
                index
                    .global_writers
                    .entry(written.clone())
                    .or_default()
                    .insert(root.clone());
            }
            if closure.global_writes.any_name {
                index.global_writers_any.insert(root.clone());
            }
        }
        if let Some(name) = manifest(Path::new(&root))
            .ok()
            .and_then(|manifest| manifest.get("name")?.as_str().map(str::to_owned))
        {
            index.names.insert(root.clone(), name);
        }
        index.by_root.insert(root, reach);
    }
    index.linked = linked_roots(facts);
    index.project_js = project_js(facts, &project_declarations);
    // ADR 0973: the reviewed premises whose files match the installed bytes.
    let installed_roots = index
        .by_root
        .keys()
        .filter_map(|root| Some((root.clone(), Path::new(root).canonicalize().ok()?)))
        .collect::<BTreeMap<_, _>>();
    index.reviewed = crate::reviewed_packages::verified_premises(
        &installed_roots,
        crate::reviewed_packages::PREMISES,
    );
    index
}

/// The linked workspace packages the project imports: for each attested import of a `node_modules` link (the compiler's `symlink_path`), the real
/// directory of the link's package root, keyed to the link's root ([`linked_root`]). The compiler gives the package's declarations that real path,
/// which lies in no `node_modules`, so without this the code behind them reads as the project's own.
fn linked_roots(facts: &solid_facts::ProjectFacts) -> BTreeMap<String, String> {
    let mut linked = BTreeMap::new();
    let Some(imports) = &facts.resolved_imports else {
        return linked;
    };
    for (importer, import) in imports.iter() {
        if importer.contains("/node_modules/") || facts.dependency_source_root(importer).is_some() {
            continue;
        }
        if let Some((real, link)) = linked_root(&import.symlink_path, &import.resolved_path) {
            linked.insert(real, link);
        }
    }
    linked
}

/// The real directory and the link's root of an import the compiler resolved through a `node_modules` link: the link's package root, canonicalized.
/// Only a real directory the compiler's own resolved path lies in counts, and one outside every `node_modules` (a store slot's declarations already
/// are installed code, and a root that is no link has nothing to map).
fn linked_root(symlink_path: &str, resolved_path: &str) -> Option<(String, String)> {
    if symlink_path.is_empty() || resolved_path.is_empty() {
        return None;
    }
    let link_root = package_root_of(symlink_path)?;
    let real = Path::new(link_root)
        .canonicalize()
        .ok()?
        .to_str()?
        .to_owned();
    if real == link_root
        || real.contains("/node_modules/")
        || !resolved_path.starts_with(&format!("{real}/"))
    {
        return None;
    }
    Some((real, link_root.to_owned()))
}

/// The runtime files beside a declaration file of an installed package (`dist/util.d.ts`: `dist/util.js`, `.mjs`, `.cjs`, `.jsx`), and the file itself
/// when it is a script (an untyped package the compiler resolved to its JavaScript). Only files that exist, canonical.
fn runtime_siblings(path: &str) -> Vec<PathBuf> {
    let stem = path
        .strip_suffix(".d.ts")
        .or_else(|| path.strip_suffix(".d.mts"))
        .or_else(|| path.strip_suffix(".d.cts"));
    let mut found = Vec::new();
    match stem {
        Some(stem) => {
            for extension in [".js", ".mjs", ".cjs", ".jsx"] {
                let candidate = PathBuf::from(format!("{stem}{extension}"));
                if candidate.is_file()
                    && let Ok(canonical) = candidate.canonicalize()
                {
                    found.push(canonical);
                }
            }
        }
        None => {
            if SCRIPT_EXTENSIONS
                .iter()
                .any(|extension| path.ends_with(extension))
                && let Ok(canonical) = Path::new(path).canonicalize()
            {
                found.push(canonical);
            }
        }
    }
    found
}

/// The files the project's own files name a declaration in: each call's resolved declaration (outside the standard library), each referenced symbol's
/// declarations, through its alias, and each attested import's file. The installed ones (under `node_modules`) and, apart, the project's own declaration
/// files (outside every `node_modules`, not a package read as source): runtime code may sit beside those ([`project_js`]).
fn declaration_paths(facts: &solid_facts::ProjectFacts) -> (BTreeSet<String>, BTreeSet<String>) {
    let mut installed = BTreeSet::new();
    let mut project = BTreeSet::new();
    let mut keep = |path: &str| {
        if path.starts_with("bundled:") {
            return;
        }
        if path.contains("/node_modules/") {
            installed.insert(path.to_owned());
        } else if is_declaration_file(path) && facts.dependency_source_root(path).is_none() {
            project.insert(path.to_owned());
        }
    };
    // The files the compiler resolved the project's imports to: a tag or a member no entity names is decided through its import. A linked
    // workspace package resolves to its real path; the `node_modules` spelling of the link names it.
    if let Some(imports) = &facts.resolved_imports {
        for (importer, import) in imports.iter() {
            if importer.contains("/node_modules/")
                || facts.dependency_source_root(importer).is_some()
            {
                continue;
            }
            if import.symlink_path.is_empty() {
                keep(&import.resolved_path);
            } else {
                keep(&import.symlink_path);
            }
        }
    }
    for file in &facts.files {
        let path = file.path.as_str();
        if path.contains("/node_modules/") || facts.dependency_source_root(path).is_some() {
            continue;
        }
        for entity in facts.typescript.entities_for_path(path) {
            if let Some(declaration) = entity
                .resolved_call
                .as_ref()
                .and_then(|call| call.declaration.as_ref())
                && !declaration.standard_library
            {
                keep(&declaration.location.path);
            }
            if entity.symbol.is_empty() {
                continue;
            }
            let Some(symbol) = facts.typescript.symbol(&entity.symbol) else {
                continue;
            };
            for declaration in symbol.declarations() {
                keep(&declaration.location.path);
            }
            if !symbol.alias_target().is_empty()
                && let Some(target) = facts.typescript.symbol(symbol.alias_target())
            {
                for declaration in target.declarations() {
                    keep(&declaration.location.path);
                }
            }
        }
    }
    (installed, project)
}

/// The project declaration files of `declarations` with runtime JavaScript beside them that the program does not analyze (`vendor/step.d.ts` beside
/// `vendor/step.js`, `allowJs` off): the compiler resolves an import there to the declaration, and the code that runs is a file nobody reads. A sibling
/// the program analyzes (`analyzed`) is read, so the declaration is not one of these.
fn project_js(
    facts: &solid_facts::ProjectFacts,
    declarations: &BTreeSet<String>,
) -> BTreeSet<String> {
    let analyzed = facts
        .files
        .iter()
        .map(|file| file.path.as_str())
        .collect::<BTreeSet<_>>();
    let mut unread = declarations_with_unread_js(declarations, &analyzed);
    unread.extend(imports_of_unread_js(facts, &analyzed));
    unread
}

/// The runtime files a relative import of the project reaches that the program does not analyze, where the compiler resolved the import to no analyzed
/// source (an ambient `declare module "*.vendor.js"`, a declaration file): the bundler loads the file the specifier names, and nobody reads it. A dynamic
/// `import("./view.vendor.js")` is one too (ADR 0973: `lazy(() => import(…))`). Each file is named by its path as the import joins it and by its canonical path.
fn imports_of_unread_js(
    facts: &solid_facts::ProjectFacts,
    analyzed: &BTreeSet<&str>,
) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    for file in &facts.files {
        let importer = file.path.as_str();
        if importer.contains("/node_modules/") || facts.dependency_source_root(importer).is_some() {
            continue;
        }
        let imports = file
            .ast
            .imports
            .iter()
            .filter(|import| !import.type_only)
            .map(|import| (import.span, import.module.as_str()));
        let loads = file
            .ast
            .module_loads
            .iter()
            .filter(|load| load.kind == solid_facts::ast::ModuleLoadKind::DynamicImport)
            .filter_map(|load| {
                load.specifier
                    .as_deref()
                    .map(|text| (load.specifier_span.unwrap_or(load.span), text))
            });
        // `export { vendorStep } from "./step.vendor.js"`: a barrel loads the file as an import does.
        let reexports = file
            .ast
            .exports
            .iter()
            .filter(|export| !export.type_only)
            .filter_map(|export| export.module.as_deref().map(|text| (export.span, text)));
        for (at, text) in imports.chain(loads).chain(reexports) {
            if !(text.starts_with("./") || text.starts_with("../")) {
                continue;
            }
            // The compiler resolved it to a file the program analyzes: that source is read.
            let resolved = facts
                .resolved_imports
                .as_ref()
                .map(|attested| attested.specifier(importer, at, text));
            if let Some(solid_facts::SpecifierAttestation::Attested(row)) = resolved
                && analyzed.contains(&*row.resolved_path)
            {
                continue;
            }
            let base = solid_facts::package_reach::join_relative(importer, text);
            for candidate in solid_facts::package_reach::script_candidates(&base) {
                let path = Path::new(&candidate);
                if !path.is_file() {
                    continue;
                }
                let canonical = path
                    .canonicalize()
                    .ok()
                    .and_then(|real| real.to_str().map(str::to_owned))
                    .unwrap_or_else(|| candidate.clone());
                if !analyzed.contains(candidate.as_str()) && !analyzed.contains(canonical.as_str())
                {
                    found.insert(candidate);
                    found.insert(canonical);
                }
                break;
            }
        }
    }
    found
}

fn declarations_with_unread_js(
    declarations: &BTreeSet<String>,
    analyzed: &BTreeSet<&str>,
) -> BTreeSet<String> {
    declarations
        .iter()
        .filter(|path| {
            let siblings = runtime_siblings(path);
            !siblings.is_empty()
                && !siblings.iter().any(|sibling| {
                    sibling
                        .to_str()
                        .is_some_and(|sibling| analyzed.contains(sibling))
                })
        })
        .cloned()
        .collect()
}

/// The package whose code a declaration package describes: itself, or for `@types/<name>` the `<name>` Node finds from where the types are
/// installed.
fn runtime_root(root: &Path) -> Result<PathBuf, PackageReach> {
    let Some(name) = manifest(root)
        .ok()
        .and_then(|manifest| manifest.get("name")?.as_str().map(str::to_owned))
    else {
        return Ok(root.to_path_buf());
    };
    let Some(described) = name.strip_prefix("@types/") else {
        return Ok(root.to_path_buf());
    };
    let described = match described.split_once("__") {
        Some((scope, name)) => format!("@{scope}/{name}"),
        None => described.to_owned(),
    };
    // `@types/node` describes the host's builtins (`node:fs`, `Buffer`): the runtime, no package's code, and no Solid in it.
    if described == "node" {
        return Err(PackageReach::Clear);
    }
    match find_package_root(&root.join("package.json"), &described, None)
        .and_then(|root| root.canonicalize().ok())
    {
        Some(runtime) => Ok(runtime),
        // Types for globals (`@types/webxr`, `@types/google.maps`): declarations with no module in them and no package of that name. The declaration does not
        // say who provides a global at runtime: the host, a script, or a package that writes it (`window.__act = …`, which the review of #231 ran in Chrome:
        // a global-only declaration package was clear and the global called an action). Nothing here proves no installed package provides it, so a call of
        // one of its globals is undecided like any global the pinned libraries do not declare.
        None if declares_only_globals(root) => Err(PackageReach::Undecided {
            blocker: ReachBlocker::UnknownGlobal,
            detail: format!(
                "{name} declares globals and no module: the host, a script or an installed package may provide them"
            ),
        }),
        None => Err(PackageReach::Undecided {
            blocker: ReachBlocker::MissingSource,
            detail: format!("{name} describes {described}, which is not installed"),
        }),
    }
}

/// Whether the declaration package at `root` is a script of global declarations: its entry declaration file (`types`, `typings`, `index.d.ts`) has no
/// top-level `import` or `export`, so it declares globals (`@types/webxr`), not a module an installed package implements. Who provides them is not said.
fn declares_only_globals(root: &Path) -> bool {
    let Ok(manifest) = manifest(root) else {
        return false;
    };
    let entry = ["types", "typings"]
        .iter()
        .find_map(|field| manifest.get(field).and_then(serde_json::Value::as_str))
        .map_or_else(|| root.join("index.d.ts"), |types| root.join(types));
    let Ok(source) = fs::read_to_string(&entry) else {
        return false;
    };
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(&allocator, &source, oxc_span::SourceType::d_ts()).parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return false;
    }
    !parsed.program.body.iter().any(|statement| {
        matches!(
            statement,
            oxc_ast::ast::Statement::ImportDeclaration(_)
                | oxc_ast::ast::Statement::ExportAllDeclaration(_)
                | oxc_ast::ast::Statement::ExportNamedDeclaration(_)
                | oxc_ast::ast::Statement::ExportDefaultDeclaration(_)
                | oxc_ast::ast::Statement::TSExportAssignment(_)
                | oxc_ast::ast::Statement::TSImportEqualsDeclaration(_)
        ) || matches!(statement, oxc_ast::ast::Statement::TSModuleDeclaration(module) if matches!(module.id, oxc_ast::ast::TSModuleDeclarationName::StringLiteral(_)))
    })
}

/// The manifests read during one [`package_reach_index`] computation, by directory: a manifest is read and parsed once however many probes of the walk
/// ask for it (every bare load, every self-reference check, every `main` of a directory).
type Manifests = HashMap<PathBuf, Result<Arc<serde_json::Value>, PackageReach>>;

/// What the walks of one [`package_reach_index`] computation have read of the file system: one snapshot of the tree as it stands while the computation runs. A
/// lookup answered once (a manifest, a resolution, a canonical path, the package that installs a name, the nearest manifest above a directory) is answered again
/// from here, and the threads that work ahead of a walk share it. The next computation starts empty.
#[derive(Default)]
struct Snapshot {
    manifests: Mutex<Manifests>,
    /// What [`resolve_path_following`] answered for a path and `follow_main`: the same target is named by many loads.
    resolved: Mutex<HashMap<(PathBuf, bool), Option<PathBuf>>>,
    /// What a path canonicalizes to (or why it does not).
    canonical: Mutex<HashMap<PathBuf, Result<PathBuf, String>>>,
    /// The directory that installs `name` for a file in a directory ([`find_package_root`]).
    package_roots: Mutex<HashMap<(PathBuf, String), Option<PathBuf>>>,
    /// The nearest directory at or above one that holds a `package.json`.
    manifest_directories: Mutex<HashMap<PathBuf, Option<PathBuf>>>,
}

thread_local! {
    /// The snapshot of the computation this thread works for.
    static READING: std::cell::RefCell<Option<Arc<Snapshot>>> = const { std::cell::RefCell::new(None) };
}

/// The snapshot this thread reads through: a thread that works for no computation reads through one of its own.
fn snapshot() -> Arc<Snapshot> {
    READING.with(|reading| Arc::clone(reading.borrow_mut().get_or_insert_with(Arc::default)))
}

/// A thread reads through a snapshot until this drops, then through the one it read through before.
struct Reading(Option<Arc<Snapshot>>);

impl Reading {
    fn through(snapshot: Arc<Snapshot>) -> Self {
        Self(READING.with(|reading| reading.borrow_mut().replace(snapshot)))
    }

    /// The snapshot of a computation that starts now: empty.
    fn fresh() -> Self {
        Self::through(Arc::default())
    }
}

impl Drop for Reading {
    fn drop(&mut self) {
        READING.with(|reading| *reading.borrow_mut() = self.0.take());
    }
}

fn locked<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// `path.canonicalize()`, once per path: the error as its text.
fn canonical(path: &Path) -> Result<PathBuf, String> {
    let snapshot = snapshot();
    let held = locked(&snapshot.canonical).get(path).cloned();
    if let Some(held) = held {
        return held;
    }
    let found = path.canonicalize().map_err(|error| error.to_string());
    locked(&snapshot.canonical).insert(path.to_path_buf(), found.clone());
    found
}

/// The nearest directory at or above `start` that holds a `package.json`, once per directory (every directory on the way to it shares the answer).
fn nearest_manifest_directory(start: &Path) -> Option<PathBuf> {
    let snapshot = snapshot();
    let mut walked = Vec::<PathBuf>::new();
    let mut found = None;
    let mut directory = Some(start);
    while let Some(current) = directory {
        let held = locked(&snapshot.manifest_directories).get(current).cloned();
        if let Some(held) = held {
            found = held;
            break;
        }
        walked.push(current.to_path_buf());
        if current.join("package.json").is_file() {
            found = Some(current.to_path_buf());
            break;
        }
        directory = current.parent();
    }
    let mut held = locked(&snapshot.manifest_directories);
    for directory in walked {
        held.insert(directory, found.clone());
    }
    found
}

/// [`find_package_root`] for a file, once per (directory, name): Node's lookup depends on the directory chain alone.
fn package_root_of_load(file: &Path, name: &str) -> Option<PathBuf> {
    let snapshot = snapshot();
    let directory = file.parent()?.to_path_buf();
    let key = (directory, name.to_owned());
    let held = locked(&snapshot.package_roots).get(&key).cloned();
    if let Some(held) = held {
        return held;
    }
    let found = find_package_root(file, name, None);
    locked(&snapshot.package_roots).insert(key, found.clone());
    found
}

fn manifest(root: &Path) -> Result<Arc<serde_json::Value>, PackageReach> {
    let snapshot = snapshot();
    let held = locked(&snapshot.manifests).get(root).cloned();
    if let Some(held) = held {
        return held;
    }
    let unread = |detail: String| PackageReach::Undecided {
        blocker: ReachBlocker::ManifestUnread,
        detail,
    };
    let read = fs::read(root.join("package.json"))
        .map_err(|error| unread(format!("{}: {error}", short(root))))
        .and_then(|bytes| {
            serde_json::from_slice(&bytes)
                .map(Arc::new)
                .map_err(|error| unread(format!("{}: {error}", short(root))))
        });
    locked(&snapshot.manifests).insert(root.to_path_buf(), read.clone());
    read
}

/// Which entries of a package a walk starts from.
#[derive(Clone, Debug, Default)]
struct Demand {
    /// The subpaths the program loads (`.`, `./sub`): only what an `exports` map (or the files themselves) names for them.
    subpaths: BTreeSet<String>,
    /// The package is named by its declarations alone, no load the compiler attested: every entry stands, whatever a resolver may pick.
    everything: bool,
}

#[cfg(test)]
impl Demand {
    /// What a program that loads one subpath of a package asks of it.
    fn subpath(subpath: &str) -> Self {
        Self {
            subpaths: BTreeSet::from([subpath.to_owned()]),
            everything: false,
        }
    }
}

/// What the walk of one package's closure found.
struct Closure {
    /// The verdict: the first thing that makes the closure reach Solid or not decided, in walk order, else clear.
    reach: PackageReach,
    /// Everything that makes it so, each once, in walk order (the verdict first).
    #[cfg_attr(not(test), allow(dead_code))]
    problems: Vec<PackageReach>,
    /// Where the closure may define an accessor.
    accessors: AccessorFacts,
    /// The globals the closure writes (ADR 0973).
    global_writes: solid_facts::module_reach::GlobalWrites,
}

/// The closure walk of one package. It reads the whole closure: a hazard, an unreadable file or a load of Solid ends nothing but that branch (a Solid package is
/// the dialect's runtime and is not walked), so what a later file holds is known whatever an earlier one did (the review of #231, S6: the first hazard hid a later
/// load of Solid, and the accessors a later file defines).
fn package_reach(
    root: &Path,
    solid: &SolidPackages<'_>,
    demand: &Demand,
    extra_entries: &BTreeSet<PathBuf>,
    memo: &mut Memo,
) -> Closure {
    let name = manifest(root)
        .ok()
        .and_then(|manifest| manifest.get("name")?.as_str().map(str::to_owned))
        .unwrap_or_default();
    if solid.contains(&name) {
        return Closure {
            reach: PackageReach::ReachesSolid { via: name.clone() },
            problems: vec![PackageReach::ReachesSolid { via: name }],
            accessors: AccessorFacts::open(),
            global_writes: solid_facts::module_reach::GlobalWrites::default(),
        };
    }
    let mut walk = Walk {
        solid,
        memo,
        files: IdSet::default(),
        queue: VecDeque::new(),
        entered: IdSet::default(),
        noted: IdSet::default(),
        problems: Vec::new(),
        accessor_names: BTreeSet::new(),
        accessors_any_name: false,
        written: BTreeSet::new(),
        written_any: false,
    };
    walk.run(root, demand, extra_entries);
    let problems = walk
        .problems
        .iter()
        .map(|&problem| walk.memo.problem(problem).clone())
        .collect::<Vec<_>>();
    // Only a load of Solid's runtime is a known value from outside the closure: any other problem leaves code, or a value, the walk did not read.
    let closed = problems
        .iter()
        .all(|problem| matches!(problem, PackageReach::ReachesSolid { .. }));
    Closure {
        reach: problems.first().cloned().unwrap_or(PackageReach::Clear),
        problems,
        accessors: AccessorFacts {
            closed,
            names: walk.accessor_names,
            any_name: walk.accessors_any_name,
        },
        global_writes: solid_facts::module_reach::GlobalWrites {
            names: walk.written.into_iter().collect(),
            any_name: walk.written_any,
        },
    }
}

/// The most distinct problems one walk keeps: the first is the verdict, the rest only say what else the closure holds.
const MAX_PROBLEMS: usize = 256;

/// A file a walk names, by the index the computation's [`Memo`] gave its path.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FileId(usize);

/// A problem a walk notes, by the index the [`Memo`] gave it: two equal problems are one.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ProblemId(usize);

/// A package entered at a subpath (`*` is every entry), by the index the [`Memo`] gave the pair.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct EntryId(usize);

/// What a [`PackageReach`] problem is, as a hash key: equal problems have equal keys.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum ProblemKey {
    Clear,
    ReachesSolid(String),
    Undecided(ReachBlocker, String),
}

impl ProblemKey {
    fn of(problem: &PackageReach) -> Self {
        match problem {
            PackageReach::Clear => Self::Clear,
            PackageReach::ReachesSolid { via } => Self::ReachesSolid(via.clone()),
            PackageReach::Undecided { blocker, detail } => {
                Self::Undecided(*blocker, detail.clone())
            }
        }
    }
}

/// A set of indexes, a bit each.
#[derive(Default)]
struct IdSet {
    words: Vec<u64>,
    len: usize,
}

impl IdSet {
    /// Adds `index`; whether it was not there.
    fn insert(&mut self, index: usize) -> bool {
        let (word, bit) = (index / 64, 1_u64 << (index % 64));
        if word >= self.words.len() {
            self.words.resize(word + 1, 0);
        }
        let fresh = self.words[word] & bit == 0;
        self.words[word] |= bit;
        self.len += usize::from(fresh);
        fresh
    }

    fn len(&self) -> usize {
        self.len
    }
}

/// One thing visiting a file does to a walk.
#[derive(Clone, Copy, Debug)]
enum Step {
    /// A problem the file or one of its loads has.
    Note(ProblemId),
    /// A file one of its loads names.
    Push(FileId),
    /// A package one of its loads names, entered at a subpath.
    Enter(EntryId),
}

/// What visiting one file does to a walk, in the order the visit meets it. A file's visit depends on the file and on the tree alone, so it is worked out once and
/// every closure that holds the file replays it.
struct Visit {
    /// What the file's scan says of the accessors and global writes a walk collects; `None` for a file nobody scans.
    scan: Option<Arc<ModuleScan>>,
    steps: Vec<Step>,
}

/// The entry files a package's subpath names, once read.
struct Entry {
    root: PathBuf,
    subpath: String,
    files: Option<Result<Rc<[FileId]>, ProblemId>>,
}

/// What the closure walks of one [`package_reach_index`] computation have worked out, each once: the files they name (by index), what visiting each does, the
/// distinct problems they find and the entries each package subpath names. Every package's closure is walked in full, and the closures overlap (a package's
/// dependencies are walked again in the closure of each package that loads it), so a file is visited, its loads resolved and its problems built a single time
/// and every walk that reaches it replays that: the walks answer as they would had each read everything itself.
#[derive(Default)]
struct Memo {
    paths: Vec<PathBuf>,
    file_ids: HashMap<PathBuf, FileId>,
    /// By file: what visiting it does, once worked out.
    visits: Vec<Option<Rc<Visit>>>,
    problems: Vec<PackageReach>,
    problem_ids: HashMap<ProblemKey, ProblemId>,
    entries: Vec<Entry>,
    entry_ids: HashMap<(PathBuf, String), EntryId>,
}

impl Memo {
    fn file(&mut self, path: PathBuf) -> FileId {
        if let Some(&id) = self.file_ids.get(&path) {
            return id;
        }
        let id = FileId(self.paths.len());
        self.file_ids.insert(path.clone(), id);
        self.paths.push(path);
        self.visits.push(None);
        id
    }

    fn path(&self, file: FileId) -> &Path {
        &self.paths[file.0]
    }

    fn problem_id(&mut self, problem: PackageReach) -> ProblemId {
        let key = ProblemKey::of(&problem);
        if let Some(&id) = self.problem_ids.get(&key) {
            return id;
        }
        let id = ProblemId(self.problems.len());
        self.problem_ids.insert(key, id);
        self.problems.push(problem);
        id
    }

    fn problem(&self, problem: ProblemId) -> &PackageReach {
        &self.problems[problem.0]
    }

    /// The pair of a package root (canonical) and a subpath.
    fn entry(&mut self, root: PathBuf, subpath: String) -> EntryId {
        let key = (root, subpath);
        if let Some(&id) = self.entry_ids.get(&key) {
            return id;
        }
        let id = EntryId(self.entries.len());
        self.entries.push(Entry {
            root: key.0.clone(),
            subpath: key.1.clone(),
            files: None,
        });
        self.entry_ids.insert(key, id);
        id
    }

    /// The files the entry names, read from the package's manifest the first time; what cannot be read is the problem.
    fn entry_files(&mut self, entry: EntryId) -> Result<Rc<[FileId]>, ProblemId> {
        if let Some(held) = &self.entries[entry.0].files {
            return held.clone();
        }
        let (root, subpath) = (
            self.entries[entry.0].root.clone(),
            self.entries[entry.0].subpath.clone(),
        );
        let read = if subpath == "*" {
            all_entries(&root)
        } else {
            subpath_entries(&root, &subpath)
        };
        let files = match read {
            Ok(paths) => Ok(paths
                .into_iter()
                .map(|path| self.file(path))
                .collect::<Rc<[FileId]>>()),
            Err(problem) => Err(self.problem_id(problem)),
        };
        self.entries[entry.0].files = Some(files.clone());
        files
    }

    /// What visiting `file` does to a walk, worked out the first time.
    fn visit(&mut self, solid: &SolidPackages<'_>, file: FileId) -> Rc<Visit> {
        if let Some(visit) = &self.visits[file.0] {
            return Rc::clone(visit);
        }
        let raw = record(&self.paths[file.0], solid);
        self.keep(file, raw)
    }

    /// Keeps what visiting `file` does, the files, problems and entries it names numbered.
    fn keep(&mut self, file: FileId, raw: RawVisit) -> Rc<Visit> {
        let steps = raw
            .steps
            .into_iter()
            .map(|step| match step {
                RawStep::Note(problem) => Step::Note(self.problem_id(problem)),
                RawStep::Push(path) => Step::Push(self.file(path)),
                RawStep::Enter(root, subpath) => Step::Enter(self.entry(root, subpath)),
            })
            .collect();
        let visit = Rc::new(Visit {
            scan: raw.scan,
            steps,
        });
        self.visits[file.0] = Some(Rc::clone(&visit));
        visit
    }
}

/// One thing visiting a file does to a walk, before the memo has numbered what it names.
enum RawStep {
    Note(PackageReach),
    Push(PathBuf),
    /// A package (canonical root) entered at a subpath.
    Enter(PathBuf, String),
}

/// What visiting a file does, as [`record`] found it.
struct RawVisit {
    scan: Option<Arc<ModuleScan>>,
    steps: Vec<RawStep>,
}

/// What visiting `file` does to a walk: its hazards are noted and its loads followed, in the order the visit meets them. It depends on the file, the dialect's Solid
/// packages and the tree alone, so any thread may work it out.
fn record(file: &Path, solid: &SolidPackages<'_>) -> RawVisit {
    let mut recorder = Recorder {
        solid,
        steps: Vec::new(),
    };
    let scan = match recorder.visit(file) {
        Ok(scan) => scan,
        Err(problem) => {
            recorder.note(problem);
            None
        }
    };
    RawVisit {
        scan,
        steps: recorder.steps,
    }
}

/// Works out what visiting one file does: its hazards are noted and its loads followed, as steps.
struct Recorder<'a, 'd> {
    solid: &'a SolidPackages<'d>,
    steps: Vec<RawStep>,
}

impl Recorder<'_, '_> {
    fn note(&mut self, problem: PackageReach) {
        self.steps.push(RawStep::Note(problem));
    }

    fn push(&mut self, file: PathBuf) {
        self.steps.push(RawStep::Push(file));
    }

    /// One file: its hazards are noted and its loads followed; `Err` when the file cannot be read as code. The scan, for a file that has one.
    fn visit(&mut self, file: &Path) -> Result<Option<Arc<ModuleScan>>, PackageReach> {
        let name = file.to_string_lossy();
        if NATIVE_EXTENSIONS
            .iter()
            .any(|extension| name.ends_with(extension))
        {
            return Err(PackageReach::Undecided {
                blocker: ReachBlocker::NativeModule,
                detail: short(file),
            });
        }
        if is_declaration_file(&name) || is_code_free(&name) {
            // A declaration file runs nothing; a stylesheet, an image or JSON holds no code a package wrote.
            return Ok(None);
        }
        if !is_script(&name) {
            // A file a bundler plugin compiles (`.vue`, `.svelte`) may become code that loads anything.
            return Err(PackageReach::Undecided {
                blocker: ReachBlocker::Unparseable,
                detail: format!("{}: a file type the walk does not read", short(file)),
            });
        }
        let scan = scanned(file)?;
        for hazard in &scan.hazards {
            self.note(PackageReach::Undecided {
                blocker: blocker_of(hazard.kind),
                detail: format!("{}: {}", short(file), hazard.detail),
            });
        }
        let directory = file.parent().unwrap_or(Path::new("/"));
        for specifier in &scan.loads {
            if let Err(problem) = self.load(file, directory, specifier) {
                self.note(problem);
            }
        }
        Ok(Some(scan))
    }

    /// One load of `file`: the files it names are pushed; `Err` when it names a Solid package (not walked: the dialect's runtime) or code nothing installed holds.
    fn load(&mut self, file: &Path, directory: &Path, specifier: &str) -> Result<(), PackageReach> {
        if specifier.starts_with("node:") {
            return Ok(());
        }
        if specifier.starts_with("data:")
            || specifier.starts_with("http:")
            || specifier.starts_with("https:")
        {
            return Err(PackageReach::Undecided {
                blocker: ReachBlocker::DynamicLoad,
                detail: format!("{} loads {specifier}", short(file)),
            });
        }
        if specifier == "."
            || specifier == ".."
            || specifier.starts_with("./")
            || specifier.starts_with("../")
            || specifier.starts_with('/')
        {
            // `./shader.frag?raw`, `./logo.svg?url`: a bundler's query on a file, read as a string or a URL, runs nothing; any other query names the
            // file the same way without it.
            let (specifier, query) = specifier.split_once('?').unwrap_or((specifier, ""));
            if matches!(query, "raw" | "url" | "inline" | "no-inline") {
                return Ok(());
            }
            let target = resolve_path(&directory.join(specifier)).ok_or_else(|| {
                missing(format!(
                    "{} loads {specifier}, which resolves to no file",
                    short(file)
                ))
            })?;
            self.push(target);
            return Ok(());
        }
        if specifier.starts_with('#') {
            for target in package_imports(file, specifier)? {
                match target {
                    Target::File(path) => self.push(path),
                    Target::Bare(bare) => {
                        if let Err(problem) = self.load(file, directory, &bare) {
                            self.note(problem);
                        }
                    }
                }
            }
            return Ok(());
        }
        let Some((name, subpath)) = split_bare_specifier(specifier) else {
            return Err(missing(format!("{} loads {specifier:?}", short(file))));
        };
        if self.solid.contains(&name) {
            return Err(PackageReach::ReachesSolid {
                via: format!("{} loads {specifier}", short(file)),
            });
        }
        let root = self_reference(file, &name).or_else(|| package_root_of_load(file, &name));
        let Some(root) = root else {
            let builtin =
                NODE_BUILTINS.contains(&specifier) || NODE_BUILTINS.contains(&name.as_str());
            if builtin {
                return Ok(());
            }
            return Err(missing(format!(
                "{} loads {specifier}, which no installed package answers",
                short(file)
            )));
        };
        // The installed package's own name decides, not the specifier (an alias install).
        let installed = manifest(&root)?
            .get("name")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
            .unwrap_or_default();
        if self.solid.contains(&installed) {
            return Err(PackageReach::ReachesSolid {
                via: format!("{} loads {specifier}", short(file)),
            });
        }
        // Only what the load names: the subpath's targets of an `exports` map, or the file a deep path names.
        let root =
            canonical(&root).map_err(|error| missing(format!("{}: {error}", short(&root))))?;
        self.steps.push(RawStep::Enter(root, subpath));
        Ok(())
    }
}

struct Walk<'s, 'd, 'm> {
    solid: &'s SolidPackages<'d>,
    memo: &'m mut Memo,
    /// The files pushed so far: each is visited once.
    files: IdSet,
    queue: VecDeque<FileId>,
    /// The (package, subpath) pairs whose entries were pushed; `*` is every entry.
    entered: IdSet,
    /// The problems in `problems`, so that each is kept once.
    noted: IdSet,
    /// Everything that makes the closure reach Solid or not decided, each once, in walk order.
    problems: Vec<ProblemId>,
    /// The names accessors are defined under in the files read, and whether any is defined under a name the syntax does not fix.
    accessor_names: BTreeSet<String>,
    accessors_any_name: bool,
    /// The globals the files read write (ADR 0973), and whether any is written under a name the syntax does not fix.
    written: BTreeSet<String>,
    written_any: bool,
}

impl Walk<'_, '_, '_> {
    fn run(&mut self, root: &Path, demand: &Demand, extra_entries: &BTreeSet<PathBuf>) {
        self.enter(root, demand);
        for entry in extra_entries {
            let file = self.memo.file(entry.clone());
            self.push(file);
        }
        // The files one step out are scanned side by side and then visited one by one in the order they were found (the queue's own, so the files a visit
        // finds come after the ones already waiting): what the walk answers does not depend on how many threads scan.
        while !self.queue.is_empty() {
            let level = self.queue.drain(..).collect::<Vec<_>>();
            prepare(self.memo, self.solid, &level);
            for file in level {
                if self.files.len() > MAX_CLOSURE_FILES {
                    self.note(PackageReach::Undecided {
                        blocker: ReachBlocker::ClosureLimit,
                        detail: format!("more than {MAX_CLOSURE_FILES} files"),
                    });
                    return;
                }
                self.visit(file);
            }
        }
    }

    fn note(&mut self, problem: PackageReach) {
        let problem = self.memo.problem_id(problem);
        self.note_problem(problem);
    }

    fn note_problem(&mut self, problem: ProblemId) {
        if self.problems.len() < MAX_PROBLEMS && self.noted.insert(problem.0) {
            self.problems.push(problem);
        }
    }

    fn push(&mut self, file: FileId) {
        if self.files.insert(file.0) {
            self.queue.push_back(file);
        }
    }

    /// The entries `demand` asks of the package at `root`, each pair once; what cannot be read is a problem, and the rest of the walk goes on.
    fn enter(&mut self, root: &Path, demand: &Demand) {
        if let Err(problem) = self.try_enter(root, demand) {
            self.note(problem);
        }
    }

    fn try_enter(&mut self, root: &Path, demand: &Demand) -> Result<(), PackageReach> {
        let root = canonical(root).map_err(|error| missing(format!("{}: {error}", short(root))))?;
        if demand.everything {
            let entry = self.memo.entry(root.clone(), "*".to_owned());
            self.enter_entry(entry)
                .map_err(|problem| self.memo.problem(problem).clone())?;
        }
        for subpath in &demand.subpaths {
            let entry = self.memo.entry(root.clone(), subpath.clone());
            self.enter_entry(entry)
                .map_err(|problem| self.memo.problem(problem).clone())?;
        }
        Ok(())
    }

    /// The files of an entry are pushed the first time the walk enters it; `Err` when the package's manifest names them in a way the walk does not read.
    fn enter_entry(&mut self, entry: EntryId) -> Result<(), ProblemId> {
        if !self.entered.insert(entry.0) {
            return Ok(());
        }
        for &file in self.memo.entry_files(entry)?.iter() {
            self.push(file);
        }
        Ok(())
    }

    /// One file: what visiting it does, replayed in order.
    fn visit(&mut self, file: FileId) {
        let visit = self.memo.visit(self.solid, file);
        if let Some(scan) = &visit.scan {
            extend(&mut self.accessor_names, &scan.accessors.names);
            self.accessors_any_name |= scan.accessors.any_name;
            extend(&mut self.written, &scan.global_writes.names);
            self.written_any |= scan.global_writes.any_name;
        }
        for step in &visit.steps {
            match *step {
                Step::Note(problem) => self.note_problem(problem),
                Step::Push(file) => self.push(file),
                Step::Enter(entry) => {
                    if let Err(problem) = self.enter_entry(entry) {
                        self.note_problem(problem);
                    }
                }
            }
        }
    }
}

/// `into.extend(names)`, without a copy of a name `into` holds.
fn extend(into: &mut BTreeSet<String>, names: &[String]) {
    for name in names {
        if !into.contains(name.as_str()) {
            into.insert(name.clone());
        }
    }
}

/// Whether a file name is a stylesheet, an image or JSON: no code a package wrote.
fn is_code_free(name: &str) -> bool {
    CODE_FREE_EXTENSIONS
        .iter()
        .any(|extension| name.ends_with(extension))
}

/// Whether a file name is one the walk scans as code.
fn is_script(name: &str) -> bool {
    SCRIPT_EXTENSIONS
        .iter()
        .any(|extension| name.ends_with(extension))
}

/// The most threads that work ahead of one level of a walk at once, and the fewest bytes of source worth a thread of its own: a level with less is visited by the walk.
const SCAN_THREADS: usize = 6;
const SCAN_AHEAD_THREAD_BYTES: u64 = 128 << 10;
/// A file of more bytes than this is big. Its syntax tree and semantic tables take many times its size, so the threads hold at most [`SCAN_AHEAD_BIG_BUDGET`] bytes
/// of big files open at once (the biggest first; one bigger than that scans alone) and work on the small files meanwhile. A file bigger than [`SCAN_AHEAD_MAX`] is
/// scanned when it is visited.
const SCAN_AHEAD_BIG: u64 = 1 << 20;
const SCAN_AHEAD_BIG_BUDGET: u64 = 24 << 20;
const SCAN_AHEAD_MAX: u64 = 32 << 20;

/// The files (by place in the level) the threads of [`prepare`] have yet to start, and the bytes of big files they hold open.
struct ScanQueue {
    /// Big files, the biggest first, with their sizes.
    big: Vec<(u64, usize)>,
    small: Vec<usize>,
    next_big: usize,
    next_small: usize,
    held: u64,
}

/// Gives back the bytes of a big file when its scan ends, or unwinds.
struct Held<'q> {
    queue: &'q (Mutex<ScanQueue>, Condvar),
    bytes: u64,
}

impl Drop for Held<'_> {
    fn drop(&mut self) {
        if self.bytes > 0 {
            locked(&self.queue.0).held -= self.bytes;
            self.queue.1.notify_all();
        }
    }
}

/// Works out, on a few threads, what visiting each file of `level` that no walk has visited does: the scan of the file and the resolution of its loads, which is
/// where the file system is asked. The memo keeps the results in the order of the level, as the visits of the walk would have, so what the walk finds does not
/// depend on how many threads worked. A file that cannot be read or scanned here is left for its visit, which meets the same problem and notes it.
fn prepare(memo: &mut Memo, solid: &SolidPackages<'_>, level: &[FileId]) {
    let mut big = Vec::new();
    let mut small = Vec::new();
    let mut bytes = 0_u64;
    for (at, file) in level.iter().enumerate() {
        if memo.visits[file.0].is_some() {
            continue;
        }
        let path = memo.path(*file);
        let name = path.to_string_lossy();
        if NATIVE_EXTENSIONS
            .iter()
            .any(|extension| name.ends_with(extension))
            || is_declaration_file(&name)
            || is_code_free(&name)
            || !is_script(&name)
        {
            continue;
        }
        let Ok(size) = fs::metadata(path).map(|metadata| metadata.len()) else {
            continue;
        };
        if size > SCAN_AHEAD_MAX {
            continue;
        }
        bytes += size;
        if size > SCAN_AHEAD_BIG {
            big.push((size, at));
        } else {
            small.push(at);
        }
    }
    let tasks = big.len() + small.len();
    let threads = std::thread::available_parallelism()
        .map_or(1, std::num::NonZero::get)
        .min(SCAN_THREADS)
        .min(tasks)
        .min(usize::try_from(bytes / SCAN_AHEAD_THREAD_BYTES).unwrap_or(usize::MAX));
    if threads < 2 {
        return;
    }
    big.sort_by_key(|(size, _)| std::cmp::Reverse(*size));
    let queue = (
        Mutex::new(ScanQueue {
            big,
            small,
            next_big: 0,
            next_small: 0,
            held: 0,
        }),
        Condvar::new(),
    );
    let results = Mutex::new(
        std::iter::repeat_with(|| None)
            .take(level.len())
            .collect::<Vec<Option<RawVisit>>>(),
    );
    let files = level
        .iter()
        .map(|file| memo.path(*file))
        .collect::<Vec<_>>();
    let reading = snapshot();
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                let _reading = Reading::through(Arc::clone(&reading));
                loop {
                    // A big file when the big files being scanned leave room for it, else a small one; with neither, wait for a big one to end.
                    let task = {
                        let mut state = locked(&queue.0);
                        loop {
                            if let Some(&(size, at)) = state.big.get(state.next_big)
                                && (state.held == 0 || state.held + size <= SCAN_AHEAD_BIG_BUDGET)
                            {
                                state.next_big += 1;
                                state.held += size;
                                break Some((at, size));
                            }
                            if let Some(&at) = state.small.get(state.next_small) {
                                state.next_small += 1;
                                break Some((at, 0));
                            }
                            if state.next_big >= state.big.len() {
                                break None;
                            }
                            state = queue.1.wait(state).unwrap_or_else(PoisonError::into_inner);
                        }
                    };
                    let Some((at, held)) = task else {
                        break;
                    };
                    let file = files[at];
                    {
                        // The syntax tree is gone when the scan ends; what is left is small.
                        let _held = Held {
                            queue: &queue,
                            bytes: held,
                        };
                        let _ = scanned(file);
                    }
                    let raw = record(file, solid);
                    locked(&results)[at] = Some(raw);
                }
            });
        }
    });
    let results = results.into_inner().unwrap_or_else(PoisonError::into_inner);
    for (at, raw) in results.into_iter().enumerate() {
        if let Some(raw) = raw {
            memo.keep(level[at], raw);
        }
    }
}

/// A path as a report names it: from the package directory under its last `node_modules` (`fz-step/index.js`), else its file name.
fn short(path: &Path) -> String {
    let text = path.to_string_lossy();
    match text.rfind("/node_modules/") {
        Some(at) => text[at + "/node_modules/".len()..].to_owned(),
        None => path.file_name().map_or_else(
            || text.clone().into_owned(),
            |name| name.to_string_lossy().into_owned(),
        ),
    }
}

fn missing(detail: String) -> PackageReach {
    PackageReach::Undecided {
        blocker: ReachBlocker::MissingSource,
        detail,
    }
}

const fn blocker_of(kind: ModuleHazardKind) -> ReachBlocker {
    match kind {
        ModuleHazardKind::Unparseable => ReachBlocker::Unparseable,
        ModuleHazardKind::DynamicLoad => ReachBlocker::DynamicLoad,
        ModuleHazardKind::CodeGeneration => ReachBlocker::CodeGeneration,
        ModuleHazardKind::UnknownGlobal => ReachBlocker::UnknownGlobal,
        ModuleHazardKind::GlobalObjectEscape => ReachBlocker::GlobalObjectEscape,
    }
}

/// A file's scan, kept for the process while its length and modification time hold.
fn scanned(file: &Path) -> Result<Arc<ModuleScan>, PackageReach> {
    type Kept = Mutex<HashMap<PathBuf, ((u64, Option<SystemTime>), Arc<ModuleScan>)>>;
    static KEPT: OnceLock<Kept> = OnceLock::new();
    let metadata =
        fs::metadata(file).map_err(|error| missing(format!("{}: {error}", short(file))))?;
    let stamp = (metadata.len(), metadata.modified().ok());
    let kept = KEPT.get_or_init(Mutex::default);
    if let Some((kept_stamp, scan)) = kept
        .lock()
        .map_err(|_| missing("scan cache poisoned".into()))?
        .get(file)
        && *kept_stamp == stamp
    {
        return Ok(Arc::clone(scan));
    }
    let source = fs::read_to_string(file).map_err(|error| PackageReach::Undecided {
        blocker: ReachBlocker::Unparseable,
        detail: format!("{}: {error}", short(file)),
    })?;
    let scan = Arc::new(scan_module(file, &source));
    kept.lock()
        .map_err(|_| missing("scan cache poisoned".into()))?
        .insert(file.to_path_buf(), (stamp, Arc::clone(&scan)));
    Ok(scan)
}

/// A path as a bundler resolves it: the file, a `.ts` source for a `.js` specifier (TypeScript's ESM convention), the file with an extension, a
/// directory's manifest `main` or its `index` file. Canonical.
fn resolve_path(base: &Path) -> Option<PathBuf> {
    resolve_path_following(base, true)
}

/// The TypeScript source extensions a JavaScript specifier may name: `./a.js` is `a.ts` or `a.tsx`, `./a.jsx` is `a.tsx`, `./a.mjs` is `a.mts`.
fn source_spellings(path: &str) -> Vec<String> {
    let table: &[(&str, &[&str])] = &[
        (".js", &[".ts", ".tsx"]),
        (".jsx", &[".tsx"]),
        (".mjs", &[".mts"]),
        (".cjs", &[".cts"]),
    ];
    let mut spellings = Vec::new();
    for (from, to) in table {
        if let Some(stem) = path.strip_suffix(from) {
            spellings.extend(to.iter().map(|extension| format!("{stem}{extension}")));
        }
    }
    spellings
}

/// [`resolve_path`]; `follow_main` is whether a directory's `main` is read, which only the first directory of a resolution does (a `main` naming a
/// directory is resolved by its `index` file, and a `main` of `""`, `"."` or the directory itself would otherwise never end).
fn resolve_path_following(base: &Path, follow_main: bool) -> Option<PathBuf> {
    let snapshot = snapshot();
    let key = (base.to_path_buf(), follow_main);
    let held = locked(&snapshot.resolved).get(&key).cloned();
    if let Some(held) = held {
        return held;
    }
    let found = resolve_path_unheld(base, follow_main);
    locked(&snapshot.resolved).insert(key, found.clone());
    found
}

fn resolve_path_unheld(base: &Path, follow_main: bool) -> Option<PathBuf> {
    if base.is_file() {
        return canonical(base).ok();
    }
    let text = base.to_string_lossy();
    for spelling in source_spellings(&text) {
        let candidate = PathBuf::from(spelling);
        if candidate.is_file() {
            return canonical(&candidate).ok();
        }
    }
    for extension in RESOLVE_EXTENSIONS {
        let candidate = PathBuf::from(format!("{text}{extension}"));
        if candidate.is_file() {
            return canonical(&candidate).ok();
        }
    }
    if base.is_dir() {
        if follow_main
            && let Ok(manifest) = manifest(base)
            && let Some(main) = manifest
                .get("main")
                .and_then(serde_json::Value::as_str)
                .filter(|main| !main.is_empty())
            && let Some(target) = resolve_path_following(&base.join(main), false)
        {
            return Some(target);
        }
        for extension in RESOLVE_EXTENSIONS {
            let candidate = base.join(format!("index{extension}"));
            if candidate.is_file() {
                return canonical(&candidate).ok();
            }
        }
    }
    None
}

enum Target {
    File(PathBuf),
    Bare(String),
}

/// Every runtime file a package can be entered at: the targets of `exports` under every condition (patterns expanded over the scripts they match),
/// `main`, `module`, an object `browser` field's files, else `index.js`. A target that is not installed loads nothing and is skipped.
fn all_entries(root: &Path) -> Result<Vec<PathBuf>, PackageReach> {
    let manifest = manifest(root)?;
    let mut targets = Vec::<String>::new();
    if let Some(exports) = manifest.get("exports") {
        collect_targets(exports, &mut targets, 0).map_err(|detail| PackageReach::Undecided {
            blocker: ReachBlocker::ManifestUnread,
            detail: format!("{}: {detail}", short(root)),
        })?;
    }
    legacy_fields(&manifest, &mut targets);
    Ok(resolve_targets(root, targets))
}

/// The files `main`, `module` and `browser` name, for a resolver that does not read `exports` (and the only entry of a package with none).
fn legacy_fields(manifest: &serde_json::Value, targets: &mut Vec<String>) {
    for field in ["main", "module"] {
        if let Some(target) = manifest.get(field).and_then(serde_json::Value::as_str) {
            targets.push(target.to_owned());
        }
    }
    match manifest.get("browser") {
        Some(serde_json::Value::String(target)) => targets.push(target.clone()),
        Some(serde_json::Value::Object(map)) => {
            for value in map.values() {
                if let serde_json::Value::String(target) = value {
                    targets.push(target.clone());
                }
            }
        }
        _ => {}
    }
}

/// The files of `targets` (paths relative to the package, `*` expanded over the scripts it matches): `index.js` when none names anything.
fn resolve_targets(root: &Path, mut targets: Vec<String>) -> Vec<PathBuf> {
    // `"main": ""` (csstype, @webgpu/types) names nothing: Node falls back to `index.js`.
    targets.retain(|target| !target.is_empty());
    if targets.is_empty() {
        targets.push("./index.js".to_owned());
    }
    let mut entries = BTreeSet::new();
    for target in targets {
        let relative = target.trim_start_matches("./");
        if target.contains('*') {
            entries.extend(expand_pattern(root, relative));
            continue;
        }
        if target.contains(':') || target.starts_with('/') {
            continue;
        }
        if let Some(path) = resolve_path(&root.join(relative)) {
            entries.insert(path);
        }
    }
    entries.into_iter().collect()
}

/// The entries one subpath of the package at `root` selects (`.`, `./sub`): what an `exports` map names for it under every condition (an exact key,
/// else a `*` pattern with its match substituted), or without `exports` the legacy fields for `.` and the file a deep path names. A subpath the map
/// does not name resolves to nothing at run time; here it keeps every entry, since a resolver this walk does not model may read it another way.
fn subpath_entries(root: &Path, subpath: &str) -> Result<Vec<PathBuf>, PackageReach> {
    let manifest = manifest(root)?;
    let unread = |detail: String| PackageReach::Undecided {
        blocker: ReachBlocker::ManifestUnread,
        detail: format!("{}: {detail}", short(root)),
    };
    let mut targets = Vec::<String>::new();
    match manifest.get("exports") {
        Some(exports) => {
            let mapped = exports_for_subpath(exports, subpath, &mut targets).map_err(unread)?;
            if !mapped {
                return all_entries(root);
            }
            if subpath == "." {
                legacy_fields(&manifest, &mut targets);
            }
        }
        None if subpath == "." => legacy_fields(&manifest, &mut targets),
        None => targets.push(subpath.to_owned()),
    }
    Ok(resolve_targets(root, targets))
}

/// Pushes the targets an `exports` value maps `subpath` to; whether it maps it at all. A value with no subpath keys is the `.` entry's alone.
fn exports_for_subpath(
    exports: &serde_json::Value,
    subpath: &str,
    targets: &mut Vec<String>,
) -> Result<bool, String> {
    let serde_json::Value::Object(map) = exports else {
        return match exports {
            serde_json::Value::String(_) | serde_json::Value::Array(_) if subpath == "." => {
                collect_targets(exports, targets, 0)?;
                Ok(true)
            }
            _ => Ok(false),
        };
    };
    if !map.keys().any(|key| key.starts_with('.')) {
        // Conditions only: the `.` entry.
        if subpath != "." {
            return Ok(false);
        }
        collect_targets(exports, targets, 0)?;
        return Ok(true);
    }
    if let Some(value) = map.get(subpath) {
        collect_targets(value, targets, 0)?;
        return Ok(true);
    }
    let mut mapped = false;
    for (key, value) in map {
        let Some((prefix, suffix)) = key.split_once('*') else {
            continue;
        };
        if subpath.len() < key.len() - 1
            || !subpath.starts_with(prefix)
            || !subpath.ends_with(suffix)
        {
            continue;
        }
        let star = &subpath[prefix.len()..subpath.len() - suffix.len()];
        let mut found = Vec::new();
        collect_targets(value, &mut found, 0)?;
        targets.extend(found.into_iter().map(|target| target.replace('*', star)));
        mapped = true;
    }
    Ok(mapped)
}

/// Every string target of an `exports` value: every condition's, every array element's.
fn collect_targets(
    value: &serde_json::Value,
    targets: &mut Vec<String>,
    depth: usize,
) -> Result<(), String> {
    if depth > 32 {
        return Err("`exports` nests too deep".into());
    }
    match value {
        serde_json::Value::String(target) => targets.push(target.clone()),
        serde_json::Value::Object(map) => {
            for value in map.values() {
                collect_targets(value, targets, depth + 1)?;
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                collect_targets(value, targets, depth + 1)?;
            }
        }
        serde_json::Value::Null | serde_json::Value::Bool(false) => {}
        other => return Err(format!("`exports` holds {other}")),
    }
    Ok(())
}

/// The files a target pattern (`dist/*.js`, `*` matching any path) matches under `root`.
fn expand_pattern(root: &Path, pattern: &str) -> Vec<PathBuf> {
    let Some((prefix, suffix)) = pattern.split_once('*') else {
        return Vec::new();
    };
    let suffix = suffix.replace('*', "");
    let base = match prefix.rfind('/') {
        Some(slash) => root.join(&prefix[..slash]),
        None => root.to_path_buf(),
    };
    let mut found = Vec::new();
    let mut stack = vec![base];
    while let Some(directory) = stack.pop() {
        let Ok(entries) = fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if path.file_name().is_some_and(|name| name != "node_modules") {
                    stack.push(path);
                }
                continue;
            }
            let Ok(relative) = path.strip_prefix(root) else {
                continue;
            };
            let relative = relative.to_string_lossy();
            // A pattern such as `./*` matches every file of the package: the scripts are entries, a licence or a stylesheet is data.
            if relative.starts_with(prefix)
                && relative.ends_with(&suffix)
                && SCRIPT_EXTENSIONS
                    .iter()
                    .any(|extension| relative.ends_with(extension))
                && !is_declaration_file(&relative)
                && let Ok(path) = canonical(&path)
            {
                found.push(path);
            }
        }
    }
    found
}

/// The targets of a `#` specifier in the `imports` field of the nearest manifest above `file`: every condition's.
fn package_imports(file: &Path, specifier: &str) -> Result<Vec<Target>, PackageReach> {
    let unread = |detail: String| PackageReach::Undecided {
        blocker: ReachBlocker::ManifestUnread,
        detail,
    };
    let mut directory = file.parent();
    while let Some(current) = directory {
        if current.join("package.json").is_file() {
            let manifest = manifest(current)?;
            let imports = manifest
                .get("imports")
                .and_then(serde_json::Value::as_object)
                .ok_or_else(|| {
                    unread(format!(
                        "{} loads {specifier}, and {} has no `imports`",
                        short(file),
                        short(current)
                    ))
                })?;
            let mut raw = Vec::new();
            match imports.get(specifier) {
                Some(value) => collect_targets(value, &mut raw, 0).map_err(unread)?,
                None => {
                    // A pattern key: every key's targets whose prefix and suffix surround the specifier.
                    let mut matched = false;
                    for (key, value) in imports {
                        if let Some((prefix, suffix)) = key.split_once('*')
                            && specifier.starts_with(prefix)
                            && specifier.ends_with(suffix)
                        {
                            matched = true;
                            let star = &specifier[prefix.len()..specifier.len() - suffix.len()];
                            let mut targets = Vec::new();
                            collect_targets(value, &mut targets, 0).map_err(unread)?;
                            raw.extend(targets.into_iter().map(|target| target.replace('*', star)));
                        }
                    }
                    if !matched {
                        return Err(unread(format!(
                            "{} loads {specifier}, which `imports` does not map",
                            short(file)
                        )));
                    }
                }
            }
            let mut targets = Vec::new();
            for target in raw {
                if target.starts_with("./") {
                    let path = resolve_path(&current.join(target.trim_start_matches("./")))
                        .ok_or_else(|| {
                            missing(format!(
                                "{specifier} maps to {target}, which is not installed"
                            ))
                        })?;
                    targets.push(Target::File(path));
                } else {
                    targets.push(Target::Bare(target));
                }
            }
            return Ok(targets);
        }
        directory = current.parent();
    }
    Err(unread(format!(
        "{} loads {specifier} outside any package",
        short(file)
    )))
}

/// Node's package self-reference: the nearest manifest above `file` is named `name` and has `exports`.
fn self_reference(file: &Path, name: &str) -> Option<PathBuf> {
    let current = nearest_manifest_directory(file.parent()?)?;
    let manifest = manifest(&current).ok()?;
    (manifest.get("name").and_then(serde_json::Value::as_str) == Some(name)
        && manifest.get("exports").is_some())
    .then_some(current)
}

/// The verdicts by root, for a report: how many of each.
#[must_use]
pub fn summary(index: &PackageReachIndex) -> BTreeMap<&'static str, usize> {
    let mut counts = BTreeMap::new();
    for reach in index.by_root.values() {
        *counts.entry(reach.word()).or_default() += 1;
    }
    counts
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOLID: SolidPackages<'static> = SolidPackages {
        names: &["solid-js", "@solidjs/signals", "@solidjs/web"],
        scopes: &["@solidjs/"],
    };

    fn write(root: &Path, path: &str, text: &str) {
        let path = root.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    struct Project(PathBuf);

    impl Project {
        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for Project {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn project() -> Project {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let directory = std::env::temp_dir().join(format!(
            "solid-checker-package-reach-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&directory);
        write(
            &directory,
            "node_modules/solid-js/package.json",
            r#"{"name":"solid-js","version":"2.0.0"}"#,
        );
        write(
            &directory,
            "node_modules/solid-js/index.js",
            "export const action = (f) => f;",
        );
        Project(directory)
    }

    fn reach(root: &Path, name: &str) -> PackageReach {
        reach_of(root, name, &everything())
    }

    fn closure(root: &Path, name: &str) -> Closure {
        reach_closure(root, name, &everything())
    }

    /// One computation: the lookups and the walks' memo start empty, as in [`package_reach_index`].
    fn reach_closure(root: &Path, name: &str, demand: &Demand) -> Closure {
        let _reading = Reading::fresh();
        package_reach(
            &root.join("node_modules").join(name).canonicalize().unwrap(),
            &SOLID,
            demand,
            &BTreeSet::new(),
            &mut Memo::default(),
        )
    }

    fn everything() -> Demand {
        Demand {
            everything: true,
            ..Demand::default()
        }
    }

    fn reach_of(root: &Path, name: &str, demand: &Demand) -> PackageReach {
        reach_closure(root, name, demand).reach
    }

    #[test]
    fn a_project_declaration_with_unread_javascript_beside_it_is_undecided() {
        let directory = project();
        let base = directory.path().canonicalize().unwrap();
        write(
            &base,
            "vendor/step.d.ts",
            "export declare const step: () => void;",
        );
        write(&base, "vendor/step.js", "export const step = () => {};");
        write(&base, "types/only.d.ts", "export type T = number;");
        write(
            &base,
            "read/read.d.ts",
            "export declare const read: number;",
        );
        write(&base, "read/read.js", "export const read = 1;");
        let path = |relative: &str| base.join(relative).to_str().unwrap().to_owned();
        let declarations = ["vendor/step.d.ts", "types/only.d.ts", "read/read.d.ts"]
            .iter()
            .map(|relative| path(relative))
            .collect::<BTreeSet<_>>();
        let analyzed_js = path("read/read.js");
        let analyzed = BTreeSet::from([analyzed_js.as_str()]);
        // The declaration with a sibling nobody reads is one; a declaration with no runtime file, and one whose sibling the program analyzes, are not.
        assert_eq!(
            declarations_with_unread_js(&declarations, &analyzed),
            BTreeSet::from([path("vendor/step.d.ts")])
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_link_to_a_workspace_package_maps_its_real_directory_to_the_link() {
        let directory = project();
        let base = directory.path().canonicalize().unwrap();
        write(
            &base,
            "packages/kernel/package.json",
            r#"{"name":"@ws/kernel"}"#,
        );
        write(&base, "packages/kernel/dist/index.d.ts", "export {};");
        fs::create_dir_all(base.join("app/node_modules/@ws")).unwrap();
        std::os::unix::fs::symlink(
            base.join("packages/kernel"),
            base.join("app/node_modules/@ws/kernel"),
        )
        .unwrap();
        let link_root = base.join("app/node_modules/@ws/kernel");
        let real = base.join("packages/kernel");
        let linked = linked_root(
            link_root.join("dist/index.d.ts").to_str().unwrap(),
            real.join("dist/index.d.ts").to_str().unwrap(),
        );
        assert_eq!(
            linked,
            Some((
                real.to_str().unwrap().to_owned(),
                link_root.to_str().unwrap().to_owned()
            ))
        );
        // The compiler's own path must lie in the real directory: another file is no evidence of this link.
        assert_eq!(
            linked_root(
                link_root.join("dist/index.d.ts").to_str().unwrap(),
                base.join("elsewhere/index.d.ts").to_str().unwrap()
            ),
            None
        );
        // A package that is no link, a path that names no package and an empty attestation map nothing.
        write(
            &base,
            "app/node_modules/plain/package.json",
            r#"{"name":"plain"}"#,
        );
        let plain = base.join("app/node_modules/plain/index.d.ts");
        assert_eq!(
            linked_root(plain.to_str().unwrap(), plain.to_str().unwrap()),
            None
        );
        assert_eq!(linked_root("/p/src/a.ts", "/p/src/a.ts"), None);
        assert_eq!(linked_root("", plain.to_str().unwrap()), None);
    }

    #[test]
    fn a_package_that_imports_solid_reaches_it_and_a_pure_one_is_clear() {
        let directory = project();
        let root = directory.path();
        write(
            root,
            "node_modules/step/package.json",
            r#"{"name":"step","exports":{".":{"types":"./i.d.ts","default":"./i.js"}}}"#,
        );
        write(
            root,
            "node_modules/step/i.js",
            "import { helper } from './h.js'; export const run = helper;",
        );
        write(
            root,
            "node_modules/step/h.js",
            "import { action } from 'solid-js'; export const helper = action;",
        );
        write(
            root,
            "node_modules/pure/package.json",
            r#"{"name":"pure","main":"./i.cjs"}"#,
        );
        write(
            root,
            "node_modules/pure/i.cjs",
            "const dep = require('dep'); module.exports = (x) => dep(x) + Math.max(1, 2);",
        );
        write(root, "node_modules/dep/package.json", r#"{"name":"dep"}"#);
        write(
            root,
            "node_modules/dep/index.js",
            "module.exports = (x) => x;",
        );
        assert!(
            matches!(reach(root, "step"), PackageReach::ReachesSolid { .. }),
            "{:?}",
            reach(root, "step")
        );
        assert_eq!(reach(root, "pure"), PackageReach::Clear);
    }

    #[test]
    fn a_dependency_that_reaches_solid_through_another_package_is_found() {
        let directory = project();
        let root = directory.path();
        write(
            root,
            "node_modules/outer/package.json",
            r#"{"name":"outer","module":"./o.mjs"}"#,
        );
        write(
            root,
            "node_modules/outer/o.mjs",
            "export { inner } from 'inner/sub';",
        );
        write(
            root,
            "node_modules/outer2/package.json",
            r#"{"name":"outer2","module":"./o.mjs"}"#,
        );
        write(
            root,
            "node_modules/outer2/o.mjs",
            "export { action } from 'inner/other';",
        );
        write(
            root,
            "node_modules/inner/package.json",
            r#"{"name":"inner","exports":{"./sub":"./sub.js","./other":"./other.js"}}"#,
        );
        write(root, "node_modules/inner/sub.js", "export const inner = 1;");
        write(
            root,
            "node_modules/inner/other.js",
            "export { action } from '@solidjs/signals';",
        );
        // A package is entered at the subpath a load names: `outer` loads `inner/sub`, which loads nothing; `outer2` loads `inner/other`.
        assert_eq!(reach(root, "outer"), PackageReach::Clear);
        assert!(matches!(
            reach(root, "outer2"),
            PackageReach::ReachesSolid { .. }
        ));
    }

    #[test]
    fn undecidable_closures_are_undecided() {
        let directory = project();
        let root = directory.path();
        for (name, source, blocker) in [
            (
                "dyn",
                "export const load = (n) => import(n);",
                ReachBlocker::DynamicLoad,
            ),
            (
                "gen",
                "export const run = (s) => new Function(s)();",
                ReachBlocker::CodeGeneration,
            ),
            (
                "env",
                "export const dev = process.env.NODE_ENV;",
                ReachBlocker::UnknownGlobal,
            ),
            (
                "lost",
                "export * from 'not-installed';",
                ReachBlocker::MissingSource,
            ),
            ("broken", "export const = ;", ReachBlocker::Unparseable),
        ] {
            write(
                root,
                &format!("node_modules/{name}/package.json"),
                &format!(r#"{{"name":"{name}"}}"#),
            );
            write(root, &format!("node_modules/{name}/index.js"), source);
            match reach(root, name) {
                PackageReach::Undecided { blocker: found, .. } => {
                    assert_eq!(found, blocker, "{name}")
                }
                other => panic!("{name}: {other:?}"),
            }
        }
        write(
            root,
            "node_modules/native/package.json",
            r#"{"name":"native","main":"./binding.node"}"#,
        );
        write(root, "node_modules/native/binding.node", "");
        assert!(matches!(
            reach(root, "native"),
            PackageReach::Undecided {
                blocker: ReachBlocker::NativeModule,
                ..
            }
        ));
    }

    #[test]
    fn a_node_builtin_loads_nothing_and_types_name_their_runtime_package() {
        let directory = project();
        let root = directory.path();
        write(root, "node_modules/b/package.json", r#"{"name":"b"}"#);
        write(
            root,
            "node_modules/b/index.js",
            "import { join } from 'path'; import fs from 'node:fs'; export const j = join;",
        );
        assert_eq!(reach(root, "b"), PackageReach::Clear);
        write(
            root,
            "node_modules/@types/b/package.json",
            r#"{"name":"@types/b"}"#,
        );
        let types = root.join("node_modules/@types/b").canonicalize().unwrap();
        assert_eq!(
            runtime_root(&types).unwrap(),
            root.join("node_modules/b").canonicalize().unwrap()
        );
        write(
            root,
            "node_modules/@types/gone/package.json",
            r#"{"name":"@types/gone"}"#,
        );
        let gone = root
            .join("node_modules/@types/gone")
            .canonicalize()
            .unwrap();
        assert!(runtime_root(&gone).is_err());
    }

    /// csstype and @webgpu/types ship `"main": ""`: resolving it followed the directory's own `main` back to the directory and overflowed the stack.
    #[test]
    fn a_main_that_names_nothing_ends() {
        let directory = project();
        let root = directory.path();
        for (name, main) in [
            ("empty", ""),
            ("dot", "."),
            ("slash", "./"),
            ("self", "../self"),
        ] {
            write(
                root,
                &format!("node_modules/{name}/package.json"),
                &format!(r#"{{"name":"{name}","main":"{main}"}}"#),
            );
            write(
                root,
                &format!("node_modules/{name}/index.js"),
                "export const x = 1;",
            );
            assert_eq!(reach(root, name), PackageReach::Clear, "{name}");
        }
        // A manifest naming no `main` and no `index.js` has no runtime file: nothing to reach Solid.
        write(
            root,
            "node_modules/typesonly/package.json",
            r#"{"name":"typesonly","main":"","types":"index.d.ts"}"#,
        );
        write(
            root,
            "node_modules/typesonly/index.d.ts",
            "export declare const x: number;",
        );
        assert_eq!(reach(root, "typesonly"), PackageReach::Clear);
    }

    /// A package with no `exports` map can be imported by a deep path no entry reaches: the runtime file beside the declaration the project names is an
    /// entry, so a deep file that loads Solid is found.
    #[test]
    fn a_deep_file_the_project_names_is_an_entry() {
        let directory = project();
        let root = directory.path();
        write(
            root,
            "node_modules/deep/package.json",
            r#"{"name":"deep","main":"./index.js"}"#,
        );
        write(root, "node_modules/deep/index.js", "export const a = 1;");
        write(
            root,
            "node_modules/deep/lib/deep.js",
            "import { action } from 'solid-js'; export const run = action;",
        );
        write(
            root,
            "node_modules/deep/lib/deep.d.ts",
            "export declare const run: () => void;",
        );
        assert_eq!(
            reach(root, "deep"),
            PackageReach::Clear,
            "the entries alone do not reach the deep file"
        );
        let declaration = root
            .join("node_modules/deep/lib/deep.d.ts")
            .to_string_lossy()
            .into_owned();
        let siblings = runtime_siblings(&declaration)
            .into_iter()
            .collect::<BTreeSet<_>>();
        assert_eq!(siblings.len(), 1);
        let package = root.join("node_modules/deep").canonicalize().unwrap();
        let _reading = Reading::fresh();
        let found = package_reach(
            &package,
            &SOLID,
            &everything(),
            &siblings,
            &mut Memo::default(),
        )
        .reach;
        assert!(
            matches!(found, PackageReach::ReachesSolid { .. }),
            "{found:?}"
        );
    }

    /// A walk starts from the subpaths the program loads: a subpath nothing imports is not walked, so a Solid load behind it is not reached.
    #[test]
    fn only_the_imported_subpaths_are_entries() {
        let directory = project();
        let root = directory.path();
        write(
            root,
            "node_modules/sub/package.json",
            r#"{"name":"sub","exports":{".":"./a.js","./solid":{"default":"./b.js"},"./lib/*":"./lib/*.js"}}"#,
        );
        write(root, "node_modules/sub/a.js", "export const a = 1;");
        write(
            root,
            "node_modules/sub/b.js",
            "import { action } from 'solid-js'; export const b = action;",
        );
        write(root, "node_modules/sub/lib/x.js", "export const x = 1;");
        write(root, "node_modules/sub/lib/y.js", "import 'solid-js';");
        assert_eq!(
            reach_of(root, "sub", &Demand::subpath(".")),
            PackageReach::Clear
        );
        assert_eq!(
            reach_of(root, "sub", &Demand::subpath("./lib/x")),
            PackageReach::Clear
        );
        assert!(matches!(
            reach_of(root, "sub", &Demand::subpath("./solid")),
            PackageReach::ReachesSolid { .. }
        ));
        assert!(matches!(
            reach_of(root, "sub", &Demand::subpath("./lib/y")),
            PackageReach::ReachesSolid { .. }
        ));
        assert!(matches!(
            reach_of(root, "sub", &everything()),
            PackageReach::ReachesSolid { .. }
        ));
        // A subpath the map does not name keeps every entry.
        assert!(matches!(
            reach_of(root, "sub", &Demand::subpath("./nowhere")),
            PackageReach::ReachesSolid { .. }
        ));
    }

    /// The walk reads the whole closure (the review of #231, S6): the first problem is the verdict, and a later file's load of Solid is known whatever an earlier one
    /// held.
    #[test]
    fn the_whole_closure_is_read_past_a_hazard() {
        let directory = project();
        let root = directory.path();
        write(
            root,
            "node_modules/late/package.json",
            r#"{"name":"late","main":"a.js"}"#,
        );
        write(
            root,
            "node_modules/late/a.js",
            "import './b.js'; export const e = process.env.X;",
        );
        write(
            root,
            "node_modules/late/b.js",
            "import { action } from 'solid-js'; export const b = action;",
        );
        let found = closure(root, "late");
        assert!(
            matches!(
                found.reach,
                PackageReach::Undecided {
                    blocker: ReachBlocker::UnknownGlobal,
                    ..
                }
            ),
            "{:?}",
            found.reach
        );
        assert_eq!(found.problems.len(), 2, "{:?}", found.problems);
        assert!(matches!(
            found.problems[1],
            PackageReach::ReachesSolid { .. }
        ));
        // The same files with the Solid load first: Solid is the verdict.
        write(
            root,
            "node_modules/early/package.json",
            r#"{"name":"early","main":"a.js"}"#,
        );
        write(
            root,
            "node_modules/early/a.js",
            "import { action } from 'solid-js'; import './b.js'; export const a = action;",
        );
        write(
            root,
            "node_modules/early/b.js",
            "export const e = process.env.X;",
        );
        assert!(matches!(
            closure(root, "early").reach,
            PackageReach::ReachesSolid { .. }
        ));
        assert_eq!(closure(root, "early").problems.len(), 2);
    }

    /// What a closure answers, whole.
    fn answer(closure: &Closure) -> String {
        format!(
            "{:?} {:?} {:?} {:?}",
            closure.reach, closure.problems, closure.accessors, closure.global_writes
        )
    }

    /// The closures of `names` in one computation: one memo, so a file a closure shares with an earlier one is replayed, not read again.
    fn closures_in_one_computation(root: &Path, names: &[&str]) -> Vec<Closure> {
        let _reading = Reading::fresh();
        let mut memo = Memo::default();
        names
            .iter()
            .map(|name| {
                package_reach(
                    &root.join("node_modules").join(name).canonicalize().unwrap(),
                    &SOLID,
                    &everything(),
                    &BTreeSet::new(),
                    &mut memo,
                )
            })
            .collect()
    }

    /// The walks of one computation share what they read (each file is visited once, its loads resolved once, a package's entries read once), and every closure
    /// answers as it would have alone: the verdict, every problem in walk order, the accessors and the global writes.
    #[test]
    fn closures_that_share_files_answer_as_each_would_alone() {
        let directory = project();
        let root = directory.path();
        let package = |name: &str, main: &str, source: &str| {
            write(
                root,
                &format!("node_modules/{name}/package.json"),
                &format!(r#"{{"name":"{name}","main":"{main}"}}"#),
            );
            write(root, &format!("node_modules/{name}/{main}"), source);
        };
        // `shared` holds a hazard, an accessor and a global write, and loads a file only it holds.
        package(
            "shared",
            "index.js",
            "import './inner.js'; export const o = { get x() { return 1; } }; window.__shared = 1; export const p = process.env.X;",
        );
        write(
            root,
            "node_modules/shared/inner.js",
            "export const i = eval('1');",
        );
        package("a", "index.js", "import 'shared'; export const a = 1;");
        package(
            "b",
            "index.js",
            "import 'shared'; import { action } from 'solid-js'; export const b = action;",
        );
        package(
            "c",
            "index.js",
            "import 'b'; import 'a'; export const c = 1;",
        );
        package(
            "d",
            "index.js",
            "import 'missing-package'; export const d = 1;",
        );
        package(
            "e",
            "index.js",
            "import 'c'; import 'd'; export const e = 1;",
        );
        let names = ["e", "d", "c", "b", "a", "shared"];
        let together = closures_in_one_computation(root, &names);
        for (name, together) in names.iter().zip(&together) {
            let alone = closures_in_one_computation(root, &[name]);
            assert_eq!(answer(together), answer(&alone[0]), "{name}");
        }
        // The shared dependency's problems reach every package that loads it, in the order the walk meets them: `b` loads Solid first (its own file's
        // loads are in order), then `shared` and the file `shared` loads.
        assert_eq!(together[3].problems.len(), 3, "{:?}", together[3].problems);
        assert!(matches!(
            together[3].problems[0],
            PackageReach::ReachesSolid { .. }
        ));
        assert_eq!(together[4].problems.len(), 2, "{:?}", together[4].problems);
        assert!(together[4].accessors.names.contains("x"));
        assert!(
            together[4]
                .global_writes
                .names
                .contains(&"__shared".to_owned())
        );
    }

    /// Many files one step out are scanned, and their loads resolved, side by side; they are still visited in the order the walk found them, so the problems keep
    /// that order (the files are padded to be enough bytes for threads to start).
    #[test]
    fn a_level_of_many_files_is_walked_in_the_order_it_was_found() {
        let directory = project();
        let root = directory.path();
        write(
            root,
            "node_modules/wide/package.json",
            r#"{"name":"wide","main":"index.js"}"#,
        );
        let padding = format!("// {}\n", "x".repeat(8192));
        let mut index = String::new();
        for number in 0..40 {
            index.push_str(&format!("import './f{number}.js';\n"));
            let source = match number {
                7 => "export const x = eval('1');",
                30 => "export const x = process.env.X;",
                _ => "export const x = 1;",
            };
            write(
                root,
                &format!("node_modules/wide/f{number}.js"),
                &format!("{padding}import './sub/g{number}.js';\n{source}"),
            );
            let source = match number {
                12 => "export const g = new Function('return 1');",
                _ => "export const g = 1;",
            };
            write(
                root,
                &format!("node_modules/wide/sub/g{number}.js"),
                &format!("{padding}{source}"),
            );
        }
        write(root, "node_modules/wide/index.js", &index);
        let found = closure(root, "wide");
        // A load list is sorted, so the files are visited as strings: `f30.js` before `f7.js`; the files they load come after the whole level, in the order found.
        let files = found
            .problems
            .iter()
            .map(|problem| match problem {
                PackageReach::Undecided { detail, .. } => {
                    detail.split(':').next().unwrap_or_default().to_owned()
                }
                other => format!("{other:?}"),
            })
            .collect::<Vec<_>>();
        assert_eq!(
            files,
            ["wide/f30.js", "wide/f7.js", "wide/sub/g12.js"],
            "{:?}",
            found.problems
        );
        assert!(matches!(
            found.reach,
            PackageReach::Undecided {
                blocker: ReachBlocker::UnknownGlobal,
                ..
            }
        ));
    }

    /// ADR 0973: where a package's closure may define an accessor, read from the whole closure; a closure not read whole answers for every name.
    #[test]
    fn a_getter_a_package_declares_is_known_by_its_name_where_the_whole_closure_is_read() {
        let directory = project();
        let root = directory.path();
        let getter = "import { action } from 'solid-js'; export const clock = { get now() { void action(function* () {})(); return 1; }, later: 2 };";
        write(
            root,
            "node_modules/fz-get/package.json",
            r#"{"name":"fz-get","main":"index.js"}"#,
        );
        write(root, "node_modules/fz-get/index.js", getter);
        let found = closure(root, "fz-get");
        assert!(matches!(found.reach, PackageReach::ReachesSolid { .. }));
        assert!(found.accessors.closed);
        assert!(found.accessors.may_run(&[Some("now")]));
        // An accessor under another name stays silent: the twin.
        assert!(!found.accessors.may_run(&[Some("later")]));
        // A computed key, a Proxy and `Object.defineProperty` with a computed name match every name.
        for (name, source) in [
            (
                "computed",
                "export const o = { get [key]() { return 1; } }; const key = 'x';",
            ),
            (
                "proxy",
                "export const o = new Proxy({}, { get() { return 1; } });",
            ),
            (
                "define",
                "const key = 'x'; Object.defineProperty(exports, key, { get() { return 1; } });",
            ),
        ] {
            write(
                root,
                &format!("node_modules/fz-{name}/package.json"),
                &format!(r#"{{"name":"fz-{name}","main":"index.js"}}"#),
            );
            write(
                root,
                &format!("node_modules/fz-{name}/index.js"),
                &format!("import {{ action }} from 'solid-js'; export const a = action; {source}"),
            );
            let found = closure(root, &format!("fz-{name}"));
            assert!(found.accessors.closed, "{name}");
            assert!(found.accessors.may_run(&[Some("later")]), "{name}");
        }
        // A value from outside the closure, code nobody read, a computed load, generated code: not closed, every name.
        for (name, source) in [
            ("global", "export const e = process.env.X;"),
            ("missing", "import './gone.js';"),
            ("dynamic", "export const l = (n) => import(n);"),
            ("eval", "export const v = (c) => eval(c);"),
        ] {
            write(
                root,
                &format!("node_modules/fz-{name}/package.json"),
                &format!(r#"{{"name":"fz-{name}","main":"index.js"}}"#),
            );
            write(
                root,
                &format!("node_modules/fz-{name}/index.js"),
                &format!("import {{ action }} from 'solid-js'; export const a = action; {source}"),
            );
            let found = closure(root, &format!("fz-{name}"));
            assert!(!found.accessors.closed, "{name}");
            assert!(found.accessors.may_run(&[Some("later")]), "{name}");
        }
        // Another file of the closure defines it: the whole closure counts.
        write(
            root,
            "node_modules/fz-deep/package.json",
            r#"{"name":"fz-deep","main":"index.js"}"#,
        );
        write(
            root,
            "node_modules/fz-deep/index.js",
            "import { action } from 'solid-js'; import './sub.js'; export const a = action;",
        );
        write(
            root,
            "node_modules/fz-deep/sub.js",
            "export class K { static get shared() { return 1; } }",
        );
        let deep = closure(root, "fz-deep");
        assert!(deep.accessors.closed);
        assert!(deep.accessors.may_run(&[Some("shared")]));
        assert!(!deep.accessors.may_run(&[Some("other")]));
    }

    /// A `.js` specifier names a `.ts` source, and a `./*` pattern matches scripts only (a licence is data).
    #[test]
    fn typescript_sources_and_data_files_are_read_as_a_bundler_reads_them() {
        let directory = project();
        let root = directory.path();
        write(
            root,
            "node_modules/ts/package.json",
            r#"{"name":"ts","main":"index.ts"}"#,
        );
        write(
            root,
            "node_modules/ts/index.ts",
            "import { b } from './b.js'; export const a = b;",
        );
        write(
            root,
            "node_modules/ts/b.ts",
            "import { action } from 'solid-js'; export const b = action;",
        );
        assert!(
            matches!(reach(root, "ts"), PackageReach::ReachesSolid { .. }),
            "{:?}",
            reach(root, "ts")
        );
        write(
            root,
            "node_modules/data/package.json",
            r#"{"name":"data","exports":{"./*":"./*","./package.json":"./package.json"}}"#,
        );
        write(root, "node_modules/data/index.js", "export const a = 1;");
        write(root, "node_modules/data/LICENSE", "MIT");
        write(
            root,
            "node_modules/data/types.d.ts",
            "export declare const a: number;",
        );
        assert_eq!(reach(root, "data"), PackageReach::Clear);
    }

    /// `@types/webxr` declares globals with no module in them and no runtime package: nothing to read, and nothing says who provides them (the host, a script or a
    /// package that writes the global), so it is undecided as an unknown global (ADR 0973; ADR 0972 judged it clear). A types package that is a module
    /// (`export …`) for a package that is not installed stays undecided as missing source.
    #[test]
    fn types_of_globals_are_undecided_and_types_of_a_missing_package_are_too() {
        let directory = project();
        let root = directory.path();
        write(
            root,
            "node_modules/@types/hostapi/package.json",
            r#"{"name":"@types/hostapi","types":"index.d.ts"}"#,
        );
        write(
            root,
            "node_modules/@types/hostapi/index.d.ts",
            "interface Xr { run(): void }\ndeclare var xr: Xr;",
        );
        write(
            root,
            "node_modules/@types/gone2/package.json",
            r#"{"name":"@types/gone2","types":"index.d.ts"}"#,
        );
        write(
            root,
            "node_modules/@types/gone2/index.d.ts",
            "export declare const run: () => void;",
        );
        let host = root
            .join("node_modules/@types/hostapi")
            .canonicalize()
            .unwrap();
        let gone = root
            .join("node_modules/@types/gone2")
            .canonicalize()
            .unwrap();
        assert!(matches!(
            runtime_root(&host),
            Err(PackageReach::Undecided {
                blocker: ReachBlocker::UnknownGlobal,
                ..
            })
        ));
        assert!(matches!(
            runtime_root(&gone),
            Err(PackageReach::Undecided {
                blocker: ReachBlocker::MissingSource,
                ..
            })
        ));
    }

    /// A bundler's `?raw` and `?url` on a relative load name data, and any other query names the file without it.
    #[test]
    fn an_asset_query_runs_nothing() {
        let directory = project();
        let root = directory.path();
        write(
            root,
            "node_modules/assets/package.json",
            r#"{"name":"assets","main":"index.js"}"#,
        );
        write(
            root,
            "node_modules/assets/index.js",
            "import frag from './s.frag?raw'; import u from './l.svg?url'; import w from './w.js?worker'; export default [frag, u, w];",
        );
        write(root, "node_modules/assets/w.js", "export default 1;");
        assert_eq!(reach(root, "assets"), PackageReach::Clear);
    }

    #[test]
    fn package_imports_and_patterns_are_followed() {
        let directory = project();
        let root = directory.path();
        write(
            root,
            "node_modules/hash/package.json",
            r##"{"name":"hash","exports":{"./*":"./lib/*.js"},"imports":{"#dep":{"browser":"./b.js","default":"solid-js"}}}"##,
        );
        write(
            root,
            "node_modules/hash/lib/a.js",
            "import x from '#dep'; export default x;",
        );
        write(root, "node_modules/hash/b.js", "export default 1;");
        assert!(
            matches!(reach(root, "hash"), PackageReach::ReachesSolid { .. }),
            "{:?}",
            reach(root, "hash")
        );
    }
}
