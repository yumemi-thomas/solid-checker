//! Package-contract resolution and export-summary construction.
//!
//! Resolves imported contract bindings to local symbols (`resolve_contract_imports`)
//! and turns the interprocedural summaries into the per-export contract artifacts
//! that a downstream package sees. Owns both the full and incremental summary
//! builds; the public contract data types stay in the crate root.

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    path::Path,
    sync::Arc,
};

use solid_dialect::Dialect;
use solid_facts::ProjectFacts;
use typefacts::{Callability, Location, ReferenceSpace};

use super::{
    ContractCallback, ContractClaim, ContractExport, ContractExportVariant, ContractReactiveRead,
    ContractReturn, ContractUnknownClaim, EntitySymbols, PackageContract, ReactiveSourceKind,
    RuntimeEnvironment, StaticDefect, StaticDefectKind, SummaryNode, SummaryRead, SummaryReads,
    SymbolId, location, location_order,
};
use crate::cache::{CachedContractExports, ContractExportFragment, ContractNodeKey};
use crate::pipeline::parallel_slice_results;
#[derive(Clone)]
pub(super) struct ResolvedContractBinding {
    pub(super) local_name: String,
    pub(super) imported_name: String,
    pub(super) package_name: String,
    pub(super) symbol: SymbolId,
    pub(super) runtime_identity: String,
    pub(super) contract_location: Location,
    pub(super) summary: ContractExport,
}

pub(super) struct ResolvedContracts {
    pub(super) bindings: Vec<ResolvedContractBinding>,
    pub(super) by_symbol: HashMap<SymbolId, ResolvedContractBinding>,
    pub(super) missing_exports: Vec<StaticDefect>,
}

fn runtime_identity_at(facts: &ProjectFacts, location: &Location) -> String {
    facts
        .typescript
        .entities()
        .find(|entity| entity.location == *location)
        .map_or_else(String::new, |entity| entity.runtime_identity.to_string())
}

fn source_name_at(facts: &ProjectFacts, location: &Location) -> String {
    facts
        .files
        .iter()
        .find(|file| file.path.as_str() == location.path.as_ref())
        .and_then(|file| {
            file.source_text(solid_facts::core::Span::new(
                u32::try_from(location.start_byte).ok()?,
                u32::try_from(location.end_byte).ok()?,
            ))
        })
        .unwrap_or_default()
        .to_owned()
}

fn push_runtime_identity_conflict(
    missing_exports: &mut Vec<StaticDefect>,
    location: &Location,
    seen_locations: &mut HashSet<(String, u64, u64)>,
) {
    let key = (
        location.path.to_string(),
        location.start_byte,
        location.end_byte,
    );
    if !seen_locations.insert(key) {
        return;
    }
    missing_exports.push(StaticDefect {
        kind: StaticDefectKind::PackageContractExportMissing {
            module: "<runtime-identity-conflict>".into(),
            export: "<conflicting-contract-summaries>".into(),
            reexported: true,
        },
        location: location.clone(),
        analysis_context:
            "multiple exact package contracts describe the same runtime export differently".into(),
        fixes: vec![],
        uncertain: false,
    });
}

/// Joins package summaries through exact runtime identity after direct import
/// and explicit re-export discovery.
///
/// This is deliberately an O(entities + contracted-bindings) pass. It does
/// not resolve by spelling, scan every entity for every shorthand, or turn an
/// empty identity into project ownership. Export-star chains can participate
/// when TypeFacts exposes the same identity at a concrete binding; a missing
/// identity remains fail-closed.
fn join_runtime_identity_aliases(
    facts: &ProjectFacts,
    entities: &EntitySymbols,
    bindings: &mut Vec<ResolvedContractBinding>,
    by_symbol: &mut HashMap<SymbolId, ResolvedContractBinding>,
    missing_exports: &mut Vec<StaticDefect>,
) {
    let mut candidates = HashMap::<String, Vec<ResolvedContractBinding>>::new();
    for binding in bindings
        .iter()
        .filter(|binding| !binding.runtime_identity.is_empty())
    {
        let entries = candidates
            .entry(binding.runtime_identity.clone())
            .or_default();
        if !entries.iter().any(|existing| {
            existing.package_name == binding.package_name
                && existing.contract_location == binding.contract_location
                && existing.summary == binding.summary
        }) {
            entries.push(binding.clone());
        }
    }

    let mut index = HashMap::new();
    let mut conflicts = HashSet::new();
    for (identity, entries) in candidates {
        let Some(first) = entries.first().cloned() else {
            continue;
        };
        if entries
            .iter()
            .skip(1)
            .any(|entry| entry.package_name != first.package_name || entry.summary != first.summary)
        {
            conflicts.insert(identity);
            continue;
        }
        index.insert(identity, first);
    }

    let mut bound_symbols = by_symbol.keys().cloned().collect::<HashSet<_>>();
    let mut seen_locations = HashSet::new();
    for entity in facts
        .typescript
        .entities()
        .filter(|entity| !entity.runtime_identity.is_empty())
    {
        let Some(symbol) = entities.get(&entity.location).cloned() else {
            continue;
        };
        if conflicts.contains(entity.runtime_identity.as_ref()) {
            push_runtime_identity_conflict(missing_exports, &entity.location, &mut seen_locations);
            continue;
        }
        let Some(template) = index.get(entity.runtime_identity.as_ref()).cloned() else {
            continue;
        };
        if let Some(existing) = by_symbol.get(&symbol)
            && (existing.package_name != template.package_name
                || existing.summary != template.summary)
        {
            push_runtime_identity_conflict(missing_exports, &entity.location, &mut seen_locations);
            continue;
        }
        if !bound_symbols.insert(symbol.clone()) {
            continue;
        }
        let mut binding = template;
        binding.local_name = source_name_at(facts, &entity.location);
        binding.symbol = symbol.clone();
        binding.runtime_identity = entity.runtime_identity.to_string();
        by_symbol.insert(symbol, binding.clone());
        bindings.push(binding);
    }
}

