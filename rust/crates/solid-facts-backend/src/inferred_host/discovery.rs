//! Index discovery: the discovered host index and the attachment of its inputs.

use super::*;

#[cfg(test)]
pub(super) fn discovered_index(
    directory: &Path,
    facts: &ProjectFacts,
    dialect: &dyn solid_dialect::Dialect,
    admitted: bool,
) -> Result<ProjectHostIndex, DiscoveryRefusal> {
    let inputs = discovery_inputs(directory, facts, dialect)?;
    let mut index = discovered_index_with_inputs(&inputs, facts, dialect, admitted)?;
    if !index.is_empty() {
        attach_discovery_inputs(&mut index, &inputs, facts, dialect, admitted)?;
    }
    Ok(index)
}

pub(super) fn attach_discovery_inputs(
    index: &mut ProjectHostIndex,
    prepared: &DiscoveryInputs,
    facts: &ProjectFacts,
    dialect: &dyn solid_dialect::Dialect,
    admitted: bool,
) -> Result<(), DiscoveryRefusal> {
    let _stage = HostStage::new("host-discovery-replay");
    let directory = &prepared.directory;
    let inputs = input_manifest(
        directory,
        facts,
        &prepared.config,
        prepared.config_inputs.clone(),
    )?;
    // A graph may refuse without ever needing a full manifest. On success,
    // recheck the earlier semantic premises against this input generation.
    // Never overwrite a fingerprint already observed while constructing edges.
    for (path, identity) in inputs {
        if index
            .manifest
            .inputs
            .get(&path)
            .is_some_and(|observed| observed != &identity)
        {
            return Err(DiscoveryRefusal::new(
                Path::new(&path),
                "graph inputs changed; retry analysis",
            ));
        }
        index.manifest.inputs.insert(path, identity);
    }
    let project = Path::new(&facts.project_id);
    let config_replay = HostStage::new("host-discovery-config-replay");
    let mut current_inputs = Vec::new();
    application_inputs(directory, project, &mut current_inputs)?;
    let current =
        configuration_in_generation(directory, &mut current_inputs, Some(&index.manifest.inputs))?;
    let paths = paths_targets(directory, project)?;
    current_inputs.sort();
    current_inputs.dedup();
    let mut previous_inputs = prepared.config_inputs.clone();
    previous_inputs.sort();
    previous_inputs.dedup();
    if current != prepared.config || paths != prepared.paths || current_inputs != previous_inputs {
        return Err(DiscoveryRefusal::new(
            directory,
            "configuration inputs changed; retry analysis",
        ));
    }
    drop(config_replay);
    // Probe decisions such as an absent nested package or Vite extension
    // precedence must also be made *after* the full input generation was
    // captured. Rebuild scopes now; the preliminary graph only selected
    // whether this work was necessary, and supplies no final authority.
    let mut current = discovered_index_with_inputs(prepared, facts, dialect, admitted)?;
    for (path, identity) in &index.manifest.inputs {
        if current
            .manifest
            .inputs
            .get(path)
            .is_some_and(|observed| observed != identity)
        {
            return Err(DiscoveryRefusal::new(
                Path::new(path),
                "graph inputs changed; retry analysis",
            ));
        }
        current
            .manifest
            .inputs
            .insert(path.clone(), identity.clone());
    }
    *index = current;
    Ok(())
}

