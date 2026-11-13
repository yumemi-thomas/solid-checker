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

/// The reach verdicts for every package (and project declaration file) that the project's facts name a declaration in.
#[must_use]
pub fn package_reach_index(
    facts: &solid_facts::ProjectFacts,
    solid: &SolidPackages<'_>,
) -> PackageReachIndex {
    let mut index = PackageReachIndex::default();
    let mut runtime = HashMap::<PathBuf, PackageReach>::new();
    for path in declaration_paths(facts) {
        let key = package_root_of(&path).unwrap_or(&path).to_owned();
        if index.by_root.contains_key(&key) {
            continue;
        }
        let reach = match package_root_of(&path) {
            Some(root) => {
                let root = PathBuf::from(root);
                match runtime_root(&root) {
                    Ok(root) => runtime
                        .entry(root.clone())
                        .or_insert_with(|| package_reach(&root, solid))
                        .clone(),
                    Err(reach) => reach,
                }
            }
            None => PackageReach::Undecided {
                blocker: ReachBlocker::AmbientDeclaration,
                detail: short(Path::new(&path)),
            },
        };
        index.by_root.insert(key, reach);
    }
    index
}

/// The declaration files and installed files the project's own files name a declaration in: each call's resolved declaration (outside the
/// standard library), each referenced symbol's declarations, through its alias, and each attested import's file.
fn declaration_paths(facts: &solid_facts::ProjectFacts) -> BTreeSet<String> {
    let mut paths = BTreeSet::new();
    let mut keep = |path: &str| {
        if !path.starts_with("bundled:") && (path.contains("/node_modules/") || is_declaration_file(path)) {
            paths.insert(path.to_owned());
        }
    };
    // The files the compiler resolved the project's imports to: a tag or a member no entity names is decided through its import.
    if let Some(imports) = &facts.resolved_imports {
        for (importer, import) in imports.iter() {
            if !importer.contains("/node_modules/") && facts.dependency_source_root(importer).is_none() && !import.resolved_path.is_empty() {
                keep(&import.resolved_path);
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
    find_package_root(&root.join("package.json"), &described, None)
        .and_then(|root| root.canonicalize().ok())
        .ok_or_else(|| PackageReach::Undecided {
            blocker: ReachBlocker::MissingSource,
            detail: format!("{name} describes {described}, which is not installed"),
        })
}

fn manifest(root: &Path) -> Result<serde_json::Value, PackageReach> {
    let unread = |detail: String| PackageReach::Undecided {
        blocker: ReachBlocker::ManifestUnread,
        detail,
    };
    let bytes = fs::read(root.join("package.json"))
        .map_err(|error| unread(format!("{}: {error}", short(root))))?;
    serde_json::from_slice(&bytes).map_err(|error| unread(format!("{}: {error}", short(root))))
}

/// The closure walk of one package.
fn package_reach(root: &Path, solid: &SolidPackages<'_>) -> PackageReach {
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
        packages: BTreeSet::new(),
    };
    match walk.run(root) {
        Ok(()) => PackageReach::Clear,
        Err(reach) => reach,
    }
}

struct Walk<'s, 'd> {
    solid: &'s SolidPackages<'d>,
    files: BTreeSet<PathBuf>,
    queue: VecDeque<PathBuf>,
    packages: BTreeSet<PathBuf>,
}

impl Walk<'_, '_> {
    fn run(&mut self, root: &Path) -> Result<(), PackageReach> {
        self.enter_package(root)?;
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

    /// Every entry of the package at `root`, once.
    fn enter_package(&mut self, root: &Path) -> Result<(), PackageReach> {
        let root = root
            .canonicalize()
            .map_err(|error| missing(format!("{}: {error}", short(root))))?;
        if !self.packages.insert(root.clone()) {
            return Ok(());
        }
        for entry in package_entries(&root)? {
            self.push(entry);
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
        if subpath == "." || manifest(&root)?.get("exports").is_some() {
            // `exports` lists every file a subpath may select: the package's entries are a superset of any one.
            return self.enter_package(&root);
        }
        let target =
            resolve_path(&root.join(subpath.trim_start_matches("./"))).ok_or_else(|| {
                missing(format!(
                    "{} loads {specifier}, which resolves to no file",
                    short(file)
                ))
            })?;
        self.push(target);
        Ok(())
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

/// A path as a bundler resolves it: the file, the file with an extension, a directory's manifest `main` or its `index` file. Canonical.
fn resolve_path(base: &Path) -> Option<PathBuf> {
    if base.is_file() {
        return base.canonicalize().ok();
    }
    let text = base.to_string_lossy();
    for extension in RESOLVE_EXTENSIONS {
        let candidate = PathBuf::from(format!("{text}{extension}"));
        if candidate.is_file() {
            return candidate.canonicalize().ok();
        }
    }
    if base.is_dir() {
        if let Ok(manifest) = manifest(base)
            && let Some(main) = manifest.get("main").and_then(serde_json::Value::as_str)
            && let Some(target) = resolve_path(&base.join(main))
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

/// Every runtime file a package can be entered at: the targets of `exports` under every condition (patterns expanded), `main`, `module`,
/// an object `browser` field's files, else `index.js`. A target that is not installed loads nothing and is skipped.
fn package_entries(root: &Path) -> Result<Vec<PathBuf>, PackageReach> {
    let manifest = manifest(root)?;
    let mut targets = Vec::<String>::new();
    if let Some(exports) = manifest.get("exports") {
        collect_targets(exports, &mut targets, 0).map_err(|detail| PackageReach::Undecided {
            blocker: ReachBlocker::ManifestUnread,
            detail: format!("{}: {detail}", short(root)),
        })?;
    }
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
    Ok(entries.into_iter().collect())
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
            if relative.starts_with(prefix)
                && relative.ends_with(&suffix)
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
        )
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
            "node_modules/inner/package.json",
            r#"{"name":"inner","exports":{"./sub":"./sub.js","./other":"./other.js"}}"#,
        );
        write(root, "node_modules/inner/sub.js", "export const inner = 1;");
        write(
            root,
            "node_modules/inner/other.js",
            "export { action } from '@solidjs/signals';",
        );
        // Every export of `inner` is a candidate: a superset never clears what one subpath could reach.
        assert!(matches!(
            reach(root, "outer"),
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