/// Whether the dialect's own vocabulary outranks a package contract for a name
/// imported from `module`.
///
/// Solid's built-ins have richer native semantics than any cross-package
/// contract summary can express: ownership, async provenance, writes, and
/// cleanup phases. The reviewed contract stays as evidence and for export
/// completeness, but its coarse callbacks/returns must not be layered over
/// native facts.
///
/// The gate is the dialect's module-ownership answer, not the literal package
/// name `solid-js`. 1.x reaches `createStore` only through `solid-js/store` and
/// `Portal` only through `solid-js/web`; 2.0 moved the whole DOM surface to the
/// separate `@solidjs/web` package. Comparing package names gave the package
/// root native precedence and every other entrypoint the contract's coarse
/// answer, for the same primitives.
fn native_vocabulary_outranks_contract(
    dialect: &dyn Dialect,
    module: &str,
    imported: &str,
) -> bool {
    dialect.owns_module(module) && dialect.declares_primitive(imported)
}

fn push_environment_dependent_export(
    missing_exports: &mut Vec<StaticDefect>,
    module: &str,
    export: &str,
    reexported: bool,
    location: Location,
) {
    missing_exports.push(StaticDefect {
        kind: StaticDefectKind::PackageContractEnvironmentDependent {
            module: module.to_owned(),
            export: export.to_owned(),
            reexported,
        },
        location,
        analysis_context: String::new(),
        fixes: vec![],
        uncertain: false,
    });
}

/// Keep the known parts of a partial export usable while opening the existing
/// per-export contract obligation for every non-callback claim that cannot yet
/// be consumed demand-sensitively. Unknown callbacks are handled separately:
/// omitting their symbol from the callback map preserves the existing
/// callable-argument obligation and stays quiet for calls with no callable
/// argument.
fn push_unknown_contract_claims(
    missing_exports: &mut Vec<StaticDefect>,
    summary: &ContractExport,
    module: &str,
    export: &str,
    reexported: bool,
    location: Location,
) {
    let mut claims = Vec::new();
    if summary.reactive_reads.is_unknown() {
        claims.push("reactiveReads");
    }
    if summary.returns.is_unknown() {
        claims.push("returns");
    }
    if summary.owner_requirements.is_unknown() {
        claims.push("ownerRequirements");
    }
    if summary.async_behavior.is_unknown() {
        claims.push("asyncBehavior");
    }
    if claims.is_empty() {
        return;
    }
    missing_exports.push(StaticDefect {
        kind: StaticDefectKind::PackageContractExportMissing {
            module: module.to_owned(),
            export: export.to_owned(),
            reexported,
        },
        location,
        analysis_context: format!("unknown-contract-claims:{}", claims.join(",")),
        fixes: vec![],
        uncertain: false,
    });
}

fn selected_contract_export(
    contract: &PackageContract,
    module: &str,
    summary: ContractExport,
    environment: &RuntimeEnvironment,
) -> Option<ContractExport> {
    let suffix = module.strip_prefix(&contract.package.name)?;
    let entrypoint_name = if suffix.is_empty() {
        ".".to_owned()
    } else if suffix.starts_with('/') {
        format!(".{suffix}")
    } else {
        return None;
    };
    let entrypoint = contract.entrypoints.get(&entrypoint_name)?;
    if !entrypoint.conditions.is_empty()
        && !environment.matches_entrypoint_conditions(&entrypoint.conditions)
    {
        return None;
    }
    if summary.variants.is_empty() {
        return Some(summary);
    }
    let matching = summary
        .variants
        .iter()
        .filter(|variant| environment.matches_conditions(&variant.conditions))
        .collect::<Vec<_>>();
    match matching.len() {
        0 => None,
        1 => Some(*matching[0].summary.clone()),
        _ => selected_variant(&matching).map(|variant| *variant.summary.clone()),
    }
}

/// Choose between several export-map branches that all match this environment.
///
/// Generated contracts carry `precedence` on every variant, which is the
/// export map's own first-match-wins order and therefore the exact answer.
/// A handwritten contract carries none; there the only thing that can be
/// proven without inventing an order is that an explicitly named branch is
/// more specific than the unconditional `default` fallback. Anything else --
/// two named branches with no recorded order, or a tie in `precedence` --
/// stays fail-closed.
fn selected_variant<'a>(
    matching: &'a [&'a ContractExportVariant],
) -> Option<&'a ContractExportVariant> {
    if matching.iter().all(|variant| variant.precedence.is_some()) {
        return lowest_unique_precedence(matching);
    }
    let mut named = matching.iter().filter(|variant| {
        !variant
            .conditions
            .iter()
            .any(|condition| condition == "default")
    });
    let winner = *named.next()?;
    named.next().is_none().then_some(winner)
}

/// Resolve two or more overlapping export-map branches by `precedence`.
///
/// `package.json#exports` is an ordered map resolved first-match-wins, so
/// when several variants match the runtime environment, the branch with the
/// lowest `precedence` is the one Node itself would have resolved. That
/// substitution is only safe when it removes ambiguity rather than guessing
/// through it: every matching variant must declare a `precedence`, and the
/// minimum among them must be unique. A tie, or any matching variant missing
/// `precedence`, leaves the choice undetermined and must stay fail-closed.
fn lowest_unique_precedence<'a>(
    matching: &'a [&'a ContractExportVariant],
) -> Option<&'a ContractExportVariant> {
    let mut precedences = Vec::with_capacity(matching.len());
    for variant in matching {
        precedences.push(variant.precedence?);
    }
    let min = *precedences.iter().min()?;
    let mut winner = None;
    for (variant, precedence) in matching.iter().zip(precedences.iter()) {
        if *precedence == min {
            if winner.is_some() {
                return None;
            }
            winner = Some(*variant);
        }
    }
    winner
}

