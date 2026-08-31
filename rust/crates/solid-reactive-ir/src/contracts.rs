//! Package-contract resolution and export-summary construction.
//!
//! Resolves imported contract bindings to local symbols (`resolve_contract_imports`)
//! and turns the interprocedural summaries into the per-export contract artifacts
//! that a downstream package sees. Owns both the full and incremental summary
//! builds; the public contract data types stay in the crate root.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    path::Path,
    sync::Arc,
};

use solid_dialect::Dialect;
use solid_facts::ProjectFacts;
use typefacts::{Callability, Constructability, Location, ReferenceSpace, RuntimeBindingKind};

use super::{
    ContractCallback, ContractClaim, ContractExport, ContractOwnerRequirement,
    ContractReactiveRead, ContractReturn, EntitySymbols, OwnerRequirementOperation,
    PackageContract, ReactiveSourceKind, StaticDefect, StaticDefectKind, SummaryNode, SummaryRead,
    SummaryReads, SymbolId, location, location_order,
};
use crate::cache::{CachedContractExports, ContractExportFragment, ContractNodeKey};
use crate::contract_semantics::{
    AcceptedContractIndex, AcceptedContractUse, ClaimDomain, KnowledgeSet, OperationKind,
    OwnerSource, Requirement, Schedule, Tracking, UncertifiableImportReason, ValueShape,
    ValueSource,
};
use crate::identity::symbol_id;
use crate::pipeline::parallel_slice_results;

/// Projects an already receipt-validated normalized export into the compact
/// indexes used by the current interprocedural solver. This is deliberately
/// downstream of exact import/artifact selection: no schema version, summary
/// ID, condition label, evidence spelling, or closure-array mechanic is inspected.
pub fn project_accepted_export(accepted: &AcceptedContractUse<'_>) -> ContractExport {
    let export = accepted.export();
    let mut open_claims = BTreeSet::new();
    let kind = if matches!(export.shape, ValueShape::Callable | ValueShape::Component) {
        "function"
    } else {
        "value"
    };

    let callbacks = project_callbacks(export, &mut open_claims);
    let reactive_reads = project_reactive_reads(export, &mut open_claims);
    let returns = project_return(export, &mut open_claims);
    let owner_requirements = project_owner_requirements(export, &mut open_claims);
    let async_behavior = project_async_behavior(export, &mut open_claims);

    ContractExport {
        kind: kind.into(),
        reactive_reads,
        returns,
        callbacks,
        owner_requirements,
        async_behavior,
        open_claims,
    }
}

fn project_callbacks(
    export: &crate::contract_semantics::ExportSemantics,
    open: &mut BTreeSet<ClaimDomain>,
) -> ContractClaim<Vec<ContractCallback>> {
    let knowledge = export.callbacks();
    if !knowledge.is_closed() {
        open.insert(ClaimDomain::Callbacks);
    }
    let mut callbacks = Vec::new();
    for callback in knowledge.items() {
        let ValueSource::Parameter { index, .. } = callback.from else {
            open.insert(ClaimDomain::Callbacks);
            continue;
        };
        let Some(operation) = export.operation(&callback.operation.0) else {
            open.insert(ClaimDomain::Callbacks);
            continue;
        };
        let Some(execution) = projected_execution(operation) else {
            open.insert(ClaimDomain::Callbacks);
            continue;
        };
        callbacks.push(ContractCallback {
            parameter: usize::from(index),
            execution: execution.into(),
            arguments: operation.inputs.iter().map(project_return_shape).collect(),
            owner: match operation.owner.source {
                OwnerSource::None => Some("none".into()),
                OwnerSource::Created(_)
                    if operation.owner.requirements.child_owners == Requirement::Forbidden
                        && operation.owner.requirements.cleanup == Requirement::Forbidden =>
                {
                    Some("leaf".into())
                }
                OwnerSource::Created(_) => Some("created".into()),
                OwnerSource::Captured(_)
                | OwnerSource::AmbientAtCall
                | OwnerSource::AmbientAtExecution => Some("inherited".into()),
                OwnerSource::Unknown => None,
            },
        });
    }
    callbacks.sort_by_key(|callback| callback.parameter);
    match knowledge {
        KnowledgeSet::Unknown if callbacks.is_empty() => ContractClaim::Open,
        _ => ContractClaim::Known(callbacks),
    }
}

fn projected_execution(operation: &crate::contract_semantics::Operation) -> Option<&'static str> {
    if operation.tracking == Tracking::Tracked {
        return Some("tracked");
    }
    match operation.schedule {
        Some(Schedule::SameStack) => Some("inline"),
        Some(Schedule::Queued | Schedule::External) => Some("deferred"),
        None => None,
    }
}

fn project_reactive_reads(
    export: &crate::contract_semantics::ExportSemantics,
    open: &mut BTreeSet<ClaimDomain>,
) -> ContractClaim<Vec<ContractReactiveRead>> {
    let knowledge = export
        .operation_claim(ClaimDomain::Reads)
        .expect("reads is an operation domain");
    if !knowledge.is_closed() {
        open.insert(ClaimDomain::Reads);
    }
    let mut reads = Vec::new();
    for id in knowledge.items() {
        let Some(operation) = export.operation(&id.0) else {
            open.insert(ClaimDomain::Reads);
            continue;
        };
        match operation.inputs.first() {
            Some(ValueShape::Parameter { index, path }) => reads.push(ContractReactiveRead {
                kind: "parameter-member".into(),
                label: String::new(),
                parameter: Some(usize::from(*index)),
                member: path.last().cloned(),
            }),
            Some(ValueShape::Reactive { .. }) => reads.push(ContractReactiveRead {
                kind: "accessor".into(),
                label: "normalized reactive read".into(),
                parameter: None,
                member: None,
            }),
            Some(ValueShape::Store { .. }) => reads.push(ContractReactiveRead {
                kind: "store-path".into(),
                label: "normalized store read".into(),
                parameter: None,
                member: None,
            }),
            _ => {
                open.insert(ClaimDomain::Reads);
            }
        }
    }
    match knowledge {
        KnowledgeSet::Unknown if reads.is_empty() => ContractClaim::Open,
        _ => ContractClaim::Known(reads),
    }
}

fn project_return(
    export: &crate::contract_semantics::ExportSemantics,
    open: &mut BTreeSet<ClaimDomain>,
) -> ContractClaim<Option<ContractReturn>> {
    let knowledge = export
        .operation_claim(ClaimDomain::Returns)
        .expect("returns is an operation domain");
    if !knowledge.is_closed() {
        open.insert(ClaimDomain::Returns);
    }
    let mut returns = knowledge
        .items()
        .iter()
        .filter_map(|id| export.operation(&id.0))
        .filter_map(|operation| operation.output.as_ref())
        .filter_map(project_return_shape)
        .collect::<Vec<_>>();
    returns.sort_by(|left, right| format!("{left:?}").cmp(&format!("{right:?}")));
    returns.dedup();
    match (knowledge, returns.as_slice()) {
        (KnowledgeSet::Complete(items), []) if items.is_empty() => ContractClaim::Known(None),
        (KnowledgeSet::Unknown, []) => ContractClaim::Open,
        (_, [returned]) => ContractClaim::Known(Some(returned.clone())),
        _ => {
            open.insert(ClaimDomain::Returns);
            ContractClaim::Open
        }
    }
}

