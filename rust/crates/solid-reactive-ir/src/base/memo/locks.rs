//! The memo store's locks. Its critical sections are short and taken by
//! every worker of a parallel stage many times per file; `parking_lot`'s
//! spin-then-park locks keep that from convoying the workers (std's parked
//! hand-off cost the first build's census and resolver stages most of
//! their wall time on many cores). The `Result` signatures keep the call
//! sites' `expect`s: these locks never poison.

use std::convert::Infallible;

#[derive(Default)]
pub(crate) struct Mutex<T>(parking_lot::Mutex<T>);

impl<T> Mutex<T> {
    pub(crate) fn lock(&self) -> Result<parking_lot::MutexGuard<'_, T>, Infallible> {
        Ok(self.0.lock())
    }
}

#[derive(Default)]
pub(crate) struct RwLock<T>(parking_lot::RwLock<T>);

impl<T> RwLock<T> {
    pub(crate) fn read(&self) -> Result<parking_lot::RwLockReadGuard<'_, T>, Infallible> {
        Ok(self.0.read())
    }

    pub(crate) fn write(&self) -> Result<parking_lot::RwLockWriteGuard<'_, T>, Infallible> {
        Ok(self.0.write())
    }
}
