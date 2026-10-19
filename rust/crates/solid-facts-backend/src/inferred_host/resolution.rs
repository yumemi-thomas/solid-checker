//! Local module resolution: normalized paths, tsconfig paths, aliases and Vite default extensions.

use super::*;

pub(super) fn normalized(path: &Path) -> Option<PathBuf> {
    let mut result = PathBuf::new();
    for part in path.components() {
        match part {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                if !result.pop() {
                    return None;
                }
            }
            other => result.push(other.as_os_str()),
        }
    }
    Some(result)
}

pub(super) fn paths_targets(
    directory: &Path,
    project: &Path,
) -> Result<BTreeMap<String, Vec<String>>, DiscoveryRefusal> {
    let mut inputs = Vec::new();
    tsconfig_inputs(project, &mut inputs, &mut BTreeSet::new())?;
    let mut base = directory.to_path_buf();
    let mut paths = BTreeMap::new();
    // tsconfig_inputs records the derived config before its base.
    for input in inputs.into_iter().rev() {
        let source = fs::read_to_string(&input)
            .ok()
            .required(&input, "cannot read inherited path mappings")?;
        let value = host_config::jsonc(&source).required(&input, "invalid path-mapping JSONC")?;
        let Some(options) = value.get("compilerOptions") else {
            continue;
        };
        if let Some(value) = options.get("baseUrl") {
            base = normalized(
                &input
                    .parent()
                    .required(&input, "path-mapping config has no parent")?
                    .join(value.as_str().required(&input, "baseUrl is not a string")?),
            )
            .required(&input, "baseUrl escapes its filesystem root")?;
        }
        if let Some(value) = options.get("paths") {
            paths.clear();
            let origin = if options.get("baseUrl").is_some() || base != directory {
                &base
            } else {
                input
                    .parent()
                    .required(&input, "paths config has no parent")?
            };
            for (key, values) in value
                .as_object()
                .required(&input, "paths is not an object")?
            {
                let targets = values
                    .as_array()
                    .required(&input, "paths target list is not an array")?
                    .iter()
                    .map(|value| {
                        let target = normalized(
                            &origin.join(
                                value
                                    .as_str()
                                    .required(&input, "paths target is not a string")?,
                            ),
                        )
                        .required(&input, "paths target escapes its filesystem root")?;
                        Ok(target
                            .to_str()
                            .required(&input, "non-UTF-8 paths target")?
                            .to_owned())
                    })
                    .collect::<Result<Vec<_>, DiscoveryRefusal>>()?;
                paths.insert(key.clone(), targets);
            }
        }
    }
    Ok(paths)
}

pub(super) fn congruent_alias(
    config: &Config,
    paths: &BTreeMap<String, Vec<String>>,
    specifier: &str,
) -> bool {
    let aliases = config
        .aliases
        .iter()
        .filter(|(find, _)| {
            specifier == find.as_str()
                || specifier
                    .strip_prefix(find.as_str())
                    .is_some_and(|tail| tail.starts_with('/'))
        })
        .collect::<Vec<_>>();
    if aliases.is_empty() {
        return true;
    }
    // Vite's explicit alias plugin precedes user pre-plugins, including
    // vite-tsconfig-paths. Its presence cannot authorize a conflicting rewrite.
    // Multiple matching Vite entries require ordering/priority facts, not guessing.
    let [(find, Some(replacement))] = aliases.as_slice() else {
        return false;
    };
    if !Path::new(replacement).is_absolute() {
        return false;
    }
    let Some(vite) = normalized(Path::new(&format!(
        "{replacement}{}",
        &specifier[find.len()..]
    ))) else {
        return false;
    };
    let targets = paths
        .iter()
        .filter_map(|(pattern, targets)| {
            if pattern == specifier {
                return Some((pattern.len(), targets, None));
            }
            let (prefix, suffix) = pattern.split_once('*')?;
            if suffix.contains('*') {
                return None;
            }
            let tail = specifier.strip_prefix(prefix)?.strip_suffix(suffix)?;
            Some((prefix.len(), targets, Some(tail)))
        })
        .collect::<Vec<_>>();
    let Some(longest) = targets.iter().map(|(length, _, _)| *length).max() else {
        // No TS paths rewrite competes with this literal Vite alias. The
        // exact_local_target join still must agree with the resolved file.
        return true;
    };
    let mut selected = targets.iter().filter(|(length, _, _)| *length == longest);
    let Some((_, targets, tail)) = selected.next() else {
        return false;
    };
    if selected.next().is_some() {
        return false;
    }
    let [target] = targets.as_slice() else {
        return false;
    };
    let target = match tail {
        Some(tail) if target.matches('*').count() == 1 => target.replace('*', tail),
        None if !target.contains('*') => target.clone(),
        _ => return false,
    };
    normalized(Path::new(&target)).is_some_and(|target| target == vite)
}

