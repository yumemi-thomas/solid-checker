//! Graph discovery inputs: the explicit static surface, empty-graph refusals and selection premises.

use super::*;

// A resolved file can still fail ESM linking if a named value export is absent.
// Direct authored exports are proved here; indirect surfaces retain baseline
// until their exact reexport/binding trail has been authenticated.
pub(super) fn explicit_static_surface(
    facts: &ProjectFacts,
    file: &FileFacts,
    span: Span,
    target: &FileFacts,
) -> bool {
    let mut named = file
        .ast
        .imports
        .iter()
        .filter(|import| import.span == span)
        .flat_map(|import| &import.bindings)
        .filter(|binding| {
            !binding.type_only && matches!(binding.kind, ImportKind::Named | ImportKind::Default)
        })
        .map(|binding| (binding.imported.as_deref(), binding.local.span))
        .chain(
            file.ast
                .exports
                .iter()
                .filter(|export| export.span == span)
                .flat_map(|export| &export.specifiers)
                .filter(|specifier| !specifier.type_only)
                .map(|specifier| (file.source_text(specifier.local.span), specifier.local.span)),
        );
    named.all(|(name, source)| {
        let (Some(name), Some(symbol)) = (name, exact_symbol(facts, file.path.as_str(), source))
        else {
            return false;
        };
        target
            .ast
            .module_level_exports()
            .filter(|export| export.module.is_none() && !export.type_only)
            .flat_map(|export| export.declarations.iter().chain(&export.specifiers))
            .any(|export| {
                !export.type_only
                    && export.exported == name
                    && (exact_symbol(facts, target.path.as_str(), export.local.span).as_deref()
                        == Some(symbol.as_str())
                        || (facts
                            .typescript
                            .entities_for_path(target.path.as_str())
                            .iter()
                            .all(|entity| {
                                entity.location.start_byte != u64::from(export.local.span.start)
                                    || entity.location.end_byte != u64::from(export.local.span.end)
                            })
                            && facts.typescript.symbol(&symbol).is_some_and(|record| {
                                // An exported binding need not have its own entity
                                // row. Join the canonical symbol's one declaration
                                // exactly; never use containment or spelling.
                                let [declaration] = record.declarations() else {
                                    return false;
                                };
                                declaration.location.path.as_ref() == target.path.as_str()
                                    && declaration.location.start_byte
                                        == u64::from(export.local.span.start)
                                    && declaration.location.end_byte
                                        == u64::from(export.local.span.end)
                            })))
            })
    })
}

