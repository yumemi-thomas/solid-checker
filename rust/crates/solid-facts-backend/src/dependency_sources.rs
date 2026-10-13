//! Experimental dependency-source mode (ADR 0284): analyze the runtime source
//! of selected installed packages as if it were project code.
//!
//! TypeScript resolves an import of a package to its declarations, so the
//! package's JavaScript never enters the program. This module selects the
//! file a Solid browser build loads for each requested specifier (the
//! package's `exports` read with the `solid`, `browser`, `development`,
//! `import`, `module` and `default` conditions, in document order), walks its
//! relative imports to a closure, and writes a sibling tsconfig that extends
//! the project's and lists that closure as root files with `allowJs`. The
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

/// The conditions a Vite + `vite-plugin-solid` development build in the
/// browser sets, in no order: `exports` is read in its own document order.
const CONDITIONS: &[&str] = &[
    "solid",
    "browser",
    "development",
    "import",
    "module",
    "default",
];
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

/// Select, walk and write. `tsconfig` is the project's canonical tsconfig.
/// `None` when the project installs none of the packages: the analysis is
/// then the ordinary one.
pub fn prepare(
    tsconfig: &Path,
    specifiers: &[String],
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
        let relative = entry_target(&manifest, &subpath).ok_or_else(|| {
            format!(
                "--dependency-source {specifier:?}: no runtime entry for {subpath} under the solid/browser/import conditions"
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
    let base = fs::read_to_string(tsconfig).map_err(|error| error.to_string())?;
    // A child `files` with no inherited `include` would drop the project's
    // default `**/*`.
    let include = (!base.contains("\"include\"") && !base.contains("\"files\""))
        .then(|| serde_json::json!(["**/*"]));
    let mut document = serde_json::json!({
        "extends": format!("./{tsconfig_name}"),
        "compilerOptions": { "allowJs": true, "checkJs": false },
        "files": files.iter().map(|file| file.to_string_lossy().into_owned()).collect::<Vec<_>>(),
    });
    if let Some(include) = include {
        document["include"] = include;
    }
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

fn is_source(path: &Path) -> bool {
    let name = path.to_string_lossy();
    !name.ends_with(".d.ts")
        && !name.ends_with(".d.mts")
        && !name.ends_with(".d.cts")
        && SOURCE_EXTENSIONS
            .iter()
            .any(|extension| name.ends_with(extension))
}

/// The runtime target `exports` (or `module`/`main`) selects for `subpath`.
fn entry_target(manifest: &Ordered, subpath: &str) -> Option<String> {
    match manifest.get("exports") {
        Some(exports) => {
            let entry = match exports {
                Ordered::Object(map) if map.iter().any(|(key, _)| key.starts_with('.')) => {
                    exports.get(subpath)?
                }
                _ if subpath == "." => exports,
                _ => return None,
            };
            select(entry, 0)
        }
        None if subpath == "." => manifest
            .get("module")
            .or_else(|| manifest.get("main"))
            .and_then(Ordered::as_str)
            .map(str::to_owned),
        None => None,
    }
}

/// Node's conditional resolution: the first key, in document order, that is
/// a set condition and resolves.
fn select(entry: &Ordered, depth: usize) -> Option<String> {
    if depth > 16 {
        return None;
    }
    match entry {
        Ordered::String(target) if !target.contains('*') => Some(target.clone()),
        Ordered::Object(map) => map
            .iter()
            .filter(|(key, _)| CONDITIONS.contains(&key.as_str()))
            .find_map(|(_, value)| select(value, depth + 1)),
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
    use super::{entry_target, prepare, select};
    use crate::host_constants::Ordered;

    #[test]
    fn a_project_installing_none_of_the_packages_is_analyzed_as_usual() {
        let directory = std::env::temp_dir().join(format!(
            "solid-checker-dependency-sources-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let tsconfig = directory.join("tsconfig.json");
        std::fs::write(&tsconfig, "{}").unwrap();
        let prepared = prepare(&tsconfig, &["not-installed-here".to_owned()]).unwrap();
        let written = std::fs::read_dir(&directory).unwrap().count();
        std::fs::remove_dir_all(&directory).unwrap();
        assert!(prepared.is_none());
        assert_eq!(written, 1, "no sibling tsconfig is written");
    }

    fn ordered(json: &str) -> Ordered {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn conditions_are_read_in_document_order() {
        let router = ordered(
            r#"{"exports":{".":{"solid":"./dist/index.jsx","default":"./dist/index.js"}}}"#,
        );
        assert_eq!(
            entry_target(&router, ".").as_deref(),
            Some("./dist/index.jsx")
        );
        let tanstack = ordered(
            r#"{"exports":{".":{"solid":{"types":"./a.d.ts","default":"./dist/source/index.jsx"},"import":{"types":"./b.d.ts","default":"./dist/esm/index.js"}}}}"#,
        );
        assert_eq!(
            entry_target(&tanstack, ".").as_deref(),
            Some("./dist/source/index.jsx")
        );
        let node_first = ordered(r#"{"exports":{"node":"./node.js","import":"./browser.js"}}"#);
        assert_eq!(
            entry_target(&node_first, ".").as_deref(),
            Some("./browser.js")
        );
        let plain = ordered(r#"{"module":"./dist/index.js","main":"./dist/index.cjs"}"#);
        assert_eq!(
            entry_target(&plain, ".").as_deref(),
            Some("./dist/index.js")
        );
        assert!(select(&ordered(r#"{"require":"./a.cjs"}"#), 0).is_none());
    }
}
