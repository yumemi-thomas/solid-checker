//! The client's symbol closure patched from the previous table
//! (instant-edits P4).
//!
//! A full closure ([`Session::close_symbols_full`]) asks the producer for the
//! row of every id the table's seeds reach through alias edges, and for the
//! references of every id of the reference tier. When the producer names
//! every id whose answer changed since the previous table (per-file passes:
//! `reference_changes_exact`), the closure is the previous one patched: only
//! a changed or newly reached id is asked again, and only the references of a
//! tier id that changed or entered the tier. [`ClosureIndex`] keeps the
//! closure's reasons as counts, so an edit costs what its changed paths and
//! ids hold, not the table's size. `TYPEFACTS_VERIFY_SYMBOL_PATCH=1` compares
//! every patch with a full closure of the same table.
//!
//! The index relies on one shape the full closure has: an alias row's target
//! is not itself an alias (the checker resolves an alias chain to its end).
//! The index is built only when every row has it, and a patch that would
//! break it falls back to the table-wide walk.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use super::{Operation, Session, SessionError, SymbolQuery, request};
use crate::retained_table::FactTable;
use crate::{Location, SymbolFact};

/// The reference locations of the analysis being closed: the retained runs'
/// with the changed runs' replaced and the removed runs' dropped. Owns the
/// retained map for the analysis; [`Self::commit`] or [`Self::restore`]
/// hands it back.
pub(super) struct ReferenceLocations {
    retained: HashMap<Arc<str>, HashSet<Location>>,
    replaced: HashMap<Arc<str>, HashSet<Location>>,
}

impl ReferenceLocations {
    pub(super) fn new(
        retained: HashMap<Arc<str>, HashSet<Location>>,
        changed: &[super::DemandGroup<'_>],
        removed: &[String],
    ) -> Self {
        let mut replaced = HashMap::<Arc<str>, HashSet<Location>>::new();
        for path in removed {
            replaced.insert(Arc::from(path.as_str()), HashSet::new());
        }
        for group in changed {
            let locations = group
                .demands()
                .iter()
                .filter(|demand| demand.references)
                .map(|demand| demand.location.clone())
                .collect();
            replaced.insert(Arc::from(group.path()), locations);
        }
        Self { retained, replaced }
    }

    #[must_use]
    pub(super) fn contains(&self, location: &Location) -> bool {
        match self.replaced.get(location.path.as_ref()) {
            Some(locations) => locations.contains(location),
            None => self
                .retained
                .get(location.path.as_ref())
                .is_some_and(|locations| locations.contains(location)),
        }
    }

    /// The analysis was accepted: its locations are the retained ones now.
    pub(super) fn commit(self) -> HashMap<Arc<str>, HashSet<Location>> {
        let Self {
            mut retained,
            replaced,
        } = self;
        for (path, locations) in replaced {
            if locations.is_empty() {
                retained.remove(&path);
            } else {
                retained.insert(path, locations);
            }
        }
        retained
    }

    /// The analysis failed: the retained locations stay.
    pub(super) fn restore(self) -> HashMap<Arc<str>, HashSet<Location>> {
        self.retained
    }
}

/// The reasons each id of the closure holds: seed occurrences, alias rows
/// pointing at it, and reference roots by path with what they resolve to.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct ClosureIndex {
    seeds: HashMap<Arc<str>, u32>,
    alias_in: HashMap<Arc<str>, u32>,
    roots: HashMap<Arc<str>, u32>,
    roots_by_path: HashMap<Arc<str>, Vec<Arc<str>>>,
    tier: HashMap<Arc<str>, u32>,
}

/// What the table's paths name as closure seeds.
fn path_seeds<'a>(table: &'a FactTable, path: &str) -> impl Iterator<Item = &'a Arc<str>> {
    let entities = table.entities_for_path(path).iter().flat_map(|entity| {
        std::iter::once(&entity.symbol).chain(entity.resolved_call.iter().map(|call| &call.target))
    });
    let files = table.file(path).into_iter().flat_map(|file| {
        file.async_functions
            .iter()
            .flat_map(|function| [&function.symbol, &function.target])
    });
    entities.chain(files).filter(|id| !id.is_empty())
}

