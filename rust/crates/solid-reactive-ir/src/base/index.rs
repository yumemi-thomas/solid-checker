//! L1: a base index, `key -> [(FileId, contributions)]`, folded from per-file
//! contributions and updated exactly.

use std::hash::Hash;

use super::file_table::FileId;
use crate::{HashMap, HashSet};

/// One file's contribution to an index: `(key, value)` entries in the order
/// the contribution function produced them, and where each of the file's
/// slots ends (a path listed twice contributes once per slot).
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FileContribution<K, C> {
    entries: Vec<(K, C)>,
    slot_ends: Vec<u32>,
}

impl<K, C> FileContribution<K, C> {
    /// The contribution of a file whose slots produced `slots`, in file order.
    pub(crate) fn from_slots(slots: impl IntoIterator<Item = Vec<(K, C)>>) -> Self {
        let mut entries = Vec::new();
        let mut slot_ends = Vec::new();
        for slot in slots {
            entries.extend(slot);
            slot_ends.push(u32::try_from(entries.len()).expect("fewer than 2^32 entries"));
        }
        Self { entries, slot_ends }
    }

    /// Which slot of the file produced the entry at `position`.
    fn rank_of(&self, position: u32) -> usize {
        self.slot_ends.partition_point(|end| *end <= position)
    }

    /// The entries the file's `rank`-th slot produced.
    pub(crate) fn slot(&self, rank: usize) -> &[(K, C)] {
        let start = if rank == 0 {
            0
        } else {
            self.slot_ends.get(rank - 1).copied().unwrap_or(0) as usize
        };
        let end = self
            .slot_ends
            .get(rank)
            .copied()
            .map_or(start, |end| end as usize);
        &self.entries[start..end]
    }
}

/// A posting: one file's entries under one key, as positions into its
/// contribution.
type Posting = (FileId, Vec<u32>);

/// `key -> [(FileId, values)]` over every file's contribution.
///
/// The index is a deterministic function of the set of `(FileId,
/// contribution)` pairs: postings are kept sorted by `FileId`, and each lists
/// the file's entries under the key in production order. So replacing one
/// file's contribution ([`Self::update`]) yields exactly the index a refold
/// over the new set yields, and the keys whose values changed are known
/// without comparing anything else.
#[derive(Clone, Debug)]
pub(crate) struct BaseIndex<K, C> {
    by_file: HashMap<FileId, FileContribution<K, C>>,
    by_key: HashMap<K, Vec<Posting>>,
}

impl<K, C> Default for BaseIndex<K, C> {
    fn default() -> Self {
        Self {
            by_file: HashMap::default(),
            by_key: HashMap::default(),
        }
    }
}

impl<K: Eq + Hash, C: PartialEq> PartialEq for BaseIndex<K, C> {
    fn eq(&self, other: &Self) -> bool {
        self.by_file == other.by_file && self.by_key == other.by_key
    }
}

fn positions_by_key<K: Eq + Hash + Clone, C>(
    contribution: &FileContribution<K, C>,
) -> HashMap<K, Vec<u32>> {
    let mut grouped = HashMap::<K, Vec<u32>>::default();
    for (position, (key, _)) in contribution.entries.iter().enumerate() {
        grouped
            .entry(key.clone())
            .or_default()
            .push(u32::try_from(position).expect("fewer than 2^32 entries"));
    }
    grouped
}

impl<K: Eq + Hash + Clone + Send + Sync, C: PartialEq + Sync> BaseIndex<K, C> {
    /// The index of `contributions` (any order; one per file). Each file's
    /// entries are grouped by key on the analysis workers; the postings are
    /// then joined in file order.
    pub(crate) fn fold(
        contributions: impl IntoIterator<Item = (FileId, FileContribution<K, C>)>,
    ) -> Self {
        let mut by_file = contributions.into_iter().collect::<HashMap<_, _>>();
        by_file.retain(|_, contribution| !contribution.entries.is_empty());
        let mut files = by_file.keys().copied().collect::<Vec<_>>();
        files.sort_unstable();
        let grouped = crate::pipeline::parallel_slice_results(&files, |file| {
            positions_by_key(&by_file[file])
        });
        let mut by_key = HashMap::<K, Vec<Posting>>::default();
        for (file, grouped) in files.into_iter().zip(grouped) {
            for (key, positions) in grouped {
                by_key.entry(key).or_default().push((file, positions));
            }
        }
        Self { by_file, by_key }
    }
}

