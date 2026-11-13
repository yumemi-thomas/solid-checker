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
//! action. A declaration in `@types/<name>` describes the code of `<name>`, looked up from where the types are installed; a declaration in
//! a declaration file of the project itself (`declare module`, `declare function`) describes code no package holds, and is undecided.
//!
//! File scans are kept for the life of the process, keyed by path and checked against the file's length and modification time, so a daemon
//! scans a package once.

use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::SystemTime;

use solid_facts::module_reach::{ModuleHazardKind, ModuleScan, scan_module};
use solid_facts::package_reach::{
    PackageReach, PackageReachIndex, ReachBlocker, is_declaration_file, package_root_of,
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
    MANIFESTS.with(|held| held.borrow_mut().clear());
    // Each declaration root, with the runtime root that holds its code, the runtime files beside its declarations and the subpaths the program imports.
    struct Named {
        runtime: Result<PathBuf, PackageReach>,
        siblings: BTreeSet<PathBuf>,
        subpaths: BTreeSet<String>,
    }
    let mut roots = BTreeMap::<String, Named>::new();
    for path in declaration_paths(facts) {
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
    let verdicts = by_runtime
        .into_iter()
        .map(|(runtime, (demand, siblings))| {
            let reach = package_reach(&runtime, solid, &demand, &siblings);
            (runtime, reach)
        })
        .collect::<HashMap<_, _>>();
    let mut index = PackageReachIndex::default();
    for (root, named) in roots {
        let reach = match named.runtime {
            Ok(runtime) => verdicts
                .get(&runtime)
                .cloned()
                .unwrap_or_else(|| missing(format!("no verdict for {}", short(&runtime)))),
            Err(reach) => reach,
        };
        if let Some(name) = manifest(Path::new(&root))
            .ok()
            .and_then(|manifest| manifest.get("name")?.as_str().map(str::to_owned))
        {
            index.names.insert(root.clone(), name);
        }
        index.by_root.insert(root, reach);
    }
    index.linked = linked_roots(facts);
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

/// The installed files (under `node_modules`) the project's own files name a declaration in: each call's resolved declaration (outside the
/// standard library), each referenced symbol's declarations, through its alias, and each attested import's file.
fn declaration_paths(facts: &solid_facts::ProjectFacts) -> BTreeSet<String> {
    let mut paths = BTreeSet::new();
    let mut keep = |path: &str| {
        if !path.starts_with("bundled:") && path.contains("/node_modules/") {
            paths.insert(path.to_owned());
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
    paths
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
        // Types for the host (`@types/webxr`, the browser's APIs): declarations with no module in them, so no installed package holds their code.
        None if declares_only_globals(root) => Err(PackageReach::Clear),
        None => Err(PackageReach::Undecided {
            blocker: ReachBlocker::MissingSource,
            detail: format!("{name} describes {described}, which is not installed"),
        }),
    }
}

/// Whether the declaration package at `root` is a script of global declarations: its entry declaration file (`types`, `typings`, `index.d.ts`) has no
/// top-level `import` or `export`, so it describes what the host provides (`@types/webxr`), not a module an installed package implements.
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
/// ask for it (every bare load, every self-reference check, every `main` of a directory). An index is computed from the tree as it stands, so one
/// computation's manifests are one snapshot; the next computation starts empty.
type Manifests = HashMap<PathBuf, Result<Arc<serde_json::Value>, PackageReach>>;

thread_local! {
    static MANIFESTS: std::cell::RefCell<Manifests> = std::cell::RefCell::default();
}

fn manifest(root: &Path) -> Result<Arc<serde_json::Value>, PackageReach> {
    if let Some(held) = MANIFESTS.with(|held| held.borrow().get(root).cloned()) {
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
    MANIFESTS.with(|held| held.borrow_mut().insert(root.to_path_buf(), read.clone()));
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

impl Demand {
    fn subpath(subpath: &str) -> Self {
        Self {
            subpaths: BTreeSet::from([subpath.to_owned()]),
            everything: false,
        }
    }
}

/// The closure walk of one package.
fn package_reach(
    root: &Path,
    solid: &SolidPackages<'_>,
    demand: &Demand,
    extra_entries: &BTreeSet<PathBuf>,
) -> PackageReach {
    let name = manifest(root)
        .ok()
        .and_then(|manifest| manifest.get("name")?.as_str().map(str::to_owned))
        .unwrap_or_default();
    if solid.contains(&name) {
        return PackageReach::ReachesSolid { via: name };
    }
    let mut walk = Walk {
        solid,
        files: BTreeSet::new(),
        queue: VecDeque::new(),
        entered: BTreeSet::new(),
    };
    match walk.run(root, demand, extra_entries) {
        Ok(()) => PackageReach::Clear,
        Err(reach) => reach,
    }
}

struct Walk<'s, 'd> {
    solid: &'s SolidPackages<'d>,
    files: BTreeSet<PathBuf>,
    queue: VecDeque<PathBuf>,
    /// The (package, subpath) pairs whose entries were pushed; `*` is every entry.
    entered: BTreeSet<(PathBuf, String)>,
}

impl Walk<'_, '_> {
    fn run(
        &mut self,
        root: &Path,
        demand: &Demand,
        extra_entries: &BTreeSet<PathBuf>,
    ) -> Result<(), PackageReach> {
        self.enter(root, demand)?;
        for entry in extra_entries {
            self.push(entry.clone());
        }
        while let Some(file) = self.queue.pop_front() {
            if self.files.len() > MAX_CLOSURE_FILES {
                return Err(PackageReach::Undecided {
                    blocker: ReachBlocker::ClosureLimit,
                    detail: format!("more than {MAX_CLOSURE_FILES} files"),
                });
            }
            self.visit(&file)?;
        }
        Ok(())
    }

    fn push(&mut self, file: PathBuf) {
        if self.files.insert(file.clone()) {
            self.queue.push_back(file);
        }
    }

    /// The entries `demand` asks of the package at `root`, each pair once.
    fn enter(&mut self, root: &Path, demand: &Demand) -> Result<(), PackageReach> {
        let root = root
            .canonicalize()
            .map_err(|error| missing(format!("{}: {error}", short(root))))?;
        if demand.everything && self.entered.insert((root.clone(), "*".to_owned())) {
            for entry in all_entries(&root)? {
                self.push(entry);
            }
        }
        for subpath in &demand.subpaths {
            if self.entered.insert((root.clone(), subpath.clone())) {
                for entry in subpath_entries(&root, subpath)? {
                    self.push(entry);
                }
            }
        }
        Ok(())
    }

    fn visit(&mut self, file: &Path) -> Result<(), PackageReach> {
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
        if is_declaration_file(&name)
            || CODE_FREE_EXTENSIONS
                .iter()
                .any(|extension| name.ends_with(extension))
        {
            // A declaration file runs nothing; a stylesheet, an image or JSON holds no code a package wrote.
            return Ok(());
        }
        if !SCRIPT_EXTENSIONS
            .iter()
            .any(|extension| name.ends_with(extension))
        {
            // A file a bundler plugin compiles (`.vue`, `.svelte`) may become code that loads anything.
            return Err(PackageReach::Undecided {
                blocker: ReachBlocker::Unparseable,
                detail: format!("{}: a file type the walk does not read", short(file)),
            });
        }
        let scan = scanned(file)?;
        if let Some(hazard) = scan.hazards.first() {
            return Err(PackageReach::Undecided {
                blocker: blocker_of(hazard.kind),
                detail: format!("{}: {}", short(file), hazard.detail),
            });
        }
        let directory = file.parent().unwrap_or(Path::new("/"));
        for specifier in &scan.loads {
            self.load(file, directory, specifier)?;
        }
        Ok(())
    }

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
                    Target::Bare(bare) => self.load(file, directory, &bare)?,
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
        let root = self_reference(file, &name).or_else(|| find_package_root(file, &name, None));
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
        self.enter(&root, &Demand::subpath(&subpath))
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
    if base.is_file() {
        return base.canonicalize().ok();
    }
    let text = base.to_string_lossy();
    for spelling in source_spellings(&text) {
        let candidate = PathBuf::from(spelling);
        if candidate.is_file() {
            return candidate.canonicalize().ok();
        }
    }
    for extension in RESOLVE_EXTENSIONS {
        let candidate = PathBuf::from(format!("{text}{extension}"));
        if candidate.is_file() {
            return candidate.canonicalize().ok();
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
                return candidate.canonicalize().ok();
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
                && let Ok(path) = path.canonicalize()
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
    let mut directory = file.parent();
    while let Some(current) = directory {
        if current.join("package.json").is_file() {
            let manifest = manifest(current).ok()?;
            return (manifest.get("name").and_then(serde_json::Value::as_str) == Some(name)
                && manifest.get("exports").is_some())
            .then(|| current.to_path_buf());
        }
        directory = current.parent();
    }
    None
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
        package_reach(
            &root.join("node_modules").join(name).canonicalize().unwrap(),
            &SOLID,
            &everything(),
            &BTreeSet::new(),
        )
    }

    fn everything() -> Demand {
        Demand {
            everything: true,
            ..Demand::default()
        }
    }

    fn reach_of(root: &Path, name: &str, demand: &Demand) -> PackageReach {
        package_reach(
            &root.join("node_modules").join(name).canonicalize().unwrap(),
            &SOLID,
            demand,
            &BTreeSet::new(),
        )
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
        assert!(
            matches!(
                package_reach(&package, &SOLID, &everything(), &siblings),
                PackageReach::ReachesSolid { .. }
            ),
            "{:?}",
            package_reach(&package, &SOLID, &everything(), &siblings)
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

    /// `@types/webxr` declares host globals with no module in them and no runtime package: nothing to read, no Solid in it. A types package that is a module
    /// (`export …`) for a package that is not installed stays undecided.
    #[test]
    fn types_of_the_host_are_clear_and_types_of_a_missing_package_are_not() {
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
        assert_eq!(runtime_root(&host).unwrap_err(), PackageReach::Clear);
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