fn project_return_shape(shape: &ValueShape) -> Option<ContractReturn> {
    match shape {
        ValueShape::Reactive { .. } => Some(ContractReturn {
            kind: "accessor".into(),
            label: "normalized reactive result".into(),
            ..ContractReturn::default()
        }),
        ValueShape::Store { .. } => Some(ContractReturn {
            kind: "store-path".into(),
            label: "normalized store result".into(),
            ..ContractReturn::default()
        }),
        ValueShape::Parameter { index, .. } => Some(ContractReturn {
            kind: "argument".into(),
            parameter: Some(usize::from(*index)),
            ..ContractReturn::default()
        }),
        ValueShape::Tuple(KnowledgeSet::Complete(items)) => Some(ContractReturn {
            kind: "tuple".into(),
            elements: items.iter().map(project_return_shape).collect(),
            ..ContractReturn::default()
        }),
        ValueShape::Object(KnowledgeSet::Complete(properties)) => Some(ContractReturn {
            kind: "object".into(),
            properties: properties
                .iter()
                .filter_map(|property| {
                    project_return_shape(&property.value)
                        .map(|value| (property.name.clone(), value))
                })
                .collect(),
            ..ContractReturn::default()
        }),
        ValueShape::Promise(value) | ValueShape::AsyncIterable(value) => {
            project_return_shape(value)
        }
        ValueShape::Unknown
        | ValueShape::Plain
        | ValueShape::Tuple(_)
        | ValueShape::Array { .. }
        | ValueShape::Object(_)
        | ValueShape::Choice(_)
        | ValueShape::Callable
        | ValueShape::Action { .. }
        | ValueShape::Component
        | ValueShape::Cleanup { .. }
        | ValueShape::RefApplication
        | ValueShape::ServerFunctionReference { .. } => None,
    }
}

fn project_owner_requirements(
    export: &crate::contract_semantics::ExportSemantics,
    open: &mut BTreeSet<ClaimDomain>,
) -> ContractClaim<Vec<ContractOwnerRequirement>> {
    let knowledge = export
        .operation_claim(ClaimDomain::Creates)
        .expect("creates is an operation domain");
    if !knowledge.is_closed() {
        open.insert(ClaimDomain::Creates);
    }
    let mut requirements = Vec::new();
    for operation in knowledge
        .items()
        .iter()
        .filter_map(|id| export.operation(&id.0))
    {
        if operation.owner.requirements.owner == Requirement::Required {
            let operation = match operation.kind {
                OperationKind::Cleanup | OperationKind::Dispose => {
                    OwnerRequirementOperation::Cleanup
                }
                _ => OwnerRequirementOperation::Effect,
            };
            requirements.push(ContractOwnerRequirement { operation });
        }
    }
    requirements.sort_by_key(|requirement| format!("{:?}", requirement.operation));
    requirements.dedup_by_key(|requirement| requirement.operation);
    match knowledge {
        KnowledgeSet::Unknown if requirements.is_empty() => ContractClaim::Open,
        _ => ContractClaim::Known(requirements),
    }
}

fn project_async_behavior(
    export: &crate::contract_semantics::ExportSemantics,
    open: &mut BTreeSet<ClaimDomain>,
) -> ContractClaim<String> {
    let mut protocol = None;
    let returns = export
        .operation_claim(ClaimDomain::Returns)
        .expect("returns is an operation domain");
    if !returns.is_closed() {
        open.insert(ClaimDomain::Returns);
    }
    for operation in returns.items() {
        match export
            .operation(&operation.0)
            .and_then(|operation| operation.output.as_ref())
        {
            Some(ValueShape::Promise(_)) => protocol = Some("promise"),
            Some(ValueShape::AsyncIterable(_)) => protocol = Some("async-iterable"),
            _ => {}
        }
    }
    ContractClaim::Known(protocol.unwrap_or_default().into())
}
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
    /// How binding answered per declaration, so a refusal is countable rather
    /// than merely silent. See [`crate::ContractBindingCounts`].
    pub(super) counts: crate::ContractBindingCounts,
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

fn missing_accepted_export_needs_obligation(
    dialect: &dyn Dialect,
    module: &str,
    imported: &str,
) -> bool {
    !native_vocabulary_outranks_contract(dialect, module, imported)
}

