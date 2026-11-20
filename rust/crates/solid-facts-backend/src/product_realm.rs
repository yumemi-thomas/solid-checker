//! ADR 1145: the product realm of a program, from its build configuration.
//!
//! A site is judged against the prototype writers of the page it runs in. A program's product entries (the module scripts of its `index.html`, the targets of its
//! package's `exports`) load the files the product ships, and the packages those files load are the writers that realm holds. A test, a story, a script or a config is
//! another entry: it loads its own packages (a runner, an assertion library) into a process the page does not share. So a site in a file the product's entries load is
//! judged against the writers the product's realm holds, and every other site against all of them ([`solid_facts::package_reach::ProductWriters`]).
//!
//! The entry set is a read of the build configuration, never a file name, and every doubt ends it ([`Realm::Unknown`]: no site changes):
//! - the entries: the module scripts of the `index.html` beside the project (host discovery's HTML grammar, which refuses a page with a classic script, an inline
//!   module, an external URL or markup it does not read), and the runtime targets of the `package.json` beside it (`exports` without a pattern, `main`, `module`,
//!   `browser`), each a file of the program; a program with neither is unknown;
//! - the bundler: every `vite.config.*` from the project directory up to its workspace root must parse in the closed Vite config grammar of host discovery
//!   (`host_config`) with only plugins that add no load but Solid's own and the dev client (Solid's, Tailwind, tsconfig paths, Uno without a config file); any other
//!   bundler configuration file in that chain is unknown, and so is a second config of one directory;
//! - the closure of the entries over every `import`, `export … from`, literal `import()` and `require`, and `import x = require()`, by the attested resolution of each
//!   specifier. A load by a computed specifier, `import.meta.glob`, a worker, a service worker, `new URL(…, import.meta.url)`, a specifier no row attests or that
//!   resolves nowhere, a resolved file the program does not analyze (an asset and a JSON module excepted), and an entry the program does not hold end it;
//! - the declared host: a browser target with a declared build. In a development build the page also loads the dev server's client, so the packages named `vite` and
//!   the config's own plugin packages are in the realm whatever the files import.
//!
//! Premise, named in `docs/open-work.md`: no recognized plugin adds a load of a package that writes a built-in's prototype to the page, and code outside the program
//! shares the page with it as the package integrity premise (ADR 0300) already assumes.

use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::fs;
use std::path::{Component, Path, PathBuf};

use solid_facts::ast::ModuleHazardKind;
use solid_facts::core::Span;
use solid_facts::package_reach::{PackageReachIndex, ProductWriters, package_root_of};
use solid_facts::resolution::{ImportResolution, SpecifierAttestation};
use solid_facts::{FileFacts, ProjectFacts};
use solid_reactive_ir::{RuntimeBuild, RuntimeEnvironment, RuntimeTarget};

/// One module the product's real entries name, and what named it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Entry {
    pub path: String,
    pub source: String,
}

/// The product realm of one program.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Realm {
    Known(Known),
    /// The entry set is not decided: no site changes. Every stage that could not decide it names itself (a measurement reads them all; the first ended the read).
    Unknown(Vec<Blocker>),
}

/// One stage of the read that did not decide, and why.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Blocker {
    pub stage: &'static str,
    pub reason: String,
    /// A measurement assumed the stage away ([`assumed`]): it does not keep the realm unknown.
    pub assumed: bool,
}

impl Blocker {
    fn new(stage: &'static str, reason: impl Into<String>) -> Self {
        Self {
            stage,
            reason: reason.into(),
            assumed: assumed(stage),
        }
    }
}

