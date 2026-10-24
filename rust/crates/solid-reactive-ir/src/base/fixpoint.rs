//! Project passes on kept state: the solvers an in-stage pass keeps between
//! builds, and how it publishes its output to memo readers (S7 design §1).
//!
//! The memo framework ([`super::memo`]) memoizes context-free answers whose
//! reads form a DAG. A project pass that folds every file's rows, or iterates
//! to a fixed point, is neither. Four kinds of computation cover those
//! passes, and each has a solver here whose nodes, edges and per-node values
//! survive between builds:
//!
//! - **K1, keyed fold** ([`KeyedFold`]): a per-key fold over per-file rows in
//!   path order (first wins, last wins, union, ...). A changed file refolds
//!   exactly the keys its old or new rows name (study lemma 1).
//! - **K2, monotone cone** ([`Monotone`]): a least fixed point over a
//!   join-semilattice. The seeds' forward cone is reset to its direct values
//!   and solved by a worklist; values outside it are held (study lemma 4).
//! - **K3, round replay** ([`RoundReplay`]): a bounded or non-monotone Jacobi
//!   loop. Each node keeps its per-round values; the cone's trajectories are
//!   recomputed against the stored ones (lemma 4′ below).
//! - **K4, ordered fold** ([`OrderedFold`]): a sequential pass whose items read
//!   keys that earlier items wrote. Dirty items replay in position order
//!   (lemma 4″ below).
//!
//! **Node keys.** A node is named by a key that survives the build: a
//! [`NodeKey`] (file and item ordinal, or file and span until S8) or any
//! other stable key, never an index into a per-build concatenation. A
//! [`DenseIndex`] maps keys to slots and reuses freed slots; slot order is
//! allocation history, so no solver lets it reach an answer. Order comes only
//! from keys: K1 folds in the files' current path rank ([`FileOrder`]), never
//! by [`FileId`] (an added file gets the next id but sorts at its path); K3
//! steps see a node's predecessors in the order its definition lists them;
//! K4 positions are totally ordered keys.
//!
//! **Seeds and the kept graph.** A node is defined by its direct value (or
//! input) and its ordered predecessor list; an update names the definitions
//! that changed, and those nodes are the seeds. An added or removed edge is a
//! changed definition of its target, so the forward closure of the seeds is
//! the same in the old graph, the new graph and their union: the only edges
//! that differ end at seeds.
//!
//! **Publication** ([`Published`], [`super::memo::derived`]). A solver's
//! output is a keyed table with a revision per key. An equal recomputed value
//! keeps its revision (early cutoff); a changed one invalidates the memos
//! that recorded a read of that key, as an index Δ does at
//! [`super::memo::Store::begin`].
//!
//! **Fresh-solve fallbacks** (study lemma 6). A cone above a quarter of the
//! nodes is solved fresh (same result, cheaper). So is a K1 or K4 whose
//! surviving files changed relative order, a K3 whose last build stopped at
//! its round cap, and every solver of a poisoned or new store.
//!
//! **Lemma 4′ (round replay).** In a Jacobi loop, node n's round-r value is a
//! deterministic function of n's input and the round-(r−1) values of n and
//! the nodes it reads. So n's trajectory (its value at every round) is a
//! function of its input and its predecessors' trajectories, and a node with
//! no seed among its ancestors keeps its trajectory. The incremental solve
//! recomputes the trajectories of the seeds' forward cone, reading stored
//! trajectories for predecessors outside it; past an outside node's last
//! stored round its value is its final value. The global stop round (the
//! first round in which no node changed) is then one past the latest change,
//! cone or outside. That holds only if the last build converged: a loop
//! stopped by its cap has no final values to extend, which is recorded and
//! forces a fresh solve. Property: `lemma_4_prime_*` in the tests.
//!
//! **Lemma 4″ (ordered fold).** Items run in a total order; each reads keys
//! (each read recorded with the value it saw) and writes keys, and a read sees
//! the last earlier write. An item whose input is unchanged and none of whose
//! read keys had a changed write at an earlier position sees the same values,
//! so by trace replay (study lemma 2) it performs the same reads and writes.
//! The incremental solve pops dirty items from a queue in position order; a
//! changed write dirties the key's readers up to its next writer, and a
//! dirtied item whose reads still see their recorded values is skipped. By
//! induction on position, every item's trace, hence the final state, equals
//! the fresh one. Property: `lemma_4_double_prime_*` in the tests.

