//! The only module of this crate allowed to use `unsafe`.
//!
//! Each function is a safe wrapper over one operating-system call that has no
//! equivalent in `std`; the crate denies `unsafe_code` everywhere else.
#![allow(unsafe_code)]

/// Asks macOS not to keep the file's pages in the unified buffer cache.
///
/// The transition arena is transient transport, not cached state. The Go
/// producer makes the same call on its side; both came with the v6 transport.
#[cfg(target_os = "macos")]
pub(crate) fn disable_file_cache(file: &std::fs::File) -> std::io::Result<()> {
    use std::{ffi::c_int, os::fd::AsRawFd};

    const F_NOCACHE: c_int = 48;
    unsafe extern "C" {
        fn fcntl(file_descriptor: c_int, command: c_int, ...) -> c_int;
    }
    // SAFETY: the descriptor is open for the duration of the borrow, and
    // F_NOCACHE takes one integer argument that it does not retain.
    if unsafe { fcntl(file.as_raw_fd(), F_NOCACHE, 1 as c_int) } == -1 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}
