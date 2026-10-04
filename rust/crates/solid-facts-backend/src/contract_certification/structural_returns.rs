//! ADR 0172: complete literal construction and independently proved members.
use super::*;
use solid_reactive_ir::contract_semantics::{ObjectProperty, ValuePath};
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
        // ADR 0177: a member the claim leaves undescribed asserts nothing, so
        // any construction covers it (`covers`). It is never the root: a
        // structural claim is a tuple or an object.
        ValueShape::Unknown => ValueShape::Unknown,
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
    gaps: &mut Gaps,
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
            let mut items = Vec::with_capacity(node.items.len());
            for (index, item) in node.items.iter().enumerate() {
                gaps.path.push(ValuePathSegment::TupleItem(
                    u32::try_from(index).map_err(|_| "tuple member index exceeds model limits")?,
                ));
                let value = member(
                    item,
                    &node.location,
                    implementation,
                    certified,
                    roots,
                    depth + 1,
                    budget,
                    sites,
                    gaps,
                );
                gaps.path.pop();
                items.push(value?);
            }
            Ok(ValueShape::Tuple(KnowledgeSet::Complete(items)))
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
                gaps.path
                    .push(ValuePathSegment::ObjectProperty(property.name.clone()));
                let value = member(
                    &property.value,
                    &node.location,
                    implementation,
                    certified,
                    roots,
                    depth + 1,
                    budget,
                    sites,
                    gaps,
                );
                gaps.path.pop();
                let value = value?;
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

/// ADR 0177: the members of the tree being walked that the census could not
/// describe, each with the path that reaches it and the census's reason.
#[derive(Default)]
struct Gaps {
    path: Vec<ValuePathSegment>,
    members: Vec<(ValuePath, String)>,
}

/// The marker a structural refusal carries when the claim is covered once the
/// listed members are left undescribed. The JSON array of member paths
/// follows it (`withheld_member_paths` reads it back), then `": "` and the
/// first member's own refusal.
pub(crate) const STRUCTURAL_MEMBERS_UNKNOWN_MARKER: &str = "structural members left unknown ";

