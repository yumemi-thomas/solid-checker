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
//! not read (a pattern, an array, a condition this does not set leading to
//! nothing), or a closure larger than [`MAX_CLOSURE_FILES`]. Nothing here
//! guesses a file.

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
    let directory = tsconfig
        .parent()
        .ok_or("--dependency-source: the project tsconfig has no directory")?;
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
        let manifest = fs::read(root.join("package.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Ordered>(&bytes).ok())
            .ok_or_else(|| format!("--dependency-source {specifier:?}: unreadable package.json"))?;
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
            return Err(format!(
                "--dependency-source {specifier:?}: a `browser` field map is not supported"
            ));
        }
        let relative = entry_target(&manifest, &subpath, host).ok_or_else(|| {
            format!(
                "--dependency-source {specifier:?}: no runtime entry for {subpath} under the declared host's conditions"
            )
        })?;
        let entry = root
            .join(relative.trim_start_matches("./"))
            .canonicalize()
            .map_err(|error| {
                format!("--dependency-source {specifier:?}: entry {relative}: {error}")
            })?;
        if !entry.starts_with(&root) || !is_source(&entry) {
            return Err(format!(
                "--dependency-source {specifier:?}: entry {relative} is not a source file inside the package"
            ));
        }
        resolutions.insert(ImportResolution {
            importer: None,
            specifier: specifier.clone(),
            target: entry.clone(),
        });
        walk(&root, &entry, &mut files, &mut resolutions, specifier)?;
        package_names.insert(name);
        roots.push(root);
    }
    if roots.is_empty() {
        return Ok(None);
    }
    let tsconfig_name = tsconfig
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("--dependency-source: the project tsconfig name is not UTF-8")?;
    let root_files = project_files
        .iter()
        .chain(&files)
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
    Ok(Some(DependencySources {
        roots,
        package_names,
        files: files.into_iter().collect(),
        resolutions: resolutions.into_iter().collect(),
        tsconfig: written,
    }))
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
    let selected = |module: &str| package_name(module).is_some_and(|name| packages.contains(&name));
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
        .filter(|module| selected(module))
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
            let entry = match exports {
                Ordered::Object(map) if map.iter().any(|(key, _)| key.starts_with('.')) => {
                    exports.get(subpath)?
                }
                _ if subpath == "." => exports,
                _ => return None,
            };
            select(entry, host, 0)
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

/// Node's conditional resolution: the first key, in document order, that is
/// a set condition and resolves.
fn select(entry: &Ordered, host: &Host, depth: usize) -> Option<String> {
    if depth > 16 {
        return None;
    }
    match entry {
        Ordered::String(target) if !target.contains('*') => Some(target.clone()),
        Ordered::Object(map) => map
            .iter()
            .filter(|(key, _)| {
                BUILD_CONDITIONS.contains(&key.as_str()) || host.conditions.contains(key.as_str())
            })
            .find_map(|(_, value)| select(value, host, depth + 1)),
        _ => None,
    }
}

/// Every file `entry` reaches through relative static imports and
/// re-exports, inside `root`.
fn walk(
    root: &Path,
    entry: &Path,
    files: &mut BTreeSet<PathBuf>,
    resolutions: &mut BTreeSet<ImportResolution>,
    specifier: &str,
) -> Result<(), String> {
    let mut queue = VecDeque::from([entry.to_path_buf()]);
    while let Some(file) = queue.pop_front() {
        if !files.insert(file.clone()) {
            continue;
        }
        if files.len() > MAX_CLOSURE_FILES {
            return Err(format!(
                "--dependency-source {specifier:?}: closure exceeds {MAX_CLOSURE_FILES} files"
            ));
        }
        let source =
            fs::read_to_string(&file).map_err(|error| format!("{}: {error}", file.display()))?;
        let facts = solid_facts::ast::extract(file.to_string_lossy().into_owned(), &source)
            .map_err(|error| format!("{}: {error}", file.display()))?;
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
                    "--dependency-source {specifier:?}: {} imports {module:?}, which resolves to no source file",
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
        assert!(select(&ordered(r#"{"require":"./a.cjs"}"#), &browser, 0).is_none());
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