pub(super) fn resolve_contract_imports(
    facts: &ProjectFacts,
    contracts: &[PackageContract],
    entities: &EntitySymbols,
    dialect: &dyn Dialect,
    environment: &RuntimeEnvironment,
) -> ResolvedContracts {
    let mut bindings = Vec::new();
    let mut by_symbol = HashMap::new();
    let mut missing_exports = Vec::new();
    for file in &facts.files {
        for import in &file.ast.imports {
            if import.type_only {
                continue;
            }
            let Some(contract) = PackageContract::for_module(contracts, &import.module) else {
                continue;
            };
            for binding in &import.bindings {
                if binding.type_only {
                    continue;
                }
                if binding.kind == solid_facts::ast::ImportKind::Namespace {
                    let namespace_location = location(file.path.shared(), binding.local.span);
                    let Some(namespace_symbol) = entities.get(&namespace_location) else {
                        continue;
                    };
                    for member in file.ast.members.iter().filter(|member| {
                        file.ast
                            .computed_members
                            .binary_search(&member.span)
                            .is_err()
                            && entities.get(&location(file.path.shared(), member.object))
                                == Some(namespace_symbol)
                    }) {
                        let imported = file
                            .source_text(member.property)
                            .unwrap_or_default()
                            .to_owned();
                        let Some(summary) = contract
                            .exports_for_module(&import.module)
                            .and_then(|exports| exports.get(imported.as_str()))
                            .cloned()
                        else {
                            missing_exports.push(StaticDefect {
                                kind: StaticDefectKind::PackageContractExportMissing {
                                    module: import.module.to_string(),
                                    export: imported,
                                    reexported: false,
                                },
                                location: location(file.path.shared(), member.property),
                                analysis_context: String::new(),
                                fixes: vec![],
                                uncertain: false,
                            });
                            continue;
                        };
                        let member_location = location(file.path.shared(), member.property);
                        let Some(symbol) = entities.get(&member_location).cloned() else {
                            continue;
                        };
                        if native_vocabulary_outranks_contract(dialect, &import.module, &imported) {
                            continue;
                        }
                        let Some(summary) = selected_contract_export(
                            contract,
                            &import.module,
                            summary,
                            environment,
                        ) else {
                            push_environment_dependent_export(
                                &mut missing_exports,
                                &import.module,
                                &imported,
                                false,
                                member_location,
                            );
                            continue;
                        };
                        push_unknown_contract_claims(
                            &mut missing_exports,
                            &summary,
                            &import.module,
                            &imported,
                            false,
                            member_location.clone(),
                        );
                        let resolved = ResolvedContractBinding {
                            local_name: imported.clone(),
                            imported_name: imported.clone(),
                            package_name: contract.package.name.clone(),
                            symbol: symbol.clone(),
                            runtime_identity: runtime_identity_at(facts, &member_location),
                            contract_location: Location {
                                path: format!("{}#{imported}", contract.source_path).into(),
                                start_byte: 0,
                                end_byte: 0,
                            },
                            summary,
                        };
                        // Native dialect facts are richer than the package
                        // schema, but the reviewed package contract remains
                        // the only semantic evidence for public Solid exports
                        // outside that native vocabulary. Apply the same
                        // precedence to namespace and named imports.
                        bindings.push(resolved.clone());
                        by_symbol.insert(symbol, resolved);
                    }
                    continue;
                }
                let Some(imported) = binding.imported.as_deref().or_else(|| {
                    (binding.kind == solid_facts::ast::ImportKind::Default).then_some("default")
                }) else {
                    continue;
                };
                let binding_location = location(file.path.shared(), binding.local.span);
                let Some(symbol) = entities.get(&binding_location).cloned() else {
                    continue;
                };
                let Some(summary) = contract
                    .exports_for_module(&import.module)
                    .and_then(|exports| exports.get(imported))
                    .cloned()
                else {
                    let import_entity = facts.typescript.entities().find(|entity| {
                        entity.location.path == binding_location.path
                            && entity.location.start_byte == binding_location.start_byte
                            && entity.location.end_byte == binding_location.end_byte
                    });
                    let runtime_referenced = import_entity
                        .and_then(|entity| entity.reference_space)
                        .is_none_or(|space| {
                            matches!(space, ReferenceSpace::Value | ReferenceSpace::Both)
                        });
                    if !runtime_referenced {
                        // TypeScript reports no value-space reference for
                        // mixed imports such as `import { JSX, Portal }`.
                        // A type-only binding cannot consume runtime
                        // reactivity and therefore needs no export summary.
                        continue;
                    }
                    missing_exports.push(StaticDefect {
                        kind: StaticDefectKind::PackageContractExportMissing {
                            module: import.module.to_string(),
                            export: imported.to_owned(),
                            reexported: false,
                        },
                        location: binding_location,
                        analysis_context: String::new(),
                        fixes: vec![],
                        uncertain: false,
                    });
                    continue;
                };
                if native_vocabulary_outranks_contract(dialect, &import.module, imported) {
                    continue;
                }
                let Some(summary) =
                    selected_contract_export(contract, &import.module, summary, environment)
                else {
                    push_environment_dependent_export(
                        &mut missing_exports,
                        &import.module,
                        imported,
                        false,
                        binding_location,
                    );
                    continue;
                };
                push_unknown_contract_claims(
                    &mut missing_exports,
                    &summary,
                    &import.module,
                    imported,
                    false,
                    binding_location.clone(),
                );
                let resolved = ResolvedContractBinding {
                    local_name: file
                        .source_text(binding.local.span)
                        .unwrap_or_default()
                        .to_owned(),
                    imported_name: imported.into(),
                    package_name: contract.package.name.clone(),
                    symbol: symbol.clone(),
                    runtime_identity: runtime_identity_at(facts, &binding_location),
                    contract_location: Location {
                        path: format!("{}#{imported}", contract.source_path).into(),
                        start_byte: 0,
                        end_byte: 0,
                    },
                    summary,
                };
                // Solid's built-ins have richer native semantics than their
                // cross-package contract summary (ownership, async
                // provenance, writes, and cleanup phases). Keep the bundled
                // contract as evidence and for export completeness, but do
                // not layer its coarse callbacks/returns over native facts.
                bindings.push(resolved.clone());
                by_symbol.insert(symbol, resolved);
            }
        }
        for export in &file.ast.exports {
            if export.type_only {
                continue;
            }
            let Some(module) = export.module.as_deref() else {
                continue;
            };
            let Some(contract) = PackageContract::for_module(contracts, module) else {
                continue;
            };
            for specifier in &export.specifiers {
                if specifier.type_only {
                    continue;
                }
                let imported = file.source_text(specifier.local.span).unwrap_or_default();
                let specifier_location = location(file.path.shared(), specifier.local.span);
                let Some(symbol) = entities.get(&specifier_location).cloned() else {
                    continue;
                };
                let Some(summary) = contract
                    .exports_for_module(module)
                    .and_then(|exports| exports.get(imported))
                    .cloned()
                else {
                    missing_exports.push(StaticDefect {
                        kind: StaticDefectKind::PackageContractExportMissing {
                            module: module.to_owned(),
                            export: imported.to_owned(),
                            reexported: true,
                        },
                        location: specifier_location,
                        analysis_context: String::new(),
                        fixes: vec![],
                        uncertain: false,
                    });
                    continue;
                };
                if native_vocabulary_outranks_contract(dialect, module, imported) {
                    continue;
                }
                let Some(summary) =
                    selected_contract_export(contract, module, summary, environment)
                else {
                    push_environment_dependent_export(
                        &mut missing_exports,
                        module,
                        imported,
                        true,
                        specifier_location,
                    );
                    continue;
                };
                push_unknown_contract_claims(
                    &mut missing_exports,
                    &summary,
                    module,
                    imported,
                    true,
                    specifier_location.clone(),
                );
                let resolved = ResolvedContractBinding {
                    local_name: specifier.exported.to_string(),
                    imported_name: imported.to_owned(),
                    package_name: contract.package.name.clone(),
                    symbol: symbol.clone(),
                    runtime_identity: runtime_identity_at(facts, &specifier_location),
                    contract_location: Location {
                        path: format!("{}#{imported}", contract.source_path).into(),
                        start_byte: 0,
                        end_byte: 0,
                    },
                    summary,
                };
                bindings.push(resolved.clone());
                by_symbol.insert(symbol, resolved);
            }
        }
    }
    join_runtime_identity_aliases(
        facts,
        entities,
        &mut bindings,
        &mut by_symbol,
        &mut missing_exports,
    );
    ResolvedContracts {
        bindings,
        by_symbol,
        missing_exports,
    }
}