// K1, K3 and K4 have no pass yet (S7a, S7c and S7e port onto them): their
// property tests are their only users until then.
#[cfg_attr(not(test), allow(dead_code))]
mod fold;
mod graph;
mod kept;
mod monotone;
#[cfg_attr(not(test), allow(dead_code))]
mod ordered;
#[cfg_attr(not(test), allow(dead_code))]
mod rounds;
#[cfg(test)]
mod tests;

use std::hash::Hash;

use solid_facts::core::Span;

use super::file_table::{FileId, ItemKind};
use crate::HashMap;

#[cfg_attr(not(test), allow(unused_imports))]
pub(crate) use fold::KeyedFold;
#[cfg(test)]
pub(crate) use graph::Change;
pub(crate) use graph::Definition;
pub(crate) use monotone::{Lattice, Monotone};
#[cfg_attr(not(test), allow(unused_imports))]
pub(crate) use ordered::{OrderedFold, Tracer};
#[cfg_attr(not(test), allow(unused_imports))]
pub(crate) use rounds::{RoundReplay, Step};

/// A node's slot in a [`DenseIndex`].
pub(crate) type Slot = u32;

/// A node that survives the build: an item of a file. No pass keys its
/// nodes by it yet (S7c and S7e will: summary and owner nodes).
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct NodeKey {
    pub(crate) file: FileId,
    pub(crate) item: ItemKey,
}

/// An item within its file.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum ItemKey {
    /// The item's ordinal in the file's item table (S3): stable when text
    /// before it is edited.
    Item(ItemKind, u32),
    /// The item's span, until S8 keys every cross-file answer by item.
    Span(Span),
}

impl super::memo::Canonical for NodeKey {}
impl super::memo::Canonical for ItemKey {}

/// Keys to dense slots. A removed key's slot is reused by the next insert.
#[derive(Clone, Debug)]
pub(crate) struct DenseIndex<K> {
    slots: HashMap<K, Slot>,
    keys: Vec<Option<K>>,
    free: Vec<Slot>,
}

impl<K> Default for DenseIndex<K> {
    fn default() -> Self {
        Self {
            slots: HashMap::default(),
            keys: Vec::new(),
            free: Vec::new(),
        }
    }
}

impl<K: Clone + Eq + Hash> DenseIndex<K> {
    pub(crate) fn get(&self, key: &K) -> Option<Slot> {
        self.slots.get(key).copied()
    }

    /// The slot of `key`, allocated if it has none; `true` when new.
    pub(crate) fn insert(&mut self, key: &K) -> (Slot, bool) {
        if let Some(slot) = self.slots.get(key) {
            return (*slot, false);
        }
        let slot = match self.free.pop() {
            Some(slot) => {
                self.keys[slot as usize] = Some(key.clone());
                slot
            }
            None => {
                self.keys.push(Some(key.clone()));
                Slot::try_from(self.keys.len() - 1).expect("fewer than 2^32 nodes")
            }
        };
        self.slots.insert(key.clone(), slot);
        (slot, true)
    }

    /// Frees `slot`; its key no longer has one.
    pub(crate) fn remove(&mut self, slot: Slot) {
        if let Some(key) = self.keys.get_mut(slot as usize).and_then(Option::take) {
            self.slots.remove(&key);
            self.free.push(slot);
        }
    }

    pub(crate) fn key(&self, slot: Slot) -> Option<&K> {
        self.keys.get(slot as usize).and_then(Option::as_ref)
    }

    /// One past the highest slot ever allocated.
    pub(crate) fn capacity(&self) -> usize {
        self.keys.len()
    }

    /// Live slots.
    pub(crate) fn len(&self) -> usize {
        self.slots.len()
    }

    /// Live slots, ascending.
    pub(crate) fn slots(&self) -> impl Iterator<Item = Slot> + '_ {
        self.keys
            .iter()
            .enumerate()
            .filter(|(_, key)| key.is_some())
            .map(|(slot, _)| Slot::try_from(slot).expect("fewer than 2^32 nodes"))
    }
}

/// The analyzed files' current order: each id's rank in the build's list
/// (path order: the sources are path-sorted).
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct FileOrder {
    ids: Vec<FileId>,
    ranks: HashMap<FileId, u32>,
}

#[cfg_attr(not(test), allow(dead_code))]
impl FileOrder {
    pub(crate) fn new(ids: &[FileId]) -> Self {
        let ranks = ids
            .iter()
            .enumerate()
            .map(|(rank, id)| (*id, u32::try_from(rank).expect("fewer than 2^32 files")))
            .collect();
        Self {
            ids: ids.to_vec(),
            ranks,
        }
    }

