//! K1: a per-key fold over per-file rows in path order, kept between builds
//! and refolded on the keys a changed file names (study lemma 1).

use std::collections::{BTreeMap, BTreeSet};

use super::super::file_table::FileId;
use super::{FileOrder, Solved};

/// `key -> fold(values)`, the values being every file's rows under the key,
/// by the file's path rank, each file's in production order. The fold is
/// the pass's rule: first wins, last wins, a union, a count.
pub(crate) struct KeyedFold<K, C, V> {
    order: Option<FileOrder>,
    rows: BTreeMap<FileId, Vec<(K, C)>>,
    postings: BTreeMap<K, BTreeSet<FileId>>,
    values: BTreeMap<K, V>,
}

impl<K, C, V> Default for KeyedFold<K, C, V> {
    fn default() -> Self {
        Self {
            order: None,
            rows: BTreeMap::new(),
            postings: BTreeMap::new(),
            values: BTreeMap::new(),
        }
    }
}

/// A file's rows grouped by key, each key's in production order.
fn grouped<K: Ord, C>(rows: &[(K, C)]) -> BTreeMap<&K, Vec<&C>> {
    let mut grouped = BTreeMap::<&K, Vec<&C>>::new();
    for (key, value) in rows {
        grouped.entry(key).or_default().push(value);
    }
    grouped
}

impl<K, C, V> KeyedFold<K, C, V>
where
    K: Ord + Clone,
    C: Eq,
    V: Eq,
{
    /// Brings the fold to `order` with each listed file's rows replaced
    /// (`None`: the file is gone). A file `order` does not list is gone too.
    /// Folded fresh on the first update, or when the files both orders list
    /// changed relative order.
    pub(crate) fn update(
        &mut self,
        order: FileOrder,
        changes: impl IntoIterator<Item = (FileId, Option<Vec<(K, C)>>)>,
        fold: &impl Fn(&K, &[&C]) -> Option<V>,
    ) -> Solved<K> {
        let fresh = self
            .order
            .as_ref()
            .is_none_or(|previous| !order.keeps_relative_order(previous));
        let mut changes = changes.into_iter().collect::<BTreeMap<_, _>>();
        if self.rows.is_empty() && self.postings.is_empty() && self.values.is_empty() {
            return self.fold_all(order, changes, fold, fresh);
        }
        for file in self.rows.keys() {
            if order.rank(*file).is_none() {
                changes.insert(*file, None);
            }
        }
        let mut delta = BTreeSet::new();
        for (file, next) in changes {
            let next = next.filter(|rows| !rows.is_empty());
            let previous = self.rows.remove(&file);
            {
                let old = previous.as_deref().map(grouped).unwrap_or_default();
                let new = next.as_deref().map(grouped).unwrap_or_default();
                for (key, values) in &old {
                    if new.get(key) != Some(values) {
                        delta.insert((*key).clone());
                    }
                    if !new.contains_key(key)
                        && let Some(files) = self.postings.get_mut(*key)
                    {
                        files.remove(&file);
                        if files.is_empty() {
                            self.postings.remove(*key);
                        }
                    }
                }
                for key in new.keys() {
                    if !old.contains_key(key) {
                        delta.insert((*key).clone());
                        self.postings
                            .entry((*key).clone())
                            .or_default()
                            .insert(file);
                    }
                }
            }
            if let Some(next) = next {
                self.rows.insert(file, next);
            }
        }
        if fresh {
            delta.extend(self.postings.keys().cloned());
            delta.extend(self.values.keys().cloned());
        }
        let mut changed = Vec::new();
        for key in &delta {
            let mut files = self
                .postings
                .get(key)
                .map(|files| files.iter().copied().collect::<Vec<_>>())
                .unwrap_or_default();
            files.sort_by_key(|file| order.rank(*file));
            let values = files
                .iter()
                .flat_map(|file| {
                    self.rows[file]
                        .iter()
                        .filter(|(row, _)| row == key)
                        .map(|(_, value)| value)
                })
                .collect::<Vec<_>>();
            let next = if values.is_empty() {
                None
            } else {
                fold(key, &values)
            };
            let differs = match next {
                Some(next) => {
                    let differs = self.values.get(key) != Some(&next);
                    self.values.insert(key.clone(), next);
                    differs
                }
                None => self.values.remove(key).is_some(),
            };
            if differs {
                changed.push(key.clone());
            }
        }
        self.order = Some(order);
        Solved {
            changed,
            recomputed: delta.len(),
            fresh,
        }
    }

    /// [`Self::update`] from nothing kept (a store's first build): every
    /// key is in the delta, so the rows are sorted once by key, path rank,
    /// file and production order -- each key's values in the order the
    /// per-key refold reads them -- instead of each key scanning its files'
    /// rows, and the maps are built from sorted runs.
    fn fold_all(
        &mut self,
        order: FileOrder,
        changes: BTreeMap<FileId, Option<Vec<(K, C)>>>,
        fold: &impl Fn(&K, &[&C]) -> Option<V>,
        fresh: bool,
    ) -> Solved<K> {
        for (file, next) in changes {
            if let Some(rows) = next.filter(|rows| !rows.is_empty()) {
                self.rows.insert(file, rows);
            }
        }
        let mut all = self
            .rows
            .iter()
            .flat_map(|(file, rows)| {
                let rank = order.rank(*file);
                rows.iter()
                    .enumerate()
                    .map(move |(position, (key, value))| (key, rank, *file, position, value))
            })
            .collect::<Vec<_>>();
        all.sort_unstable_by(|a, b| {
            a.0.cmp(b.0)
                .then(a.1.cmp(&b.1))
                .then(a.2.cmp(&b.2))
                .then(a.3.cmp(&b.3))
        });
        let mut postings = Vec::new();
        let mut values = Vec::new();
        let mut changed = Vec::new();
        let mut recomputed = 0;
        for group in all.chunk_by(|a, b| a.0.cmp(b.0).is_eq()) {
            let key = group[0].0;
            recomputed += 1;
            let folded = group.iter().map(|row| row.4).collect::<Vec<_>>();
            if let Some(value) = fold(key, &folded) {
                values.push((key.clone(), value));
                changed.push(key.clone());
            }
            postings.push((
                key.clone(),
                group.iter().map(|row| row.2).collect::<BTreeSet<_>>(),
            ));
        }
        self.postings = postings.into_iter().collect();
        self.values = values.into_iter().collect();
        self.order = Some(order);
        Solved {
            changed,
            recomputed,
            fresh,
        }
    }

    pub(crate) fn values(&self) -> &BTreeMap<K, V> {
        &self.values
    }
}
