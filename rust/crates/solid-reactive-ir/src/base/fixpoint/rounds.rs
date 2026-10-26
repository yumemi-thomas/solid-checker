//! K3: a bounded or non-monotone Jacobi loop, replayed from each node's
//! stored per-round values (lemma 4′ in the module docs).

use std::collections::BTreeMap;
use std::hash::Hash;

use super::graph::{Change, KeptGraph};
use super::{Slot, Solved, fresh_cone};

/// A node's trajectory: `(round, value)` at round 0 and at every round its
/// value changed. Its value at round r is the last entry at or before r.
type Trajectory<V> = Vec<(u32, V)>;

fn at<V>(trajectory: &Trajectory<V>, round: u32) -> Option<&V> {
    let after = trajectory.partition_point(|(changed, _)| *changed <= round);
    after.checked_sub(1).map(|index| &trajectory[index].1)
}

/// The loop `x⁰(n) = init(n)`, `xʳ(n) = step(n, xʳ⁻¹(n), xʳ⁻¹(preds))`,
/// run until a round changes nothing or `cap` rounds have run; the answer
/// is the last round's values. Steps see a node's predecessors in its
/// definition's order, an undefined one as `None`.
pub(crate) struct RoundReplay<K, I, V, E> {
    graph: KeptGraph<K, I, E>,
    trajectories: Vec<Trajectory<V>>,
    /// Each defined node's last change round, counted.
    last_changes: BTreeMap<u32, u32>,
    cap: u32,
    /// The last solve stopped on a round that changed nothing (not at the
    /// cap); a new solver has nothing to extend.
    converged: bool,
    /// Rounds the last solve ran.
    rounds: u32,
}

impl<K, I, V, E> RoundReplay<K, I, V, E> {
    /// A solver whose loop runs at most `cap` rounds (at least one).
    pub(crate) fn new(cap: u32) -> Self {
        assert!(cap >= 1, "a round loop runs at least one round");
        Self {
            graph: KeptGraph::default(),
            trajectories: Vec::new(),
            last_changes: BTreeMap::new(),
            cap,
            converged: false,
            rounds: 0,
        }
    }
}

/// The loop's step functions.
pub(crate) trait Step<K, I, V, E> {
    fn init(&self, key: &K, input: &I) -> V;
    fn step(&self, key: &K, input: &I, own: &V, preds: &[(&E, Option<&V>)]) -> V;
}