/// ADR 0177: one member of a literal container. A leaf the census cannot
/// describe is `Unknown` -- the one claim that is true of every value -- and
/// its path and reason are kept, so a claim that does describe it still
/// refuses by name and can be weakened at exactly that member. A container
/// that contradicts itself is still an error.
#[allow(clippy::too_many_arguments)] // the same evidence context as `shape`
fn member(
    node: &ReturnStructure,
    parent: &typefacts::Location,
    implementation: &typefacts::ExportImplementationTranscript,
    certified: &super::super::ArtifactSnapshot,
    roots: &[SnapshotSourceRoot<'_>],
    depth: usize,
    budget: &mut usize,
    sites: &mut Vec<String>,
    gaps: &mut Gaps,
) -> Result<ValueShape, String> {
    let leaf = node.kind == ReturnStructureKind::Leaf;
    match shape(
        node,
        parent,
        implementation,
        certified,
        roots,
        depth,
        budget,
        sites,
        gaps,
    ) {
        Err(reason) if leaf && within(&node.location, parent) => {
            gaps.members.push((ValuePath(gaps.path.clone()), reason));
            Ok(ValueShape::Unknown)
        }
        result => result,
    }
}

/// Whether a claimed structure is true of a produced one: equal, except that a
/// member the claim leaves `Unknown` is covered by anything.
fn covers(claimed: &ValueShape, produced: &ValueShape) -> bool {
    match (claimed, produced) {
        (ValueShape::Unknown, _) => true,
        (ValueShape::Tuple(claimed), ValueShape::Tuple(produced)) => {
            matches!(claimed, KnowledgeSet::Complete(_))
                && matches!(produced, KnowledgeSet::Complete(_))
                && claimed.items().len() == produced.items().len()
                && claimed
                    .items()
                    .iter()
                    .zip(produced.items())
                    .all(|(claimed, produced)| covers(claimed, produced))
        }
        (ValueShape::Object(claimed), ValueShape::Object(produced)) => {
            matches!(claimed, KnowledgeSet::Complete(_))
                && matches!(produced, KnowledgeSet::Complete(_))
                && claimed.items().len() == produced.items().len()
                && claimed
                    .items()
                    .iter()
                    .zip(produced.items())
                    .all(|(claimed, produced)| {
                        claimed.name == produced.name && covers(&claimed.value, &produced.value)
                    })
        }
        (claimed, produced) => claimed == produced,
    }
}

/// ADR 0177: the members to leave undescribed so that `claimed` covers
/// `produced`, when that is the *only* disagreement: the two have the same
/// containers, lengths and keys, and every leaf agrees or was left `Unknown`
/// by the census. `None` when anything else differs, or when nothing
/// described would survive -- an all-unknown container states no member.
fn unknown_members(claimed: &ValueShape, produced: &ValueShape) -> Option<Vec<ValuePath>> {
    fn walk(
        claimed: &ValueShape,
        produced: &ValueShape,
        path: &mut Vec<ValuePathSegment>,
        weakened: &mut Vec<ValuePath>,
        kept: &mut bool,
    ) -> bool {
        match (claimed, produced) {
            (ValueShape::Unknown, _) => true,
            (_, ValueShape::Unknown) if !path.is_empty() => {
                weakened.push(ValuePath(path.clone()));
                true
            }
            (
                ValueShape::Tuple(KnowledgeSet::Complete(claimed)),
                ValueShape::Tuple(KnowledgeSet::Complete(produced)),
            ) => {
                claimed.len() == produced.len()
                    && claimed
                        .iter()
                        .zip(produced)
                        .enumerate()
                        .all(|(index, (c, p))| {
                            let Ok(index) = u32::try_from(index) else {
                                return false;
                            };
                            path.push(ValuePathSegment::TupleItem(index));
                            let agrees = walk(c, p, path, weakened, kept);
                            path.pop();
                            agrees
                        })
            }
            (
                ValueShape::Object(KnowledgeSet::Complete(claimed)),
                ValueShape::Object(KnowledgeSet::Complete(produced)),
            ) => {
                claimed.len() == produced.len()
                    && claimed.iter().zip(produced).all(|(c, p)| {
                        if c.name != p.name {
                            return false;
                        }
                        path.push(ValuePathSegment::ObjectProperty(c.name.clone()));
                        let agrees = walk(&c.value, &p.value, path, weakened, kept);
                        path.pop();
                        agrees
                    })
            }
            (ValueShape::Tuple(_) | ValueShape::Object(_), _) => false,
            (claimed, produced) => {
                *kept |= claimed == produced;
                claimed == produced
            }
        }
    }
    let mut weakened = Vec::new();
    let mut kept = false;
    (walk(claimed, produced, &mut Vec::new(), &mut weakened, &mut kept)
        && kept
        && !weakened.is_empty())
    .then_some(weakened)
}

/// ADR 0177: the refusal that asks to weaken members, decided over *every*
/// live completion at once. Only a single claimed structure is weakened: the
/// members the census left unknown at any mismatched completion, united, must
/// leave a claim that covers every completion -- the matched ones and the
/// mismatched ones -- and still describe something. Anything else is the
/// plain refusal, which withdraws the return as before, so a weakening never
/// trades one refusal for a later one.
fn weakening_refusal(
    claimed: &[ValueShape],
    produced: &[ValueShape],
    mismatched: &[(ValueShape, Gaps)],
) -> Option<String> {
    let [claim] = claimed else {
        return None;
    };
    let mut members: Vec<ValuePath> = Vec::new();
    for (value, _) in mismatched {
        for member in unknown_members(claim, value)? {
            if !members.contains(&member) {
                members.push(member);
            }
        }
    }
    let mut weakened = claim.clone();
    for member in &members {
        *candidate_at_path_mut(&mut weakened, &member.0)? = ValueShape::Unknown;
    }
    let everything = produced
        .iter()
        .chain(mismatched.iter().map(|(value, _)| value));
    if !everything.clone().all(|value| covers(&weakened, value))
        || unknown_members(claim, &weakened).is_none()
    {
        return None;
    }
    let gaps = mismatched
        .iter()
        .find(|(_, gaps)| !gaps.members.is_empty())
        .map(|(_, gaps)| gaps)?;
    Some(members_unknown_refusal(&members, gaps))
}

fn candidate_at_path_mut<'a>(
    value: &'a mut ValueShape,
    path: &[ValuePathSegment],
) -> Option<&'a mut ValueShape> {
    let Some((segment, rest)) = path.split_first() else {
        return Some(value);
    };
    let next = match (value, segment) {
        (ValueShape::Tuple(KnowledgeSet::Complete(items)), ValuePathSegment::TupleItem(index)) => {
            items.get_mut(usize::try_from(*index).ok()?)?
        }
        (
            ValueShape::Object(KnowledgeSet::Complete(properties)),
            ValuePathSegment::ObjectProperty(name),
        ) => {
            &mut properties
                .iter_mut()
                .find(|property| property.name == *name)?
                .value
        }
        _ => return None,
    };
    candidate_at_path_mut(next, rest)
}

