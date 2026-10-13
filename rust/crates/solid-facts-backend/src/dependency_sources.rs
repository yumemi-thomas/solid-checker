//! Experimental dependency-source mode (ADR 0284): analyze the runtime source
//! of selected installed packages as if it were project code.
//!
//! TypeScript resolves an import of a package to its declarations, so the
//! package's JavaScript never enters the program. This module selects the
//! file a Vite + `vite-plugin-solid` build for the declared host loads for
//! each requested specifier (the package's `exports` read in document order
//! with `solid`, `import`, `module`, `default` and the host's own conditions:
//! its target, its build and every `--runtime-condition`), walks its
//! relative imports to a closure, and writes a sibling tsconfig that extends
//! the project's and lists exactly the project's configured sources plus that
//! closure as root files, with `allowJs` and an empty `include`, so neither an
//! inherited `include` nor `allowJs` changes which project files are analyzed
//! (review round 2). The
//! caller then binds each import of the package, and each relative import
//! inside it, to the runtime file it loads (`ImportResolution`), because the
//! compiler still resolves them to declarations.
//!
//! A package the project does not install is skipped with a note. Every other
//! refusal is an error that names the package: an `exports` form this does
//! not read (an array, a condition this does not set leading to nothing), or
//! a closure larger than [`MAX_CLOSURE_FILES`]. Subpath patterns (`./*`)
//! resolve as Node resolves them. Nothing here guesses a file.

use std::collections::{BTreeSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};

use crate::host_constants::{Ordered, find_package_root, split_bare_specifier};

/// The conditions every Vite + `vite-plugin-solid` build sets, beside the
/// host's: `exports` is read in its own document order.
const BUILD_CONDITIONS: &[&str] = &["solid", "import", "module", "default"];
const MAX_CLOSURE_FILES: usize = 4000;
const SOURCE_EXTENSIONS: &[&str] = &[".js", ".jsx", ".mjs", ".ts", ".tsx", ".mts"];

/// One import the compiler resolves to declarations, and the runtime file it
/// loads.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ImportResolution {
    /// The importing file, or `None` for every project file that imports
    /// the package by this bare specifier.
    pub importer: Option<PathBuf>,
    pub specifier: String,
    pub target: PathBuf,
}

/// The selected packages, their closure, and the written tsconfig.
#[derive(Debug)]
pub struct DependencySources {
    /// Canonical package directories.
    pub roots: Vec<PathBuf>,
    /// The selected packages' names, whose contracts are not bound.
    pub package_names: std::collections::BTreeSet<String>,
    /// Canonical closure files, the tsconfig's `files`.
    pub files: Vec<PathBuf>,
    pub resolutions: Vec<ImportResolution>,
    /// The written tsconfig, removed when this is dropped.
    pub tsconfig: PathBuf,
}

impl Drop for DependencySources {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.tsconfig);
    }
}

impl DependencySources {
    /// Whether `path` lies inside one of the selected packages.
    #[must_use]
    pub fn contains(&self, path: &Path) -> bool {
        self.roots.iter().any(|root| path.starts_with(root))
    }
}

/// The declared host a package entry is selected for.
#[derive(Clone, Debug)]
pub struct Host {
    /// The host's conditions: `browser` or `node`, `development` or
    /// `production`, and every `--runtime-condition`.
    pub conditions: BTreeSet<String>,
    pub browser: bool,
}

