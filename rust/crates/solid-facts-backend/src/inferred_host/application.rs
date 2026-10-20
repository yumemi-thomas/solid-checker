//! Application discovery: Vite configuration, tsconfig and route inputs of one project directory.

use super::*;

#[cfg(test)]
pub(super) fn application(directory: &Path, project: &Path) -> Result<(), DiscoveryRefusal> {
    application_inputs(directory, project, &mut Vec::new())
}

pub(super) fn application_inputs(
    directory: &Path,
    project: &Path,
    inputs: &mut Vec<PathBuf>,
) -> Result<(), DiscoveryRefusal> {
    let path = directory.join("package.json");
    inputs.push(path.clone());
    if fs::symlink_metadata(&path).is_ok_and(|metadata| metadata.is_symlink()) {
        return Err(DiscoveryRefusal::new(
            &path,
            "symlinked application manifest has unknown config selection",
        ));
    }
    let package: serde_json::Value = serde_json::from_slice(
        &fs::read(&path)
            .ok()
            .required(&path, "cannot read application manifest")?,
    )
    .ok()
    .required(&path, "invalid application manifest JSON")?;
    if !unpublished(&package) {
        return Err(DiscoveryRefusal::new(
            &path,
            "published entry points permit unknown execution hosts",
        ));
    }
    if let Some(scripts) = package.get("scripts")
        && let Some(reason) = crate::host_invocation::refusal(
            scripts,
            crate::host_invocation::ManifestRole::Application,
        )
    {
        return Err(DiscoveryRefusal::new(&path, reason));
    }
    // A workspace leaf has its own application roots and project inventory.
    // Published enclosing packages can still expose the leaf to unknown hosts.
    for ancestor in directory.ancestors().skip(1) {
        let path = ancestor.join("package.json");
        inputs.push(path.clone());
        if path.exists() {
            if fs::symlink_metadata(&path).is_ok_and(|metadata| metadata.is_symlink()) {
                return Err(DiscoveryRefusal::new(
                    &path,
                    "symlinked enclosing manifest has unknown config selection",
                ));
            }
            let package: serde_json::Value = serde_json::from_slice(
                &fs::read(&path)
                    .ok()
                    .required(&path, "cannot read enclosing manifest")?,
            )
            .ok()
            .required(&path, "invalid enclosing manifest JSON")?;
            if !unpublished(&package) {
                return Err(DiscoveryRefusal::new(
                    &path,
                    "published enclosing package permits unknown execution hosts",
                ));
            }
            if let Some(scripts) = package.get("scripts")
                && let Some(reason) = crate::host_invocation::refusal(
                    scripts,
                    crate::host_invocation::ManifestRole::Enclosing,
                )
            {
                return Err(DiscoveryRefusal::new(&path, reason));
            }
        }
    }
    tsconfig_inputs(project, inputs, &mut BTreeSet::new())?;
    Ok(())
}

pub(super) fn tsconfig_inputs(
    path: &Path,
    inputs: &mut Vec<PathBuf>,
    seen: &mut BTreeSet<PathBuf>,
) -> Result<(), DiscoveryRefusal> {
    inputs.push(path.to_owned());
    let path = fs::canonicalize(path)
        .ok()
        .required(path, "cannot resolve tsconfig")?;
    if !seen.insert(path.clone()) {
        return Err(DiscoveryRefusal::new(&path, "cyclic tsconfig inheritance"));
    }
    inputs.push(path.clone());
    let source = fs::read_to_string(&path)
        .ok()
        .required(&path, "cannot read tsconfig")?;
    let config = host_config::jsonc(&source).required(&path, "invalid tsconfig JSONC")?;
    if let Some(references) = config.get("references") {
        let references = references
            .as_array()
            .required(&path, "project references is not an array")?;
        if !references.is_empty() {
            return Err(DiscoveryRefusal::at(
                &path,
                &source,
                source.find("\"references\"").unwrap_or(0),
                "project references need a complete execution inventory",
            ));
        }
    }
    if let Some(options) = config
        .get("compilerOptions")
        .and_then(serde_json::Value::as_object)
        && ["rootDirs", "moduleSuffixes", "customConditions", "plugins"]
            .iter()
            .any(|key| options.contains_key(*key))
    {
        let key = ["rootDirs", "moduleSuffixes", "customConditions", "plugins"]
            .into_iter()
            .find(|key| options.contains_key(*key))
            .expect("matched option");
        return Err(DiscoveryRefusal::at(
            &path,
            &source,
            source.find(&format!("\"{key}\"")).unwrap_or(0),
            format!("unsupported resolution option {key}"),
        ));
    }
    if let Some(extends) = config.get("extends") {
        let extends = extends
            .as_str()
            .required(&path, "nonliteral tsconfig extends")?;
        // No guessed package export resolution for config inheritance.
        if !extends.starts_with('.') || !extends.ends_with(".json") {
            return Err(DiscoveryRefusal::at(
                &path,
                &source,
                source.find("\"extends\"").unwrap_or(0),
                "tsconfig extends requires an exact relative JSON path",
            ));
        }
        tsconfig_inputs(
            &path
                .parent()
                .required(&path, "tsconfig has no parent")?
                .join(extends),
            inputs,
            seen,
        )?;
    }
    Ok(())
}

