//! Conditional export-root proofs from exact imported parameter-return calls.
//! No receipt authority lives here. Finalization requires every recorded
//! dependency obligation to be discharged by authenticated graph composition.

use super::*;

pub(super) const DEPENDENCY_PREFIX: &str = "factory-return-dependency:";

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub(in crate::contract_certification) struct FactoryReturnDependencyClaim {
    pub package: String,
    pub artifact_case: String,
    pub accepted_contract_digest: String,
    pub specifier: String,
    pub importer: String,
    pub resolved_import_root: String,
    pub export: String,
    pub semantic_claim_id: String,
    pub parameter_index: u16,
}

impl FactoryReturnDependencyClaim {
    pub(in crate::contract_certification) fn is_closed_in(
        &self,
        candidate: &solid_reactive_ir::contract_semantics::NormalizedContract,
    ) -> bool {
        let Some(export) = candidate
            .artifact_case(&self.artifact_case)
            .and_then(|case| case.exports.get(&self.export))
        else {
            return false;
        };
        let subject = return_subject(&self.artifact_case, &self.export);
        export
            .operation_claim(ClaimDomain::Returns)
            .is_some_and(|claim| claim.is_closed())
            && returned_parameter(export) == Some(self.parameter_index)
            && candidate
                .claim_id(&subject)
                .is_ok_and(|id| id.as_str() == self.semantic_claim_id)
    }
}

fn return_subject(
    case: &str,
    export: &str,
) -> solid_reactive_ir::contract_semantics::SemanticClaimSubject {
    solid_reactive_ir::contract_semantics::SemanticClaimSubject {
        artifact_case: case.into(),
        export: export.into(),
        path: SemanticClaimPath::Domain(ClaimPath::Call(ClaimDomain::Returns)),
    }
}

fn returned_parameter(
    export: &solid_reactive_ir::contract_semantics::ExportSemantics,
) -> Option<u16> {
    let [id] = export.operation_claim(ClaimDomain::Returns)?.items() else {
        return None;
    };
    let operation = export.operation(&id.0)?;
    if operation.kind != OperationKind::Return {
        return None;
    }
    let ValueShape::Parameter { index, path } = operation.output.as_ref()? else {
        return None;
    };
    path.is_empty().then_some(*index)
}

