//! Experimental dependency-source mode (ADR 0284): analyze the runtime source
//! of selected installed packages as if it were project code.
//!
//! TypeScript resolves an import of a package to its declarations, so the
//! package's JavaScript never enters the program. This module selects the
//! file a Vite + `vite-plugin-solid` build for the declared host loads for
//! each requested specifier (the package's `exports` read in document order
//! with `solid`, `import`, `module`, `default` and the host's own conditions:
//! its target, its build and every `--runtime-condition`), walks its
//! relative imports to a closure, and composes a sibling tsconfig that extends
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
//!
//! Preparation writes nothing (ADR 0296): [`prepare_program`] answers a
//! [`PreparedDependencyProgram`], a value a one-shot check writes under its
//! process id and a retained daemon compares against the one it holds before
//! reusing any work.

use std::collections::{BTreeSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::SourceFile;
use crate::host_constants::{Ordered, find_package_root, split_bare_specifier};
use solid_reactive_ir::package_boundary::{BindingForm, SelectionFailure, UnanalyzableReason};

/// The conditions every Vite + `vite-plugin-solid` build sets, beside the
/// host's: `exports` is read in its own document order.
const BUILD_CONDITIONS: &[&str] = &["solid", "import", "module", "default"];
/// The Vite plugins for Solid. Each adds `solid` to Vite's resolve
/// conditions, which is the resolver [`BUILD_CONDITIONS`] reads.
const SOLID_VITE_PLUGINS: &[&str] = &["@solidjs/vite-plugin", "vite-plugin-solid"];
/// Frameworks that ship one of [`SOLID_VITE_PLUGINS`] as their own
/// dependency rather than the project's.
const SOLID_VITE_FRAMEWORKS: &[&str] = &["@solidjs/start"];
const MAX_CLOSURE_FILES: usize = 4000;

/// Review round 1 of ADR 0297: the manifest of the Solid Vite plugin that
/// establishes this module's resolver premise for `project` (a tsconfig).
/// That is the nearest `node_modules/<plugin>/package.json` above the
/// project carrying a `version` string, walked as dialect detection walks
/// for `solid-js`, or else one a SolidStart install resolves from its own
/// directory. `None` when neither resolves: a declared target and build
/// alone do not say the project is built by Vite with the Solid plugin, so
/// nothing establishes that `exports` is read with `solid`, `import` and
/// `module` set.
///
/// Installed is all this reads. Whether `vite.config` registers the plugin,
/// or overrides `resolve.conditions`, is not observed.
#[must_use]
pub fn solid_vite_plugin(project: &Path) -> Option<PathBuf> {
    let start = project.parent()?;
    SOLID_VITE_PLUGINS
        .iter()
        .find_map(|plugin| nearest_versioned_manifest(start, plugin))
        .or_else(|| {
            SOLID_VITE_FRAMEWORKS.iter().find_map(|framework| {
                let manifest = nearest_versioned_manifest(start, framework)?;
                let directory = manifest.parent()?.canonicalize().ok()?;
                SOLID_VITE_PLUGINS
                    .iter()
                    .find_map(|plugin| nearest_versioned_manifest(&directory, plugin))
            })
        })
}

/// The nearest `node_modules/<package>/package.json` at or above `start`
/// whose `version` is a string; a manifest without one does not stop the
/// walk.
fn nearest_versioned_manifest(start: &Path, package: &str) -> Option<PathBuf> {
    start.ancestors().find_map(|directory| {
        let manifest = directory
            .join("node_modules")
            .join(package)
            .join("package.json");
        let bytes = fs::read(&manifest).ok()?;
        serde_json::from_slice::<serde_json::Value>(&bytes)
            .ok()?
            .get("version")?
            .as_str()?;
        Some(manifest)
    })
}

/// Vite's `resolve.extensions` order (`DEFAULT_EXTENSIONS`, minus `.json`):
/// a relative import that matches two files loads the first (review round 1
/// of ADR 0290, Sol: `./read` with `read.js` and `read.mjs` loads `.mjs`).
const SOURCE_EXTENSIONS: &[&str] = &[".mjs", ".js", ".mts", ".ts", ".jsx", ".tsx"];

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

/// One source-selection decision that left a package off the analyzed
/// program, recorded where it was made, beside the note that says so.
/// `detail` is the note's own reason text. Explanation only: the selection and
/// the notes are exactly what they were without it.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SourceRefusal {
    /// The package name the decision was about.
    pub name: String,
    /// The exact installed root, when the decision was about one copy; `None`
    /// when it applies to the name, every copy included.
    pub root: Option<PathBuf>,
    pub kind: UnanalyzableReason,
    pub detail: String,
}

/// What selection said: the notes a check prints, in order, and the decisions
/// behind them.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SelectionNotes {
    pub lines: Vec<String>,
    pub refusals: Vec<SourceRefusal>,
}