pub(super) fn side_inputs(
    directory: &Path,
    inputs: &mut Vec<PathBuf>,
) -> Result<(), DiscoveryRefusal> {
    // Vite's default PostCSS discovery loads JavaScript and named plugins in
    // Node. Include absence above the leaf too; JSON is not an inert exemption.
    for ancestor in directory.ancestors() {
        let package = ancestor.join("package.json");
        inputs.push(package.clone());
        if package.exists() {
            let package: serde_json::Value = serde_json::from_slice(
                &fs::read(&package)
                    .ok()
                    .required(&package, "cannot read PostCSS discovery manifest")?,
            )
            .ok()
            .required(&package, "invalid PostCSS discovery manifest")?;
            if package.get("postcss").is_some() {
                return Err(DiscoveryRefusal::new(
                    &ancestor.join("package.json"),
                    "executable PostCSS configuration",
                ));
            }
        }
        for stem in [".postcssrc", "postcss.config"] {
            for suffix in [
                "", ".json", ".yaml", ".yml", ".ts", ".cts", ".mts", ".js", ".cjs", ".mjs",
            ] {
                let path = ancestor.join(format!("{stem}{suffix}"));
                inputs.push(path.clone());
                if path.exists() {
                    return Err(DiscoveryRefusal::new(
                        &path,
                        "executable PostCSS side input",
                    ));
                }
            }
        }
    }
    Ok(())
}

pub(super) fn configuration(
    directory: &Path,
    inputs: &mut Vec<PathBuf>,
) -> Result<Config, DiscoveryRefusal> {
    configuration_in_generation(directory, inputs, None)
}

pub(super) fn configuration_in_generation(
    directory: &Path,
    inputs: &mut Vec<PathBuf>,
    generation: Option<&BTreeMap<String, String>>,
) -> Result<Config, DiscoveryRefusal> {
    side_inputs(directory, inputs)?;
    // Observe directory membership even on refusal: adding an alternate or
    // mode-specific config must invalidate a cached conventional answer.
    inputs.push(directory.to_owned());
    let entries = fs::read_dir(directory)
        .ok()
        .required(directory, "cannot enumerate Vite config variants")?;
    let mut variants = Vec::new();
    for entry in entries {
        let entry = entry
            .ok()
            .required(directory, "cannot enumerate Vite config variant")?;
        let name = entry.file_name();
        let name = name
            .to_str()
            .required(directory, "non-UTF-8 config candidate")?;
        if name.starts_with("vite.")
            && (name.contains(".config.") || name.starts_with("vite.config."))
            && !CONFIGS.contains(&name)
        {
            variants.push(entry.path());
        }
    }
    variants.sort();
    inputs.extend(variants.iter().cloned());
    if let Some(path) = variants.first() {
        return Err(DiscoveryRefusal::new(
            path,
            "non-conventional or mode-specific Vite config",
        ));
    }
    let configs = CONFIGS
        .iter()
        .map(|name| directory.join(name))
        .filter(|path| path.exists())
        .collect::<Vec<_>>();
    inputs.extend(CONFIGS.iter().map(|name| directory.join(name)));
    let [path] = configs.as_slice() else {
        return Err(DiscoveryRefusal::new(
            directory,
            format!("expected one Vite config, found {}", configs.len()),
        ));
    };
    if fs::symlink_metadata(path).is_ok_and(|metadata| metadata.is_symlink()) {
        return Err(DiscoveryRefusal::new(
            path,
            "symlinked Vite config is outside the closed selection grammar",
        ));
    }
    let source = fs::read_to_string(path)
        .ok()
        .required(path, "cannot read Vite config")?;
    let config = host_config::parse_detailed(path, &source)
        .map_err(|reason| DiscoveryRefusal::at(path, &source, reason.offset, reason.reason))?;
    let mut audited_packages = crate::host_plugins::PackageAudits::new();
    for name in &config.plugins {
        crate::host_plugins::audit(directory, name, inputs, &mut audited_packages, generation)
            .map_err(|reason| DiscoveryRefusal::new(path, reason))?;
    }
    if config.plugins.contains("@tanstack/router-plugin/vite") {
        // JSON can point at executable virtual-route modules. Options alone
        // do not disable generator discovery of this file.
        let path = directory.join("tsr.config.json");
        inputs.push(path.clone());
        if path.exists() {
            return Err(DiscoveryRefusal::new(
                &path,
                "executable virtual-route configuration",
            ));
        }
        let mut routes = Vec::new();
        let root = directory.join("src/routes");
        inputs.push(root.clone());
        if root.exists() {
            inventory_paths(&root, &mut routes).required(&root, "cannot inventory routes")?;
            if routes.iter().any(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.contains("__virtual."))
            }) {
                let path = routes
                    .iter()
                    .find(|path| {
                        path.file_name()
                            .and_then(|name| name.to_str())
                            .is_some_and(|name| name.contains("__virtual."))
                    })
                    .expect("matched virtual route");
                return Err(DiscoveryRefusal::new(
                    path,
                    "executable virtual-route module",
                ));
            }
            inputs.extend(routes);
        }
    }
    if config.tailwind {
        let mut inventory = Vec::new();
        inventory_paths(directory, &mut inventory)
            .required(directory, "cannot inventory Tailwind side inputs")?;
        // Keep the complete inventory in cache identity. Successful builds
        // discharge CSS compilation only; executable plugin/config side inputs
        // still cannot enter the reviewed configuration through a stylesheet.
        inputs.extend(inventory.iter().cloned());
        if !crate::host_loadable::stylesheet_closure(&inventory, &config, inputs) {
            return Err(DiscoveryRefusal::new(
                directory,
                "executable stylesheet plugin/config side-input closure is unproved",
            ));
        }
    }
    Ok(config)
}