pub(super) struct ContractSemantics<'a> {
    pub(super) bundled_returns: &'a HashMap<SymbolId, ContractReturn>,
    pub(super) source_kinds: &'a HashMap<SymbolId, ReactiveSourceKind>,
    pub(super) source_primitives: &'a HashMap<SymbolId, SymbolId>,
}

pub(super) struct ContractGraph<'a> {
    pub(super) nodes: &'a [SummaryNode],
    pub(super) nodes_by_path: &'a HashMap<String, Vec<usize>>,
    pub(super) by_symbol: &'a HashMap<SymbolId, usize>,
    pub(super) entities: &'a EntitySymbols,
}

pub(super) struct ContractAnalysis<'a> {
    pub(super) summaries: &'a [SummaryReads],
    pub(super) returned: &'a [SummaryReads],
    pub(super) structured_returns: &'a [Option<ContractReturn>],
    pub(super) callbacks: &'a [Vec<ContractCallback>],
    /// Per node, the parameters whose caller-supplied value the analysis never
    /// accounted for. Any one of them makes this export's `callbacks` domain
    /// the unknown sentinel — see
    /// `interproc::push_unaccounted_parameter_escapes`.
    pub(super) escaped_parameters: &'a [Vec<usize>],
    pub(super) invoked_parameter_members: &'a [Vec<(usize, String)>],
    pub(super) semantics: ContractSemantics<'a>,
}

/// One node's inputs to [`contract_export_function`], indexed out of a
/// [`ContractAnalysis`].
struct ContractExportNode<'a> {
    node: &'a SummaryNode,
    summary: &'a SummaryReads,
    returned_summary: &'a SummaryReads,
    structured_return: Option<&'a ContractReturn>,
    callbacks: &'a [ContractCallback],
    escaped_parameters: &'a [usize],
    invoked_parameter_members: &'a [(usize, String)],
}

impl<'a> ContractExportNode<'a> {
    fn at(analysis: &ContractAnalysis<'a>, node: &'a SummaryNode, index: usize) -> Self {
        Self {
            node,
            summary: &analysis.summaries[index],
            returned_summary: &analysis.returned[index],
            structured_return: analysis.structured_returns[index].as_ref(),
            callbacks: &analysis.callbacks[index],
            escaped_parameters: &analysis.escaped_parameters[index],
            invoked_parameter_members: &analysis.invoked_parameter_members[index],
        }
    }
}

fn contract_export_function(
    inputs: ContractExportNode<'_>,
    semantics: &ContractSemantics<'_>,
) -> ContractExport {
    let ContractExportNode {
        node,
        summary,
        returned_summary,
        structured_return,
        callbacks,
        escaped_parameters,
        invoked_parameter_members,
    } = inputs;
    let mut seen_reactive_reads = HashSet::new();
    let mut reactive_reads = summary
        .iter()
        .filter_map(|read| {
            let reactive_read = ContractReactiveRead {
                kind: read.kind.clone().unwrap_or_else(|| "accessor".into()),
                label: semantics
                    .source_primitives
                    .get(&read.symbol)
                    .and_then(|primitive| semantics.bundled_returns.get(primitive))
                    .map_or_else(
                        || read.display.to_string(),
                        |returned| returned.label.clone(),
                    ),
                evidence: None,
                parameter: None,
            };
            seen_reactive_reads
                .insert((reactive_read.kind.clone(), reactive_read.label.clone()))
                .then_some(reactive_read)
        })
        .collect::<Vec<_>>();
    for (parameter, _) in invoked_parameter_members {
        if seen_reactive_reads.insert(("parameter-member".into(), parameter.to_string())) {
            reactive_reads.push(ContractReactiveRead {
                kind: "parameter-member".into(),
                label: String::new(),
                parameter: Some(*parameter),
                evidence: None,
            });
        }
    }
    let first_returned =
        returned_summary
            .iter()
            .fold(None::<&SummaryRead>, |current, candidate| match current {
                None => Some(candidate),
                Some(best) if location_order(&candidate.declaration, &best.declaration).is_lt() => {
                    Some(candidate)
                }
                Some(best) => Some(best),
            });
    let returns = structured_return.cloned().or_else(|| {
        first_returned.map(|read| ContractReturn {
            kind: if semantics.source_kinds.get(&read.symbol) == Some(&ReactiveSourceKind::Store) {
                "store-path".into()
            } else {
                "accessor".into()
            },
            label: semantics
                .source_primitives
                .get(&read.symbol)
                .and_then(|primitive| semantics.bundled_returns.get(primitive))
                .map_or_else(
                    || read.display.to_string(),
                    |returned| returned.label.clone(),
                ),
            parameter: None,
            elements: Vec::new(),
            properties: BTreeMap::new(),
            evidence: None,
        })
    });
    let mut callback_summary = callbacks.to_vec();
    callback_summary.sort_by_key(|callback| callback.parameter);
    // An omitted `callbacks` list is a negative claim. Where a caller-supplied
    // parameter escaped without being accounted for, the honest list is not the
    // rows that were proven -- it is "unknown".
    let callbacks = if escaped_parameters.is_empty() {
        callback_summary.into()
    } else {
        ContractClaim::Unknown(ContractUnknownClaim::new())
    };
    ContractExport {
        kind: "function".into(),
        evidence: None,
        variants: Vec::new(),
        reactive_reads: reactive_reads.into(),
        callbacks,
        owner_requirements: Vec::new().into(),
        returns: returns.into(),
        async_behavior: if node.r#async {
            String::from("promise").into()
        } else {
            String::new().into()
        },
    }
}

