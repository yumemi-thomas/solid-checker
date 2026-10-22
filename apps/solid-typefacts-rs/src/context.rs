//! Request cancellation, the analogue of Go's `context.Context` with
//! `WithCancel`: cloning shares one flag, and cancelling any clone cancels all.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone, Debug, Default)]
pub struct Context {
    cancelled: Arc<AtomicBool>,
}

impl Context {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    /// Go's `ctx.Err()`: the error text of a cancelled context.
    #[must_use]
    pub fn err(&self) -> Option<&'static str> {
        self.is_cancelled().then_some("context canceled")
    }
}
