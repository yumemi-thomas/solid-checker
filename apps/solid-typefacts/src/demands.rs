//! The session-owned canonical demand set (Go: `session_demands.go`, the
//! `DemandGroup` half of `semantic_demand.go`, and `demandGroup` of
//! `semantic_retained.go`).
//!
//! Its sorted groups are mutated transactionally: sparse analyzes edit only
//! named paths, while an undo log restores the preceding state if any later
//! stage fails. A [`RetainedDemandTransaction`] that is dropped without
//! [`RetainedDemandTransaction::commit`] rolls back, which is how every early
//! return of `analyze` restores the demand set.

use crate::contributions::ContributionRef;
use crate::gocompat::{clean, quote};
use std::collections::BTreeMap;
use std::sync::Arc;
use typefacts::producer::{
    append_compact_demands_with_flag, compact_demand_shape, expand_compact_group,
};
use typefacts::v3::{CompactDemandGroup, CompactDemands, DEMAND_FLAG_ASYNC, EntityDemand};

/// A retained packed run and the string table it indexes into.
#[derive(Clone, Debug)]
pub struct CompactBacking {
    pub group: CompactDemandGroup,
    pub strings: Arc<[String]>,
}

/// One file's slice of the canonical demand list (Go: `demandGroup`). A
/// compact group keeps its packed form and expands lazily.
#[derive(Clone, Debug, Default)]
pub struct DemandGroup {
    /// The clean path; the retention key.
    pub path: String,
    pub demands: Option<Arc<[EntityDemand]>>,
    pub compact: Option<CompactBacking>,
    pub contribution: Option<ContributionRef>,
    /// The clean locations of [`DemandGroup::append_async_demands`]'s
    /// answer, computed once.
    pub async_locations: Option<Arc<[typefacts::Location]>>,
}

impl DemandGroup {
    /// A plain group over already-canonical demands.
    #[must_use]
    pub fn plain(path: &str, demands: Vec<EntityDemand>) -> Self {
        Self {
            path: path.to_owned(),
            demands: Some(demands.into()),
            ..Self::default()
        }
    }

    #[must_use]
    pub fn is_compact(&self) -> bool {
        self.compact.is_some()
    }

    /// The run's demands, expanding a compact group on first use.
    pub fn ensure_expanded(&mut self) -> Result<(), String> {
        if self.demands.is_some() {
            return Ok(());
        }
        let Some(backing) = &self.compact else {
            return Ok(());
        };
        let mut demands = expand_compact_group(&backing.group, &backing.strings)?;
        canonicalize_demand_run(&self.path, &mut demands);
        self.demands = Some(demands.into());
        Ok(())
    }

    /// Drops the expansion of a compact group (Go: `releaseExpanded`).
    pub fn release_expanded(&mut self) {
        if self.is_compact() {
            self.demands = None;
        }
    }