/// Select, walk and write. `tsconfig` is the project's canonical tsconfig.
/// `None` when the project installs none of the packages: the analysis is
/// then the ordinary one.
pub fn prepare(
    tsconfig: &Path,
    specifiers: &[String],
    host: &Host,
    project_files: &[PathBuf],
) -> Result<Option<DependencySources>, String> {
    let mut roots = Vec::new();
    let mut package_names = BTreeSet::new();
    let mut files = BTreeSet::new();
    let mut resolutions = BTreeSet::new();
    for specifier in specifiers {
        let (name, subpath) = split_bare_specifier(specifier)
            .ok_or_else(|| format!("--dependency-source {specifier:?} is not a bare specifier"))?;
        // A package the project does not install is no source to analyze;
        // a sweep may name the same packages for every project.
        let Some(root) = find_package_root(tsconfig, &name, None) else {
            eprintln!(
                "solid-checker: note: --dependency-source {specifier}: not installed here, skipped"
            );
            continue;
        };
        let root = root
            .canonicalize()
            .map_err(|error| format!("--dependency-source {specifier:?}: {error}"))?;
        let entry = select_entry(&root, &subpath, host)
            .map_err(|reason| format!("--dependency-source {specifier:?}: {reason}"))?;
        resolutions.insert(ImportResolution {
            importer: None,
            specifier: specifier.clone(),
            target: entry.clone(),
        });
        walk(&root, &entry, &mut files, &mut resolutions, &mut Vec::new())
            .map_err(|reason| format!("--dependency-source {specifier:?}: {reason}"))?;
        package_names.insert(name);
        if !roots.contains(&root) {
            roots.push(root);
        }
    }
    if roots.is_empty() {
        return Ok(None);
    }
    let tsconfig = write_program(tsconfig, project_files, &files)?;
    Ok(Some(DependencySources {
        roots,
        package_names,
        files: files.into_iter().collect(),
        resolutions: resolutions.into_iter().collect(),
        tsconfig,
    }))
}