/// `SOLID_CHECKER_PRODUCT_REALM_ASSUME=<stage>[,<stage>...]` (`pages`, `bundler`): a measurement of what a stage's reader would free if it decided, the way
/// `--assume` is for a hazard (ADR 0950). The stage's blocker is not raised; nothing else is read differently. Never set by a check that ships a verdict.
fn assumed(stage: &str) -> bool {
    static ASSUMED: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    ASSUMED
        .get_or_init(|| {
            std::env::var("SOLID_CHECKER_PRODUCT_REALM_ASSUME")
                .map(|value| {
                    value
                        .split(',')
                        .map(|stage| stage.trim().to_owned())
                        .collect()
                })
                .unwrap_or_default()
        })
        .iter()
        .any(|assumed| assumed == stage)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Known {
    pub entries: Vec<Entry>,
    /// The program's files the entries load.
    pub files: BTreeSet<String>,
    /// The installed package roots the files load, each as the resolver gave it (a symlink path where there is one, the real path too).
    pub roots: BTreeSet<String>,
    /// The package names whose roots the declared host loads in a development build.
    pub host_names: BTreeSet<String>,
    /// Whether the declared build is a development build (the dev client is loaded).
    pub development: bool,
    /// The stages a measurement assumed (empty in any check that ships a verdict).
    pub assumed: Vec<Blocker>,
}

/// The sources the closed config grammar records ([`crate::host_config::Config::plugins`]) whose reviewed behaviour adds no load to the page but Solid's own and
/// the dev client: the two config helpers (`defineConfig`, which is no plugin), Solid's plugin, Tailwind, tsconfig paths and Uno without a config file.
const INERT_PLUGINS: [&str; 8] = [
    "vite",
    "vitest/config",
    "@solidjs/vite-plugin",
    "vite-plugin-solid",
    "@tailwindcss/vite",
    "vite-tsconfig-paths",
    "unocss/vite",
    "@unocss/vite",
];

/// Bundler and framework configuration files whose entries or injected loads this reader does not read: a directory holding one is unknown.
const UNREAD_CONFIGS: [&str; 14] = [
    "rsbuild.config",
    "rspack.config",
    "webpack.config",
    "rollup.config",
    "esbuild.config",
    "tsup.config",
    "app.config",
    "nuxt.config",
    "next.config",
    "astro.config",
    "svelte.config",
    "farm.config",
    "parcel.config",
    "bun.build",
];

const CONFIG_EXTENSIONS: [&str; 6] = ["ts", "js", "mts", "mjs", "cts", "cjs"];

/// The product realm of the program in `directory` (its project directory) under `runtime`: every stage is read, so an unknown realm names all that failed.
pub(crate) fn read(directory: &Path, facts: &ProjectFacts, runtime: &RuntimeEnvironment) -> Realm {
    let mut blockers = Vec::<Blocker>::new();
    if runtime.target != Some(RuntimeTarget::Browser) {
        blockers.push(Blocker::new(
            "runtime",
            "the declared target is not a browser",
        ));
    }
    let build = runtime.build;
    if build.is_none() {
        blockers.push(Blocker::new("runtime", "no runtime build is declared"));
    }
    let by_path = facts
        .files
        .iter()
        .map(|file| (file.path.as_str(), file))
        .collect::<HashMap<_, _>>();
    match other_page(directory) {
        Ok(None) => {}
        Ok(Some(other)) => blockers.push(Blocker::new(
            "pages",
            format!(
                "{} is a second HTML page, and a module may be shared between pages",
                other.display()
            ),
        )),
        Err(reason) => blockers.push(Blocker::new("pages", reason)),
    }
    let host_names = match bundler(directory) {
        Ok(names) => names,
        Err(reason) => {
            blockers.push(Blocker::new("bundler", reason));
            config_import_names(directory)
        }
    };
    let entries = match entries_of(directory, &|path| by_path.contains_key(path)) {
        Ok(entries) => entries,
        Err(reason) => {
            blockers.push(Blocker::new("entries", reason));
            return Realm::Unknown(blockers);
        }
    };
    let (files, roots) = match closure(facts, &by_path, &entries) {
        Ok(closure) => closure,
        Err(reason) => {
            blockers.push(Blocker::new("closure", reason));
            return Realm::Unknown(blockers);
        }
    };
    let Some(build) = build.filter(|_| blockers.iter().all(|blocker| blocker.assumed)) else {
        return Realm::Unknown(blockers);
    };
    Realm::Known(Known {
        entries,
        files,
        roots,
        host_names,
        development: build == RuntimeBuild::Development,
        assumed: blockers,
    })
}

/// A path as the program names its files: absolute, with no `.` or `..` component, the real path where the file exists.
fn program_path(path: &Path) -> String {
    let mut normal = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normal.pop();
            }
            other => normal.push(other.as_os_str()),
        }
    }
    fs::canonicalize(&normal)
        .unwrap_or(normal)
        .to_string_lossy()
        .into_owned()
}

fn entries_of(directory: &Path, in_program: &dyn Fn(&str) -> bool) -> Result<Vec<Entry>, String> {
    let mut entries = Vec::<Entry>::new();
    let add = |entries: &mut Vec<Entry>, path: PathBuf, source: String| -> Result<(), String> {
        // A style sheet, an image or a data file an entry names loads no module.
        if is_asset(&path.to_string_lossy()) || path.extension().is_some_and(|e| e == "json") {
            return Ok(());
        }
        let path = program_path(&path);
        if !in_program(path.as_str()) {
            return Err(format!(
                "the entry {path} ({source}) is not a file of the program"
            ));
        }
        if !entries.iter().any(|entry| entry.path == path) {
            entries.push(Entry { path, source });
        }
        Ok(())
    };
    // The module scripts of the page beside the project.
    let html = directory.join("index.html");
    if html.is_file() {
        let source =
            fs::read_to_string(&html).map_err(|error| format!("{}: {error}", html.display()))?;
        let modules = crate::inferred_host::html_module_entries(&html, &source)?;
        if modules.is_empty() {
            return Err(format!("{} names no module script", html.display()));
        }
        for module in modules {
            let relative = module.trim_start_matches('/');
            if relative.is_empty() || relative.contains("://") {
                return Err(format!(
                    "{}: the module entry {module} is not a project file",
                    html.display()
                ));
            }
            add(&mut entries, directory.join(relative), "html".to_owned())?;
        }
    }
    // The runtime targets of the package beside the project.
    let manifest = directory.join("package.json");
    if manifest.is_file() {
        let text = fs::read_to_string(&manifest)
            .map_err(|error| format!("{}: {error}", manifest.display()))?;
        let package: serde_json::Value = serde_json::from_str(&text)
            .map_err(|error| format!("{}: {error}", manifest.display()))?;
        let mut targets = Vec::<String>::new();
        if let Some(exports) = package.get("exports") {
            exports_targets(exports, &mut targets)
                .map_err(|reason| format!("{}: {reason}", manifest.display()))?;
        }
        for field in ["main", "module", "browser"] {
            match package.get(field) {
                None => {}
                Some(serde_json::Value::String(target)) => targets.push(target.clone()),
                Some(_) => {
                    return Err(format!("{}: `{field}` is not a string", manifest.display()));
                }
            }
        }
        for target in targets {
            let relative = target.trim_start_matches("./");
            if relative.is_empty() || relative.starts_with('/') || target.contains("://") {
                return Err(format!(
                    "{}: the target {target} is not a project file",
                    manifest.display()
                ));
            }
            add(&mut entries, directory.join(relative), "package".to_owned())?;
        }
    }
    if entries.is_empty() {
        return Err(format!(
            "{} has neither an index.html nor a package manifest naming entries",
            directory.display()
        ));
    }
    Ok(entries)
}