/// Keep exact, artifact-selected callback timing from an accepted package
/// contract even when the dialect owns the rest of the primitive. Ownership,
/// returns, reads, and async behavior remain native.
fn native_callback_overlay(summary: &ContractExport) -> Option<ContractExport> {
    let callbacks = summary.callbacks.known()?.clone();
    (!callbacks.is_empty()).then(|| ContractExport {
        kind: summary.kind.clone(),
        callbacks: ContractClaim::Known(callbacks),
        ..ContractExport::default()
    })
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
    if summary.reactive_reads.is_open()
        || summary
            .open_claims
            .contains(&crate::contract_semantics::ClaimDomain::Reads)
    {
        claims.push("reactiveReads");
    }
    if summary.returns.is_open()
        || summary
            .open_claims
            .contains(&crate::contract_semantics::ClaimDomain::Returns)
    {
        claims.push("returns");
    }
    if summary.owner_requirements.is_open()
        || summary
            .open_claims
            .contains(&crate::contract_semantics::ClaimDomain::Creates)
    {
        claims.push("ownerRequirements");
    }
    if summary.async_behavior.is_open()
        || summary
            .open_claims
            .contains(&crate::contract_semantics::ClaimDomain::Throws)
    {
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

pub(super) fn resolve_accepted_contract_imports(
    facts: &ProjectFacts,
    contracts: &AcceptedContractIndex,
    entities: &EntitySymbols,
    dialect: &dyn Dialect,
) -> ResolvedContracts {
    let projected = project_accepted_contracts(facts, contracts);
    resolve_contract_imports_inner(facts, &projected, contracts, entities, dialect)
}

pub(super) fn accepted_bundled_returns(
    facts: &ProjectFacts,
    contracts: &AcceptedContractIndex,
) -> HashMap<SymbolId, ContractReturn> {
    let mut returned = HashMap::new();
    for file in &facts.files {
        if !file
            .ast
            .imports
            .iter()
            .any(|import| !import.type_only && import.module.as_str() == "solid-js")
        {
            continue;
        }
        let Ok(contract) = contracts.contract(file.path.as_str(), "solid-js") else {
            continue;
        };
        if contract.artifact_case().entrypoint != "." {
            continue;
        }
        for name in contract.artifact_case().exports.keys() {
            let Ok(accepted) = contracts.resolve_name(file.path.as_str(), "solid-js", name) else {
                continue;
            };
            if let Some(value) = project_accepted_export(&accepted)
                .returns
                .known()
                .and_then(Clone::clone)
            {
                returned.entry(symbol_id(name)).or_insert(value);
            }
        }
    }
    returned
}

fn project_accepted_contracts(
    facts: &ProjectFacts,
    contracts: &AcceptedContractIndex,
) -> HashMap<(String, String), PackageContract> {
    let mut projected = HashMap::new();
    for file in &facts.files {
        let modules = file
            .ast
            .imports
            .iter()
            .filter(|import| !import.type_only)
            .map(|import| import.module.as_str())
            .chain(
                file.ast
                    .exports
                    .iter()
                    .filter(|export| !export.type_only)
                    .filter_map(|export| export.module.as_deref()),
            )
            .collect::<BTreeSet<_>>();
        for module in modules {
            let Ok(contract) = contracts.contract(file.path.as_str(), module) else {
                continue;
            };
            let artifact_case = contract.artifact_case();
            let exports = artifact_case
                .exports
                .keys()
                .filter_map(|name| {
                    contracts
                        .resolve_name(file.path.as_str(), module, name)
                        .ok()
                        .map(|accepted| (name.clone(), project_accepted_export(&accepted)))
                })
                .collect();
            projected.insert(
                (file.path.to_string(), module.to_owned()),
                PackageContract {
                    package: crate::ContractPackage {
                        name: contract.package().name.clone(),
                        version: contract.package().version.clone(),
                        integrity: contract.package().integrity.clone(),
                    },
                    entrypoints: BTreeMap::from([(
                        artifact_case.entrypoint.clone(),
                        crate::ContractEntrypoint { exports },
                    )]),
                    source_path: format!(
                        "accepted:{}",
                        contract.receipt().semantic_digest.as_str()
                    ),
                },
            );
        }
    }
    projected
}

fn resolve_contract_imports_inner(
    facts: &ProjectFacts,
    exact: &HashMap<(String, String), PackageContract>,
    accepted: &AcceptedContractIndex,
    entities: &EntitySymbols,
    dialect: &dyn Dialect,
) -> ResolvedContracts {
    let mut bindings = Vec::new();
    let mut by_symbol = HashMap::new();
    let mut missing_exports = Vec::new();
    let mut counts = crate::ContractBindingCounts::default();
    for file in &facts.files {
        for import in &file.ast.imports {
            if import.type_only {
                continue;
            }
            let Some(contract) = exact.get(&(file.path.to_string(), import.module.to_string()))
            else {
                if let Some(reason) =
                    accepted.uncertifiable_reason(file.path.as_str(), &import.module)
                {
                    push_missing_accepted_import(
                        &mut missing_exports,
                        file,
                        import,
                        entities,
                        dialect,
                        reason,
                    );
                }
                continue;
            };
            counts.bound += 1;
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
                            if missing_accepted_export_needs_obligation(
                                dialect,
                                &import.module,
                                &imported,
                            ) {
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
                            }
                            continue;
                        };
                        let member_location = location(file.path.shared(), member.property);
                        let Some(symbol) = entities.get(&member_location).cloned() else {
                            continue;
                        };
                        let native =
                            native_vocabulary_outranks_contract(dialect, &import.module, &imported);
                        let mut summary = summary;
                        if native {
                            let Some(overlay) = native_callback_overlay(&summary) else {
                                continue;
                            };
                            summary = overlay;
                        }
                        if !summary.open_claims.is_empty() {
                            push_unknown_contract_claims(
                                &mut missing_exports,
                                &summary,
                                &import.module,
                                &imported,
                                false,
                                member_location.clone(),
                            );
                        }
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
                    if missing_accepted_export_needs_obligation(dialect, &import.module, imported) {
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
                    }
                    continue;
                };
                let native = native_vocabulary_outranks_contract(dialect, &import.module, imported);
                let mut summary = summary;
                if native {
                    let Some(overlay) = native_callback_overlay(&summary) else {
                        continue;
                    };
                    summary = overlay;
                }
                if !summary.open_claims.is_empty() {
                    push_unknown_contract_claims(
                        &mut missing_exports,
                        &summary,
                        &import.module,
                        imported,
                        false,
                        binding_location.clone(),
                    );
                }
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
            let Some(contract) = exact.get(&(file.path.to_string(), module.to_owned())) else {
                continue;
            };
            counts.bound += 1;
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
                    if missing_accepted_export_needs_obligation(dialect, module, imported) {
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
                    }
                    continue;
                };
                let native = native_vocabulary_outranks_contract(dialect, module, imported);
                let mut summary = summary;
                if native {
                    let Some(overlay) = native_callback_overlay(&summary) else {
                        continue;
                    };
                    summary = overlay;
                }
                if !summary.open_claims.is_empty() {
                    push_unknown_contract_claims(
                        &mut missing_exports,
                        &summary,
                        module,
                        imported,
                        true,
                        specifier_location.clone(),
                    );
                }
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
        counts,
    }
}

fn push_missing_accepted_import(
    missing: &mut Vec<StaticDefect>,
    file: &solid_facts::FileFacts,
    import: &solid_facts::ast::ImportFact,
    entities: &EntitySymbols,
    dialect: &dyn Dialect,
    reason: UncertifiableImportReason,
) {
    for binding in &import.bindings {
        if binding.type_only || !binding.runtime_referenced {
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
                let export = file.source_text(member.property).unwrap_or_default();
                if !native_vocabulary_outranks_contract(dialect, &import.module, export) {
                    push_missing_accepted_export(
                        missing,
                        &import.module,
                        export,
                        location(file.path.shared(), member.property),
                        reason,
                    );
                }
            }
            continue;
        }
        let Some(export) = binding.imported.as_deref().or_else(|| {
            (binding.kind == solid_facts::ast::ImportKind::Default).then_some("default")
        }) else {
            continue;
        };
        if !native_vocabulary_outranks_contract(dialect, &import.module, export) {
            push_missing_accepted_export(
                missing,
                &import.module,
                export,
                location(file.path.shared(), import.span),
                reason,
            );
        }
    }
}