    /// The clean locations of the run's async demands
    /// ([`DemandGroup::append_async_demands`]), computed on first use: a
    /// group is replaced, never edited, when its demands change.
    pub fn async_locations(&mut self) -> Result<&[typefacts::Location], String> {
        if self.async_locations.is_none() {
            let mut demands = Vec::new();
            self.append_async_demands(&mut demands)?;
            let locations: Vec<typefacts::Location> = demands
                .into_iter()
                .filter(|demand| demand.r#async)
                .map(|demand| {
                    let mut location = demand.location;
                    location.path = clean(&location.path).into();
                    location
                })
                .collect();
            self.async_locations = Some(locations.into());
        }
        Ok(self.async_locations.as_deref().unwrap_or_default())
    }

    /// Appends the run's async demands without expanding a compact run's other
    /// rows (Go: `appendAsyncDemands`).
    pub fn append_async_demands(&self, target: &mut Vec<EntityDemand>) -> Result<(), String> {
        let Some(backing) = &self.compact else {
            if let Some(demands) = &self.demands {
                target.extend(demands.iter().filter(|demand| demand.r#async).cloned());
            }
            return Ok(());
        };
        let start = target.len();
        append_compact_demands_with_flag(
            target,
            &backing.group,
            &backing.strings,
            DEMAND_FLAG_ASYNC,
        )?;
        canonicalize_demand_run(&self.path, &mut target[start..]);
        Ok(())
    }
}

/// Restores the retained group's clean-path invariant after expanding compact
/// rows. The compact string table carries paths as the client sent them
/// (Go: `canonicalizeDemandRun`).
fn canonicalize_demand_run(path: &str, demands: &mut [EntityDemand]) {
    // One shared path for the run (a retained group holds every location):
    // the same strings as one allocation per demand.
    let shared: Arc<str> = path.into();
    for demand in demands {
        demand.location.path = Arc::clone(&shared);
        if let Some(query) = &mut demand.query_location {
            let cleaned = clean(&query.path);
            if *cleaned == *shared {
                query.path = Arc::clone(&shared);
            } else if *cleaned != *query.path {
                query.path = cleaned.into();
            }
        }
    }
}

/// Establishes the retained closure's ordering and clean-path invariants at
/// its external input seam (Go: `canonicalDemandRun`). Go borrows an already
/// canonical run; this copies it, which is the same value.
#[must_use]
pub fn canonical_demand_run(mut demands: Vec<EntityDemand>) -> Arc<[EntityDemand]> {
    let mut ordered = true;
    for index in 0..demands.len() {
        if clean(&demands[index].location.path) != *demands[index].location.path {
            demands[index].location.path = clean(&demands[index].location.path).into();
        }
        if let Some(query) = &mut demands[index].query_location
            && clean(&query.path) != *query.path
        {
            query.path = clean(&query.path).into();
        }
        if index == 0 {
            continue;
        }
        let (previous, current) = (&demands[index - 1].location, &demands[index].location);
        if previous.start_byte > current.start_byte
            || (previous.start_byte == current.start_byte && previous.end_byte > current.end_byte)
        {
            ordered = false;
        }
    }
    if !ordered {
        demands.sort_by(|left, right| {
            (left.location.start_byte, left.location.end_byte)
                .cmp(&(right.location.start_byte, right.location.end_byte))
        });
    }
    demands.into()
}

/// The canonical per-file runs of a plain demand list (Go:
/// `sessionChangedDemandRuns`): contiguous path-ordered runs when the request
/// is already canonical, otherwise grouped and path-sorted.
fn session_changed_demand_runs(changes: &[EntityDemand]) -> Vec<(String, Vec<EntityDemand>)> {
    let mut runs: Vec<(String, Vec<EntityDemand>)> = Vec::new();
    let mut canonical = true;
    let mut start = 0;
    while start < changes.len() {
        let path = clean(&changes[start].location.path);
        let mut end = start + 1;
        while end < changes.len() && clean(&changes[end].location.path) == path {
            end += 1;
        }
        if runs.last().is_some_and(|last| last.0 >= path) {
            canonical = false;
            break;
        }
        runs.push((path, changes[start..end].to_vec()));
        start = end;
    }
    if canonical {
        return runs;
    }
    let mut grouped: BTreeMap<String, Vec<EntityDemand>> = BTreeMap::new();
    for demand in changes {
        grouped
            .entry(clean(&demand.location.path))
            .or_default()
            .push(demand.clone());
    }
    grouped.into_iter().collect()
}

/// Sorted paths with duplicates dropped (Go: `compactSortedPaths`).
fn compact_sorted_paths(mut paths: Vec<String>) -> Vec<String> {
    paths.sort();
    paths.dedup();
    paths
}

#[derive(Clone, Debug)]
enum DemandUndo {
    Replace { index: usize, previous: DemandGroup },
    Insert { index: usize },
    Remove { index: usize, previous: DemandGroup },
}

/// The session-owned canonical demand set (Go: `retainedDemandStore`).
#[derive(Debug, Default)]
pub struct RetainedDemandStore {
    pub groups: Vec<DemandGroup>,
}

impl RetainedDemandStore {
    /// Begins a plain-demand transaction (Go: `begin`).
    pub fn begin(
        &mut self,
        changes: &[EntityDemand],
        removed: &[String],
        reset: bool,
    ) -> RetainedDemandTransaction<'_> {
        let changed_runs = session_changed_demand_runs(changes)
            .into_iter()
            .map(|(path, demands)| {
                let path = clean(&path);
                DemandGroup {
                    demands: Some(canonical_demand_run(demands)),
                    path,
                    ..DemandGroup::default()
                }
            })
            .collect();
        self.begin_runs(changed_runs, removed, reset)
    }

    /// Begins a compact-demand transaction (Go: `beginCompact`). A malformed
    /// snapshot is refused before the store is touched.
    pub fn begin_compact(
        &mut self,
        compact: &CompactDemands,
        removed: &[String],
        reset: bool,
    ) -> Result<RetainedDemandTransaction<'_>, String> {
        let strings: Arc<[String]> = compact.strings.clone().into();
        let mut changed_runs = Vec::with_capacity(compact.groups.len());
        for group in &compact.groups {
            let path = usize::try_from(group.0)
                .ok()
                .and_then(|index| strings.get(index))
                .ok_or_else(|| {
                    format!(
                        "compact string index {} out of range ({} strings)",
                        group.0,
                        strings.len()
                    )
                })?;
            compact_demand_shape(std::slice::from_ref(group))?;
            changed_runs.push(DemandGroup {
                path: clean(path),
                compact: Some(CompactBacking {
                    group: group.clone(),
                    strings: Arc::clone(&strings),
                }),
                ..DemandGroup::default()
            });
        }
        changed_runs.sort_by(|left, right| left.path.cmp(&right.path));
        for index in 1..changed_runs.len() {
            if changed_runs[index - 1].path == changed_runs[index].path {
                return Err(format!(
                    "compact demand groups name {} twice; each path may appear once",
                    quote(&changed_runs[index].path)
                ));
            }
        }
        Ok(self.begin_runs(changed_runs, removed, reset))
    }

    fn begin_runs(
        &mut self,
        changed_runs: Vec<DemandGroup>,
        removed: &[String],
        reset: bool,
    ) -> RetainedDemandTransaction<'_> {
        let mut changed_paths: Vec<String> = changed_runs
            .iter()
            .map(|group| group.path.clone())
            .collect();
        changed_paths.extend(removed.iter().map(|path| clean(path)));
        let changed_paths = compact_sorted_paths(changed_paths);

        let reset_old = reset.then(|| std::mem::take(&mut self.groups));
        let mut transaction = RetainedDemandTransaction {
            store: self,
            undo: Vec::new(),
            changed_paths,
            reset_old,
            finished: false,
        };
        for group in changed_runs {
            transaction.replace(group);
        }
        for path in removed {
            transaction.remove(&clean(path));
        }
        transaction
    }