/// ADR 0291: select every Solid package the project loads at run time, and
/// every Solid package those load in turn, each from where its importer
/// resolves it.
///
/// `requests` are the project's own runtime imports, `(importer, bare
/// specifier)`. `excluded` names the packages that stay dialect vocabulary
/// (the primitive-defining runtime). A Solid package is one whose
/// `dependencies` or `peerDependencies` name that runtime or a package in one
/// of `ecosystem_scopes`; any other package stays what it is without this
/// mode, an unknown package whose behavior is an obligation. A package that
/// cannot be analyzed (an
/// `exports` form this does not read, a `browser` map, an entry or relative
/// import that is not a source file, an oversized closure) is skipped with a
/// note and keeps its contract: nothing of it enters the program, and its
/// other copies are judged on their own.
pub fn prepare_all(
    tsconfig: &Path,
    requests: Vec<(PathBuf, String)>,
    host: &Host,
    project_files: &[PathBuf],
    excluded: &BTreeSet<String>,
    ecosystem_scopes: &[&str],
) -> Result<Option<DependencySources>, String> {
    let solid_package = |root: &Path| {
        let Some(manifest) = fs::read(root.join("package.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Ordered>(&bytes).ok())
        else {
            return false;
        };
        ["dependencies", "peerDependencies"].into_iter().any(|field| {
            matches!(manifest.get(field), Some(Ordered::Object(map)) if map.iter().any(|(name, _)| {
                excluded.contains(name)
                    || ecosystem_scopes.iter().any(|scope| name.starts_with(scope))
            }))
        })
    };
    let mut queue = VecDeque::from(requests);
    let mut not_solid = BTreeSet::<PathBuf>::new();
    let mut seen_requests = BTreeSet::new();
    let mut unsupported = BTreeSet::<PathBuf>::new();
    let mut names_by_root = std::collections::BTreeMap::<PathBuf, String>::new();
    let mut walked = BTreeSet::<PathBuf>::new();
    let mut files = BTreeSet::new();
    let mut resolutions = BTreeSet::new();
    let mut pending = Vec::<ImportResolution>::new();
    while let Some((importer, specifier)) = queue.pop_front() {
        if !seen_requests.insert((importer.clone(), specifier.clone())) {
            continue;
        }
        let Some((name, subpath)) = split_bare_specifier(&specifier) else {
            continue;
        };
        if excluded.contains(&name) || name.starts_with("node:") {
            continue;
        }
        // An alias, a virtual module or a builtin resolves to no installed
        // package directory, and stays what it was.
        let Some(root) =
            find_package_root(&importer, &name, None).and_then(|root| root.canonicalize().ok())
        else {
            continue;
        };
        // A workspace package linked into `node_modules` resolves to the
        // project's own directory: it is project code, not a dependency.
        if unsupported.contains(&root)
            || not_solid.contains(&root)
            || !root
                .components()
                .any(|component| component.as_os_str() == "node_modules")
        {
            continue;
        }
        if !names_by_root.contains_key(&root) && !solid_package(&root) {
            not_solid.insert(root);
            continue;
        }
        let entry = match select_entry(&root, &subpath, host) {
            Ok(entry) => entry,
            Err(reason) => {
                eprintln!(
                    "solid-checker: note: dependency source {name} ({}): {reason}; its contract stays",
                    root.display()
                );
                unsupported.insert(root);
                continue;
            }
        };
        names_by_root.insert(root.clone(), name);
        pending.push(ImportResolution {
            importer: Some(importer),
            specifier,
            target: entry.clone(),
        });
        if walked.insert(entry.clone()) {
            let mut closure = BTreeSet::new();
            let mut closure_resolutions = BTreeSet::new();
            let mut bare = Vec::new();
            match walk(
                &root,
                &entry,
                &mut closure,
                &mut closure_resolutions,
                &mut bare,
            ) {
                Ok(()) => {
                    files.extend(closure);
                    resolutions.extend(closure_resolutions);
                    queue.extend(bare);
                }
                Err(reason) => {
                    eprintln!(
                        "solid-checker: note: dependency source {} ({}): {reason}; its contract stays",
                        names_by_root[&root],
                        root.display()
                    );
                    unsupported.insert(root);
                }
            }
        }
    }
    // A package found unsupported after some of its entries were walked
    // takes all of them out.
    files.retain(|file| !unsupported.iter().any(|root| file.starts_with(root)));
    resolutions.retain(|resolution| {
        !unsupported
            .iter()
            .any(|root| resolution.target.starts_with(root))
    });
    resolutions.extend(pending.into_iter().filter(|resolution| {
        !unsupported
            .iter()
            .any(|root| resolution.target.starts_with(root))
    }));
    names_by_root.retain(|root, _| !unsupported.contains(root));
    if names_by_root.is_empty() {
        return Ok(None);
    }
    // A contract is unbound by package name, so a name with an unsupported
    // copy keeps its contract for every copy.
    let unsupported_names = unsupported
        .iter()
        .filter_map(|root| package_name_of_root(root))
        .collect::<BTreeSet<_>>();
    let package_names = names_by_root
        .values()
        .filter(|name| !unsupported_names.contains(*name))
        .cloned()
        .collect();
    let tsconfig = write_program(tsconfig, project_files, &files)?;
    Ok(Some(DependencySources {
        roots: names_by_root.into_keys().collect(),
        package_names,
        files: files.into_iter().collect(),
        resolutions: resolutions.into_iter().collect(),
        tsconfig,
    }))
}

/// The `name` a package directory's manifest declares.
fn package_name_of_root(root: &Path) -> Option<String> {
    let manifest = fs::read(root.join("package.json")).ok()?;
    let manifest = serde_json::from_slice::<Ordered>(&manifest).ok()?;
    manifest.get("name")?.as_str().map(str::to_owned)
}

/// The source file `subpath` of the package at `root` loads for `host`.
fn select_entry(root: &Path, subpath: &str, host: &Host) -> Result<PathBuf, String> {
    let manifest = fs::read(root.join("package.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Ordered>(&bytes).ok())
        .ok_or("unreadable package.json")?;
    // A `browser` map remaps files inside the package for a browser
    // build, which the closure walk does not model (review round 2). An
    // empty map remaps nothing.
    if host.browser
        && manifest.get("browser").is_some_and(|field| match field {
            Ordered::String(_) => false,
            Ordered::Object(map) => !map.is_empty(),
            _ => true,
        })
    {
        return Err("a `browser` field map is not supported".into());
    }
    let relative = entry_target(&manifest, subpath, host).ok_or_else(|| {
        format!("no runtime entry for {subpath} under the declared host's conditions")
    })?;
    let entry = root
        .join(relative.trim_start_matches("./"))
        .canonicalize()
        .map_err(|error| format!("entry {relative}: {error}"))?;
    if !entry.starts_with(root) || !is_source(&entry) {
        return Err(format!(
            "entry {relative} is not a source file inside the package"
        ));
    }
    Ok(entry)
}

/// Write the sibling tsconfig: the project's own program files plus the
/// closure, with an empty `include` (review round 2).
fn write_program(
    tsconfig: &Path,
    project_files: &[PathBuf],
    files: &BTreeSet<PathBuf>,
) -> Result<PathBuf, String> {
    let directory = tsconfig
        .parent()
        .ok_or("--dependency-source: the project tsconfig has no directory")?;
    let tsconfig_name = tsconfig
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("--dependency-source: the project tsconfig name is not UTF-8")?;
    let root_files = project_files
        .iter()
        .chain(files)
        .map(|file| file.to_string_lossy().into_owned())
        .collect::<BTreeSet<_>>();
    let document = serde_json::json!({
        "extends": format!("./{tsconfig_name}"),
        "compilerOptions": { "allowJs": true, "checkJs": false },
        "files": root_files,
        "include": [],
    });
    let written = directory.join(format!(
        ".solid-checker-dependency-sources.{}.tsconfig.json",
        std::process::id()
    ));
    fs::write(
        &written,
        serde_json::to_vec_pretty(&document).map_err(|error| error.to_string())?,
    )
    .map_err(|error| {
        format!(
            "--dependency-source: cannot write {}: {error}",
            written.display()
        )
    })?;
    Ok(written)
}

/// The package a bare specifier names (`@scope/pkg` for `@scope/pkg/sub`).
#[must_use]
pub fn package_name(specifier: &str) -> Option<String> {
    split_bare_specifier(specifier).map(|(name, _)| name)
}

/// The runtime-bound specifiers of `packages` that `source` imports or
/// re-exports: every subpath a selected package is loaded through (review
/// round 2). A side-effect or type-only import binds nothing to redirect.
#[must_use]
pub fn imported_subpaths(
    path: &str,
    source: &str,
    packages: &BTreeSet<String>,
) -> BTreeSet<String> {
    let Ok(facts) = solid_facts::ast::extract(path.to_owned(), source) else {
        return BTreeSet::new();
    };
    runtime_bare_imports(&facts)
        .into_iter()
        .filter(|module| package_name(module).is_some_and(|name| packages.contains(&name)))
        .collect()
}

/// Every bare specifier `path` loads at run time, for [`prepare_all`].
#[must_use]
pub fn runtime_bare_imports_of(path: &str, source: &str) -> BTreeSet<String> {
    solid_facts::ast::extract(path.to_owned(), source)
        .map(|facts| runtime_bare_imports(&facts))
        .unwrap_or_default()
}

/// The bare specifiers a module loads with a runtime binding, or
/// re-exports. A side-effect or type-only import binds nothing to redirect.
fn runtime_bare_imports(facts: &solid_facts::ast::AstFacts) -> BTreeSet<String> {
    facts
        .imports
        .iter()
        .filter(|import| {
            !import.type_only
                && import.bindings.iter().any(|binding| {
                    !binding.type_only
                        && binding.runtime_referenced
                        && binding.kind != solid_facts::ast::ImportKind::SideEffect
                })
        })
        .map(|import| import.module.as_str())
        .chain(
            facts
                .exports
                .iter()
                .filter(|export| !export.type_only)
                .filter_map(|export| export.module.as_deref()),
        )
        .filter(|module| split_bare_specifier(module).is_some())
        .map(str::to_owned)
        .collect()
}

fn is_source(path: &Path) -> bool {
    let name = path.to_string_lossy();
    !name.ends_with(".d.ts")
        && !name.ends_with(".d.mts")
        && !name.ends_with(".d.cts")
        && SOURCE_EXTENSIONS
            .iter()
            .any(|extension| name.ends_with(extension))
}

/// The runtime target `exports` selects for `subpath`, or without `exports`
/// the fields Vite reads (`mainFields`): `browser` for a browser host, then
/// `module`, then `main`. An object `browser` field remaps files one by one,
/// which this does not model, so a browser host refuses it.
fn entry_target(manifest: &Ordered, subpath: &str, host: &Host) -> Option<String> {
    match manifest.get("exports") {
        Some(exports) => {
            let (entry, star) = match exports {
                Ordered::Object(map) if map.iter().any(|(key, _)| key.starts_with('.')) => {
                    match exports.get(subpath) {
                        Some(entry) => (entry, None),
                        None => pattern_entry(map, subpath)?,
                    }
                }
                _ if subpath == "." => (exports, None),
                _ => return None,
            };
            select(entry, host, 0, star)
        }
        None if subpath == "." => {
            // An empty map, as `@solid-primitives` packages ship, remaps
            // nothing.
            let browser = host
                .browser
                .then(|| manifest.get("browser"))
                .flatten()
                .filter(|field| !matches!(field, Ordered::Object(map) if map.is_empty()));
            if browser.is_some_and(|field| field.as_str().is_none()) {
                return None;
            }
            browser
                .or_else(|| manifest.get("module"))
                .or_else(|| manifest.get("main"))
                .and_then(Ordered::as_str)
                .map(str::to_owned)
        }
        None => None,
    }
}

/// Node's subpath pattern match (`PATTERN_KEY_COMPARE`): among keys with one
/// `*` whose prefix and suffix surround `subpath`, the longest prefix wins,
/// then the longer key. The matched text substitutes every `*` of the
/// target. A match reaching out of the directory (`..`) selects nothing.
fn pattern_entry<'a>(
    map: &'a [(String, Ordered)],
    subpath: &'a str,
) -> Option<(&'a Ordered, Option<&'a str>)> {
    let (_, entry, star) = map
        .iter()
        .filter_map(|(key, entry)| {
            let (prefix, suffix) = key.split_once('*')?;
            if suffix.contains('*')
                || subpath.len() < key.len()
                || !subpath.starts_with(prefix)
                || !subpath.ends_with(suffix)
            {
                return None;
            }
            let star = &subpath[prefix.len()..subpath.len() - suffix.len()];
            Some((key, entry, star))
        })
        .max_by(|(left, ..), (right, ..)| {
            let prefix = |key: &str| key.find('*').unwrap_or(key.len());
            prefix(left)
                .cmp(&prefix(right))
                .then(left.len().cmp(&right.len()))
        })?;
    (!star
        .split('/')
        .any(|segment| segment == ".." || segment == "."))
    .then_some((entry, Some(star)))
}

/// Node's conditional resolution: the first key, in document order, that is
/// a set condition and resolves. `star` is a pattern match's text.
fn select(entry: &Ordered, host: &Host, depth: usize, star: Option<&str>) -> Option<String> {
    if depth > 16 {
        return None;
    }
    match entry {
        Ordered::String(target) => match star {
            Some(star) => Some(target.replace('*', star)),
            None => (!target.contains('*')).then(|| target.clone()),
        },
        Ordered::Object(map) => map
            .iter()
            .filter(|(key, _)| {
                BUILD_CONDITIONS.contains(&key.as_str()) || host.conditions.contains(key.as_str())
            })
            .find_map(|(_, value)| select(value, host, depth + 1, star)),
        _ => None,
    }
}

/// Every file `entry` reaches through relative static imports and
/// re-exports, inside `root`, and the bare runtime imports of those files
/// into `bare`.
fn walk(
    root: &Path,
    entry: &Path,
    files: &mut BTreeSet<PathBuf>,
    resolutions: &mut BTreeSet<ImportResolution>,
    bare: &mut Vec<(PathBuf, String)>,
) -> Result<(), String> {
    let mut queue = VecDeque::from([entry.to_path_buf()]);
    while let Some(file) = queue.pop_front() {
        if !files.insert(file.clone()) {
            continue;
        }
        if files.len() > MAX_CLOSURE_FILES {
            return Err(format!("closure exceeds {MAX_CLOSURE_FILES} files"));
        }
        let source =
            fs::read_to_string(&file).map_err(|error| format!("{}: {error}", file.display()))?;
        let facts = solid_facts::ast::extract(file.to_string_lossy().into_owned(), &source)
            .map_err(|error| format!("{}: {error}", file.display()))?;
        bare.extend(
            runtime_bare_imports(&facts)
                .into_iter()
                .map(|module| (file.clone(), module)),
        );
        let modules = facts
            .imports
            .iter()
            .filter(|import| !import.type_only)
            .map(|import| import.module.as_str())
            .chain(
                facts
                    .exports
                    .iter()
                    .filter(|export| !export.type_only)
                    .filter_map(|export| export.module.as_deref()),
            )
            .filter(|module| module.starts_with("./") || module.starts_with("../"))
            .map(str::to_owned)
            .collect::<BTreeSet<_>>();
        let directory = file.parent().ok_or("closure file has no directory")?;
        for module in modules {
            let Some(target) = resolve_relative(directory, &module) else {
                return Err(format!(
                    "{} imports {module:?}, which resolves to no source file",
                    file.display()
                ));
            };
            if !target.starts_with(root) {
                continue;
            }
            resolutions.insert(ImportResolution {
                importer: Some(file.clone()),
                specifier: module,
                target: target.clone(),
            });
            queue.push_back(target);
        }
    }
    Ok(())
}

/// A relative specifier as a bundler resolves it: the exact file, then the
/// source extensions, then an `index` file.
fn resolve_relative(directory: &Path, module: &str) -> Option<PathBuf> {
    let base = directory.join(module);
    let candidates = std::iter::once(base.clone())
        .chain(
            SOURCE_EXTENSIONS
                .iter()
                .map(|extension| PathBuf::from(format!("{}{extension}", base.to_string_lossy()))),
        )
        .chain(
            SOURCE_EXTENSIONS
                .iter()
                .map(|extension| base.join(format!("index{extension}"))),
        );
    candidates
        .filter(|candidate| candidate.is_file() && is_source(candidate))
        .find_map(|candidate| candidate.canonicalize().ok())
}

#[cfg(test)]
mod tests {
    use super::{Host, entry_target, prepare, select};
    use crate::host_constants::Ordered;

    fn ordered(json: &str) -> Ordered {
        serde_json::from_str(json).unwrap()
    }

    fn host(conditions: &[&str]) -> Host {
        Host {
            conditions: conditions.iter().map(|&c| c.to_owned()).collect(),
            browser: conditions.contains(&"browser"),
        }
    }

    #[test]
    fn a_project_installing_none_of_the_packages_is_analyzed_as_usual() {
        let directory = std::env::temp_dir().join(format!(
            "solid-checker-dependency-sources-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let tsconfig = directory.join("tsconfig.json");
        std::fs::write(&tsconfig, "{}").unwrap();
        let prepared = prepare(
            &tsconfig,
            &["not-installed-here".to_owned()],
            &host(&["browser", "development"]),
            &[],
        )
        .unwrap();
        let written = std::fs::read_dir(&directory).unwrap().count();
        std::fs::remove_dir_all(&directory).unwrap();
        assert!(prepared.is_none());
        assert_eq!(written, 1, "no sibling tsconfig is written");
    }

    #[test]
    fn conditions_are_read_in_document_order() {
        let browser = host(&["browser", "development"]);
        let router = ordered(
            r#"{"exports":{".":{"solid":"./dist/index.jsx","default":"./dist/index.js"}}}"#,
        );
        assert_eq!(
            entry_target(&router, ".", &browser).as_deref(),
            Some("./dist/index.jsx")
        );
        let tanstack = ordered(
            r#"{"exports":{".":{"solid":{"types":"./a.d.ts","default":"./dist/source/index.jsx"},"import":{"types":"./b.d.ts","default":"./dist/esm/index.js"}}}}"#,
        );
        assert_eq!(
            entry_target(&tanstack, ".", &browser).as_deref(),
            Some("./dist/source/index.jsx")
        );
        let node_first = ordered(r#"{"exports":{"node":"./node.js","import":"./browser.js"}}"#);
        assert_eq!(
            entry_target(&node_first, ".", &browser).as_deref(),
            Some("./browser.js")
        );
        let plain = ordered(r#"{"module":"./dist/index.js","main":"./dist/index.cjs"}"#);
        assert_eq!(
            entry_target(&plain, ".", &browser).as_deref(),
            Some("./dist/index.js")
        );
        assert!(select(&ordered(r#"{"require":"./a.cjs"}"#), &browser, 0, None).is_none());
    }

    #[test]
    fn a_linked_workspace_package_is_project_code() {
        let directory = std::env::temp_dir().join(format!(
            "solid-checker-dependency-workspace-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&directory);
        let package = |root: &std::path::Path, name: &str| {
            std::fs::create_dir_all(root.join("src")).unwrap();
            std::fs::write(
                root.join("package.json"),
                format!(
                    r#"{{"name":"{name}","peerDependencies":{{"solid-js":"2"}},"exports":{{".":{{"solid":"./src/index.js"}}}}}}"#
                ),
            )
            .unwrap();
            std::fs::write(root.join("src/index.js"), "export const value = 1;\n").unwrap();
        };
        package(&directory.join("packages/ui"), "ui");
        package(&directory.join("node_modules/real"), "real");
        std::fs::create_dir_all(directory.join("node_modules")).unwrap();
        std::os::unix::fs::symlink(
            directory.join("packages/ui"),
            directory.join("node_modules/ui"),
        )
        .unwrap();
        let tsconfig = directory.join("tsconfig.json");
        std::fs::write(&tsconfig, "{}").unwrap();
        let app = directory.join("App.tsx");
        let prepared = super::prepare_all(
            &tsconfig,
            vec![(app.clone(), "ui".into()), (app, "real".into())],
            &host(&["browser", "development"]),
            &[],
            &["solid-js".to_owned()].into_iter().collect(),
            &["@solidjs/"],
        )
        .unwrap()
        .unwrap();
        let roots = prepared
            .roots
            .iter()
            .map(|root| root.file_name().unwrap().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        drop(prepared);
        std::fs::remove_dir_all(&directory).unwrap();
        assert_eq!(roots, ["real"]);
    }

    #[test]
    fn every_runtime_imported_subpath_of_a_selected_package_is_selected() {
        let packages = ["@scope/pkg".to_owned()].into_iter().collect();
        let source = r#"
            import { a } from "@scope/pkg";
            import { b } from "@scope/pkg/sub";
            import type { C } from "@scope/pkg/types";
            import "@scope/pkg/styles.css";
            import { d } from "@scope/other";
            export { e } from "@scope/pkg/more";
            console.log(a, b, d);
        "#;
        let selected = super::imported_subpaths("/p/App.ts", source, &packages);
        assert_eq!(
            selected.into_iter().collect::<Vec<_>>(),
            ["@scope/pkg", "@scope/pkg/more", "@scope/pkg/sub"]
        );
    }

    #[test]
    fn subpath_patterns_follow_node() {
        let browser = host(&["browser", "development"]);
        // Kobalte 2.0.0-alpha.0's map.
        let kobalte = ordered(
            r#"{"exports":{"./*":{"types":"./dist/*/index.d.ts","solid":"./dist/*/index.jsx","default":"./dist/*/index.js"}}}"#,
        );
        assert_eq!(
            entry_target(&kobalte, "./dialog", &browser).as_deref(),
            Some("./dist/dialog/index.jsx")
        );
        let nested = ordered(
            r#"{"exports":{"./*":"./dist/*.js","./icons/*":"./icons/*.js","./exact":"./exact.js"}}"#,
        );
        assert_eq!(
            entry_target(&nested, "./icons/a", &browser).as_deref(),
            Some("./icons/a.js")
        );
        assert_eq!(
            entry_target(&nested, "./exact", &browser).as_deref(),
            Some("./exact.js")
        );
        assert!(entry_target(&nested, "./../escape", &browser).is_none());
    }

    #[test]
    fn the_declared_host_selects_the_entry() {
        // Review round 1: a fixed browser/development set read the source a
        // Node or production build never loads.
        let split = ordered(
            r#"{"exports":{"node":"./dist/node.js","development":{"solid":"./src/index.jsx"},"production":"./dist/prod.js","default":"./dist/index.js"}}"#,
        );
        assert_eq!(
            entry_target(&split, ".", &host(&["node", "development"])).as_deref(),
            Some("./dist/node.js")
        );
        assert_eq!(
            entry_target(&split, ".", &host(&["browser", "production"])).as_deref(),
            Some("./dist/prod.js")
        );
        assert_eq!(
            entry_target(&split, ".", &host(&["browser", "development"])).as_deref(),
            Some("./src/index.jsx")
        );
        // Without `exports`, a browser host reads `browser` before `module`,
        // and refuses a per-file `browser` map.
        let fields =
            ordered(r#"{"browser":"./browser.js","module":"./module.js","main":"./main.js"}"#);
        assert_eq!(
            entry_target(&fields, ".", &host(&["browser", "development"])).as_deref(),
            Some("./browser.js")
        );
        assert_eq!(
            entry_target(&fields, ".", &host(&["node", "development"])).as_deref(),
            Some("./module.js")
        );
        let remap = ordered(r#"{"browser":{"./a.js":"./b.js"},"module":"./module.js"}"#);
        assert!(entry_target(&remap, ".", &host(&["browser", "development"])).is_none());
        let empty = ordered(r#"{"browser":{},"module":"./module.js"}"#);
        assert_eq!(
            entry_target(&empty, ".", &host(&["browser", "development"])).as_deref(),
            Some("./module.js")
        );
    }
}
