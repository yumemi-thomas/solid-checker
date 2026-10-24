//! K4: a sequential pass over totally ordered items whose reads see earlier
//! items' writes, replayed per dirty item in position order (lemma 4″ in the
//! module docs).

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Bound;

use super::{Solved, fresh_cone};

struct Item<I, K, V> {
    input: I,
    /// Each key the item read before writing it, with the value it saw.
    reads: BTreeMap<K, Option<V>>,
    /// Each key's last write by the item.
    writes: BTreeMap<K, V>,
}

/// The last write to `key` before `position`.
fn visible<'s, P: Ord, K: Ord, V>(
    writers: &'s BTreeMap<K, BTreeMap<P, V>>,
    key: &K,
    position: &P,
) -> Option<&'s V> {
    writers
        .get(key)
        .and_then(|writes| writes.range(..position).next_back())
        .map(|(_, value)| value)
}

/// What an item sees and does: reads see the last earlier write (or the
/// item's own), and are recorded with the value seen.
pub(crate) struct Tracer<'s, P, K, V> {
    position: &'s P,
    writers: &'s BTreeMap<K, BTreeMap<P, V>>,
    reads: BTreeMap<K, Option<V>>,
    writes: BTreeMap<K, V>,
}

impl<P: Ord, K: Ord + Clone, V: Clone> Tracer<'_, P, K, V> {
    pub(crate) fn read(&mut self, key: &K) -> Option<V> {
        if let Some(value) = self.writes.get(key) {
            return Some(value.clone());
        }
        let value = visible(self.writers, key, self.position).cloned();
        self.reads
            .entry(key.clone())
            .or_insert_with(|| value.clone());
        value
    }

    pub(crate) fn write(&mut self, key: K, value: V) {
        self.writes.insert(key, value);
    }
}

/// The final state of running every item in position order: each key's
/// last write.
pub(crate) struct OrderedFold<P, I, K, V> {
    items: BTreeMap<P, Item<I, K, V>>,
    writers: BTreeMap<K, BTreeMap<P, V>>,
    readers: BTreeMap<K, BTreeSet<P>>,
}

impl<P, I, K, V> Default for OrderedFold<P, I, K, V> {
    fn default() -> Self {
        Self {
            items: BTreeMap::new(),
            writers: BTreeMap::new(),
            readers: BTreeMap::new(),
        }
    }
}

