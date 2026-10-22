//! Execution scopes: scope identities, call edges, execution constants and regions.

use super::*;

pub(super) fn default_scope(
    file: &FileFacts,
    facts: &ProjectFacts,
    functions: &BTreeMap<String, String>,
) -> Option<String> {
    let defaults = file
        .ast
        .module_level_exports()
        .filter(|export| export.kind == ExportKind::Default)
        .collect::<Vec<_>>();
    let [export] = defaults.as_slice() else {
        return None;
    };
    let [declaration] = export.declarations.as_slice() else {
        return None;
    };
    let span = unwrapped(file, declaration.local.span);
    let symbol = exact_symbol(facts, file.path.as_str(), span);
    file.ast
        .functions
        .iter()
        .find(|function| {
            function
                .name
                .as_ref()
                .map_or(function.span, |name| name.span)
                == span
                || symbol.as_ref().and_then(|symbol| functions.get(symbol))
                    == Some(&scope_id(file.path.as_str(), Some(function)))
        })
        .map(|function| scope_id(file.path.as_str(), Some(function)))
}

pub(super) fn scope_id(file: &str, function: Option<&FunctionFact>) -> String {
    function.map_or_else(
        || format!("{file}#init"),
        |function| format!("{file}#{}:{}", function.span.start, function.span.end),
    )
}

pub(super) fn client_root(exists: impl Fn(&str) -> bool) -> Option<(String, bool)> {
    for extension in ["tsx", "jsx", "ts", "js", "mjs", "tsrx"] {
        let path = format!("src/entry-client.{extension}");
        if exists(&path) {
            return Some((path, false));
        }
    }
    for stem in ["App", "app"] {
        for extension in ["tsx", "jsx", "ts", "js", "tsrx"] {
            let path = format!("src/{stem}.{extension}");
            if exists(&path) {
                return Some((path, true));
            }
        }
    }
    None
}

pub(super) fn at_scope(file: &FileFacts, span: Span) -> String {
    let function = file
        .ast
        .functions
        .iter()
        .filter(|function| function.span.contains(span))
        .min_by_key(|function| function.span.end - function.span.start);
    if let Some(default) = file
        .ast
        .host_defaults
        .iter()
        .filter(|default| {
            default.span.contains(span)
                && function.is_some_and(|function| function.span == default.function)
        })
        .min_by_key(|default| default.span.end - default.span.start)
    {
        return default_scope_id(file.path.as_str(), default);
    }
    scope_id(file.path.as_str(), function)
}

pub(super) fn default_scope_id(path: &str, default: &HostDefaultInitializer) -> String {
    format!("{path}#default:{}:{}", default.span.start, default.span.end)
}

/// A default runs only on a proven undefined slot. A spread before the slot
/// makes its runtime position unknown. Only a normalized void-zero value is
/// positive undefined evidence; unresolved names and type casts are not.
/// Other supplied arguments must be inert literal values: a spread/getter/call
/// can exit before the callee is entered, even after the defaulted slot.
pub(super) fn omitted_or_undefined(
    call: &solid_facts::ast::CallFact,
    index: usize,
    undefined_values: &[Span],
) -> bool {
    inert_arguments(call, undefined_values)
        && call.arguments.get(index).is_none_or(|argument| {
            undefined_values.contains(&argument.value_span.unwrap_or(argument.span))
        })
}

pub(super) fn inert_arguments(
    call: &solid_facts::ast::CallFact,
    undefined_values: &[Span],
) -> bool {
    call.arguments.iter().all(|argument| {
        !argument.spread
            && (undefined_values.contains(&argument.value_span.unwrap_or(argument.span))
                || matches!(
                    &argument.literal_value,
                    solid_facts::ast::ArgumentLiteralFact::Null
                        | solid_facts::ast::ArgumentLiteralFact::Boolean(_)
                        | solid_facts::ast::ArgumentLiteralFact::Integer(_)
                        | solid_facts::ast::ArgumentLiteralFact::String(_)
                        | solid_facts::ast::ArgumentLiteralFact::Function
                ))
    })
}