fn push_missing_accepted_export(
    missing: &mut Vec<StaticDefect>,
    module: &str,
    export: &str,
    location: Location,
    reason: UncertifiableImportReason,
) {
    missing.push(StaticDefect {
        kind: StaticDefectKind::PackageContractExportMissing {
            module: module.into(),
            export: export.into(),
            reexported: false,
        },
        location,
        analysis_context: match reason {
            UncertifiableImportReason::Unspecified => {
                "no receipt-accepted contract matches this exact import"
            }
            UncertifiableImportReason::ObsoletePolicy1 => {
                "obsolete-policy1-receipt: policy 1 cannot authorize analyzer semantics"
            }
        }
        .into(),
        fixes: vec![],
        uncertain: false,
    });
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
    /// its callback domain open — see
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
                parameter: None,
                member: None,
            };
            seen_reactive_reads
                .insert((reactive_read.kind.clone(), reactive_read.label.clone()))
                .then_some(reactive_read)
        })
        .collect::<Vec<_>>();
    let mut members_by_parameter = BTreeMap::<usize, HashSet<&str>>::new();
    for (parameter, member) in invoked_parameter_members {
        members_by_parameter
            .entry(*parameter)
            .or_default()
            .insert(member.as_str());
    }
    for (parameter, members) in members_by_parameter {
        if seen_reactive_reads.insert(("parameter-member".into(), parameter.to_string())) {
            reactive_reads.push(ContractReactiveRead {
                kind: "parameter-member".into(),
                label: String::new(),
                parameter: Some(parameter),
                member: (members.len() == 1)
                    .then(|| members.into_iter().next().unwrap().to_owned()),
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
        })
    });
    let mut callback_summary = callbacks.to_vec();
    callback_summary.sort_by_key(|callback| callback.parameter);
    // An omitted `callbacks` list is a negative claim. Where a caller-supplied
    // parameter escaped without being accounted for, the honest list is not the
    // rows that were proven -- it is "unknown".
    let callbacks = if escaped_parameters.is_empty()
        && !callbacks_contradict_on_a_parameter(&callback_summary)
    {
        callback_summary.into()
    } else {
        ContractClaim::Open
    };
    ContractExport {
        kind: "function".into(),
        reactive_reads: reactive_reads.into(),
        callbacks,
        owner_requirements: Vec::new().into(),
        returns: returns.into(),
        async_behavior: if node.r#async {
            String::from("promise").into()
        } else {
            String::new().into()
        },
        open_claims: BTreeSet::new(),
    }
}

/// Whether two rows claim different executions for the same parameter.
///
/// One row is pushed per *invocation site*, and `push_contract_callback` dedups
/// only exactly equal rows, so a parameter invoked twice with two schedules
/// publishes both -- `@solid-primitives/range`'s `mapRange` carried
/// `callbacks[2]` as `deferred` and as `tracked` in the same summary. Schema v1
/// has one execution axis per parameter, and the runtime has one behavior, so
/// at least one of the two rows is false and a consumer choosing either is
/// guessing. The generator-local knowledge state stays open for that domain.
///
/// Rows that agree on `execution` and differ elsewhere (argument descriptors,
/// owner) are *not* contradictory: those are additional facts about the same
/// schedule, and collapsing them would discard proven claims.
///
/// **The sentinel is per export, and that is wider than the contradiction.** One
/// contradicted parameter discards the *other* parameters' undisputed rows too
/// (`fixtures/package-contracts/multi-role-callback-parameter`'s
/// `contradictOnZeroOnly` pins it). Schema v1 offers no narrower spelling: the
/// only granularity below `{"status": "unknown"}` is whether a row is present,
/// and an absent row is a certified *negative* — "never invokes a
/// caller-supplied callback there" (docs/package-contracts.md, the
/// "no callback execution row" review section). Dropping only the contradicted
/// parameter's rows would therefore replace one contradiction with one
/// affirmative false negative, and there is no encoding for "unknown at this
/// parameter, proven at that one". The pre-existing `escaped_parameters`
/// sentinel two lines up has exactly the same width for the same reason.
///
/// `callbacks` must already be sorted by parameter.
fn callbacks_contradict_on_a_parameter(callbacks: &[ContractCallback]) -> bool {
    callbacks.windows(2).any(|pair| {
        pair[0].parameter == pair[1].parameter && pair[0].execution != pair[1].execution
    })
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
    // `module_level_exports`, not `exports`: an `export` nested in a
    // `namespace` body publishes a member of the namespace object, which no
    // importer of this module can name. See `AstFacts::module_level_exports`.
    for export in file
        .ast
        .module_level_exports()
        .filter(|export| !export.type_only)
    {
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
    // `module_level_exports`, not `exports`: an `export` inside a `namespace`
    // body publishes a member of the namespace object, which no importer of
    // this module can name. See `AstFacts::module_level_exports`.
    for export in file
        .ast
        .module_level_exports()
        .filter(|export| !export.type_only)
    {
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
        ..ContractExport::default()
    }
}

fn entity_at<'a>(facts: &'a ProjectFacts, target: &Location) -> Option<&'a typefacts::EntityFact> {
    facts.typescript.entities().find(|entity| {
        entity.location.path == target.path
            && entity.location.start_byte == target.start_byte
            && entity.location.end_byte == target.end_byte
    })
}

/// The honest summary for an export *raised* to `kind: "function"`:
/// callbacks fail closed.
///
/// An omitted `callbacks` list is a *negative* claim — "invokes no
/// caller-supplied function" — and a consumer reads `new Store(onChange)`
/// through exactly the same contract path as `store(onChange)`, so publishing
/// that silence certifies an inertness the export can contradict. Every raise
/// reaches here with a summary whose `kind` was still `value`, which is
/// precisely the state in which no function body was summarized for it:
///
/// - a **class** never has one, because the generator summarizes function
///   declarations, not construct signatures, so nothing carries what a
///   constructor — the class's own, or the one it inherits through `extends` —
///   does with its arguments;
/// - a **callable** binding reaching a raise had no summary node either. Had
///   its body been analyzed, the summary would already say `kind: "function"`
///   and carry that analysis's claims, and no raise would happen. Leaving the
///   domains absent here certified "invokes no caller-supplied callback" for a
///   body this run never read.
///
/// The open callback domain is demand-sensitive at the consumer: constructing or calling
/// with no callable argument stays clean.
pub fn raised_function_export(mut summary: ContractExport) -> ContractExport {
    summary.kind = "function".into();
    summary.callbacks = ContractClaim::Open;
    summary
}

/// What this analysis can prove about an exported binding's runtime `kind`.
///
/// `kind` is a structural inference premise, and `validate_export` bars a
/// `kind: "value"` summary from carrying any function claim. A value summary
/// therefore demands a proof that the export is not a function, not merely
/// the absence of a proof that it is.
///
/// Two facts decide it, and only together. Neither
/// [`typefacts::Callability`] nor [`typefacts::Constructability`] answers "is
/// this a runtime function" alone: the type system reads a construct signature
/// as *not* a call signature, so every class answers `NonCallable`, while
/// `Constructable` says nothing about a plain function. See the producer's ADR
/// 0020.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExportKindProof {
    /// The type has a call signature, belongs to the signature-less
    /// `Function` family, has a construct signature, or combines them.
    /// `typeof === "function"` at runtime either way, and a raise leaves
    /// `callbacks` unknown because no body was summarized — see
    /// [`raised_function_export`].
    ///
    /// A class lands here through `Constructable`, which is what retired the
    /// syntactic class search this decision used to run first. That search was
    /// defeated by exactly the shapes a published package contains:
    /// `@solidjs/web@2.0.0-rc.1`'s `ResponseEnvelope`
    /// (`const C = (() => { class C {…}; …; return C; })()`, whose initializer
    /// is a *call*) and `@tanstack/*-devtools`' `*DevtoolsCore`
    /// (`const C = pair[0]`, whose element type is a class declared in another
    /// package) have no class expression in the analyzed artifact to find. The
    /// type has the answer in both.
    Callable,
    /// The type has neither a call nor a construct signature: `NonCallable`
    /// **and** `NonConstructable`.
    ///
    /// This is the full negative. Signature-less `Function` supertypes no
    /// longer reach it: producer ADR 0021 gives them the distinct positive
    /// `UntypedCallable` answer. Broad non-callable supertypes such as
    /// `object`, `{}`, and `Record<string, unknown>` remain here because those
    /// declared types genuinely admit non-function values.
    NonCallable,
    /// One or both facts are present and closed nothing. `Unknown` is `any`,
    /// `unknown`, `never` or an error type; `Mixed` is a union holding both a
    /// signature-carrying and a signature-less constituent. The two aggregate
    /// *independently*, so `Mixed` on both is not a per-constituent proof
    /// either: `(() => void) | number | (new () => X)` answers `Mixed` twice
    /// and still holds a constituent that is neither. `typeof` is therefore
    /// not statically determined and neither `kind` is a claim this analysis
    /// can make.
    Unresolvable(Callability, Constructability),
    /// One or both facts are absent at this location.
    ///
    /// Not "undemanded": `demand_plan` requests both facts at *every* export
    /// specifier and every exported declaration name — the only spans this
    /// decision is ever asked about, pinned by an assertion in
    /// `solid_facts_backend::semantic_demands`' tests — so absence here is the
    /// producer finding no node to classify at a span this analysis did ask
    /// about. That is missing evidence, and missing evidence may not publish
    /// the maximal certified negative a `value` summary is.
    Unanswered,
}

