use super::{Guard, GuardAtom, GuardPartition, GuardTruth, GuardedCase, KnowledgeSet};

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
                GuardTruth::True => {
                    return if partition.cases.is_closed() {
                        operations.clone()
                    } else {
                        operations.clone().weaken()
                    };
                }
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