pub(super) fn parameter_initialization_complete(
    ast: &solid_facts::ast::AstFacts,
    function: &FunctionFact,
    call: &solid_facts::ast::CallFact,
    index: usize,
) -> bool {
    let parameter = &function.parameters[index];
    parameter.shape == BindingShape::Identifier
        && (parameter.initializer.is_none()
            || call.arguments.get(index).is_some_and(|argument| {
                matches!(
                    &argument.literal_value,
                    solid_facts::ast::ArgumentLiteralFact::Null
                        | solid_facts::ast::ArgumentLiteralFact::Boolean(_)
                        | solid_facts::ast::ArgumentLiteralFact::Integer(_)
                        | solid_facts::ast::ArgumentLiteralFact::String(_)
                        | solid_facts::ast::ArgumentLiteralFact::Function
                )
            })
            || ast.host_defaults.iter().any(|default| {
                default.function == function.span
                    && default.argument_index == Some(index)
                    && default.inert
            }))
}

pub(super) fn call_edges(
    facts: &ProjectFacts,
    caller: &FileFacts,
    owner: &str,
    target: &str,
    call: &solid_facts::ast::CallFact,
    scopes: &mut BTreeMap<String, HostScope>,
) -> Option<()> {
    let target_scope = scopes.get(target)?;
    let target_path = target_scope.path.clone();
    let target_span = target_scope.span?;
    let file = facts
        .files
        .iter()
        .find(|file| file.path.as_str() == target_path)?;
    let function = file
        .ast
        .functions
        .iter()
        .find(|function| function.span == target_span)?;
    // Argument evaluation and earlier defaults precede this initializer. An
    // arbitrary previous initializer/destructuring step may throw. Do not
    // promote later defaults or the body from that uncertain transition.
    let defaults = file
        .ast
        .host_defaults
        .iter()
        .filter(|default| {
            default.function == target_span
                && default.argument_index.is_some_and(|index| {
                    omitted_or_undefined(call, index, &caller.ast.host_undefined_arguments)
                        && function
                            .parameters
                            .get(index)
                            .is_some_and(|parameter| parameter.shape == BindingShape::Identifier)
                        && (0..index).all(|previous| {
                            parameter_initialization_complete(&file.ast, function, call, previous)
                        })
                })
        })
        .map(|default| default_scope_id(&target_path, default))
        .collect::<Vec<_>>();
    let scope = scopes.get_mut(owner)?;
    if inert_arguments(call, &caller.ast.host_undefined_arguments)
        && (0..function.parameters.len())
            .all(|index| parameter_initialization_complete(&file.ast, function, call, index))
    {
        scope.edges.insert(target.into());
    }
    scope.edges.extend(defaults);
    Some(())
}

/// JSX supplies a props object, not an exact ordinary call argument list.
/// Destructuring/getters/defaults can throw before the body is entered.
pub(super) fn jsx_body_ready(
    facts: &ProjectFacts,
    scopes: &BTreeMap<String, HostScope>,
    id: &str,
) -> bool {
    let Some(scope) = scopes.get(id) else {
        return false;
    };
    facts
        .files
        .iter()
        .find(|file| file.path.as_str() == scope.path)
        .and_then(|file| {
            file.ast
                .functions
                .iter()
                .find(|function| Some(function.span) == scope.span)
        })
        .is_some_and(|function| {
            !function.generator
                && !function.rest_parameter
                && function.parameters.iter().all(|parameter| {
                    parameter.shape == BindingShape::Identifier && parameter.initializer.is_none()
                })
        })
}

