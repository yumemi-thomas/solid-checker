//! The immutable canonical symbol representation retained between generations
//! (Go: `symbol_store.go`). Chunks not touched by an exact symbol delta are
//! shared; changed chunks are copied before patching.

use super::intern::SymbolHandleSet;
use crate::model::SymbolId;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::sync::Arc;
use typefacts::SymbolFact;

pub const SYMBOL_FACT_CHUNK_SIZE: usize = 256;

#[derive(Clone, Debug, Default)]
pub struct SymbolFactStore {
    pub chunks: Vec<Arc<[SymbolFact]>>,
    length: usize,
}

fn chunked(facts: Vec<SymbolFact>) -> Vec<Arc<[SymbolFact]>> {
    facts
        .chunks(SYMBOL_FACT_CHUNK_SIZE)
        .map(|chunk| Arc::from(chunk.to_vec()))
        .collect()
}

impl SymbolFactStore {
    /// A store over ID-ordered facts (Go: `newSymbolFactStore`).
    #[must_use]
    pub fn new(facts: Vec<SymbolFact>) -> Self {
        let length = facts.len();
        Self {
            chunks: chunked(facts),
            length,
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.length
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.length == 0
    }

    pub fn range(&self, mut visit: impl FnMut(&SymbolFact)) {
        for chunk in &self.chunks {
            chunk.iter().for_each(&mut visit);
        }
    }

    /// Every fact in canonical order.
    #[must_use]
    pub fn facts(&self) -> Vec<SymbolFact> {
        let mut facts = Vec::with_capacity(self.length);
        self.range(|fact| facts.push(fact.clone()));
        facts
    }

    #[must_use]
    pub fn get(&self, id: &str) -> Option<&SymbolFact> {
        if self.chunks.is_empty() {
            return None;
        }
        let chunk = &self.chunks[self.chunk_for(id)];
        let index = chunk.partition_point(|fact| fact.id.as_ref() < id);
        chunk.get(index).filter(|fact| fact.id.as_ref() == id)
    }

    fn chunk_for(&self, id: &str) -> usize {
        let index = self
            .chunks
            .partition_point(|chunk| chunk.last().is_some_and(|last| last.id.as_ref() < id));
        if index == self.chunks.len() {
            self.chunks.len() - 1
        } else {
            index
        }
    }

    /// Returns a new immutable generation. `present` is the complete current
    /// symbol universe; `patches` holds current rows for every added or
    /// changed symbol. The integer counts chunks shared with the preceding
    /// store, the list names removed symbols (ordered by id), and the flag says
    /// the result covers exactly `present`.
    #[must_use]
    pub fn patch(
        &self,
        present: &SymbolHandleSet,
        patches: &HashMap<SymbolId, SymbolFact>,
        removal_candidates: &HashSet<SymbolId>,
    ) -> (Self, usize, Vec<SymbolId>, bool) {
        if self.chunks.is_empty() {
            let mut facts: Vec<SymbolFact> = patches.values().cloned().collect();
            facts.sort_by(|left, right| left.id.cmp(&right.id));
            let store = Self::new(facts);
            let complete = store.len() == present.len();
            return (store, 0, Vec::new(), complete);
        }

        let mut additions: BTreeMap<usize, Vec<SymbolFact>> = BTreeMap::new();
        let mut affected: BTreeSet<usize> = BTreeSet::new();
        for (id, fact) in patches {
            let chunk_index = self.chunk_for(id);
            if self.get(id).is_none() {
                additions.entry(chunk_index).or_default().push(fact.clone());
            }
            affected.insert(chunk_index);
        }
        let mut removed: Vec<SymbolId> = Vec::new();
        let mut removed_set: HashSet<&str> = HashSet::new();
        let mut candidates: Vec<&SymbolId> = removal_candidates.iter().collect();
        candidates.sort();
        for id in candidates {
            if present.contains_id(id) || self.get(id).is_none() {
                continue;
            }
            removed.push(id.clone());
            removed_set.insert(id);
            affected.insert(self.chunk_for(id));
        }

        let mut next = Self::default();
        let mut next_chunks: Vec<Arc<[SymbolFact]>> = Vec::with_capacity(self.chunks.len());
        let mut shared = 0;
        for (chunk_index, chunk) in self.chunks.iter().enumerate() {
            if !affected.contains(&chunk_index) {
                next_chunks.push(Arc::clone(chunk));
                next.length += chunk.len();
                shared += 1;
                continue;
            }
            let mut rows: Vec<SymbolFact> =
                Vec::with_capacity(chunk.len() + additions.get(&chunk_index).map_or(0, Vec::len));
            for fact in chunk.iter() {
                if removed_set.contains(fact.id.as_ref()) {
                    continue;
                }
                rows.push(patches.get(&fact.id).unwrap_or(fact).clone());
            }
            if let Some(added) = additions.remove(&chunk_index) {
                rows.extend(added);
            }
            rows.sort_by(|left, right| left.id.cmp(&right.id));
            next.length += rows.len();
            next_chunks.extend(chunked(rows));
        }
        next.chunks = next_chunks;
        let complete = next.len() == present.len();
        (next, shared, removed, complete)
    }

    /// Replaces rows in a proven-stable symbol universe. It refuses additions
    /// so the caller's fixed-point proof cannot accidentally publish an
    /// incomplete store (Go: `PatchExisting`).
    #[must_use]
    pub fn patch_existing(&self, patches: &HashMap<SymbolId, SymbolFact>) -> Option<(Self, usize)> {
        let mut affected: BTreeSet<usize> = BTreeSet::new();
        for id in patches.keys() {
            self.get(id)?;
            affected.insert(self.chunk_for(id));
        }
        let mut next = Self {
            chunks: Vec::with_capacity(self.chunks.len()),
            length: self.length,
        };
        let mut shared = 0;
        for (chunk_index, chunk) in self.chunks.iter().enumerate() {
            if !affected.contains(&chunk_index) {
                next.chunks.push(Arc::clone(chunk));
                shared += 1;
                continue;
            }
            let rows: Vec<SymbolFact> = chunk
                .iter()
                .map(|fact| patches.get(&fact.id).unwrap_or(fact).clone())
                .collect();
            next.chunks.push(Arc::from(rows));
        }
        Some((next, shared))
    }
}
