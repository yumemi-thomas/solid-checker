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

use std::any::Any;
use std::cell::Cell;
use std::collections::BTreeMap;
use std::fmt::Debug;
use std::hash::Hash;
use std::sync::{Mutex, RwLock};

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
pub(crate) struct DerivedId(pub(crate) u8);

/// Which kept solver.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct SolverId(pub(crate) u8);

/// A pass's published output: a value per key.
pub(crate) trait Derived: 'static {
    type Key: Clone + Eq + Hash + Debug + Send + Sync + 'static;
    type Value: Canonical;
    const NAME: &'static str;
    const ID: DerivedId;
}

// No memo reads a derived table yet (S7b's will): the reading half is used
// by tests only until then.
#[cfg_attr(not(test), allow(dead_code))]
trait ErasedPublished: Send + Sync {
    fn fingerprint(&self, key: u32) -> u64;
    fn release(&mut self, key: u32);
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

impl<K, V> ErasedPublished for Published<K, V>
where
    K: Clone + Eq + Hash + Send + Sync + 'static,
    V: Clone + Eq + Send + Sync + 'static,
{
    fn fingerprint(&self, key: u32) -> u64 {
        self.key(key).map_or(0, |key| u64::from(self.revision(key)))
    }

    fn release(&mut self, key: u32) {
        Published::release(self, key);
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// One answer of a derived table to a reader.
#[cfg_attr(not(test), allow(dead_code))]
struct Read<V> {
    id: u32,
    value: Option<V>,
    fingerprint: u64,
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

    /// The revision in which a recorded key's value last changed.
    pub(crate) fn fingerprint(&self, id: DerivedId, key: u32) -> u64 {
        self.published
            .read()
            .expect("derived lock")
            .get(&id)
            .map_or(0, |table| table.fingerprint(key))
    }

    /// Forgets a key no memo records any more.
    pub(crate) fn release(&self, id: DerivedId, key: u32) {
        if let Some(table) = self.published.write().expect("derived lock").get_mut(&id) {
            table.release(key);
        }
    }

    fn with_table<D: Derived, R>(
        &self,
        f: impl FnOnce(&mut Published<D::Key, D::Value>) -> R,
    ) -> R {
        let mut published = self.published.write().expect("derived lock");
        let table = published
            .entry(D::ID)
            .or_insert_with(|| Box::new(Published::<D::Key, D::Value>::default()));
        f(table
            .as_any_mut()
            .downcast_mut()
            .unwrap_or_else(|| panic!("derived table {} has one type", D::NAME)))
    }

    #[cfg_attr(not(test), allow(dead_code))]
    fn read<D: Derived>(&self, key: &D::Key, revision: Revision) -> Read<D::Value> {
        {
            let published = self.published.read().expect("derived lock");
            if let Some(table) = published
                .get(&D::ID)
                .and_then(|table| table.as_any().downcast_ref::<Published<D::Key, D::Value>>())
                && let Some(id) = table.id(key)
            {
                return Read {
                    id,
                    value: table.get(key).cloned(),
                    fingerprint: u64::from(table.revision(key)),
                    published_now: table.published_at() == Some(revision),
                };
            }
        }
        self.with_table::<D, _>(|table| Read {
            id: table.intern(key),
            value: table.get(key).cloned(),
            fingerprint: u64::from(table.revision(key)),
            published_now: table.published_at() == Some(revision),
        })
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
        self.record_input_known(S::input(D::ID, read.id), read.fingerprint);
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
