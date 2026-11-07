//! A `OnceLock` that panics when the thread initializing it asks for it
//! again. `std` leaves a re-entrant `get_or_init` unspecified, and its
//! implementation waits on itself forever: three such waits in ADR 0803 step 2
//! (a memo probe's census reaching the memo store's begin, the assigned-member
//! census, the object-member targets) each surfaced only as an executor
//! timeout. A panic names the cell and the chain of cells this thread is
//! initializing, so a fuzz run stops on the first program that reaches one.
//! On in every build: the bookkeeping is one thread-local push per first
//! initialization, and a hang is worse than a crash.
//!
//! It covers re-entry on one thread. Two threads each waiting on a cell the
//! other is initializing would still wait.

use std::cell::RefCell;
use std::panic::Location;
use std::sync::OnceLock;

thread_local! {
    /// The cells this thread is initializing, outermost first, with where
    /// each was asked for.
    static INITIALIZING: RefCell<Vec<(usize, &'static Location<'static>)>> =
        const { RefCell::new(Vec::new()) };
}

/// `OnceLock`'s interface, with re-entry on the initializing thread a panic.
pub(crate) struct Once<T> {
    cell: OnceLock<T>,
}

impl<T> Default for Once<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: std::fmt::Debug> std::fmt::Debug for Once<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.cell.fmt(formatter)
    }
}

/// Pops this thread's innermost initialization, also when it unwinds.
struct Popped;

impl Drop for Popped {
    fn drop(&mut self) {
        INITIALIZING.with(|stack| {
            stack.borrow_mut().pop();
        });
    }
}

impl<T> Once<T> {
    pub(crate) const fn new() -> Self {
        Self {
            cell: OnceLock::new(),
        }
    }

    pub(crate) fn get(&self) -> Option<&T> {
        self.cell.get()
    }

    pub(crate) fn set(&self, value: T) -> Result<(), T> {
        self.cell.set(value)
    }

    #[track_caller]
    pub(crate) fn get_or_init(&self, f: impl FnOnce() -> T) -> &T {
        if let Some(value) = self.cell.get() {
            return value;
        }
        let key = std::ptr::from_ref(self).cast::<()>() as usize;
        let at = Location::caller();
        INITIALIZING.with(|stack| {
            let mut stack = stack.borrow_mut();
            if let Some(first) = stack.iter().position(|(cell, _)| *cell == key) {
                let chain = stack[first..]
                    .iter()
                    .map(|(_, location)| location.to_string())
                    .chain(std::iter::once(at.to_string()))
                    .collect::<Vec<_>>()
                    .join(" -> ");
                drop(stack);
                panic!("a lazily built answer asked for itself while being built: {chain}");
            }
            stack.push((key, at));
        });
        let _popped = Popped;
        self.cell.get_or_init(f)
    }
}

#[cfg(test)]
mod tests {
    use super::Once;

    #[test]
    fn a_cell_asked_for_while_it_is_built_panics_instead_of_waiting() {
        let cell = Once::<u32>::new();
        let reentered = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            *cell.get_or_init(|| *cell.get_or_init(|| 1) + 1)
        }));
        let message = reentered.expect_err("re-entry panics");
        let message = message
            .downcast_ref::<String>()
            .expect("a formatted message");
        assert!(message.contains("asked for itself"), "{message}");
        // The stack unwound: other cells, and this one, still initialize.
        assert_eq!(*Once::<u32>::new().get_or_init(|| 3), 3);
        assert_eq!(*cell.get_or_init(|| 4), 4);
    }

    #[test]
    fn nested_distinct_cells_initialize() {
        let outer = Once::<u32>::new();
        let inner = Once::<u32>::new();
        assert_eq!(*outer.get_or_init(|| *inner.get_or_init(|| 1) + 1), 2);
    }
}