impl SelectionNotes {
    fn refuse(
        &mut self,
        name: &str,
        root: Option<&Path>,
        kind: UnanalyzableReason,
        detail: impl Into<String>,
    ) {
        let refusal = SourceRefusal {
            name: name.to_owned(),
            root: root.map(Path::to_path_buf),
            kind,
            detail: detail.into(),
        };
        if !self.refusals.contains(&refusal) {
            self.refusals.push(refusal);
        }
    }
}

/// The selected packages and their closure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DependencySelection {
    /// Canonical package directories.
    pub roots: Vec<PathBuf>,
    /// The selected packages' names.
    pub package_names: BTreeSet<String>,
    /// Canonical closure files.
    pub files: Vec<PathBuf>,
    pub resolutions: Vec<ImportResolution>,
}

/// Everything a dependency-source program is, before it is written (ADR
/// 0296): the selection, the project half of its root files, the host it was
/// selected for, and the sibling tsconfig's exact bytes. Two equal values
/// describe the same program, the same redirects and the same unredirected
/// removal; anything that differs is a different program.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedDependencyProgram {
    /// The project's canonical tsconfig, which the sibling extends.
    pub tsconfig: PathBuf,
    pub host: Host,
    /// The project's own program files outside `node_modules`.
    pub project_files: BTreeSet<PathBuf>,
    pub selection: DependencySelection,
    /// The sibling tsconfig's bytes.
    pub document: Vec<u8>,
}

impl PreparedDependencyProgram {
    /// Whether `path` lies inside one of the selected packages.
    #[must_use]
    pub fn contains(&self, path: &Path) -> bool {
        self.selection
            .roots
            .iter()
            .any(|root| path.starts_with(root))
    }

    /// The tsconfig path the program is opened at: beside the project's own,
    /// so it is anchored as the project is (its directory, default type
    /// roots, `${configDir}`). Nothing is written there; the producer reads
    /// its bytes from [`GeneratedProgram::config`] (`-project-config`).
    pub fn logical_path(&self) -> Result<PathBuf, String> {
        let directory = self
            .tsconfig
            .parent()
            .ok_or("--dependency-source: the project tsconfig has no directory")?;
        Ok(directory.join(".solid-checker-dependency-sources.tsconfig.json"))
    }

    /// Write the program's bytes to a scratch file under this process's id,
    /// as a one-shot check does. It is removed when the returned value drops.
    pub fn write_for_process(&self) -> Result<GeneratedProgram, String> {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_nanos());
        let config =
            scratch_directory()?.join(format!("{}-{nanos}.tsconfig.json", std::process::id()));
        fs::write(&config, &self.document).map_err(|error| {
            format!(
                "--dependency-source: cannot write {}: {error}",
                config.display()
            )
        })?;
        Ok(GeneratedProgram {
            logical: self.logical_path()?,
            config,
        })
    }

    /// Write the program's bytes to a scratch file named by `domain` and the
    /// bytes, as a retained daemon does. An existing file of that name is
    /// reused only when its bytes are these; it is never rewritten, and a
    /// missing one is created without replacing anything a concurrent writer
    /// made. Nothing removes it here: its owner does.
    pub fn write_retained(&self, domain: &[u8]) -> Result<(PathBuf, PathBuf), String> {
        let mut digest = Sha256::new();
        digest.update(domain);
        digest.update([0]);
        digest.update(self.tsconfig.as_os_str().as_encoded_bytes());
        digest.update([0]);
        digest.update(&self.document);
        let key = digest
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let written = scratch_directory()?.join(format!("{key}.tsconfig.json"));
        let matches = |path: &Path| fs::read(path).is_ok_and(|bytes| bytes == self.document);
        let logical = self.logical_path()?;
        if written.exists() {
            return if matches(&written) {
                Ok((logical, written))
            } else {
                Err(format!(
                    "--dependency-source: {} holds other bytes",
                    written.display()
                ))
            };
        }
        let staged = written.with_extension(format!("json.{}.tmp", std::process::id()));
        fs::write(&staged, &self.document).map_err(|error| {
            format!(
                "--dependency-source: cannot write {}: {error}",
                staged.display()
            )
        })?;
        // A hard link never replaces an existing file, so a concurrent
        // writer's copy is kept and then verified like any other.
        let linked = fs::hard_link(&staged, &written);
        let _ = fs::remove_file(&staged);
        match linked {
            Ok(()) => Ok((logical, written)),
            Err(_) if matches(&written) => Ok((logical, written)),
            Err(error) => Err(format!(
                "--dependency-source: cannot write {}: {error}",
                written.display()
            )),
        }
    }
}

/// Where generated program configurations are written: a scratch directory,
/// never the project (`SOLID_CHECKER_PROGRAM_DIR` overrides it).
fn scratch_directory() -> Result<PathBuf, String> {
    let directory = std::env::var_os("SOLID_CHECKER_PROGRAM_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("solid-checker-programs"));
    fs::create_dir_all(&directory).map_err(|error| {
        format!(
            "--dependency-source: cannot create {}: {error}",
            directory.display()
        )
    })?;
    Ok(directory)
}

/// The program a one-shot check opens: its logical tsconfig path, which is
/// never written, and the scratch file holding its bytes, removed when this
/// drops.
#[derive(Debug)]
pub struct GeneratedProgram {
    logical: PathBuf,
    config: PathBuf,
}

impl GeneratedProgram {
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.logical
    }

    /// The scratch file holding the program's bytes.
    #[must_use]
    pub fn config(&self) -> &Path {
        &self.config
    }
}

