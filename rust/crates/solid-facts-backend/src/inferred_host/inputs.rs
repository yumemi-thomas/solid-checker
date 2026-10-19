//! Discovery inputs: the paths, identities and digests a daemon fingerprints before its shortcut, and their validation.

use super::*;

/// Presence is an input too. Daemons retain this closure and fingerprint it
/// before their shortcut; discovery itself runs only on a cache miss.
#[must_use]
pub fn inferred_host_input_paths(directory: &Path) -> Vec<PathBuf> {
    inferred_host_input_paths_for_project(directory, &directory.join("tsconfig.json"))
}

/// Include the analyzed config and its exact inheritance chain. Application
/// configs may have names such as tsconfig.app.json.
#[must_use]
pub fn inferred_host_input_paths_for_project(directory: &Path, project: &Path) -> Vec<PathBuf> {
    inferred_host_input_paths_for_sources(directory, project, &[])
}

/// Daemons also know explicit project sources omitted by the ordinary inventory.
#[must_use]
pub fn inferred_host_input_paths_for_sources(
    directory: &Path,
    project: &Path,
    sources: &[PathBuf],
) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if application_inputs(directory, project, &mut paths).is_err() {
        paths.sort();
        paths.dedup();
        return paths;
    }
    let config = match configuration(directory, &mut paths) {
        Ok(config) => config,
        Err(_) => {
            paths.sort();
            paths.dedup();
            return paths;
        }
    };
    inferred_host_graph_input_paths(directory, project, paths, &config, None, None, sources)
}