    /// The expanded demands of one path, or `None` when it has none or its
    /// compact run is malformed (Go: `at`).
    pub fn at(&mut self, path: &str) -> Option<Arc<[EntityDemand]>> {
        let path = clean(path);
        let index = self.groups.partition_point(|group| group.path < path);
        let group = self.groups.get_mut(index)?;
        if group.path != path {
            return None;
        }
        group.ensure_expanded().ok()?;
        group.demands.clone()
    }
}

/// One in-flight edit of the demand set. Dropping it without committing
/// restores the set as it was before [`RetainedDemandStore::begin`].
#[derive(Debug)]
pub struct RetainedDemandTransaction<'a> {
    store: &'a mut RetainedDemandStore,
    undo: Vec<DemandUndo>,
    changed_paths: Vec<String>,
    reset_old: Option<Vec<DemandGroup>>,
    finished: bool,
}

impl RetainedDemandTransaction<'_> {
    /// The candidate canonical groups, in path order.
    pub fn groups(&self) -> &[DemandGroup] {
        &self.store.groups
    }

    /// The candidate groups for the closure to materialize (it records hashes
    /// and contributions on them), with every path replaced or removed.
    pub fn parts(&mut self) -> (&mut [DemandGroup], &[String]) {
        (&mut self.store.groups, &self.changed_paths)
    }

    /// Every path whose run was replaced or removed, sorted and unique.
    #[must_use]
    pub fn paths(&self) -> &[String] {
        &self.changed_paths
    }

    fn replace(&mut self, group: DemandGroup) {
        let groups = &mut self.store.groups;
        let index = groups.partition_point(|existing| existing.path < group.path);
        if index < groups.len() && groups[index].path == group.path {
            let previous = std::mem::replace(&mut groups[index], group);
            self.undo.push(DemandUndo::Replace { index, previous });
            return;
        }
        self.undo.push(DemandUndo::Insert { index });
        groups.insert(index, group);
    }

    fn remove(&mut self, path: &str) {
        let groups = &mut self.store.groups;
        let index = groups.partition_point(|existing| existing.path.as_str() < path);
        if index == groups.len() || groups[index].path != path {
            return;
        }
        let previous = groups.remove(index);
        self.undo.push(DemandUndo::Remove { index, previous });
    }

    /// Publishes the edit.
    pub fn commit(mut self) {
        self.finish();
    }

    /// Restores the preceding set (Go: `rollback`); a finished transaction is
    /// left alone.
    pub fn rollback(&mut self) {
        if self.finished {
            return;
        }
        if let Some(old) = self.reset_old.take() {
            self.store.groups = old;
            self.finish();
            return;
        }
        while let Some(undo) = self.undo.pop() {
            let groups = &mut self.store.groups;
            match undo {
                DemandUndo::Replace { index, previous } => groups[index] = previous,
                DemandUndo::Insert { index } => {
                    groups.remove(index);
                }
                DemandUndo::Remove { index, previous } => groups.insert(index, previous),
            }
        }
        self.finish();
    }

    fn finish(&mut self) {
        self.finished = true;
        self.undo.clear();
        self.reset_old = None;
    }
}

impl Drop for RetainedDemandTransaction<'_> {
    fn drop(&mut self) {
        self.rollback();
    }
}