/// The dialect supplies the exact candidate export/artifact vocabulary.
/// The shared facts contain no Solid spelling or package behavior. Aliases,
/// missing attestations, another artifact, and absent runtime bytes leave it
/// unknown. Record every byte/presence premise in the inference fingerprint.
pub(super) fn execution_constants(
    prepared: &DiscoveryInputs,
    facts: &ProjectFacts,
    file: &FileFacts,
    dialect: &dyn solid_dialect::Dialect,
    manifest: &mut BrowserRootManifest,
) -> BTreeMap<Span, (bool, bool)> {
    let directory = &prepared.directory;
    let config = &prepared.config;
    let mut values = BTreeMap::new();
    let Some(resolutions) = &facts.resolved_imports else {
        return values;
    };
    let browser = std::cell::OnceCell::new();
    let server = std::cell::OnceCell::new();
    for candidate_export in dialect.host_boolean_exports() {
        for import in file
            .ast
            .imports
            .iter()
            .filter(|import| !import.type_only && import.module == candidate_export.module)
        {
            if (config.tsconfig_paths
                || browser_selection_required(Path::new(file.path.as_str()), manifest))
                && facts.runtime_resolutions.is_none()
            {
                continue;
            }
            if config.aliases.keys().any(|alias| {
                import.module == alias.as_str()
                    || import
                        .module
                        .strip_prefix(alias.as_str())
                        .is_some_and(|tail| tail.starts_with('/'))
            }) {
                continue;
            }
            let SpecifierAttestation::Attested(row) =
                resolutions.specifier(file.path.as_str(), import.span, import.module.as_str())
            else {
                continue;
            };
            if row.resolution != ImportResolution::NodeModules
                || !row.symlink_path.is_empty()
                || !row.included_path.is_empty()
                || row.package_name.as_deref() != Some(candidate_export.module)
                || row.package_version.as_deref() != Some(candidate_export.version)
            {
                continue;
            }
            let Some(package_json) = row.package_manifest.as_deref().map(Path::new) else {
                continue;
            };
            let Some(package) = package_json.parent() else {
                continue;
            };
            let candidate = Path::new(file.path.as_str()).parent().and_then(|parent| {
                parent.ancestors().find_map(|ancestor| {
                    let candidate = ancestor.join("node_modules").join(candidate_export.module);
                    let path = candidate.join("package.json");
                    let observed =
                        graph_identity(manifest, &path).unwrap_or_else(|| "unreadable".into());
                    manifest
                        .inputs
                        .insert(path.to_string_lossy().into_owned(), observed);
                    path.exists().then_some(candidate)
                })
            });
            if candidate.as_deref() != Some(package)
                || !package.starts_with(directory)
                || fs::canonicalize(package).ok().as_deref() != Some(package)
            {
                continue;
            }
            let Some(package_identity) = graph_identity(manifest, package_json) else {
                continue;
            };
            manifest.inputs.insert(
                package_json.to_string_lossy().into_owned(),
                package_identity,
            );
            let Ok(bytes) = fs::read(package_json) else {
                continue;
            };
            let Ok(metadata) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
                continue;
            };
            if metadata.get("name").and_then(serde_json::Value::as_str)
                != Some(candidate_export.module)
                || metadata.get("version").and_then(serde_json::Value::as_str)
                    != Some(candidate_export.version)
            {
                continue;
            }
            for target in candidate_export.runtime_targets {
                let path = package.join(target);
                let observed =
                    graph_identity(manifest, &path).unwrap_or_else(|| "unreadable".into());
                manifest
                    .inputs
                    .insert(path.to_string_lossy().into_owned(), observed);
            }
            // Keep every package/path identity above even when this import
            // does not use the candidate. Runtime parsing cannot contribute a
            // constant without a named value binding for this exact export.
            if !import.bindings.iter().any(|binding| {
                !binding.type_only
                    && binding.kind == ImportKind::Named
                    && binding.imported.as_deref() == Some(candidate_export.export)
            }) {
                continue;
            }
            let browser = browser.get_or_init(|| {
                crate::host_constants::host_constants_of_module(
                    Path::new(file.path.as_str()),
                    &file.source,
                    solid_dialect::HostTargetCondition::Browser,
                    Some(directory),
                )
            });
            let server = server.get_or_init(|| {
                crate::host_constants::host_constants_of_module(
                    Path::new(file.path.as_str()),
                    &file.source,
                    solid_dialect::HostTargetCondition::Node,
                    Some(directory),
                )
            });
            let proved = |constants: &[crate::host_constants::ResolvedHostConstant], expected| {
                constants.iter().any(|constant| {
                    constant.fold.specifier == candidate_export.module
                        && constant.fold.imported == candidate_export.export
                        && constant.package_version == candidate_export.version
                        && constant.fold.value == expected
                        && constant.targets.iter().all(|(target, digest)| {
                            let path = package.join(target);
                            candidate_export.runtime_targets.contains(&target.as_str())
                                && fs::canonicalize(&path).ok().as_deref() == Some(path.as_path())
                                && manifest.inputs.get(path.to_string_lossy().as_ref())
                                    == Some(&format!("sha256:{digest}"))
                        })
                })
            };
            if !proved(browser, candidate_export.client) || !proved(server, candidate_export.server)
            {
                continue;
            }
            // The resolver must select one of the very browser files whose
            // bytes proved this constant. Successful build/start is not a
            // file-selection premise, nor is a declaration in this package.
            if facts.runtime_resolutions.as_ref().is_some_and(|runtime| {
                !matches!(runtime.outcome(file.path.as_str(), import.span, &import.module),
                    solid_facts::runtime_resolution::RuntimeOutcome::File { path, physical_path }
                    if path == physical_path && browser.iter().any(|constant|
                        constant.fold.specifier == candidate_export.module
                            && constant.fold.imported == candidate_export.export
                            && constant.targets.iter().any(|(target, _)| package.join(target) == Path::new(path.as_ref()))))
            }) {
                continue;
            }
            for binding in import.bindings.iter().filter(|binding| {
                !binding.type_only
                    && binding.kind == ImportKind::Named
                    && binding.imported.as_deref() == Some(candidate_export.export)
            }) {
                // Canonical Type Facts identity is required at this exact binding,
                // in addition to the Oxc declaration identity carried by the fact.
                if let Some(symbol) = exact_symbol(facts, file.path.as_str(), binding.local.span)
                    && let Some(symbol) = facts.typescript.symbol(&symbol)
                    && !symbol.declarations().is_empty()
                    && symbol.declarations().iter().all(|declaration| {
                        declaration.name.as_ref() == candidate_export.export
                            && Path::new(declaration.location.path.as_ref()).starts_with(package)
                    })
                {
                    values.insert(
                        binding.local.span,
                        (candidate_export.client, candidate_export.server),
                    );
                }
            }
        }
    }
    values
}

