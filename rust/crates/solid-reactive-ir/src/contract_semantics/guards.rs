use std::collections::BTreeMap;

use super::{
    Guard, GuardAtom, GuardPartition, GuardTruth, GuardedCase, KnowledgeSet, Literal, ModelError,
    ValueKind,
};

pub(super) fn select_operations(
    partition: &GuardPartition,
    mut evaluate: impl FnMut(&GuardAtom) -> GuardTruth,
) -> KnowledgeSet<super::OperationId> {
    if matches!(partition.cases, KnowledgeSet::Unknown) {
        return KnowledgeSet::Unknown;
    }

    let mut possible = Vec::new();
    let mut otherwise = None;
    for case in partition.cases.items() {
        match case {
            GuardedCase::When { guard, operations } => match evaluate_guard(guard, &mut evaluate) {
                GuardTruth::True => return operations.clone(),
                GuardTruth::Unknown => possible.push(operations.clone()),
                GuardTruth::False => {}
            },
            GuardedCase::Otherwise { operations } => otherwise = Some(operations.clone()),
        }
    }

    if let Some(otherwise) = otherwise {
        possible.push(otherwise);
    }
    if possible.is_empty() {
        return if partition.cases.is_closed() {
            KnowledgeSet::Complete(Vec::new())
        } else {
            KnowledgeSet::Unknown
        };
    }
    let joined = KnowledgeSet::join(possible);
    if partition.cases.is_closed() {
        joined
    } else {
        joined.weaken()
    }
}

fn evaluate_guard(
    guard: &Guard,
    evaluate: &mut impl FnMut(&GuardAtom) -> GuardTruth,
) -> GuardTruth {
    let mut unknown = false;
    for atom in &guard.0 {
        match evaluate(atom) {
            GuardTruth::False => return GuardTruth::False,
            GuardTruth::Unknown => unknown = true,
            GuardTruth::True => {}
        }
    }
    if unknown {
        GuardTruth::Unknown
    } else {
        GuardTruth::True
    }
}

pub(super) fn normalize_guard(guard: &mut Guard, path: &str) -> Result<(), ModelError> {
    if guard.0.is_empty() {
        return Err(ModelError::InvalidGuard {
            path: path.into(),
            reason: "a conditional guard must contain at least one atom".into(),
        });
    }
    for atom in &mut guard.0 {
        match atom {
            GuardAtom::Signature(signature) | GuardAtom::ArtifactCase(signature)
                if signature.is_empty() =>
            {
                return Err(ModelError::InvalidGuard {
                    path: path.into(),
                    reason: "identity atoms must not be empty".into(),
                });
            }
            GuardAtom::ArgumentCount {
                min,
                max: Some(max),
            } if *min > *max => {
                return Err(ModelError::InvalidGuard {
                    path: path.into(),
                    reason: "argument-count minimum exceeds its maximum".into(),
                });
            }
            GuardAtom::Property { name, .. } if name.is_empty() => {
                return Err(ModelError::InvalidGuard {
                    path: path.into(),
                    reason: "property atoms must name a fixed property".into(),
                });
            }
            GuardAtom::Literal {
                value: Literal::Number(number),
                ..
            } => canonicalize_number(number, path)?,
            _ => {}
        }
    }
    guard.0.sort();
    if guard.0.windows(2).any(|atoms| atoms[0] == atoms[1]) {
        return Err(ModelError::InvalidGuard {
            path: path.into(),
            reason: "duplicate guard atom".into(),
        });
    }
    if atoms_contradict(&guard.0) {
        return Err(ModelError::InvalidGuard {
            path: path.into(),
            reason: "the conjunction is unsatisfiable".into(),
        });
    }
    Ok(())
}

