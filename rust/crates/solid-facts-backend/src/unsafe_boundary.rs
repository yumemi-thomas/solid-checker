//! The only module of this crate allowed to use `unsafe`.
//!
//! Each function is a safe wrapper over one operating-system interface that
//! has no equivalent in `std`; the crate denies `unsafe_code` everywhere else.
#![allow(unsafe_code)]

/// Asks the system allocator to return currently free pages to the OS.
#[cfg(target_os = "macos")]
pub(crate) fn reclaim_allocator_pages() {
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

/// Asks the system allocator to return currently free pages to the OS.
#[cfg(all(target_os = "linux", target_env = "gnu"))]
pub(crate) fn reclaim_allocator_pages() {
    unsafe extern "C" {
        fn malloc_trim(pad: usize) -> i32;
    }

    // SAFETY: malloc_trim is a process-wide glibc allocator maintenance call.
    // Zero asks glibc to retain no additional top-of-heap padding.
    unsafe {
        malloc_trim(0);
    }
}

/// Lowers the calling thread to background priority, so work it starts
/// (including threads and child processes it spawns) yields the CPU to a
/// request being served.
#[cfg(target_os = "macos")]
pub(crate) fn lower_thread_priority() {
    use std::ffi::c_int;

    unsafe extern "C" {
        fn pthread_set_qos_class_self_np(qos_class: u32, relative_priority: c_int) -> c_int;
    }

    // SAFETY: affects only the calling thread's scheduling class; 0x09 is
    // QOS_CLASS_BACKGROUND and a zero relative priority is the class default.
    // A failure leaves the priority unchanged, which is safe.
    unsafe {
        pthread_set_qos_class_self_np(0x09, 0);
    }
}

/// Lowers the calling thread to the lowest niceness.
#[cfg(target_os = "linux")]
pub(crate) fn lower_thread_priority() {
    use std::ffi::c_int;

    unsafe extern "C" {
        fn setpriority(which: c_int, who: u32, priority: c_int) -> c_int;
    }

    // SAFETY: on Linux, `who == 0` with PRIO_PROCESS (0) names the calling
    // thread, so the niceness applies to it alone. A failure changes nothing.
    unsafe {
        setpriority(0, 0, 19);
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub(crate) fn lower_thread_priority() {}

/// The running executable's mapped Mach-O load commands and their count.
///
/// This reads the loaded image, not its replaceable pathname, so a rebuild at
/// the same path cannot change the answer (no safe `std` route reads the
/// image rather than the path).
#[cfg(target_os = "macos")]
pub(crate) fn loaded_executable_load_commands() -> Result<(&'static [u8], u32), String> {
    // SDK mach-o/dyld.h and loader.h: image zero is the executable.
    #[repr(C)]
    struct Header {
        magic: u32,
        cpu_type: i32,
        cpu_subtype: i32,
        file_type: u32,
        commands: u32,
        command_bytes: u32,
        flags: u32,
        reserved: u32,
    }
    unsafe extern "C" {
        fn _dyld_get_image_header(index: u32) -> *const Header;
    }
    // SAFETY: dyld owns the executable header and its mapped load commands for
    // the process lifetime. No caller-supplied pointer or file bytes are read.
    let header =
        unsafe { _dyld_get_image_header(0).as_ref() }.ok_or("missing loaded executable header")?;
    if header.magic != 0xfeed_facf || header.command_bytes > 1024 * 1024 || header.commands > 4096 {
        return Err("unsupported loaded executable header".into());
    }
    // SAFETY: a validated native 64-bit Mach-O header's load commands follow
    // the header in the same dyld mapping; their extent is provided by dyld,
    // which keeps the image mapped for the process lifetime.
    let commands = unsafe {
        std::slice::from_raw_parts(
            (std::ptr::from_ref(header).cast::<u8>()).add(std::mem::size_of::<Header>()),
            header.command_bytes as usize,
        )
    };
    Ok((commands, header.commands))
}

/// Closes every descriptor above standard error this process inherited.
///
/// A retained daemon outlives the client that started it, and a client
/// started by a multithreaded parent can inherit descriptors the parent was
/// still marking close-on-exec (on macOS a pipe is created and marked in two
/// steps). The daemon would then hold another of the parent's children's
/// pipes open for as long as it lives, and that child's reader would never
/// see its end (ADR 0543). Called first thing, before the daemon opens
/// anything of its own.
#[cfg(unix)]
pub(crate) fn close_inherited_descriptors() {
    unsafe extern "C" {
        fn close(descriptor: i32) -> i32;
    }
    let listing = if cfg!(target_os = "linux") {
        "/proc/self/fd"
    } else {
        "/dev/fd"
    };
    // Collected first: the listing holds a descriptor of its own while it
    // is read, closed when it is dropped, before anything is closed here.
    let descriptors = match std::fs::read_dir(listing) {
        Ok(entries) => entries
            .filter_map(|entry| entry.ok()?.file_name().to_str()?.parse::<i32>().ok())
            .filter(|descriptor| *descriptor > 2)
            .collect::<Vec<_>>(),
        Err(_) => return,
    };
    for descriptor in descriptors {
        // SAFETY: closing a descriptor number is sound whatever it names;
        // one that is no longer open (the listing's own) fails with EBADF.
        // Nothing in this process holds these: the daemon calls this before
        // it opens a file, a socket or a thread.
        unsafe {
            close(descriptor);
        }
    }
}