pub(super) fn empty_graph_refusal(
    facts: &ProjectFacts,
    config: &Config,
    roots: &[PathBuf],
    modules: &[HostModule],
    loads: &BTreeMap<(String, String), &str>,
) -> DiscoveryRefusal {
    let mut pending = roots
        .iter()
        .filter_map(|path| path.to_str().map(str::to_owned))
        .collect::<BTreeSet<_>>();
    let mut visited = BTreeSet::new();
    while let Some(path) = pending.pop_first() {
        if !visited.insert(path.clone()) {
            continue;
        }
        let Some(module) = modules.iter().find(|module| module.path == path) else {
            return DiscoveryRefusal::new(
                Path::new(&path),
                "root/dependency absent from analyzed module inventory",
            );
        };
        pending.extend(module.static_dependencies.iter().cloned());
        if module.completion == Some(false) {
            return DiscoveryRefusal::new(
                Path::new(&path),
                "certain top-level exit contradicts the app-starts premise; browser root withheld",
            );
        }
        if let Some(((path, text), reason)) =
            loads.iter().find(|((importer, _), _)| importer == &path)
        {
            let file = facts.files.iter().find(|file| file.path.as_str() == path);
            let offset = file
                .and_then(|file| {
                    file.ast
                        .imports
                        .iter()
                        .find(|import| import.module.as_str() == text)
                        .map(|import| import.span.start)
                        .or_else(|| {
                            file.ast
                                .exports
                                .iter()
                                .find(|export| export.module.as_deref() == Some(text.as_str()))
                                .map(|export| export.span.start)
                        })
                })
                .unwrap_or(0) as usize;
            let source = fs::read_to_string(path).unwrap_or_default();
            return DiscoveryRefusal::at(
                Path::new(path),
                &source,
                offset,
                format!("{reason}: {text}"),
            );
        }
        if module.refused {
            let file = facts.files.iter().find(|file| file.path.as_str() == path);
            let source_cause = file.and_then(|file| {
                file.ast
                    .module_directives
                    .iter()
                    .find(|directive| directive.value == "use server")
                    .map(|directive| (directive.span, "server-only module directive"))
                    .or_else(|| {
                        file.ast
                            .module_hazards
                            .iter()
                            .find(|hazard| {
                                hazard.kind != ModuleHazardKind::NonliteralDynamicLoading
                            })
                            .map(|hazard| (hazard.span, "opaque module evaluation hazard"))
                    })
                    .or_else(|| {
                        file.ast
                            .module_loads
                            .iter()
                            .find(|load| load.kind == ModuleLoadKind::Require)
                            .map(|load| {
                                (
                                    load.span,
                                    "CommonJS execution is not attested by the ESM root",
                                )
                            })
                    })
                    .or_else(|| {
                        file.ast
                            .import_equals
                            .iter()
                            .find(|import| !import.type_only)
                            .map(|import| {
                                (
                                    import.span,
                                    "CommonJS execution is not attested by the ESM root",
                                )
                            })
                    })
            });
            if let Some((span, reason)) = source_cause {
                let source = fs::read_to_string(&path).unwrap_or_default();
                return DiscoveryRefusal::at(
                    Path::new(&path),
                    &source,
                    span.start as usize,
                    reason,
                );
            }
            let reason = if !Path::new(&path).starts_with(
                Path::new(&facts.project_id)
                    .parent()
                    .unwrap_or(Path::new(".")),
            ) {
                "module lies outside the application"
            } else if file.is_some_and(|file| {
                file.ast
                    .module_directives
                    .iter()
                    .any(|directive| directive.value == "use server")
            }) {
                "server-only module directive"
            } else if file.is_some_and(|file| {
                file.ast
                    .module_hazards
                    .iter()
                    .any(|hazard| hazard.kind != ModuleHazardKind::NonliteralDynamicLoading)
            }) {
                "opaque module evaluation hazard"
            } else if file.is_some_and(|file| {
                file.ast
                    .module_loads
                    .iter()
                    .any(|load| load.kind == ModuleLoadKind::Require)
                    || file
                        .ast
                        .import_equals
                        .iter()
                        .any(|import| !import.type_only)
            }) {
                "CommonJS execution is not attested by the ESM root"
            } else if facts
                .resolved_imports
                .as_ref()
                .is_none_or(|rows| !rows.covers(&path))
            {
                "missing import-resolution inventory for module"
            } else if let Some(package) = Path::new(&path)
                .parent()
                .into_iter()
                .flat_map(Path::ancestors)
                .take_while(|ancestor| Some(*ancestor) != Path::new(&facts.project_id).parent())
                .map(|ancestor| ancestor.join("package.json"))
                .find(|path| path.exists())
            {
                return DiscoveryRefusal::new(&package, "nested package execution boundary");
            } else if [".d.ts", ".d.mts", ".d.cts"]
                .iter()
                .any(|suffix| path.ends_with(suffix))
            {
                "declaration-only module has no runtime implementation"
            } else if Path::new(&path)
                .file_stem()
                .and_then(|stem| stem.to_str())
                .is_some_and(|stem| ["entry-server", "middleware"].contains(&stem))
            {
                "server or middleware entry has a separate execution model"
            } else if config.mode == EntryMode::ClientStart
                && Path::new(&path)
                    .file_stem()
                    .is_some_and(|stem| stem == "Document")
            {
                "generated document is not a client-executed root"
            } else if file.is_some_and(|file| {
                route_file(
                    Path::new(&facts.project_id)
                        .parent()
                        .unwrap_or(Path::new(".")),
                    config,
                    file,
                )
            }) {
                "route client-only export/default callable model is unproved"
            } else {
                "module refusal has no recorded load or execution premise"
            };
            return DiscoveryRefusal::new(Path::new(&path), reason);
        }
        for dependency in &module.static_dependencies {
            let completion = modules
                .iter()
                .find(|module| &module.path == dependency)
                .and_then(|module| module.completion);
            if completion != Some(true) {
                return DiscoveryRefusal::new(
                    Path::new(dependency),
                    if completion == Some(false) {
                        "certain top-level exit contradicts the app-starts premise; browser root withheld"
                    } else {
                        "dependency initialization completion is unproved"
                    },
                );
            }
        }
    }
    DiscoveryRefusal::new(
        Path::new(&facts.project_id),
        "cyclic static initialization prevents browser scope admission",
    )
}