// TypeScript resolution is not a runtime-resolution attestation. In particular,
// extension substitution and tsconfig-only paths can select another file than
// Vite. After the already-congruence-checked literal alias, admit an exact file
// or the first ordinary Vite extension/index probe that agrees with Type
// Facts. Directory package entries require a separate package-resolution proof.
// Audited installed Vite 8.3.0: chunks/node.js DEFAULT_EXTENSIONS and
// tryCleanFsResolve/tryResolveRealFileWithExtensions. Vite 6/7 use the same list.
pub(super) const VITE_DEFAULT_EXTENSIONS: &[&str] =
    &[".mjs", ".js", ".mts", ".ts", ".jsx", ".tsx", ".json"];

pub(super) fn probe_metadata(path: &Path) -> Option<Option<fs::Metadata>> {
    match fs::metadata(path) {
        Ok(metadata) => Some(Some(metadata)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            // A dangling symlink is not absence authority.
            match fs::symlink_metadata(path) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Some(None),
                _ => None,
            }
        }
        Err(_) => None,
    }
}

pub(super) fn default_local_file(config: &Config, path: &Path) -> Option<PathBuf> {
    let path = normalized(path)?;
    if path.extension().is_some() {
        return path
            .extension()
            .is_some_and(|extension| {
                ["ts", "tsx", "mts", "cts", "js", "jsx", "mjs", "cjs"]
                    .iter()
                    .any(|allowed| extension == *allowed)
            })
            .then_some(path);
    }
    let metadata = probe_metadata(&path)?;
    if !config.default_extensions || metadata.as_ref().is_some_and(fs::Metadata::is_file) {
        return None;
    }
    for extension in VITE_DEFAULT_EXTENSIONS {
        let candidate = PathBuf::from(format!("{}{extension}", path.to_str()?));
        if probe_metadata(&candidate)?
            .as_ref()
            .is_some_and(fs::Metadata::is_file)
        {
            // Vite selects the first existing file. A later candidate cannot
            // make that choice ambiguous; the Type Facts target must still
            // equal this exact selected canonical file.
            return (fs::canonicalize(&candidate).ok().as_deref() == Some(candidate.as_path()))
                .then_some(candidate);
        }
    }
    if metadata.as_ref().is_some_and(fs::Metadata::is_dir) {
        // Vite attempts package.json before index. Never silently skip it,
        // including broken/symlink manifests or unknown conditional exports.
        match fs::symlink_metadata(path.join("package.json")) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            _ => return None,
        }
        for extension in VITE_DEFAULT_EXTENSIONS {
            let candidate = path.join(format!("index{extension}"));
            if probe_metadata(&candidate)?
                .as_ref()
                .is_some_and(fs::Metadata::is_file)
            {
                return (fs::canonicalize(&candidate).ok().as_deref() == Some(candidate.as_path()))
                    .then_some(candidate);
            }
        }
    }
    None
}

pub(super) fn exact_local_target(
    config: &Config,
    importer: &str,
    text: &str,
    target: &str,
) -> bool {
    if text.contains(['?', '#', '%', '\\']) {
        return false;
    }
    let matching = config
        .aliases
        .iter()
        .filter(|(find, _)| {
            text == find.as_str()
                || text
                    .strip_prefix(find.as_str())
                    .is_some_and(|tail| tail.starts_with('/'))
        })
        .collect::<Vec<_>>();
    let runtime = match matching.as_slice() {
        [] if text.starts_with('.') => Path::new(importer).parent().map(|parent| parent.join(text)),
        [(find, Some(replacement))] => Some(PathBuf::from(format!(
            "{replacement}{}",
            &text[find.len()..]
        ))),
        _ => None,
    };
    runtime
        .and_then(|path| default_local_file(config, &path))
        .is_some_and(|path| {
            path == Path::new(target)
                && fs::canonicalize(&path).ok().as_deref() == Some(path.as_path())
        })
}
