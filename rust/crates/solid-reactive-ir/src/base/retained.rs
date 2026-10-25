//! Shadow proof of the incremental base: the indexes of the previous build,
//! updated with the files whose views changed, must equal a refold of this
//! build, and each update's Δ must be exactly the keys whose values changed.
//!
//! Runs only with shadow mode on ([`crate::shadow`]), after the build's
//! lookup exists. The build itself never reads the retained copy.

use std::time::{Duration, Instant};

use super::file_table::{FileId, FileInterner, FileTable};
use super::{AllIndexes, Views};
use crate::HashMap;

/// The base kept between builds: its interner (so ids stay stable), each
/// file's view fingerprint, and every index.
pub(crate) struct RetainedBase {
    interner: FileInterner,
    fingerprints: HashMap<FileId, u64>,
    /// Each file's source hash, to tell edited files from files whose
    /// TypeFacts rows alone changed.
    sources: HashMap<FileId, solid_facts::core::SourceHash>,
    indexes: AllIndexes,
}

/// What one shadow check of the base found.
#[derive(Clone, Debug, Default)]
pub(crate) struct BaseCheck {
    /// Recomputing the changed files' contributions and applying them to
    /// every index.
    pub(crate) update: Duration,
    /// Fingerprinting every file to find the changed ones (L0's input; a
    /// TypeFacts delta will name them instead).
    pub(crate) fingerprint: Duration,
    /// The refold the update is compared with.
    pub(crate) refold: Duration,
    pub(crate) changed_files: usize,
    /// Of `changed_files`, those whose source changed: what a TypeFacts
    /// delta naming only edited files would hand L0.
    pub(crate) edited_files: usize,
    /// The update restricted to the edited files (and removed ones), timed
    /// on a copy and not compared: the cost once ids are stable (study S0).
    pub(crate) edited_update: Duration,
    pub(crate) removed_files: usize,
    pub(crate) delta_keys: usize,
    /// `None` when the update equals the refold and the Δ is exact.
    pub(crate) mismatch: Option<String>,
}

/// This build's file table under the retained interner, so every id the
/// retained base knows keeps its meaning.
pub(crate) fn table(
    retained: &Option<RetainedBase>,
    files: &[solid_facts::FileFacts],
) -> FileTable {
    let interner = retained
        .as_ref()
        .map(|base| base.interner.clone())
        .unwrap_or_default();
    FileTable::with_interner(files, interner)
}

/// Checks the retained base against this build's `views` (over
/// [`table`]'s table) and retains this build's. Returns `None` on the first
/// build (nothing to update yet).
pub(crate) fn check(retained: &mut Option<RetainedBase>, views: &Views<'_>) -> Option<BaseCheck> {
    let table = views.table;
    let fingerprint_started = Instant::now();
    let ids = table.ids().to_vec();
    let fingerprints = crate::pipeline::parallel_slice_results(&ids, |id| {
        let mut hasher_input = Vec::with_capacity(table.slots_of(*id).len());
        for slot in table.slots_of(*id) {
            hasher_input.push(views.view(*slot).fingerprint());
        }
        (*id, hasher_input)
    })
    .into_iter()
    .map(|(id, slots)| {
        use std::hash::{Hash, Hasher};
        let mut hasher = rustc_hash::FxHasher::default();
        slots.hash(&mut hasher);
        (id, hasher.finish())
    })
    .collect::<HashMap<_, _>>();
    let fingerprint = fingerprint_started.elapsed();
    let sources = ids
        .iter()
        .map(|id| {
            let slot = table.slots_of(*id)[0];
            (*id, views.files[slot].source_hash.clone())
        })
        .collect::<HashMap<_, _>>();
    let Some(previous) = retained.take() else {
        *retained = Some(RetainedBase {
            interner: table.interner().clone(),
            fingerprints,
            sources,
            indexes: AllIndexes::fold(views),
        });
        return None;
    };
    let mut changed = ids
        .iter()
        .copied()
        .filter(|id| previous.fingerprints.get(id) != fingerprints.get(id))
        .collect::<Vec<_>>();
    changed.sort_unstable();
    let mut removed = previous
        .fingerprints
        .keys()
        .copied()
        .filter(|id| !fingerprints.contains_key(id))
        .collect::<Vec<_>>();
    removed.sort_unstable();
    let edited = changed
        .iter()
        .copied()
        .filter(|id| previous.sources.get(id) != sources.get(id))
        .collect::<Vec<_>>();
    let before = previous.indexes.clone();
    let mut edited_only = previous.indexes.clone();
    let edited_started = Instant::now();
    edited_only.update(views, &edited, &removed);
    let edited_update = edited_started.elapsed();
    drop(edited_only);
    let mut updated = previous.indexes;
    let update_started = Instant::now();
    let deltas = updated.update(views, &changed, &removed);
    let update = update_started.elapsed();
    let refold_started = Instant::now();
    let fresh = AllIndexes::fold(views);
    let refold = refold_started.elapsed();
    let mut mismatch = updated.first_difference(&fresh).map(|index| {
        format!(
            "IR shadow mismatch: base index `{index}`: the update of {} changed and {} removed files differs from a refold",
            changed.len(),
            removed.len()
        )
    });
    if mismatch.is_none() {
        for ((index, mut delta), (_, mut expected)) in
            deltas.iter().cloned().zip(before.changed_keys(&fresh))
        {
            delta.sort();
            expected.sort();
            if delta != expected {
                mismatch = Some(format!(
                    "IR shadow mismatch: base index `{index}`: the update's delta {delta:?} is not the changed keys {expected:?}"
                ));
                break;
            }
        }
    }
    let delta_keys = deltas.iter().map(|(_, keys)| keys.len()).sum();
    *retained = Some(RetainedBase {
        interner: table.interner().clone(),
        fingerprints,
        sources,
        indexes: fresh,
    });
    Some(BaseCheck {
        update,
        fingerprint,
        refold,
        changed_files: changed.len(),
        edited_files: edited.len(),
        edited_update,
        removed_files: removed.len(),
        delta_keys,
        mismatch,
    })
}