pub(super) fn static_loads<'a>(
    facts: &'a ProjectFacts,
    file: &'a FileFacts,
) -> impl Iterator<Item = (Span, &'a str, bool)> {
    file.ast
        .imports
        .iter()
        .filter(|import| {
            !import.type_only
                && (import.bindings.is_empty()
                    || import.bindings.iter().any(|binding| !binding.type_only))
        })
        .map(|import| {
            (
                import.span,
                import.module.as_str(),
                runtime_import(facts, file, import),
            )
        })
        .chain(file.ast.exports.iter().filter_map(|export| {
            export
                .module
                .as_deref()
                .filter(|_| {
                    !export.type_only
                        && (export.kind == ExportKind::All
                            || export.namespace.is_some()
                            || export.specifiers.is_empty()
                            || export
                                .specifiers
                                .iter()
                                .any(|specifier| !specifier.type_only))
                })
                .map(|text| (export.span, text, true))
        }))
}

pub(super) fn execution_regions(
    file: &FileFacts,
    scope: Option<Span>,
    constants: &BTreeMap<Span, (bool, bool)>,
    module_startup: bool,
) -> Vec<HostExecutionRegion> {
    let value = |identity: &HostConstantIdentity, server: bool| match identity {
        // Discovery has already admitted the closed Vite client configuration.
        HostConstantIdentity::ViteSsr => Some(server),
        HostConstantIdentity::Import { declaration } => constants
            .get(declaration)
            .map(|(client, node)| if server { *node } else { *client }),
    };
    file.ast
        .host_execution
        .iter()
        .filter(|fact| {
            fact.scope == scope
                && !matches!(
                    fact.kind,
                    HostExecutionSiteKind::ModuleCompletion
                        | HostExecutionSiteKind::ModuleStartupCompletion
                )
        })
        .map(|fact| HostExecutionRegion {
            span: fact.span,
            client: fact
                .predicate
                .evaluate_with_startup(&|identity| value(identity, false), module_startup),
            server: fact.predicate.evaluate(&|identity| value(identity, true)),
        })
        .collect()
}

pub(super) fn unwrapped(file: &FileFacts, mut span: Span) -> Span {
    for _ in 0..file.ast.transparent_wrappers.len() {
        let mut wrappers = file
            .ast
            .transparent_wrappers
            .iter()
            .filter(|wrapper| wrapper.span == span);
        let Some(wrapper) = wrappers.next() else {
            break;
        };
        if wrappers.next().is_some() || wrapper.inner == span {
            break;
        }
        span = wrapper.inner;
    }
    span
}

pub(super) fn direct_identifier(file: &FileFacts, span: Span) -> bool {
    let span = unwrapped(file, span);
    file.ast
        .identifiers
        .iter()
        .any(|identifier| identifier.span == span)
}

pub(super) fn exact_symbol(facts: &ProjectFacts, path: &str, span: Span) -> Option<String> {
    let mut matches = facts
        .typescript
        .entities_for_path(path)
        .iter()
        .filter(|entity| {
            entity.location.start_byte == u64::from(span.start)
                && entity.location.end_byte == u64::from(span.end)
                && !entity.symbol_unresolved()
                && !entity.symbol.is_empty()
        });
    let mut symbol = matches.next()?.symbol.to_string();
    if matches.next().is_some() {
        return None;
    }
    let mut seen = BTreeSet::new();
    loop {
        if !seen.insert(symbol.clone()) {
            return None;
        }
        let record = facts.typescript.symbol(&symbol)?;
        if record.alias_target().is_empty() || record.alias_target() == symbol {
            return Some(symbol);
        }
        symbol = record.alias_target().to_owned();
    }
}