impl Drop for GeneratedProgram {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.config);
    }
}

/// The declared host a package entry is selected for.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Host {
    /// The host's conditions: `browser` or `node`, `development` or
    /// `production`, and every `--runtime-condition`.
    pub conditions: BTreeSet<String>,
    pub browser: bool,
}

/// Compose the dependency-source program from the project's own program:
/// its configured sources (`project_sources`) and every module it holds
/// (`program_modules`). `selection` is the `--dependency-source` list; `*`
/// selects every Solid package the project loads ([`prepare_all`]).
/// `excluded` and `ecosystem_scopes` are the dialect's runtime and scopes.
/// Notes are pushed to `notes` as they arise, so a caller can print them
/// even when this then refuses.
///
/// `None` when nothing is selected: the analysis is then the ordinary one.
#[allow(clippy::too_many_arguments)]
pub fn prepare_program(
    tsconfig: &Path,
    selection: &[String],
    host: &Host,
    project_sources: &[SourceFile],
    program_modules: &[PathBuf],
    excluded: &BTreeSet<String>,
    ecosystem_scopes: &[&str],
    notes: &mut SelectionNotes,
) -> Result<Option<PreparedDependencyProgram>, String> {
    // Configured sources omit declaration files, and an ambient `declare
    // module` stub is part of the project's program too: take every file the
    // program holds outside `node_modules`.
    let project_files = program_modules
        .iter()
        .cloned()
        .chain(
            project_sources
                .iter()
                .map(|source| PathBuf::from(&source.path)),
        )
        .filter(|path| {
            path.is_file()
                && !path
                    .components()
                    .any(|component| component.as_os_str() == "node_modules")
        })
        .collect::<BTreeSet<_>>();
    // Imported project barrels need not be configured roots (`files` may
    // name only App.tsx). Inspect the complete runtime project inventory.
    let project_imports = project_files
        .iter()
        .filter(|path| is_source(path) && !is_build_config(path))
        .map(|path| {
            let source = project_sources
                .iter()
                .find(|source| Path::new(&source.path) == path.as_path())
                .map(|source| source.source.clone())
                .or_else(|| fs::read_to_string(path).ok().map(Into::into));
            let facts = source.as_deref().and_then(|source| {
                solid_facts::ast::extract(path.to_string_lossy().into_owned(), source).ok()
            });
            let bare = facts.as_ref().map(runtime_bare_imports).unwrap_or_default();
            let unredirected = facts
                .as_ref()
                .map(unredirected_packages)
                .unwrap_or_else(|| UnsupportedBindings {
                    opaque: Some("a project module's runtime bindings could not be read"),
                    ..UnsupportedBindings::default()
                });
            (path.clone(), bare, unredirected)
        })
        .collect::<Vec<_>>();
    // ADR 0297 review rounds 1 and 2: keep off source every package reached
    // through a runtime binding form that symbol_redirects does not join to
    // the selected source, in project files and selected package closures
    // alike.
    let mut unredirected = project_imports.iter().fold(
        UnsupportedBindings::default(),
        |mut all, (_, _, bindings)| {
            all.extend(bindings.clone());
            all
        },
    );
    unredirected.runtime_packages = excluded.clone();
    unredirected
        .packages
        .retain(|name, _| !excluded.contains(name));
    // ADR 0291: `*` selects every package the project loads at run time,
    // and every package those load, except the dialect's own runtime.
    let selected = if selection.iter().any(|source| source == "*") {
        let requests = project_imports
            .iter()
            .flat_map(|(path, bare, _)| {
                bare.iter()
                    .map(|specifier| (path.clone(), specifier.clone()))
            })
            .collect::<Vec<_>>();
        prepare_all(
            requests,
            host,
            excluded,
            ecosystem_scopes,
            &mut unredirected,
            notes,
        )?
    } else {
        // Every subpath of a selected package the project loads is selected
        // too, so an unselected subpath sharing a declaration cannot be bound
        // to another subpath's source.
        let packages = selection
            .iter()
            .filter_map(|specifier| package_name(specifier))
            .collect::<BTreeSet<_>>();
        let mut specifiers = selection.iter().cloned().collect::<BTreeSet<_>>();
        for (_, bare, _) in &project_imports {
            specifiers.extend(subpaths_of(bare, &packages));
        }
        prepare(
            tsconfig,
            &specifiers.into_iter().collect::<Vec<_>>(),
            host,
            &mut unredirected,
            notes,
        )?
    };
    let Some(selection) = selected
        .and_then(|selection| without_unredirected_packages(selection, &unredirected, notes))
    else {
        return Ok(None);
    };
    let document = program_document(tsconfig, &project_files, &selection.files)?;
    Ok(Some(PreparedDependencyProgram {
        tsconfig: tsconfig.to_path_buf(),
        host: host.clone(),
        project_files,
        selection,
        document,
    }))
}

