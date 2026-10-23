//! Symbol closure: draining the worklist of symbols a demand set reaches to a
//! fixed point, then ordering the result canonically (Go: `symbol_closure.go`).
//! Alias and declaration facts come from the backend; reference lists come
//! from the batch capability and, where the backend reports an exact delta,
//! are patched into the previous generation's immutable chunks rather than
//! rebuilt.
//!
//! Deviation from Go: the per-chunk worker goroutines of `closeSymbols` are
//! sequential here. They only filled index-disjoint slots from a read-only
//! cache, so the result is identical.

use super::intern::{ChangedSymbolSet, SharedInterner, SymbolHandleSet};
use super::store::SymbolFactStore;
use crate::backend::{BackendError, ClosureBackend};
use crate::context::Context;
use crate::model::{SymbolId, blank_symbol, durable_symbol_id};
use crate::trace::SharedTrace;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;
use typefacts::producer::ChangedClosure;
use typefacts::{EntityFact, Location, SymbolFact, TypeDescriptor};

/// Accumulates one generation's symbol closure: the queue of symbols reached
/// from the demanded entities, the facts resolved for them, and the retained
/// state carried over from the preceding generation (Go: `closureBuilder`).
pub struct ClosureBuilder<'a> {
    pub backend: &'a mut dyn ClosureBackend,
    pub trace: SharedTrace,
    pub entities: HashMap<Location, EntityFact>,
    /// Assigns symbol ids their session-stable dense handles; `symbol_queue`
    /// and `queue_handles` grow in lockstep, so the fact built for queue
    /// position `i` carries handle `queue_handles[i]`.
    pub interner: SharedInterner,
    pub symbol_queue: Vec<SymbolId>,
    pub queue_handles: Vec<i32>,
    pub symbol_seen: SymbolHandleSet,
    /// The symbols whose reference lists this generation must carry: those
    /// reached by a demand that asked for references. Symbols reached only to
    /// be classified stay out and get no list. Full tier propagates along
    /// alias-target edges, because a caller canonicalizes before fanning out
    /// on references.
    pub full_tier: SymbolHandleSet,
    pub descriptors: HashMap<SymbolId, Arc<TypeDescriptor>>,
    /// `None` is Go's nil map: a first build, with nothing to compare against.
    pub cached_references: Option<HashMap<SymbolId, Vec<Location>>>,
    pub cached_canonical_store: Option<Arc<SymbolFactStore>>,
    pub invalidated_symbols: HashSet<SymbolId>,
    pub closed_symbol_store: Option<Arc<SymbolFactStore>>,
    pub closed_references: Option<HashMap<SymbolId, Vec<Location>>>,
    pub cached_symbol_hits: usize,
    pub recomputed_symbol_facts: usize,
    pub cached_reference_hits: usize,
    pub recomputed_references: usize,
    pub patched_symbol_rows: usize,
    pub shared_symbol_chunks: usize,
    pub changed_symbols: ChangedSymbolSet,
    pub removed_symbol_candidates: HashSet<SymbolId>,
    pub removed_symbol_ids: Vec<SymbolId>,
    pub reference_changes_exact: bool,
    pub stable_universe_patch: bool,
}

fn cancelled(ctx: &Context) -> Result<(), BackendError> {
    ctx.err()
        .map_or(Ok(()), |reason| Err(BackendError(reason.to_owned())))
}

impl<'a> ClosureBuilder<'a> {
    /// A builder with empty working sets over `interner`.
    #[must_use]
    pub fn new(
        backend: &'a mut dyn ClosureBackend,
        interner: &SharedInterner,
        trace: SharedTrace,
    ) -> Self {
        Self {
            backend,
            trace,
            entities: HashMap::new(),
            interner: Rc::clone(interner),
            symbol_queue: Vec::new(),
            queue_handles: Vec::new(),
            symbol_seen: SymbolHandleSet::new(interner, Vec::new()),
            full_tier: SymbolHandleSet::new(interner, Vec::new()),
            descriptors: HashMap::new(),
            cached_references: None,
            cached_canonical_store: None,
            invalidated_symbols: HashSet::new(),
            closed_symbol_store: None,
            closed_references: None,
            cached_symbol_hits: 0,
            recomputed_symbol_facts: 0,
            cached_reference_hits: 0,
            recomputed_references: 0,
            patched_symbol_rows: 0,
            shared_symbol_chunks: 0,
            changed_symbols: ChangedSymbolSet::new(interner),
            removed_symbol_candidates: HashSet::new(),
            removed_symbol_ids: Vec::new(),
            reference_changes_exact: false,
            stable_universe_patch: false,
        }
    }

