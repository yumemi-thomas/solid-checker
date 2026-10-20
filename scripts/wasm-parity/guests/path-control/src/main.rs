//! Raw WASI path controls for the WASM parity gate (S3a).
//!
//! Each argument is one row, `id|op|dirfd|lookup|oflags|fdflags|write|path|buflen`;
//! each row prints one line, `id|errno|detail`. The raw rows call
//! `wasi_snapshot_preview1` directly, so the runtime under test, not Rust's
//! standard library, decides every result; the `std-*` rows go through std.

#[link(wasm_import_module = "wasi_snapshot_preview1")]
unsafe extern "C" {
    fn path_open(
        fd: u32,
        dirflags: u32,
        path: *const u8,
        path_len: usize,
        oflags: u32,
        rights_base: u64,
        rights_inheriting: u64,
        fdflags: u32,
        opened: *mut u32,
    ) -> u32;
    fn path_filestat_get(
        fd: u32,
        flags: u32,
        path: *const u8,
        path_len: usize,
        stat: *mut u8,
    ) -> u32;
    fn path_readlink(
        fd: u32,
        path: *const u8,
        path_len: usize,
        buf: *mut u8,
        buf_len: usize,
        used: *mut usize,
    ) -> u32;
    fn path_create_directory(fd: u32, path: *const u8, path_len: usize) -> u32;
    fn path_remove_directory(fd: u32, path: *const u8, path_len: usize) -> u32;
    fn path_unlink_file(fd: u32, path: *const u8, path_len: usize) -> u32;
    fn fd_close(fd: u32) -> u32;
    fn fd_fdstat_get(fd: u32, stat: *mut u8) -> u32;
    fn fd_fdstat_set_flags(fd: u32, flags: u32) -> u32;
    fn fd_readdir(fd: u32, buf: *mut u8, buf_len: usize, cookie: u64, used: *mut usize) -> u32;
}

const PREOPEN: u32 = 3;
const FOLLOW: u32 = 1;
const O_DIRECTORY: u32 = 2;
// The writing rights: FD_DATASYNC, FD_WRITE, FD_ALLOCATE, FD_FILESTAT_SET_SIZE.
const WRITE_RIGHTS: u64 = (1 << 0) | (1 << 6) | (1 << 8) | (1 << 22);

/// What a row may request: the preopen's inheritable rights, without the
/// writing ones unless the row writes. A request outside them is
/// ENOTCAPABLE before any path is looked at, which no row is about.
fn granted(write: bool) -> u64 {
    let mut stat = [0u8; 24];
    unsafe { fd_fdstat_get(PREOPEN, stat.as_mut_ptr()) };
    let inheritable = u64::from_le_bytes(stat[16..24].try_into().unwrap());
    if write {
        inheritable
    } else {
        inheritable & !WRITE_RIGHTS
    }
}

fn open(
    dirfd: u32,
    lookup: u32,
    path: &str,
    oflags: u32,
    fdflags: u32,
    rights: u64,
) -> Result<u32, u32> {
    let mut opened = 0u32;
    let errno = unsafe {
        path_open(
            dirfd,
            lookup,
            path.as_ptr(),
            path.len(),
            oflags,
            rights,
            rights,
            fdflags,
            &mut opened,
        )
    };
    if errno == 0 { Ok(opened) } else { Err(errno) }
}

fn filetype_of(fd: u32) -> u8 {
    let mut stat = [0u8; 24];
    unsafe { fd_fdstat_get(fd, stat.as_mut_ptr()) };
    stat[0]
}