fn canonicalize_number(value: &mut String, path: &str) -> Result<(), ModelError> {
    if value.is_empty()
        || value.starts_with('+')
        || value.starts_with("00")
        || value.starts_with("-00")
    {
        return Err(ModelError::InvalidGuard {
            path: path.into(),
            reason: format!("number literal {value:?} is not canonicalizable"),
        });
    }
    let Ok(parsed) = value.parse::<f64>() else {
        return Err(ModelError::InvalidGuard {
            path: path.into(),
            reason: format!("number literal {value:?} is invalid"),
        });
    };
    if !parsed.is_finite() || (parsed == 0.0 && value.starts_with('-')) {
        return Err(ModelError::InvalidGuard {
            path: path.into(),
            reason: format!("number literal {value:?} is not finite canonical data"),
        });
    }
    *value = parsed.to_string();
    Ok(())
}

pub(super) fn guards_overlap(left: &Guard, right: &Guard) -> bool {
    let atoms = left.0.iter().chain(&right.0).cloned().collect::<Vec<_>>();
    !atoms_contradict(&atoms)
}

#[derive(Default)]
struct Constraints<'a> {
    signature: Option<&'a str>,
    argument_count: Option<(u16, Option<u16>)>,
    literals: BTreeMap<(u16, &'a [String]), &'a Literal>,
    kinds: BTreeMap<(u16, &'a [String]), ValueKind>,
    properties: BTreeMap<(u16, &'a [String], &'a str), Option<bool>>,
    tuple_alternatives: BTreeMap<u16, u16>,
    result_protocol: Option<ValueKind>,
    artifact_case: Option<&'a str>,
}

fn atoms_contradict(atoms: &[GuardAtom]) -> bool {
    let mut constraints = Constraints::default();
    for atom in atoms {
        let contradiction = match atom {
            GuardAtom::Signature(value) => set_equal(&mut constraints.signature, value.as_str()),
            GuardAtom::ArgumentCount { min, max } => {
                let next = (*min, *max);
                if let Some(current) = constraints.argument_count {
                    let lower = current.0.max(next.0);
                    let upper = min_upper(current.1, next.1);
                    if upper.is_some_and(|upper| lower > upper) {
                        true
                    } else {
                        constraints.argument_count = Some((lower, upper));
                        false
                    }
                } else {
                    constraints.argument_count = Some(next);
                    false
                }
            }
            GuardAtom::Literal {
                argument,
                path,
                value,
            } => {
                let key = (*argument, path.as_slice());
                constraints
                    .literals
                    .insert(key, value)
                    .is_some_and(|existing| existing != value)
                    || constraints
                        .kinds
                        .get(&key)
                        .is_some_and(|kind| *kind != ValueKind::Plain)
            }
            GuardAtom::ValueKind {
                argument,
                path,
                kind,
            } => {
                let key = (*argument, path.as_slice());
                constraints
                    .kinds
                    .insert(key, *kind)
                    .is_some_and(|existing| existing != *kind)
                    || (*kind != ValueKind::Plain && constraints.literals.contains_key(&key))
            }
            GuardAtom::Property {
                argument,
                path,
                name,
                callable,
            } => {
                let key = (*argument, path.as_slice(), name.as_str());
                match constraints.properties.insert(key, *callable) {
                    Some(Some(existing)) => callable.is_some_and(|value| value != existing),
                    _ => false,
                }
            }
            GuardAtom::TupleAlternative {
                argument,
                alternative,
            } => constraints
                .tuple_alternatives
                .insert(*argument, *alternative)
                .is_some_and(|existing| existing != *alternative),
            GuardAtom::ResultProtocol(kind) => set_equal(&mut constraints.result_protocol, *kind),
            GuardAtom::ArtifactCase(value) => {
                set_equal(&mut constraints.artifact_case, value.as_str())
            }
        };
        if contradiction {
            return true;
        }
    }
    false
}

fn min_upper(left: Option<u16>, right: Option<u16>) -> Option<u16> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.min(right)),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}

fn set_equal<T: Copy + PartialEq>(slot: &mut Option<T>, value: T) -> bool {
    match *slot {
        Some(existing) => existing != value,
        None => {
            *slot = Some(value);
            false
        }
    }
}