pub(super) fn route_file(directory: &Path, config: &Config, file: &FileFacts) -> bool {
    config
        .routes
        .as_ref()
        .is_some_and(|dir| Path::new(file.path.as_str()).starts_with(directory.join(dir)))
}

pub(super) fn server_route(ast: &solid_facts::ast::AstFacts) -> bool {
    ast.module_level_exports().any(|export| {
        export.kind == ExportKind::All
            || export
                .specifiers
                .iter()
                .chain(&export.declarations)
                .any(|name| {
                    [
                        "GET", "HEAD", "POST", "PUT", "DELETE", "PATCH", "OPTIONS", "CONNECT",
                        "TRACE", "config", "route",
                    ]
                    .contains(&name.exported.as_str())
                })
    })
}

/// Audited filesystem-routing emits a static named import for every $$route
/// ref, even with code splitting on. Inventory excluded files too; the analyzed
/// file set alone cannot prove that no generated eager ref exists.
pub(super) fn lazy_route_manifest(
    directory: &Path,
    config: &Config,
    inputs: &mut Vec<PathBuf>,
) -> bool {
    fn lazy_tree(path: &Path, inputs: &mut Vec<PathBuf>) -> bool {
        inputs.push(path.to_owned());
        let Ok(metadata) = fs::symlink_metadata(path) else {
            return false;
        };
        if metadata.file_type().is_symlink() {
            return false;
        }
        if metadata.is_dir() {
            let Ok(entries) = fs::read_dir(path) else {
                return false;
            };
            return entries
                .into_iter()
                .all(|entry| entry.is_ok_and(|entry| lazy_tree(&entry.path(), inputs)));
        }
        if !metadata.is_file() {
            return false;
        }
        if !path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| ["js", "jsx", "ts", "tsx"].contains(&extension))
        {
            return true;
        }
        let Some(ast) = fs::read_to_string(path).ok().and_then(|source| {
            solid_facts::ast::extract(path.to_string_lossy().into_owned(), &source).ok()
        }) else {
            return false;
        };
        !ast.module_level_exports().any(|export| {
            export.kind == ExportKind::All
                || export
                    .specifiers
                    .iter()
                    .chain(&export.declarations)
                    .any(|specifier| specifier.exported == "route")
        })
    }
    config.plugins.contains("filesystem-routing/vite")
        && config.lazy_routes
        && config
            .routes
            .as_ref()
            .is_some_and(|routes| lazy_tree(&directory.join(routes), inputs))
}