/// The directory fd a row runs against: the preopen, a directory opened
/// through it, a regular file opened through it, or a closed directory fd.
fn dirfd(spec: &str) -> Result<u32, String> {
    let (kind, path) = spec.split_once('=').unwrap_or((spec, ""));
    let fd = match kind {
        "pre" => return Ok(PREOPEN),
        "dir" | "closed" => open(PREOPEN, FOLLOW, path, O_DIRECTORY, 0, granted(false)),
        "file" => open(PREOPEN, FOLLOW, path, 0, 0, granted(false)),
        _ => return Err(format!("unknown dirfd {spec}")),
    }
    .map_err(|errno| format!("dirfd {spec} failed to open: {errno}"))?;
    if kind == "closed" {
        unsafe { fd_close(fd) };
    }
    Ok(fd)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn row(fields: &[&str]) -> (u32, String) {
    let [_, op, fd, lookup, oflags, fdflags, write, path, buflen] = fields else {
        return (u32::MAX, "malformed row".into());
    };
    let lookup: u32 = lookup.parse().unwrap_or(0);
    let oflags: u32 = oflags.parse().unwrap_or(0);
    let fdflags: u32 = fdflags.parse().unwrap_or(0);
    let rights = granted(*write == "1");
    match *op {
        "std-meta" => match std::fs::metadata(path) {
            Ok(meta) => (0, format!("dir={} file={}", meta.is_dir(), meta.is_file())),
            Err(error) => (error.raw_os_error().unwrap_or(-1) as u32, String::new()),
        },
        "std-readlink" => match std::fs::read_link(path) {
            Ok(target) => (0, format!("len={}", target.as_os_str().len())),
            Err(error) => (error.raw_os_error().unwrap_or(-1) as u32, String::new()),
        },
        "std-canon" => match std::fs::canonicalize(path) {
            Ok(target) => (0, target.display().to_string()),
            Err(error) => (error.raw_os_error().unwrap_or(-1) as u32, String::new()),
        },
        _ => {
            let spec = *fd;
            let fd = match dirfd(spec) {
                Ok(fd) => fd,
                Err(message) => return (u32::MAX - 1, message),
            };
            let result = match *op {
                "stat" => {
                    let mut stat = [0u8; 64];
                    let errno = unsafe {
                        path_filestat_get(fd, lookup, path.as_ptr(), path.len(), stat.as_mut_ptr())
                    };
                    let size = u64::from_le_bytes(stat[32..40].try_into().unwrap());
                    (
                        errno,
                        // Size only for regular files (filetype 4).
                        if errno == 0 && stat[16] == 4 {
                            format!("type={} size={size}", stat[16])
                        } else if errno == 0 {
                            format!("type={}", stat[16])
                        } else {
                            String::new()
                        },
                    )
                }
                "open" => match open(fd, lookup, path, oflags, fdflags, rights) {
                    Ok(opened) => {
                        let filetype = filetype_of(opened);
                        unsafe { fd_close(opened) };
                        (0, format!("type={filetype}"))
                    }
                    Err(errno) => (errno, String::new()),
                },
                "readlink" => {
                    let capacity: usize = buflen.parse().unwrap_or(256);
                    // Eight sentinel bytes before and after, and the buffer itself
                    // pre-filled, so any byte the runtime writes is visible.
                    let mut buffer = vec![0xAAu8; capacity + 16];
                    let mut used = usize::MAX;
                    let errno = unsafe {
                        path_readlink(
                            fd,
                            path.as_ptr(),
                            path.len(),
                            buffer.as_mut_ptr().add(8),
                            capacity,
                            &mut used,
                        )
                    };
                    let before = buffer[..8].iter().all(|&byte| byte == 0xAA);
                    let after = buffer[8 + capacity..].iter().all(|&byte| byte == 0xAA);
                    if errno != 0 {
                        return (errno, format!("before={before} after={after}"));
                    }
                    let placed = used.min(capacity);
                    let untouched = buffer[8 + placed..8 + capacity]
                        .iter()
                        .all(|&byte| byte == 0xAA);
                    (
                        0,
                        format!(
                            "used={used} bytes={} before={before} after={after} untouched={untouched}",
                            hex(&buffer[8..8 + placed])
                        ),
                    )
                }
                "mkdir" => (
                    unsafe { path_create_directory(fd, path.as_ptr(), path.len()) },
                    String::new(),
                ),
                "rmdir" => (
                    unsafe { path_remove_directory(fd, path.as_ptr(), path.len()) },
                    String::new(),
                ),
                "unlink" => (
                    unsafe { path_unlink_file(fd, path.as_ptr(), path.len()) },
                    String::new(),
                ),
                "setflags" => (unsafe { fd_fdstat_set_flags(fd, 0) }, String::new()),
                "readdir" => {
                    // A 64-byte buffer forces continuation through cookies.
                    let mut names = Vec::new();
                    let mut cookie = 0u64;
                    loop {
                        let mut buffer = [0u8; 64];
                        let mut used = 0usize;
                        let errno = unsafe {
                            fd_readdir(fd, buffer.as_mut_ptr(), buffer.len(), cookie, &mut used)
                        };
                        if errno != 0 {
                            return (errno, String::new());
                        }
                        let mut at = 0;
                        let mut progressed = false;
                        while at + 24 <= used {
                            let next = u64::from_le_bytes(buffer[at..at + 8].try_into().unwrap());
                            let length =
                                u32::from_le_bytes(buffer[at + 16..at + 20].try_into().unwrap())
                                    as usize;
                            if at + 24 + length > used {
                                break;
                            }
                            names.push(
                                String::from_utf8_lossy(&buffer[at + 24..at + 24 + length])
                                    .into_owned(),
                            );
                            cookie = next;
                            at += 24 + length;
                            progressed = true;
                        }
                        if used < buffer.len() || !progressed {
                            break;
                        }
                    }
                    names.retain(|name| name != "." && name != "..");
                    names.sort();
                    (0, names.join(","))
                }
                other => (u32::MAX, format!("unknown op {other}")),
            };
            // A row's own directory fd closes with the row: wasi-libc scans
            // fds from 3 for preopens, and stray ones would stop that scan.
            if fd != PREOPEN && !spec.starts_with("closed") {
                unsafe { fd_close(fd) };
            }
            result
        }
    }
}

fn main() {
    for argument in std::env::args().skip(1) {
        let fields: Vec<&str> = argument.split('|').collect();
        let (errno, detail) = row(&fields);
        println!("{}|{errno}|{detail}", fields[0]);
    }
}