fn resolve_local_reexport(
    facts: &ProjectFacts,
    graph: &ContractGraph<'_>,
    by_symbol: &HashMap<SymbolId, ContractExport>,
    source_file: &solid_facts::FileFacts,
    module: &str,
    name: &str,
) -> Option<(ContractExport, usize)> {
    let mut visiting = HashSet::new();
    resolve_local_reexport_with_visiting(
        facts,
        graph,
        by_symbol,
        source_file,
        module,
        name,
        &mut visiting,
    )
}

fn resolve_local_import(
    facts: &ProjectFacts,
    graph: &ContractGraph<'_>,
    by_symbol: &HashMap<SymbolId, ContractExport>,
    source_file: &solid_facts::FileFacts,
    local_name: &str,
) -> Option<(ContractExport, usize)> {
    let mut visiting = HashSet::new();
    resolve_local_import_with_visiting(
        facts,
        graph,
        by_symbol,
        source_file,
        local_name,
        &mut visiting,
    )
}

fn resolve_local_binding_initializer(
    facts: &ProjectFacts,
    file: &solid_facts::FileFacts,
    graph: &ContractGraph<'_>,
    by_symbol: &HashMap<SymbolId, ContractExport>,
    local: solid_facts::core::Span,
) -> Option<(ContractExport, usize)> {
    let local_symbol = graph.entities.get(&location(file.path.shared(), local));
    let binding = file.ast.bindings.iter().find(|binding| {
        binding.names.iter().any(|name| {
            name.span == local
                || local_symbol.is_some_and(|symbol| {
                    graph.entities.get(&location(file.path.shared(), name.span)) == Some(symbol)
                })
        })
    })?;
    let initializer = binding.initializer?;
    graph
        .entities
        .get(&location(file.path.shared(), initializer))
        .and_then(|symbol| {
            graph.by_symbol.get(symbol).and_then(|index| {
                by_symbol
                    .get(symbol)
                    .cloned()
                    .map(|summary| (summary, *index))
            })
        })
        .or_else(|| {
            file.source_text(initializer)
                .and_then(|name| resolve_local_import(facts, graph, by_symbol, file, name))
        })
}

fn resolve_local_import_with_visiting(
    facts: &ProjectFacts,
    graph: &ContractGraph<'_>,
    by_symbol: &HashMap<SymbolId, ContractExport>,
    source_file: &solid_facts::FileFacts,
    local_name: &str,
    visiting: &mut HashSet<(String, String)>,
) -> Option<(ContractExport, usize)> {
    for import in source_file
        .ast
        .imports
        .iter()
        .filter(|import| !import.type_only)
    {
        for binding in import.bindings.iter().filter(|binding| !binding.type_only) {
            if source_file.source_text(binding.local.span) != Some(local_name) {
                continue;
            }
            let imported_name = binding.imported.as_deref().or_else(|| {
                (binding.kind == solid_facts::ast::ImportKind::Default).then_some("default")
            })?;
            if import.module.starts_with('.') {
                return resolve_local_reexport_with_visiting(
                    facts,
                    graph,
                    by_symbol,
                    source_file,
                    import.module.as_str(),
                    imported_name,
                    visiting,
                );
            }
        }
    }
    None
}

fn resolve_local_reexport_with_visiting(
    facts: &ProjectFacts,
    graph: &ContractGraph<'_>,
    by_symbol: &HashMap<SymbolId, ContractExport>,
    source_file: &solid_facts::FileFacts,
    module: &str,
    name: &str,
    visiting: &mut HashSet<(String, String)>,
) -> Option<(ContractExport, usize)> {
    let source_path = Path::new(source_file.path.as_str());
    let target_path = source_path.parent()?.join(module).canonicalize().ok()?;
    let target = facts.files.iter().find(|file| {
        Path::new(file.path.as_str()).canonicalize().ok().as_ref() == Some(&target_path)
    })?;
    resolve_named_export(facts, graph, by_symbol, target, name, visiting)
}

fn resolve_named_export(
    facts: &ProjectFacts,
    graph: &ContractGraph<'_>,
    by_symbol: &HashMap<SymbolId, ContractExport>,
    file: &solid_facts::FileFacts,
    name: &str,
    visiting: &mut HashSet<(String, String)>,
) -> Option<(ContractExport, usize)> {
    if !visiting.insert((file.path.to_string(), name.to_owned())) {
        return None;
    }
    for index in graph
        .nodes_by_path
        .get(file.path.as_str())
        .into_iter()
        .flatten()
        .copied()
    {
        let node = &graph.nodes[index];
        if node.exported
            && node.name.as_deref() == Some(name)
            && let Some(symbol) = &node.symbol
            && let Some(summary) = by_symbol.get(symbol)
        {
            return Some((summary.clone(), index));
        }
    }
    for export in file.ast.exports.iter().filter(|export| !export.type_only) {
        for specifier in export
            .specifiers
            .iter()
            .chain(export.declarations.iter())
            .filter(|specifier| !specifier.type_only && specifier.exported.as_str() == name)
        {
            let local_name = file.source_text(specifier.local.span).unwrap_or(name);
            if let Some(module) = export.module.as_deref()
                && let Some(summary) = resolve_local_reexport_with_visiting(
                    facts, graph, by_symbol, file, module, local_name, visiting,
                )
            {
                return Some(summary);
            }
            if let Some(summary) = resolve_local_import_with_visiting(
                facts, graph, by_symbol, file, local_name, visiting,
            ) {
                return Some(summary);
            }
            if let Some(summary) = resolve_local_binding_initializer(
                facts,
                file,
                graph,
                by_symbol,
                specifier.local.span,
            ) {
                return Some(summary);
            }
            if let Some(resolved) = graph
                .entities
                .get(&location(file.path.shared(), specifier.local.span))
                .and_then(|symbol| {
                    graph.by_symbol.get(symbol).and_then(|index| {
                        by_symbol
                            .get(symbol)
                            .cloned()
                            .map(|summary| (summary, *index))
                    })
                })
            {
                return Some(resolved);
            }
        }
        if export.kind == solid_facts::ast::ExportKind::All
            && let Some(module) = export.module.as_deref()
            && let Some(summary) = resolve_local_reexport_with_visiting(
                facts, graph, by_symbol, file, module, name, visiting,
            )
        {
            return Some(summary);
        }
    }
    None
}

