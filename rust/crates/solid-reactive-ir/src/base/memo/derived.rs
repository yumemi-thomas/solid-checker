//! Derived inputs: a project pass's output, published per key
//! ([`crate::base::fixpoint::Published`]) and read by memos as recorded
//! inputs (S7 design §1, "Publication").
//!
//! A pass runs inside the build, after [`Store::begin`]. It brings its kept
//! solver up to date ([`DerivedTables::solver`]), then publishes the keys
//! whose output moved ([`Store::publish`]): a key whose recomputed value is
//! equal keeps its revision, and each moved key that a memo recorded marks
//! that memo dirty and its readers maybe-dirty, as an index Δ does at
//! `begin`. That is exact only if no reader of the table was asked earlier
//! in the build: a read of a table its pass has not published in this build
//! is a pass-order bug, which fails closed (the store is poisoned, so the
//! next build starts empty) and panics in debug builds. A table is published
//! once per build for the same reason.
//!
//! One reader asks earlier on purpose: a stage chunk's proof
//! ([`crate::base::reads::ReadProof`]) checks its recorded memo reads before
//! the passes run, and bringing a dirty memo up to date may need a table not
//! published yet. That check runs [`deferring`]: such a read marks the
//! computation undecided instead of failing closed, the memo computing it
//! (and every memo above it) installs nothing and answers an error, and the
//! proof counts the read as moved. The chunk reruns, which is exact; its
//! pass publishes before anything reads the table for real.

use super::locks::{Mutex, RwLock};
use std::any::Any;
use std::cell::Cell;
use std::collections::BTreeMap;
use std::fmt::Debug;
use std::hash::Hash;

use super::{Canonical, Ctx, Invalidation, Revision, Schema, Store};
use crate::base::fixpoint::Published;

thread_local! {
    /// A proof is checking recorded reads ([`deferring`]).
    static DEFERRING: Cell<bool> = const { Cell::new(false) };
    /// The current computation read a table not published in this build,
    /// under [`deferring`].
    static UNPUBLISHED: Cell<bool> = const { Cell::new(false) };
}

/// Runs `f` as a proof's check of recorded reads: a memo it brings up to
/// date that reads a derived table not yet published in this build answers
/// an error (undecided) instead of failing closed.
pub(crate) fn deferring<R>(f: impl FnOnce() -> R) -> R {
    let outer = DEFERRING.with(|deferring| deferring.replace(true));
    let value = f();
    DEFERRING.with(|deferring| deferring.set(outer));
    value
}

/// Whether this thread is checking a proof ([`deferring`]): the parallel
/// helpers carry it to their workers.
pub(crate) fn is_deferring() -> bool {
    DEFERRING.with(Cell::get)
}

/// Runs `f` with [`deferring`] set to `on` (a worker of a parallel helper).
pub(crate) fn deferring_as<R>(on: bool, f: impl FnOnce() -> R) -> R {
    if on { deferring(f) } else { f() }
}

/// Runs one memo computation and answers whether it (or one nested in it)
/// read a table not published yet under [`deferring`]; the enclosing
/// computation sees the mark too.
pub(super) fn computing<R>(f: impl FnOnce() -> R) -> (R, bool) {
    let outer = UNPUBLISHED.with(|unpublished| unpublished.replace(false));
    let value = f();
    let unpublished = UNPUBLISHED.with(|mark| mark.replace(outer || mark.get()));
    (value, unpublished)
}

/// Which derived table.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct DerivedId(pub(crate) u16);

/// Which kept solver.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct SolverId(pub(crate) u16);

/// A pass's published output: a value per key.
pub(crate) trait Derived: 'static {
    type Key: Clone + Eq + Hash + Debug + Send + Sync + 'static;
    type Value: Canonical;
    const NAME: &'static str;
    const ID: DerivedId;
}

/// A derived table's keys are split by hash into this many shards, each
/// with its own lock: the workers of a stage reading (and, in a first
/// build, interning) keys of one table rarely wait on each other. A key's
/// id is its shard's id times [`DERIVED_SHARDS`] plus the shard.
const DERIVED_SHARDS: usize = 16;

/// One derived table, sharded ([`DERIVED_SHARDS`]).
struct Sharded<K, V> {
    shards: Box<[RwLock<Published<K, V>>; DERIVED_SHARDS]>,
}