    pub(crate) fn rank(&self, id: FileId) -> Option<u32> {
        self.ranks.get(&id).copied()
    }

    /// Whether the files both orders list keep their relative order: a
    /// fold kept under `previous` is then exact for every key no added or
    /// removed file names.
    pub(crate) fn keeps_relative_order(&self, previous: &Self) -> bool {
        let surviving = |order: &Self, other: &Self| {
            order
                .ids
                .iter()
                .filter(|id| other.ranks.contains_key(id))
                .copied()
                .collect::<Vec<_>>()
        };
        surviving(self, previous) == surviving(previous, self)
    }
}

/// What one solver update did.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Solved<K> {
    /// Keys whose output differs from the last build's (added, removed or
    /// changed), each once.
    pub(crate) changed: Vec<K>,
    /// Nodes (K1: keys; K4: items) recomputed.
    pub(crate) recomputed: usize,
    /// Solved from scratch ([`fresh_cone`], or a fallback).
    pub(crate) fresh: bool,
}

/// Whether a cone of `cone` out of `nodes` nodes is solved fresh instead:
/// above a quarter, a fresh solve gives the same result for less work.
pub(crate) fn fresh_cone(cone: usize, nodes: usize) -> bool {
    cone.saturating_mul(4) > nodes
}

/// A solver's output as memo readers see it: a value per key, each with the
/// revision in which it last changed. Keys are interned on first read
/// (`super::memo::derived`). No memo reads one yet (S7b's will), so the
/// reading half is used by tests only.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) struct Published<K, V> {
    values: HashMap<K, (V, u32)>,
    /// The revision of the last [`Self::publish`].
    published_at: Option<u32>,
    ids: HashMap<K, u32>,
    keys: HashMap<u32, K>,
    next: u32,
}

impl<K, V> Default for Published<K, V> {
    fn default() -> Self {
        Self {
            values: HashMap::default(),
            published_at: None,
            ids: HashMap::default(),
            keys: HashMap::default(),
            next: 0,
        }
    }
}

#[cfg_attr(not(test), allow(dead_code))]
impl<K: Clone + Eq + Hash, V: Clone + Eq> Published<K, V> {
    /// Applies `changes` (`None`: the key has no value any more) as of
    /// `revision`, and returns the interned ids of the keys whose value
    /// differs: the keys a reader recorded. An equal value keeps its
    /// revision.
    pub(crate) fn publish(
        &mut self,
        revision: u32,
        changes: impl IntoIterator<Item = (K, Option<V>)>,
    ) -> Vec<u32> {
        self.published_at = Some(revision);
        let mut moved = Vec::new();
        for (key, value) in changes {
            let differs = match value {
                Some(value) => match self.values.get_mut(&key) {
                    Some(entry) if entry.0 == value => false,
                    Some(entry) => {
                        *entry = (value, revision);
                        true
                    }
                    None => {
                        self.values.insert(key.clone(), (value, revision));
                        true
                    }
                },
                None => self.values.remove(&key).is_some(),
            };
            if differs && let Some(id) = self.ids.get(&key) {
                moved.push(*id);
            }
        }
        moved.sort_unstable();
        moved.dedup();
        moved
    }

    pub(crate) fn published_at(&self) -> Option<u32> {
        self.published_at
    }

    pub(crate) fn get(&self, key: &K) -> Option<&V> {
        self.values.get(key).map(|(value, _)| value)
    }

    /// The revision in which `key`'s value last changed (0: no value).
    pub(crate) fn revision(&self, key: &K) -> u32 {
        self.values.get(key).map_or(0, |(_, revision)| *revision)
    }

    /// The interned id of `key`.
    pub(crate) fn intern(&mut self, key: &K) -> u32 {
        if let Some(id) = self.ids.get(key) {
            return *id;
        }
        let id = self.next;
        self.next = self.next.checked_add(1).expect("fewer than 2^32 keys");
        self.ids.insert(key.clone(), id);
        self.keys.insert(id, key.clone());
        id
    }

    pub(crate) fn id(&self, key: &K) -> Option<u32> {
        self.ids.get(key).copied()
    }

    /// Forgets an interned id no reader records any more.
    pub(crate) fn release(&mut self, id: u32) {
        if let Some(key) = self.keys.remove(&id) {
            self.ids.remove(&key);
        }
    }
}
