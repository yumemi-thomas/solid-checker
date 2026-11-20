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
//! - the pages: any other HTML file of the project that can run code may share a module with the page, so it ends the read. The directories listed are the project's
//!   own (`listable`: the project directory and the directories up to a workspace root), never an ancestor above them, which the owned (WASM) host's input store
//!   faults on where it holds a FIFO, a socket or a device; a listing that cannot be read or holds such an entry ends the read too (`listing`);
//! - the bundler: some `vite.config.*` from the project directory up to its workspace root must name the build, and every one there must parse in the closed Vite
//!   config grammar of host discovery (`host_config`, with the bundler's output naming options added) with no plugin but Solid's. The Solid plugin is the dialect's
//!   compile step, and where the realm would narrow anything its installed closure must be one of ADR 0270's pins (a missing or unpinned plugin ends the read);
//!   no other plugin is trusted for what it is called, and none is read for what it does, so it ends the read. The config helper `defineConfig` only returns its
//!   argument (Vite's documented typing helper; its installed bytes are not read). A Vite alias, a deduplicated package, the Solid plugin's start mode, any other
//!   bundler's configuration file in that chain, another Vite config file (`vite.config.prod.ts`), a second Vite config of one directory, and a `package.json`
//!   script that gives `vite` a `--config` or a `--mode` end it too;
//! - the resolution: the closure follows TypeScript's attested resolution and the page is built by Vite's, so the realm holds only where they agree. A relative
//!   specifier is probed the way Vite does (its own `resolve.extensions` order, the `.js` to `.ts` swap, `index.*`) and must pick the file the attestation names; a
//!   `#` specifier (package `imports`, read under other conditions by each), a root-absolute one, a tsconfig `customConditions`, `rootDirs` or `moduleSuffixes`, and
//!   `paths` or `baseUrl` that Vite does not apply (no `resolve.tsconfigPaths`) end the read; a query on a script module or a package (`?url`, `?raw`, `?inline`: the
//!   page may load or run what the string holds) ends it, and only a data file's query is no load;
//! - the closure of the entries over every `import`, `export … from`, literal `import()` and `require`, and `import x = require()`, by the attested resolution of each
//!   specifier. A load by a computed specifier, `import.meta.glob`, a worker, a service worker, `new URL(…, import.meta.url)`, a specifier no row attests or that
//!   resolves nowhere, a resolved file the program does not analyze (an asset and a JSON module excepted), and an entry the program does not hold end it;
//! - what the loaded packages load that no entry names, by manifest: the reach index does not walk Solid's own packages (`@solidjs/*`, the dialect's), so a writer only
//!   such a package's dependencies load is no writer of its root, and the page loads it all the same. The realm holds every package the manifests of the loaded
//!   packages depend on (`dependencies`, `optionalDependencies` and `peerDependencies`, transitively: [`manifest_closure`]), those of the Solid runtime the plugin's
//!   compile output imports (`solid-js`, `@solidjs/web`, `@solidjs/signals`) and, where the Solid plugin's dev-serve diagnostics bridge may be on, those of the app's own
//!   `@solidjs/diagnostics` (the bridge injects a module that imports it into `index.html`): the bridge is off only for `diagnostics: false` with no manifest of the
//!   chain declaring the package;
//! - the declared host: a browser target with a declared build. Vite itself puts code in the page (the dev server's client in a development build, the
//!   `modulepreload` polyfill in a production one), so the packages named `vite` and the config's own imports are in the realm whatever the files import, in either
//!   build (a name kept is a writer kept: the direction that only narrows the change).
//!
//! Premises (`docs/decisions.md`, ADR 1145): the closure equals Vite's resolution wherever the read has not ended (the refusals above are where it can differ); the
//! Solid plugin, an installed and pinned closure, adds only Solid's own loads and its diagnostics bridge's (held, above); `defineConfig` is the identity; the build is the Vite build the config and the scripts
//! describe; tests, stories and configs are not the product (the owner's ruling), so a product file's site is judged against what the page loads though a test loads
//! the file with more; code outside the program shares the page with it as the package integrity premise (ADR 0300) already assumes.

use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::fs;
use std::path::{Component, Path, PathBuf};

use solid_facts::ast::ModuleHazardKind;
use solid_facts::core::Span;
use solid_facts::package_reach::{PackageReachIndex, ProductWriters, package_root_of};
use solid_facts::resolution::{ImportResolution, SpecifierAttestation};
use solid_facts::{FileFacts, ProjectFacts};
use solid_reactive_ir::{RuntimeEnvironment, RuntimeTarget};

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
}