/// Select and walk. `tsconfig` is the project's canonical tsconfig. `None`
/// when the project installs none of the packages.
pub fn prepare(
    tsconfig: &Path,
    specifiers: &[String],
    host: &Host,
    unredirected: &mut UnsupportedBindings,
    notes: &mut SelectionNotes,
) -> Result<Option<DependencySelection>, String> {
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
            notes.lines.push(format!(
                "solid-checker: note: --dependency-source {specifier}: not installed here, skipped"
            ));
            notes.refuse(
                &name,
                None,
                UnanalyzableReason::ResolutionUnavailable,
                "not installed here",
            );
            continue;
        };
        if unredirected.contains(&name) {
            unredirected.refuse(notes, &name, Some(&root));
            continue;
        }
        let root = root
            .canonicalize()
            .map_err(|error| format!("--dependency-source {specifier:?}: {error}"))?;
        let entry = select_entry(&root, &subpath, host)
            .map_err(|(_, reason)| format!("--dependency-source {specifier:?}: {reason}"))?;
        resolutions.insert(ImportResolution {
            importer: None,
            specifier: specifier.clone(),
            target: entry.clone(),
        });
        walk(
            &root,
            &entry,
            &mut files,
            &mut resolutions,
            &mut Vec::new(),
            unredirected,
        )
        .map_err(|(_, reason)| format!("--dependency-source {specifier:?}: {reason}"))?;
        package_names.insert(name);
        if !roots.contains(&root) {
            roots.push(root);
        }
    }
    if roots.is_empty() {
        return Ok(None);
    }
    Ok(Some(DependencySelection {
        roots,
        package_names,
        files: files.into_iter().collect(),
        resolutions: resolutions.into_iter().collect(),
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
/// note and stays off source: nothing of it enters the program, and its
/// other copies are judged on their own.
pub fn prepare_all(
    requests: Vec<(PathBuf, String)>,
    host: &Host,
    excluded: &BTreeSet<String>,
    ecosystem_scopes: &[&str],
    unredirected: &mut UnsupportedBindings,
    notes: &mut SelectionNotes,
) -> Result<Option<DependencySelection>, String> {
    unredirected
        .runtime_packages
        .extend(excluded.iter().cloned());
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
        let Some(root) = runtime_package_root(&importer, &name).ok().flatten() else {
            continue;
        };
        if unsupported.contains(&root) || not_solid.contains(&root) {
            continue;
        }
        // A workspace package linked into `node_modules` resolves to the
        // project's own directory: it is project code, not a dependency.
        if !root
            .components()
            .any(|component| component.as_os_str() == "node_modules")
        {
            not_solid.insert(root);
            continue;
        }
        if !names_by_root.contains_key(&root) && !solid_package(&root) {
            notes.refuse(
                &name,
                Some(&root),
                UnanalyzableReason::NotSelected(SelectionFailure::NoDemand),
                "its manifest's dependencies and peerDependencies name no Solid package",
            );
            not_solid.insert(root);
            continue;
        }
        // A package reached through an unredirected binding stays off source
        // (`without_unredirected_packages`): one known to be is not walked.
        if unredirected.contains(&name) {
            unredirected.refuse(notes, &name, Some(&root));
            continue;
        }
        let entry = match select_entry(&root, &subpath, host) {
            Ok(entry) => entry,
            Err((kind, reason)) => {
                notes.lines.push(format!(
                    "solid-checker: note: dependency source {name} ({}): {reason}; it stays off source; {}",
                    root.display(), request_edge(&importer, &specifier)
                ));
                notes.refuse(&name, Some(&root), kind, reason);
                unsupported.insert(root);
                continue;
            }
        };
        names_by_root.insert(root.clone(), name);
        pending.push(ImportResolution {
            importer: Some(importer.clone()),
            specifier: specifier.clone(),
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
                unredirected,
            ) {
                Ok(()) => {
                    files.extend(closure);
                    resolutions.extend(closure_resolutions);
                    queue.extend(bare);
                }
                Err((kind, reason)) => {
                    notes.lines.push(format!(
                        "solid-checker: note: dependency source {} ({}): {reason}; it stays off source; {}",
                        names_by_root[&root],
                        root.display(), request_edge(&importer, &specifier)
                    ));
                    let name = names_by_root[&root].clone();
                    notes.refuse(&name, Some(&root), kind, reason);
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
    // Source is selected by package name, so a name with any copy that is
    // not analyzed (unsupported, not a Solid package, a workspace link) stays
    // off source for every copy (review round 1 of ADR 0290, Sol).
    let unsupported_names = unsupported
        .iter()
        .chain(&not_solid)
        .filter_map(|root| package_name_of_root(root))
        .collect::<BTreeSet<_>>();
    for (root, name) in &names_by_root {
        if unsupported_names.contains(name) {
            notes.refuse(
                name,
                Some(root),
                UnanalyzableReason::NotSelected(SelectionFailure::RetainedCopy),
                "another installed copy of this name is not analyzed, so it stays off source for every copy",
            );
        }
    }
    let package_names = names_by_root
        .values()
        .filter(|name| !unsupported_names.contains(*name))
        .cloned()
        .collect();
    Ok(Some(DependencySelection {
        roots: names_by_root.into_keys().collect(),
        package_names,
        files: files.into_iter().collect(),
        resolutions: resolutions.into_iter().collect(),
    }))
}

/// The `name` a package directory's manifest declares.
fn package_name_of_root(root: &Path) -> Option<String> {
    let manifest = fs::read(root.join("package.json")).ok()?;
    let manifest = serde_json::from_slice::<Ordered>(&manifest).ok()?;
    manifest.get("name")?.as_str().map(str::to_owned)
}

/// The source file `subpath` of the package at `root` loads for `host`.
fn select_entry(
    root: &Path,
    subpath: &str,
    host: &Host,
) -> Result<PathBuf, (UnanalyzableReason, String)> {
    let manifest = fs::read(root.join("package.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Ordered>(&bytes).ok())
        .ok_or_else(|| {
            (
                UnanalyzableReason::ManifestUnavailable,
                "unreadable package.json".to_owned(),
            )
        })?;
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
        return Err((
            UnanalyzableReason::RuntimeEntryUnavailable,
            "a `browser` field map is not supported".into(),
        ));
    }
    let relative = entry_target(&manifest, subpath, host).ok_or_else(|| {
        (
            UnanalyzableReason::RuntimeEntryUnavailable,
            format!("no runtime entry for {subpath} under the declared host's conditions"),
        )
    })?;
    let entry = root
        .join(relative.trim_start_matches("./"))
        .canonicalize()
        .map_err(|error| {
            (
                UnanalyzableReason::RuntimeSourceUnavailable,
                format!("entry {relative}: {error}"),
            )
        })?;
    if !entry.starts_with(root) || !is_source(&entry) {
        return Err((
            UnanalyzableReason::RuntimeSourceUnavailable,
            format!("entry {relative} is not a source file inside the package"),
        ));
    }
    Ok(entry)
}

/// The sibling tsconfig's bytes: the project's own program files plus the
/// closure, with an empty `include` (review round 2).
fn program_document(
    tsconfig: &Path,
    project_files: &BTreeSet<PathBuf>,
    files: &[PathBuf],
) -> Result<Vec<u8>, String> {
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
    serde_json::to_vec_pretty(&document).map_err(|error| error.to_string())
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
    subpaths_of(&runtime_bare_imports(&facts), packages)
}

/// The specifiers among `bare` that name one of `packages`.
fn subpaths_of(bare: &BTreeSet<String>, packages: &BTreeSet<String>) -> BTreeSet<String> {
    bare.iter()
        .filter(|module| package_name(module).is_some_and(|name| packages.contains(&name)))
        .cloned()
        .collect()
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
                .filter(|export| runtime_reexport(export))
                .filter_map(|export| export.module.as_deref()),
        )
        .chain(
            facts
                .import_equals
                .iter()
                .filter(|edge| !edge.type_only)
                .map(|edge| edge.module.as_str()),
        )
        .chain(
            facts
                .module_loads
                .iter()
                .filter_map(|edge| edge.specifier.as_deref()),
        )
        .filter(|module| split_bare_specifier(module).is_some())
        .map(str::to_owned)
        .collect()
}

/// Binding forms that `symbol_redirects` does not join to runtime exports.
/// Source is selected by package name, so one such edge keeps every copy of
/// that package off source, including otherwise redirectable imports.
#[derive(Clone, Debug, Default)]
pub struct UnsupportedBindings {
    packages: std::collections::BTreeMap<String, (UnanalyzableReason, String)>,
    opaque: Option<&'static str>,
    runtime_packages: BTreeSet<String>,
}

impl UnsupportedBindings {
    pub fn retain(&mut self, name: String, kind: UnanalyzableReason, reason: String) {
        self.packages.insert(name, (kind, reason));
    }
    fn contains(&self, name: &str) -> bool {
        !self.runtime_packages.contains(name)
            && (self.opaque.is_some() || self.packages.contains_key(name))
    }

    fn extend(&mut self, other: Self) {
        self.packages.extend(other.packages);
        self.opaque = self.opaque.or(other.opaque);
    }

    /// The decision behind [`Self::note`], in the same precedence.
    fn reason(&self, name: &str) -> (UnanalyzableReason, &str) {
        if let Some(opaque) = self.opaque {
            return (
                UnanalyzableReason::UnsupportedBinding(BindingForm::OpaqueInventory),
                opaque,
            );
        }
        self.packages.get(name).map_or(
            (
                UnanalyzableReason::NotSelected(SelectionFailure::RetainedCopy),
                "its source overlaps an unredirected package or shares that package's selection name",
            ),
            |(kind, reason)| (*kind, reason.as_str()),
        )
    }

    /// Pushes [`Self::note`] once and records the decision behind it.
    fn refuse(&self, notes: &mut SelectionNotes, name: &str, root: Option<&Path>) {
        push_note(&mut notes.lines, self.note(name));
        let (kind, reason) = self.reason(name);
        notes.refuse(name, root, kind, reason);
    }

    fn note(&self, name: &str) -> String {
        let (_, reason) = self.reason(name);
        format!(
            "solid-checker: note: dependency source {name}: {reason}, \
             which source analysis does not redirect; it stays off source"
        )
    }
}

/// The packages a module reaches through a binding no redirect binds, each of
/// which stays off source whole.
///
/// A runtime namespace *import* is not one of them any more (ADR 0313). It
/// kept its package off source for the sake of ADR 0297's namespace binding;
/// ADR 0312 removed that binding, so keeping the package off source bought
/// that import nothing and took source away from every named import of the
/// same package. The import
/// itself stays explicitly uncertifiable: the package boundary records a
/// `NamespaceValue` obligation for a namespace import of selected source
/// (`package_requirements`, review round 1 of ADR 0290).
fn unredirected_packages(facts: &solid_facts::ast::AstFacts) -> UnsupportedBindings {
    let mut bindings = UnsupportedBindings::default();
    for export in &facts.exports {
        if runtime_reexport(export)
            && export.namespace.is_some()
            && let Some(name) = export.module.as_deref().and_then(package_name)
        {
            bindings.packages.insert(
                name,
                (
                    UnanalyzableReason::UnsupportedBinding(BindingForm::NamespaceReexport),
                    binding_reason(
                        facts,
                        export.span,
                        "re-exported through a namespace binding",
                    ),
                ),
            );
        }
    }
    for import in &facts.import_equals {
        if !import.type_only
            && let Some(name) = package_name(&import.module)
        {
            bindings.packages.insert(
                name,
                (
                    UnanalyzableReason::UnsupportedBinding(BindingForm::ImportEquals),
                    binding_reason(
                        facts,
                        import.span,
                        "loaded through an import-equals binding",
                    ),
                ),
            );
        }
    }
    for load in &facts.module_loads {
        if let Some(name) = load.specifier.as_deref().and_then(package_name) {
            let (form, wording) = match load.kind {
                solid_facts::ast::ModuleLoadKind::DynamicImport => (
                    BindingForm::DynamicImport,
                    "loaded through a literal dynamic import binding",
                ),
                solid_facts::ast::ModuleLoadKind::Require => (
                    BindingForm::Require,
                    "loaded through a literal require binding",
                ),
            };
            bindings.packages.insert(
                name,
                (
                    UnanalyzableReason::UnsupportedBinding(form),
                    binding_reason(facts, load.span, wording),
                ),
            );
        }
    }
    bindings
}

fn binding_reason(
    facts: &solid_facts::ast::AstFacts,
    span: solid_facts::core::Span,
    form: &str,
) -> String {
    let line = fs::read_to_string(facts.source.path.as_str())
        .ok()
        .and_then(|source| {
            source
                .get(..span.start as usize)
                .map(|prefix| prefix.bytes().filter(|byte| *byte == b'\n').count() + 1)
        });
    format!(
        "{form} at {}:{}",
        facts.source.path.as_str(),
        line.map_or_else(|| format!("byte {}", span.start), |line| line.to_string())
    )
}

/// A refusal's requesting edge, not just the root whose source was refused.
fn request_edge(path: &Path, specifier: &str) -> String {
    let location = || {
        let source = fs::read_to_string(path).ok()?;
        let facts = solid_facts::ast::extract(path.to_string_lossy().into_owned(), &source).ok()?;
        let edge = facts
            .imports
            .iter()
            .filter(|edge| !edge.type_only && edge.module == specifier)
            .map(|edge| (edge.span, "static import"))
            .chain(
                facts
                    .exports
                    .iter()
                    .filter(|edge| {
                        runtime_reexport(edge) && edge.module.as_deref() == Some(specifier)
                    })
                    .map(|edge| (edge.span, "static re-export")),
            )
            .chain(
                facts
                    .import_equals
                    .iter()
                    .filter(|edge| !edge.type_only && edge.module == specifier)
                    .map(|edge| (edge.span, "import-equals")),
            )
            .chain(
                facts
                    .module_loads
                    .iter()
                    .filter(|edge| edge.specifier.as_deref() == Some(specifier))
                    .map(|edge| (edge.span, "literal module load")),
            )
            .next()?;
        Some(binding_reason(&facts, edge.0, edge.1))
    };
    location().unwrap_or_else(|| {
        format!(
            "runtime request {specifier:?} from {} could not be reread",
            path.display()
        )
    })
}

pub fn is_build_config(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with("vite.config.") || name.starts_with("vitest.config."))
}

fn runtime_reexport(export: &solid_facts::ast::ExportFact) -> bool {
    !export.type_only
        && (export.kind == solid_facts::ast::ExportKind::All
            || export
                .specifiers
                .iter()
                .any(|specifier| !specifier.type_only))
}

/// `selection` without the packages named in `unredirected`, every copy of
/// each: none of its files, redirects or names, so each stays off source
/// exactly as without this mode (review rounds 1 and 2 of ADR 0297). Source is
/// selected by package name, so a name is taken out whole. `None` when nothing
/// is left.
pub fn without_unredirected_packages(
    mut selection: DependencySelection,
    unredirected: &UnsupportedBindings,
    notes: &mut SelectionNotes,
) -> Option<DependencySelection> {
    if unredirected.opaque.is_some() {
        for name in &selection.package_names {
            unredirected.refuse(notes, name, None);
        }
        return None;
    }
    // Removal from the selection and source-root admission must disappear
    // together.
    // A root can overlap another installed root, or be loaded under an npm
    // alias. Use the selector's exact targets beside manifest names, and close
    // removal over overlapping roots and every copy sharing a removal name.
    let root_names = selection
        .roots
        .iter()
        .map(|root| {
            let mut names = package_name_of_root(root)
                .into_iter()
                .collect::<BTreeSet<_>>();
            names.extend(
                selection
                    .resolutions
                    .iter()
                    .filter(|resolution| resolution.target.starts_with(root))
                    .filter_map(|resolution| package_name(&resolution.specifier)),
            );
            (root.clone(), names)
        })
        .collect::<Vec<_>>();
    let mut removed = BTreeSet::<PathBuf>::new();
    let mut names = selection
        .package_names
        .iter()
        .filter(|name| unredirected.contains(name))
        .cloned()
        .collect::<BTreeSet<_>>();
    loop {
        let previous = (removed.len(), names.len());
        for (root, associated) in &root_names {
            if associated
                .iter()
                .any(|name| unredirected.contains(name) || names.contains(name))
                || removed
                    .iter()
                    .any(|other| root.starts_with(other) || other.starts_with(root))
            {
                removed.insert(root.clone());
                names.extend(associated.iter().cloned());
            }
        }
        if previous == (removed.len(), names.len()) {
            break;
        }
    }
    for name in &names {
        unredirected.refuse(notes, name, None);
    }
    let inside = |path: &Path| removed.iter().any(|root| path.starts_with(root));
    selection.roots.retain(|root| !removed.contains(root));
    selection.files.retain(|file| !inside(file));
    selection.resolutions.retain(|resolution| {
        !inside(&resolution.target) && !resolution.importer.as_deref().is_some_and(inside)
    });
    selection.package_names.retain(|name| !names.contains(name));
    (!selection.roots.is_empty()).then_some(selection)
}

fn push_note(notes: &mut Vec<String>, note: String) {
    if !notes.contains(&note) {
        notes.push(note);
    }
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

/// Vite's conditional resolution (`resolve.exports` 2.0.3, which Vite 8
/// vendors): the first key, in document order, that is a set condition
/// decides, whatever its value selects. `star` is a pattern match's text.
///
/// `None` when that value is a form this does not read -- an array, `null`,
/// a nested object no set condition of which resolves -- and the selection is
/// then refused, never continued at a later key (review round 1 of ADR 0295:
/// Vite reads `{"browser": ["./cycle.js"], "default": "./leaf.js"}` as
/// `./cycle.js`, and falling through to `default` hid an import cycle). Node
/// would try a later key after a nested object that resolves nothing; Vite
/// does not, so neither answer is claimed.
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
            .find(|(key, _)| {
                BUILD_CONDITIONS.contains(&key.as_str()) || host.conditions.contains(key.as_str())
            })
            .and_then(|(_, value)| select(value, host, depth + 1, star)),
        _ => None,
    }
}

/// Every file `entry` reaches through relative static imports and
/// re-exports, inside `root`, the bare runtime imports of those files into
/// `bare`, and their unsupported runtime bindings into `unredirected`.
fn walk(
    root: &Path,
    entry: &Path,
    files: &mut BTreeSet<PathBuf>,
    resolutions: &mut BTreeSet<ImportResolution>,
    bare: &mut Vec<(PathBuf, String)>,
    unredirected: &mut UnsupportedBindings,
) -> Result<(), (UnanalyzableReason, String)> {
    let unavailable = |reason: String| (UnanalyzableReason::ClosureUnavailable, reason);
    let mut queue = VecDeque::from([entry.to_path_buf()]);
    while let Some(file) = queue.pop_front() {
        if !files.insert(file.clone()) {
            continue;
        }
        if files.len() > MAX_CLOSURE_FILES {
            return Err((
                UnanalyzableReason::ClosureLimit,
                format!("closure exceeds {MAX_CLOSURE_FILES} files"),
            ));
        }
        let source = fs::read_to_string(&file)
            .map_err(|error| unavailable(format!("{}: {error}", file.display())))?;
        let facts = solid_facts::ast::extract(file.to_string_lossy().into_owned(), &source)
            .map_err(|error| unavailable(format!("{}: {error}", file.display())))?;
        bare.extend(
            runtime_bare_imports(&facts)
                .into_iter()
                .map(|module| (file.clone(), module)),
        );
        unredirected.extend(unredirected_packages(&facts));
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
            .chain(
                facts
                    .import_equals
                    .iter()
                    .filter(|edge| !edge.type_only)
                    .map(|edge| edge.module.as_str()),
            )
            .chain(
                facts
                    .module_loads
                    .iter()
                    .filter_map(|edge| edge.specifier.as_deref()),
            )
            .filter(|module| module.starts_with("./") || module.starts_with("../"))
            .map(str::to_owned)
            .collect::<BTreeSet<_>>();
        let directory = file
            .parent()
            .ok_or_else(|| unavailable("closure file has no directory".to_owned()))?;
        for module in modules {
            let Some(target) = resolve_relative(directory, &module) else {
                return Err(unavailable(format!(
                    "{} imports {module:?}, which resolves to no source file",
                    file.display()
                )));
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

/// ADR 0295: whether `module` is outside every static import cycle, as a
/// Vite build for `host` loads it: no module its static loads reach,
/// transitively, loads it back. A module in a cycle can be entered by a
/// module that runs before its body, which may then call its hoisted
/// functions before a module-level `var` is initialized.
///
/// Every load is followed as [`prepare`] follows one -- a relative import to
/// the source file it names, a bare one through package self-reference, then
/// Node's lookup, and the package's `exports` for the host
/// ([`runtime_package_root`], [`select`]) -- into every package, selected or
/// not.
/// `false` whenever a load on the way cannot be followed (an unreadable or
/// unparsable file, a CommonJS `require`, an `import.meta.glob`, a bare
/// specifier no installed package answers, an `exports` form this does not
/// read, a non-source target) or the walk exceeds [`MAX_CLOSURE_FILES`]: an
/// unknown edge may close a cycle. A `node:` builtin loads no package code.
#[must_use]
pub fn outside_import_cycles(module: &Path, host: &Host) -> bool {
    let Ok(module) = module.canonicalize() else {
        return false;
    };
    let Some(first) = module_loads(&module, host) else {
        return false;
    };
    let mut seen = BTreeSet::new();
    let mut queue = VecDeque::from(first);
    while let Some(file) = queue.pop_front() {
        if file == module {
            return false;
        }
        if !seen.insert(file.clone()) {
            continue;
        }
        if seen.len() > MAX_CLOSURE_FILES {
            return false;
        }
        let Some(loads) = module_loads(&file, host) else {
            return false;
        };
        queue.extend(loads);
    }
    true
}

/// The canonical files `file`'s static loads evaluate before it, for
/// [`outside_import_cycles`]; `None` when one cannot be followed.
fn module_loads(file: &Path, host: &Host) -> Option<Vec<PathBuf>> {
    let source = fs::read_to_string(file).ok()?;
    let specifiers = solid_facts::ast::module_load_specifiers(file, &source)?;
    let directory = file.parent()?;
    let mut loads = Vec::with_capacity(specifiers.len());
    for specifier in specifiers {
        if specifier.starts_with("node:") {
            continue;
        }
        if specifier.starts_with("./") || specifier.starts_with("../") {
            loads.push(resolve_relative(directory, &specifier)?);
            continue;
        }
        let (name, subpath) = split_bare_specifier(&specifier)?;
        let root = runtime_package_root(file, &name).ok()??;
        loads.push(select_entry(&root, &subpath, host).ok()?);
    }
    Some(loads)
}

/// The canonical package directory a bare import of `name` from `importer`
/// loads, as Vite's `tryNodeResolve` finds it: the package's own directory
/// when the nearest `package.json` above `importer` is named `name` and has
/// `exports` (Node's package self-reference), and otherwise the nearest
/// `node_modules/<name>`. `Ok(None)` when no installed package answers;
/// `Err` when the nearest manifest cannot be read, so whether it refers to
/// itself is unknown (review round 1 of ADR 0295: inside `p`, `import "p"`
/// is `p` itself, not a nested `node_modules/p`).
fn runtime_package_root(importer: &Path, name: &str) -> Result<Option<PathBuf>, ()> {
    let mut directory = importer.parent();
    while let Some(current) = directory {
        let manifest = current.join("package.json");
        if manifest.is_file() {
            let manifest = fs::read(&manifest)
                .ok()
                .and_then(|bytes| serde_json::from_slice::<Ordered>(&bytes).ok())
                .ok_or(())?;
            if manifest.get("name").and_then(Ordered::as_str) == Some(name) {
                match manifest.get("exports") {
                    // No `exports` (or an empty string): no self-reference.
                    None => {}
                    Some(Ordered::String(target)) if target.is_empty() => {}
                    // `null` and `false` refer to nothing, an array or a
                    // number to itself: this does not tell them apart.
                    Some(Ordered::Other) => return Err(()),
                    Some(_) => return current.canonicalize().map(Some).map_err(|_| ()),
                }
            }
            break;
        }
        directory = current.parent();
    }
    Ok(find_package_root(importer, name, None).and_then(|root| root.canonicalize().ok()))
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
mod tests;
