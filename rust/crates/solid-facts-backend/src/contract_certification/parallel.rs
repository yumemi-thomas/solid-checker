//! Bounded fan-out for independent certification work.
//!
//! Two places in the certifier do the same work many times over inputs that
//! share nothing mutable: the graph lanes run one authenticated probe-gate
//! batch per node of a published graph, and every harness census hashes each
//! watched input. Both were sequential, and the wall time of a wide graph row
//! was their sum. `run_each` runs them on a bounded pool of scoped threads and
//! hands the results back *in input order*, so every consumer stays
//! deterministic: which result is applied first, which error is reported, and
//! what the timing report says do not depend on scheduling.
//!
//! What this does not change: each unit still does exactly what it did alone.
//! A probe-gate batch still materializes its own private workspace, launches
//! its sessions one at a time with a census between them, and refuses on its
//! own evidence; the process-wide spawn lock in `probe_harness` keeps one
//! batch's report pipe from being inherited by another batch's child. A census
//! label is a read of immutable bytes. Panics in a worker propagate at the end
//! of the scope, as they would have in the sequential loop.

use std::sync::{
    Mutex, PoisonError,
    atomic::{AtomicUsize, Ordering},
};

/// The worker count for `jobs` independent units: one per available core, and
/// never more than there are units.
#[must_use]
pub(super) fn workers_for(jobs: usize) -> usize {
    std::thread::available_parallelism()
        .map_or(1, usize::from)
        .min(jobs)
        .max(1)
}

/// Runs `run` over every item on at most `workers` scoped threads and returns
/// the results in the items' order.
pub(super) fn run_each<T, R>(items: &[T], workers: usize, run: impl Fn(&T) -> R + Sync) -> Vec<R>
where
    T: Sync,
    R: Send,
{
    if workers <= 1 || items.len() <= 1 {
        return items.iter().map(&run).collect();
    }
    let next = AtomicUsize::new(0);
    let results = Mutex::new(Vec::with_capacity(items.len()));
    std::thread::scope(|scope| {
        for _ in 0..workers.min(items.len()) {
            scope.spawn(|| {
                loop {
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    let Some(item) = items.get(index) else {
                        break;
                    };
                    let result = run(item);
                    results
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .push((index, result));
                }
            });
        }
    });
    let mut results = results.into_inner().unwrap_or_else(PoisonError::into_inner);
    results.sort_by_key(|(index, _)| *index);
    results.into_iter().map(|(_, result)| result).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn results_come_back_in_input_order_whatever_the_schedule() {
        let items = (0..64_u64).collect::<Vec<_>>();
        let results = run_each(&items, 8, |item| {
            std::thread::sleep(std::time::Duration::from_micros(64 - item));
            item * 3
        });
        assert_eq!(
            results,
            items.iter().map(|item| item * 3).collect::<Vec<_>>()
        );
        assert_eq!(run_each(&items[..1], 8, |item| *item), vec![0]);
        assert!(run_each::<u64, u64>(&[], 8, |item| *item).is_empty());
    }

    #[test]
    fn worker_count_is_bounded_by_the_jobs() {
        assert_eq!(workers_for(0), 1);
        assert_eq!(workers_for(1), 1);
        assert!(workers_for(3) <= 3);
    }
}