impl<K, V> Default for Sharded<K, V> {
    fn default() -> Self {
        Self {
            shards: Box::new(std::array::from_fn(|_| RwLock::default())),
        }
    }
}

fn shard_of<K: Hash>(key: &K) -> usize {
    use std::hash::Hasher;
    let mut hasher = rustc_hash::FxHasher::default();
    key.hash(&mut hasher);
    // The high bits: FxHash's low bits are weak for small integer keys.
    (hasher.finish() >> 60) as usize % DERIVED_SHARDS
}

fn sharded_id(shard: usize, local: u32) -> u32 {
    local
        .checked_mul(DERIVED_SHARDS as u32)
        .and_then(|id| id.checked_add(shard as u32))
        .expect("fewer than 2^28 derived keys per shard")
}

impl<K: Clone + Eq + Hash, V: Clone + Eq> Sharded<K, V> {
    /// Publishes `changes` in every shard (each records the revision, with
    /// or without a change) and answers the moved keys' ids, sorted.
    fn publish(
        &self,
        revision: Revision,
        changes: impl IntoIterator<Item = (K, Option<V>)>,
    ) -> Vec<u32> {
        let mut parts: [Vec<(K, Option<V>)>; DERIVED_SHARDS] = std::array::from_fn(|_| Vec::new());
        for (key, value) in changes {
            parts[shard_of(&key)].push((key, value));
        }
        let mut moved = Vec::new();
        for (shard, part) in parts.into_iter().enumerate() {
            let local = self.shards[shard]
                .write()
                .expect("derived lock")
                .publish(revision, part);
            moved.extend(local.into_iter().map(|id| sharded_id(shard, id)));
        }
        moved.sort_unstable();
        moved
    }

    fn published_at(&self) -> Option<Revision> {
        self.shards[0].read().expect("derived lock").published_at()
    }

    #[cfg_attr(not(test), allow(dead_code))]
    fn read(&self, key: &K, revision: Revision) -> Read<V> {
        let shard = shard_of(key);
        {
            let table = self.shards[shard].read().expect("derived lock");
            if let Some(id) = table.id(key) {
                return Read {
                    id: sharded_id(shard, id),
                    value: table.get(key).cloned(),
                    published_now: table.published_at() == Some(revision),
                };
            }
        }
        let mut table = self.shards[shard].write().expect("derived lock");
        Read {
            id: sharded_id(shard, table.intern(key)),
            value: table.get(key).cloned(),
            published_now: table.published_at() == Some(revision),
        }
    }
}

// No memo reads a derived table yet (S7b's will): the reading half is used
// by tests only until then.
#[cfg_attr(not(test), allow(dead_code))]
trait ErasedPublished: Send + Sync {
    fn release(&self, key: u32);
    fn as_any(&self) -> &dyn Any;
}