pub(super) fn inferred_host_graph_input_paths(
    directory: &Path,
    project: &Path,
    mut paths: Vec<PathBuf>,
    resource_config: &Config,
    facts: Option<&ProjectFacts>,
    inventory: Option<Vec<PathBuf>>,
    source_paths: &[PathBuf],
) -> Vec<PathBuf> {
    paths.extend([
        directory.join("package.json"),
        project.to_owned(),
        directory.join("index.html"),
    ]);
    paths.extend(
        directory
            .ancestors()
            .skip(1)
            .map(|ancestor| ancestor.join("package.json")),
    );
    paths.extend(CONFIGS.iter().map(|name| directory.join(name)));
    let _ = tsconfig_inputs(project, &mut paths, &mut BTreeSet::new());
    for stem in [
        "entry-client",
        "entry-server",
        "App",
        "app",
        "Document",
        "middleware",
    ] {
        for extension in ["tsx", "jsx", "ts", "js", "mjs", "tsrx"] {
            paths.push(directory.join(format!("src/{stem}.{extension}")));
        }
    }
    if let Some(inventory) = inventory {
        paths.extend(inventory);
    } else if CONFIGS.iter().any(|name| directory.join(name).exists())
        || directory.join("index.html").exists()
    {
        let _ = inventory_paths(directory, &mut paths);
    }
    let observed = facts
        .into_iter()
        .flat_map(|facts| &facts.files)
        .map(|file| (file.path.as_str(), file))
        .collect::<BTreeMap<_, _>>();
    // Plugin implementations are audited as an installed dependency closure.
    // They are not application importers. Preserve every plugin fingerprint,
    // but resolve requests only for app inventory and actual analyzed files.
    let sources = paths
        .iter()
        .filter(|path| {
            path.starts_with(directory)
                && !path
                    .components()
                    .any(|part| part.as_os_str() == "node_modules")
        })
        .cloned()
        .chain(observed.keys().map(|path| PathBuf::from(*path)))
        .chain(source_paths.iter().cloned())
        .collect::<BTreeSet<_>>();
    let host_constant_ancestors = sources
        .iter()
        .filter_map(|source| source.parent())
        .flat_map(Path::ancestors)
        .filter(|ancestor| ancestor.starts_with(directory))
        .map(Path::to_path_buf)
        .collect::<BTreeSet<_>>();
    // The narrow provably-unloadable ~/ request proof also observes package
    // candidates above the application. Cover it before daemon cache reuse.
    // Each distinct ancestor directory once: walking every path's ancestors
    // was quadratic in the input set and dominated whole runs.
    paths.sort();
    paths.dedup();
    let ancestors = sources
        .iter()
        .filter_map(|source| source.parent())
        .flat_map(Path::ancestors)
        .map(Path::to_path_buf)
        .collect::<BTreeSet<_>>();
    for ancestor in &ancestors {
        // Importer/enclosing browser maps are file-selection inputs, including
        // absent nested manifests and manifests above the application.
        paths.push(ancestor.join("package.json"));
        paths.extend(
            ["~", "~.js", "~.json", "~.node"]
                .iter()
                .map(|name| ancestor.join("node_modules").join(name)),
        );
    }
    // Static package linking consumes manifests and selected client files even
    // when no behavior summary exists. Observe the same candidates before a
    // daemon shortcut, including absent/shadowing installs and store symlinks.
    let mut resolved_requests = BTreeSet::new();
    let mut package_loads = crate::host_loadable::PackageLoads::default();
    for source in sources {
        if !source
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|value| {
                ["ts", "tsx", "mts", "cts", "js", "jsx", "mjs", "cjs"].contains(&value)
            })
        {
            continue;
        }
        let observed = source.to_str().and_then(|path| observed.get(path));
        // A light parse gives the same specifiers; full extraction only when
        // it declines, so the input set never shrinks.
        let requests = if let Some(file) = observed {
            Some(static_fact_requests(&file.ast))
        } else if let Ok(text) = fs::read_to_string(&source) {
            solid_facts::ast::static_module_requests(&source, &text).or_else(|| {
                let ast =
                    solid_facts::ast::extract(source.to_string_lossy().into_owned(), &text).ok()?;
                Some(static_fact_requests(&ast))
            })
        } else {
            None
        };
        let Some(requests) = requests else {
            continue;
        };
        for text in requests.iter().map(String::as_str) {
            // Resolution depends on the importer directory and request, not
            // the source filename. Resource reads keep their occurrence path.
            let _ = crate::host_loadable::resource(
                directory,
                &source,
                text,
                resource_config,
                &mut paths,
            );
            for importer in [&source, &directory.join("index.ts")] {
                if resolved_requests
                    .insert((importer.parent().map(Path::to_path_buf), text.to_owned()))
                    && let Some((_, _, entries)) =
                        package_loads.candidates(importer, text, &mut paths)
                {
                    let _ = crate::host_loadable::stylesheet_closure(
                        &entries,
                        resource_config,
                        &mut paths,
                    );
                }
            }
        }
    }
    // Presence and content of every possible host-constant package are part
    // of the pre-session daemon shortcut too, not just the final manifest.
    {
        for ancestor in &host_constant_ancestors {
            for candidate in solid_dialect::DIALECTS
                .iter()
                .flat_map(|dialect| dialect.host_boolean_exports())
            {
                let package = ancestor.join("node_modules").join(candidate.module);
                paths.push(package.join("package.json"));
                paths.extend(
                    candidate
                        .runtime_targets
                        .iter()
                        .map(|target| package.join(target)),
                );
            }
        }
    }
    paths.sort();
    paths.dedup();
    paths
}

pub(super) fn static_fact_requests(ast: &solid_facts::ast::AstFacts) -> Vec<String> {
    ast.imports
        .iter()
        .filter(|import| !import.type_only)
        .map(|import| import.module.to_string())
        .chain(
            ast.exports
                .iter()
                .filter(|export| !export.type_only)
                .filter_map(|export| export.module.as_deref().map(str::to_owned)),
        )
        .collect()
}

