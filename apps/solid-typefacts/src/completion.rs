//! A pass's entity rows and the answers that complete them after every pass
//! (`semantic_runs::Deferrals`, ADR 0398).
//!
//! A pass prepares its run's entity rows (`contributions::prepare_rows`)
//! from its result before the answers that read every file, and records
//! which rows those answers can change. The table completes them later: a
//! row's field is the one of the last entity at its location that has it, so
//! a deferred answer changes a row only where that entity is the field's
//! last holder:
//!
//! * a deferred reference space (always present once completed) wins
//!   `reference_space` when no later entity at the location holds one;
//! * a withdrawn default-library binding changes the row's
//!   `resolved_call` only when the entity is the call's last holder;
//! * a suppressed type descriptor (an entity of a structural-accessor
//!   symbol, ADR 0398's batch-wide suppression) leaves the row the last
//!   descriptor that is not suppressed.
//!
//! [`Patches`] records exactly those (row, question) pairs, and [`Answers`]
//! their answers; [`Patches::apply`] turns base rows and answers into the
//! rows the table serves, the same rows `complete_deferred` and
//! `prepare_retained_contribution` make from the completed result.

use crate::model::SymbolId;
use crate::semantic_runs::Deferrals;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use typefacts::{EntityFact, ReferenceSpace, ResolvedCall, TypeDescriptor};

/// A run's rows as its pass prepared them, before the deferred answers.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PassRows {
    pub rows: Arc<[EntityFact]>,
    pub durable: bool,
    /// Entities with a symbol (the retention counts).
    pub symbols: usize,
    pub patches: Patches,
}

/// The deferred answers of a run, in the order the run deferred them, and
/// the rows each can change, with what changing them needs.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Patches {
    /// Every deferred reference space: the row it sets (`None` when a later
    /// entity at the location holds the field), and the id whose merged
    /// space it is.
    pub spaces: Vec<(Option<u32>, SymbolId)>,
    /// Every deferred default-library binding: the row whose call it can
    /// withdraw (`None` when the entity does not hold the row's call), and
    /// the id whose write census decides it.
    pub library: Vec<(Option<u32>, SymbolId)>,
    /// The calls the library deferrals can withdraw a binding from, by row,
    /// as the pass resolved them.
    pub calls: Vec<(u32, Arc<ResolvedCall>)>,
    /// Rows whose descriptor a structural accessor of another file's run can
    /// suppress: each entity at the location holding one, in order, with its
    /// symbol when it can be suppressed. The row's descriptor is the last
    /// one not suppressed.
    pub descriptors: Vec<(u32, Chain)>,
}

/// The descriptors at one location, in order, each with its symbol when it
/// can be suppressed.
pub(crate) type Chain = Vec<(Arc<TypeDescriptor>, Option<SymbolId>)>;

/// One completion of [`Patches`], index-aligned with its lists: what
/// `complete_deferred` would record for the run.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Answers {
    pub spaces: Vec<ReferenceSpace>,
    /// Whether a reference of the id is a write (the binding withdrawn).
    pub library: Vec<bool>,
    /// Per descriptor chain, which entries are suppressed.
    pub suppressed: Vec<Vec<bool>>,
}

impl Patches {
    /// The patches of `entities` (a run's aligned result, before the
    /// deferred answers) under `deferrals`.
    pub(crate) fn of(entities: &[EntityFact], deferrals: &Deferrals) -> Self {
        // Each entity's row, and each row's last holder of the two fields
        // (an entity whose space is deferred holds one once completed).
        let mut row_of: Vec<u32> = Vec::with_capacity(entities.len());
        let mut groups: Vec<(usize, usize)> = Vec::new();
        for (at, entity) in entities.iter().enumerate() {
            if at == 0 || entities[at - 1].location != entity.location {
                groups.push((at, at + 1));
            } else if let Some(group) = groups.last_mut() {
                group.1 = at + 1;
            }
            row_of.push(u32::try_from(groups.len() - 1).unwrap_or(u32::MAX));
        }
        let deferred: HashSet<usize> = deferrals.spaces.iter().map(|(at, _)| *at).collect();
        let mut patches = Self::default();
        // The last deferral of an entity's space is the one that stands.
        let mut last_space: HashMap<usize, usize> = HashMap::new();
        for (position, (at, _)) in deferrals.spaces.iter().enumerate() {
            last_space.insert(*at, position);
        }
        for (position, (at, id)) in deferrals.spaces.iter().enumerate() {
            let row = entities.get(*at).and_then(|_| {
                let (start, end) = groups[row_of[*at] as usize];
                let holder = (start..end).rev().find(|other| {
                    entities[*other].reference_space.is_some() || deferred.contains(other)
                })?;
                (holder == *at && last_space.get(at) == Some(&position)).then_some(row_of[*at])
            });
            patches.spaces.push((row, id.clone()));
        }
        for (at, id) in &deferrals.library {
            let row = entities.get(*at).and_then(|entity| {
                let call = entity.resolved_call.as_ref()?;
                let (start, end) = groups[row_of[*at] as usize];
                let holder = (start..end)
                    .rev()
                    .find(|other| entities[*other].resolved_call.is_some())?;
                if holder != *at {
                    return None;
                }
                let row = row_of[*at];
                if !patches.calls.iter().any(|(held, _)| *held == row) {
                    patches.calls.push((row, Arc::clone(call)));
                }
                Some(row)
            });
            patches.library.push((row, id.clone()));
        }
        for (row, (start, end)) in groups.iter().enumerate() {
            let chain: Chain = (*start..*end)
                .filter_map(|at| {
                    let entity = &entities[at];
                    let descriptor = entity.type_descriptor.as_ref()?;
                    let candidate = (!entity.symbol.is_empty()
                        && !deferrals.structural.contains(&entity.symbol))
                    .then(|| entity.symbol.clone());
                    Some((Arc::clone(descriptor), candidate))
                })
                .collect();
            if chain.iter().any(|(_, candidate)| candidate.is_some()) {
                patches
                    .descriptors
                    .push((u32::try_from(row).unwrap_or(u32::MAX), chain));
            }
        }
        patches
    }

