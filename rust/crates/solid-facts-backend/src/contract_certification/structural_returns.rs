//! ADR 0172: complete literal construction and independently proved members.
use super::*;
use solid_reactive_ir::contract_semantics::ObjectProperty;
use typefacts::{ReturnStructure, ReturnStructureKind};

pub(super) fn candidate_at_path<'a>(
    value: &'a ValueShape,
    path: &[ValuePathSegment],
) -> Option<&'a ValueShape> {
    match path.split_first() {
        None => Some(value),
        Some((ValuePathSegment::TupleItem(index), rest)) => {
            let ValueShape::Tuple(items) = value else {
                return None;
            };
            candidate_at_path(items.items().get(usize::try_from(*index).ok()?)?, rest)
        }
        Some((ValuePathSegment::ObjectProperty(name), rest)) => {
            let ValueShape::Object(properties) = value else {
                return None;
            };
            candidate_at_path(
                &properties
                    .items()
                    .iter()
                    .find(|property| property.name == *name)?
                    .value,
                rest,
            )
        }
        _ => None,
    }
}

// Candidate sets have already been weakened to Partial by the scheduler.
// Closing them here is a comparison representation, never an accepted claim.
fn candidate(value: &ValueShape) -> Result<ValueShape, String> {
    Ok(match value {
        ValueShape::Tuple(items) if !matches!(items, KnowledgeSet::Unknown) => {
            ValueShape::Tuple(KnowledgeSet::Complete(
                items
                    .items()
                    .iter()
                    .map(candidate)
                    .collect::<Result<_, _>>()?,
            ))
        }
        ValueShape::Object(properties) if !matches!(properties, KnowledgeSet::Unknown) => {
            let mut values = properties
                .items()
                .iter()
                .map(|property| {
                    Ok(ObjectProperty {
                        name: property.name.clone(),
                        value: candidate(&property.value)?,
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;
            values.sort_by(|a, b| a.name.cmp(&b.name));
            if values.windows(2).any(|pair| pair[0].name == pair[1].name)
                || values.iter().any(|p| p.name == "__proto__")
            {
                return Err("ambiguous structural property names".into());
            }
            ValueShape::Object(KnowledgeSet::Complete(values))
        }
        ValueShape::Plain => ValueShape::Plain,
        ValueShape::Parameter { index, path } if path.is_empty() => ValueShape::Parameter {
            index: *index,
            path: vec![],
        },
        ValueShape::Reactive {
            role: ReactiveRole::Accessor,
            resource: None,
            capabilities: KnowledgeSet::Unknown,
        } => value.clone(),
        _ => return Err("unsupported structural member claim".into()),
    })
}

pub(super) fn require_callability(
    value: &ValueShape,
    path: &[ValuePathSegment],
    demanded: DemandedCallability,
) -> Result<(), String> {
    let value = candidate_at_path(value, path).ok_or("structural positive path is absent")?;
    let expected = match value {
        ValueShape::Tuple(_) | ValueShape::Object(_) | ValueShape::Plain => {
            DemandedCallability::NonCallable
        }
        ValueShape::Parameter { .. } => DemandedCallability::Unknown,
        ValueShape::Reactive {
            role: ReactiveRole::Accessor,
            ..
        } => DemandedCallability::Callable,
        _ => return Err("no structural callability evidence for this member".into()),
    };
    // Unknown is an unasserted callability demand, not evidence that the
    // value's callability is unknown. The entire tree was already proved.
    if demanded != DemandedCallability::Unknown && demanded != expected {
        return Err("structural positive callability contradicts the census".into());
    }
    Ok(())
}

fn within(inner: &typefacts::Location, outer: &typefacts::Location) -> bool {
    inner.path == outer.path
        && outer.start_byte <= inner.start_byte
        && inner.start_byte < inner.end_byte
        && inner.end_byte <= outer.end_byte
}

#[allow(clippy::too_many_arguments)] // recursive evidence context and bounded traversal state
fn shape(
    node: &ReturnStructure,
    parent: &typefacts::Location,
    implementation: &typefacts::ExportImplementationTranscript,
    certified: &super::super::ArtifactSnapshot,
    roots: &[SnapshotSourceRoot<'_>],
    depth: usize,
    budget: &mut usize,
    sites: &mut Vec<String>,
) -> Result<ValueShape, String> {
    if depth > 8 || *budget == 0 || !within(&node.location, parent) {
        return Err("structural return exceeds its bounds or transcript frame".into());
    }
    *budget -= 1;
    sites.push(format!(
        "structural-member:{}:{}..{}:{:?}",
        node.location.path, node.location.start_byte, node.location.end_byte, node.kind
    ));
    if node.kind == ReturnStructureKind::Leaf {
        if node.complete || !node.items.is_empty() || !node.properties.is_empty() {
            return Err("a structural leaf states container enumeration".into());
        }
        if node.primitive_syntax
            || (!node.default_library_call.is_empty()
                && reviewed_default_library_call_return(&node.default_library_call)
                    == Some(ValueShape::Plain))
        {
            sites.push(format!(
                "structural-primitive:syntax={}:default-library={}",
                node.primitive_syntax, node.default_library_call
            ));
            return Ok(ValueShape::Plain);
        }
        if let Some(parameter) = &node.parameter {
            if !parameter.path.is_empty() {
                return Err("structural member is not a whole caller parameter".into());
            }
            let index = u16::try_from(parameter.parameter_index)
                .map_err(|_| "structural parameter exceeds model limits")?;
            sites.push(format!("structural-parameter:{index}"));
            return Ok(ValueShape::Parameter {
                index,
                path: vec![],
            });
        }
        if node.sources.is_empty() || node.sources.iter().any(|source| !source.path.is_empty()) {
            return Err("structural leaf has no exact primitive, caller parameter or owned accessor evidence".into());
        }
        let witnesses = node
            .sources
            .iter()
            .map(|source| owned_returned_accessor_witness(source, implementation, certified, roots))
            .collect::<Result<Vec<_>, _>>()?;
        if witnesses.iter().any(|w| w.0 != witnesses[0].0) {
            return Err("structural accessor traces disagree".into());
        }
        sites.extend(witnesses.into_iter().map(|w| w.1));
        return Ok(ValueShape::Reactive {
            role: ReactiveRole::Accessor,
            resource: None,
            capabilities: KnowledgeSet::Unknown,
        });
    }
    if !node.complete
        || node.primitive_syntax
        || node.parameter.is_some()
        || !node.sources.is_empty()
        || !node.default_library_call.is_empty()
    {
        return Err("literal member enumeration is absent, partial or contradictory".into());
    }
    match node.kind {
        ReturnStructureKind::Tuple => {
            if !node.properties.is_empty() {
                return Err("tuple census states object properties".into());
            }
            if node
                .items
                .windows(2)
                .any(|pair| pair[0].location.end_byte > pair[1].location.start_byte)
            {
                return Err("tuple member spans overlap or reorder".into());
            }
            Ok(ValueShape::Tuple(KnowledgeSet::Complete(
                node.items
                    .iter()
                    .map(|item| {
                        shape(
                            item,
                            &node.location,
                            implementation,
                            certified,
                            roots,
                            depth + 1,
                            budget,
                            sites,
                        )
                    })
                    .collect::<Result<_, _>>()?,
            )))
        }
        ReturnStructureKind::Object => {
            if !node.items.is_empty() {
                return Err("object census states tuple items".into());
            }
            let mut properties = Vec::new();
            let mut end = node.location.start_byte;
            for property in &node.properties {
                if property.name == "__proto__"
                    || !within(&property.key, &node.location)
                    || property.key.start_byte < end
                    || property.key.end_byte > property.value.location.start_byte
                {
                    return Err(
                        "object key spans overlap, reorder or escape their construction".into(),
                    );
                }
                let value = shape(
                    &property.value,
                    &node.location,
                    implementation,
                    certified,
                    roots,
                    depth + 1,
                    budget,
                    sites,
                )?;
                end = property.value.location.end_byte;
                properties.push(ObjectProperty {
                    name: property.name.clone(),
                    value,
                });
            }
            properties.sort_by(|a, b| a.name.cmp(&b.name));
            if properties
                .windows(2)
                .any(|pair| pair[0].name == pair[1].name)
            {
                return Err("duplicate object properties".into());
            }
            Ok(ValueShape::Object(KnowledgeSet::Complete(properties)))
        }
        ReturnStructureKind::Leaf => unreachable!(),
    }
}

pub(super) fn return_sites(
    export: &ExportSemantics,
    implementation: &typefacts::ExportImplementationTranscript,
    certified: &super::super::ArtifactSnapshot,
    roots: &[SnapshotSourceRoot<'_>],
) -> Result<Vec<String>, String> {
    let claimed = export
        .operation_claim(ClaimDomain::Returns)
        .ok_or("no structural returns claim")?
        .items()
        .iter()
        .map(|id| {
            let op = export
                .operation(&id.0)
                .ok_or("structural return operation is absent")?;
            if !op.is_bare_return() {
                return Err("structural output is not a bare return".into());
            }
            let output = op.output.as_ref().ok_or("structural output is absent")?;
            if !matches!(output, ValueShape::Tuple(_) | ValueShape::Object(_)) {
                return Err("mixed structural return claim is unsupported".into());
            }
            candidate(output)
        })
        .collect::<Result<Vec<_>, String>>()?;
    let flow = require_plain_classified_completion(implementation, "structural return")?;
    // Export and resolved declaration locations can both name only an
    // identifier. The producer states the exact body it actually walked.
    let frame = flow
        .body_location
        .as_ref()
        .ok_or("structural body frame is absent")?;
    let declaration = implementation
        .implementation_of
        .as_ref()
        .or(implementation.declaration.as_ref())
        .ok_or("structural implementation declaration is absent")?;
    if frame.path != declaration.location.path || frame.start_byte >= frame.end_byte {
        return Err("structural body frame contradicts its declaration".into());
    }
    if flow.end_reach != Some(Reachability::Unreachable)
        || flow
            .returns
            .iter()
            .any(|site| site.reach != Reachability::Unreachable && site.value.is_none())
    {
        return Err("structural return cannot exclude an undefined completion".into());
    }
    let mut produced = Vec::new();
    let mut sites = Vec::new();
    for site in flow
        .returns
        .iter()
        .filter(|site| site.reach != Reachability::Unreachable && site.value.is_some())
    {
        if !within(&site.location, frame) {
            return Err("return site escapes its implementation".into());
        }
        let nodes = if site.arms.is_empty() {
            vec![(&site.location, site.structure.as_ref())]
        } else {
            site.arms
                .iter()
                .map(|arm| (&arm.location, arm.structure.as_ref()))
                .collect()
        };
        for (location, node) in nodes {
            if !within(location, &site.location) {
                return Err("return arm escapes its site".into());
            }
            let node = node.ok_or("live return has no exhaustive literal structure")?;
            if !matches!(
                node.kind,
                ReturnStructureKind::Tuple | ReturnStructureKind::Object
            ) {
                return Err("structural return root is not a literal container".into());
            }
            let value = shape(
                node,
                location,
                implementation,
                certified,
                roots,
                0,
                &mut 128,
                &mut sites,
            )?;
            if !claimed.contains(&value) {
                return Err("literal return does not match the entire claimed structure".into());
            }
            produced.push(value);
        }
    }
    if claimed.is_empty() || claimed.iter().any(|value| !produced.contains(value)) {
        return Err("structural claim has no live completion witness".into());
    }
    sites.push(format!("structural-returns-total:{}", produced.len()));
    Ok(sites)
}
