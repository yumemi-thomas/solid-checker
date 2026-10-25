//! K2: a least fixed point over a join-semilattice, kept between builds and
//! re-solved on the seeds' forward cone (study lemma 4).

use std::collections::BTreeSet;
use std::hash::Hash;

use super::graph::{Change, Definition, KeptGraph};
use super::{Slot, Solved, fresh_cone};

/// A join-semilattice of finite height.
pub(crate) trait Lattice: Clone + Eq + std::fmt::Debug {
    fn bottom() -> Self;
    /// Joins `other` into `self`.
    fn join(&mut self, other: &Self);
}

/// The least solution of `value(n) = direct(n) ⊔ ⨆ transfer(e, value(p))`
/// over each predecessor `(p, e)` of `n`, for a monotone `transfer`. An
/// undefined predecessor is bottom.
///
/// An update resets the forward cone of the changed definitions to their
/// direct values and runs a worklist on it, every value outside held. A node
/// outside the cone has no changed definition among its ancestors, so its
/// least value is the one kept (lemma 4); inside, the worklist climbs from
/// below to the least solution given the held values.
pub(crate) struct Monotone<K, V, E> {
    graph: KeptGraph<K, V, E>,
    values: Vec<V>,
}

impl<K, V, E> Default for Monotone<K, V, E> {
    fn default() -> Self {
        Self {
            graph: KeptGraph::default(),
            values: Vec::new(),
        }
    }
}

impl<K, V, E> Monotone<K, V, E>
where
    K: Clone + Eq + Hash,
    V: Lattice,
    E: Clone + Eq,
{
    /// Applies definition changes (`None`: undefined) and re-solves.
    pub(crate) fn update(
        &mut self,
        changes: impl IntoIterator<Item = Change<K, V, E>>,
        transfer: &impl Fn(&E, &V) -> V,
    ) -> Solved<K> {
        self.update_with(changes, transfer, false)
    }

    /// Replaces every definition with `definitions` (a key listed twice
    /// takes the last) and re-solves: the seeds are the definitions that
    /// differ from the kept ones.
    pub(crate) fn replace(
        &mut self,
        definitions: Vec<(K, Definition<K, V, E>)>,
        transfer: &impl Fn(&E, &V) -> V,
    ) -> Solved<K> {
        let changes = self.graph.diff(definitions);
        self.update(changes, transfer)
    }

    /// [`Self::update`], solving every node whatever the cone.
    pub(crate) fn update_with(
        &mut self,
        changes: impl IntoIterator<Item = Change<K, V, E>>,
        transfer: &impl Fn(&E, &V) -> V,
        force_fresh: bool,
    ) -> Solved<K> {
        let applied = self.graph.apply(changes);
        let capacity = self.graph.index.capacity();
        if self.values.len() < capacity {
            self.values.resize(capacity, V::bottom());
        }
        for slot in &applied.allocated {
            self.values[*slot as usize] = V::bottom();
        }
        // The cone holds the seeds, so seeds enough for a fresh solve need no
        // cone walk (every slot is a seed in a first build).
        let nodes = self.graph.index.len();
        let mut fresh = force_fresh || fresh_cone(applied.seeds.len(), nodes);
        let cone = if fresh {
            self.graph.slots()
        } else {
            let cone = self.graph.cone(&applied.seeds);
            fresh = fresh_cone(cone.len(), nodes);
            if fresh { self.graph.slots() } else { cone }
        };
        let before = cone
            .iter()
            .map(|slot| {
                let defined = if applied.seeds.contains(slot) {
                    applied.was_defined.contains(slot)
                } else {
                    self.graph.is_defined(*slot)
                };
                defined.then(|| self.values[*slot as usize].clone())
            })
            .collect::<Vec<_>>();
        self.solve(&cone, transfer);
        let mut changed = applied.undefined.clone();
        for (slot, before) in cone.iter().zip(before) {
            if !self.graph.is_defined(*slot) {
                continue;
            }
            if before.as_ref() != Some(&self.values[*slot as usize])
                && let Some(key) = self.graph.index.key(*slot)
            {
                changed.push(key.clone());
            }
        }
        self.graph.sweep(&applied);
        Solved {
            changed,
            recomputed: cone.len(),
            fresh,
        }
    }

    fn direct(&self, slot: Slot) -> V {
        self.graph
            .definition(slot)
            .map_or_else(V::bottom, |(direct, _)| direct.clone())
    }

    fn solve(&mut self, cone: &BTreeSet<Slot>, transfer: &impl Fn(&E, &V) -> V) {
        // Membership by slot: the cone and the queue as flags over the
        // values' slots, in the order the sets gave.
        let mut in_cone = vec![false; self.values.len()];
        for slot in cone {
            self.values[*slot as usize] = self.direct(*slot);
            in_cone[*slot as usize] = true;
        }
        let mut queued = in_cone.clone();
        let mut queue = cone
            .iter()
            .copied()
            .collect::<std::collections::VecDeque<_>>();
        while let Some(slot) = queue.pop_front() {
            queued[slot as usize] = false;
            let Some((direct, preds)) = self.graph.definition(slot) else {
                continue;
            };
            let mut next = direct.clone();
            for (pred, edge) in preds {
                next.join(&transfer(edge, &self.values[*pred as usize]));
            }
            if next == self.values[slot as usize] {
                continue;
            }
            self.values[slot as usize] = next;
            for reader in self.graph.readers(slot) {
                let reader_index = *reader as usize;
                if in_cone[reader_index] && !queued[reader_index] {
                    queued[reader_index] = true;
                    queue.push_back(*reader);
                }
            }
        }
    }

    /// The value of a defined node.
    pub(crate) fn value(&self, key: &K) -> Option<&V> {
        let slot = self.graph.index.get(key)?;
        self.graph
            .is_defined(slot)
            .then(|| &self.values[slot as usize])
    }

    /// Every defined node with its value, by slot (allocation order: not an
    /// order any answer may depend on).
    pub(crate) fn values(&self) -> impl Iterator<Item = (&K, &V)> + '_ {
        self.graph.index.slots().filter_map(move |slot| {
            self.graph
                .is_defined(slot)
                .then(|| self.graph.index.key(slot))
                .flatten()
                .map(|key| (key, &self.values[slot as usize]))
        })
    }

    /// Defined nodes.
    pub(crate) fn len(&self) -> usize {
        self.graph.defined()
    }
}