pub(super) fn require(
    plan: &CertificationPlan,
    proof: &ScheduledProofDemand,
    transcript: &ExportValueTranscript,
    evidence: CensusEvidence<'_>,
    open: &impl Fn(&str) -> TypeFactsCertificationError,
    sites: &mut Vec<String>,
) -> Result<bool, TypeFactsCertificationError> {
    let ProofDemandSubject::PositiveFact(PositiveFactSubject::RecursiveValue {
        root: ValueRoot::Export,
        path,
        callable,
        ..
    }) = &proof.subject
    else {
        return Ok(false);
    };
    if !path.0.is_empty() || *callable == DemandedCallability::Callable {
        return Ok(false);
    }
    let Some(initializer) = &transcript.initializer else {
        return Ok(false);
    };
    let (_, export_name) = proof_artifact_export(&proof.subject);
    let Some((runtime_path, _, span, owner)) = plan.verified_exports.runtime_binding(export_name)
    else {
        return Ok(false);
    };
    let paths = evidence
        .roots
        .iter()
        .map(|root| root.path.clone())
        .collect::<Vec<_>>();
    let runtime_location = initializer.location.path.replace('\\', "/");
    let Some((runtime_index, runtime_relative)) =
        strip_materialized_source_root(&runtime_location, &paths)
    else {
        return Ok(false);
    };
    if evidence.roots[runtime_index].snapshot.root() != owner
        || runtime_relative.trim_start_matches("./") != runtime_path.trim_start_matches("./")
        || initializer.location.start_byte != u64::from(span.start)
        || initializer.location.end_byte != u64::from(span.end)
        || initializer.call.path != initializer.location.path
    {
        return Err(open(
            "factory initializer does not bind the authenticated runtime export",
        ));
    }
    // Recover the original importing module from the exact materialized
    // installation, not merely the snapshot hash (which may repeat).
    let mut importers = std::iter::once(plan)
        .chain(evidence.dependencies.iter().copied())
        .filter(|candidate| candidate.snapshot.root() == owner)
        .filter_map(|candidate| {
            let marker = private_project_package_marker(
                plan,
                &candidate.resolved_import.package_root,
                candidate.snapshot.package_name(),
            );
            runtime_location
                .ends_with(&format!(
                    "{marker}{}",
                    runtime_relative.trim_start_matches("./")
                ))
                .then(|| {
                    Path::new(&candidate.resolved_import.package_root)
                        .join(runtime_relative.trim_start_matches("./"))
                        .to_string_lossy()
                        .replace('\\', "/")
                })
        })
        .collect::<Vec<_>>();
    importers.sort();
    importers.dedup();
    let [importer] = importers.as_slice() else {
        return Err(open(
            "factory initializer has no unique original importing module",
        ));
    };
    let declaration = &initializer.declaration;
    let normalized = declaration.location.path.replace('\\', "/");
    let Some((index, relative)) = strip_materialized_source_root(&normalized, &paths) else {
        return Ok(false);
    };
    let root = &evidence.roots[index];
    if !root.dependency {
        return Ok(false);
    }
    let Some(bytes) = root.snapshot.read(relative) else {
        return Ok(false);
    };
    let Ok(source) = std::str::from_utf8(bytes) else {
        return Ok(false);
    };
    let Ok(facts) = solid_facts::ast::extract(
        relative.to_owned(),
        source.strip_prefix('\u{feff}').unwrap_or(source),
    ) else {
        return Ok(false);
    };
    let mut matches = Vec::new();
    for dependency in
        plan.demand_graph()
            .demands()
            .iter()
            .filter_map(|demand| match demand.subject() {
                ProofDemandSubject::DependencyArtifact { dependency }
                    if dependency.specifier == initializer.specifier.as_ref() =>
                {
                    Some(dependency)
                }
                _ => None,
            })
    {
        for child in evidence.dependencies {
            if child.snapshot.package_name() != dependency.package
                || child.resolved_import.importer.replace('\\', "/") != *importer
                || child.resolved_import.specifier != initializer.specifier.as_ref()
            {
                continue;
            }
            let marker = private_project_package_marker(
                plan,
                &child.resolved_import.package_root,
                child.snapshot.package_name(),
            );
            if !normalized.ends_with(&format!("{marker}{}", relative.trim_start_matches("./"))) {
                continue;
            }
            let Some(case) = child
                .candidates
                .proposal()
                .artifact_case(&dependency.artifact_case)
            else {
                continue;
            };
            let Some(export) = case.exports.get(initializer.export_name.as_ref()) else {
                continue;
            };
            let Some((path, name, reference, owner)) = child
                .verified_exports
                .declaration_reference(&initializer.export_name)
            else {
                continue;
            };
            if owner != root.snapshot.root()
                || path.trim_start_matches("./") != relative.trim_start_matches("./")
                || name != declaration.name.as_ref()
            {
                continue;
            }
            let target = facts.reference_declaration(reference).unwrap_or(reference);
            if u64::from(target.start) != declaration.location.start_byte
                || u64::from(target.end) != declaration.location.end_byte
            {
                continue;
            }
            let Some(parameter_index) = returned_parameter(export) else {
                continue;
            };
            if !initializer
                .object_arguments
                .iter()
                .any(|argument| argument.index == u32::from(parameter_index))
            {
                continue;
            }
            let subject = return_subject(&case.id, &initializer.export_name);
            if !export
                .operation_claim(ClaimDomain::Returns)
                .is_some_and(|claim| claim.is_closed())
                && !child.candidates.closure_candidates().contains(&subject)
            {
                continue;
            }
            let Ok(claim_id) = child.candidates.proposal().claim_id(&subject) else {
                continue;
            };
            matches.push(FactoryReturnDependencyClaim {
                package: dependency.package.clone(),
                artifact_case: dependency.artifact_case.clone(),
                accepted_contract_digest: dependency.accepted_contract_digest.clone(),
                specifier: dependency.specifier.clone(),
                importer: child.resolved_import.importer.clone(),
                resolved_import_root: super::super::policy2_resolved_import_root(
                    &child.resolved_import,
                )
                .map_err(|_| open("factory dependency resolved-import identity is unavailable"))?
                .as_str()
                .to_owned(),
                export: initializer.export_name.to_string(),
                semantic_claim_id: claim_id.as_str().to_owned(),
                parameter_index,
            });
        }
    }
    // Equivalent importer variants may repeat an identical conditional claim;
    // incompatible selections remain ambiguous and grant no premise.
    matches.dedup();
    let [claim] = matches.as_slice() else {
        return Err(open(
            "factory initializer requires one exact dependency export with an exhaustive whole-parameter return and a matching object argument",
        ));
    };
    sites.push(format!(
        "{DEPENDENCY_PREFIX}{}",
        serde_json::to_string(claim).expect("native factory claim")
    ));
    sites.push(format!(
        "factory-initializer:{}",
        serde_json::to_string(initializer).expect("source initializer")
    ));
    Ok(true)
}