/// The reference roots of one path: its entities at a demanded reference
/// location, in entity order.
fn path_roots(
    table: &FactTable,
    path: &str,
    reference_locations: &ReferenceLocations,
) -> Vec<Arc<str>> {
    table
        .entities_for_path(path)
        .iter()
        .filter(|entity| {
            !entity.symbol.is_empty() && reference_locations.contains(&entity.location)
        })
        .map(|entity| Arc::clone(&entity.symbol))
        .collect()
}

fn increment(counts: &mut HashMap<Arc<str>, u32>, id: &Arc<str>) -> bool {
    let count = counts.entry(Arc::clone(id)).or_default();
    *count += 1;
    *count == 1
}

/// `false` when the count was already zero (an index out of step: the caller
/// abandons it).
fn decrement(counts: &mut HashMap<Arc<str>, u32>, id: &Arc<str>) -> Option<bool> {
    let count = counts.get_mut(id)?;
    *count -= 1;
    if *count == 0 {
        counts.remove(id);
        Some(true)
    } else {
        Some(false)
    }
}

fn held(counts: &HashMap<Arc<str>, u32>, id: &str) -> bool {
    counts.get(id).is_some_and(|count| *count > 0)
}

impl ClosureIndex {
    /// The index of a closed table, or `None` when a row breaks the one-hop
    /// alias shape or no seed reaches it.
    pub(super) fn build(
        table: &FactTable,
        reference_locations: &ReferenceLocations,
    ) -> Option<Self> {
        let mut index = Self::default();
        let mut paths = Vec::new();
        for source in table.sources() {
            paths.push(Arc::clone(&source.path));
        }
        let mut seen = HashSet::new();
        for entity in table.entities() {
            if seen.insert(Arc::clone(&entity.location.path)) {
                paths.push(Arc::clone(&entity.location.path));
            }
        }
        for file in table.files() {
            paths.push(Arc::clone(&file.path));
        }
        paths.sort();
        paths.dedup();
        for path in &paths {
            for id in path_seeds(table, path) {
                increment(&mut index.seeds, id);
            }
            let roots = path_roots(table, path, reference_locations);
            if !roots.is_empty() {
                index.roots_by_path.insert(Arc::clone(path), roots);
            }
        }
        for symbol in table.symbols() {
            let target = symbol.alias_target();
            if !target.is_empty() {
                if !table.symbol(target)?.alias_target().is_empty() {
                    return None;
                }
                increment(&mut index.alias_in, table.symbol_alias_target(symbol.id())?);
            }
        }
        for id in table.symbol_ids() {
            if !held(&index.seeds, id) && !held(&index.alias_in, id) {
                return None;
            }
        }
        for roots in index.roots_by_path.values() {
            for root in roots {
                increment(&mut index.roots, root);
                let resolved = resolve(table, None, root)?;
                increment(&mut index.tier, &resolved);
            }
        }
        Some(index)
    }
}

/// The first part of two indexes that differs, for a verification report.
pub(super) fn index_difference(
    patched: Option<&ClosureIndex>,
    fresh: Option<&ClosureIndex>,
) -> String {
    fn counts(
        name: &str,
        left: &HashMap<Arc<str>, u32>,
        right: &HashMap<Arc<str>, u32>,
    ) -> Option<String> {
        let key = left
            .iter()
            .find(|(key, count)| right.get(*key) != Some(count))
            .map(|(key, _)| key)
            .or_else(|| right.keys().find(|key| !left.contains_key(*key)))?;
        Some(format!(
            " ({name} of {key}: {:?} vs {:?})",
            left.get(key),
            right.get(key)
        ))
    }
    let (Some(patched), Some(fresh)) = (patched, fresh) else {
        return String::new();
    };
    counts("seeds", &patched.seeds, &fresh.seeds)
        .or_else(|| counts("alias_in", &patched.alias_in, &fresh.alias_in))
        .or_else(|| counts("roots", &patched.roots, &fresh.roots))
        .or_else(|| counts("tier", &patched.tier, &fresh.tier))
        .or_else(|| {
            (patched.roots_by_path != fresh.roots_by_path).then(|| {
                let path = patched
                    .roots_by_path
                    .iter()
                    .find(|(path, roots)| fresh.roots_by_path.get(*path) != Some(roots))
                    .map(|(path, _)| path)
                    .or_else(|| {
                        fresh
                            .roots_by_path
                            .keys()
                            .find(|path| !patched.roots_by_path.contains_key(*path))
                    });
                format!(
                    " (roots of {path:?}: {:?} vs {:?})",
                    path.and_then(|path| patched.roots_by_path.get(path)),
                    path.and_then(|path| fresh.roots_by_path.get(path))
                )
            })
        })
        .unwrap_or_default()
}