impl<K, V> ErasedPublished for Sharded<K, V>
where
    K: Clone + Eq + Hash + Send + Sync + 'static,
    V: Clone + Eq + Send + Sync + 'static,
{
    fn release(&self, key: u32) {
        let shard = key as usize % DERIVED_SHARDS;
        self.shards[shard]
            .write()
            .expect("derived lock")
            .release(key / DERIVED_SHARDS as u32);
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// One answer of a derived table to a reader.
#[cfg_attr(not(test), allow(dead_code))]
struct Read<V> {
    id: u32,
    value: Option<V>,
    published_now: bool,
}

/// The published tables and kept solvers a store holds between builds.
#[derive(Default)]
pub(crate) struct DerivedTables {
    published: RwLock<BTreeMap<DerivedId, Box<dyn ErasedPublished>>>,
    solvers: Mutex<BTreeMap<SolverId, Box<dyn Any + Send>>>,
}

impl DerivedTables {
    /// Drops every published value and kept solver: the next pass solves
    /// fresh.
    pub(crate) fn clear(&self) {
        self.published.write().expect("derived lock").clear();
        self.solvers.lock().expect("solver lock").clear();
    }

    /// Forgets a key no memo records any more.
    pub(crate) fn release(&self, id: DerivedId, key: u32) {
        if let Some(table) = self.published.read().expect("derived lock").get(&id) {
            table.release(key);
        }
    }

    /// Runs `f` on `D`'s table, created empty if the store has none. The map
    /// is written only to create a table; each table locks its own shards.
    fn with_table<D: Derived, R>(&self, f: impl FnOnce(&Sharded<D::Key, D::Value>) -> R) -> R {
        fn typed<D: Derived>(table: &dyn ErasedPublished) -> &Sharded<D::Key, D::Value> {
            table
                .as_any()
                .downcast_ref()
                .unwrap_or_else(|| panic!("derived table {} has one type", D::NAME))
        }
        {
            let published = self.published.read().expect("derived lock");
            if let Some(table) = published.get(&D::ID) {
                return f(typed::<D>(table.as_ref()));
            }
        }
        let mut published = self.published.write().expect("derived lock");
        let table = published
            .entry(D::ID)
            .or_insert_with(|| Box::new(Sharded::<D::Key, D::Value>::default()));
        f(typed::<D>(table.as_ref()))
    }

    #[cfg_attr(not(test), allow(dead_code))]
    fn read<D: Derived>(&self, key: &D::Key, revision: Revision) -> Read<D::Value> {
        self.with_table::<D, _>(|table| table.read(key, revision))
    }

    /// Runs `f` on the kept solver `id` (a new one, `T::default()`, when the
    /// store keeps none), with whether it was kept.
    pub(crate) fn solver<T: Default + Send + 'static, R>(
        &self,
        id: SolverId,
        f: impl FnOnce(&mut T, bool) -> R,
    ) -> R {
        let mut solvers = self.solvers.lock().expect("solver lock");
        let kept = solvers.contains_key(&id);
        let solver = solvers
            .entry(id)
            .or_insert_with(|| Box::new(T::default()))
            .downcast_mut::<T>()
            .unwrap_or_else(|| panic!("kept solver {id:?} has one type"));
        f(solver, kept)
    }
}

/// A schema whose memos may read derived tables.
pub(crate) trait DerivedSchema: Schema {
    #[cfg_attr(not(test), allow(dead_code))]
    fn derived<'a>(world: &'a Self::World<'_>) -> &'a DerivedTables;
    fn input(id: DerivedId, key: u32) -> Self::Input;
}

#[cfg_attr(not(test), allow(dead_code))]
impl<S: DerivedSchema> Ctx<'_, '_, S> {
    /// `D`'s value at `key`, recorded. Its pass must have published in this
    /// build.
    pub(crate) fn derived<D: Derived>(&self, key: &D::Key) -> Option<D::Value> {
        let read = S::derived(self.world()).read::<D>(key, self.store.revision());
        if !read.published_now && is_deferring() {
            // A proof's check: undecided, not a pass-order bug.
            UNPUBLISHED.with(|unpublished| unpublished.set(true));
        } else if !read.published_now {
            // A pass-order bug. The value is the last build's: the memo
            // installs nothing and answers an error, as under a proof's
            // check, so no reader of this build takes it (final review R2,
            // finding 6b); the store is poisoned for the next.
            UNPUBLISHED.with(|unpublished| unpublished.set(true));
            self.store.poison(&format!(
                "IR derived table `{}` read at {key:?} before its pass published it in this build (failed closed)",
                D::NAME
            ));
            debug_assert!(
                false,
                "derived table {} read before it was published",
                D::NAME
            );
        }
        self.record_input(S::input(D::ID, read.id));
        read.value
    }
}

impl<S: DerivedSchema> Store<S> {
    /// Publishes `changes` to `D` (`None`: the key has no value) and marks
    /// the memos that recorded a moved key.
    pub(crate) fn publish<D: Derived>(
        &self,
        tables: &DerivedTables,
        changes: impl IntoIterator<Item = (D::Key, Option<D::Value>)>,
    ) -> Invalidation {
        let revision = self.revision();
        let moved = tables.with_table::<D, _>(|table| {
            if table.published_at() == Some(revision) {
                self.poison(&format!(
                    "IR derived table `{}` published twice in one build (failed closed)",
                    D::NAME
                ));
                debug_assert!(false, "derived table {} published twice", D::NAME);
            }
            table.publish(revision, changes)
        });
        let inputs = moved
            .into_iter()
            .map(|key| S::input(D::ID, key))
            .collect::<Vec<_>>();
        self.invalidate(&inputs)
    }
}

#[cfg(test)]
mod tests;
