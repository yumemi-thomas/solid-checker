//! A pass's solver kept in the build's memo store between builds, checked
//! against a fresh solve in shadow and verify-reused modes.

use std::fmt::Debug;
use std::hash::Hash;

use super::super::memo::build::MemoStore;
use super::super::memo::derived::SolverId;
use super::{Definition, Lattice, Monotone, Solved};

/// The first key whose value differs between two solves, if any.
fn first_difference<K, V, E>(kept: &Monotone<K, V, E>, fresh: &Monotone<K, V, E>) -> Option<String>
where
    K: Clone + Eq + Hash + Debug,
    V: Lattice,
    E: Clone + Eq,
{
    let mut differing = fresh
        .values()
        .filter(|(key, value)| kept.value(key) != Some(*value))
        .map(|(key, _)| format!("{key:?}"))
        .chain(
            kept.values()
                .filter(|(key, _)| fresh.value(key).is_none())
                .map(|(key, _)| format!("{key:?}")),
        )
        .collect::<Vec<_>>();
    differing.sort_unstable();
    differing.into_iter().next()
}

impl MemoStore {
    /// Brings the kept K2 solver `id` to `definitions` (its seeds are the
    /// definitions that differ from the last build's) and hands it to `read`
    /// with what moved. A store that keeps no solver yet solves fresh. In
    /// shadow and verify-reused modes a fresh solve is compared: a
    /// difference names the first differing key, poisons the store, and the
    /// fresh solve is kept and served, every key counted as moved.
    pub(crate) fn solve_monotone<K, V, E, R>(
        &self,
        id: SolverId,
        definitions: Vec<(K, Definition<K, V, E>)>,
        transfer: &impl Fn(&E, &V) -> V,
        read: impl FnOnce(&Monotone<K, V, E>, &Solved<K>) -> R,
    ) -> R
    where
        K: Clone + Eq + Hash + Debug + Send + 'static,
        V: Lattice + Send + 'static,
        E: Clone + Eq + Send + 'static,
    {
        let check = self.check_solvers();
        self.derived()
            .solver::<Monotone<K, V, E>, R>(id, |kept, was_kept| {
                let checked = (check && was_kept).then(|| definitions.clone());
                let mut solved = kept.replace(definitions, transfer);
                solved.fresh |= !was_kept;
                if let Some(definitions) = checked {
                    let mut fresh = Monotone::default();
                    fresh.replace(definitions, transfer);
                    if let Some(key) = first_difference(kept, &fresh) {
                        self.poison(&format!(
                            "IR fixpoint verify mismatch: solver {id:?} key {key}: the kept solve differs from a fresh one"
                        ));
                        let mut changed = kept
                            .values()
                            .map(|(key, _)| key.clone())
                            .collect::<Vec<_>>();
                        changed.extend(
                            fresh
                                .values()
                                .filter(|(key, _)| kept.value(key).is_none())
                                .map(|(key, _)| key.clone()),
                        );
                        *kept = fresh;
                        solved = Solved {
                            recomputed: kept.len(),
                            changed,
                            fresh: true,
                        };
                    }
                }
                read(kept, &solved)
            })
    }
}