impl<P, I, K, V> OrderedFold<P, I, K, V>
where
    P: Ord + Clone,
    I: Eq,
    K: Ord + Clone,
    V: Clone + Eq,
{
    /// Applies item changes (`None`: the item is gone; a position listed
    /// twice takes the last) and replays.
    pub(crate) fn update(
        &mut self,
        changes: impl IntoIterator<Item = (P, Option<I>)>,
        run: &impl Fn(&P, &I, &mut Tracer<'_, P, K, V>),
    ) -> Solved<K> {
        self.update_with(changes, run, false)
    }

    /// [`Self::update`], replaying every item when `force_fresh`.
    pub(crate) fn update_with(
        &mut self,
        changes: impl IntoIterator<Item = (P, Option<I>)>,
        run: &impl Fn(&P, &I, &mut Tracer<'_, P, K, V>),
        force_fresh: bool,
    ) -> Solved<K> {
        // Each key whose writes moved, with its final value before.
        let mut touched = BTreeMap::<K, Option<V>>::new();
        let mut dirty = BTreeSet::new();
        let mut suspect = BTreeSet::new();
        for (position, input) in changes {
            match (self.items.get_mut(&position), input) {
                (None, None) => {}
                (Some(item), Some(input)) if item.input == input => {}
                (Some(item), Some(input)) => {
                    item.input = input;
                    dirty.insert(position);
                }
                (None, Some(input)) => {
                    self.items.insert(
                        position.clone(),
                        Item {
                            input,
                            reads: BTreeMap::new(),
                            writes: BTreeMap::new(),
                        },
                    );
                    dirty.insert(position);
                }
                (Some(_), None) => {
                    let item = self.items.remove(&position).expect("a present item");
                    dirty.remove(&position);
                    for key in item.reads.keys() {
                        self.unread(key, &position);
                    }
                    for key in item.writes.keys() {
                        self.unwrite(key, &position, &mut touched, &mut suspect);
                    }
                }
            }
        }
        let fresh = force_fresh || fresh_cone(dirty.len(), self.items.len());
        if fresh {
            for (key, writes) in &self.writers {
                touched
                    .entry(key.clone())
                    .or_insert_with(|| writes.values().next_back().cloned());
            }
            self.writers.clear();
            self.readers.clear();
            for item in self.items.values_mut() {
                item.reads.clear();
                item.writes.clear();
            }
            dirty = self.items.keys().cloned().collect();
            suspect.clear();
        }
        let mut recomputed = 0;
        loop {
            let next_dirty = dirty.first().cloned();
            let next_suspect = suspect.first().cloned();
            let (position, checked) = match (next_dirty, next_suspect) {
                (None, None) => break,
                (Some(position), None) => (position, false),
                (None, Some(position)) => (position, true),
                (Some(left), Some(right)) => {
                    if left <= right {
                        (left, false)
                    } else {
                        (right, true)
                    }
                }
            };
            dirty.remove(&position);
            suspect.remove(&position);
            let Some(item) = self.items.get(&position) else {
                continue;
            };
            if checked
                && item
                    .reads
                    .iter()
                    .all(|(key, seen)| visible(&self.writers, key, &position) == seen.as_ref())
            {
                continue;
            }
            recomputed += 1;
            let mut tracer = Tracer {
                position: &position,
                writers: &self.writers,
                reads: BTreeMap::new(),
                writes: BTreeMap::new(),
            };
            run(&position, &item.input, &mut tracer);
            let Tracer { reads, writes, .. } = tracer;
            let item = self.items.get_mut(&position).expect("a present item");
            let old_reads = std::mem::replace(&mut item.reads, reads);
            let old_writes = std::mem::replace(&mut item.writes, writes.clone());
            for key in old_reads.keys() {
                self.unread(key, &position);
            }
            let item = &self.items[&position];
            for key in item.reads.keys() {
                self.readers
                    .entry(key.clone())
                    .or_default()
                    .insert(position.clone());
            }
            for key in old_writes.keys() {
                if !writes.contains_key(key) {
                    self.unwrite(key, &position, &mut touched, &mut suspect);
                }
            }
            for (key, value) in writes {
                if old_writes.get(&key) == Some(&value) {
                    continue;
                }
                let final_value = self.final_value(&key).cloned();
                touched.entry(key.clone()).or_insert(final_value);
                self.writers
                    .entry(key.clone())
                    .or_default()
                    .insert(position.clone(), value);
                self.dirty_readers(&key, &position, &mut suspect);
            }
        }
        let changed = touched
            .into_iter()
            .filter(|(key, before)| self.final_value(key) != before.as_ref())
            .map(|(key, _)| key)
            .collect();
        Solved {
            changed,
            recomputed,
            fresh,
        }
    }

    fn unread(&mut self, key: &K, position: &P) {
        if let Some(readers) = self.readers.get_mut(key) {
            readers.remove(position);
            if readers.is_empty() {
                self.readers.remove(key);
            }
        }
    }

    fn unwrite(
        &mut self,
        key: &K,
        position: &P,
        touched: &mut BTreeMap<K, Option<V>>,
        suspect: &mut BTreeSet<P>,
    ) {
        let final_value = self.final_value(key).cloned();
        touched.entry(key.clone()).or_insert(final_value);
        if let Some(writes) = self.writers.get_mut(key) {
            writes.remove(position);
            if writes.is_empty() {
                self.writers.remove(key);
            }
        }
        self.dirty_readers(key, position, suspect);
    }

    /// The readers of `key` that saw the write at `position`: after it, up
    /// to and including the next writer (whose read precedes its write).
    fn dirty_readers(&self, key: &K, position: &P, suspect: &mut BTreeSet<P>) {
        let Some(readers) = self.readers.get(key) else {
            return;
        };
        let next_writer = self.writers.get(key).and_then(|writes| {
            writes
                .range((Bound::Excluded(position), Bound::Unbounded))
                .next()
                .map(|(next, _)| next)
        });
        let upper = next_writer.map_or(Bound::Unbounded, Bound::Included);
        for reader in readers.range((Bound::Excluded(position), upper)) {
            suspect.insert(reader.clone());
        }
    }

    /// `key`'s last write.
    pub(crate) fn final_value(&self, key: &K) -> Option<&V> {
        self.writers
            .get(key)
            .and_then(|writes| writes.values().next_back())
    }

    /// Every key's last write, by key.
    pub(crate) fn state(&self) -> BTreeMap<K, V> {
        self.writers
            .iter()
            .filter_map(|(key, writes)| {
                writes
                    .values()
                    .next_back()
                    .map(|value| (key.clone(), value.clone()))
            })
            .collect()
    }

    pub(crate) fn len(&self) -> usize {
        self.items.len()
    }
}