fn contract_export_fragment(
    facts: &ProjectFacts,
    file: &solid_facts::FileFacts,
    project_directory: Option<&Path>,
    graph: &ContractGraph<'_>,
    node_keys: &[ContractNodeKey],
    node_contracts: &HashMap<ContractNodeKey, ContractExport>,
    by_symbol: &HashMap<SymbolId, ContractExport>,
) -> ContractExportFragment {
    let mut fragment = ContractExportFragment::default();
    if project_directory
        .is_some_and(|directory| !path_within_project(Path::new(file.path.as_str()), directory))
    {
        return fragment;
    }
    for index in graph
        .nodes_by_path
        .get(file.path.as_str())
        .into_iter()
        .flatten()
        .copied()
    {
        let node = &graph.nodes[index];
        if node.exported
            && let (Some(name), Some(symbol)) = (&node.name, &node.symbol)
            && let Some(target) = graph.by_symbol.get(symbol).copied()
            && let Some(summary) = node_contracts.get(&node_keys[target])
        {
            fragment.dependencies.insert(node_keys[target].clone());
            fragment.direct.push((name.clone(), summary.clone()));
        }
    }
    for export in file.ast.exports.iter().filter(|export| !export.type_only) {
        for specifier in export
            .specifiers
            .iter()
            .chain(export.declarations.iter())
            .filter(|specifier| !specifier.type_only)
        {
            let target = graph
                .entities
                .get(&location(file.path.shared(), specifier.local.span))
                .and_then(|symbol| graph.by_symbol.get(symbol))
                .copied();
            let summary = export
                .module
                .as_deref()
                .and_then(|module| {
                    let local_name = file
                        .source_text(specifier.local.span)
                        .unwrap_or(specifier.exported.as_str());
                    resolve_local_reexport(facts, graph, by_symbol, file, module, local_name)
                })
                .or_else(|| {
                    let local_name = file
                        .source_text(specifier.local.span)
                        .unwrap_or(specifier.exported.as_str());
                    resolve_local_import(facts, graph, by_symbol, file, local_name)
                })
                .or_else(|| {
                    resolve_local_binding_initializer(
                        facts,
                        file,
                        graph,
                        by_symbol,
                        specifier.local.span,
                    )
                })
                .map(|(summary, index)| {
                    fragment.dependencies.insert(node_keys[index].clone());
                    summary
                })
                .or_else(|| {
                    target.and_then(|index| {
                        fragment.dependencies.insert(node_keys[index].clone());
                        node_contracts.get(&node_keys[index]).cloned()
                    })
                })
                .unwrap_or_else(value_contract_export);
            let summary = promote_callable_export(facts, file, specifier.local.span, summary);
            fragment
                .syntax
                .push((specifier.exported.to_string(), summary, true));
        }
        for binding in file.ast.exported_bindings(export) {
            for name in &binding.names {
                let target = graph
                    .entities
                    .get(&location(file.path.shared(), name.span))
                    .and_then(|symbol| graph.by_symbol.get(symbol))
                    .copied();
                let summary = target
                    .and_then(|index| {
                        fragment.dependencies.insert(node_keys[index].clone());
                        node_contracts.get(&node_keys[index]).cloned()
                    })
                    .or_else(|| {
                        resolve_local_binding_initializer(facts, file, graph, by_symbol, name.span)
                            .map(|(summary, index)| {
                                fragment.dependencies.insert(node_keys[index].clone());
                                summary
                            })
                    })
                    .unwrap_or_else(value_contract_export);
                let summary = promote_callable_export(facts, file, name.span, summary);
                fragment.syntax.push((
                    file.source_text(name.span).unwrap_or_default().to_owned(),
                    summary,
                    false,
                ));
            }
        }
    }
    fragment
}

