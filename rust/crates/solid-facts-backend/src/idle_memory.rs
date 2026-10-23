//! Platform allocator maintenance for a long-lived retained daemon.
//!
//! Analyses release large temporary ownership graphs in bursts. General-
//! purpose allocators keep many of those free pages mapped for reuse, which is
//! useful during a request but wasteful once the response has reached the
//! client. This module keeps the platform-specific mechanism behind one small
//! lifecycle operation.

/// Asks the system allocator to return currently free pages to the OS.
///
/// Call this only after a materialized response has been flushed. Cached
/// responses allocate no comparable temporary graph and should stay off this
/// path.
pub(crate) fn reclaim_idle_pages() {
    reclaim_platform_pages();
}

#[cfg(any(target_os = "macos", all(target_os = "linux", target_env = "gnu")))]
fn reclaim_platform_pages() {
    crate::unsafe_boundary::reclaim_allocator_pages();
}

#[cfg(not(any(target_os = "macos", all(target_os = "linux", target_env = "gnu"))))]
fn reclaim_platform_pages() {}