    /// What the transport manifest reads off a finished builder.
    #[must_use]
    pub fn changed_closure(&self) -> ChangedClosure {
        ChangedClosure {
            reference_changes_exact: self.reference_changes_exact,
            changed_symbols: self.changed_symbols.ids.clone(),
        }
    }

    /// The entity row for `location`, created on first use (Go: `entity`).
    pub fn entity(&mut self, location: &Location) -> &mut EntityFact {
        self.entities
            .entry(location.clone())
            .or_insert_with(|| crate::model::blank_entity(location.clone()))
    }

    pub fn enqueue_symbol(&mut self, id: &SymbolId) {
        if id.is_empty() {
            return;
        }
        let handle = self.interner.borrow_mut().handle(id);
        if !self.symbol_seen.add(handle) {
            return;
        }
        self.symbol_queue.push(id.clone());
        self.queue_handles.push(handle);
    }

    fn cached_fact(&self, id: &str) -> Option<SymbolFact> {
        if self.invalidated_symbols.contains(id) {
            return None;
        }
        let fact = self.cached_canonical_store.as_deref()?.get(id)?;
        if !durable_symbol_id(&fact.id)
            || !durable_symbol_id(&fact.alias_target)
            || fact.declarations.is_empty()
        {
            return None;
        }
        Some(fact.clone())
    }

    /// A proof-gated specialization for generations whose raw reachability and
    /// reference-tier seeds are unchanged. Re-resolving every invalidated
    /// reached fact proves that its one outgoing alias edge is unchanged;
    /// together those predicates prove that both alias fixed points are
    /// identical to the preceding generation (Go: `patchStableSymbolUniverse`).
    pub fn patch_stable_symbol_universe(
        &mut self,
        ctx: &Context,
        invalidated: &HashSet<SymbolId>,
        memo_complete: bool,
        expanded_full_tier: Option<&[bool]>,
    ) -> Result<bool, BackendError> {
        let Some(previous) = self.cached_canonical_store.clone() else {
            return Ok(false);
        };
        let Some(expanded_full_tier) = expanded_full_tier else {
            return Ok(false);
        };
        if previous.is_empty() || self.cached_references.is_none() || !memo_complete {
            return Ok(false);
        }

        let mut patches: HashMap<SymbolId, SymbolFact> = HashMap::with_capacity(invalidated.len());
        let mut ordered: Vec<&SymbolId> = invalidated.iter().collect();
        ordered.sort();
        for id in ordered {
            cancelled(ctx)?;
            let Some(retained) = previous.get(id) else {
                return Ok(false);
            };
            let mut current = blank_symbol(id.clone());
            current.references = Arc::clone(&retained.references);
            if let Some(target) = self.backend.resolve_alias(ctx, id)? {
                current.alias_target = target;
            }
            if current.alias_target != retained.alias_target {
                return Ok(false);
            }
            if let Some(declarations) = self.backend.declarations(ctx, id)? {
                current.declarations = declarations.into();
            }
            patches.insert(id.clone(), current);
            self.changed_symbols.add(id);
        }

        let (changed_references, exact) = self.backend.changed_references(ctx)?;
        if !exact {
            return Ok(false);
        }
        let mut refresh: Vec<SymbolId> = Vec::with_capacity(changed_references.len());
        for id in &changed_references {
            if !self
                .cached_references
                .as_ref()
                .is_some_and(|cached| cached.contains_key(id))
            {
                continue;
            }
            refresh.push(id.clone());
            self.changed_symbols.add(id);
        }
        let references = self.backend.references_batch(ctx, &refresh)?;
        for id in &refresh {
            let fact = match patches.get(id) {
                Some(fact) => fact.clone(),
                None => match previous.get(id) {
                    Some(fact) => fact.clone(),
                    None => return Ok(false),
                },
            };
            let mut fact = fact;
            fact.references = references.get(id).cloned().unwrap_or_default().into();
            patches.insert(id.clone(), fact);
        }
        patches.retain(|id, fact| previous.get(id).is_none_or(|retained| fact != retained));
        let Some((next, shared)) = previous.patch_existing(&patches) else {
            return Ok(false);
        };

        self.full_tier.replace_members(expanded_full_tier);
        if let Some(cached) = &mut self.cached_references {
            for id in &refresh {
                cached.insert(id.clone(), references.get(id).cloned().unwrap_or_default());
            }
        }
        self.closed_references = self.cached_references.take();
        self.closed_symbol_store = Some(Arc::new(next));
        self.reference_changes_exact = true;
        self.stable_universe_patch = true;
        self.recomputed_symbol_facts = invalidated.len();
        self.cached_symbol_hits = previous.len().saturating_sub(self.recomputed_symbol_facts);
        self.recomputed_references = refresh.len();
        self.cached_reference_hits = self
            .closed_references
            .as_ref()
            .map_or(0, HashMap::len)
            .saturating_sub(refresh.len());
        self.patched_symbol_rows = patches.len();
        self.shared_symbol_chunks = shared;
        Ok(true)
    }