/// The `kind` proof for the binding at `target`.
///
/// The whole rule, in order:
///
/// 1. `(Callable ∨ UntypedCallable) ∨ Constructable ⇒`
///    [`ExportKindProof::Callable`]. Either callability positive or a construct
///    signature is a function at runtime, and neither fact's own absence or
///    uncertainty can subtract from the other's positive. `UntypedCallable`
///    deliberately proves no readable signature or parameter facts.
/// 2. `NonCallable ∧ NonConstructable ⇒` [`ExportKindProof::NonCallable`].
///    Both closed negatives, and only both.
/// 3. Either fact absent `⇒` [`ExportKindProof::Unanswered`].
/// 4. Anything else — `Mixed` or `Unknown` on either side `⇒`
///    [`ExportKindProof::Unresolvable`].
///
/// The multi-hop class search that used to run before any type answer — an
/// alias-and-initializer symbol walk, a class-expression initializer fact, and
/// an assignment scan — is gone: `Constructable` subsumes it, and reaches the
/// IIFE-wrapped and cross-package shapes it could not. What remains of syntax
/// is [`class_declaration_name`], and it is not a proof about a value. It
/// picks which question the facts are answering, because for the class
/// declaration shapes — a declaration name, and an anonymous
/// `export default class {}`, whose recorded span is the class node — the
/// demanded span is not the export's *value*.
pub fn export_kind_proof(facts: &ProjectFacts, target: &Location) -> ExportKindProof {
    export_kind_proof_from_entity(facts, target, entity_at(facts, target))
}

/// Decides an export's runtime kind using an already indexed exact entity.
///
/// Contract generation asks this question for every public name in wide
/// barrels. Accepting the entity separately lets that caller retain one exact
/// location index without changing the proof rule or bypassing the class-name
/// addressing correction.
pub fn export_kind_proof_from_entity(
    facts: &ProjectFacts,
    target: &Location,
    entity: Option<&typefacts::EntityFact>,
) -> ExportKindProof {
    if class_declaration_name(facts, target) {
        return ExportKindProof::Callable;
    }
    let callability = entity.and_then(|entity| entity.callability);
    let constructability = entity.and_then(|entity| entity.constructability);
    let signature_proof = match (callability, constructability) {
        (Some(Callability::Callable | Callability::UntypedCallable), _)
        | (_, Some(Constructability::Constructable)) => ExportKindProof::Callable,
        (Some(Callability::NonCallable), Some(Constructability::NonConstructable)) => {
            ExportKindProof::NonCallable
        }
        (Some(callability), Some(constructability)) => {
            ExportKindProof::Unresolvable(callability, constructability)
        }
        _ => ExportKindProof::Unanswered,
    };
    // A closed census of the exact runtime binding corrects a declaration-
    // surface negative. Published packages commonly ship `index.js` beside
    // `index.d.ts`; the configured compiler may attach the declaration
    // module's non-callable answer to the runtime declarator even though the
    // exact runtime symbol is initialized and remains bound to a function.
    // Only the closed callable census corrects that contradiction. Open/mixed
    // censuses and a non-callable census beside a positive signature retain
    // the established fail-closed signature rule.
    if entity.and_then(|entity| entity.runtime_binding_kind) == Some(RuntimeBindingKind::Callable) {
        return ExportKindProof::Callable;
    }
    match signature_proof {
        ExportKindProof::Unresolvable(_, _) | ExportKindProof::Unanswered => {
            match entity.and_then(|entity| entity.runtime_binding_kind) {
                Some(RuntimeBindingKind::Callable) => ExportKindProof::Callable,
                Some(RuntimeBindingKind::NonCallable) => ExportKindProof::NonCallable,
                // A mixed or open write census is explicit evidence that no
                // closed runtime kind exists. Preserve the signature failure
                // so the caller refuses with its established diagnostic.
                Some(RuntimeBindingKind::Mixed | RuntimeBindingKind::Open) | None => {
                    signature_proof
                }
            }
        }
        _ => signature_proof,
    }
}

/// Whether `target` is exactly a class's binding name, or exactly an anonymous
/// class node, in the file that declares it.
///
/// **A span-addressing fact, not a class-ness proof.** Most export shapes carry
/// a span whose type *is* the exported value: an export specifier, a variable
/// declarator's name, a function declaration's name. A class declaration is the
/// exception, in two spellings, and neither has a specifier span to demand at
/// instead:
///
/// - `export class C {}` — the demanded span is the declaration's *name*, where
///   the compiler's type is the class's **instance** type. It honestly answers
///   `NonCallable` and `NonConstructable`, because an instance is neither.
/// - `export default class {}` — anonymous, so there is no name to record and
///   the export carries the `class …` node's own span. The facts there describe
///   the instance type for the same reason.
///
/// The producer pins that by test and its ADR 0020 says so outright: demand at
/// the export-specifier span, never at a declaration name. These are the shapes
/// where the two facts answer about a different value than the export's and must
/// not be read at all.
///
/// What is left over after that is not a heuristic. `class C {}` binds the
/// constructor by language definition — named or anonymous, `typeof` of a class
/// declaration is `"function"` and needs no type answer. Nor can a bundler
/// defeat it the way it defeated the retired search: a lowered class has no
/// class *declaration* left, so nothing reaches here and the facts — asked at a
/// declarator name, which types as the constructor — decide it correctly.
fn class_declaration_name(facts: &ProjectFacts, target: &Location) -> bool {
    let span = solid_facts::core::Span::new(
        u32::try_from(target.start_byte).unwrap_or(u32::MAX),
        u32::try_from(target.end_byte).unwrap_or(u32::MAX),
    );
    facts
        .files
        .iter()
        .any(|file| file.path.as_str() == target.path.as_ref() && file.ast.declares_class_at(span))
}