    /// Every id a space or library answer reads.
    pub(crate) fn asked(&self) -> impl Iterator<Item = &SymbolId> {
        self.spaces.iter().chain(&self.library).map(|(_, id)| id)
    }

    /// Every symbol whose structural standing a descriptor answer reads.
    pub(crate) fn candidates(&self) -> impl Iterator<Item = &SymbolId> {
        self.descriptors
            .iter()
            .flat_map(|(_, chain)| chain.iter().filter_map(|(_, candidate)| candidate.as_ref()))
    }

    /// The answers, from the merged space of an id, whether a reference of
    /// an id is a write, and the batch's structural accessors.
    pub(crate) fn answer(
        &self,
        mut space: impl FnMut(&SymbolId) -> ReferenceSpace,
        mut written: impl FnMut(&SymbolId) -> bool,
        structural: impl Fn(&SymbolId) -> bool,
    ) -> Answers {
        Answers {
            spaces: self.spaces.iter().map(|(_, id)| space(id)).collect(),
            library: self.library.iter().map(|(_, id)| written(id)).collect(),
            suppressed: self
                .descriptors
                .iter()
                .map(|(_, chain)| {
                    chain
                        .iter()
                        .map(|(_, candidate)| candidate.as_ref().is_some_and(&structural))
                        .collect()
                })
                .collect(),
        }
    }

    /// `base` with `answers` applied; `base` itself when they change no row.
    pub(crate) fn apply(&self, base: &Arc<[EntityFact]>, answers: &Answers) -> Arc<[EntityFact]> {
        let spaces: Vec<(usize, ReferenceSpace)> = self
            .spaces
            .iter()
            .zip(&answers.spaces)
            .filter_map(|((row, _), space)| row.map(|row| (row as usize, *space)))
            .filter(|(row, space)| base[*row].reference_space != Some(*space))
            .collect();
        // A call loses its binding when any deferral of its entity is
        // written.
        let withdrawn: HashSet<u32> = self
            .library
            .iter()
            .zip(&answers.library)
            .filter_map(|((row, _), written)| row.filter(|_| *written))
            .collect();
        let descriptors: Vec<(usize, Option<Arc<TypeDescriptor>>)> = self
            .descriptors
            .iter()
            .zip(&answers.suppressed)
            .filter_map(|((row, chain), suppressed)| {
                let stands = chain
                    .iter()
                    .zip(suppressed)
                    .rev()
                    .find(|(_, suppressed)| !**suppressed)
                    .map(|((descriptor, _), _)| Arc::clone(descriptor));
                let held = base[*row as usize].type_descriptor.as_ref();
                let same = match (&stands, held) {
                    (Some(stands), Some(held)) => Arc::ptr_eq(stands, held),
                    (None, None) => true,
                    _ => false,
                };
                (!same).then_some((*row as usize, stands))
            })
            .collect();
        if spaces.is_empty() && withdrawn.is_empty() && descriptors.is_empty() {
            return Arc::clone(base);
        }
        let mut rows: Vec<EntityFact> = base.to_vec();
        for (row, space) in spaces {
            rows[row].reference_space = Some(space);
        }
        for (row, call) in &self.calls {
            if withdrawn.contains(row) {
                let mut call = ResolvedCall::clone(call);
                call.default_library_binding = None;
                rows[*row as usize].resolved_call = Some(Arc::new(call));
            }
        }
        for (row, stands) in descriptors {
            rows[row].type_descriptor = stands;
        }
        rows.into()
    }
}

/// The structural accessors of a table's runs, kept as counts so a changed
/// run moves only its own.
#[derive(Default)]
pub(crate) struct Structural {
    counts: HashMap<SymbolId, u32>,
}

impl Structural {
    pub(crate) fn contains(&self, id: &SymbolId) -> bool {
        self.counts.contains_key(id)
    }

    /// Adds (or with `add` false removes) one run's accessors; `flipped`
    /// gains every symbol whose membership changed.
    pub(crate) fn count(
        &mut self,
        accessors: &HashSet<SymbolId>,
        add: bool,
        flipped: &mut HashSet<SymbolId>,
    ) {
        for id in accessors {
            if add {
                let count = self.counts.entry(id.clone()).or_default();
                *count += 1;
                if *count == 1 {
                    flipped.insert(id.clone());
                }
            } else if let Some(count) = self.counts.get_mut(id) {
                *count -= 1;
                if *count == 0 {
                    self.counts.remove(id);
                    flipped.insert(id.clone());
                }
            }
        }
    }

    pub(crate) fn clear(&mut self) {
        self.counts.clear();
    }
}