/// The directories a page search does not enter: installed code, VCS data and build output (a page there is generated or a package's own).
const PAGE_SEARCH_SKIPS: [&str; 9] = [
    "node_modules",
    ".git",
    "dist",
    "build",
    "out",
    "coverage",
    ".vite",
    ".solid-checker",
    "target",
];

/// The files a page search reads before it gives up: a tree this large is not read, and the realm is unknown.
const PAGE_SEARCH_LIMIT: usize = 50_000;

/// Whether the HTML file at `path` may run code in a page that shares modules with another: a script element, a dynamic `import(`, a `javascript:` URL or an inline
/// event handler. A static file (a site-verification token, an error page of markup alone) loads nothing. A file that cannot be read may.
fn may_run_code(path: &Path) -> Result<bool, String> {
    let Ok(bytes) = fs::read(path) else {
        return Ok(true);
    };
    let text = String::from_utf8_lossy(&bytes).to_ascii_lowercase();
    if text.contains("<script") || text.contains("import(") || text.contains("javascript:") {
        return Ok(true);
    }
    // An inline handler: whitespace, `on`, letters, optional spaces, `=`.
    let bytes = text.as_bytes();
    for at in 0..bytes.len().saturating_sub(3) {
        if bytes[at].is_ascii_whitespace() && bytes[at + 1] == b'o' && bytes[at + 2] == b'n' {
            let mut end = at + 3;
            while end < bytes.len() && bytes[end].is_ascii_lowercase() {
                end += 1;
            }
            while end < bytes.len() && bytes[end].is_ascii_whitespace() {
                end += 1;
            }
            if end > at + 3 && bytes.get(end) == Some(&b'=') {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

/// The first HTML file under `directory` that is not its own `index.html`, or an error where the tree is too large to search or cannot be read.
fn other_page(directory: &Path) -> Result<Option<PathBuf>, String> {
    let own = directory.join("index.html");
    let mut pending = vec![directory.to_owned()];
    let mut seen = 0_usize;
    while let Some(dir) = pending.pop() {
        let listing = fs::read_dir(&dir).map_err(|error| format!("{}: {error}", dir.display()))?;
        for entry in listing {
            let entry = entry.map_err(|error| format!("{}: {error}", dir.display()))?;
            seen += 1;
            if seen > PAGE_SEARCH_LIMIT {
                return Err(format!(
                    "{} holds too many files to look for a second HTML page",
                    directory.display()
                ));
            }
            let path = entry.path();
            let kind = entry
                .file_type()
                .map_err(|error| format!("{}: {error}", path.display()))?;
            if kind.is_dir() {
                let name = entry.file_name();
                if !PAGE_SEARCH_SKIPS.iter().any(|skip| name == *skip) {
                    pending.push(path);
                }
            } else if path.extension().is_some_and(|extension| {
                extension.eq_ignore_ascii_case("html") || extension.eq_ignore_ascii_case("htm")
            }) && path != own
                && may_run_code(&path)?
            {
                return Ok(Some(path));
            }
        }
    }
    Ok(None)
}

/// The runtime targets of an `exports` value: every string leaf outside a `types` condition. A pattern (`*`) names files this reader cannot list.
fn exports_targets(value: &serde_json::Value, targets: &mut Vec<String>) -> Result<(), String> {
    match value {
        serde_json::Value::String(target) => {
            if target.contains('*') {
                return Err(format!(
                    "the export pattern {target} names files it does not list"
                ));
            }
            targets.push(target.clone());
            Ok(())
        }
        serde_json::Value::Object(map) => {
            for (key, inner) in map {
                if key == "types" || key.starts_with("types@") {
                    continue;
                }
                exports_targets(inner, targets)?;
            }
            Ok(())
        }
        serde_json::Value::Array(items) => {
            for item in items {
                exports_targets(item, targets)?;
            }
            Ok(())
        }
        serde_json::Value::Null => Ok(()),
        _ => Err("an export target that is not a string".to_owned()),
    }
}

/// The directories from `directory` up to its workspace root (the first holding a `.git`, a `pnpm-workspace.yaml` or a manifest with `workspaces`), at most eight.
fn chain(directory: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for ancestor in directory.ancestors().take(8) {
        found.push(ancestor.to_owned());
        if ancestor.join(".git").exists()
            || ancestor.join("pnpm-workspace.yaml").is_file()
            || fs::read_to_string(ancestor.join("package.json"))
                .ok()
                .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
                .is_some_and(|package| package.get("workspaces").is_some())
        {
            break;
        }
    }
    found
}

/// The Vite configs of the chain, or why the chain is not read: another bundler's configuration, or two Vite configs of one directory.
fn vite_configs(directory: &Path) -> Result<Vec<PathBuf>, String> {
    let mut configs = Vec::new();
    for dir in chain(directory) {
        for unread in UNREAD_CONFIGS {
            for extension in CONFIG_EXTENSIONS {
                let path = dir.join(format!("{unread}.{extension}"));
                if path.exists() {
                    return Err(format!(
                        "{} is a bundler configuration this reader does not read",
                        path.display()
                    ));
                }
            }
        }
        let vites = CONFIG_EXTENSIONS
            .iter()
            .map(|extension| dir.join(format!("vite.config.{extension}")))
            .filter(|path| path.exists())
            .collect::<Vec<_>>();
        match vites.as_slice() {
            [] => {}
            [path] => configs.push(path.clone()),
            _ => return Err(format!("{} holds more than one Vite config", dir.display())),
        }
    }
    Ok(configs)
}

/// Every bundler configuration of the chain, read: the package names the Vite configs import (the dev client's plugins), or why one is unknown.
fn bundler(directory: &Path) -> Result<BTreeSet<String>, String> {
    let mut names = BTreeSet::<String>::from(["vite".to_owned()]);
    for path in vite_configs(directory)? {
        let source =
            fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))?;
        let config = crate::host_config::parse_realm(&path, &source)
            .map_err(|refusal| format!("{}: {}", path.display(), refusal.reason))?;
        if let Some(plugin) = config
            .plugins
            .iter()
            .find(|plugin| !INERT_PLUGINS.contains(&plugin.as_str()))
        {
            return Err(format!(
                "{}: the plugin {plugin} may add loads this reader does not follow",
                path.display()
            ));
        }
        names.extend(imported_packages(&source));
    }
    Ok(names)
}

/// The package names every readable Vite config of the chain imports, whether or not the grammar admits it (the names an assumed bundler stage keeps in a dev
/// build, a superset of what its plugins load).
fn config_import_names(directory: &Path) -> BTreeSet<String> {
    let mut names = BTreeSet::<String>::from(["vite".to_owned()]);
    for dir in chain(directory) {
        for extension in CONFIG_EXTENSIONS {
            if let Ok(source) = fs::read_to_string(dir.join(format!("vite.config.{extension}"))) {
                names.extend(imported_packages(&source));
            }
        }
    }
    names
}

/// The package names a config imports (`from "pkg"`, a scoped name whole, a subpath dropped), a superset of what its plugins load.
fn imported_packages(source: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let mut rest = source;
    while let Some(at) = rest.find("from ") {
        rest = &rest[at + 5..];
        let trimmed = rest.trim_start();
        let Some(quote) = trimmed.chars().next().filter(|c| *c == '"' || *c == '\'') else {
            continue;
        };
        let Some(end) = trimmed[1..].find(quote) else {
            continue;
        };
        let specifier = &trimmed[1..1 + end];
        if specifier.starts_with('.')
            || specifier.starts_with("node:")
            || specifier.starts_with('#')
        {
            continue;
        }
        let mut parts = specifier.split('/');
        let first = parts.next().unwrap_or_default();
        let name = if first.starts_with('@') {
            format!("{first}/{}", parts.next().unwrap_or_default())
        } else {
            first.to_owned()
        };
        names.insert(name);
    }
    names
}

/// The extensions and queries a bundler serves as an asset, which evaluate no module of the program.
const ASSET_EXTENSIONS: &[&str] = &[
    "css", "scss", "sass", "less", "styl", "pcss", "svg", "png", "jpg", "jpeg", "gif", "webp",
    "avif", "ico", "bmp", "woff", "woff2", "ttf", "otf", "eot", "mp3", "mp4", "webm", "wav", "ogg",
    "txt", "md",
];

fn is_asset(specifier: &str) -> bool {
    let (path, query) = specifier.split_once('?').unwrap_or((specifier, ""));
    if !query.is_empty() {
        return matches!(query, "raw" | "url" | "inline" | "no-inline");
    }
    path.rsplit_once('.').is_some_and(|(_, extension)| {
        ASSET_EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str())
    })
}

/// Whether `source` names `word` as an identifier of its own (no identifier character on either side): `Worker` in `new Worker(url)` or `Reflect.construct(Worker, …)`,
/// not in `decodeWorker(entry)` or `WorkerRow`.
fn names_identifier(source: &str, word: &str) -> bool {
    let bytes = source.as_bytes();
    let identifier = |byte: u8| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$';
    let mut from = 0;
    while let Some(found) = source[from..].find(word) {
        let at = from + found;
        let end = at + word.len();
        let before = at.checked_sub(1).and_then(|at| bytes.get(at)).copied();
        if !before.is_some_and(identifier) && !bytes.get(end).copied().is_some_and(identifier) {
            return true;
        }
        from = end;
    }
    false
}

/// The text shapes that load code another way than an import: a worker (another realm that may share a module), a service worker, `importScripts`, a glob, and a URL
/// resolved against the module (a bundler emits the file it names).
fn hidden_load(source: &str) -> Option<&'static str> {
    if source.contains("import.meta.glob") || source.contains("import.meta[") {
        return Some("import.meta.glob");
    }
    if names_identifier(source, "Worker") || names_identifier(source, "SharedWorker") {
        return Some("a worker");
    }
    if source.contains("serviceWorker") || names_identifier(source, "importScripts") {
        return Some("a service worker");
    }
    if source.contains("new URL(") && source.contains("import.meta.url") {
        return Some("new URL(…, import.meta.url)");
    }
    None
}