/// Raises a `value` summary to what the binding at `span` is proven to be.
///
/// This is the project-wide analysis map (`Program::contract_exports`), not a
/// published entrypoint: it cannot refuse, so an unprovable kind stays the
/// `value` default here and the emission path
/// (`promote_entry_callable` in the backend) is what refuses to publish it.
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
    match export_kind_proof(facts, &target) {
        // Both raises carry `callbacks` unknown, and for the same reason: a
        // summary still saying `value` here is one no function body was
        // analyzed for. See `raised_function_export`.
        ExportKindProof::Callable => raised_function_export(summary),
        ExportKindProof::NonCallable
        | ExportKindProof::Unresolvable(_, _)
        | ExportKindProof::Unanswered => summary,
    }
}

#[cfg(test)]
mod native_overlay_tests {
    use super::{
        ContractExport, missing_accepted_export_needs_obligation, native_callback_overlay,
    };
    use crate::{ContractCallback, ContractClaim, ContractReturn};
    use solid_dialect::Solid2;

    #[test]
    fn native_overlay_keeps_only_exact_callback_timing() {
        let summary = ContractExport {
            kind: "function".into(),
            callbacks: ContractClaim::Known(vec![ContractCallback {
                parameter: 1,
                execution: "inline".into(),
                arguments: Vec::new(),
                owner: None,
            }]),
            returns: ContractClaim::Known(Some(ContractReturn {
                kind: "accessor".into(),
                label: "package return".into(),
                ..ContractReturn::default()
            })),
            async_behavior: ContractClaim::Open,
            ..ContractExport::default()
        };
        let overlay = native_callback_overlay(&summary).expect("known callback row");
        assert_eq!(overlay.callbacks, summary.callbacks);
        assert_eq!(overlay.returns, ContractClaim::Known(None));
        assert_eq!(overlay.async_behavior, ContractClaim::Known(String::new()));
    }

    #[test]
    fn partial_accepted_contract_does_not_reopen_dialect_owned_primitives() {
        assert!(!missing_accepted_export_needs_obligation(
            &Solid2,
            "solid-js",
            "createSignal"
        ));
        assert!(missing_accepted_export_needs_obligation(
            &Solid2,
            "@solidjs/signals",
            "isEqual"
        ));
        assert!(missing_accepted_export_needs_obligation(
            &Solid2,
            "solid-js",
            "notAPrimitive"
        ));
        assert!(missing_accepted_export_needs_obligation(
            &Solid2,
            "some-other-package",
            "createSignal"
        ));
    }
}

#[cfg(test)]
mod callback_contradiction_tests {
    use super::{ContractCallback, callbacks_contradict_on_a_parameter};

    fn row(parameter: usize, execution: &str) -> ContractCallback {
        ContractCallback {
            parameter,
            execution: execution.into(),
            arguments: Vec::new(),
            owner: None,
        }
    }

    #[test]
    fn two_executions_for_one_parameter_are_contradictory() {
        // `@solid-primitives/range`'s `mapRange`: parameter 2 invoked in the
        // export body and again inside the accessor it returns.
        let rows = [row(2, "deferred"), row(2, "tracked")];
        assert!(callbacks_contradict_on_a_parameter(&rows));
        // `createDerivedSpring`: an inline site and a tracked site.
        let rows = [row(0, "inline"), row(0, "tracked")];
        assert!(callbacks_contradict_on_a_parameter(&rows));
        // Three rows, with the disagreeing pair not adjacent by execution: all
        // rows for one parameter are contiguous after the sort, so any pair of
        // distinct executions produces at least one differing neighbour.
        let rows = [row(0, "inline"), row(0, "inline"), row(0, "tracked")];
        assert!(callbacks_contradict_on_a_parameter(&rows));
    }

    #[test]
    fn agreeing_and_distinct_parameters_stay_known() {
        // Different parameters may of course differ.
        let rows = [row(0, "inline"), row(1, "tracked")];
        assert!(!callbacks_contradict_on_a_parameter(&rows));
        // Two invocation sites with the same schedule: `push_contract_callback`
        // dedups identical rows, and rows that agree on `execution` while
        // differing elsewhere are additional facts about one schedule, not a
        // contradiction.
        let mut accessor = row(0, "tracked");
        accessor.arguments = vec![None];
        let rows = [row(0, "tracked"), accessor];
        assert!(!callbacks_contradict_on_a_parameter(&rows));
        assert!(!callbacks_contradict_on_a_parameter(&[]));
        assert!(!callbacks_contradict_on_a_parameter(&[row(0, "inline")]));
    }
}

/// The `kind` decision table, every combination, against synthetic facts.
///
/// The two process tests in
/// rust/crates/solid-facts-backend/tests/contracts_process.rs pin what the
/// *generator* publishes end to end; these pin the decision itself over the
/// complete 6x5 product of what the two facts can say — each callability
/// answer, each constructability answer, and absence on either side —
/// including the combinations no fixture reaches.
#[cfg(test)]
mod export_kind_proof_tests {
    use super::{ExportKindProof, ProjectFacts, export_kind_proof, raised_function_export};
    use crate::{ContractClaim, ContractExport};
    use solid_facts::FileFacts;
    use solid_facts::TypeScriptTable;
    use solid_facts::ast;
    use solid_facts::compiler::{COMPILER_FACTS_PROTOCOL, ExecutionMap};
    use solid_facts::core::{Generation, Span};
    use typefacts::{
        Callability, Constructability, EntityFact, Location, PrimitiveValueDomain,
        RuntimeBindingKind,
    };

    const PATH: &str = "artifact.ts";

    fn entity(
        span: Span,
        callability: Option<Callability>,
        constructability: Option<Constructability>,
    ) -> EntityFact {
        EntityFact {
            location: Location {
                path: PATH.into(),
                start_byte: u64::from(span.start),
                end_byte: u64::from(span.end),
            },
            symbol: "".into(),
            symbol_unresolved: false,
            type_descriptor: None,
            resolved_call: None,
            callability,
            constructability,
            runtime_binding_kind: None,
            runtime_value_domain: None,
            primitive_value_domain: PrimitiveValueDomain::default(),
            primitive_literal_candidates: None,
            call_result_domain: None,
            constant_value: None,
            array_shape: None,
            tuple_shape: None,
            library_types: None,
            reference_space: None,
            runtime_identity: "".into(),
        }
    }

    /// The proof for the binding named `name` in `source`, with the two facts
    /// standing in for what Type Facts answered at that exact span.
    ///
    /// The span is the *declarator's* name, which is what an export
    /// declaration's specifier carries. No symbol is recorded and no syntax is
    /// consulted any more: the answer is the fact pair and nothing else, which
    /// is the property these tests exist to hold.
    fn proof(
        source: &str,
        name: &str,
        callability: Option<Callability>,
        constructability: Option<Constructability>,
    ) -> ExportKindProof {
        proof_with_binding(source, name, callability, constructability, None)
    }