pub(super) fn identity(path: &Path) -> Option<String> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => Some(metadata),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Some("absent".into()),
        Err(_) => None,
    };
    if metadata
        .as_ref()
        .is_some_and(|metadata| metadata.file_type().is_symlink())
    {
        return Some(format!(
            "symlink:{}:{}",
            fs::read_link(path).ok()?.display(),
            fs::canonicalize(path).ok()?.display()
        ));
    }
    if metadata
        .as_ref()
        .map_or_else(|| path.is_dir(), fs::Metadata::is_dir)
    {
        let mut entries = fs::read_dir(path)
            .ok()?
            .map(|entry| {
                let entry = entry.ok()?;
                let name = entry.file_name().to_str()?.to_owned();
                if name == ".git" || name == "node_modules" {
                    return Some(None);
                }
                let kind = entry.file_type().ok()?;
                Some(Some((
                    name,
                    kind.is_dir(),
                    kind.is_file(),
                    kind.is_symlink(),
                )))
            })
            .collect::<Option<Vec<_>>>()?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        entries.sort();
        return Some(format!(
            "directory:{}:{:x}",
            fs::canonicalize(path).ok()?.display(),
            Sha256::digest(serde_json::to_vec(&entries).ok()?)
        ));
    }
    match fs::read(path) {
        Ok(bytes) => Some(format!("sha256:{:x}", Sha256::digest(bytes))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Some("absent".into()),
        Err(_) => None,
    }
}

/// One graph pass observes each input once. Its manifest is re-read at every
/// transaction boundary, and the post-manifest graph has a fresh observation
/// map, so this never substitutes an earlier generation's bytes for validation.
pub(super) fn graph_identity(manifest: &mut BrowserRootManifest, path: &Path) -> Option<String> {
    let key = path.to_str()?;
    if let Some(observed) = manifest.inputs.get(key) {
        return Some(observed.clone());
    }
    let observed = identity(path)?;
    manifest.inputs.insert(key.into(), observed.clone());
    Some(observed)
}

// Every pass still reads current bytes and path identities. Bound parallel I/O
// only; never reuse an earlier pass's hash or a metadata-only observation.
pub(crate) fn identities(paths: &[&Path]) -> Vec<Option<String>> {
    let workers = std::thread::available_parallelism()
        .map_or(1, usize::from)
        .min(8);
    if paths.len() < 128 || workers == 1 {
        return paths.iter().map(|path| identity(path)).collect();
    }
    std::thread::scope(|scope| {
        // Interleave paths so a large package's adjacent files cannot all land
        // on one worker. Preserve the caller's order when joining observations.
        let jobs = (0..workers)
            .map(|worker| {
                scope.spawn(move || {
                    (worker..paths.len())
                        .step_by(workers)
                        .map(|index| (index, identity(paths[index])))
                        .collect::<Vec<_>>()
                })
            })
            .collect::<Vec<_>>();
        let mut observations = jobs
            .into_iter()
            .flat_map(|job| job.join().expect("input fingerprint worker panicked"))
            .collect::<Vec<_>>();
        observations.sort_by_key(|(index, _)| *index);
        observations
            .into_iter()
            .map(|(_, identity)| identity)
            .collect()
    })
}

/// Directory membership is a cache input, independently of existing file
/// contents. A new excluded server importer must invalidate a browser answer.
#[must_use]
pub fn inferred_host_directory_digest(path: &Path) -> Option<[u8; 32]> {
    inferred_host_input_digest(path)
}

/// Content, absence, directory membership, and symlink identity all matter.
/// Metadata alone is never sufficient to reuse inferred authority.
#[must_use]
pub fn inferred_host_input_digest(path: &Path) -> Option<[u8; 32]> {
    Some(Sha256::digest(identity(path)?.as_bytes()).into())
}