pub(super) struct DiscoveryInputs {
    pub(super) directory: PathBuf,
    pub(super) config: Config,
    pub(super) paths: BTreeMap<String, Vec<String>>,
    pub(super) config_inputs: Vec<PathBuf>,
}

/// Browser object maps can select bare requests, resolved relative files,
/// extension probes and false modules. Until that dispatch is modeled exactly,
/// any nonempty enclosing object map withholds every default request selection.
/// String browser entry points do not rewrite the importer's requests.
pub(super) fn browser_selection_required(
    importer: &Path,
    manifest: &mut BrowserRootManifest,
) -> bool {
    let Some(parent) = importer.parent() else {
        return true;
    };
    for ancestor in parent.ancestors() {
        let path = ancestor.join("package.json");
        let Some(observed) = graph_identity(manifest, &path) else {
            return true;
        };
        if observed == "absent" {
            continue;
        }
        // Pointer identity is not target content. Do not consume mutable
        // browser-map bytes behind a link as default selection authority.
        if fs::symlink_metadata(&path).is_ok_and(|metadata| metadata.is_symlink()) {
            return true;
        }
        let Some(package) = fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        else {
            return true;
        };
        if let Some(browser) = package.get("browser") {
            match browser {
                serde_json::Value::Object(map) if map.is_empty() => {}
                serde_json::Value::String(_) | serde_json::Value::Null => {}
                _ => return true,
            }
        }
    }
    false
}

pub(super) fn resolver_selection_required(config: &Config, request: &str) -> bool {
    config.tsconfig_paths
        && (!(request.starts_with("./") || request.starts_with("../") || request.starts_with('/'))
            || config.aliases.keys().any(|alias| {
                request == alias
                    || request
                        .strip_prefix(alias)
                        .is_some_and(|tail| tail.starts_with('/'))
            }))
}

pub(super) fn package_selection_agrees(
    facts: &ProjectFacts,
    importer: &str,
    span: Span,
    text: &str,
    entries: &[PathBuf],
    resolver_active: bool,
) -> bool {
    facts.runtime_resolutions.as_ref().map_or(!resolver_active, |runtime| {
        matches!(runtime.outcome(importer, span, text),
            solid_facts::runtime_resolution::RuntimeOutcome::File { path, physical_path }
            if path == physical_path && entries.iter().any(|entry| Path::new(path.as_ref()) == entry.as_path()
                && fs::canonicalize(entry).ok().as_deref() == Some(entry.as_path())))
    })
}

pub(super) fn discovery_inputs(
    directory: &Path,
    facts: &ProjectFacts,
    dialect: &dyn solid_dialect::Dialect,
) -> Result<DiscoveryInputs, DiscoveryRefusal> {
    let _stage = HostStage::new("host-discovery-config");
    if !dialect.models_server_functions() {
        return Err(DiscoveryRefusal::new(
            directory,
            "dialect has no server-function execution model",
        ));
    }
    let directory = fs::canonicalize(directory)
        .ok()
        .required(directory, "cannot resolve application directory")?;
    let project = fs::canonicalize(&facts.project_id).ok().required(
        Path::new(&facts.project_id),
        "cannot resolve analyzed tsconfig",
    )?;
    if project.parent() != Some(directory.as_path()) || !project.is_file() {
        return Err(DiscoveryRefusal::new(
            Path::new(&facts.project_id),
            "analyzed tsconfig is not a file owned by this application directory",
        ));
    }
    let mut paths_observed = Vec::new();
    application_inputs(&directory, &project, &mut paths_observed)?;
    // Refusal consumes only its witness. No app inventory, request parsing,
    // or manifest hashing is needed before the configuration can be admitted.
    let config = configuration(&directory, &mut paths_observed)?;
    let paths = paths_targets(&directory, &project)?;
    Ok(DiscoveryInputs {
        directory,
        config,
        paths,
        config_inputs: paths_observed,
    })
}