/// Why `file` may load a module its syntax does not name.
fn file_hidden_load(file: &FileFacts) -> Option<&'static str> {
    if file
        .ast
        .module_hazards
        .iter()
        .any(|hazard| hazard.kind == ModuleHazardKind::NonliteralDynamicLoading)
    {
        return Some("a load by a computed specifier");
    }
    if !file.ast.in_place_import_globs.is_empty() {
        return Some("import.meta.glob");
    }
    hidden_load(&file.source)
}

/// Every runtime load of `file` with a literal specifier: the span its attestation is joined by, and the specifier.
fn loads(file: &FileFacts) -> Vec<(Span, &str)> {
    let ast = &file.ast;
    let mut found = Vec::new();
    found.extend(
        ast.imports
            .iter()
            .filter(|import| !import.type_only)
            .map(|import| (import.span, import.module.as_str())),
    );
    found.extend(
        ast.exports
            .iter()
            .filter(|export| !export.type_only)
            .filter_map(|export| export.module.as_deref().map(|module| (export.span, module))),
    );
    found.extend(ast.module_loads.iter().filter_map(|load| {
        load.specifier
            .as_deref()
            .map(|specifier| (load.span, specifier))
    }));
    found.extend(
        ast.import_equals
            .iter()
            .map(|load| (load.span, load.module.as_str())),
    );
    found
}

