//! The kept dependency graph of a K2 or K3 solver: each node's definition
//! (its input and ordered predecessors) and the reverse map.

use std::collections::BTreeSet;
use std::hash::Hash;

use super::{DenseIndex, Slot};

/// A node's definition: its direct value or input, and the nodes it reads,
/// in the order its step sees them.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Definition<K, I, E> {
    pub(crate) input: I,
    pub(crate) preds: Vec<(K, E)>,
}

struct Node<I, E> {
    /// The definition, with its predecessors as slots.
    defined: Option<(I, Vec<(Slot, E)>)>,
    readers: BTreeSet<Slot>,
}

impl<I, E> Default for Node<I, E> {
    fn default() -> Self {
        Self {
            defined: None,
            readers: BTreeSet::new(),
        }
    }
}

/// A definition change: `None` undefines the key.
pub(crate) type Change<K, I, E> = (K, Option<Definition<K, I, E>>);

/// What [`KeptGraph::apply`] changed.
pub(super) struct Applied<K> {
    /// Nodes whose definition changed (added, removed or different).
    pub(super) seeds: BTreeSet<Slot>,
    /// Slots allocated by the update: their kept values are stale.
    pub(super) allocated: Vec<Slot>,
    /// Keys defined before and not after.
    pub(super) undefined: Vec<K>,
    /// Seeds, and the old predecessors of changed definitions: slots that
    /// may be neither defined nor read any more ([`KeptGraph::sweep`]).
    candidates: BTreeSet<Slot>,
    /// Seeds that were defined before the update.
    pub(super) was_defined: BTreeSet<Slot>,
}

pub(super) struct KeptGraph<K, I, E> {
    pub(super) index: DenseIndex<K>,
    nodes: Vec<Node<I, E>>,
    defined: usize,
}

impl<K, I, E> Default for KeptGraph<K, I, E> {
    fn default() -> Self {
        Self {
            index: DenseIndex::default(),
            nodes: Vec::new(),
            defined: 0,
        }
    }
}

impl<K, I, E> KeptGraph<K, I, E>
where
    K: Clone + Eq + Hash,
    I: Clone + Eq,
    E: Clone + Eq,
{
    fn slot(&mut self, key: &K, allocated: &mut Vec<Slot>) -> Slot {
        let (slot, new) = self.index.insert(key);
        if new {
            allocated.push(slot);
        }
        let at = slot as usize;
        if self.nodes.len() <= at {
            self.nodes.resize_with(at + 1, Node::default);
        }
        slot
    }

    /// Applies definition changes (`None`: the key is no longer defined);
    /// a key listed twice takes the last.
    pub(super) fn apply(
        &mut self,
        changes: impl IntoIterator<Item = (K, Option<Definition<K, I, E>>)>,
    ) -> Applied<K> {
        let mut applied = Applied {
            seeds: BTreeSet::new(),
            allocated: Vec::new(),
            undefined: Vec::new(),
            candidates: BTreeSet::new(),
            was_defined: BTreeSet::new(),
        };
        for (key, next) in changes {
            if next.is_none() && self.index.get(&key).is_none() {
                continue;
            }
            let slot = self.slot(&key, &mut applied.allocated);
            let same = match (&self.nodes[slot as usize].defined, &next) {
                (None, None) => true,
                (Some((input, preds)), Some(next)) => {
                    *input == next.input
                        && preds.len() == next.preds.len()
                        && preds
                            .iter()
                            .zip(&next.preds)
                            .all(|((pred, edge), (key, next))| {
                                edge == next && self.index.key(*pred) == Some(key)
                            })
                }
                _ => false,
            };
            if same {
                continue;
            }
            if !applied.seeds.contains(&slot) && self.nodes[slot as usize].defined.is_some() {
                applied.was_defined.insert(slot);
            }
            applied.seeds.insert(slot);
            applied.candidates.insert(slot);
            if let Some((_, preds)) = self.nodes[slot as usize].defined.take() {
                self.defined -= 1;
                for (pred, _) in preds {
                    self.nodes[pred as usize].readers.remove(&slot);
                    applied.candidates.insert(pred);
                }
            }
            if let Some(next) = next {
                let preds = next
                    .preds
                    .into_iter()
                    .map(|(key, edge)| {
                        let pred = self.slot(&key, &mut applied.allocated);
                        self.nodes[pred as usize].readers.insert(slot);
                        (pred, edge)
                    })
                    .collect();
                self.nodes[slot as usize].defined = Some((next.input, preds));
                self.defined += 1;
            }
        }
        for slot in &applied.was_defined {
            if self.nodes[*slot as usize].defined.is_none()
                && let Some(key) = self.index.key(*slot)
            {
                applied.undefined.push(key.clone());
            }
        }
        applied
    }

    /// Frees the candidates that are neither defined nor read.
    pub(super) fn sweep(&mut self, applied: &Applied<K>) {
        for slot in &applied.candidates {
            let node = &self.nodes[*slot as usize];
            if node.defined.is_none() && node.readers.is_empty() {
                self.index.remove(*slot);
            }
        }
    }

    /// The forward closure of `seeds` over the reverse map.
    pub(super) fn cone(&self, seeds: &BTreeSet<Slot>) -> BTreeSet<Slot> {
        let mut cone = seeds.clone();
        let mut stack = seeds.iter().copied().collect::<Vec<_>>();
        while let Some(slot) = stack.pop() {
            for reader in &self.nodes[slot as usize].readers {
                if cone.insert(*reader) {
                    stack.push(*reader);
                }
            }
        }
        cone
    }

    pub(super) fn definition(&self, slot: Slot) -> Option<&(I, Vec<(Slot, E)>)> {
        self.nodes[slot as usize].defined.as_ref()
    }

    pub(super) fn readers(&self, slot: Slot) -> &BTreeSet<Slot> {
        &self.nodes[slot as usize].readers
    }

    pub(super) fn is_defined(&self, slot: Slot) -> bool {
        self.nodes
            .get(slot as usize)
            .is_some_and(|node| node.defined.is_some())
    }

    /// Defined nodes.
    pub(super) fn defined(&self) -> usize {
        self.defined
    }

    /// Every live slot, ascending.
    pub(super) fn slots(&self) -> BTreeSet<Slot> {
        self.index.slots().collect()
    }

    /// The defined keys whose definition differs from `next`'s, and the
    /// defined keys `next` lacks: the changes that turn this graph's
    /// definitions into `next`.
    pub(super) fn diff(&self, next: Vec<(K, Definition<K, I, E>)>) -> Vec<Change<K, I, E>> {
        let mut listed = BTreeSet::new();
        let mut changes = Vec::new();
        for (key, definition) in next {
            if let Some(slot) = self.index.get(&key) {
                listed.insert(slot);
                if let Some((input, preds)) = &self.nodes[slot as usize].defined
                    && *input == definition.input
                    && preds.len() == definition.preds.len()
                    && preds
                        .iter()
                        .zip(&definition.preds)
                        .all(|((pred, edge), (key, next))| {
                            edge == next && self.index.key(*pred) == Some(key)
                        })
                {
                    continue;
                }
            }
            changes.push((key, Some(definition)));
        }
        for slot in self.index.slots() {
            if !listed.contains(&slot)
                && self.is_defined(slot)
                && let Some(key) = self.index.key(slot)
            {
                changes.push((key.clone(), None));
            }
        }
        changes
    }
}