pub(super) fn discovered_index_with_inputs(
    prepared: &DiscoveryInputs,
    facts: &ProjectFacts,
    dialect: &dyn solid_dialect::Dialect,
    // `false` is provisional only. The final pass enforces static package
    // loads itself.
    admitted: bool,
) -> Result<ProjectHostIndex, DiscoveryRefusal> {
    let _stage = HostStage::new(if admitted {
        "host-discovery-admitted"
    } else {
        "host-discovery-provisional"
    });
    let directory = prepared.directory.clone();
    let config = prepared.config.clone();
    let paths = prepared.paths.clone();
    let inputs = BTreeMap::new();
    let mut root_files = Vec::new();
    let mut default_body = false;
    match config.mode {
        EntryMode::ClientStart => {
            let (root, generated) = client_root(|path| directory.join(path).exists()).required(
                &directory,
                "no audited client entry (src/entry-client or src/App)",
            )?;
            root_files.push(directory.join(root));
            default_body = generated;
        }
        EntryMode::Html => {
            let html_path = directory.join("index.html");
            let source = fs::read_to_string(&html_path)
                .ok()
                .required(&html_path, "cannot read HTML entry")?;
            for entry in html_entries_at(&html_path, &source)? {
                let path = directory.join(entry);
                if fs::canonicalize(&path)
                    .ok()
                    .required(&path, "missing HTML module entry")?
                    != path
                    || !path.starts_with(&directory)
                {
                    return Err(DiscoveryRefusal::new(
                        &path,
                        "HTML entry escapes the application or follows a symlink",
                    ));
                }
                root_files.push(path);
            }
        }
    }
    let resolutions = facts
        .resolved_imports
        .as_ref()
        .required(&directory, "missing Type Facts import-resolution inventory")?;
    // Select only exact static JS/TS edges. Loadability, export surfaces,
    // hazards and cycles still independently gate the final root admission.
    // Dynamic imports and type-only references never enlarge this premise.
    let mut pending = root_files
        .iter()
        .filter_map(|path| path.to_str().map(str::to_owned))
        .collect::<BTreeSet<_>>();
    let mut startup_modules = BTreeSet::new();
    while let Some(path) = pending.pop_first() {
        if !startup_modules.insert(path.clone()) {
            continue;
        }
        let Some(file) = facts.files.iter().find(|file| file.path.as_str() == path) else {
            continue;
        };
        for (span, text, runtime) in static_loads(facts, file) {
            if !runtime {
                // An unused/type-space/server-erased value binding does not
                // establish startup evaluation, even if the same module is
                // dynamically imported in a later turn.
                continue;
            }
            let SpecifierAttestation::Attested(row) = resolutions.specifier(&path, span, text)
            else {
                continue;
            };
            let target = row.resolved_path.as_ref();
            if matches!(
                row.resolution,
                ImportResolution::Relative | ImportResolution::NonRelative
            ) && row.symlink_path.is_empty()
                && row.included_path.is_empty()
                && !text.contains(['?', '#', '%', '\\'])
                && !crate::host_loadable::resource_request(text)
                && local_edge(facts, &config, &paths, &path, span, text, target)
                && facts.files.iter().any(|file| file.path.as_str() == target)
                && fs::canonicalize(target).ok().as_deref() == Some(Path::new(target))
            {
                pending.insert(target.to_owned());
            }
        }
    }
    let mut manifest = BrowserRootManifest {
        inputs,
        reason: match config.mode {
            EntryMode::ClientStart => {
                "inferred browser: audited Solid client start entry probe order"
            }
            EntryMode::Html => "inferred browser: Vite index.html module entry",
        }
        .into(),
        ..BrowserRootManifest::default()
    };
    manifest.runtime_resolution = facts.runtime_resolutions.as_ref().map(|index| {
        let rows = serde_json::to_vec(&index.identity_rows()).expect("runtime rows serialize");
        format!("sha256:{:x}", Sha256::digest(rows))
    });
    let mut modules = Vec::new();
    let mut scopes = BTreeMap::<String, HostScope>::new();
    let mut functions = BTreeMap::<String, String>::new();
    let mut blocked_imports = BTreeSet::new();
    let mut unproven_semantics = BTreeSet::new();
    let mut withheld_loads = BTreeMap::new();
    let mut package_loads = crate::host_loadable::PackageLoads::default();
    let scope_selection = HostStage::new("scope-host-view-selection");
    let mut constants_by_file = BTreeMap::new();
    // Writes are generation facts, independent of the function being checked.
    // Resolve them once rather than joining every function with every write.
    let written_symbols = facts
        .files
        .iter()
        .flat_map(|file| {
            file.ast
                .assignments
                .iter()
                .filter_map(|write| exact_symbol(facts, file.path.as_str(), write.target))
        })
        .collect::<BTreeSet<_>>();
    for file in &facts.files {
        if file.ast.schema != AST_FACTS_SCHEMA {
            return Err(DiscoveryRefusal::new(
                Path::new(file.path.as_str()),
                "unsupported AST fact schema",
            ));
        }
        let constants = execution_constants(prepared, facts, file, dialect, &mut manifest);
        // Contents with separate execution models cannot inherit initialization.
        for region in file
            .ast
            .classes
            .iter()
            .map(|class| class.span)
            .chain(file.ast.module_blocks.iter().copied())
        {
            let id = format!("{}#region:{}:{}", file.path, region.start, region.end);
            scopes.insert(
                id.clone(),
                HostScope {
                    id,
                    path: file.path.to_string(),
                    span: Some(region),
                    refused: true,
                    ..HostScope::default()
                },
            );
        }
        for function in &file.ast.functions {
            if let Some(symbol) = body_symbol(facts, file, function, &written_symbols)
                && functions
                    .insert(symbol, scope_id(file.path.as_str(), Some(function)))
                    .is_some()
            {
                return Err(DiscoveryRefusal::new(
                    Path::new(file.path.as_str()),
                    "ambiguous callable symbol has multiple bodies",
                ));
            }
        }
        for function in std::iter::once(None).chain(file.ast.functions.iter().map(Some)) {
            let id = scope_id(file.path.as_str(), function);
            scopes.insert(
                id.clone(),
                HostScope {
                    id,
                    path: file.path.to_string(),
                    span: function.map(|function| function.span),
                    refused: function.is_some_and(|function| {
                        function.generator
                            || file
                                .ast
                                .classes
                                .iter()
                                .any(|class| class.span.contains(function.span))
                            || file
                                .ast
                                .module_blocks
                                .iter()
                                .any(|block| block.contains(function.span))
                            || file.ast.functions.iter().any(|ancestor| {
                                ancestor.span.contains(function.span)
                                    && ancestor.has_directive("use server")
                            })
                    }),
                    execution: execution_regions(
                        file,
                        function.map(|function| function.span),
                        &constants,
                        function.is_none() && startup_modules.contains(file.path.as_str()),
                    ),
                    ..HostScope::default()
                },
            );
        }
        for default in &file.ast.host_defaults {
            let id = default_scope_id(file.path.as_str(), default);
            let refused = default.argument_index.is_none()
                || scopes
                    .get(&format!(
                        "{}#{}:{}",
                        file.path, default.function.start, default.function.end
                    ))
                    .is_none_or(|scope| scope.refused);
            scopes.insert(
                id.clone(),
                HostScope {
                    id,
                    path: file.path.to_string(),
                    span: Some(default.span),
                    refused,
                    execution: execution_regions(file, Some(default.span), &constants, false),
                    ..HostScope::default()
                },
            );
        }
        constants_by_file.insert(file.path.as_str(), constants);
    }
    drop(scope_selection);
    for file in &facts.files {
        let path = file.path.as_str();
        let is_root = root_files.iter().any(|root| root.to_str() == Some(path));
        let filename = Path::new(path)
            .file_stem()
            .and_then(|stem| stem.to_str())
            .required(Path::new(path), "source has no UTF-8 file stem")?;
        let mut module = HostModule {
            path: path.into(),
            static_dependencies: BTreeSet::new(),
            completion: file
                .ast
                .host_execution
                .iter()
                .find(|fact| {
                    fact.kind
                        == if startup_modules.contains(path) {
                            HostExecutionSiteKind::ModuleStartupCompletion
                        } else {
                            HostExecutionSiteKind::ModuleCompletion
                        }
                        && fact.scope.is_none()
                })
                .and_then(|fact| {
                    let constants = &constants_by_file[path];
                    fact.predicate.evaluate_with_startup(
                        &|identity| match identity {
                            HostConstantIdentity::ViteSsr => Some(false),
                            HostConstantIdentity::Import { declaration } => {
                                constants.get(declaration).map(|(client, _)| *client)
                            }
                        },
                        startup_modules.contains(path),
                    )
                }),
            refused: !Path::new(path).starts_with(&directory)
                || [".d.ts", ".d.mts", ".d.cts"]
                    .iter()
                    .any(|suffix| path.ends_with(suffix))
                || !resolutions.covers(path)
                || file
                    .ast
                    .module_directives
                    .iter()
                    .any(|directive| directive.value == "use server")
                || ["entry-server", "middleware"].contains(&filename)
                || (config.mode == EntryMode::ClientStart && filename == "Document"),
        };
        if startup_modules.contains(path) && module.completion == Some(false) {
            module.refused = true;
        }
        if file
            .ast
            .module_hazards
            .iter()
            .any(|hazard| hazard.kind != ModuleHazardKind::NonliteralDynamicLoading)
        {
            // Opaque evaluation (notably eval) can replace a later local call
            // target. Withholding only this file's findings cannot authenticate
            // execution of a separately analyzed callee reached through it.
            unproven_semantics.insert(path.into());
            module.refused = true;
        }
        if file
            .ast
            .module_loads
            .iter()
            .any(|load| load.kind == ModuleLoadKind::Require)
            || file
                .ast
                .import_equals
                .iter()
                .any(|import| !import.type_only)
        {
            // Authored CommonJS lowering/host availability is not attested by
            // the admitted ESM roots. A Require syntax fact is unknown here,
            // including a shadowed implementation; never guess a Node host.
            module.refused = true;
            unproven_semantics.insert(path.into());
        }
        for ancestor in Path::new(path)
            .parent()
            .required(Path::new(path), "source has no parent directory")?
            .ancestors()
            .take_while(|ancestor| *ancestor != directory.as_path())
        {
            if ancestor.join("package.json").exists() {
                module.refused = true;
            }
        }
        let is_route = route_file(&directory, &config, file);
        if is_route && (server_route(&file.ast) || default_scope(file, facts, &functions).is_none())
        {
            // Picked mixed-route lowering needs separate compiler authority.
            module.refused = true;
        }
        let mut allowed_functions = functions
            .iter()
            .filter(|(_, id)| scopes.get(*id).is_some_and(|scope| scope.path == path))
            .map(|(symbol, _)| symbol.clone())
            .collect::<BTreeSet<_>>();
        for (span, text, runtime) in static_loads(facts, file) {
            // Resolver activity is app-wide. Do not approximate tsconfig
            // discovery, inheritance, tokens, patterns or negative lookups.
            // Only ADR 0220 can authenticate bare/aliased file selection.
            let browser_map = browser_selection_required(Path::new(path), &mut manifest);
            if (resolver_selection_required(&config, text) || browser_map)
                && facts.runtime_resolutions.is_none()
            {
                module.refused = true;
                blocked_imports.insert((path.into(), text.into()));
                unproven_semantics.insert(path.into());
                withheld_loads.insert(
                    (path.to_owned(), text.to_owned()),
                    if browser_map {
                        "importer/enclosing browser map requires authenticated file selection"
                    } else {
                        "active tsconfig resolver requires authenticated file selection"
                    },
                );
                continue;
            }
            // Even an unused value binding can survive client lowering.
            // Without exact erasure facts, every authored value import and
            // export-from must be loadable before this module can evaluate.
            if facts.runtime_resolutions.is_none() && !congruent_alias(&config, &paths, text) {
                blocked_imports.insert((path.into(), text.into()));
                unproven_semantics.insert(path.into());
                module.refused = true;
                withheld_loads.insert(
                    (path.to_owned(), text.to_owned()),
                    "Vite alias and Type Facts paths disagree",
                );
                continue;
            }
            if text == "virtual:file-routes" && config.routes.is_some() {
                // A manifest contains lazy/eager references, not executed page
                // bodies. Exact picked exports and invocation facts are needed.
                unproven_semantics.insert(path.into());
                // The exact audited provider always resolves this id. Only its
                // all-lazy variant is independently loadable: $$route exports
                // and codeSplitting:false emit eager picked static imports.
                let mut route_inputs = Vec::new();
                let loadable = lazy_route_manifest(&directory, &config, &mut route_inputs)
                    && facts.runtime_resolutions.is_none();
                for input in route_inputs {
                    let key = input
                        .to_str()
                        .required(&input, "non-UTF-8 route manifest input")?
                        .to_owned();
                    let observed = graph_identity(&mut manifest, &input)
                        .required(&input, "cannot fingerprint route manifest input")?;
                    manifest.inputs.insert(key, observed);
                }
                if !loadable {
                    module.refused = true;
                    withheld_loads.insert((path.to_owned(), text.to_owned()), "generated route manifest has unproved eager dependencies or requested resolver answer");
                }
                continue;
            }
            if crate::host_loadable::resource_request(text) {
                let mut resource_inputs = Vec::new();
                let resource = crate::host_loadable::resource(
                    &directory,
                    Path::new(path),
                    text,
                    &config,
                    &mut resource_inputs,
                );
                for input in resource_inputs {
                    let key = input
                        .to_str()
                        .required(&input, "non-UTF-8 resource input")?
                        .to_owned();
                    let observed = graph_identity(&mut manifest, &input)
                        .required(&input, "cannot fingerprint resource input")?;
                    manifest.inputs.insert(key, observed);
                }
                let admitted = resource.as_ref().is_some_and(|resource| {
                    facts.runtime_resolutions.as_ref().is_none_or(|runtime| {
                        resource.matches_outcome(runtime.outcome(path, span, text))
                    })
                });
                if !admitted {
                    module.refused = true;
                    unproven_semantics.insert(path.into());
                    blocked_imports.insert((path.into(), text.into()));
                    withheld_loads.insert((path.to_owned(), text.to_owned()), "Vite resource target, executable side input, or requested resolution is unproved");
                }
                // No analyzed implementation or callable edge: generated CSS
                // maps and asset strings are a linking premise only.
                continue;
            }
            let SpecifierAttestation::Attested(row) = resolutions.specifier(path, span, text)
            else {
                blocked_imports.insert((path.into(), text.into()));
                unproven_semantics.insert(path.into());
                module.refused = true;
                withheld_loads.insert(
                    (path.to_owned(), text.to_owned()),
                    "missing exact Type Facts resolution row",
                );
                continue;
            };
            if (row.resolution != ImportResolution::NodeModules && !row.symlink_path.is_empty())
                || !row.included_path.is_empty()
                || text.contains(['?', '#', '%', '\\'])
            {
                blocked_imports.insert((path.into(), text.into()));
                unproven_semantics.insert(path.into());
                module.refused = true;
                withheld_loads.insert(
                    (path.to_owned(), text.to_owned()),
                    "redirected, symlinked local, or query-bearing module target",
                );
                continue;
            }
            if matches!(
                row.resolution,
                ImportResolution::Relative | ImportResolution::NonRelative
            ) {
                let target = row.resolved_path.as_ref();
                if !local_edge(facts, &config, &paths, path, span, text, target) {
                    blocked_imports.insert((path.into(), text.into()));
                    unproven_semantics.insert(path.into());
                    module.refused = true;
                    // With an empty, closed plugin/alias set a TS-only paths
                    // request has no Vite resolver. Other mismatches are unknown,
                    // including requested resolver failures and divergent files.
                    let unloaded = facts.runtime_resolutions.is_none()
                        && config.plugins.is_empty()
                        && config.aliases.is_empty()
                        && text.starts_with("~/")
                        && row.resolution == ImportResolution::NonRelative
                        && Path::new(path).parent().required(Path::new(path), "source has no parent directory")?.ancestors().all(|ancestor| {
                            ["~", "~.js", "~.json", "~.node"].iter().all(|name| {
                                matches!(fs::symlink_metadata(ancestor.join("node_modules").join(name)),
                                    Err(error) if matches!(error.kind(), std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory))
                            })
                        });
                    withheld_loads.insert(
                        (path.to_owned(), text.to_owned()),
                        if unloaded {
                            "provably unloadable TS-only paths request"
                        } else {
                            "Vite and Type Facts runtime targets are not congruent"
                        },
                    );
                    continue;
                }
                if let Some(target_file) =
                    facts.files.iter().find(|file| file.path.as_str() == target)
                    && fs::canonicalize(target).ok().as_deref() == Some(Path::new(target))
                {
                    module.static_dependencies.insert(target.to_owned());
                    if !explicit_static_surface(facts, file, span, target_file) {
                        module.refused = true;
                        withheld_loads.insert(
                            (path.to_owned(), text.to_owned()),
                            "unproved direct runtime export surface",
                        );
                        continue;
                    }
                    if runtime {
                        scopes
                            .get_mut(&scope_id(path, None))
                            .required(Path::new(path), "missing initialization scope")?
                            .edges
                            .insert(scope_id(target, None));
                    }
                    for binding in file
                        .ast
                        .imports
                        .iter()
                        .filter(|import| import.span == span)
                        .flat_map(|import| &import.bindings)
                        .filter(|binding| !binding.type_only)
                    {
                        if let Some(symbol) = exact_symbol(facts, path, binding.local.span)
                            && functions
                                .get(&symbol)
                                .and_then(|id| scopes.get(id))
                                .is_some_and(|scope| scope.path == target)
                        {
                            // Canonical symbol identity alone loses reexport
                            // provenance. A barrel can have an incongruent edge
                            // even when this definition is initialized elsewhere.
                            // Indirect callable exports need an exact value trail.
                            allowed_functions.insert(symbol);
                        }
                    }
                } else {
                    module.refused = true;
                    withheld_loads.insert(
                        (path.to_owned(), text.to_owned()),
                        "resolved local target is not a canonical analyzed implementation",
                    );
                }
            } else if row.resolution == ImportResolution::NodeModules
                && config.aliases.keys().any(|find| {
                    text == find.as_str()
                        || text
                            .strip_prefix(find.as_str())
                            .is_some_and(|tail| tail.starts_with('/'))
                })
            {
                // Local resolver edges do not authenticate a substituted
                // package export's runtime surface. Keep package admission
                // independent; an explicit package alias cannot strengthen it.
                blocked_imports.insert((path.into(), text.into()));
                unproven_semantics.insert(path.into());
                module.refused = true;
                withheld_loads.insert(
                    (path.to_owned(), text.to_owned()),
                    "package alias has no authenticated runtime export surface",
                );
            } else if row.resolution == ImportResolution::Unresolved {
                blocked_imports.insert((path.into(), text.into()));
                unproven_semantics.insert(path.into());
                module.refused = true;
                withheld_loads.insert(
                    (path.to_owned(), text.to_owned()),
                    "unresolved static module request",
                );
            } else if row.resolution == ImportResolution::NodeModules {
                let mut package_inputs = Vec::new();
                let resolver_active = config.tsconfig_paths || browser_map;
                if !admitted {
                    // The provisional graph schedules package analysis; its
                    // optimistic edges are not final linking authority. Even
                    // here a selected file must be handled by reviewed Vite.
                    let unsupported_loader = package_loads
                        .candidates(Path::new(path), text, &mut package_inputs)
                        .is_some_and(|(_, _, entries)| {
                            (resolver_active
                                && !package_selection_agrees(
                                    facts, path, span, text, &entries, true,
                                ))
                                || !entries
                                    .iter()
                                    .all(|entry| crate::host_loadable::static_entry(entry))
                                || !crate::host_loadable::stylesheet_closure(
                                    &entries,
                                    &config,
                                    &mut package_inputs,
                                )
                        });
                    for input in package_inputs {
                        let key = input
                            .to_str()
                            .required(&input, "non-UTF-8 provisional package input")?
                            .to_owned();
                        let observed = graph_identity(&mut manifest, &input)
                            .required(&input, "cannot fingerprint provisional package input")?;
                        manifest.inputs.insert(key, observed);
                    }
                    if unsupported_loader
                        || (resolver_active && facts.runtime_resolutions.is_none())
                    {
                        module.refused = true;
                        blocked_imports.insert((path.into(), text.into()));
                        withheld_loads.insert(
                            (path.to_owned(), text.to_owned()),
                            "canonical installed package entry uses an unproved Vite loader or tsconfig resolver selection",
                        );
                    }
                    continue;
                }
                let entries = package_loads
                    .loadable(
                        Path::new(path),
                        text,
                        row,
                        dialect,
                        &mut package_inputs,
                        &config.optimize_exclude,
                    )
                    .filter(|entries| {
                        crate::host_loadable::stylesheet_closure(
                            entries.entries(),
                            &config,
                            &mut package_inputs,
                        )
                    });
                // The audited provider adds runtime-owner dedupe in dev. The
                // declaration package must also be the root-selected install.
                let provider = config.plugins.iter().any(|plugin| {
                    ["@solidjs/vite-plugin", "vite-plugin-solid"].contains(&plugin.as_str())
                });
                let core = crate::host_loadable::request(text).is_some_and(|(name, _)| {
                    dialect
                        .audited_installation()
                        .iter()
                        .any(|(owner, _)| *owner == name)
                });
                let deduped = !provider
                    || !core
                    || package_loads
                        .loadable(
                            &directory.join("index.ts"),
                            text,
                            row,
                            dialect,
                            &mut package_inputs,
                            &config.optimize_exclude,
                        )
                        .is_some();
                for input in package_inputs {
                    let key = input
                        .to_str()
                        .required(&input, "non-UTF-8 static package input")?
                        .to_owned();
                    let observed = graph_identity(&mut manifest, &input)
                        .required(&input, "cannot fingerprint static package input")?;
                    manifest.inputs.insert(key, observed);
                }
                let loadable = deduped
                    && !config.package_dedupe
                    && entries.is_some_and(|entries| {
                        package_selection_agrees(
                            facts,
                            path,
                            span,
                            text,
                            entries.entries(),
                            resolver_active,
                        )
                    });
                if !loadable {
                    module.refused = true;
                    blocked_imports.insert((path.into(), text.into()));
                    withheld_loads.insert((path.to_owned(), text.to_owned()), "missing exact installed client package entry, tsconfig rewrite, or requested resolver disagreement");
                }
            }
        }
        // Dynamic import attestation joins to its literal's own exact span.
        // Nonliteral imports and require add no edge and never cancel another.
        for load in &file.ast.module_loads {
            if load.kind != ModuleLoadKind::DynamicImport {
                continue;
            }
            if !scopes
                .get(&at_scope(file, load.span))
                .required(Path::new(path), "missing dynamic-load execution scope")?
                .client_live_site(load.span)
            {
                continue;
            }
            let (Some(text), Some(span)) = (load.specifier.as_deref(), load.specifier_span) else {
                continue;
            };
            if (resolver_selection_required(&config, text)
                || browser_selection_required(Path::new(path), &mut manifest))
                && facts.runtime_resolutions.is_none()
            {
                unproven_semantics.insert(path.into());
                continue;
            }
            if facts.runtime_resolutions.is_none() && !congruent_alias(&config, &paths, text) {
                unproven_semantics.insert(path.into());
                continue;
            }
            let SpecifierAttestation::Attested(row) = resolutions.specifier(path, span, text)
            else {
                continue;
            };
            let target = row.resolved_path.as_ref();
            if matches!(
                row.resolution,
                ImportResolution::Relative | ImportResolution::NonRelative
            ) && row.symlink_path.is_empty()
                && row.included_path.is_empty()
                && local_edge(facts, &config, &paths, path, span, text, target)
                && !text.contains(['?', '#', '%', '\\'])
                && facts.files.iter().any(|file| file.path.as_str() == target)
                && fs::canonicalize(target).ok().as_deref() == Some(Path::new(target))
            {
                scopes
                    .get_mut(&at_scope(file, load.span))
                    .required(Path::new(path), "missing dynamic-load execution scope")?
                    .edges
                    .insert(scope_id(target, None));
            }
        }
        if is_root {
            manifest.roots.insert(scope_id(path, None));
            if default_body && module.completion == Some(true) {
                let body = default_scope(file, facts, &functions).required(
                    Path::new(path),
                    "generated client root has no exact default callable export",
                )?;
                if jsx_body_ready(facts, &scopes, &body) {
                    manifest.roots.insert(body);
                }
            }
        }
        for call in file.ast.calls.iter().filter(|call| !call.construct) {
            let owner = at_scope(file, call.span);
            if !scopes
                .get(&owner)
                .required(Path::new(path), "missing call/JSX execution scope")?
                .client_live_site(call.span)
            {
                continue;
            }
            if let Some(function) = literal_body(file, call.callee) {
                call_edges(
                    facts,
                    file,
                    &owner,
                    &scope_id(path, Some(function)),
                    call,
                    &mut scopes,
                )
                .required(Path::new(path), "inconsistent local callable scope")?;
            } else if direct_identifier(file, call.callee)
                && let Some(symbol) = exact_symbol(facts, path, unwrapped(file, call.callee))
                && allowed_functions.contains(&symbol)
                && let Some(target) = functions.get(&symbol)
            {
                call_edges(facts, file, &owner, target, call, &mut scopes)
                    .required(Path::new(path), "inconsistent local callable scope")?;
            }
            // Constructing/passing a function value does not execute its body.
            // Callback edges need exact invocation and execution-host facts.
        }
        for element in &file.ast.jsx_elements {
            let owner = at_scope(file, element.span);
            if !scopes
                .get(&owner)
                .required(Path::new(path), "missing call/JSX execution scope")?
                .client_live_site(element.span)
            {
                continue;
            }
            if element.member_object.is_none()
                && let Some(symbol) = exact_symbol(facts, path, element.name.span)
                && allowed_functions.contains(&symbol)
                && let Some(target) = functions.get(&symbol)
                && jsx_body_ready(facts, &scopes, target)
            {
                scopes
                    .get_mut(&owner)
                    .required(Path::new(path), "missing JSX execution scope")?
                    .edges
                    .insert(target.clone());
            }
            // JSX child/prop function values likewise require invocation proof.
        }
        modules.push(module);
    }
    if manifest.roots.is_empty()
        || root_files.iter().any(|root| {
            !facts
                .files
                .iter()
                .any(|file| root.to_str() == Some(file.path.as_str()))
        })
    {
        return Err(DiscoveryRefusal::new(
            &directory,
            "browser entry is absent from the analyzed Type Facts file set",
        ));
    }
    manifest.reason.push_str(&format!(
        "; roots: {}",
        root_files
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    ));
    if !withheld_loads.is_empty() {
        manifest
            .reason
            .push_str(&format!("; withheld static loads: {withheld_loads:?}"));
    }
    for module in modules
        .iter()
        .filter(|module| startup_modules.contains(&module.path) && module.completion == Some(false))
    {
        manifest.reason.push_str(&format!("; certain top-level exit contradicts the app-starts premise; browser root withheld: {}", module.path));
    }
    let mut index = ProjectHostIndex::build(
        manifest,
        &modules,
        &scopes.into_values().collect::<Vec<_>>(),
    );
    index.blocked_imports = blocked_imports;
    index.unproven_semantics = unproven_semantics;
    if index.is_empty() {
        index.manifest.reason =
            empty_graph_refusal(facts, &config, &root_files, &modules, &withheld_loads).message();
    }
    Ok(index)
}