impl<K, I, V, E> RoundReplay<K, I, V, E>
where
    K: Clone + Eq + Hash,
    I: Clone + Eq,
    V: Clone + Eq,
    E: Clone + Eq,
{
    /// Applies definition changes (`None`: undefined) and re-solves. Solved
    /// fresh when the last solve stopped at the cap, or the cone is large.
    pub(crate) fn update(
        &mut self,
        changes: impl IntoIterator<Item = Change<K, I, E>>,
        rules: &impl Step<K, I, V, E>,
    ) -> Solved<K> {
        self.update_with(changes, rules, false)
    }

    /// [`Self::update`], solving every node when `force_fresh`.
    pub(crate) fn update_with(
        &mut self,
        changes: impl IntoIterator<Item = Change<K, I, E>>,
        rules: &impl Step<K, I, V, E>,
        force_fresh: bool,
    ) -> Solved<K> {
        let applied = self.graph.apply(changes);
        let capacity = self.graph.index.capacity();
        if self.trajectories.len() < capacity {
            self.trajectories.resize_with(capacity, Vec::new);
        }
        for slot in &applied.allocated {
            // A slot is freed only once undefined, and an undefined node's
            // trajectory is empty: a reused slot carries nothing stale.
            debug_assert!(self.trajectories[*slot as usize].is_empty());
        }
        let cone = self.graph.cone(&applied.seeds);
        let fresh =
            force_fresh || !self.converged || fresh_cone(cone.len(), self.graph.index.len());
        let cone = if fresh {
            self.last_changes.clear();
            self.graph.slots()
        } else {
            for slot in &cone {
                if let Some((last, _)) = self.trajectories[*slot as usize].last() {
                    let count = self
                        .last_changes
                        .get_mut(last)
                        .expect("a kept trajectory is counted");
                    *count -= 1;
                    if *count == 0 {
                        self.last_changes.remove(last);
                    }
                }
            }
            cone
        };
        let before = cone
            .iter()
            .map(|slot| {
                let defined = if applied.seeds.contains(slot) {
                    applied.was_defined.contains(slot)
                } else {
                    self.graph.is_defined(*slot)
                };
                if defined {
                    self.trajectories[*slot as usize]
                        .last()
                        .map(|(_, value)| value.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        if !cone.is_empty() {
            self.solve(&cone, rules, fresh);
        } else if fresh {
            // No node at all: nothing to extend, nothing to cap.
            self.converged = true;
            self.rounds = 0;
        }
        let mut changed = applied.undefined.clone();
        for (slot, before) in cone.iter().zip(before) {
            if !self.graph.is_defined(*slot) {
                continue;
            }
            let after = self.trajectories[*slot as usize]
                .last()
                .map(|(_, value)| value);
            if before.as_ref() != after
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

    fn solve(
        &mut self,
        cone: &std::collections::BTreeSet<Slot>,
        rules: &impl Step<K, I, V, E>,
        fresh: bool,
    ) {
        let outside_last = if fresh {
            0
        } else {
            self.last_changes.keys().next_back().copied().unwrap_or(0)
        };
        let slots = cone.iter().copied().collect::<Vec<_>>();
        let mut position = vec![u32::MAX; self.graph.index.capacity()];
        for (index, slot) in slots.iter().enumerate() {
            position[*slot as usize] = u32::try_from(index).expect("fewer than 2^32 nodes");
        }
        let key = |slot: Slot| {
            self.graph
                .index
                .key(slot)
                .expect("a live slot has a key")
                .clone()
        };
        let keys = slots.iter().map(|slot| key(*slot)).collect::<Vec<_>>();
        let mut current = slots
            .iter()
            .zip(&keys)
            .map(|(slot, key)| {
                self.graph
                    .definition(*slot)
                    .map(|(input, _)| rules.init(key, input))
            })
            .collect::<Vec<_>>();
        let mut trajectories = current
            .iter()
            .map(|value| value.iter().map(|value| (0, value.clone())).collect())
            .collect::<Vec<Trajectory<V>>>();
        let mut round = 0;
        let mut converged = false;
        while round < self.cap {
            round += 1;
            let mut next = Vec::with_capacity(slots.len());
            for (index, slot) in slots.iter().enumerate() {
                let Some((input, preds)) = self.graph.definition(*slot) else {
                    next.push(None);
                    continue;
                };
                let seen = preds
                    .iter()
                    .map(|(pred, edge)| {
                        let value = match position[*pred as usize] {
                            u32::MAX => at(&self.trajectories[*pred as usize], round - 1),
                            inside => current[inside as usize].as_ref(),
                        };
                        (edge, value)
                    })
                    .collect::<Vec<_>>();
                let own = current[index].as_ref().expect("a defined node has a value");
                next.push(Some(rules.step(&keys[index], input, own, &seen)));
            }
            let mut moved = false;
            for (index, value) in next.iter().enumerate() {
                if *value != current[index]
                    && let Some(value) = value
                {
                    moved = true;
                    trajectories[index].push((round, value.clone()));
                }
            }
            current = next;
            if !moved && round > outside_last {
                converged = true;
                break;
            }
        }
        self.converged = converged;
        self.rounds = round;
        for (slot, trajectory) in slots.iter().zip(trajectories) {
            if let Some((last, _)) = trajectory.last() {
                *self.last_changes.entry(*last).or_default() += 1;
            }
            self.trajectories[*slot as usize] = trajectory;
        }
    }

    /// The final value of a defined node.
    pub(crate) fn value(&self, key: &K) -> Option<&V> {
        let slot = self.graph.index.get(key)?;
        if !self.graph.is_defined(slot) {
            return None;
        }
        self.trajectories[slot as usize]
            .last()
            .map(|(_, value)| value)
    }

    /// A defined node's value at `round` (its final value past its last
    /// change).
    pub(crate) fn value_at(&self, key: &K, round: u32) -> Option<&V> {
        let slot = self.graph.index.get(key)?;
        if !self.graph.is_defined(slot) {
            return None;
        }
        at(&self.trajectories[slot as usize], round)
    }

    /// Whether the last solve stopped on a quiet round, not at the cap.
    pub(crate) fn converged(&self) -> bool {
        self.converged
    }

    /// The rounds the last solve ran.
    pub(crate) fn rounds(&self) -> u32 {
        self.rounds
    }
}