impl Blocker {
    fn new(stage: &'static str, reason: impl Into<String>) -> Self {
        Self {
            stage,
            reason: reason.into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Known {
    pub entries: Vec<Entry>,
    /// The program's files the entries load.
    pub files: BTreeSet<String>,
    /// The installed package roots the files load, each as the resolver gave it (a symlink path where there is one, the real path too).
    pub roots: BTreeSet<String>,
    /// The package names whose roots the page's host loads whatever the files import: Vite itself and the packages the Vite configs import.
    pub host_names: BTreeSet<String>,
    /// The names of the packages the loaded packages may load by their manifests ([`manifest_closure`]): the reach index does not walk a Solid package (the dialect's,
    /// `@solidjs/*`), so no writer its dependencies hold is attributed to it, and the page loads them all the same.
    pub manifest_names: BTreeSet<String>,
    /// Each Solid plugin the configs import, by the directory of the config and the specifier: where the realm narrows anything, its installed closure is audited
    /// against ADR 0270's pins ([`apply`]).
    pub solid_plugins: Vec<(PathBuf, String)>,
}

/// What [`apply`] did with a realm.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Applied {
    /// No realm is decided: every site keeps every writer.
    Unknown,
    /// The realm holds every writer the program has: nothing changes.
    Unchanged,
    /// The realm holds fewer writers: the files its entries load are judged against them.
    Narrowed,
    /// The realm would narrow, and a Solid plugin it names is not an installed closure ADR 0270 pins: every site keeps every writer.
    Refused(String),
}

/// The Solid plugin, by the specifier a config imports it from: the dialect's compile step. Its output loads Solid's own packages (the runtime, and in a development
/// build the refresh runtime), which are the dialect's and write no prototype; host discovery reads the installed closure of the same two names against its pins
/// (ADR 0270). No other plugin is trusted for its name (invariant 3): one whose installed source were read and proved to add no load (no `resolveId`, `load`,
/// `transform` or `transformIndexHtml` that adds an import, no `config` hook that adds an entry) would be allowed, and no such reading is built, so any other plugin
/// leaves the realm unknown.
const SOLID_PLUGINS: [&str; 2] = ["@solidjs/vite-plugin", "vite-plugin-solid"];

/// The config helper: `defineConfig` returns its argument, in Vite and in Vitest (the sources the closed grammar admits it from). It is no plugin and runs in Node.
const CONFIG_HELPERS: [&str; 2] = ["vite", "vitest/config"];

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
pub(crate) fn read(
    directory: &Path,
    project: &Path,
    facts: &ProjectFacts,
    runtime: &RuntimeEnvironment,
) -> Realm {
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
    let bundled = match bundler(directory) {
        Ok(bundled) => bundled,
        Err(reason) => {
            blockers.push(Blocker::new("bundler", reason));
            Bundled::default()
        }
    };
    // The user's tsconfig (`project`; a combined dependency-source program is analyzed at a logical path beside it, which extends it).
    let tsconfig = if project.is_dir() {
        project.join("tsconfig.json")
    } else {
        project.to_owned()
    };
    if let Err(reason) = tsconfig_resolution(&tsconfig, bundled.applies_paths) {
        blockers.push(Blocker::new("resolution", reason));
    }
    let entries = match entries_of(directory, &|path| by_path.contains_key(path)) {
        Ok(entries) => entries,
        Err(reason) => {
            blockers.push(Blocker::new("entries", reason));
            return Realm::Unknown(blockers);
        }
    };
    let (files, roots) = match closure(facts, &by_path, &entries, bundled.applies_paths) {
        Ok(closure) => closure,
        Err(reason) => {
            blockers.push(Blocker::new("closure", reason));
            return Realm::Unknown(blockers);
        }
    };
    // What the page may load that the closure does not name, by manifests: the dependencies of each package the files load, of the diagnostics package where the
    // Solid plugin's bridge may be on, and of the runtime the plugin's compile output imports.
    let mut starts = roots.iter().map(PathBuf::from).collect::<Vec<_>>();
    let mut loaded_by_plugin = Vec::new();
    if bundled.host_names.contains(DIAGNOSTICS_PACKAGE) {
        loaded_by_plugin.push(DIAGNOSTICS_PACKAGE);
    }
    if !bundled.solid_plugins.is_empty() {
        loaded_by_plugin.extend(["solid-js", "@solidjs/web", "@solidjs/signals"]);
    }
    for name in loaded_by_plugin {
        starts.extend(
            chain(directory)
                .iter()
                .filter_map(|dir| installed_package(dir, name)),
        );
    }
    let manifest_names = match manifest_closure(starts, MANIFEST_LIMIT) {
        Ok(names) => names,
        Err(reason) => {
            blockers.push(Blocker::new("closure", reason));
            return Realm::Unknown(blockers);
        }
    };
    if build.is_none() || !blockers.is_empty() {
        return Realm::Unknown(blockers);
    }
    Realm::Known(Known {
        entries,
        files,
        roots,
        host_names: bundled.host_names,
        manifest_names,
        solid_plugins: bundled.solid_plugins,
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

/// The directories a page search does not enter: installed code and VCS data (host discovery's own inventory skips the same two). A built output with a page that
/// can run code is a second page, so a project that has been built is unknown until its output is removed.
const PAGE_SEARCH_SKIPS: [&str; 2] = ["node_modules", ".git"];

/// The files a page search reads before it gives up: a tree this large is not read, and the realm is unknown.
const PAGE_SEARCH_LIMIT: usize = 50_000;

/// Whether the HTML file at `path` may run code in a page that shares modules with another: a script element, a dynamic `import(`, a `javascript:` URL or an inline
/// event handler. A static file (a site-verification token, an error page of markup alone) loads nothing. A file that cannot be read may.
///
/// An inline handler is an attribute name `on…` followed by `=`, compared in lower case (`ONFOCUS=`). HTML's tokenizer starts an attribute name after white space, a solidus
/// (`<body/onfocus=…>`) or a quote (`<a href='/'onclick=…>`), and this scan takes any byte that is no part of a longer name before the `on` for the start of one, so a
/// text that only looks like a handler ends the premise too (the fail-closed direction).
fn may_run_code(path: &Path) -> Result<bool, String> {
    let Ok(bytes) = fs::read(path) else {
        return Ok(true);
    };
    let text = String::from_utf8_lossy(&bytes).to_ascii_lowercase();
    Ok(text_may_run_code(&text))
}

/// [`may_run_code`] over the lower-cased text of a page.
fn text_may_run_code(text: &str) -> bool {
    if text.contains("<script") || text.contains("import(") || text.contains("javascript:") {
        return true;
    }
    let bytes = text.as_bytes();
    let part_of_a_name =
        |byte: u8| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b':' | b'.');
    for at in 0..bytes.len().saturating_sub(3) {
        if bytes[at] != b'o' || bytes[at + 1] != b'n' || (at > 0 && part_of_a_name(bytes[at - 1])) {
            continue;
        }
        let mut end = at + 2;
        while end < bytes.len() && bytes[end].is_ascii_lowercase() {
            end += 1;
        }
        while end < bytes.len() && bytes[end].is_ascii_whitespace() {
            end += 1;
        }
        if end > at + 2 && bytes.get(end) == Some(&b'=') {
            return true;
        }
    }
    false
}

/// The first HTML file under `directory` that is not its own `index.html`, or an error where the tree is too large to search or cannot be read.
fn other_page(directory: &Path) -> Result<Option<PathBuf>, String> {
    let own = directory.join("index.html");
    let mut pending = vec![directory.to_owned()];
    let mut seen = 0_usize;
    while let Some(dir) = pending.pop() {
        for (path, name, kind) in listing(&dir)? {
            seen += 1;
            if seen > PAGE_SEARCH_LIMIT {
                return Err(format!(
                    "{} holds too many files to look for a second HTML page",
                    directory.display()
                ));
            }
            if kind.is_dir() {
                if !PAGE_SEARCH_SKIPS.iter().any(|skip| name == *skip) {
                    pending.push(path);
                }
            } else if path.extension().is_some_and(|extension| {
                extension.eq_ignore_ascii_case("html") || extension.eq_ignore_ascii_case("htm")
            }) && path != own
            {
                // A link is read as what it names, and a read of anything but a regular file may block.
                if kind.is_symlink() && !fs::metadata(&path).is_ok_and(|target| target.is_file()) {
                    return Err(format!(
                        "{} is a link to something that is no regular file",
                        path.display()
                    ));
                }
                if may_run_code(&path)? {
                    return Ok(Some(path));
                }
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
/// Whether `dir` is the root of a workspace: it holds a `.git`, a `pnpm-workspace.yaml`, or a `package.json` that declares `workspaces`.
fn is_workspace_root(dir: &Path) -> bool {
    dir.join(".git").exists()
        || dir.join("pnpm-workspace.yaml").is_file()
        || fs::read_to_string(dir.join("package.json"))
            .ok()
            .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
            .is_some_and(|package| package.get("workspaces").is_some())
}

fn chain(directory: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for ancestor in directory.ancestors().take(8) {
        found.push(ancestor.to_owned());
        if is_workspace_root(ancestor) {
            break;
        }
    }
    found
}

/// The directories of [`chain`] whose listing is the realm's to read: the project directory and, where the chain reaches a workspace root, the directories up to
/// it. A chain that finds no root runs up through ancestors that are no part of the project (`/tmp`, a home directory), and such a directory may hold a FIFO, a
/// socket or a device, which the owned (WASM) host's input store faults on, and the fault poisons the checker: those directories are asked about by name
/// (`exists`, `is_file`) and never listed.
fn listable(directory: &Path) -> Vec<PathBuf> {
    let found = chain(directory);
    if found.last().is_some_and(|last| is_workspace_root(last)) {
        found
    } else {
        vec![directory.to_owned()]
    }
}

/// The entries of `dir`: path, name and kind. A listing that cannot be read, or holds an entry that is no regular file, directory or link (a FIFO, a socket, a
/// device), is no listing: the realm ends there, as an unobservable input does (a read of such an entry may block, and the owned host's store faults on it).
fn listing(dir: &Path) -> Result<Vec<(PathBuf, std::ffi::OsString, fs::FileType)>, String> {
    let entries = fs::read_dir(dir).map_err(|error| format!("{}: {error}", dir.display()))?;
    let mut found = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| format!("{}: {error}", dir.display()))?;
        let path = entry.path();
        let kind = entry
            .file_type()
            .map_err(|error| format!("{}: {error}", path.display()))?;
        if !(kind.is_file() || kind.is_dir() || kind.is_symlink()) {
            return Err(format!(
                "{} is not a regular file, directory or link",
                path.display()
            ));
        }
        found.push((path, entry.file_name(), kind));
    }
    Ok(found)
}

/// Whether `name` is the name of a Vite configuration file other than the conventional `vite.config.<ext>` (`vite.config.prod.ts`, `vite.prod.config.js`,
/// `app.vite.config.ts`): `vite build --config <file>` can select any of them, and which one a build uses is not read. `vitest.config.*` is a test runner's.
fn other_vite_config(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    if CONFIG_EXTENSIONS
        .iter()
        .any(|extension| name == format!("vite.config.{extension}"))
    {
        return false;
    }
    (name.starts_with("vite.") && name.contains("config"))
        || name.contains(".vite.config.")
        || name.starts_with("vite-config")
        || name.starts_with("vite_config")
        || name.starts_with("viteconfig")
}

/// The commands of a `package.json` script line, each a list of words (split at `&&`, `||`, `;`, `|` and `&`), or `None` for a line with quoting, expansion, grouping,
/// redirection, a glob or a continuation: what a `vite` in such a line runs, and where, is not read.
fn script_commands(line: &str) -> Option<Vec<Vec<&str>>> {
    if line.contains([
        '"', '\'', '`', '\\', '(', ')', '<', '>', '$', '{', '}', '*', '\n',
    ]) {
        return None;
    }
    Some(
        line.split(['&', ';', '|'])
            .map(|command| command.split_ascii_whitespace().collect::<Vec<_>>())
            .filter(|words| !words.is_empty())
            .collect(),
    )
}

/// Whether `word` names the `vite` executable (`vite`, `node_modules/.bin/vite`, `node_modules/vite/bin/vite.js`); `vitest` and `vite-node` are other programs.
fn is_vite(word: &str) -> bool {
    matches!(
        word.rsplit('/').next(),
        Some("vite" | "vite.js" | "vite.mjs" | "vite.cjs")
    )
}

/// Whether a script line names `vite` as a word, however it is quoted.
fn mentions_vite(line: &str) -> bool {
    line.split(|c: char| !(c.is_alphanumeric() || "-_./@".contains(c)))
        .any(is_vite)
}

/// The flags of `vite` that take their value as the next word.
const VITE_VALUE_FLAGS: [&str; 10] = [
    "--port",
    "--mode",
    "-m",
    "--base",
    "--logLevel",
    "-l",
    "--outDir",
    "--assetsDir",
    "--target",
    "--assetsInlineLimit",
];

/// The commands of `vite`; its first word may name one (`vite build`), and any other word that is no flag or flag value is the project root.
const VITE_COMMANDS: [&str; 5] = ["build", "dev", "serve", "preview", "optimize"];

/// The words that may precede `vite` in a command: runners and environment setters that run it where the script runs, with the options none of them has a directory
/// in (`pnpm --dir ../.. exec vite`, `npm --prefix web run`).
const VITE_LAUNCHERS: [&str; 12] = [
    "npx",
    "pnpx",
    "bunx",
    "pnpm",
    "npm",
    "yarn",
    "exec",
    "run",
    "dlx",
    "node",
    "env",
    "cross-env",
];
const VITE_LAUNCHER_FLAGS: [&str; 5] = ["--", "--bun", "--no-install", "-y", "--yes"];

/// Whether `word` is a host address (`localhost`, `0.0.0.0`, `::1`): the value of `--host`, as a project root is a directory name.
fn address_like(word: &str) -> bool {
    matches!(word, "localhost" | "true" | "false")
        || (word
            .chars()
            .all(|c| c.is_ascii_hexdigit() || c == '.' || c == ':')
            && word.contains(['.', ':']))
}

/// Why `words` (one command of a script) run `vite` other than as the plain `vite [build|dev|preview] [flags]` of the project directory, or `None`: a `--config` file
/// is another configuration than the one read, and a launcher option, a project root argument or a directory change run it in another directory or project, where
/// another configuration (and another `index.html`) is the build's. A `--mode` is none of these: it picks `.env` files and the branches of a function-form config,
/// and the closure is the union of those branches (the config grammar takes both) with both the development and the production page code of Vite in it.
fn vite_run_differs(words: &[&str]) -> Option<String> {
    #[derive(Clone, Copy, PartialEq)]
    enum Next {
        Any,
        Value,
        Host,
    }
    let at = words.iter().position(|word| is_vite(word))?;
    for word in &words[..at] {
        let assignment = word.split_once('=').is_some_and(|(name, _)| {
            !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        });
        if !assignment && !VITE_LAUNCHERS.contains(word) && !VITE_LAUNCHER_FLAGS.contains(word) {
            return Some(format!(
                "`{word}` before vite may run it in another directory or project"
            ));
        }
    }
    let mut next = Next::Any;
    for (index, word) in words[at + 1..].iter().enumerate() {
        let expected = std::mem::replace(&mut next, Next::Any);
        if word.starts_with('-') {
            let (name, has_value) = word
                .split_once('=')
                .map_or((*word, false), |(name, _)| (name, true));
            if matches!(name, "--config" | "-c") {
                return Some("it gives vite a --config file".to_owned());
            }
            if !has_value && VITE_VALUE_FLAGS.contains(&name) {
                next = Next::Value;
            } else if !has_value && name == "--host" {
                next = Next::Host;
            }
            continue;
        }
        let accepted = match expected {
            Next::Value => true,
            Next::Host => address_like(word),
            Next::Any => index == 0 && VITE_COMMANDS.contains(word),
        };
        if !accepted {
            return Some(format!("`{word}` after vite may be a project root"));
        }
    }
    None
}

/// Why the script `line` runs `vite` other than plainly in the project directory, or `None` (also for a line that does not run it).
fn script_run_differs(line: &str) -> Option<String> {
    if !mentions_vite(line) {
        return None;
    }
    let Some(commands) = script_commands(line) else {
        return Some("a script with quoting, expansion or grouping".to_owned());
    };
    if commands
        .iter()
        .any(|words| matches!(words.first(), Some(&("cd" | "pushd" | "popd"))))
    {
        return Some("it changes directory".to_owned());
    }
    commands.iter().find_map(|words| vite_run_differs(words))
}

/// The script runners whose commands run a script of a `package.json` by name (`npm run build`, `pnpm build`, `yarn build`, `bun run build`).
const SCRIPT_RUNNERS: [&str; 4] = ["npm", "pnpm", "yarn", "bun"];

/// Whether the commands of `line` run one of `names` through a script runner, and the words that follow the name (the runner hands them to the script).
fn runs_script_with(line: &str, names: &BTreeSet<String>) -> Option<bool> {
    let Some(commands) = script_commands(line) else {
        // A line this reader cannot split: it counts as running a script and forwarding arguments when it names a runner and one of the scripts.
        let words = line
            .split(|c: char| !(c.is_alphanumeric() || "-_.:@".contains(c)))
            .collect::<Vec<_>>();
        return (words.iter().any(|word| SCRIPT_RUNNERS.contains(word))
            && words.iter().any(|word| names.contains(*word)))
        .then_some(true);
    };
    let mut found = None;
    for words in commands {
        let Some(runner) = words.iter().position(|word| SCRIPT_RUNNERS.contains(word)) else {
            continue;
        };
        if let Some(at) = words[runner + 1..]
            .iter()
            .position(|word| names.contains(*word))
        {
            let forwarded = words.len() > runner + 1 + at + 1;
            found = Some(found.unwrap_or(false) || forwarded);
        }
    }
    found
}

/// The names of the scripts of one `package.json` that run `vite`, directly or by running another of them (`"ci": "npm run build"`).
fn scripts_running_vite(scripts: &serde_json::Map<String, serde_json::Value>) -> BTreeSet<String> {
    let mut names = BTreeSet::<String>::new();
    loop {
        let before = names.len();
        for (name, line) in scripts {
            let Some(line) = line.as_str() else { continue };
            if !names.contains(name)
                && (mentions_vite(line) || runs_script_with(line, &names).is_some())
            {
                names.insert(name.clone());
            }
        }
        if names.len() == before {
            return names;
        }
    }
}

/// The first `package.json` script of the chain whose run of `vite` is not the plain build of the project directory (see [`vite_run_differs`]): the build it runs
/// reads a configuration, or a project, that this reader does not.
fn script_selecting_vite_config(directory: &Path) -> Option<String> {
    for dir in chain(directory) {
        let Some(package) = fs::read_to_string(dir.join("package.json"))
            .ok()
            .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        else {
            continue;
        };
        let Some(scripts) = package
            .get("scripts")
            .and_then(serde_json::Value::as_object)
        else {
            continue;
        };
        let vite_scripts = scripts_running_vite(scripts);
        for (name, command) in scripts {
            let forwarded = |line: &str| {
                runs_script_with(line, &vite_scripts)
                    .filter(|forwarded| *forwarded)
                    .map(|_| "it forwards arguments to a script that runs vite".to_owned())
            };
            if let Some(why) = command
                .as_str()
                .and_then(|line| script_run_differs(line).or_else(|| forwarded(line)))
            {
                return Some(format!(
                    "{}: the script `{name}` runs vite other than plainly in the project directory: {why}",
                    dir.join("package.json").display()
                ));
            }
        }
    }
    None
}

/// The Vite configs of the chain, or why the chain is not read: another bundler's configuration, another Vite config file, or two Vite configs of one directory.
fn vite_configs(directory: &Path) -> Result<Vec<PathBuf>, String> {
    let mut configs = Vec::new();
    let listed = listable(directory);
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
        // Only the project's own directories are listed (`listable`): an ancestor above the workspace is not the realm's to read.
        if listed.contains(&dir) {
            for (path, name, _) in listing(&dir)? {
                if name.to_str().is_some_and(other_vite_config) {
                    return Err(format!(
                        "{} is another Vite configuration file: `vite --config` may select it",
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

/// What the Vite configs of the chain say, read.
#[derive(Debug, Default)]
struct Bundled {
    /// The package names the page's host loads whatever the files import: Vite and the packages the configs import.
    host_names: BTreeSet<String>,
    /// Each Solid plugin a config imports: the directory of the config and the specifier.
    solid_plugins: Vec<(PathBuf, String)>,
    /// Whether every config of the chain has Vite apply the project's tsconfig `paths` on every branch (`resolve.tsconfigPaths: true`): the build may read any of them.
    applies_paths: bool,
}

/// The package the Solid plugin's dev-serve diagnostics bridge loads into the page: the bridge is a virtual module, injected into the page's `index.html` when the plugin
/// finds `@solidjs/diagnostics` declared in the nearest manifest (or `diagnostics: true`), and it imports the app's own copy of the package.
const DIAGNOSTICS_PACKAGE: &str = "@solidjs/diagnostics";

/// Whether a `package.json` of the chain declares [`DIAGNOSTICS_PACKAGE`] (in `dependencies`, `devDependencies`, `optionalDependencies` or `peerDependencies`), or is
/// one this reader cannot parse (the plugin's own detection would read the nearest manifest, and which one is nearest depends on the root it runs in).
fn declares_diagnostics(directory: &Path) -> bool {
    chain(directory).iter().any(|dir| {
        let Ok(text) = fs::read_to_string(dir.join("package.json")) else {
            return false;
        };
        let Ok(package) = serde_json::from_str::<serde_json::Value>(&text) else {
            return true;
        };
        [
            "dependencies",
            "devDependencies",
            "optionalDependencies",
            "peerDependencies",
        ]
        .iter()
        .any(|field| {
            package
                .get(field)
                .and_then(serde_json::Value::as_object)
                .is_some_and(|names| names.contains_key(DIAGNOSTICS_PACKAGE))
        })
    })
}

/// Every bundler configuration of the chain, read, or why one ends the read. Some Vite config must name the build: a page and its modules with no configuration say
/// nothing of how they are built.
fn bundler(directory: &Path) -> Result<Bundled, String> {
    let configs = vite_configs(directory)?;
    if configs.is_empty() {
        return Err(format!(
            "no Vite configuration from {} up to its workspace root names the build",
            directory.display()
        ));
    }
    if let Some(script) = script_selecting_vite_config(directory) {
        return Err(script);
    }
    let mut bundled = Bundled {
        host_names: BTreeSet::from(["vite".to_owned()]),
        applies_paths: true,
        ..Bundled::default()
    };
    let mut diagnostics = crate::host_config::SolidDiagnostics::Off;
    for path in configs {
        let source =
            fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))?;
        let config = crate::host_config::parse_realm(&path, &source)
            .map_err(|refusal| format!("{}: {}", path.display(), refusal.reason))?;
        if config.mode != crate::host_config::EntryMode::Html {
            return Err(format!(
                "{}: the Solid plugin's start mode takes the framework's entries, not the page's",
                path.display()
            ));
        }
        // A Vite-only alias or a deduplicated package may send a specifier elsewhere than the resolution the closure follows.
        if !config.aliases.is_empty() || config.package_dedupe {
            return Err(format!(
                "{}: a Vite alias or a deduplicated package may redirect a specifier after the resolution the closure follows",
                path.display()
            ));
        }
        if let Some(plugin) = config.plugins.iter().find(|plugin| {
            !CONFIG_HELPERS.contains(&plugin.as_str()) && !SOLID_PLUGINS.contains(&plugin.as_str())
        }) {
            return Err(format!(
                "{}: the plugin {plugin} is not Solid's and no reading of its installed source proves it adds no load",
                path.display()
            ));
        }
        for plugin in &config.plugins {
            if SOLID_PLUGINS.contains(&plugin.as_str()) {
                bundled.solid_plugins.push((
                    path.parent().unwrap_or(directory).to_owned(),
                    plugin.clone(),
                ));
            }
        }
        bundled.applies_paths &= config.tsconfig_paths_every;
        diagnostics = diagnostics.max(config.solid_diagnostics);
        bundled.host_names.extend(imported_packages(&source));
    }
    // The Solid plugin's dev-serve diagnostics bridge puts the app's own `@solidjs/diagnostics` in the page: a package no file of the entries need import. It is
    // off only where `diagnostics` is literally `false` and no manifest of the chain declares the package (the plugin's detection is of the declaration, and an
    // installed closure the pins cover may detect it another way): then a name kept is a writer kept, the direction that only narrows the change.
    if !bundled.solid_plugins.is_empty()
        && (diagnostics != crate::host_config::SolidDiagnostics::Off
            || declares_diagnostics(directory))
    {
        bundled.host_names.insert(DIAGNOSTICS_PACKAGE.to_owned());
    }
    Ok(bundled)
}

/// The tsconfig options that make TypeScript's resolution (the closure's) differ from Vite's. `customConditions`, `rootDirs` and `moduleSuffixes` have no Vite
/// counterpart; `paths` and `baseUrl` are Vite's only where a config applies them (`resolve.tsconfigPaths`), and then only for the `tsconfig.json` it discovers
/// beside the project.
fn tsconfig_resolution(project: &Path, vite_applies_paths: bool) -> Result<(), String> {
    let mut configs = vec![project.to_owned()];
    configs.extend(
        crate::host_config::tsconfig_extends_chain(project)
            .ok_or_else(|| format!("{}: its extends chain is not read", project.display()))?,
    );
    let mut paths = false;
    for config in &configs {
        let source =
            fs::read_to_string(config).map_err(|error| format!("{}: {error}", config.display()))?;
        let value = crate::host_config::jsonc(&source)
            .ok_or_else(|| format!("{}: not JSONC", config.display()))?;
        let Some(options) = value
            .get("compilerOptions")
            .and_then(serde_json::Value::as_object)
        else {
            continue;
        };
        if let Some(key) = ["customConditions", "rootDirs", "moduleSuffixes"]
            .iter()
            .find(|key| options.contains_key(**key))
        {
            return Err(format!(
                "{}: compilerOptions.{key} changes TypeScript's resolution, and Vite has no counterpart",
                config.display()
            ));
        }
        paths |= options.contains_key("paths") || options.contains_key("baseUrl");
    }
    if !paths {
        return Ok(());
    }
    if !vite_applies_paths {
        return Err(format!(
            "{}: tsconfig paths or baseUrl resolve specifiers that Vite does not (no resolve.tsconfigPaths)",
            project.display()
        ));
    }
    if project
        .file_name()
        .is_none_or(|name| name != "tsconfig.json")
    {
        return Err(format!(
            "{}: Vite applies the paths of the tsconfig.json it finds, and the project is another file",
            project.display()
        ));
    }
    Ok(())
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

/// What a load's specifier is to a bundler.
#[derive(Debug, Eq, PartialEq)]
enum Specifier {
    /// A data file (a style sheet, an image, a font): nothing of it evaluates as a module of the program.
    Asset,
    /// A module: the attested resolution decides what it loads.
    Module,
    /// A query on a script module or a package (`?url`, `?raw`, `?inline`, `?worker`): the page may load or run what the string or the URL holds, so no module is known.
    Refused(String),
}

fn is_asset(specifier: &str) -> bool {
    classify(specifier) == Specifier::Asset
}

/// The extensions of files a bundler evaluates as code: a query on one (`?url`, `?raw`, `?inline`) hands the page its URL or its text, which it can load or run.
const SCRIPT_EXTENSIONS: [&str; 9] = ["js", "jsx", "mjs", "cjs", "ts", "tsx", "mts", "cts", "wasm"];

fn classify(specifier: &str) -> Specifier {
    let (path, query) = specifier.split_once('?').unwrap_or((specifier, ""));
    let extension = path
        .rsplit_once('.')
        .map(|(_, extension)| extension.to_ascii_lowercase())
        .filter(|extension| !extension.contains('/'));
    if query.is_empty() {
        return if extension
            .as_deref()
            .is_some_and(|extension| ASSET_EXTENSIONS.contains(&extension))
        {
            Specifier::Asset
        } else {
            Specifier::Module
        };
    }
    // A data file's `?raw`, `?url` or `?inline` is its text or its URL (a shader, a style sheet, an image, a JSON file): no module. A script's or a package's, or a
    // query this reader does not know, may be what the page loads or runs.
    let bare = !path.starts_with('.') && !path.starts_with('/');
    let script = extension
        .as_deref()
        .is_none_or(|extension| SCRIPT_EXTENSIONS.contains(&extension));
    if !bare && !script && matches!(query, "raw" | "url" | "inline" | "no-inline") {
        Specifier::Asset
    } else {
        Specifier::Refused(format!(
            "the query ?{query} on {path} (a script module or a package, or a query this reader does not know)"
        ))
    }
}

/// Vite's default `resolve.extensions` (the grammar admits no other), in its order: it takes the first that exists, so `util.js` beside `util.ts` is the page's file
/// though TypeScript resolves `util.ts`.
const VITE_EXTENSIONS: [&str; 7] = [".mjs", ".js", ".mts", ".ts", ".jsx", ".tsx", ".json"];

fn appended(path: &Path, suffix: &str) -> PathBuf {
    let mut os = path.as_os_str().to_owned();
    os.push(suffix);
    PathBuf::from(os)
}

/// The file Vite's resolver takes for the relative specifier `specifier` written in a file of `directory` (its `tryCleanFsResolve`): the file itself; the TypeScript
/// source a `.js`, `.jsx`, `.mjs` or `.cjs` emit names where no such file exists; the first of [`VITE_EXTENSIONS`] appended; the first `index` of them in a directory. A
/// directory with a `package.json` is read through its fields, which this does not follow: an error. `Ok(None)` is no file.
fn vite_pick(directory: &Path, specifier: &str) -> Result<Option<PathBuf>, String> {
    let base = directory.join(specifier);
    if base.is_file() {
        return Ok(Some(base));
    }
    if let Some(sources) = base
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| match extension {
            "js" => &["ts", "tsx"][..],
            "jsx" => &["tsx"][..],
            "mjs" => &["mts"][..],
            "cjs" => &["cts"][..],
            _ => &[][..],
        })
    {
        for source in sources {
            let candidate = base.with_extension(source);
            if candidate.is_file() {
                return Ok(Some(candidate));
            }
        }
    }
    for extension in VITE_EXTENSIONS {
        let candidate = appended(&base, extension);
        if candidate.is_file() {
            return Ok(Some(candidate));
        }
    }
    if base.is_dir() {
        if base.join("package.json").exists() {
            return Err(format!(
                "{} is a directory with a package.json, whose fields Vite reads",
                base.display()
            ));
        }
        for extension in VITE_EXTENSIONS {
            let candidate = base.join(format!("index{extension}"));
            if candidate.is_file() {
                return Ok(Some(candidate));
            }
        }
    }
    Ok(None)
}

fn same_file(left: &Path, right: &Path) -> bool {
    matches!((fs::canonicalize(left), fs::canonicalize(right)), (Ok(left), Ok(right)) if left == right)
}

/// Whether Vite resolves the relative `specifier` written in `importer` to the file the closure follows (`resolved`, the attested resolution): the premise that the
/// closure equals Vite's resolution, checked where it can fail on the files of the project.
fn vite_agrees(importer: &str, specifier: &str, resolved: &str) -> Result<(), String> {
    let relative = specifier == "."
        || specifier == ".."
        || specifier.starts_with("./")
        || specifier.starts_with("../");
    if !relative {
        return Ok(());
    }
    let directory = Path::new(importer)
        .parent()
        .ok_or_else(|| format!("{importer} has no directory"))?;
    match vite_pick(directory, specifier)? {
        Some(picked) if same_file(&picked, Path::new(resolved)) => Ok(()),
        Some(picked) => Err(format!(
            "Vite resolves {specifier} to {}, and TypeScript's attested resolution (the closure's) is {resolved}",
            program_path(&picked)
        )),
        None => Err(format!(
            "Vite resolves {specifier} to no file, and TypeScript's attested resolution (the closure's) is {resolved}"
        )),
    }
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
    vite_applies_paths: bool,
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
            match classify(specifier) {
                Specifier::Asset => continue,
                Specifier::Refused(why) => return Err(format!("{path}: {why}")),
                Specifier::Module => {}
            }
            // Package `imports` are read under each tool's own conditions, and a root-absolute specifier is Vite's project-relative path and TypeScript's filesystem
            // one: no row says which file the page loads.
            if specifier.starts_with('#') || specifier.starts_with('/') {
                return Err(format!(
                    "{path}: {specifier} is read differently by TypeScript and Vite (package imports, a root-absolute path)"
                ));
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
                // A bare specifier that resolves to a file of the project went through tsconfig `paths` or `baseUrl` (or a package's self reference), which Vite applies
                // only where a config says so; a relative one must be the file Vite picks.
                if !specifier.starts_with('.') && !vite_applies_paths {
                    return Err(format!(
                        "{path}: {specifier} resolves to the project file {resolved} by a mapping Vite does not apply"
                    ));
                }
                vite_agrees(&path, specifier, resolved).map_err(|why| format!("{path}: {why}"))?;
                queue.push_back(resolved.to_owned());
            } else if resolved.ends_with(".json") {
                // Data: a JSON module loads nothing, where it is the file Vite picks (`data.js` beside `data.json` is a script it would take first).
                vite_agrees(&path, specifier, resolved).map_err(|why| format!("{path}: {why}"))?;
            } else {
                return Err(format!(
                    "{path}: {specifier} resolves to {resolved}, which the program does not analyze"
                ));
            }
        }
    }
    Ok((files, roots))
}

/// The directory of the installed copy of `name` as Node finds it from `from`: the `node_modules/<name>` of `from` or of its nearest ancestor that holds one.
fn installed_package(from: &Path, name: &str) -> Option<PathBuf> {
    from.ancestors()
        .map(|ancestor| ancestor.join("node_modules").join(name))
        .find(|candidate| candidate.join("package.json").is_file())
}

/// How many packages [`manifest_closure`] may visit before the read ends (a closure this large is not a page's).
const MANIFEST_LIMIT: usize = 20_000;

/// The names of the packages in `starts` and of every package their manifests may load: the keys of `dependencies`, `optionalDependencies` and `peerDependencies`
/// (a name counts where no copy is installed), and recursively those of each installed copy, found from the directory of the package that names it. A superset of
/// what a package's closure loads, and of what the reach index attributes to it: the index does not walk a Solid package (the dialect's: `@solidjs/*`), so a writer
/// only such a package's dependencies load is no writer of its root, and the manifests are the one place the page's load of it shows. More than `limit` packages
/// end the read.
fn manifest_closure(
    starts: impl IntoIterator<Item = PathBuf>,
    limit: usize,
) -> Result<BTreeSet<String>, String> {
    let mut names = BTreeSet::new();
    let mut seen = BTreeSet::<PathBuf>::new();
    let mut queue = starts.into_iter().collect::<Vec<_>>();
    while let Some(dir) = queue.pop() {
        let Ok(dir) = fs::canonicalize(&dir) else {
            continue;
        };
        if !seen.insert(dir.clone()) {
            continue;
        }
        if seen.len() > limit {
            return Err(format!(
                "the manifests of the loaded packages name more than {limit} packages"
            ));
        }
        let Some(manifest) = fs::read_to_string(dir.join("package.json"))
            .ok()
            .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        else {
            continue;
        };
        if let Some(name) = manifest.get("name").and_then(serde_json::Value::as_str) {
            names.insert(name.to_owned());
        }
        for field in ["dependencies", "optionalDependencies", "peerDependencies"] {
            let Some(dependencies) = manifest.get(field).and_then(serde_json::Value::as_object)
            else {
                continue;
            };
            for dependency in dependencies.keys() {
                names.insert(dependency.clone());
                queue.extend(installed_package(&dir, dependency));
            }
        }
    }
    Ok(names)
}

/// The real directory of an installed root, or `None` where it cannot be resolved (then the root counts as loaded).
fn real(root: &str) -> Option<PathBuf> {
    fs::canonicalize(root).ok()
}

/// The stamp of a path: its identity and times, or `None` where nothing is there. A kept answer is current while every stamp is the one it was made under.
#[cfg(unix)]
type Stamp = Option<(u64, u64, u64, i64, i64, i64, i64)>;

#[cfg(unix)]
fn stamp(path: &Path) -> Stamp {
    use std::os::unix::fs::MetadataExt as _;
    let metadata = fs::symlink_metadata(path).ok()?;
    Some((
        metadata.dev(),
        metadata.ino(),
        metadata.len(),
        metadata.mtime(),
        metadata.mtime_nsec(),
        metadata.ctime(),
        metadata.ctime_nsec(),
    ))
}

/// Whether the installed closure of the Solid plugin `name`, imported from a config in `directory`, is one ADR 0270 pins ([`crate::host_plugins::audit`]: the
/// digest of every file of every package it loads, and the edges between them), else why not. The audit hashes the whole closure, which a workspace's optional
/// peers make large (seconds), so a retained daemon keeps the answer while the stamp of everything the audit read is unchanged, and a one-shot check asks once.
fn plugin_pinned(directory: &Path, name: &str) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::sync::{Mutex, OnceLock, PoisonError};
        type Kept = Mutex<HashMap<(PathBuf, String), (Vec<(PathBuf, Stamp)>, Result<(), String>)>>;
        static KEPT: OnceLock<Kept> = OnceLock::new();
        let kept = KEPT.get_or_init(Default::default);
        let key = (directory.to_owned(), name.to_owned());
        if let Some((stamps, answer)) = kept
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&key)
            && stamps
                .iter()
                .all(|(path, recorded)| stamp(path) == *recorded)
        {
            return answer.clone();
        }
        let started = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| i64::try_from(elapsed.as_secs()).unwrap_or(0));
        let mut inputs = Vec::new();
        let answer = crate::host_plugins::audit(
            directory,
            name,
            &mut inputs,
            &mut crate::host_plugins::PackageAudits::new(),
            None,
        )
        .map_err(|refusal| refusal.reason);
        inputs.sort();
        inputs.dedup();
        let stamps = inputs
            .into_iter()
            .map(|path| {
                let at = stamp(&path);
                (path, at)
            })
            .collect::<Vec<_>>();
        // Kept only where nothing it read changed in or after the second the audit began: a write during the read could be a digest of other bytes.
        if stamps
            .iter()
            .all(|(_, at)| at.is_none_or(|(_, _, _, _, _, ctime, _)| ctime < started))
        {
            kept.lock()
                .unwrap_or_else(PoisonError::into_inner)
                .insert(key, (stamps, answer.clone()));
        }
        answer
    }
    #[cfg(not(unix))]
    {
        crate::host_plugins::audit(
            directory,
            name,
            &mut Vec::new(),
            &mut crate::host_plugins::PackageAudits::new(),
            None,
        )
        .map_err(|refusal| refusal.reason)
    }
}

/// Sets `index.product` from a known realm: the files, and the writers of `index` the realm's packages (or the declared host) load. An unknown realm leaves the
/// index as it is, every site judged against every writer, and so does a realm that would narrow while a Solid plugin it names is not an installed, pinned closure.
pub(crate) fn apply(realm: &Realm, index: &mut PackageReachIndex) -> Applied {
    let Realm::Known(known) = realm else {
        return Applied::Unknown;
    };
    let loaded_real = known
        .roots
        .iter()
        .filter_map(|root| real(root))
        .collect::<BTreeSet<_>>();
    let host_roots = index
        .names
        .iter()
        .filter(|(_, name)| {
            known.host_names.contains(name.as_str()) || known.manifest_names.contains(name.as_str())
        })
        .map(|(root, _)| root.clone())
        .collect::<BTreeSet<_>>();
    let host_real = host_roots
        .iter()
        .filter_map(|root| real(root))
        .collect::<BTreeSet<_>>();
    // Each writer root's real directory, asked once.
    let real_of = index
        .prototype_writers
        .values()
        .flatten()
        .chain(index.prototype_writers_any.iter())
        .map(|root| (root.clone(), real(root)))
        .collect::<std::collections::HashMap<_, _>>();
    // A root counts as loaded by the realm when its spelling or its real directory is one the files load, the declared host loads it, or its directory cannot be
    // resolved (never a reason to drop a writer).
    let loaded = |root: &str| -> bool {
        if known.roots.contains(root) || host_roots.contains(root) {
            return true;
        }
        match real_of.get(root).and_then(Option::as_ref) {
            Some(real) => loaded_real.contains(real) || host_real.contains(real),
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
    let narrows = writers != index.prototype_writers || any != index.prototype_writers_any;
    // The Solid plugin is trusted for what the audited release does: asked only where the realm changes a site, as the audit is the dear part.
    if narrows {
        for (directory, name) in &known.solid_plugins {
            if let Err(reason) = plugin_pinned(directory, name) {
                return Applied::Refused(format!(
                    "the Solid plugin {name} imported from {}: {reason}",
                    directory.display()
                ));
            }
        }
    }
    index.product = Some(ProductWriters {
        files: known.files.clone(),
        prototype_writers: writers,
        prototype_writers_any: any,
        narrows,
    });
    if narrows {
        Applied::Narrowed
    } else {
        Applied::Unchanged
    }
}

/// Appends the realm's census line to the file `SOLID_CHECKER_PRODUCT_REALM_CENSUS` names (a directory gets one `realm-<pid>.jsonl` per process): a measurement,
/// internal, changes no verdict. Unset, it costs one environment read per check.
pub(crate) fn write_census(
    realm: &Realm,
    applied: &Applied,
    index: &PackageReachIndex,
    directory: &Path,
    host: Option<&Result<ReviewedHost, String>>,
) {
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
    let mut line = census(realm, applied, index, &directory.to_string_lossy());
    // ADR 1144: the reviewed host premise, where a package named `vite` writes a prototype.
    if let Some(object) = line["productRealm"].as_object_mut() {
        object.insert(
            "reviewedHost".to_owned(),
            match host {
                None => serde_json::Value::Null,
                Some(Ok(host)) => serde_json::json!({ "roots": host.roots, "defines": host.defines, "members": host.members }),
                Some(Err(reason)) => serde_json::json!({ "refused": reason }),
            },
        );
    }
    if let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{line}");
    }
}

/// The census of one program's realm: one JSON object.
pub(crate) fn census(
    realm: &Realm,
    applied: &Applied,
    index: &PackageReachIndex,
    project: &str,
) -> serde_json::Value {
    let applied = match applied {
        Applied::Unknown => "unknown".to_owned(),
        Applied::Unchanged => "unchanged".to_owned(),
        Applied::Narrowed => "narrowed".to_owned(),
        Applied::Refused(reason) => format!("refused: {reason}"),
    };
    match realm {
        Realm::Unknown(blockers) => serde_json::json!({
            "productRealm": {
                "project": project,
                "status": "unknown",
                "applied": applied,
                "reason": blockers.first().map(|blocker| blocker.reason.as_str()),
                "blockers": blockers.iter().map(|blocker| serde_json::json!({ "stage": blocker.stage, "reason": blocker.reason })).collect::<Vec<_>>(),
            },
        }),
        Realm::Known(known) => {
            let product = index.product.as_ref();
            serde_json::json!({
                "productRealm": {
                    "project": project,
                    "status": "known",
                    "applied": applied,
                    "entries": known.entries.iter().map(|entry| serde_json::json!({ "path": entry.path, "source": entry.source })).collect::<Vec<_>>(),
                    "files": known.files.len(),
                    "roots": known.roots.len(),
                    "hostNames": known.host_names,
                    "manifestNames": known.manifest_names.len(),
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

/// ADR 1144: one release of Vite's dev client whose bytes were read: the files the dev server serves into the page (`/@vite/client`, `/@vite/env`; the bundled-dev
/// client where Vite 8 has one) and its manifest, each pinned by the lower-case hex SHA-256 of the published bytes, with what the client writes on the page's
/// globals. Read on the published tarball (`vite-<version>.tgz`, sha512 integrity below) and run in Chrome (`scripts/probes/vite-dev-client`): beside the config's
/// `define` keys (`env.mjs`, [`DEFINE_KEY`]), the client writes no built-in's prototype and no global but those `global_writes` name.
pub(crate) struct ViteClient {
    /// The release; the premise id is `vite-dev-client-<version>`.
    pub version: &'static str,
    /// The tarball's integrity, as the lockfiles of the corpus installs record it (read by the table's test only).
    #[cfg_attr(not(test), allow(dead_code))]
    pub integrity: &'static str,
    /// Each pinned file below the package root and its digest.
    pub files: &'static [(&'static str, &'static str)],
    /// The globals whose own members the client replaces (`console`, whose `error` and `warn` Vite 8's console forwarding wraps: `member_writes`).
    pub global_writes: &'static [&'static str],
    /// The own members it writes, as (global, member).
    pub member_writes: &'static [(&'static str, &'static str)],
}

/// ADR 1144: every reviewed release of Vite's dev client. A version, a patch or a file nobody read is no match: the package stays the writer its closure says.
pub(crate) const VITE_CLIENTS: &[ViteClient] = &[
    ViteClient {
        version: "8.3.0",
        integrity: "sha512-lhZBVvEHefgE+HQZC9O7EBJgCU/nVzFNl7vkS4RE0APtWLP02/8QVIkQtzBxPquh7lq5/78NHipTj7ODQ6XuyQ==",
        files: &[
            (
                "package.json",
                "0a75f2254630e11aa4fde386a2f883e2acc8430acc3fb5a833f25da264c62a16",
            ),
            (
                "dist/client/client.mjs",
                "d5a2258b7915fc2e70077dd20b63892e553b1ef4344c95621e226875f8897962",
            ),
            (
                "dist/client/env.mjs",
                "599a7191be77f077087cde291f7906fa751a794617f42d914f54991fe7bf434f",
            ),
            (
                "dist/client/bundledDevClient.mjs",
                "2a75e5d4b1cc47796064b68b62e0ab1fb1f6802c6e7118759db565aefeb2a44f",
            ),
        ],
        // `setupForwardConsoleHandler`: `console[level] = (...args) => { original(...args); sendLog(level, args) }` for the forwarded levels (`error`, `warn` by
        // default; Chrome shows those two replaced).
        global_writes: &["console"],
        member_writes: &[("console", "error"), ("console", "warn")],
    },
    ViteClient {
        version: "7.3.1",
        integrity: "sha512-w+N7Hifpc3gRjZ63vYBXA56dvvRlNWRczTdmCBBa+CotUzAPf5b7YMdMR/8CQoeYE5LX3W4wj6RYTgonm1b9DA==",
        files: &[
            (
                "package.json",
                "116af0821317eea464b9e69afda7c68b70b1caf51abe3218001b0e2175d3bed6",
            ),
            (
                "dist/client/client.mjs",
                "adc8d1d9bab80984e8f7136074af8ee8e6f63bcf8ee75d8950577e4bd73af23a",
            ),
            (
                "dist/client/env.mjs",
                "98339370fa9a7f50c9debe091db27248cbb7280e29cc9770d214e6dfa7b6f73a",
            ),
        ],
        global_writes: &[],
        member_writes: &[],
    },
];

/// ADR 1144: the `define` keys a release of the Solid plugin adds to the config (read in its installed bytes; ADR 0270 pins the closure): Vite's client writes
/// each on the page's global object (`env.mjs`). `vite-plugin-solid` re-exports `@solidjs/vite-plugin`, so the release is the one that package resolves to. A
/// release not listed ends the premise.
pub(crate) const SOLID_PLUGIN_DEFINES: &[(&str, &[&str])] = &[
    // `config` returns `define: { [SERVER_COMPONENTS_DEFINE]: serverComponentsDefine }`, `JSON.stringify(serverComponents)` of a boolean (a user `define` of the key
    // would win, and the closed grammar admits none).
    ("3.0.0-next.51", &["__SOLID_SERVER_COMPONENTS__"]),
    // The fixture issuer's stand-in (`fixtures/reactive-ir/node_modules/@solidjs/vite-plugin`, pinned in host-plugin-pins.json): `solid()` returns no plugin.
    ("0.0.0-fixture", &[]),
];

/// ADR 1144 ruling (a): a `define` key the client may write without becoming a writer of any name: a data-only new global, `__NAME__` in upper case (no standard,
/// host or prototype binding is spelled so), with no further segment. Anything else (a prototype path, `Promise.x`, `window.x`, a key of another shape) keeps
/// the host a writer of any name.
fn define_key_admitted(key: &str) -> bool {
    key.len() > 4
        && key.starts_with("__")
        && key.ends_with("__")
        && key
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
        && key.as_bytes()[2].is_ascii_uppercase()
}

/// ADR 1144: a page's host that the premise holds for, by root.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct ReviewedHost {
    pub roots: BTreeMap<String, String>,
    pub defines: BTreeSet<String>,
    /// The own members the clients write (`console.error`), for the census.
    pub members: BTreeSet<String>,
}

/// ADR 1144: whether Vite's dev client premise holds for the program in `directory`: the roots of `vite` it holds for, `None` where no package named `vite` is a
/// writer of a built-in's prototype, else why not. It holds where
/// - the build is a development build in a browser (the dev server's client is the code Vite puts in that page; a production build's is not read);
/// - no program file loads `vite` or a `vite/*` subpath at run time (a type-only import loads nothing), and no load has a specifier the file does not spell;
/// - no installed package the program holds, other than Vite, depends on `vite` by its manifest (a test runner that loads Vite into its own process, `vitest`);
/// - every Vite config of the chain reads in the closed grammar, with no plugin but Solid's (an installed closure ADR 0270 pins, whose `define` keys
///   [`SOLID_PLUGIN_DEFINES`] lists, each [`define_key_admitted`]) and no `package.json` script that selects another config;
/// - each root named `vite` is the package the configs' directory resolves, and its installed bytes are a release of [`VITE_CLIENTS`];
/// - each global the client writes (its own and the defines') is already one the package's closure writes in the index (by name, or under any name), so every
///   site that reads it still meets a writer.
pub(crate) fn reviewed_host(
    directory: &Path,
    facts: &ProjectFacts,
    runtime: &RuntimeEnvironment,
    index: &PackageReachIndex,
) -> Option<Result<ReviewedHost, String>> {
    let vite_roots = index
        .names
        .iter()
        .filter(|(_, name)| name.as_str() == "vite")
        .map(|(root, _)| root.clone())
        .filter(|root| {
            index.prototype_writers_any.contains(root)
                || index
                    .prototype_writers
                    .values()
                    .any(|roots| roots.contains(root))
        })
        .collect::<BTreeSet<_>>();
    if vite_roots.is_empty() {
        return None;
    }
    Some(reviewed_host_roots(
        directory,
        facts,
        runtime,
        index,
        &vite_roots,
    ))
}

fn reviewed_host_roots(
    directory: &Path,
    facts: &ProjectFacts,
    runtime: &RuntimeEnvironment,
    index: &PackageReachIndex,
    vite_roots: &BTreeSet<String>,
) -> Result<ReviewedHost, String> {
    if runtime.target != Some(RuntimeTarget::Browser)
        || runtime.build != Some(solid_reactive_ir::RuntimeBuild::Development)
    {
        return Err("the declared runtime is not a development build in a browser".to_owned());
    }
    for file in &facts.files {
        if file
            .ast
            .module_loads
            .iter()
            .any(|load| load.specifier.is_none())
        {
            return Err(format!(
                "{} loads a module by a specifier it does not spell",
                file.path.as_str()
            ));
        }
        if let Some((_, specifier)) = loads(file)
            .into_iter()
            .find(|(_, specifier)| *specifier == "vite" || specifier.starts_with("vite/"))
        {
            return Err(format!(
                "{} loads {specifier} at run time",
                file.path.as_str()
            ));
        }
    }
    for (root, name) in &index.names {
        if name == "vite" {
            continue;
        }
        let Ok(text) = fs::read_to_string(Path::new(root).join("package.json")) else {
            return Err(format!("{root}: the manifest cannot be read"));
        };
        let Ok(manifest) = serde_json::from_str::<serde_json::Value>(&text) else {
            return Err(format!("{root}: the manifest does not parse"));
        };
        if ["dependencies", "optionalDependencies", "peerDependencies"]
            .iter()
            .any(|field| {
                manifest
                    .get(field)
                    .and_then(serde_json::Value::as_object)
                    .is_some_and(|names| names.contains_key("vite"))
            })
        {
            return Err(format!(
                "{name} depends on vite: it may load Vite's own code where the program runs"
            ));
        }
    }
    let configs = vite_configs(directory)?;
    if configs.is_empty() {
        return Err(format!(
            "no Vite configuration from {} up to its workspace root names the build",
            directory.display()
        ));
    }
    if let Some(script) = script_selecting_vite_config(directory) {
        return Err(script);
    }
    let mut defines = BTreeSet::new();
    let mut served = BTreeSet::new();
    for path in &configs {
        let config_dir = path.parent().unwrap_or(directory);
        let source =
            fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
        let config = crate::host_config::parse_realm(path, &source)
            .map_err(|refusal| format!("{}: {}", path.display(), refusal.reason))?;
        if config.mode != crate::host_config::EntryMode::Html {
            return Err(format!(
                "{}: the Solid plugin's start mode is not read",
                path.display()
            ));
        }
        for plugin in &config.plugins {
            if CONFIG_HELPERS.contains(&plugin.as_str()) {
                continue;
            }
            if !SOLID_PLUGINS.contains(&plugin.as_str()) {
                return Err(format!(
                    "{}: the plugin {plugin} is not Solid's, and its defines are not read",
                    path.display()
                ));
            }
            plugin_pinned(config_dir, plugin)?;
            let installed = installed_package(config_dir, plugin)
                .ok_or_else(|| format!("{}: {plugin} is not installed", path.display()))?;
            let core = if plugin == "@solidjs/vite-plugin" {
                installed
            } else {
                installed_package(&installed, "@solidjs/vite-plugin").ok_or_else(|| {
                    format!(
                        "{}: {plugin} resolves no @solidjs/vite-plugin",
                        path.display()
                    )
                })?
            };
            let version = fs::read(core.join("package.json"))
                .ok()
                .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
                .and_then(|manifest| manifest.get("version")?.as_str().map(str::to_owned))
                .ok_or_else(|| format!("{}: no version", core.display()))?;
            let (_, keys) = SOLID_PLUGIN_DEFINES
                .iter()
                .find(|(listed, _)| *listed == version)
                .ok_or_else(|| {
                    format!("@solidjs/vite-plugin {version}: its defines are not read")
                })?;
            for key in *keys {
                if !define_key_admitted(key) {
                    return Err(format!(
                        "@solidjs/vite-plugin {version} defines {key}, which may name an existing binding"
                    ));
                }
                defines.insert((*key).to_owned());
            }
        }
        served.insert(
            installed_package(config_dir, "vite")
                .and_then(|vite| fs::canonicalize(vite).ok())
                .ok_or_else(|| format!("{}: no vite is installed", path.display()))?,
        );
    }
    let mut roots = BTreeMap::new();
    let mut members = BTreeSet::new();
    for root in vite_roots {
        let real = real(root).ok_or_else(|| format!("{root} does not resolve"))?;
        if !served.contains(&real) {
            return Err(format!("{root} is not the vite the configs resolve"));
        }
        let client = VITE_CLIENTS
            .iter()
            .find(|client| {
                client.files.iter().all(|(file, sha256)| {
                    crate::reviewed_packages::digest_of(&real.join(file)).as_deref()
                        == Some(*sha256)
                })
            })
            .ok_or_else(|| {
                format!(
                    "{root}: no reviewed release of Vite's dev client matches the installed bytes"
                )
            })?;
        let covered = |name: &str| {
            index.global_writers_any.contains(root)
                || index
                    .global_writers
                    .get(name)
                    .is_some_and(|roots| roots.contains(root))
        };
        if let Some(name) = client
            .global_writes
            .iter()
            .copied()
            .chain(defines.iter().map(String::as_str))
            .find(|name| !covered(name))
        {
            return Err(format!(
                "{root}: the client writes {name}, which the package's facts do not hold"
            ));
        }
        members.extend(
            client
                .member_writes
                .iter()
                .map(|(global, member)| format!("{global}.{member}")),
        );
        roots.insert(root.clone(), format!("vite-dev-client-{}", client.version));
    }
    Ok(ReviewedHost {
        roots,
        defines,
        members,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ADR 1144: each row of [`VITE_CLIENTS`] is the published bytes of its release, as the fixtures copy them from the tarball (integrity in the row and in
    /// the fixture's README), and a release the table does not pin has no row.
    #[test]
    fn the_vite_client_rows_are_the_published_bytes_the_fixtures_copy() {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../fixtures/reactive-ir/vite-dev-client");
        let copies = [
            ("8.3.0", fixture.join("node_modules/vite")),
            ("7.3.1", fixture.join("variants/vite-7/node_modules/vite")),
        ];
        assert_eq!(VITE_CLIENTS.len(), copies.len());
        for (version, copy) in copies {
            let client = VITE_CLIENTS
                .iter()
                .find(|client| client.version == version)
                .unwrap();
            let manifest: serde_json::Value =
                serde_json::from_slice(&fs::read(copy.join("package.json")).unwrap()).unwrap();
            assert_eq!(manifest["version"], version);
            assert!(client.integrity.starts_with("sha512-"));
            for (file, sha256) in client.files {
                assert_eq!(
                    crate::reviewed_packages::digest_of(&copy.join(file)).as_deref(),
                    Some(*sha256),
                    "{version} {file}"
                );
            }
        }
        // The mismatch twin changes one pinned file: no row matches it.
        let patched = fixture.join("variants/mismatch/node_modules/vite");
        assert!(
            !VITE_CLIENTS
                .iter()
                .any(|client| client.files.iter().all(|(file, sha256)| {
                    crate::reviewed_packages::digest_of(&patched.join(file)).as_deref()
                        == Some(*sha256)
                }))
        );
    }

    /// ADR 1144: the rows match the corpus installs they were read from (`SOLID_CHECKER_VITE_CORPUS=<apps dir>` of the rc.21 corpus: helge-dev's vite 8.3.0
    /// and donegeon-client's 7.3.1). The corpus is not in the repository, so without the variable the test checks nothing.
    #[test]
    fn the_vite_client_rows_match_the_corpus_installs() {
        let Some(apps) = std::env::var_os("SOLID_CHECKER_VITE_CORPUS") else {
            return;
        };
        let apps = PathBuf::from(apps);
        for (version, install) in [
            ("8.3.0", apps.join("helge-dev/node_modules/vite")),
            ("7.3.1", apps.join("donegeon-client/web/node_modules/vite")),
        ] {
            let client = VITE_CLIENTS
                .iter()
                .find(|client| client.version == version)
                .unwrap();
            for (file, sha256) in client.files {
                assert_eq!(
                    crate::reviewed_packages::digest_of(&install.join(file)).as_deref(),
                    Some(*sha256),
                    "{version} {file}"
                );
            }
        }
    }

    /// ADR 1144 ruling (a): only a data-only new global passes; a prototype path, a standard or host global's member, and any other shape keep the host a writer.
    #[test]
    fn a_define_key_passes_only_as_a_new_upper_case_global() {
        assert!(define_key_admitted("__SOLID_SERVER_COMPONENTS__"));
        assert!(define_key_admitted("__APP_VERSION2__"));
        for refused in [
            "Array.prototype.x",
            "__proto__",
            "constructor",
            "prototype",
            "window.x",
            "globalThis.x",
            "Promise",
            "Object",
            "__x__",
            "__A__.b",
            "__A__.prototype",
            "____",
            "__2A__",
            "process.env.NODE_ENV",
        ] {
            assert!(!define_key_admitted(refused), "{refused}");
        }
        for (_, keys) in SOLID_PLUGIN_DEFINES {
            assert!(keys.iter().all(|key| define_key_admitted(key)));
        }
    }

    #[test]
    fn an_asset_loads_nothing_and_a_worker_or_glob_is_a_hidden_load() {
        for asset in [
            "./app.css",
            "./logo.SVG",
            "./shader.frag?raw",
            "@fontsource/inter/400.css",
        ] {
            assert!(is_asset(asset), "{asset}");
        }
        for module in [
            "./App",
            "./App.tsx",
            "solid-js",
            "./worker.ts?worker",
            "./x.ts?url",
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

    /// ADR 1154: the scan of another page reads an attribute name `on…` after white space, a solidus or a quote, in any case.
    #[test]
    fn a_page_scan_reads_an_inline_handler_after_a_solidus_a_quote_or_in_upper_case() {
        for text in [
            "<body onfocus=\"hook()\">",
            "<body/onfocus=\"hook()\">",
            "<body//onfocus=\"hook()\">",
            "<body / onfocus = \"hook()\">",
            "<div id=\"a\"/onclick=\"go()\">",
            "<a href='/'onclick='go()'>x</a>",
            "<BODY ONFOCUSIN=\"hook()\">",
            "<body\tonload=go()>",
            "<body\nonload=go()>",
            "<svg/onload=go()>",
            "<img src=x onerror=go()>",
            "<script src=\"/a.js\"></script>",
            "<script>go()</script>",
            "<a href=\"javascript:go()\">x</a>",
            "<p>import('/a.js')</p>",
        ] {
            assert!(text_may_run_code(&text.to_ascii_lowercase()), "{text}");
        }
        // Markup alone, a name that only contains `on`, and a path or text that has `on` in it but no `=` after the letters.
        for text in [
            "<html><body><p>hi</p></body></html>",
            "<div id=\"root\" data-online=\"1\" class=\"one\">x</div>",
            "<a href=\"/section/one\">x</a>",
            "<meta name=\"description\" content=\"a conference on lists\">",
            "<p>the person said so</p>",
            "<button type=\"button\" aria-pressed=\"true\">go</button>",
        ] {
            assert!(!text_may_run_code(&text.to_ascii_lowercase()), "{text}");
        }
    }

    /// ADR 1154: the page the entries are read from and the other pages of a project read an inline handler after a solidus, in upper case and after a quote as they read
    /// one after a space: the `index.html` is refused (the realm is unknown), and a `public/` page with a handler is a second page that can run code.
    #[test]
    fn a_handler_after_a_solidus_is_read_in_the_entry_page_and_in_a_public_page() {
        let entries = |page: &str| {
            let root = scratch("n1-entries");
            write(&root, "index.html", page);
            write(&root, "src/main.tsx", "export {};");
            let files = program(&root, &["src/main.tsx"]);
            entries_of(&root, &|path| files.contains(path)).map(|entries| entries.len())
        };
        assert_eq!(entries(PAGE), Ok(1));
        for (name, page, reason) in [
            (
                "space",
                PAGE.replace("<body>", "<body onfocus=\"hook()\">"),
                "executable inline event attribute",
            ),
            (
                "solidus",
                PAGE.replace("<body>", "<body/onfocus=\"hook()\">"),
                "executable inline event attribute",
            ),
            (
                "solidus-div",
                PAGE.replace(
                    "<div id=\"root\">",
                    "<div id=\"root\"/onfocusin=\"hook()\">",
                ),
                "executable inline event attribute",
            ),
            (
                "upper",
                PAGE.replace("<body>", "<body ONFOCUSIN=\"hook()\">"),
                "executable inline event attribute",
            ),
            (
                "upper-solidus",
                PAGE.replace("<body>", "<BODY/ONFOCUSIN=\"hook()\">"),
                "executable inline event attribute",
            ),
            (
                "bang-comment",
                PAGE.replace("<body>", "<!-- x --!><body onfocusin=\"hook()\"> -->"),
                "ended by `--!>`",
            ),
        ] {
            let error = entries(&page).expect_err(name);
            assert!(error.contains(reason), "{name}: {error}");
        }
        let other = |page: &str| {
            let root = scratch("n1-public");
            write(&root, "index.html", PAGE);
            write(&root, "src/main.tsx", "export {};");
            write(&root, "public/frame.html", page);
            other_page(&root).unwrap().is_some()
        };
        assert!(other(
            "<html><body/onfocus=\"parent.hook()\"></body></html>"
        ));
        assert!(other(
            "<HTML><BODY ONFOCUS=\"parent.hook()\"></BODY></HTML>"
        ));
        assert!(other(
            "<html><body><a href='/'onclick='go()'>x</a></body></html>"
        ));
        assert!(!other("<html><body><p>hi</p></body></html>"));
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
    fn a_vite_config_with_the_solid_plugin_is_read_and_anything_else_in_the_chain_is_unknown() {
        let root = scratch("bundler");
        let project = root.join("app");
        write(&project, "package.json", r#"{ "private": true }"#);

        // No configuration says nothing of how the page is built.
        let none = bundler(&project).unwrap_err();
        assert!(none.contains("names the build"), "{none}");

        // The Solid plugin, with either helper.
        for helper in ["vite", "vitest/config"] {
            write(
                &project,
                "vite.config.ts",
                &format!(
                    "import {{ defineConfig }} from '{helper}'; import solid from 'vite-plugin-solid'; \
                     export default defineConfig({{ plugins: [solid()], test: {{ setupFiles: ['./setup.ts'] }} }});"
                ),
            );
            let names = bundler(&project).unwrap().host_names;
            assert!(
                names.contains("vite") && names.contains("vite-plugin-solid"),
                "{names:?}"
            );
        }

        // A name proves nothing (invariant 3): a plugin the grammar admits by name is no more trusted than one it does not know, whatever it is called.
        for (import, plugin) in [
            ("import tw from '@tailwindcss/vite';", "tw()"),
            ("import paths from 'vite-tsconfig-paths';", "paths()"),
            (
                "import uno from 'unocss/vite';",
                "uno({ configFile: false })",
            ),
            ("import { VitePWA } from 'vite-plugin-pwa';", "VitePWA()"),
        ] {
            write(
                &project,
                "vite.config.ts",
                &format!(
                    "import solid from 'vite-plugin-solid'; {import} export default {{ plugins: [solid(), {plugin}] }};"
                ),
            );
            let refused = bundler(&project).unwrap_err();
            assert!(refused.contains("is not Solid's"), "{import}: {refused}");
        }

        // A plugin the grammar does not know refuses the config.
        write(
            &project,
            "vite.config.ts",
            "import other from 'some-plugin'; export default { plugins: [other()] };",
        );
        assert!(bundler(&project).is_err());

        // A Vite alias, a deduplicated package and the Solid plugin's start mode send a specifier or the entries elsewhere than the closure follows.
        for options in [
            "resolve: { alias: { '~': './src' } }",
            "resolve: { dedupe: ['solid-js'] }",
        ] {
            write(
                &project,
                "vite.config.ts",
                &format!("export default {{ plugins: [], {options} }};"),
            );
            let refused = bundler(&project).unwrap_err();
            assert!(
                refused.contains("alias or a deduplicated"),
                "{options}: {refused}"
            );
        }
        write(
            &project,
            "vite.config.ts",
            "import solid from 'vite-plugin-solid'; export default { plugins: [solid({ start: true })] };",
        );
        let start = bundler(&project).unwrap_err();
        assert!(start.contains("start mode"), "{start}");

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

    #[test]
    fn only_a_data_files_query_is_no_load() {
        for data in [
            "./a.css",
            "./logo.svg?url",
            "./shader.png?inline",
            "./data.json?raw",
            "./shader.frag?raw",
            "./notes.md?raw",
            "./a.css?no-inline",
        ] {
            assert_eq!(classify(data), Specifier::Asset, "{data}");
        }
        for module in [
            "./App",
            "./App.tsx",
            "solid-js",
            "pkg/dist/index.js",
            "./data.json",
        ] {
            assert_eq!(classify(module), Specifier::Module, "{module}");
        }
        // A script's or a package's query is a string or a URL the page can run; an unknown query is unknown.
        for refused in [
            "./x.ts?url",
            "./x.ts?raw",
            "./x.js?inline",
            "./x.mjs?raw",
            "./noextension?raw",
            "fz-prealm-any/index.js?url",
            "pkg?url",
            "pkg/style.css?inline",
            "./w.ts?worker",
            "./w.ts?sharedworker",
            "./a.css?used",
            "./a.css?react",
        ] {
            assert!(
                matches!(classify(refused), Specifier::Refused(_)),
                "{refused}"
            );
        }
    }

    #[test]
    fn vite_picks_by_its_own_extension_order_and_the_closure_must_agree() {
        let root = scratch("vite-pick");
        write(&root, "util.ts", "export const util = 1;");
        let ts = root.join("util.ts");
        let importer = root.join("main.tsx");
        let importer = importer.to_str().unwrap();
        // Only the `.ts`: Vite picks it too.
        assert_eq!(vite_pick(&root, "./util").unwrap(), Some(ts.clone()));
        assert!(vite_agrees(importer, "./util", ts.to_str().unwrap()).is_ok());
        // A `.js` beside it ranks first for Vite (`.mjs, .js, .mts, .ts, …`), and TypeScript's attestation is the `.ts`.
        write(&root, "util.js", "import 'x';");
        assert_eq!(
            vite_pick(&root, "./util").unwrap(),
            Some(root.join("util.js"))
        );
        let refused = vite_agrees(importer, "./util", ts.to_str().unwrap()).unwrap_err();
        assert!(refused.contains("util.js"), "{refused}");
        // An exact name is itself; a `.js` that names a TypeScript source where no `.js` exists is the source.
        assert_eq!(
            vite_pick(&root, "./util.ts").unwrap(),
            Some(root.join("util.ts"))
        );
        write(&root, "emit.ts", "");
        assert_eq!(
            vite_pick(&root, "./emit.js").unwrap(),
            Some(root.join("emit.ts"))
        );
        write(&root, "emit.js", "");
        assert_eq!(
            vite_pick(&root, "./emit.js").unwrap(),
            Some(root.join("emit.js"))
        );
        // A directory: its `index` in Vite's order; with a `package.json`, its fields (not followed).
        write(&root, "dir/index.ts", "");
        write(&root, "dir/index.js", "");
        assert_eq!(
            vite_pick(&root, "./dir").unwrap(),
            Some(root.join("dir/index.js"))
        );
        write(&root, "dir/package.json", "{}");
        assert!(vite_pick(&root, "./dir").is_err());
        // A file beside a directory of the same stem is the file (Vite probes extensions first).
        write(&root, "both.ts", "");
        write(&root, "both/index.ts", "");
        assert_eq!(
            vite_pick(&root, "./both").unwrap(),
            Some(root.join("both.ts"))
        );
        // Nothing there: no file; a bare or absolute specifier is not this check's.
        assert_eq!(vite_pick(&root, "./absent").unwrap(), None);
        assert!(vite_agrees(importer, "./absent", "/somewhere").is_err());
        assert!(vite_agrees(importer, "solid-js", "/node_modules/solid-js/x.js").is_ok());
    }

    #[test]
    fn the_manifest_closure_is_every_name_the_dependencies_of_the_loaded_packages_may_load() {
        let root = scratch("manifests");
        let package = |dir: &str, manifest: &str| {
            write(&root, &format!("{dir}/package.json"), manifest);
        };
        package(
            "node_modules/a",
            r#"{ "name": "a", "dependencies": { "b": "1", "ghost": "1" }, "peerDependencies": { "p": "1" }, "devDependencies": { "dev": "1" } }"#,
        );
        package(
            "node_modules/b",
            r#"{ "name": "b", "optionalDependencies": { "c": "1", "nested": "1" } }"#,
        );
        // A dependency a package carries itself is found before one beside it.
        package(
            "node_modules/b/node_modules/nested",
            r#"{ "name": "nested" }"#,
        );
        // A cycle ends.
        package(
            "node_modules/c",
            r#"{ "name": "c", "dependencies": { "a": "1" } }"#,
        );
        package("node_modules/p", r#"{ "name": "p" }"#);
        package("node_modules/dev", r#"{ "name": "dev" }"#);
        let names = manifest_closure([root.join("node_modules/a")], 100).unwrap();
        assert_eq!(
            names.iter().map(String::as_str).collect::<Vec<_>>(),
            ["a", "b", "c", "ghost", "nested", "p"],
            "a dev dependency is not shipped; a name counts where no copy is installed"
        );
        // A directory no copy answers names nothing.
        assert!(
            manifest_closure([root.join("node_modules/absent")], 100)
                .unwrap()
                .is_empty()
        );
        // More packages than the limit end the read.
        let refused = manifest_closure([root.join("node_modules/a")], 2).unwrap_err();
        assert!(refused.contains("more than 2 packages"), "{refused}");
        // A copy is found as Node finds it: in the package's own node_modules, or in an ancestor's.
        assert_eq!(
            installed_package(&root.join("node_modules/b"), "nested"),
            Some(root.join("node_modules/b/node_modules/nested"))
        );
        assert_eq!(
            installed_package(&root.join("node_modules/b"), "p"),
            Some(root.join("node_modules/p"))
        );
        assert!(installed_package(&root.join("node_modules/b"), "absent").is_none());
    }

    #[test]
    fn the_diagnostics_bridge_of_the_solid_plugin_keeps_its_package_in_the_realm_unless_it_is_off()
    {
        let root = scratch("diagnostics");
        let project = root.join("app");
        let config = |options: &str| {
            write(
                &project,
                "vite.config.ts",
                &format!(
                    "import solid from 'vite-plugin-solid'; export default {{ plugins: [solid({options})] }};"
                ),
            );
        };
        let kept = |project: &Path| {
            bundler(project)
                .unwrap()
                .host_names
                .contains("@solidjs/diagnostics")
        };
        write(&project, "package.json", r#"{ "private": true }"#);
        // The bridge is off for the literal `false` and a chain that does not declare the package, and for nothing else.
        config("{ diagnostics: false }");
        assert!(!kept(&project));
        // Forced on, or the plugin's own detection decides (the option absent, or no boolean): the page may load the package.
        for options in [
            "{ diagnostics: true }",
            "",
            "{}",
            "{ diagnostics: 'auto' }",
            "{ diagnostics: null }",
        ] {
            config(options);
            assert!(kept(&project), "{options}");
        }
        // A declaration keeps it beside `diagnostics: false` too: any of the four fields, in a manifest above the project, and a manifest that does not parse might hold it.
        config("{ diagnostics: false }");
        for field in [
            "dependencies",
            "devDependencies",
            "optionalDependencies",
            "peerDependencies",
        ] {
            write(
                &project,
                "package.json",
                &format!(r#"{{ "private": true, "{field}": {{ "@solidjs/diagnostics": "^1" }} }}"#),
            );
            assert!(kept(&project), "{field}");
        }
        write(
            &project,
            "package.json",
            r#"{ "private": true, "devDependencies": { "other": "1" } }"#,
        );
        assert!(!kept(&project));
        write(
            &root,
            "package.json",
            r#"{ "devDependencies": { "@solidjs/diagnostics": "^1" } }"#,
        );
        assert!(kept(&project));
        fs::remove_file(root.join("package.json")).unwrap();
        assert!(!kept(&project));
        write(&project, "package.json", "{ not json");
        assert!(kept(&project));
        // A conditional config takes the most on of its branches, and a plugin that is imported and never called has no bridge.
        write(&project, "package.json", r#"{ "private": true }"#);
        write(
            &project,
            "vite.config.ts",
            "import { defineConfig } from 'vite'; import solid from 'vite-plugin-solid'; \
             export default defineConfig(({ mode }) => mode ? { plugins: [solid({ diagnostics: false })] } : { plugins: [solid({ diagnostics: true })] });",
        );
        assert!(kept(&project));
        write(
            &project,
            "vite.config.ts",
            "import solid from 'vite-plugin-solid'; export default { plugins: [] };",
        );
        assert!(!kept(&project));
        // No Solid plugin, no bridge, whatever a manifest declares.
        write(
            &project,
            "package.json",
            r#"{ "devDependencies": { "@solidjs/diagnostics": "^1" } }"#,
        );
        write(
            &project,
            "vite.config.ts",
            "export default { plugins: [] };",
        );
        assert!(!kept(&project));
    }

    /// A FIFO at `path`; false where `mkfifo` is not installed (the tests that need one then pass vacuously).
    #[cfg(unix)]
    fn fifo(path: &Path) -> bool {
        std::process::Command::new("mkfifo")
            .arg(path)
            .status()
            .is_ok_and(|status| status.success())
    }

    /// The owned (WASM) host's input store faults on a directory that lists a FIFO, and the fault poisons the checker (CI's `/tmp` held one): the realm lists no
    /// directory above the project's workspace.
    #[cfg(unix)]
    #[test]
    fn a_fifo_above_the_project_is_never_listed() {
        let top = scratch("fifo-above");
        if !fifo(&top.join("pipe")) {
            return;
        }
        let project = top.join("project");
        write(&project, "package.json", r#"{ "private": true }"#);
        write(
            &project,
            "vite.config.ts",
            "export default { plugins: [] };",
        );
        // No workspace root: the chain runs up through `top`, which holds the FIFO, and only the project directory is listed. (A temporary directory inside a
        // workspace is one: its listing is then the realm's, and a FIFO in it ends the read, as the next test holds.)
        assert!(chain(&project).contains(&top));
        if listable(&project) != vec![project.clone()] {
            return;
        }
        assert!(bundler(&project).is_ok());
        assert_eq!(other_page(&project), Ok(None));
        // A directory above the workspace root is as little listed.
        let above = scratch("fifo-above-root");
        if !fifo(&above.join("pipe")) {
            return;
        }
        let root = above.join("repo");
        let nested = root.join("packages/app");
        write(&nested, "package.json", r#"{ "private": true }"#);
        write(&nested, "vite.config.ts", "export default { plugins: [] };");
        write(
            &root,
            "package.json",
            r#"{ "private": true, "workspaces": ["packages/*"] }"#,
        );
        assert_eq!(listable(&nested), chain(&nested));
        assert!(!listable(&nested).contains(&above));
        assert!(bundler(&nested).is_ok());
    }

    /// Where the realm does list (the project and its workspace), a listing that holds a FIFO, a socket or a device ends it, as does a read that would block.
    #[cfg(unix)]
    #[test]
    fn a_fifo_in_the_project_or_its_workspace_ends_the_read() {
        let top = scratch("fifo-inside");
        let project = top.join("packages/app");
        write(&project, "package.json", r#"{ "private": true }"#);
        write(
            &project,
            "vite.config.ts",
            "export default { plugins: [] };",
        );
        write(
            &top,
            "package.json",
            r#"{ "private": true, "workspaces": ["packages/*"] }"#,
        );
        assert_eq!(listable(&project).len(), 3);
        assert!(bundler(&project).is_ok());
        let refused = |why: Result<(), String>| {
            let why = why.unwrap_err();
            assert!(
                why.contains("is not a regular file, directory or link"),
                "{why}"
            );
        };
        // The workspace root's own listing.
        if !fifo(&top.join("pipe")) {
            return;
        }
        refused(bundler(&project).map(drop));
        fs::remove_file(top.join("pipe")).unwrap();
        assert!(bundler(&project).is_ok());
        // The project directory.
        assert!(fifo(&project.join("pipe")));
        refused(bundler(&project).map(drop));
        refused(other_page(&project).map(drop));
        fs::remove_file(project.join("pipe")).unwrap();
        // A directory under it, searched for a second page; a FIFO named like a page is never read.
        write(&project, "src/main.ts", "export {};");
        assert_eq!(other_page(&project), Ok(None));
        assert!(fifo(&project.join("src/page.html")));
        refused(other_page(&project).map(drop));
        fs::remove_file(project.join("src/page.html")).unwrap();
        // A link named like a page to a FIFO that is itself not listed: the link is followed to what it names, never read.
        let outside = scratch("fifo-link");
        if !fifo(&outside.join("pipe")) {
            return;
        }
        std::os::unix::fs::symlink(outside.join("pipe"), project.join("src/link.html")).unwrap();
        let why = other_page(&project).unwrap_err();
        assert!(
            why.contains("is a link to something that is no regular file"),
            "{why}"
        );
    }

    #[test]
    fn the_directories_a_listing_reads_are_the_projects_and_its_workspaces() {
        let top = scratch("listable");
        let project = top.join("packages/app");
        write(&project, "package.json", r#"{ "private": true }"#);
        // A root names itself by `pnpm-workspace.yaml` (or `.git`, or `workspaces`), and the listing reaches it.
        write(&top, "pnpm-workspace.yaml", "packages: ['packages/*']\n");
        assert_eq!(
            listable(&project),
            vec![project.clone(), top.join("packages"), top.clone()]
        );
        // A root nearer than that one stops the chain there.
        write(
            &top.join("packages"),
            "package.json",
            r#"{ "workspaces": ["app"] }"#,
        );
        assert_eq!(
            listable(&project),
            vec![project.clone(), top.join("packages")]
        );
        // The chain holds eight directories: a root beyond them is no root in reach, and the project directory alone is listed.
        let deep = top.join("a/b/c/d/e/f/g/h/i");
        write(&deep, "package.json", r#"{ "private": true }"#);
        assert_eq!(listable(&deep), vec![deep.clone()]);
    }

    #[test]
    fn another_vite_config_file_or_a_script_that_selects_one_ends_the_read() {
        for name in [
            "vite.config.prod.ts",
            "vite.prod.config.js",
            "app.vite.config.ts",
            "vite.config.staging.mjs",
            "vite-config.ts",
        ] {
            assert!(other_vite_config(name), "{name}");
        }
        for name in [
            "vite.config.ts",
            "vite.config.mjs",
            "vitest.config.ts",
            "vitest.workspace.ts",
            "vite-env.d.ts",
            "vite.svg",
        ] {
            assert!(!other_vite_config(name), "{name}");
        }
        // A run of vite that is not the plain build of the project directory ends the read: another config file, another directory or project, a project root.
        for line in [
            "vite build --config vite.config.prod.ts",
            "tsc && vite build -c other.ts",
            "vite build --config=x.ts",
            "pnpm --dir ../.. exec vite build",
            "npm --prefix web exec vite build",
            "cd web && vite build",
            "vite build web",
            "vite ./web --port 3000",
            "vite --watch ./web",
            "vite build --mode staging web",
            "FOO=1 npm exec -- vite build -c x.ts",
            "concurrently \"vite\" \"node server\"",
            "echo vite",
            "vite build && cd ../x",
        ] {
            assert!(script_run_differs(line).is_some(), "{line}");
        }
        // The plain runs: a mode (it picks env files and config branches, which the closure covers whole), flags with their values, runners, environment setters, other
        // commands in the line, and what is no vite.
        for line in [
            "vite build",
            "vite",
            "vite dev",
            "vite preview --host",
            "vite dev --port 3000",
            "vite dev --host 127.0.0.1 --port 5173",
            "vite build --watch --mode development",
            "vite --mode staging",
            "node_modules/.bin/vite build --mode=production",
            "VITE_DEV_BACKEND=emulator vite",
            "panda codegen --clean && vite build",
            "tsc --noEmit && vite build",
            "vite build && pnpm typecheck",
            "npx vite build",
            "pnpm exec vite build",
            "cross-env NODE_ENV=production vite build",
            "node node_modules/vite/bin/vite.js build",
            "vitest run --config vitest.config.ts",
            "tsc --project tsconfig.json",
            "playwright test --config pw.config.ts",
            "wrangler types --cwd src && tsc -b",
            "rimraf node_modules/.vite",
            "cd web && vitest run",
        ] {
            assert!(script_run_differs(line).is_none(), "{line}");
        }
        // Arguments a script forwards to one that runs vite (`npm run build -- --config x`) are vite's: a `--config`, a root, a mode, whatever follows.
        let scripts = |json: &str| -> serde_json::Map<String, serde_json::Value> {
            serde_json::from_str(json).unwrap()
        };
        let forwards = |json: &str| -> Vec<String> {
            let scripts = scripts(json);
            let names = scripts_running_vite(&scripts);
            scripts
                .iter()
                .filter_map(|(name, line)| {
                    let line = line.as_str()?;
                    (script_run_differs(line)
                        .or_else(|| {
                            runs_script_with(line, &names)
                                .filter(|forwarded| *forwarded)
                                .map(|_| String::new())
                        })
                        .is_some())
                    .then(|| name.clone())
                })
                .collect()
        };
        assert_eq!(
            forwards(r#"{ "build": "vite build", "prod": "npm run build -- --config x.ts" }"#),
            ["prod"]
        );
        assert_eq!(
            forwards(r#"{ "a": "vite", "b": "npm run a", "c": "npm run b -- --config y.ts" }"#),
            ["c"]
        );
        assert_eq!(
            forwards(r#"{ "build": "vite build", "ci": "pnpm build --mode staging" }"#),
            ["ci"]
        );
        assert_eq!(
            forwards(r#"{ "build": "vite build", "x": "npm run build -- \"--config\" y.ts" }"#),
            ["x"]
        );
        // Plain calls, other scripts, and what runs no vite forward nothing to it.
        assert!(
            forwards(
                r#"{ "build": "vite build", "ci": "pnpm build", "all": "eslint . && npm run build && yarn test", "test": "vitest run", "t2": "npm run test -- --coverage" }"#
            )
            .is_empty()
        );
        let root = scratch("vite-select");
        let project = root.join("app");
        write(
            &project,
            "package.json",
            r#"{ "private": true, "scripts": { "test": "vitest run --config vitest.config.ts", "build": "vite build" } }"#,
        );
        write(
            &project,
            "vite.config.ts",
            "export default { plugins: [] };",
        );
        write(&project, "vitest.config.ts", "export default {};");
        assert!(bundler(&project).is_ok());
        write(
            &project,
            "package.json",
            r#"{ "private": true, "scripts": { "build": "vite build --config vite.config.prod.ts" } }"#,
        );
        let script = bundler(&project).unwrap_err();
        assert!(script.contains("the script `build`"), "{script}");
        write(&project, "package.json", r#"{ "private": true }"#);
        // A script of an ancestor in the chain counts too (the workspace root's).
        write(
            &root,
            "package.json",
            r#"{ "scripts": { "dev": "vite --config other.config.ts" } }"#,
        );
        assert!(bundler(&project).is_err());
        // A mode is no other config: kui's `watch` script is `vite build --watch --mode development`.
        write(
            &root,
            "package.json",
            r#"{ "scripts": { "watch": "vite build --watch --mode development" } }"#,
        );
        assert!(bundler(&project).is_ok());
        fs::remove_file(root.join("package.json")).unwrap();
        assert!(bundler(&project).is_ok());
        write(&project, "vite.config.prod.ts", "export default {};");
        let other = bundler(&project).unwrap_err();
        assert!(other.contains("another Vite configuration file"), "{other}");
    }

    #[test]
    fn tsconfig_options_vite_does_not_share_end_the_read() {
        let root = scratch("tsconfig");
        let project = root.join("tsconfig.json");
        write(
            &root,
            "tsconfig.json",
            r#"{ "compilerOptions": { "strict": true } }"#,
        );
        assert!(tsconfig_resolution(&project, false).is_ok());
        // `customConditions`, `rootDirs` and `moduleSuffixes` have no Vite counterpart.
        for option in [
            r#""customConditions": ["browser"]"#,
            r#""rootDirs": ["./a", "./b"]"#,
            r#""moduleSuffixes": [".web", ""]"#,
        ] {
            write(
                &root,
                "tsconfig.json",
                &format!(r#"{{ "compilerOptions": {{ {option} }} }}"#),
            );
            for applies in [false, true] {
                let refused = tsconfig_resolution(&project, applies).unwrap_err();
                assert!(
                    refused.contains("changes TypeScript's resolution"),
                    "{option}: {refused}"
                );
            }
        }
        // `paths` and `baseUrl`: Vite's only where a config applies them, and for the `tsconfig.json` it finds.
        for option in [r#""paths": { "x": ["./y.ts"] }"#, r#""baseUrl": ".""#] {
            write(
                &root,
                "tsconfig.json",
                &format!(r#"{{ "compilerOptions": {{ {option} }} }}"#),
            );
            let refused = tsconfig_resolution(&project, false).unwrap_err();
            assert!(refused.contains("Vite does not"), "{option}: {refused}");
            assert!(tsconfig_resolution(&project, true).is_ok(), "{option}");
        }
        write(
            &root,
            "tsconfig.app.json",
            r#"{ "compilerOptions": { "paths": { "x": ["./y.ts"] } } }"#,
        );
        let other = tsconfig_resolution(&root.join("tsconfig.app.json"), true).unwrap_err();
        assert!(other.contains("another file"), "{other}");
        // An option an `extends` chain brings is the project's.
        write(
            &root,
            "base.json",
            r#"{ "compilerOptions": { "customConditions": ["browser"] } }"#,
        );
        write(
            &root,
            "tsconfig.json",
            r#"{ "extends": "./base.json", "compilerOptions": {} }"#,
        );
        assert!(tsconfig_resolution(&project, true).is_err());
        // A chain that cannot be read is no proof.
        write(&root, "tsconfig.json", r#"{ "extends": "./missing.json" }"#);
        assert!(tsconfig_resolution(&project, true).is_err());
    }

    #[test]
    fn a_solid_plugin_that_is_not_an_installed_pinned_closure_is_not_trusted() {
        let root = scratch("plugin");
        let project = root.join("app");
        write(&project, "package.json", r#"{ "private": true }"#);
        // Not installed: nothing to build with.
        let missing = plugin_pinned(&project, "vite-plugin-solid").unwrap_err();
        assert!(missing.contains("is not installed"), "{missing}");
        // Installed with the audited release's name and version, and a closure no pin names.
        write(
            &root,
            "node_modules/vite-plugin-solid/package.json",
            r#"{ "name": "vite-plugin-solid", "version": "3.0.0-next.27" }"#,
        );
        write(
            &root,
            "node_modules/vite-plugin-solid/index.js",
            "export default () => ({});",
        );
        let unpinned = plugin_pinned(&project, "vite-plugin-solid").unwrap_err();
        assert!(
            unpinned.contains("unaudited installed closure"),
            "{unpinned}"
        );
        // The answer is kept while what it read is unchanged, and read again when it changes.
        assert_eq!(
            plugin_pinned(&project, "vite-plugin-solid").unwrap_err(),
            unpinned
        );
        write(
            &root,
            "node_modules/vite-plugin-solid/index.js",
            "export default () => ({ name: 'other' });",
        );
        let changed = plugin_pinned(&project, "vite-plugin-solid").unwrap_err();
        assert!(
            changed.contains("unaudited installed closure") && changed != unpinned,
            "{changed}"
        );
    }
}