impl<K: Eq + Hash + Clone, C: PartialEq> BaseIndex<K, C> {
    /// Replaces the contribution of each listed file (`None`: the file is
    /// gone; a file listed twice takes the last) and returns the keys whose
    /// values changed: exactly the keys `k` for which [`Self::values`]
    /// answers differently now than before the call.
    pub(crate) fn update(
        &mut self,
        changes: impl IntoIterator<Item = (FileId, Option<FileContribution<K, C>>)>,
    ) -> HashSet<K> {
        // A file listed twice takes its last contribution; each file is
        // then compared once, with what it contributed before this update.
        let mut last = HashMap::<FileId, Option<FileContribution<K, C>>>::default();
        let mut order = Vec::new();
        for (file, next) in changes {
            if last.insert(file, next).is_none() {
                order.push(file);
            }
        }
        let mut delta = HashSet::default();
        for file in order {
            let next = last.remove(&file).expect("each listed file has a change");
            let next = next.filter(|contribution| !contribution.entries.is_empty());
            let previous = self.by_file.remove(&file);
            let old = previous.as_ref().map(positions_by_key).unwrap_or_default();
            let new = next.as_ref().map(positions_by_key).unwrap_or_default();
            for (key, old_positions) in &old {
                let changed = new.get(key).is_none_or(|new_positions| {
                    let next = next
                        .as_ref()
                        .expect("new positions come from a contribution");
                    let previous = previous.as_ref().expect("old positions come from one");
                    !old_positions
                        .iter()
                        .map(|position| &previous.entries[*position as usize].1)
                        .eq(new_positions
                            .iter()
                            .map(|position| &next.entries[*position as usize].1))
                });
                if changed {
                    delta.insert(key.clone());
                }
                if !new.contains_key(key) {
                    let postings = self.by_key.get_mut(key).expect("an indexed key");
                    if let Ok(at) = postings.binary_search_by_key(&file, |posting| posting.0) {
                        postings.remove(at);
                    }
                    if postings.is_empty() {
                        self.by_key.remove(key);
                    }
                }
            }
            for (key, positions) in new {
                if !old.contains_key(&key) {
                    delta.insert(key.clone());
                }
                let postings = self.by_key.entry(key).or_default();
                match postings.binary_search_by_key(&file, |posting| posting.0) {
                    Ok(at) => postings[at].1 = positions,
                    Err(at) => postings.insert(at, (file, positions)),
                }
            }
            if let Some(next) = next {
                self.by_file.insert(file, next);
            }
        }
        delta
    }

    /// The values under `key`, by file in `FileId` order, each file's in
    /// production order.
    pub(crate) fn values<'s>(&'s self, key: &K) -> impl Iterator<Item = (FileId, &'s C)> + 's {
        self.by_key
            .get(key)
            .into_iter()
            .flatten()
            .flat_map(move |(file, positions)| {
                let entries = &self.by_file[file].entries;
                positions
                    .iter()
                    .map(move |position| (*file, &entries[*position as usize].1))
            })
    }

    /// [`Self::values`], each with the rank of the file's slot that
    /// produced it.
    pub(crate) fn ranked_values<'s>(
        &'s self,
        key: &K,
    ) -> impl Iterator<Item = (FileId, usize, &'s C)> + 's {
        self.by_key
            .get(key)
            .into_iter()
            .flatten()
            .flat_map(move |(file, positions)| {
                let contribution = &self.by_file[file];
                positions.iter().map(move |position| {
                    (
                        *file,
                        contribution.rank_of(*position),
                        &contribution.entries[*position as usize].1,
                    )
                })
            })
    }

    pub(crate) fn contains_key(&self, key: &K) -> bool {
        self.by_key.contains_key(key)
    }

    /// Every key with at least one value, in no particular order.
    pub(crate) fn keys(&self) -> impl Iterator<Item = &K> {
        self.by_key.keys()
    }

    /// The entries the `rank`-th slot of `file` produced.
    pub(crate) fn slot(&self, file: FileId, rank: usize) -> &[(K, C)] {
        self.by_file
            .get(&file)
            .map_or(&[], |contribution| contribution.slot(rank))
    }

    /// The keys whose values differ between two indexes: what an update from
    /// one to the other must report.
    pub(crate) fn changed_keys(&self, other: &Self) -> HashSet<K> {
        self.by_key
            .keys()
            .chain(other.by_key.keys())
            .filter(|key| !self.values(key).eq(other.values(key)))
            .cloned()
            .collect()
    }
}