/// The files and installed package roots the entries load, or why the closure is open.
fn closure(
    facts: &ProjectFacts,
    by_path: &HashMap<&str, &FileFacts>,
    entries: &[Entry],
) -> Result<(BTreeSet<String>, BTreeSet<String>), String> {
    let Some(imports) = &facts.resolved_imports else {
        return Err("no attested import resolutions".to_owned());
    };
    let mut files = BTreeSet::<String>::new();
    let mut roots = BTreeSet::<String>::new();
    let mut queue = VecDeque::new();
    for entry in entries {
        queue.push_back(entry.path.clone());
    }
    while let Some(path) = queue.pop_front() {
        if !files.insert(path.clone()) {
            continue;
        }
        let Some(file) = by_path.get(path.as_str()) else {
            return Err(format!("{path} is not a file of the program"));
        };
        if let Some(why) = file_hidden_load(file) {
            return Err(format!("{path}: {why}"));
        }
        if !imports.covers(&path) && !loads(file).is_empty() {
            return Err(format!("{path}: its imports were not attested"));
        }
        for (at, specifier) in loads(file) {
            if is_asset(specifier) {
                continue;
            }
            if specifier.contains("?worker") || specifier.contains("?sharedworker") {
                return Err(format!("{path}: {specifier} is a worker"));
            }
            let row = match imports.specifier(&path, at, specifier) {
                SpecifierAttestation::Attested(row) => row,
                SpecifierAttestation::Unattested => {
                    return Err(format!("{path}: no resolution row answers {specifier}"));
                }
            };
            if row.resolution == ImportResolution::Unresolved || row.resolved_path.is_empty() {
                return Err(format!("{path}: {specifier} resolves nowhere"));
            }
            let resolved = row.resolved_path.as_ref();
            let spelled = (!row.symlink_path.is_empty()).then(|| row.symlink_path.as_ref());
            let mut is_package = false;
            for keyed in std::iter::once(resolved).chain(spelled) {
                if let Some(root) = package_root_of(keyed) {
                    roots.insert(root.to_owned());
                    is_package = true;
                }
            }
            if is_package {
                // An installed package: the reach index decides what its closure writes, by root (every package it loads is in that closure), so the walk
                // names the root and does not enter its files.
                continue;
            }
            if by_path.contains_key(resolved) {
                queue.push_back(resolved.to_owned());
            } else if resolved.ends_with(".json") {
                // Data: a JSON module loads nothing.
            } else {
                return Err(format!(
                    "{path}: {specifier} resolves to {resolved}, which the program does not analyze"
                ));
            }
        }
    }
    Ok((files, roots))
}