    fn proof_with_binding(
        source: &str,
        name: &str,
        callability: Option<Callability>,
        constructability: Option<Constructability>,
        runtime_binding_kind: Option<RuntimeBindingKind>,
    ) -> ExportKindProof {
        let ast = ast::extract(PATH, source).unwrap();
        let start = u32::try_from(source.find(name).expect("name occurs in source")).unwrap();
        let span = Span::new(start, start + u32::try_from(name.len()).unwrap());
        let compiler = ExecutionMap {
            compiler_facts_protocol: COMPILER_FACTS_PROTOCOL,
            source_hash: ast.source.hash.clone(),
            semantic_model: Default::default(),
            tracked_regions: Vec::new(),
            untracked_regions: Vec::new(),
            discarded_regions: Vec::new(),
            ownership_regions: Vec::new(),
            callback_roles: Vec::new(),
            jsx_operations: Vec::new(),
        };
        let generation = Generation::new(1).unwrap();
        let file = FileFacts::new(generation, source, ast, compiler).unwrap();
        let location = Location {
            path: file.path.as_str().into(),
            start_byte: u64::from(span.start),
            end_byte: u64::from(span.end),
        };
        let facts = ProjectFacts {
            generation,
            project_id: "fixture".into(),
            files: vec![file],
            typescript: TypeScriptTable::from_parts(
                3,
                1,
                "fixture",
                Vec::new(),
                vec![EntityFact {
                    runtime_binding_kind,
                    ..entity(span, callability, constructability)
                }],
                Vec::new(),
                Vec::new(),
            ),
            typescript_changes: None,
            resolved_imports: None,
            runtime_symbol_redirects: Default::default(),
        };
        export_kind_proof(&facts, &location)
    }

    #[test]
    fn exact_runtime_binding_census_closes_only_signature_failures() {
        let source = "export const value = opaque();";
        assert_eq!(
            proof_with_binding(
                source,
                "value",
                Some(Callability::Unknown),
                Some(Constructability::Unknown),
                Some(RuntimeBindingKind::Callable),
            ),
            ExportKindProof::Callable
        );
        assert_eq!(
            proof_with_binding(
                source,
                "value",
                Some(Callability::Unknown),
                Some(Constructability::Unknown),
                Some(RuntimeBindingKind::NonCallable),
            ),
            ExportKindProof::NonCallable
        );
        for open in [RuntimeBindingKind::Mixed, RuntimeBindingKind::Open] {
            assert_eq!(
                proof_with_binding(
                    source,
                    "value",
                    Some(Callability::Unknown),
                    Some(Constructability::Unknown),
                    Some(open),
                ),
                ExportKindProof::Unresolvable(Callability::Unknown, Constructability::Unknown)
            );
        }
    }

    #[test]
    fn exact_runtime_binding_census_corrects_sibling_declaration_signature_conflicts() {
        let source = "export const published = () => {};";
        assert_eq!(
            proof_with_binding(
                source,
                "published",
                Some(Callability::NonCallable),
                Some(Constructability::NonConstructable),
                Some(RuntimeBindingKind::Callable),
            ),
            ExportKindProof::Callable
        );
    }

    const CALLABILITIES: [Option<Callability>; 6] = [
        Some(Callability::Callable),
        Some(Callability::UntypedCallable),
        Some(Callability::NonCallable),
        Some(Callability::Mixed),
        Some(Callability::Unknown),
        None,
    ];

    const CONSTRUCTABILITIES: [Option<Constructability>; 5] = [
        Some(Constructability::Constructable),
        Some(Constructability::NonConstructable),
        Some(Constructability::Mixed),
        Some(Constructability::Unknown),
        None,
    ];

    /// The whole rule, restated independently of the implementation: either
    /// positive wins, both closed negatives prove a value, any absence is
    /// missing evidence, and everything left is unresolvable.
    fn expected(
        callability: Option<Callability>,
        constructability: Option<Constructability>,
    ) -> ExportKindProof {
        if matches!(
            callability,
            Some(Callability::Callable | Callability::UntypedCallable)
        ) || constructability == Some(Constructability::Constructable)
        {
            return ExportKindProof::Callable;
        }
        match (callability, constructability) {
            (Some(Callability::NonCallable), Some(Constructability::NonConstructable)) => {
                ExportKindProof::NonCallable
            }
            (Some(callability), Some(constructability)) => {
                ExportKindProof::Unresolvable(callability, constructability)
            }
            _ => ExportKindProof::Unanswered,
        }
    }

    /// A class declaration's span is not the export's *value* — the compiler
    /// answers with the instance type there, which is honestly neither callable
    /// nor constructable. `export class C {}` has no specifier span to ask at
    /// instead, so this shape is decided by the declaration and the facts are
    /// not read. Two spans carry it: a declaration's name, and — for an
    /// anonymous `export default class {}`, which has no name — the class node
    /// itself.
    #[test]
    fn a_class_declaration_is_decided_before_the_facts_are_read() {
        for (source, name) in [
            ("export class Widget {}", "Widget"),
            ("export default class Widget {}", "Widget"),
            ("class Widget {} export { Widget as Widget };", "Widget"),
            // Anonymous: the export records the class node's span, so that is
            // the span the decision is asked about.
            ("export default class {}", "class {}"),
            (
                "export default class extends Base {}",
                "class extends Base {}",
            ),
        ] {
            for callability in CALLABILITIES {
                for constructability in CONSTRUCTABILITIES {
                    assert_eq!(
                        proof(source, name, callability, constructability),
                        ExportKindProof::Callable,
                        "{source}: {callability:?}, {constructability:?}"
                    );
                }
            }
        }
        // And nothing else reaches the gate: a declarator initialized with a
        // class expression, an IIFE, or a tuple element is decided by the
        // facts, because the span there types as the constructor.
        for source in [
            "const Widget = class {}; export { Widget };",
            "const Widget = (() => { class Inner {} return Inner; })(); export { Widget };",
        ] {
            assert_eq!(
                proof(
                    source,
                    "Widget",
                    Some(Callability::NonCallable),
                    Some(Constructability::NonConstructable),
                ),
                ExportKindProof::NonCallable,
                "{source}"
            );
        }
    }

    #[test]
    fn every_fact_combination_decides_the_documented_way() {
        // A shape whose own syntax says nothing: the decision must come from
        // the fact pair alone.
        let source = "export const value = host.create();";
        for callability in CALLABILITIES {
            for constructability in CONSTRUCTABILITIES {
                assert_eq!(
                    proof(source, "value", callability, constructability),
                    expected(callability, constructability),
                    "callability {callability:?}, constructability {constructability:?}"
                );
            }
        }
        // Fifteen of the thirty decide anything, and the shape of that split
        // is the claim: fourteen functions, one value, eight unresolvable, seven
        // unanswered — counted so a rule change cannot widen the certified
        // side unnoticed.
        let mut function = 0;
        let mut value = 0;
        let mut unresolvable = 0;
        let mut unanswered = 0;
        for callability in CALLABILITIES {
            for constructability in CONSTRUCTABILITIES {
                match expected(callability, constructability) {
                    ExportKindProof::Callable => function += 1,
                    ExportKindProof::NonCallable => value += 1,
                    ExportKindProof::Unresolvable(_, _) => unresolvable += 1,
                    ExportKindProof::Unanswered => unanswered += 1,
                }
            }
        }
        assert_eq!((function, value, unresolvable, unanswered), (14, 1, 8, 7));
    }