fn members_unknown_refusal(members: &[ValuePath], gaps: &Gaps) -> String {
    let paths = members
        .iter()
        .map(|member| {
            member
                .0
                .iter()
                .map(|segment| match segment {
                    ValuePathSegment::TupleItem(index) => serde_json::json!(index),
                    ValuePathSegment::ObjectProperty(name) => serde_json::json!(name),
                    _ => unreachable!("structural member paths are items and properties"),
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let reason = members
        .first()
        .and_then(|first| gaps.members.iter().find(|(path, _)| path == first))
        .map_or(
            "structural member is not described as claimed",
            |(_, reason)| reason.as_str(),
        );
    format!(
        "{STRUCTURAL_MEMBERS_UNKNOWN_MARKER}{}: {reason}",
        serde_json::Value::Array(paths.into_iter().map(serde_json::Value::Array).collect())
    )
}

/// ADR 0177: the member paths a structural refusal names after
/// [`STRUCTURAL_MEMBERS_UNKNOWN_MARKER`], wherever in the reason the census's
/// wrappers placed it. `None` when the reason carries no such list.
pub(crate) fn withheld_member_paths(reason: &str) -> Option<Vec<ValuePath>> {
    let start =
        reason.find(STRUCTURAL_MEMBERS_UNKNOWN_MARKER)? + STRUCTURAL_MEMBERS_UNKNOWN_MARKER.len();
    let list = serde_json::Deserializer::from_str(&reason[start..])
        .into_iter::<Vec<Vec<serde_json::Value>>>()
        .next()?
        .ok()?;
    list.into_iter()
        .map(|segments| {
            segments
                .into_iter()
                .map(|segment| match segment {
                    serde_json::Value::Number(index) => index
                        .as_u64()
                        .and_then(|index| u32::try_from(index).ok())
                        .map(ValuePathSegment::TupleItem),
                    serde_json::Value::String(name) => Some(ValuePathSegment::ObjectProperty(name)),
                    _ => None,
                })
                .collect::<Option<Vec<_>>>()
                .filter(|path| !path.is_empty())
                .map(ValuePath)
        })
        .collect()
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
    let mut produced: Vec<ValueShape> = Vec::new();
    let mut mismatched: Vec<(ValueShape, Gaps)> = Vec::new();
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
            let mut gaps = Gaps::default();
            let value = shape(
                node,
                location,
                implementation,
                certified,
                roots,
                0,
                &mut 128,
                &mut sites,
                &mut gaps,
            )?;
            if !claimed.iter().any(|claim| covers(claim, &value)) {
                mismatched.push((value, gaps));
                continue;
            }
            produced.push(value);
        }
    }
    if !mismatched.is_empty() {
        return Err(
            weakening_refusal(&claimed, &produced, &mismatched).unwrap_or_else(|| {
                "literal return does not match the entire claimed structure".into()
            }),
        );
    }
    if claimed.is_empty()
        || claimed
            .iter()
            .any(|claim| !produced.iter().any(|value| covers(claim, value)))
    {
        return Err("structural claim has no live completion witness".into());
    }
    sites.push(format!("structural-returns-total:{}", produced.len()));
    Ok(sites)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn accessor() -> ValueShape {
        ValueShape::Reactive {
            role: ReactiveRole::Accessor,
            resource: None,
            capabilities: KnowledgeSet::Unknown,
        }
    }

    fn tuple(items: Vec<ValueShape>) -> ValueShape {
        ValueShape::Tuple(KnowledgeSet::Complete(items))
    }

    #[test]
    fn an_unknown_member_is_covered_by_any_construction_of_the_same_length() {
        let claim = tuple(vec![accessor(), ValueShape::Unknown]);
        assert!(covers(&claim, &tuple(vec![accessor(), ValueShape::Plain])));
        assert!(covers(
            &claim,
            &tuple(vec![accessor(), ValueShape::Unknown])
        ));
        assert!(!covers(&claim, &tuple(vec![accessor()])));
        assert!(!covers(
            &claim,
            &tuple(vec![ValueShape::Plain, ValueShape::Plain])
        ));
        assert!(!covers(
            &tuple(vec![accessor(), accessor()]),
            &tuple(vec![accessor(), ValueShape::Unknown])
        ));
    }

    #[test]
    fn only_members_the_census_left_unknown_are_weakened() {
        let claim = tuple(vec![accessor(), accessor(), ValueShape::Unknown]);
        let produced = tuple(vec![accessor(), ValueShape::Unknown, ValueShape::Unknown]);
        assert_eq!(
            unknown_members(&claim, &produced),
            Some(vec![ValuePath(vec![ValuePathSegment::TupleItem(1)])])
        );
        // A described member that disagrees is not a gap: nothing is weakened.
        let disagreeing = tuple(vec![
            ValueShape::Plain,
            ValueShape::Unknown,
            ValueShape::Unknown,
        ]);
        assert_eq!(unknown_members(&claim, &disagreeing), None);
        // A different length is not a gap either.
        assert_eq!(unknown_members(&claim, &tuple(vec![accessor()])), None);
        // Nothing described would survive: no weakened claim is offered.
        let all_unknown = tuple(vec![
            ValueShape::Unknown,
            ValueShape::Unknown,
            ValueShape::Unknown,
        ]);
        assert_eq!(unknown_members(&claim, &all_unknown), None);
    }

    #[test]
    fn a_weakening_must_cover_every_completion_at_once() {
        let claim = tuple(vec![accessor(), accessor(), ValueShape::Unknown]);
        let gap = |index| Gaps {
            path: Vec::new(),
            members: vec![(
                ValuePath(vec![ValuePathSegment::TupleItem(index)]),
                "no owned accessor evidence".into(),
            )],
        };
        let unproven = tuple(vec![accessor(), ValueShape::Unknown, ValueShape::Unknown]);
        let reason = weakening_refusal(
            std::slice::from_ref(&claim),
            &[],
            &[(unproven.clone(), gap(1))],
        )
        .expect("one unproven member weakens");
        assert_eq!(
            withheld_member_paths(&reason),
            Some(vec![ValuePath(vec![ValuePathSegment::TupleItem(1)])])
        );
        // Another completion that disagrees on a described member: weakening
        // item 1 would only trade this refusal for a later one.
        let plain = tuple(vec![accessor(), ValueShape::Plain, ValueShape::Unknown]);
        assert_eq!(
            weakening_refusal(
                std::slice::from_ref(&claim),
                &[],
                &[(unproven.clone(), gap(1)), (plain, Gaps::default())]
            ),
            None
        );
        // Two completions with different gaps unite into one weakening.
        let other = tuple(vec![ValueShape::Unknown, accessor(), ValueShape::Unknown]);
        assert_eq!(
            weakening_refusal(
                std::slice::from_ref(&claim),
                &[],
                &[(unproven, gap(1)), (other, gap(0))]
            ),
            None,
            "nothing described survives both"
        );
        // Two claimed alternatives are never weakened.
        assert_eq!(
            weakening_refusal(
                &[claim.clone(), claim],
                &[],
                &[(
                    tuple(vec![accessor(), ValueShape::Unknown, ValueShape::Unknown]),
                    gap(1)
                )]
            ),
            None
        );
    }

    #[test]
    fn member_paths_round_trip_through_the_refusal_text() {
        let members = vec![
            ValuePath(vec![ValuePathSegment::TupleItem(1)]),
            ValuePath(vec![
                ValuePathSegment::TupleItem(0),
                ValuePathSegment::ObjectProperty("a\"]: b".into()),
            ]),
        ];
        let gaps = Gaps {
            path: Vec::new(),
            members: vec![(members[0].clone(), "no owned accessor evidence".into())],
        };
        let reason = format!(
            "recursive-value-shape (artifact-case:x:createRAF): {}",
            members_unknown_refusal(&members, &gaps)
        );
        assert!(reason.ends_with(": no owned accessor evidence"));
        assert_eq!(withheld_member_paths(&reason), Some(members));
        assert_eq!(
            withheld_member_paths("structural leaf has no evidence"),
            None
        );
        assert_eq!(
            withheld_member_paths(&format!("{STRUCTURAL_MEMBERS_UNKNOWN_MARKER}[[]]: x")),
            None,
            "an empty path names no member"
        );
    }
}
