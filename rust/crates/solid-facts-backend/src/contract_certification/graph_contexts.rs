//! Reacquire an open graph request when a shared compiler program contains
//! multiple installations of its package identity. No evidence crosses sessions.

use super::super::dependencies::VerifiedGraphSourcePackage;
use super::*;

pub(super) fn is_family_open(error: &TypeFactsCertificationError) -> bool {
    match error {
        TypeFactsCertificationError::FamilyOpen { .. } => true,
        TypeFactsCertificationError::GraphNodeStage { source, .. }
        | TypeFactsCertificationError::TransactionStage { source, .. } => is_family_open(source),
        _ => false,
    }
}

pub(super) fn has_duplicate_installation(
    plan: &CertificationPlan,
    plans: &[&CertificationPlan],
    sources: &[VerifiedGraphSourcePackage],
) -> bool {
    let identity = (
        plan.snapshot.package_name(),
        plan.snapshot.package_version(),
    );
    let own_root = &plan.resolved_import.package_root;
    plans.iter().any(|other| {
        (
            other.snapshot.package_name(),
            other.snapshot.package_version(),
        ) == identity
            && other.resolved_import.package_root != *own_root
    }) || sources.iter().any(|source| {
        (
            source.snapshot.package_name(),
            source.snapshot.package_version(),
        ) == identity
            && source.installed_package_root != *own_root
    })
}

pub(super) fn acquire_isolated(
    project_root: &CertificationPlan,
    request: &GraphExportValueRequest<'_>,
    materialized_dependencies: &[&CertificationPlan],
    census_dependencies: &[&CertificationPlan],
    sources: &[VerifiedGraphSourcePackage],
    pin: &TypeFactsProducerPin,
) -> Result<VerifiedTypeFactsEvidence, TypeFactsCertificationError> {
    let started = std::time::Instant::now();
    let source_refs = sources.iter().collect::<Vec<_>>();
    let project = PrivateTypeFactsProject::materialize_with_program_roots(
        project_root,
        materialized_dependencies,
        &[request.plan],
        &source_refs,
        true,
    )
    .map_err(|error| error.at_graph_node(request.plan, "isolated graph materialization"))?;
    let schedules = derive_export_value_schedules_with_owners(
        &[request.plan],
        census_dependencies,
        &project,
        true,
    )
    .map_err(|error| error.at_graph_node(request.plan, "isolated graph schedule derivation"))?;
    let project_id = project.project_id().to_str().ok_or_else(|| {
        TypeFactsCertificationError::ProducerProvenance(
            "isolated Type Facts project path is not valid UTF-8".into(),
        )
    })?;
    let mut session = TypeFactsCertificationSession::open(pin, project_id)
        .map_err(|error| error.at_graph_node(request.plan, "isolated graph producer launch"))?;
    let result = acquire_in_session(
        request,
        &schedules[0],
        &mut session,
        &project,
        census_dependencies,
        sources,
    );
    report_certification_timing(
        "isolated-graph-acquisition",
        started,
        serde_json::json!({"package": request.plan.resolved_import.package_name,
            "entrypoint": request.plan.resolved_import.requested_entrypoint,
            "verified": result.is_ok()}),
    );
    result
}

/// Both paths run precisely the same verifier over one session's complete
/// request, including local census transcripts and the original authority sets.
pub(super) fn acquire_in_session(
    request: &GraphExportValueRequest<'_>,
    schedule: &TypeFactsCertificationSchedule,
    session: &mut TypeFactsCertificationSession,
    project: &PrivateTypeFactsProject,
    census_dependencies: &[&CertificationPlan],
    sources: &[VerifiedGraphSourcePackage],
) -> Result<VerifiedTypeFactsEvidence, TypeFactsCertificationError> {
    let live = session
        .acquire_export_values(request.plan, schedule)
        .map_err(|error| {
            error.at_graph_node(request.plan, "live graph export-value acquisition")
        })?;
    let package_marker = snapshot_package_marker(request.plan);
    let roots = snapshot_source_roots(
        request.plan,
        census_dependencies,
        sources,
        Some(project),
        &package_marker,
    )
    .map_err(|error| error.at_graph_node(request.plan, "graph census source-root derivation"))?;
    let locals = acquire_census_local_transcripts(
        session,
        request.plan,
        schedule,
        &live,
        &roots,
        &request.dependencies,
    )
    .map_err(|error| {
        error.at_graph_node(request.plan, "graph census local-declaration acquisition")
    })?;
    verify_live_export_value_answer_with_project_census(
        request.plan,
        schedule,
        &live,
        &request.dependencies,
        census_dependencies,
        sources,
        Some(project),
        &locals,
    )
    .map_err(|error| error.at_graph_node(request.plan, "live graph export-value verification"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graph_context_retry_requires_a_locally_open_family() {
        let open = TypeFactsCertificationError::FamilyOpen {
            demand: "demand".into(),
            reason: "missing exact premise".into(),
        };
        assert!(is_family_open(&open.at_stage("verification")));
        for error in [
            TypeFactsCertificationError::SourceCensus("foreign declaration".into()),
            TypeFactsCertificationError::ProducerProvenance("wrong producer".into()),
            TypeFactsCertificationError::MissingDemand("demand".into()),
            TypeFactsCertificationError::UnsupportedDemand {
                demand: "demand".into(),
                reason: "unsupported".into(),
            },
            TypeFactsCertificationError::identity_mismatch("test", "root", "own", "foreign"),
        ] {
            assert!(!is_family_open(&error.at_stage("verification")));
        }
    }
}
