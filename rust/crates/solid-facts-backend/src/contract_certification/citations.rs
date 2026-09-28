//! ADR 0151: a certified claim may cite a dependency's compiled-in acceptance.
//!
//! A plan whose verified closure names a dependency edge needs dependency
//! authority before it can finalize. On the published-graph lanes that
//! authority is a receipt the same transaction issued for the dependency
//! node. Here it is the receipt this build's compiled-in tier carries for the
//! dependency, **cited**: found for exactly the artifact and contract the edge
//! names, admitted in the dependent's own installed tree by the one admission
//! rule, and named in the dependent's receipt so that a later tier which no
//! longer carries it withdraws the dependent too.
//!
//! Nothing the adapter supplied is trusted. The edge is the closure the
//! certifier replayed from the archive bytes; the tier is compiled in; the
//! installed tree is read here, from the dependent's installed location.

use std::path::Path;

use super::CertificationPlan;
use super::environment_edges::{
    LocatedEnvironmentPackage, SourceResolutionEdge, package_name_of_specifier,
};
use crate::artifact_resolution::AcceptedDependencyEdge;
use crate::contract_certification::DependencyEnvironmentEntry;

/// One dependency edge of a plan's closure, discharged by a compiled-in
/// acceptance.
pub(super) struct CitedDependency {
    pub(super) edge: AcceptedDependencyEdge,
    pub(super) cited: crate::accepted_bundles::CompiledInCitation,
    /// The dependency as installed, beside the dependent.
    pub(super) installed: DependencyEnvironmentEntry,
    /// What the dependent's environment gains: the dependency itself and
    /// every entry the cited receipt's environment states, edges removed
    /// (the dependent states its own, from `located`).
    pub(super) environment: Vec<DependencyEnvironmentEntry>,
    /// Where each of those is installed, and the lookups that reached it.
    pub(super) located: Vec<LocatedEnvironmentPackage>,
}

/// Cites a compiled-in acceptance for every dependency edge of `plan`'s
/// verified closure, or says which edge has none and why.
///
/// All or nothing: an edge the closure names is a premise of every claim the
/// plan proposes, so a plan with one edge left undischarged has no dependency
/// authority at all.
pub(super) fn cite_closure_dependencies(
    plan: &CertificationPlan,
) -> Result<Vec<CitedDependency>, String> {
    let dependent_root = plan
        .resolved_import
        .package_real_root
        .clone()
        .unwrap_or_else(|| plan.resolved_import.package_root.clone());
    let mut edges = plan.verified_closure.manifest().dependencies.clone();
    edges.sort_by(|left, right| left.specifier.cmp(&right.specifier));
    edges.dedup();
    let mut cited = Vec::with_capacity(edges.len());
    for edge in edges {
        let query = crate::accepted_bundles::CitationQuery {
            specifier: &edge.specifier,
            export_conditions: &plan.import_request.export_conditions,
            artifact_case: Some(&edge.artifact_case),
            accepted_contract_digest: Some(&edge.accepted_contract_digest),
        };
        let located = crate::diagnostics::compiled_in_citation(Path::new(&dependent_root), &query)
            .map_err(|reason| format!("dependency {}: {reason}", edge.specifier))?;
        if located.cited.citation.package_name != edge.package_name
            || located.installed.name != edge.package_name
        {
            return Err(format!(
                "dependency {}: the closure names package {}, and the cited acceptance is for {}",
                edge.specifier, edge.package_name, located.cited.citation.package_name
            ));
        }
        let lookup_name = package_name_of_specifier(&edge.specifier);
        let mut packages = vec![LocatedEnvironmentPackage {
            entry: located.installed.clone(),
            roots: vec![located.root.to_string_lossy().into_owned()],
            resolved_from: vec![SourceResolutionEdge {
                importer_package_root: dependent_root.clone(),
                specifier: lookup_name,
            }],
        }];
        packages.extend(located.located.iter().map(|(entry, roots, lookups)| {
            LocatedEnvironmentPackage {
                entry: entry.clone(),
                roots: roots
                    .iter()
                    .map(|root| root.to_string_lossy().into_owned())
                    .collect(),
                resolved_from: lookups
                    .iter()
                    .map(|(importer, specifier)| SourceResolutionEdge {
                        importer_package_root: importer.to_string_lossy().into_owned(),
                        specifier: specifier.clone(),
                    })
                    .collect(),
            }
        }));
        let mut environment = vec![located.installed.clone()];
        environment.extend(
            located
                .cited
                .environment
                .iter()
                .map(DependencyEnvironmentEntry::without_edge),
        );
        cited.push(CitedDependency {
            edge,
            installed: located.installed,
            cited: located.cited,
            environment,
            located: packages,
        });
    }
    Ok(cited)
}