pub(super) fn inventory_paths(directory: &Path, paths: &mut Vec<PathBuf>) -> Option<()> {
    paths.push(directory.to_path_buf());
    for entry in fs::read_dir(directory).ok()? {
        let entry = entry.ok()?;
        if entry.file_name() == ".git" || entry.file_name() == "node_modules" {
            continue;
        }
        let kind = entry.file_type().ok()?;
        if kind.is_symlink() {
            paths.push(entry.path());
            continue;
        }
        if kind.is_dir() {
            inventory_paths(&entry.path(), paths)?;
        } else {
            let path = entry.path();
            if path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| {
                    [
                        "ts", "tsx", "tsrx", "mts", "cts", "js", "jsx", "mjs", "cjs", "json",
                        "html", "css", "pcss", "postcss", "less", "sass", "scss", "styl", "stylus",
                        "sss",
                    ]
                    .contains(&extension)
                })
            {
                paths.push(path);
            }
        }
    }
    Some(())
}

pub(super) fn input_manifest(
    directory: &Path,
    facts: &ProjectFacts,
    config: &Config,
    inputs: Vec<PathBuf>,
) -> Result<BTreeMap<String, String>, DiscoveryRefusal> {
    let _stage = HostStage::new("manifest-identity");
    let mut inventory = Vec::new();
    inventory_paths(directory, &mut inventory)
        .required(directory, "cannot inventory application inputs")?;
    let mut paths = inferred_host_graph_input_paths(
        directory,
        Path::new(&facts.project_id),
        inputs,
        config,
        Some(facts),
        Some(inventory),
        &[],
    );
    // The graph-input pass inventories the application. Omitted
    // implementations remain inputs, never evidence against a positive edge.
    let mut source_identities = BTreeMap::new();
    for file in &facts.files {
        let path = Path::new(file.path.as_str());
        if fs::canonicalize(path)
            .ok()
            .required(path, "cannot resolve analyzed source")?
            .as_path()
            != path
        {
            return Err(DiscoveryRefusal::new(
                path,
                "analyzed source path is not canonical",
            ));
        }
        paths.push(path.to_path_buf());
        for ancestor in path
            .parent()
            .required(path, "analyzed source has no parent")?
            .ancestors()
        {
            paths.push(ancestor.join("package.json"));
        }
        let source = fs::read_to_string(path)
            .ok()
            .required(path, "cannot read analyzed source")?;
        if solid_facts::core::SourceHash::of(&source) != file.source_hash {
            return Err(DiscoveryRefusal::new(path, "analyzed source bytes changed"));
        }
        source_identities.insert(path.to_path_buf(), file.source_hash.as_str().to_owned());
    }
    paths.sort();
    paths.dedup();
    let fingerprints = identities(&paths.iter().map(PathBuf::as_path).collect::<Vec<_>>());
    paths
        .into_iter()
        .zip(fingerprints)
        .map(|(path, fingerprint)| {
            Ok((
                path.to_str()
                    .required(&path, "non-UTF-8 input path")?
                    .to_owned(),
                source_identities
                    .remove(&path)
                    .or(fingerprint)
                    .required(&path, "cannot fingerprint input")?,
            ))
        })
        .collect()
}

/// Called before/after solving. Stale inferred authority is refused, never
/// attached to facts from another input generation.
pub(crate) fn validate_inputs(index: &ProjectHostIndex) -> Result<(), BackendError> {
    let _stage = HostStage::new("input-validation");
    let paths = index
        .manifest
        .inputs
        .keys()
        .map(Path::new)
        .collect::<Vec<_>>();
    for ((_, expected), actual) in index.manifest.inputs.iter().zip(identities(&paths)) {
        if actual.as_ref() != Some(expected) {
            return Err(BackendError::RuntimePremise(
                "inferred browser inputs changed; retry analysis".into(),
            ));
        }
    }
    Ok(())
}

pub(super) fn unpublished(package: &serde_json::Value) -> bool {
    ["exports", "main", "module", "bin", "types", "typings"]
        .iter()
        .all(|key| package.get(*key).is_none_or(serde_json::Value::is_null))
}