pub(super) fn contract_export_summaries_incremental(
    cache: &mut CachedContractExports,
    facts: &ProjectFacts,
    graph: &ContractGraph<'_>,
    reverse_edges: &[Vec<usize>],
    graph_node_reused_paths: &HashSet<&str>,
    changed_semantic_symbols: Option<&HashSet<SymbolId>>,
    analysis: &ContractAnalysis<'_>,
) -> Arc<BTreeMap<String, ContractExport>> {
    let mut ordinals = HashMap::<&str, usize>::new();
    let node_keys = graph
        .nodes
        .iter()
        .map(|node| {
            let ordinal = ordinals.entry(node.path.as_str()).or_default();
            let key = ContractNodeKey {
                path: node.path.clone(),
                ordinal: *ordinal,
            };
            *ordinal += 1;
            key
        })
        .collect::<Vec<_>>();
    let current_keys = node_keys.iter().cloned().collect::<HashSet<_>>();
    let mut dirty = graph
        .nodes
        .iter()
        .enumerate()
        .filter_map(|(index, node)| {
            (!graph_node_reused_paths.contains(node.path.as_str())
                || !cache.nodes.contains_key(&node_keys[index])
                || changed_semantic_symbols.is_some_and(|changed| {
                    analysis.summaries[index]
                        .iter()
                        .chain(analysis.returned[index].iter())
                        .any(|read| changed.contains(&read.symbol))
                }))
            .then_some(index)
        })
        .collect::<Vec<_>>();
    let mut dirty_set = dirty.iter().copied().collect::<HashSet<_>>();
    while let Some(target) = dirty.pop() {
        for owner in reverse_edges.get(target).into_iter().flatten().copied() {
            if dirty_set.insert(owner) {
                dirty.push(owner);
            }
        }
    }
    let mut dirty_indices = dirty_set.into_iter().collect::<Vec<_>>();
    dirty_indices.sort_unstable();
    let rebuilt_nodes = parallel_slice_results(&dirty_indices, |index| {
        contract_export_function(
            ContractExportNode::at(analysis, &graph.nodes[*index], *index),
            &analysis.semantics,
        )
    });
    let mut changed_nodes = HashSet::<ContractNodeKey>::new();
    for (index, contract) in dirty_indices.into_iter().zip(rebuilt_nodes) {
        let key = node_keys[index].clone();
        if cache.nodes.get(&key) != Some(&contract) {
            changed_nodes.insert(key.clone());
            cache.nodes.insert(key, contract);
        }
    }
    let removed_nodes = cache
        .nodes
        .keys()
        .filter(|key| !current_keys.contains(*key))
        .cloned()
        .collect::<Vec<_>>();
    for key in removed_nodes {
        cache.nodes.remove(&key);
        changed_nodes.insert(key);
    }
    let current_paths = facts
        .files
        .iter()
        .map(|file| file.path.as_str())
        .collect::<HashSet<_>>();
    let removed_files = cache
        .files
        .keys()
        .filter(|path| !current_paths.contains(path.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    let mut fragments_changed = !removed_files.is_empty();
    for path in removed_files {
        cache.files.remove(&path);
    }
    let project_directory = Path::new(&facts.project_id).parent();
    let by_symbol = graph
        .by_symbol
        .iter()
        .filter_map(|(symbol, index)| {
            cache
                .nodes
                .get(&node_keys[*index])
                .cloned()
                .map(|summary| (symbol.clone(), summary))
        })
        .collect::<HashMap<_, _>>();
    let rebuild_files = facts
        .files
        .iter()
        .filter(|file| {
            !graph_node_reused_paths.contains(file.path.as_str())
                || cache
                    .files
                    .get(file.path.as_str())
                    .is_none_or(|fragment| !fragment.dependencies.is_disjoint(&changed_nodes))
        })
        .collect::<Vec<_>>();
    let rebuilt_fragments = parallel_slice_results(&rebuild_files, |file| {
        contract_export_fragment(
            facts,
            file,
            project_directory,
            graph,
            &node_keys,
            &cache.nodes,
            &by_symbol,
        )
    });
    for (file, fragment) in rebuild_files.into_iter().zip(rebuilt_fragments) {
        fragments_changed |= cache.files.get(file.path.as_str()) != Some(&fragment);
        cache.files.insert(file.path.to_string(), fragment);
    }
    if !fragments_changed && let Some(aggregate) = &cache.aggregate {
        return Arc::clone(aggregate);
    }
    let aggregate = aggregate_contract_fragments(facts, &cache.files);
    let aggregate = Arc::new(aggregate);
    cache.aggregate = Some(Arc::clone(&aggregate));
    aggregate
}

pub(super) fn contract_export_summaries(
    facts: &ProjectFacts,
    graph: &ContractGraph<'_>,
    analysis: &ContractAnalysis<'_>,
) -> BTreeMap<String, ContractExport> {
    let mut ordinals = HashMap::<&str, usize>::new();
    let node_keys = graph
        .nodes
        .iter()
        .map(|node| {
            let ordinal = ordinals.entry(node.path.as_str()).or_default();
            let key = ContractNodeKey {
                path: node.path.clone(),
                ordinal: *ordinal,
            };
            *ordinal += 1;
            key
        })
        .collect::<Vec<_>>();
    let node_contracts = graph
        .nodes
        .iter()
        .enumerate()
        .map(|(index, node)| {
            (
                node_keys[index].clone(),
                contract_export_function(
                    ContractExportNode::at(analysis, node, index),
                    &analysis.semantics,
                ),
            )
        })
        .collect::<HashMap<_, _>>();
    let by_symbol = graph
        .by_symbol
        .iter()
        .filter_map(|(symbol, index)| {
            node_contracts
                .get(&node_keys[*index])
                .cloned()
                .map(|summary| (symbol.clone(), summary))
        })
        .collect::<HashMap<_, _>>();
    let project_directory = Path::new(&facts.project_id).parent();
    let fragments = facts
        .files
        .iter()
        .map(|file| {
            (
                file.path.to_string(),
                contract_export_fragment(
                    facts,
                    file,
                    project_directory,
                    graph,
                    &node_keys,
                    &node_contracts,
                    &by_symbol,
                ),
            )
        })
        .collect::<HashMap<_, _>>();
    aggregate_contract_fragments(facts, &fragments)
}

fn aggregate_contract_fragments(
    facts: &ProjectFacts,
    fragments: &HashMap<String, ContractExportFragment>,
) -> BTreeMap<String, ContractExport> {
    let mut aggregate = BTreeMap::new();
    for file in &facts.files {
        if let Some(fragment) = fragments.get(file.path.as_str()) {
            for (name, summary) in &fragment.direct {
                aggregate.insert(name.clone(), summary.clone());
            }
        }
    }
    for file in &facts.files {
        if let Some(fragment) = fragments.get(file.path.as_str()) {
            for (name, summary, replace) in &fragment.syntax {
                if *replace {
                    aggregate.insert(name.clone(), summary.clone());
                } else {
                    aggregate
                        .entry(name.clone())
                        .or_insert_with(|| summary.clone());
                }
            }
        }
    }
    aggregate
}

fn path_within_project(path: &Path, directory: &Path) -> bool {
    path.starts_with(directory)
        || path.canonicalize().is_ok_and(|path| {
            directory
                .canonicalize()
                .is_ok_and(|directory| path.starts_with(directory))
        })
}

fn value_contract_export() -> ContractExport {
    ContractExport {
        kind: "value".into(),
        variants: Vec::new(),
        ..ContractExport::default()
    }
}

/// Whether the binding at `target` is a class.
///
/// Constructability is not callability. [`Callability`] is derived from
/// `GetSignaturesOfType(…, SignatureKindCall)` over the actual union
/// constituents, and a class type carries construct signatures and no call
/// signature, so every exported class answers `nonCallable` there. At runtime
/// `typeof C === "function"` for every class — which is exactly what a
/// contract's `kind` describes and exactly what `contract probe` measures — so
/// the callability answer alone makes the generator publish `kind: "value"`
/// for a class and the probe contradict it.
///
/// Two exact facts answer the real question, and neither is name-, text- or
/// type-text-based: the compiler's own declaration kind for the resolved
/// symbol (`ast.IsClassDeclaration`), and this project's class-name spans from
/// the syntax facts. The syntax fact is consulted first because it needs no
/// symbol demand; the symbol declarations reach a class declared in another
/// analyzed file.
pub fn binding_declares_class(facts: &ProjectFacts, target: &Location) -> bool {
    if location_is_class_name(facts, target) {
        return true;
    }
    let Some(entity) = entity_at(facts, target) else {
        return false;
    };
    if entity.symbol.is_empty() {
        return false;
    }
    // A barrel's `export { ChildError }` binds an *alias* symbol whose own
    // declaration is the import specifier, and a bundler's `const Query =
    // BaseQueryBuilder` binds a variable whose initializer is the class. Both
    // hops are followed by exact symbol identity — an alias target the
    // compiler recorded, and the binder's own initializer reference — never by
    // a name. The walk is bounded by the symbols it has already visited.
    let mut pending = vec![entity.symbol.to_string()];
    let mut visited = HashSet::new();
    while let Some(symbol) = pending.pop() {
        if !visited.insert(symbol.clone()) {
            continue;
        }
        let Some(fact) = facts.typescript.symbol(&symbol) else {
            continue;
        };
        for declaration in fact.declarations() {
            if declaration.kind.as_ref() == "class"
                || location_is_class_name(facts, &declaration.location)
            {
                return true;
            }
            if let Some(next) = binding_initializer_symbol(facts, &declaration.location) {
                pending.push(next);
            }
        }
        let alias = fact.alias_target();
        if !alias.is_empty() {
            pending.push(alias.to_owned());
        }
    }
    false
}

/// The symbol a `const Alias = Original` declaration copies its value from.
///
/// Only the identifier form: an initializer that is a call, an object, or any
/// other expression is a different runtime value, not this declaration under
/// another name.
fn binding_initializer_symbol(facts: &ProjectFacts, target: &Location) -> Option<String> {
    let span = span_of(target);
    let file = facts
        .files
        .iter()
        .find(|file| file.path.as_str() == target.path.as_ref())?;
    let initializer = file
        .ast
        .bindings
        .iter()
        .find(|binding| binding.names.iter().any(|name| name.span == span))?
        .initializer_identifier
        .as_ref()?;
    let entity = entity_at(facts, &location(file.path.shared(), initializer.span))?;
    (!entity.symbol.is_empty()).then(|| entity.symbol.to_string())
}

fn span_of(target: &Location) -> solid_facts::core::Span {
    solid_facts::core::Span::new(
        u32::try_from(target.start_byte).unwrap_or(u32::MAX),
        u32::try_from(target.end_byte).unwrap_or(u32::MAX),
    )
}

fn location_is_class_name(facts: &ProjectFacts, target: &Location) -> bool {
    let span = span_of(target);
    facts
        .files
        .iter()
        .any(|file| file.path.as_str() == target.path.as_ref() && file.ast.declares_class_at(span))
}

fn entity_at<'a>(facts: &'a ProjectFacts, target: &Location) -> Option<&'a typefacts::EntityFact> {
    facts.typescript.entities().find(|entity| {
        entity.location.path == target.path
            && entity.location.start_byte == target.start_byte
            && entity.location.end_byte == target.end_byte
    })
}