/// The real directory of an installed root, or `None` where it cannot be resolved (then the root counts as loaded).
fn real(root: &str) -> Option<PathBuf> {
    fs::canonicalize(root).ok()
}

/// Sets `index.product` from a known realm: the files, and the writers of `index` the realm's packages (or the declared host) load. An unknown realm leaves the
/// index as it is, every site judged against every writer.
pub(crate) fn apply(realm: &Realm, index: &mut PackageReachIndex) {
    let Realm::Known(known) = realm else {
        return;
    };
    let loaded_real = known
        .roots
        .iter()
        .filter_map(|root| real(root))
        .collect::<BTreeSet<_>>();
    let host_roots = if known.development {
        index
            .names
            .iter()
            .filter(|(_, name)| known.host_names.contains(name.as_str()))
            .map(|(root, _)| root.clone())
            .collect::<BTreeSet<_>>()
    } else {
        BTreeSet::new()
    };
    let host_real = host_roots
        .iter()
        .filter_map(|root| real(root))
        .collect::<BTreeSet<_>>();
    // A root counts as loaded by the realm when its spelling or its real directory is one the files load, the declared host loads it, or its directory cannot be
    // resolved (never a reason to drop a writer).
    let loaded = |root: &str| -> bool {
        if known.roots.contains(root) || host_roots.contains(root) {
            return true;
        }
        match real(root) {
            Some(real) => loaded_real.contains(&real) || host_real.contains(&real),
            None => true,
        }
    };
    let mut writers = BTreeMap::<String, BTreeSet<String>>::new();
    for (name, roots) in &index.prototype_writers {
        let kept = roots
            .iter()
            .filter(|root| loaded(root))
            .cloned()
            .collect::<BTreeSet<_>>();
        if !kept.is_empty() {
            writers.insert(name.clone(), kept);
        }
    }
    let any = index
        .prototype_writers_any
        .iter()
        .filter(|root| loaded(root))
        .cloned()
        .collect::<BTreeSet<_>>();
    index.product = Some(ProductWriters {
        files: known.files.clone(),
        prototype_writers: writers,
        prototype_writers_any: any,
    });
}

/// `SOLID_CHECKER_IR_ABLATE_SILENCE=product-realm` (or `all`) leaves every site judged against every writer: the mutation test and the monotone gate's
/// ablation of this change, read once.
pub(crate) fn ablated() -> bool {
    static ABLATED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ABLATED.get_or_init(|| {
        std::env::var("SOLID_CHECKER_IR_ABLATE_SILENCE").is_ok_and(|value| {
            value
                .split(',')
                .map(str::trim)
                .any(|id| id == "all" || id == "product-realm")
        })
    })
}

/// Appends the realm's census line to the file `SOLID_CHECKER_PRODUCT_REALM_CENSUS` names (a directory gets one `realm-<pid>.jsonl` per process): a measurement,
/// internal, changes no verdict. Unset, it costs one environment read per check.
pub(crate) fn write_census(realm: &Realm, index: &PackageReachIndex, directory: &Path) {
    use std::io::Write as _;
    let Some(target) =
        std::env::var_os("SOLID_CHECKER_PRODUCT_REALM_CENSUS").filter(|t| !t.is_empty())
    else {
        return;
    };
    let mut path = PathBuf::from(target);
    if path.is_dir() {
        path = path.join(format!("realm-{}.jsonl", std::process::id()));
    }
    let line = census(realm, index, &directory.to_string_lossy());
    if let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{line}");
    }
}