/// What a reference root resolves to: its alias target, or itself.
fn resolve(
    table: &FactTable,
    fresh: Option<&HashMap<Arc<str>, SymbolFact>>,
    id: &Arc<str>,
) -> Option<Arc<str>> {
    let target = match fresh.and_then(|fresh| fresh.get(id)) {
        Some(fact) => Arc::clone(&fact.alias_target),
        None => Arc::clone(table.symbol_alias_target(id)?),
    };
    Some(if target.is_empty() {
        Arc::clone(id)
    } else {
        target
    })
}

/// The paths whose closure inputs changed: entity rows, file rows, and the
/// demand runs that decide which entities are reference roots.
pub(super) struct ChangedPaths<'a> {
    pub entities: &'a [String],
    pub files: &'a [String],
    pub demands: &'a HashSet<String>,
}

/// The successor rows a patch computed, before references are asked.
struct Patch {
    fresh: HashMap<Arc<str>, SymbolFact>,
    removed: HashSet<Arc<str>>,
    tier: HashSet<Arc<str>>,
    /// Tier ids whose references must be asked (changed, new, or entering).
    refresh: Vec<Arc<str>>,
}

impl Session {
    /// The closure of `table` patched from the previous one: see the module
    /// documentation. `previous` is the table before this transition (its
    /// symbols are `table`'s).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn patch_symbol_closure(
        &mut self,
        previous: &FactTable,
        table: &mut FactTable,
        reference_locations: &ReferenceLocations,
        changed: &HashSet<Arc<str>>,
        paths: &ChangedPaths<'_>,
        state_token: &str,
        release: bool,
    ) -> Result<Vec<String>, SessionError> {
        let indexed = match self.closure_index.take() {
            Some(index) => self.patch_indexed(
                index,
                previous,
                table,
                reference_locations,
                changed,
                paths,
                state_token,
            )?,
            None => None,
        };
        let patch = match indexed {
            Some(patch) => patch,
            None => self.patch_walk(table, reference_locations, changed, state_token)?,
        };
        let changes = self.finish_patch(table, patch, changed, state_token, release)?;
        if self.closure_index.is_none() {
            self.closure_index = ClosureIndex::build(table, reference_locations);
        }
        Ok(changes)
    }

    /// Asks the rows of `pending` (no references).
    fn ask_rows(
        &self,
        pending: &mut Vec<Arc<str>>,
        state_token: &str,
    ) -> Result<Vec<SymbolFact>, SessionError> {
        pending.sort();
        pending.dedup();
        let mut request = request(Operation::Symbols, &self.project_id, self.generation);
        request.state_token = state_token.to_owned();
        request.symbol_queries = pending
            .iter()
            .map(|id| SymbolQuery {
                id: Arc::clone(id),
                references: false,
                references_only: false,
            })
            .collect();
        let response = self.exchange_once(&mut request)?;
        if response.symbol_evidence.len() != pending.len() {
            return Err(SessionError::InvalidResponse(format!(
                "symbol oracle returned {} rows for {} queries",
                response.symbol_evidence.len(),
                pending.len()
            )));
        }
        for (expected, fact) in pending.iter().zip(&response.symbol_evidence) {
            if fact.id != *expected {
                return Err(SessionError::InvalidResponse(format!(
                    "symbol oracle returned {:?}, expected {expected:?}",
                    fact.id
                )));
            }
        }
        pending.clear();
        Ok(response.symbol_evidence)
    }

    /// The table-wide patch: every seed walked, held rows reused.
    fn patch_walk(
        &mut self,
        table: &FactTable,
        reference_locations: &ReferenceLocations,
        changed: &HashSet<Arc<str>>,
        state_token: &str,
    ) -> Result<Patch, SessionError> {
        let held_target = |id: &Arc<str>| -> Option<Arc<str>> {
            if changed.contains(id) {
                return None;
            }
            table.symbol_alias_target(id).cloned()
        };
        let mut reached = HashSet::<Arc<str>>::with_capacity(table.symbol_count());
        let mut stack = Vec::<Arc<str>>::new();
        let mut reference_roots = Vec::new();
        {
            let mut enqueue = |id: &Arc<str>| {
                if !id.is_empty() && reached.insert(Arc::clone(id)) {
                    stack.push(Arc::clone(id));
                }
            };
            for entity in table.entities() {
                enqueue(&entity.symbol);
                if let Some(call) = &entity.resolved_call {
                    enqueue(&call.target);
                }
                if reference_locations.contains(&entity.location) && !entity.symbol.is_empty() {
                    reference_roots.push(Arc::clone(&entity.symbol));
                }
            }
            for file in table.files() {
                for function in file.async_functions.iter() {
                    enqueue(&function.symbol);
                    enqueue(&function.target);
                }
            }
        }
        let mut fresh = HashMap::<Arc<str>, SymbolFact>::new();
        loop {
            let mut pending = Vec::new();
            while let Some(id) = stack.pop() {
                match held_target(&id) {
                    Some(target) => {
                        if !target.is_empty() && reached.insert(Arc::clone(&target)) {
                            stack.push(target);
                        }
                    }
                    None => pending.push(id),
                }
            }
            if pending.is_empty() {
                break;
            }
            for fact in self.ask_rows(&mut pending, state_token)? {
                if !fact.alias_target.is_empty() && reached.insert(Arc::clone(&fact.alias_target)) {
                    stack.push(Arc::clone(&fact.alias_target));
                }
                fresh.insert(Arc::clone(&fact.id), fact);
            }
        }
        let alias_target = |id: &Arc<str>| -> Option<Arc<str>> {
            fresh.get(id).map_or_else(
                || table.symbol_alias_target(id).cloned(),
                |fact| Some(Arc::clone(&fact.alias_target)),
            )
        };
        let mut full = HashSet::<Arc<str>>::with_capacity(reference_roots.len());
        let mut queue = Vec::with_capacity(reference_roots.len());
        for id in reference_roots {
            if full.insert(Arc::clone(&id)) {
                queue.push(id);
            }
        }
        while let Some(id) = queue.pop() {
            if let Some(target) = alias_target(&id)
                && !target.is_empty()
                && full.insert(Arc::clone(&target))
            {
                queue.push(target);
            }
        }
        let tier = full
            .into_iter()
            .filter(|id| alias_target(id).is_some_and(|target| target.is_empty()))
            .collect::<HashSet<_>>();
        let refresh = tier
            .iter()
            .filter(|id| fresh.contains_key(*id) || !self.reference_tier.contains(*id))
            .cloned()
            .collect();
        let removed = table
            .symbol_ids()
            .filter(|id| !reached.contains(*id))
            .cloned()
            .collect();
        Ok(Patch {
            fresh,
            removed,
            tier,
            refresh,
        })
    }

    /// The patch from the index: only the changed paths' seeds and roots and
    /// the changed ids are visited. `None` (the index dropped) when it is
    /// out of step or a row breaks the one-hop alias shape.
    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    fn patch_indexed(
        &mut self,
        mut index: ClosureIndex,
        previous: &FactTable,
        table: &FactTable,
        reference_locations: &ReferenceLocations,
        changed: &HashSet<Arc<str>>,
        paths: &ChangedPaths<'_>,
        state_token: &str,
    ) -> Result<Option<Patch>, SessionError> {
        // Seeds of the changed paths: the old ones leave, the new ones enter.
        let mut seed_paths = paths
            .entities
            .iter()
            .chain(paths.files)
            .map(String::as_str)
            .collect::<Vec<_>>();
        seed_paths.sort_unstable();
        seed_paths.dedup();
        let mut candidates = Vec::new();
        let mut entered = Vec::new();
        for path in &seed_paths {
            for id in path_seeds(table, path) {
                if increment(&mut index.seeds, id) {
                    entered.push(Arc::clone(id));
                }
            }
        }
        for path in &seed_paths {
            for id in path_seeds(previous, path) {
                match decrement(&mut index.seeds, id) {
                    None => return Ok(None),
                    Some(true) => candidates.push(Arc::clone(id)),
                    Some(false) => {}
                }
            }
        }
        // Rows to ask: changed held ids, and newly reached ones.
        let present = |id: &str| table.symbol(id).is_some();
        let mut fresh = HashMap::<Arc<str>, SymbolFact>::new();
        let mut asked = HashSet::<Arc<str>>::new();
        let mut pending = Vec::new();
        for id in changed {
            if present(id) && asked.insert(Arc::clone(id)) {
                pending.push(Arc::clone(id));
            }
        }
        for id in entered {
            if held(&index.seeds, &id) && !present(&id) && asked.insert(Arc::clone(&id)) {
                pending.push(id);
            }
        }
        while !pending.is_empty() {
            for fact in self.ask_rows(&mut pending, state_token)? {
                if let Some(old) = table.symbol_alias_target(&fact.id)
                    && !old.is_empty()
                {
                    match decrement(&mut index.alias_in, old) {
                        None => return Ok(None),
                        Some(true) => candidates.push(Arc::clone(old)),
                        Some(false) => {}
                    }
                }
                let target = &fact.alias_target;
                if !target.is_empty() {
                    increment(&mut index.alias_in, target);
                    if !present(target) && asked.insert(Arc::clone(target)) {
                        pending.push(Arc::clone(target));
                    }
                }
                fresh.insert(Arc::clone(&fact.id), fact);
            }
        }
        // Ids no reason holds any more leave, and with them their alias edge.
        let mut removed = HashSet::<Arc<str>>::new();
        while let Some(id) = candidates.pop() {
            if held(&index.seeds, &id)
                || held(&index.alias_in, &id)
                || removed.contains(&id)
                || !(present(&id) || fresh.contains_key(&id))
            {
                continue;
            }
            let target = match fresh.remove(&id) {
                Some(fact) => Arc::clone(&fact.alias_target),
                None => table
                    .symbol_alias_target(&id)
                    .cloned()
                    .unwrap_or_else(|| Arc::from("")),
            };
            if present(&id) {
                removed.insert(Arc::clone(&id));
            }
            if !target.is_empty() {
                match decrement(&mut index.alias_in, &target) {
                    None => return Ok(None),
                    Some(true) => candidates.push(target),
                    Some(false) => {}
                }
            }
        }
        // The one-hop shape: a fresh alias row's target is not an alias, and
        // nothing points at a fresh alias row.
        let row_target = |id: &Arc<str>| -> Option<Arc<str>> {
            fresh.get(id).map_or_else(
                || table.symbol_alias_target(id).cloned(),
                |fact| Some(Arc::clone(&fact.alias_target)),
            )
        };
        for (id, fact) in &fresh {
            if fact.alias_target.is_empty() {
                continue;
            }
            if held(&index.alias_in, id)
                || row_target(&fact.alias_target).is_none_or(|target| !target.is_empty())
            {
                return Ok(None);
            }
        }

        // The tier: the changed paths' old roots leave under their old
        // resolution, changed rows move the rest, new roots enter.
        let mut root_paths = paths
            .entities
            .iter()
            .map(String::as_str)
            .chain(paths.demands.iter().map(String::as_str))
            .collect::<Vec<_>>();
        root_paths.sort_unstable();
        root_paths.dedup();
        let mut moved = Vec::new();
        for path in &root_paths {
            for root in index.roots_by_path.remove(*path).unwrap_or_default() {
                let Some(resolved) = resolve(previous, None, &root) else {
                    return Ok(None);
                };
                if decrement(&mut index.roots, &root).is_none()
                    || decrement(&mut index.tier, &resolved).is_none()
                {
                    return Ok(None);
                }
                moved.push(resolved);
            }
        }
        for (id, fact) in &fresh {
            let Some(count) = index.roots.get(id).copied() else {
                continue;
            };
            let Some(old) = resolve(previous, None, id) else {
                return Ok(None);
            };
            let new = if fact.alias_target.is_empty() {
                Arc::clone(id)
            } else {
                Arc::clone(&fact.alias_target)
            };
            if old != new {
                for _ in 0..count {
                    if decrement(&mut index.tier, &old).is_none() {
                        return Ok(None);
                    }
                    increment(&mut index.tier, &new);
                }
                moved.push(old);
                moved.push(new);
            }
        }
        for path in &root_paths {
            let roots = path_roots(table, path, reference_locations);
            for root in &roots {
                let Some(resolved) = resolve(table, Some(&fresh), root) else {
                    return Ok(None);
                };
                increment(&mut index.roots, root);
                increment(&mut index.tier, &resolved);
                moved.push(resolved);
            }
            if !roots.is_empty() {
                index.roots_by_path.insert(Arc::from(*path), roots);
            }
        }
        let mut tier = self.reference_tier.clone();
        let mut refresh = HashSet::<Arc<str>>::new();
        for id in moved {
            if held(&index.tier, &id) {
                if tier.insert(Arc::clone(&id)) {
                    refresh.insert(id);
                }
            } else {
                tier.remove(&id);
            }
        }
        for id in fresh.keys() {
            if tier.contains(id) {
                refresh.insert(Arc::clone(id));
            }
        }
        self.closure_index = Some(index);
        Ok(Some(Patch {
            fresh,
            removed,
            tier,
            refresh: refresh.into_iter().collect(),
        }))
    }

    /// Asks the references a patch needs (the last request releases the
    /// analysis), then applies it to `table`, the reference tier and the
    /// path index.
    fn finish_patch(
        &mut self,
        table: &mut FactTable,
        patch: Patch,
        changed: &HashSet<Arc<str>>,
        state_token: &str,
        release: bool,
    ) -> Result<Vec<String>, SessionError> {
        let Patch {
            mut fresh,
            removed,
            tier,
            mut refresh,
        } = patch;
        refresh.sort();
        refresh.dedup();
        let mut released = false;
        let mut references = HashMap::<Arc<str>, Arc<[Location]>>::new();
        if !refresh.is_empty() {
            let mut request = request(Operation::Symbols, &self.project_id, self.generation);
            request.state_token = state_token.to_owned();
            request.symbol_queries = refresh
                .iter()
                .map(|id| SymbolQuery {
                    id: Arc::clone(id),
                    references: true,
                    references_only: true,
                })
                .collect();
            request.release_analysis = release;
            let response = self.exchange_once(&mut request)?;
            released = release;
            if response.symbol_evidence.len() != refresh.len() {
                return Err(SessionError::InvalidResponse(format!(
                    "reference oracle returned {} rows for {} queries",
                    response.symbol_evidence.len(),
                    refresh.len()
                )));
            }
            for (expected, fact) in refresh.into_iter().zip(response.symbol_evidence) {
                if fact.id != expected
                    || !fact.alias_target.is_empty()
                    || !fact.declarations.is_empty()
                {
                    return Err(SessionError::InvalidResponse(format!(
                        "reference oracle returned invalid row for {expected:?}"
                    )));
                }
                references.insert(expected, fact.references);
            }
        }
        if release && !released {
            let mut request = request(Operation::Symbols, &self.project_id, self.generation);
            request.state_token = state_token.to_owned();
            request.release_analysis = true;
            self.exchange_once(&mut request)?;
        }
        let mut upserts = Vec::with_capacity(fresh.len() + references.len());
        for (id, found) in references {
            if let Some(fact) = fresh.get_mut(&id) {
                fact.references = found;
            } else if let Some(mut fact) = table.symbol_fact(&id) {
                fact.references = found;
                upserts.push(fact);
            }
        }
        upserts.extend(fresh.into_values());
        for id in &self.reference_tier {
            if !tier.contains(id)
                && !removed.contains(id)
                && !changed.contains(id)
                && let Some(mut fact) = table.symbol_fact(id)
                && !fact.references.is_empty()
            {
                fact.references = Arc::from(Vec::new());
                upserts.push(fact);
            }
        }
        let declared_paths = |table: &FactTable, id: &str| {
            table
                .symbol(id)
                .map(|symbol| {
                    symbol
                        .declarations()
                        .iter()
                        .map(|declaration| Arc::clone(&declaration.location.path))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        };
        let touched = upserts
            .iter()
            .map(|fact| Arc::clone(&fact.id))
            .chain(removed.iter().cloned())
            .collect::<Vec<_>>();
        for id in &touched {
            for path in declared_paths(table, id) {
                if let Some(ids) = self.symbols_by_path.get_mut(&path) {
                    ids.retain(|candidate| candidate != id);
                    if ids.is_empty() {
                        self.symbols_by_path.remove(&path);
                    }
                }
            }
        }
        let changes = table.merge_symbols(upserts, &removed);
        for id in &touched {
            for path in declared_paths(table, id) {
                let ids = self.symbols_by_path.entry(path).or_default();
                if !ids.contains(id) {
                    ids.push(Arc::clone(id));
                }
            }
        }
        self.reference_tier = tier;
        Ok(changes)
    }
}