    #[test]
    fn an_untyped_callable_proves_kind_without_a_signature_claim() {
        let source = "export const value = host.create();";
        assert_eq!(
            proof(
                source,
                "value",
                Some(Callability::UntypedCallable),
                Some(Constructability::NonConstructable),
            ),
            ExportKindProof::Callable
        );
    }

    #[test]
    fn a_class_is_proven_by_constructability_alone() {
        // Every class type truthfully answers `nonCallable`; the construct
        // signature is the whole proof. These are the shapes the retired
        // syntactic search used to have to recognize — and the last two are
        // the shapes it could not: an IIFE-wrapped class and a class reached
        // as a tuple element type have no class expression to find. None of
        // them is a class *declaration name*, so none reaches the one
        // remaining syntactic gate.
        for source in [
            "const Widget = class Named {};",
            "var Widget = class {};",
            "const Widget = (() => { class Inner {} return Inner; })(); export { Widget };",
            "const pair = build(); const Widget = pair[0]; export { Widget };",
        ] {
            assert_eq!(
                proof(
                    source,
                    "Widget",
                    Some(Callability::NonCallable),
                    Some(Constructability::Constructable),
                ),
                ExportKindProof::Callable,
                "{source}"
            );
        }
    }

    #[test]
    fn a_class_expression_declarator_is_decided_by_the_facts_alone() {
        // The retired search turned on a class-expression initializer fact
        // (`BindingFact::initializer_class`, deleted with it) and an assignment
        // scan, so a reassigned class-expression binding used to answer
        // differently from a `const` one. The fact pair is the only input now:
        // whatever the type says at that span is the answer, for either
        // syntax.
        for source in [
            "var Widget = class {}; Widget = { notAFunction: true };",
            "var Widget = class {}; Widget.marker = true;",
            "const Widget = class {};",
        ] {
            assert_eq!(
                proof(
                    source,
                    "Widget",
                    Some(Callability::NonCallable),
                    Some(Constructability::NonConstructable),
                ),
                ExportKindProof::NonCallable,
                "{source}"
            );
            assert_eq!(
                proof(
                    source,
                    "Widget",
                    Some(Callability::NonCallable),
                    Some(Constructability::Constructable),
                ),
                ExportKindProof::Callable,
                "{source}"
            );
        }
        // `const Widget = class {} as unknown` used to be pinned as `function`
        // by the retired `initializer_class` fact. It is a refusal now, and
        // that is the honest answer: an `unknown` assertion erases the class,
        // both facts report `Unknown` at the declarator, and nothing left in
        // the artifact proves the binding holds a constructor at runtime.
        assert_eq!(
            proof(
                "const Widget = class {} as unknown; export { Widget };",
                "Widget",
                Some(Callability::Unknown),
                Some(Constructability::Unknown),
            ),
            ExportKindProof::Unresolvable(Callability::Unknown, Constructability::Unknown),
        );
    }

    #[test]
    fn a_destructured_binding_is_decided_like_any_other() {
        // This is what the constructability fact discharged. An object pattern
        // binds a *member* and an array pattern an *element*, which the
        // retired syntactic search could not reason about at all, so
        // `nonCallable` was refused there rather than believed. The type
        // answers the pattern directly: `(class Named {}).name` is a string
        // and provably not a function; a static class member and a tuple
        // element whose type is a class are `Constructable`.
        for source in [
            "const { Inner } = Container;",
            "const [Inner] = pair;",
            "export const { Inner } = Container;",
        ] {
            assert_eq!(
                proof(
                    source,
                    "Inner",
                    Some(Callability::NonCallable),
                    Some(Constructability::NonConstructable),
                ),
                ExportKindProof::NonCallable,
                "{source}"
            );
            assert_eq!(
                proof(
                    source,
                    "Inner",
                    Some(Callability::NonCallable),
                    Some(Constructability::Constructable),
                ),
                ExportKindProof::Callable,
                "{source}"
            );
        }
    }

    #[test]
    fn absence_on_either_fact_is_unanswered_not_a_negative() {
        // `demand_plan` asks for both facts at every export specifier and
        // every exported declaration name, so absence at a span this decision
        // is asked about is the producer finding no node to classify. Half an
        // answer is not an answer: a present `nonCallable` beside an absent
        // constructability must not publish `value`.
        let source = "export const value = host.create();";
        assert_eq!(
            proof(source, "value", Some(Callability::NonCallable), None),
            ExportKindProof::Unanswered
        );
        assert_eq!(
            proof(
                source,
                "value",
                None,
                Some(Constructability::NonConstructable)
            ),
            ExportKindProof::Unanswered
        );
        assert_eq!(
            proof(source, "value", None, None),
            ExportKindProof::Unanswered
        );
        // A positive still decides across an absence: a call signature is a
        // call signature whether or not the other walk ran.
        assert_eq!(
            proof(source, "value", Some(Callability::Callable), None),
            ExportKindProof::Callable
        );
        assert_eq!(
            proof(source, "value", None, Some(Constructability::Constructable)),
            ExportKindProof::Callable
        );
    }

    #[test]
    fn mixed_does_not_compose_across_the_two_facts() {
        // The producer aggregates the two independently, so `Mixed` twice is
        // not a per-constituent proof:
        // `(() => void) | number | (new () => X)` answers exactly this and
        // still holds a constituent that is neither callable nor
        // constructable.
        let source = "export const value = host.create();";
        assert_eq!(
            proof(
                source,
                "value",
                Some(Callability::Mixed),
                Some(Constructability::Mixed)
            ),
            ExportKindProof::Unresolvable(Callability::Mixed, Constructability::Mixed)
        );
        // `any`, `unknown`, `never` or an error type on both sides. No
        // `typeof` follows.
        assert_eq!(
            proof(
                source,
                "value",
                Some(Callability::Unknown),
                Some(Constructability::Unknown)
            ),
            ExportKindProof::Unresolvable(Callability::Unknown, Constructability::Unknown)
        );
    }

    #[test]
    fn every_raise_fails_closed_on_callbacks() {
        // The asymmetry this replaced: the class raise marked callbacks
        // unknown and the callable raise published silence, which is the
        // negative claim "invokes no caller-supplied callback" about a body
        // that was never analyzed.
        let raised = raised_function_export(ContractExport {
            kind: "value".into(),
            ..ContractExport::default()
        });
        assert_eq!(raised.kind, "function");
        assert!(matches!(raised.callbacks, ContractClaim::Open));
    }
}