/// The census of one program's realm: one JSON object.
pub(crate) fn census(realm: &Realm, index: &PackageReachIndex, project: &str) -> serde_json::Value {
    match realm {
        Realm::Unknown(blockers) => serde_json::json!({
            "productRealm": {
                "project": project,
                "status": "unknown",
                "reason": blockers.iter().find(|blocker| !blocker.assumed).map(|blocker| blocker.reason.as_str()),
                "blockers": blockers.iter().map(|blocker| serde_json::json!({ "stage": blocker.stage, "reason": blocker.reason, "assumed": blocker.assumed })).collect::<Vec<_>>(),
            },
        }),
        Realm::Known(known) => {
            let product = index.product.as_ref();
            serde_json::json!({
                "productRealm": {
                    "project": project,
                    "status": "known",
                    "entries": known.entries.iter().map(|entry| serde_json::json!({ "path": entry.path, "source": entry.source })).collect::<Vec<_>>(),
                    "files": known.files.len(),
                    "roots": known.roots.len(),
                    "development": known.development,
                    "assumed": known.assumed.iter().map(|blocker| serde_json::json!({ "stage": blocker.stage, "reason": blocker.reason })).collect::<Vec<_>>(),
                    "hostNames": known.host_names,
                    "writersAny": index.prototype_writers_any.len(),
                    "writersAnyInProduct": product.map_or(0, |product| product.prototype_writers_any.len()),
                    "namedWriters": index.prototype_writers.len(),
                    "namedWritersInProduct": product.map_or(0, |product| product.prototype_writers.len()),
                    "anyKept": product.map(|product| product.prototype_writers_any.iter().collect::<Vec<_>>()),
                    "namedKept": product.map(|product| product.prototype_writers.iter().map(|(name, roots)| (name.clone(), roots.iter().collect::<Vec<_>>())).collect::<BTreeMap<_, _>>()),
                },
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_asset_loads_nothing_and_a_worker_or_glob_is_a_hidden_load() {
        for asset in [
            "./app.css",
            "./logo.SVG",
            "./shader.frag?raw",
            "./x.ts?url",
            "@fontsource/inter/400.css",
        ] {
            assert!(is_asset(asset), "{asset}");
        }
        for module in [
            "./App",
            "./App.tsx",
            "solid-js",
            "./worker.ts?worker",
            "./data.json",
            "./x?foo",
        ] {
            assert!(!is_asset(module), "{module}");
        }
        assert_eq!(
            hidden_load("const m = import.meta.glob('./p/*.tsx');"),
            Some("import.meta.glob")
        );
        assert_eq!(
            hidden_load("new Worker(new URL('./w.ts', import.meta.url))"),
            Some("a worker")
        );
        assert_eq!(
            hidden_load("navigator.serviceWorker.register('/sw.js')"),
            Some("a service worker")
        );
        assert_eq!(
            hidden_load("const u = new URL('./a.wasm', import.meta.url);"),
            Some("new URL(…, import.meta.url)")
        );
        assert_eq!(hidden_load("console.log(import.meta.env.DEV)"), None);
        // An identifier that merely contains the word is no worker.
        assert_eq!(
            hidden_load(
                "function decodeWorker(entry: unknown): WorkerRow { return registerAppServiceWorker(); }"
            ),
            None
        );
        assert_eq!(
            hidden_load("const W = Worker; new W(url);"),
            Some("a worker")
        );
    }

    #[test]
    fn exports_targets_skip_types_and_refuse_a_pattern() {
        let mut targets = Vec::new();
        let value = serde_json::json!({
            ".": { "types": "./dist/index.d.ts", "import": "./src/index.ts", "default": "./src/index.ts" },
            "./extra": "./src/extra.ts"
        });
        exports_targets(&value, &mut targets).unwrap();
        targets.sort();
        assert_eq!(
            targets,
            ["./src/extra.ts", "./src/index.ts", "./src/index.ts"]
        );
        let pattern = serde_json::json!({ "./*": "./src/*.ts" });
        assert!(exports_targets(&pattern, &mut Vec::new()).is_err());
    }

    #[test]
    fn the_packages_a_config_imports_are_named_whole_for_a_scope_and_without_a_subpath() {
        let source = r#"
            import { defineConfig } from "vite";
            import solid from 'vite-plugin-solid';
            import tw from "@tailwindcss/vite";
            import paths from "vite-tsconfig-paths/esm";
            import { join } from "node:path";
            import local from "./local";
        "#;
        let names = imported_packages(source);
        assert_eq!(
            names.into_iter().collect::<Vec<_>>(),
            [
                "@tailwindcss/vite",
                "vite",
                "vite-plugin-solid",
                "vite-tsconfig-paths"
            ]
        );
    }

    /// A fresh directory that is its own workspace root (a `.git` marker), so a chain read from a project inside it stops there.
    fn scratch(name: &str) -> PathBuf {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "product-realm-{name}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join(".git")).unwrap();
        fs::canonicalize(root).unwrap()
    }

    fn write(directory: &Path, relative: &str, text: &str) {
        let path = directory.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    const PAGE: &str = "<!doctype html><html><head><meta charset=\"utf-8\"><title>t</title></head><body><div id=\"root\"></div><script type=\"module\" src=\"/src/main.tsx\"></script></body></html>";

    fn program(directory: &Path, files: &[&str]) -> BTreeSet<String> {
        files
            .iter()
            .map(|file| program_path(&directory.join(file)))
            .collect()
    }

    #[test]
    fn a_page_and_a_package_name_the_entries_and_a_pattern_or_a_missing_file_end_the_read() {
        let root = scratch("entries");
        write(&root, "index.html", PAGE);
        write(&root, "src/main.tsx", "export {};");
        write(&root, "src/index.ts", "export {};");
        write(
            &root,
            "package.json",
            r#"{ "main": "src/index.ts", "exports": { ".": { "types": "./dist/index.d.ts", "import": "./src/index.ts" } } }"#,
        );
        let files = program(&root, &["src/main.tsx", "src/index.ts"]);
        let entries = entries_of(&root, &|path| files.contains(path)).unwrap();
        let paths = entries
            .iter()
            .map(|entry| {
                (
                    entry.path.rsplit('/').next().unwrap(),
                    entry.source.as_str(),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(paths, [("main.tsx", "html"), ("index.ts", "package")]);

        // An entry the program does not hold.
        let missing = entries_of(&root, &|path| path.ends_with("main.tsx")).unwrap_err();
        assert!(
            missing.contains("is not a file of the program"),
            "{missing}"
        );

        // An export pattern names files this reader cannot list.
        write(
            &root,
            "package.json",
            r#"{ "exports": { "./*": "./src/*.ts" } }"#,
        );
        let pattern = entries_of(&root, &|path| files.contains(path)).unwrap_err();
        assert!(pattern.contains("pattern"), "{pattern}");

        // A style sheet or a data file an entry names loads no module.
        write(
            &root,
            "package.json",
            r#"{ "exports": { ".": "./src/index.ts", "./styles.css": "./styles/index.css", "./package.json": "./package.json" } }"#,
        );
        let skipped = entries_of(&root, &|path| files.contains(path)).unwrap();
        assert_eq!(skipped.len(), 2, "{skipped:?}");
    }

    #[test]
    fn a_project_with_neither_a_page_nor_a_package_entry_has_no_entry_set() {
        let root = scratch("no-entries");
        write(&root, "package.json", r#"{ "private": true }"#);
        write(&root, "src/main.tsx", "export {};");
        let none = entries_of(&root, &|_| true).unwrap_err();
        assert!(none.contains("neither an index.html"), "{none}");
    }

    #[test]
    fn a_page_search_skips_installed_code_and_build_output_and_finds_any_other_page() {
        let root = scratch("pages");
        write(&root, "index.html", PAGE);
        write(&root, "node_modules/pkg/index.html", "<html></html>");
        write(&root, "dist/index.html", "<html></html>");
        write(&root, "src/main.tsx", "export {};");
        assert_eq!(other_page(&root).unwrap(), None);
        // A static file loads nothing.
        write(
            &root,
            "public/google123.html",
            "google-site-verification: google123.html",
        );
        assert_eq!(other_page(&root).unwrap(), None);
        write(
            &root,
            "docs/guide.HTML",
            "<html><body><script type=\"module\" src=\"/src/main.tsx\"></script></body></html>",
        );
        assert_eq!(
            other_page(&root).unwrap().unwrap().file_name().unwrap(),
            "guide.HTML"
        );
        write(
            &root,
            "docs/guide.HTML",
            "<html><body><button onClick=\"go()\">x</button></body></html>",
        );
        assert!(other_page(&root).unwrap().is_some());
    }

    #[test]
    fn a_vite_config_with_inert_plugins_is_read_and_anything_else_in_the_chain_is_unknown() {
        let root = scratch("bundler");
        let project = root.join("app");
        write(&project, "package.json", r#"{ "private": true }"#);
        write(
            &project,
            "vite.config.ts",
            "import solid from 'vite-plugin-solid'; export default { plugins: [solid()] };",
        );
        let names = bundler(&project).unwrap();
        assert!(
            names.contains("vite") && names.contains("vite-plugin-solid"),
            "{names:?}"
        );

        // A plugin the grammar admits but that adds a load (a service worker) is not inert.
        write(
            &project,
            "vite.config.ts",
            "import { VitePWA } from 'vite-plugin-pwa'; export default { plugins: [VitePWA()] };",
        );
        let pwa = bundler(&project).unwrap_err();
        assert!(pwa.contains("vite-plugin-pwa"), "{pwa}");

        // A plugin the grammar does not know refuses the config.
        write(
            &project,
            "vite.config.ts",
            "import other from 'some-plugin'; export default { plugins: [other()] };",
        );
        assert!(bundler(&project).is_err());

        // Another bundler's configuration, beside the project or above it, is not read.
        write(
            &project,
            "vite.config.ts",
            "export default { plugins: [] };",
        );
        assert!(bundler(&project).is_ok());
        write(&root, "webpack.config.js", "module.exports = {};");
        let webpack = bundler(&project).unwrap_err();
        assert!(webpack.contains("webpack.config"), "{webpack}");
        fs::remove_file(root.join("webpack.config.js")).unwrap();

        // Two configs of one directory: which one the tool picks is not read.
        write(
            &project,
            "vite.config.js",
            "export default { plugins: [] };",
        );
        let two = bundler(&project).unwrap_err();
        assert!(two.contains("more than one Vite config"), "{two}");
    }

    #[test]
    fn the_realm_grammar_admits_a_bundlers_output_naming_and_the_host_grammar_does_not() {
        let config = |options: &str| {
            format!(
                "import {{ defineConfig }} from 'vite'; import solid from 'vite-plugin-solid'; \
                 export default defineConfig({{ plugins: [solid()], base: './', build: {{ outDir: 'dist', rolldownOptions: {options} }} }});"
            )
        };
        let naming = config(
            "{ output: { chunkFileNames(chunk) { return 'a-[hash].js'; }, assetFileNames: 'b' } }",
        );
        let path = Path::new("vite.config.ts");
        assert!(crate::host_config::parse_realm(path, &naming).is_ok());
        assert!(crate::host_config::parse_detailed(path, &naming).is_err());
        // An input, a plugin list, an external, a banner or a computed key may change what a page loads or what the file runs.
        for options in [
            "{ input: 'other.html' }",
            "{ plugins: [] }",
            "{ external: ['a'] }",
            "{ output: { banner: 'import \"x\"' } }",
            "{ output: { ['chunkFileNames']() {} } }",
            "{ ...base }",
        ] {
            let refused = config(options);
            assert!(
                crate::host_config::parse_realm(path, &refused).is_err(),
                "{options}"
            );
        }
    }
}
