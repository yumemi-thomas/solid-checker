//! A lower bound from alternative exact owner calls on every normal completion.
//! Uses existing claim forms and existing source/frame/dead-return facts.
use super::*;
use solid_facts::core::Span;

pub(super) fn require(
    plan: &CertificationPlan,
    operation: &Operation,
    implementation: &typefacts::ExportImplementationTranscript,
    evidence: CensusEvidence<'_>,
    sites: &mut Vec<String>,
) -> Result<(), String> {
    prove(
        &plan.snapshot,
        &certified_runtime_sources(plan),
        operation,
        implementation,
        evidence,
        sites,
    )
}

fn prove(
    certified: &super::super::ArtifactSnapshot,
    runtime_sources: &std::collections::BTreeSet<String>,
    operation: &Operation,
    implementation: &typefacts::ExportImplementationTranscript,
    evidence: CensusEvidence<'_>,
    sites: &mut Vec<String>,
) -> Result<(), String> {
    if !operation.imposes_owner_requirement()
        || operation.owner.source != OwnerSource::AmbientAtCall
        || !operation.resources.is_empty()
        || (operation.owner.requirements.cleanup == Requirement::Required
            && operation.owner.requirements.child_owners == Requirement::Required)
        || !matches!(
            operation.kind,
            OperationKind::Create | OperationKind::Compute | OperationKind::Cleanup
        )
        || operation.cardinality.scope != Some(CardinalityScope::Call)
        || operation.cardinality.min != Some(1)
        || operation.cardinality.max != Some(UpperBound::Many)
    {
        return Err("not a supported owner-registration lower bound".into());
    }
    let flow = require_plain_classified_completion(implementation, "owner cover")?;
    let body = flow
        .body_location
        .as_ref()
        .ok_or("owner cover has no body frame")?;
    let declaration = implementation
        .implementation_of
        .as_ref()
        .or(implementation.declaration.as_ref())
        .ok_or("owner cover has no declaration")?;
    if body.path != declaration.location.path || declaration.source_file != body.path {
        return Err("owner cover source and declaration disagree".into());
    }
    let paths = evidence
        .roots
        .iter()
        .map(|root| root.path.clone())
        .collect::<Vec<_>>();
    let (index, relative) = strip_materialized_source_root(&declaration.source_file, &paths)
        .ok_or("owner cover source has no authenticated root")?;
    let root = &evidence.roots[index];
    if root.dependency
        || root.snapshot.root() != certified.root()
        || !runtime_sources.contains(relative)
    {
        return Err("owner cover is not this artifact's runtime source".into());
    }
    let source = census_source_text(
        root.snapshot
            .read(relative)
            .ok_or("owner cover source absent")?,
    )
    .ok_or("owner cover source is not UTF-8")?;
    let facts = CensusSourceFacts {
        text: source.into(),
        facts: solid_facts::ast::extract(relative, source)
            .map_err(|_| "owner cover source does not parse")?,
    };
    let body_span = span(body)?;
    let declared_span = span(&declaration.location)?;
    let matching = facts
        .function_nodes()
        .filter(|node| {
            node.body == body_span
                && (node.span == declared_span
                    || node.demand_span == declared_span
                    || node.name == Some(declared_span))
        })
        .count();
    if matching != 1 {
        return Err("owner cover body is not the exact resolved callable".into());
    }
    let expected = if operation.owner.requirements.cleanup == Requirement::Required {
        solid_dialect::OwnerRequirementRole::Cleanup
    } else if operation.owner.requirements.child_owners == Requirement::Required {
        solid_dialect::OwnerRequirementRole::Effect
    } else {
        return Err("owner cover has no supported owner role".into());
    };
    let calls = implementation
        .calls
        .iter()
        .filter(|call| {
            is_call_expression(call)
                && !call.captured
                && !call.target.is_empty()
                && call.target_module.as_ref() == "solid-js"
                && call.reach != Reachability::Unreachable
                && call.location.path == body.path
                && solid_dialect::unambiguous_owner_requirement_role(&call.target_name)
                    == Some(expected)
        })
        .collect::<Vec<_>>();
    let candidates = calls
        .iter()
        .map(|call| span(&call.location))
        .collect::<Result<Vec<_>, _>>()?;
    let dead = flow
        .returns
        .iter()
        .filter(|returned| returned.reach == Reachability::Unreachable)
        .map(|returned| {
            if returned.location.path != body.path {
                return Err("dead return escapes body source".into());
            }
            span(&returned.location)
        })
        .collect::<Result<Vec<_>, String>>()?;
    if solid_facts::ast::completion_call_cover(
        Path::new(relative),
        source,
        body_span,
        &candidates,
        &dead,
    ) != Some(true)
    {
        return Err("owner calls do not cover every modeled normal completion".into());
    }
    sites.push(format!(
        "owner-normal-completion-cover:{}:{}..{}:{}",
        body.path,
        body.start_byte,
        body.end_byte,
        certified.root()
    ));
    for call in calls {
        sites.push(format!(
            "owner-cover-call:{}:{}..{}:{}",
            call.location.path, call.location.start_byte, call.location.end_byte, call.target
        ));
    }
    for returned in dead {
        sites.push(format!(
            "owner-cover-unreachable-return:{}:{}..{}",
            body.path, returned.start, returned.end
        ));
    }
    Ok(())
}

