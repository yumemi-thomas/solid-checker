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

#[cfg(target_os = "macos")]
fn reclaim_platform_pages() {
    use std::ffi::c_void;

    unsafe extern "C" {
        fn malloc_default_zone() -> *mut c_void;
        fn malloc_zone_pressure_relief(zone: *mut c_void, goal: usize) -> usize;
    }

    // SAFETY: both functions are process-wide libSystem allocator operations.
    // The default zone pointer is supplied by the allocator itself and remains
    // valid for the duration of this call. A zero goal requests best effort.
    unsafe {
        malloc_zone_pressure_relief(malloc_default_zone(), 0);
    }
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
fn reclaim_platform_pages() {
    unsafe extern "C" {
        fn malloc_trim(pad: usize) -> i32;
    }

    // SAFETY: malloc_trim is a process-wide glibc allocator maintenance call.
    // Zero asks glibc to retain no additional top-of-heap padding.
    unsafe {
        malloc_trim(0);
    }
}

#[cfg(not(any(target_os = "macos", all(target_os = "linux", target_env = "gnu"))))]
fn reclaim_platform_pages() {}