    /// Drains the worklist to a fixed point: every reached symbol gets its
    /// alias target (enqueueing it), declarations, and reference list.
    /// Termination is by construction: a symbol enters the queue at most once
    /// and the generation's symbol universe is finite. Afterwards, full tier
    /// propagates along alias-target edges to its own fixed point.
    ///
    /// Returns `None` when the retained-store patch path answered instead; the
    /// result is then `closed_symbol_store` (Go: `closeSymbols`).
    pub fn close_symbols(
        &mut self,
        ctx: &Context,
    ) -> Result<Option<Vec<SymbolFact>>, BackendError> {
        let started = Instant::now();
        let initial = self.symbol_queue.len();
        let mut facts: Vec<SymbolFact> = Vec::with_capacity(initial);
        let mut cached = vec![false; initial];
        for (index, id) in self.symbol_queue.clone().into_iter().enumerate() {
            if let Some(retained) = self.cached_fact(&id) {
                let mut fact = blank_symbol(id);
                fact.alias_target = retained.alias_target;
                fact.declarations = retained.declarations;
                facts.push(fact);
                cached[index] = true;
            } else {
                facts.push(blank_symbol(id));
            }
        }
        for (index, was_cached) in cached.into_iter().enumerate() {
            cancelled(ctx)?;
            if was_cached {
                let target = facts[index].alias_target.clone();
                self.enqueue_symbol(&target);
                self.cached_symbol_hits += 1;
                continue;
            }
            self.recomputed_symbol_facts += 1;
            let id = facts[index].id.clone();
            self.changed_symbols.add(&id);
            if let Some(target) = self.backend.resolve_alias(ctx, &id)? {
                facts[index].alias_target = target.clone();
                self.enqueue_symbol(&target);
            }
            if let Some(declarations) = self.backend.declarations(ctx, &id)? {
                facts[index].declarations = declarations.into();
            }
        }
        let mut index = initial;
        while index < self.symbol_queue.len() {
            cancelled(ctx)?;
            let id = self.symbol_queue[index].clone();
            index += 1;
            let mut fact = blank_symbol(id.clone());
            if let Some(retained) = self.cached_fact(&id) {
                fact.alias_target = retained.alias_target;
                fact.declarations = retained.declarations;
                let target = fact.alias_target.clone();
                self.enqueue_symbol(&target);
                self.cached_symbol_hits += 1;
                facts.push(fact);
                continue;
            }
            self.recomputed_symbol_facts += 1;
            self.changed_symbols.add(&id);
            if let Some(target) = self.backend.resolve_alias(ctx, &id)? {
                fact.alias_target = target.clone();
                self.enqueue_symbol(&target);
            }
            if let Some(declarations) = self.backend.declarations(ctx, &id)? {
                fact.declarations = declarations.into();
            }
            facts.push(fact);
        }
        let facts_duration = started.elapsed();
        let started = Instant::now();
        // Alias handles are resolved once so the fixpoint passes below are
        // pure bitset traffic. Every alias target was enqueued during the
        // drain, so the interner already knows it.
        let alias_handles: Vec<i32> = facts
            .iter()
            .map(|fact| {
                if fact.alias_target.is_empty() {
                    -1
                } else {
                    self.interner.borrow_mut().handle(&fact.alias_target)
                }
            })
            .collect();
        loop {
            let mut changed = false;
            for (position, target) in alias_handles.iter().enumerate() {
                if *target < 0 || !self.full_tier.contains(self.queue_handles[position]) {
                    continue;
                }
                if self.full_tier.add(*target) {
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        let full_tier_duration = started.elapsed();
        let started = Instant::now();
        if let Some(patched) = self.patch_canonical_symbol_store(ctx, &facts)? {
            let patched_len = patched.len();
            self.closed_symbol_store = Some(Arc::new(patched));
            let references_duration = started.elapsed();
            if let Some(trace) = &self.trace {
                trace.metrics(
                    "closeSymbols",
                    &[
                        ("factsNs", nanos(facts_duration)),
                        ("fullTierNs", nanos(full_tier_duration)),
                        ("referencesNs", nanos(references_duration)),
                        ("sortNs", 0),
                        ("initialSymbols", count(initial)),
                        ("symbols", count(patched_len)),
                        ("fullTier", count(self.full_tier.len())),
                        (
                            "references",
                            count(self.closed_references.as_ref().map_or(0, HashMap::len)),
                        ),
                    ],
                );
            }
            return Ok(None);
        }

        // fact_index maps a symbol's handle to its position in facts, offset by
        // one so the zeroed vector means absent.
        let size = self.interner.borrow().size();
        let mut fact_index = vec![0_i32; size];
        for (position, handle) in self.queue_handles.iter().enumerate().take(facts.len()) {
            fact_index[usize::try_from(*handle).unwrap_or(0)] =
                i32::try_from(position).unwrap_or(i32::MAX) + 1;
        }
        let fact_position = |interner: &SharedInterner, fact_index: &[i32], id: &str| {
            let handle = usize::try_from(interner.borrow().lookup(id)?).ok()?;
            let slot = *fact_index.get(handle)?;
            (slot != 0).then(|| usize::try_from(slot - 1).unwrap_or(0))
        };
        // Reference lists always come from the batch capability: one backend
        // lock instead of tens of thousands of round trips.
        let mut ids: Vec<SymbolId> = Vec::with_capacity(facts.len());
        for (position, fact) in facts.iter().enumerate() {
            if !fact.alias_target.is_empty() {
                continue;
            }
            // Only full-tier symbols carry reference lists; a symbol reached
            // solely to be classified gets none.
            if !self.full_tier.contains(self.queue_handles[position]) {
                continue;
            }
            ids.push(fact.id.clone());
        }

        let mut refresh = ids.clone();
        // A first build has nothing retained to compare against, so an exact
        // delta only means something once cached references exist.
        if self.cached_references.is_some() {
            let (changed_ids, exact) = self.backend.changed_references(ctx)?;
            if exact {
                let mut changed_set: HashSet<SymbolId> = HashSet::new();
                for id in &changed_ids {
                    changed_set.insert(id.clone());
                    self.changed_symbols.add(id);
                }
                self.reference_changes_exact = true;
                refresh = Vec::with_capacity(changed_set.len());
                for id in &ids {
                    let cached = self
                        .cached_references
                        .as_ref()
                        .and_then(|cached| cached.get(id));
                    match cached {
                        Some(list) if !changed_set.contains(id) => {
                            if let Some(position) = fact_position(&self.interner, &fact_index, id) {
                                facts[position].references = list.clone().into();
                            }
                            self.cached_reference_hits += 1;
                        }
                        _ => refresh.push(id.clone()),
                    }
                }
            }
        }

        let references = self.backend.references_batch(ctx, &refresh)?;
        self.recomputed_references += refresh.len();
        for id in &refresh {
            self.changed_symbols.add(id);
            // Absence is a known-empty list, not an unresolved cache miss.
            if let Some(position) = fact_position(&self.interner, &fact_index, id) {
                facts[position].references = references.get(id).cloned().unwrap_or_default().into();
            }
        }
        if self.reference_changes_exact {
            // The exact delta makes the preceding map itself reusable. Prune
            // departed and non-reference symbols and overwrite only the
            // refreshed rows instead of copying every retained list.
            let mut closed = self.cached_references.take().unwrap_or_default();
            closed.retain(|id, _| {
                fact_position(&self.interner, &fact_index, id)
                    .is_some_and(|position| facts[position].alias_target.is_empty())
                    && self.full_tier.contains_id(id)
            });
            for id in &refresh {
                if let Some(position) = fact_position(&self.interner, &fact_index, id) {
                    closed.insert(id.clone(), facts[position].references.to_vec());
                }
            }
            self.closed_references = Some(closed);
        } else {
            let mut closed = HashMap::with_capacity(ids.len());
            for id in &ids {
                if let Some(position) = fact_position(&self.interner, &fact_index, id) {
                    closed.insert(id.clone(), facts[position].references.to_vec());
                }
            }
            self.closed_references = Some(closed);
        }
        let references_duration = started.elapsed();
        let started = Instant::now();
        let facts = order_symbol_facts(
            facts,
            &mut fact_index,
            &self.interner,
            self.cached_canonical_store.as_deref(),
        );
        let sort_duration = started.elapsed();
        if let Some(trace) = &self.trace {
            trace.metrics(
                "closeSymbols",
                &[
                    ("factsNs", nanos(facts_duration)),
                    ("fullTierNs", nanos(full_tier_duration)),
                    ("referencesNs", nanos(references_duration)),
                    ("sortNs", nanos(sort_duration)),
                    ("initialSymbols", count(initial)),
                    ("symbols", count(facts.len())),
                    ("fullTier", count(self.full_tier.len())),
                    (
                        "references",
                        count(self.closed_references.as_ref().map_or(0, HashMap::len)),
                    ),
                ],
            );
        }
        Ok(Some(facts))
    }

    /// The retained-session fast path. It patches immutable chunks named by
    /// exact symbol/reference deltas and shares every untouched chunk with the
    /// preceding generation. An unexplained reference-tier change falls back
    /// to the general closure path (Go: `patchCanonicalSymbolStore`).
    pub fn patch_canonical_symbol_store(
        &mut self,
        ctx: &Context,
        facts: &[SymbolFact],
    ) -> Result<Option<SymbolFactStore>, BackendError> {
        let Some(previous) = self.cached_canonical_store.clone() else {
            return Ok(None);
        };
        if previous.is_empty() || self.cached_references.is_none() {
            return Ok(None);
        }
        for fact in facts {
            let eligible = fact.alias_target.is_empty() && self.full_tier.contains_id(&fact.id);
            let previously_eligible = self
                .cached_references
                .as_ref()
                .is_some_and(|cached| cached.contains_key(&fact.id));
            if eligible != previously_eligible && !self.changed_symbols.contains_id(&fact.id) {
                return Ok(None);
            }
        }

        let (changed_references, exact) = self.backend.changed_references(ctx)?;
        if !exact {
            return Ok(None);
        }
        self.reference_changes_exact = true;
        for id in &changed_references {
            self.changed_symbols.add(id);
        }

        let mut patches: HashMap<SymbolId, SymbolFact> = HashMap::new();
        for fact in facts {
            if !self.changed_symbols.contains_id(&fact.id) {
                continue;
            }
            let mut fact = fact.clone();
            if let Some(retained) = previous.get(&fact.id) {
                fact.references = Arc::clone(&retained.references);
            }
            patches.insert(fact.id.clone(), fact);
        }

        let mut refresh_set: HashSet<SymbolId> = HashSet::new();
        let changed_ids = self.changed_symbols.ids.clone();
        for id in &changed_ids {
            let fact = match patches.get(id) {
                Some(fact) => fact.clone(),
                None => match previous.get(id) {
                    Some(fact) => fact.clone(),
                    None => continue,
                },
            };
            let eligible = fact.alias_target.is_empty() && self.full_tier.contains_id(id);
            if !eligible {
                let mut fact = fact;
                fact.references = Arc::from(Vec::new());
                patches.insert(id.clone(), fact);
                if let Some(cached) = &mut self.cached_references {
                    cached.remove(id);
                }
                continue;
            }
            if !self
                .cached_references
                .as_ref()
                .is_some_and(|cached| cached.contains_key(id))
            {
                refresh_set.insert(id.clone());
            }
        }
        for id in &changed_references {
            if self
                .cached_references
                .as_ref()
                .is_some_and(|cached| cached.contains_key(id))
            {
                refresh_set.insert(id.clone());
            }
        }
        let mut refresh: Vec<SymbolId> = refresh_set.into_iter().collect();
        refresh.sort();
        let references = self.backend.references_batch(ctx, &refresh)?;
        self.recomputed_references += refresh.len();
        for id in &refresh {
            let fact = match patches.get(id) {
                Some(fact) => fact.clone(),
                None => match previous.get(id) {
                    Some(fact) => fact.clone(),
                    None => continue,
                },
            };
            let mut fact = fact;
            let list = references.get(id).cloned().unwrap_or_default();
            fact.references = list.clone().into();
            patches.insert(id.clone(), fact);
            if let Some(cached) = &mut self.cached_references {
                cached.insert(id.clone(), list);
            }
        }

        // A symbol is marked changed whenever it was recomputed, not when its
        // fact actually differs, so a span-stable edit patches rows identical
        // to the retained generation; because hashed ids scatter a file's
        // symbols across the whole canonical order, those no-op patches would
        // rebuild nearly every chunk. Identical rows are dropped here, after
        // every patch mutation above, so untouched chunks stay shared. The
        // transport manifest still over-names these symbols; the diff already
        // tolerates candidates that compare equal.
        patches.retain(|id, fact| previous.get(id).is_none_or(|retained| fact != retained));
        let (next, shared, removed, complete) =
            previous.patch(&self.symbol_seen, &patches, &self.removed_symbol_candidates);
        if !complete {
            return Ok(None);
        }
        if let Some(cached) = &mut self.cached_references {
            for id in &removed {
                cached.remove(id);
            }
        }
        self.removed_symbol_ids = removed;
        self.cached_reference_hits += self
            .cached_references
            .as_ref()
            .map_or(0, HashMap::len)
            .saturating_sub(refresh.len());
        self.closed_references = self.cached_references.take();
        self.patched_symbol_rows = patches.len();
        self.shared_symbol_chunks = shared;
        Ok(Some(next))
    }
}

fn nanos(elapsed: std::time::Duration) -> i64 {
    i64::try_from(elapsed.as_nanos()).unwrap_or(i64::MAX)
}

fn count(value: usize) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

/// Preserves canonical id ordering while avoiding a complete O(n log n) sort
/// when the preceding generation already established the order. Surviving rows
/// take their current values in the prior order; only new ids need sorting
/// before the two sorted runs are merged. Missing prior ids simply drop out
/// (Go: `orderSymbolFacts`).
///
/// `fact_index` maps interned handles to fact positions offset by one (zero
/// means absent) and is consumed: entries taken by the prior order are zeroed
/// so the leftovers identify this generation's additions.
#[must_use]
pub fn order_symbol_facts(
    mut facts: Vec<SymbolFact>,
    fact_index: &mut [i32],
    interner: &SharedInterner,
    previous: Option<&SymbolFactStore>,
) -> Vec<SymbolFact> {
    let fact_handle = |fact_index: &[i32], id: &str| -> Option<usize> {
        let handle = usize::try_from(interner.borrow().lookup(id)?).ok()?;
        (fact_index.get(handle).copied().unwrap_or(0) != 0).then_some(handle)
    };
    let Some(previous) = previous.filter(|previous| !previous.is_empty()) else {
        facts.sort_by(|left, right| left.id.cmp(&right.id));
        return facts;
    };
    if previous.len() == facts.len() {
        let mut ordered = Vec::with_capacity(facts.len());
        let mut complete = true;
        previous.range(
            |previous_fact| match fact_handle(fact_index, &previous_fact.id) {
                Some(handle) => {
                    ordered
                        .push(facts[usize::try_from(fact_index[handle] - 1).unwrap_or(0)].clone());
                }
                None => complete = false,
            },
        );
        if complete {
            return ordered;
        }
    }

    let mut retained: Vec<SymbolFact> = Vec::with_capacity(facts.len());
    previous.range(|previous_fact| {
        if let Some(handle) = fact_handle(fact_index, &previous_fact.id) {
            retained.push(facts[usize::try_from(fact_index[handle] - 1).unwrap_or(0)].clone());
            fact_index[handle] = 0;
        }
    });
    if retained.len() == facts.len() {
        return retained;
    }

    let mut added: Vec<SymbolFact> = Vec::with_capacity(facts.len() - retained.len());
    for (position, fact) in facts.iter().enumerate() {
        let Some(handle) = interner
            .borrow()
            .lookup(&fact.id)
            .and_then(|handle| usize::try_from(handle).ok())
        else {
            continue;
        };
        if fact_index.get(handle).copied() == Some(i32::try_from(position).unwrap_or(i32::MAX) + 1)
        {
            added.push(fact.clone());
        }
    }
    added.sort_by(|left, right| left.id.cmp(&right.id));

    let mut ordered = Vec::with_capacity(facts.len());
    let (mut left, mut right) = (0, 0);
    while left < retained.len() && right < added.len() {
        if retained[left].id < added[right].id {
            ordered.push(retained[left].clone());
            left += 1;
        } else {
            ordered.push(added[right].clone());
            right += 1;
        }
    }
    ordered.extend_from_slice(&retained[left..]);
    ordered.extend_from_slice(&added[right..]);
    ordered
}

use std::rc::Rc;