fn span(location: &typefacts::Location) -> Result<Span, String> {
    if location.start_byte >= location.end_byte {
        return Err("empty owner-cover span".into());
    }
    Ok(Span::new(
        u32::try_from(location.start_byte).map_err(|_| "owner-cover span overflows")?,
        u32::try_from(location.end_byte).map_err(|_| "owner-cover span overflows")?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn owner_cover_binds_calls_role_source_and_the_exact_body() {
        let source =
            "function factory(flag) { if(flag) createEffect(a,b); else createRenderEffect(a,b); }";
        let path = "/project/node_modules/pkg/dist/index.js";
        let locate = |text: &str| {
            let start = source.find(text).unwrap();
            json!({ "path": path, "startByte": start, "endByte": start + text.len() })
        };
        let call = |text: &str, name: &str| {
            json!({ "location": locate(text), "kind": "call",
            "reach": "reachable", "target": format!("symbol:{name}"), "targetName": name, "targetModule": "solid-js" })
        };
        let mut implementation: typefacts::ExportImplementationTranscript = serde_json::from_value(json!({
            "location": locate("factory"), "completionForm": "plain",
            "declaration": { "location": locate("factory"), "sourceFile": path, "name": "factory", "kind": "FunctionDeclaration" },
            "controlFlow": { "bodyLocation": { "path": path, "startByte": source.find('{').unwrap(), "endByte": source.len() },
                "endReach": "reachable" },
            "calls": [call("createEffect(a,b)", "createEffect"), call("createRenderEffect(a,b)", "createRenderEffect")]
        })).unwrap();
        let certified = super::super::super::super::ArtifactSnapshot {
            package_name: "pkg".into(),
            package_version: "1.0.0".into(),
            package_integrity: "sha512:test".into(),
            files: std::sync::Arc::new(std::collections::BTreeMap::from([(
                "dist/index.js".into(),
                std::sync::Arc::<[u8]>::from(source.as_bytes()),
            )])),
            directories: std::sync::Arc::new(std::collections::BTreeSet::new()),
            root: "snapshot".into(),
            provenance_root: "provenance".into(),
        };
        let mut roots = vec![SnapshotSourceRoot {
            path: "/project/node_modules/pkg/".into(),
            evidence_prefix: "/node_modules/pkg/".into(),
            snapshot: &certified,
            dependency: false,
        }];
        let mut operation = Operation {
            id: OperationId("owner".into()),
            kind: OperationKind::Compute,
            guard: None,
            trigger: None,
            at: None,
            schedule: None,
            tracking: solid_reactive_ir::contract_semantics::Tracking::Unknown,
            cardinality: solid_reactive_ir::contract_semantics::Cardinality {
                min: Some(1),
                max: Some(UpperBound::Many),
                scope: Some(CardinalityScope::Call),
            },
            owner: solid_reactive_ir::contract_semantics::OwnerRelation::default(),
            inputs: vec![],
            output: None,
            resources: std::collections::BTreeSet::new(),
            composed_from: None,
            protocol: None,
        };
        operation.owner.source = OwnerSource::AmbientAtCall;
        operation.owner.requirements.owner = Requirement::Required;
        operation.owner.requirements.child_owners = Requirement::Required;
        let runtime = std::collections::BTreeSet::from(["dist/index.js".into()]);
        let run = |implementation: &typefacts::ExportImplementationTranscript,
                   roots: &[SnapshotSourceRoot<'_>]| {
            prove(
                &certified,
                &runtime,
                &operation,
                implementation,
                CensusEvidence {
                    roots,
                    locals: &[],
                    dependencies: &[],
                },
                &mut Vec::new(),
            )
        };
        run(&implementation, &roots).expect("two exact dialect calls cover the branches");
        let original = implementation.clone();
        implementation.calls[1].target_module = "local".into();
        assert!(
            run(&implementation, &roots).is_err(),
            "a same-named local is no witness"
        );
        implementation = original.clone();
        implementation.calls[1].target_name = "onCleanup".into();
        assert!(
            run(&implementation, &roots).is_err(),
            "cleanup does not cover an effect requirement"
        );
        implementation = original.clone();
        implementation.calls[1].captured = true;
        assert!(
            run(&implementation, &roots).is_err(),
            "nested calls do not witness the factory"
        );
        implementation = original.clone();
        implementation.calls[1].location.start_byte += 1;
        assert!(
            run(&implementation, &roots).is_err(),
            "candidate spans must bind exact syntax"
        );
        implementation = original.clone();
        implementation
            .control_flow
            .as_mut()
            .unwrap()
            .body_location
            .as_mut()
            .unwrap()
            .start_byte += 1;
        assert!(
            run(&implementation, &roots).is_err(),
            "the body must bind exact syntax"
        );
        roots[0].dependency = true;
        assert!(
            run(&original, &roots).is_err(),
            "a dependency is not this artifact"
        );
        roots[0].dependency = false;
        let mut combined = operation.clone();
        combined.owner.requirements.cleanup = Requirement::Required;
        assert!(
            prove(
                &certified,
                &runtime,
                &combined,
                &original,
                CensusEvidence {
                    roots: &roots,
                    locals: &[],
                    dependencies: &[]
                },
                &mut Vec::new()
            )
            .is_err(),
            "one role cannot prove both cleanup and child-owner requirements"
        );
    }
}