/// A class export's honest summary: `kind: "function"`, callbacks fail closed.
///
/// The generator summarizes function declarations, not construct signatures.
/// Nothing here carries what a constructor — the class's own, or the one it
/// inherits through `extends` — does with the arguments a caller passes, and
/// an omitted `callbacks` list is a *negative* claim ("invokes no
/// caller-supplied function"). A consumer reads `new Store(onChange)` through
/// exactly the same contract path as `store(onChange)`, so publishing that
/// silence would certify inertness the class can contradict. The sentinel is
/// demand-sensitive at the consumer: constructing with no callable argument
/// stays clean.
pub fn class_contract_export(mut summary: ContractExport) -> ContractExport {
    summary.kind = "function".into();
    summary.callbacks = ContractClaim::Unknown(ContractUnknownClaim::new());
    summary
}

fn promote_callable_export(
    facts: &ProjectFacts,
    file: &solid_facts::FileFacts,
    span: solid_facts::core::Span,
    summary: ContractExport,
) -> ContractExport {
    if summary.kind != "value" {
        return summary;
    }
    let target = location(file.path.shared(), span);
    if binding_declares_class(facts, &target) {
        return class_contract_export(summary);
    }
    let Some(entity) = entity_at(facts, &target) else {
        return summary;
    };
    let mut summary = summary;
    if entity.callability == Some(Callability::Callable) {
        summary.kind = "function".into();
    }
    summary
}

#[cfg(test)]
mod variant_selection_tests {
    use super::{ContractExport, ContractExportVariant, selected_variant};

    fn variant(conditions: &[&str], precedence: Option<u32>) -> ContractExportVariant {
        ContractExportVariant {
            conditions: conditions.iter().map(|value| (*value).to_owned()).collect(),
            summary: Box::new(ContractExport {
                kind: conditions.join("+"),
                ..ContractExport::default()
            }),
            precedence,
        }
    }

    #[test]
    fn generated_variants_resolve_by_export_map_order() {
        // Both branches match a development environment once `default` is
        // satisfiable; `precedence` is the export map's own first-match-wins
        // order, so the earlier `development` branch wins.
        let development = variant(&["development"], Some(0));
        let fallback = variant(&["default"], Some(1));
        let matching = [&fallback, &development];
        assert_eq!(
            selected_variant(&matching).map(|winner| winner.summary.kind.as_str()),
            Some("development")
        );
    }

    #[test]
    fn a_handwritten_named_branch_beats_the_unconditional_fallback() {
        // No `precedence` anywhere: the only thing provable without inventing
        // an order is that an explicitly named branch is more specific than
        // the unconditional fallback.
        let browser = variant(&["browser"], None);
        let fallback = variant(&["default"], None);
        let matching = [&fallback, &browser];
        assert_eq!(
            selected_variant(&matching).map(|winner| winner.summary.kind.as_str()),
            Some("browser")
        );
    }

    #[test]
    fn ambiguity_with_no_recorded_order_stays_fail_closed() {
        // Two named branches and nothing that says which one the resolver
        // picks, and a tie in `precedence`, both stay undetermined.
        let browser = variant(&["browser"], None);
        let node = variant(&["node"], None);
        let matching = [&browser, &node];
        assert!(selected_variant(&matching).is_none());

        let left = variant(&["browser"], Some(2));
        let right = variant(&["node"], Some(2));
        let tied = [&left, &right];
        assert!(selected_variant(&tied).is_none());
    }
}
