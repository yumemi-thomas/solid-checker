//! Fail-stop panics for the producer process.
//!
//! Release builds unwind, so a panic on one producer thread used to leave the
//! others alive: the reader waited on a client that waited on a dead worker, at
//! 0% CPU, forever. Catching the panic per request is not enough (cleanup and
//! instrumentation run outside any request, a shutdown that joins the writer
//! can block on a full stdout, and unwinding runs request-local compiler
//! destructors, a `Generation` among them, over state the panic left half
//! updated). The hook runs before any unwinding, on whichever thread panicked,
//! so it covers every site (worker, reader, writer, rayon, `thread::scope`).
//!
//! It writes one short line to stderr, then calls `std::process::exit`. Not
//! `abort`: SIGABRT hands the process to the platform crash reporter, which on
//! macOS keeps it alive for seconds and writes a report per panic. `exit` runs
//! only the libc exit handlers and the standard library's flush (which uses a
//! try-lock), no Rust destructor of any thread. The client sees a dead
//! producer (EOF) and its existing crash recovery relaunches and replays, or
//! fails the call; a failed call yields no facts, never a certified result.

/// The exit status of a producer that stopped on a panic (EX_SOFTWARE).
pub const PANIC_EXIT_CODE: i32 = 70;

/// Installs the fail-stop panic hook. Native binaries only; the wasm reactor
/// is already fail-stop (a panic traps and the host poisons the check).
pub fn install() {
    std::panic::set_hook(Box::new(|info| {
        report(&format!(
            "solid-typefacts: fatal panic, exiting {PANIC_EXIT_CODE}: {info}\n"
        ));
        std::process::exit(PANIC_EXIT_CODE);
    }));
}

/// Best effort, bounded: no stderr lock (the panicking thread, or another one
/// blocked on a full pipe, may hold it), a closed stderr tolerated, and a
/// reader that never drains stderr cannot hold the exit up for long.
#[cfg(unix)]
fn report(line: &str) {
    use std::io::Write as _;
    use std::os::fd::AsFd as _;
    let Ok(descriptor) = std::io::stderr().as_fd().try_clone_to_owned() else {
        return;
    };
    let line = line.to_owned();
    let (done_tx, done_rx) = std::sync::mpsc::channel::<()>();
    let spawned = std::thread::Builder::new().spawn(move || {
        let _ = std::fs::File::from(descriptor).write_all(line.as_bytes());
        let _ = done_tx.send(());
    });
    if spawned.is_ok() {
        let _ = done_rx.recv_timeout(std::time::Duration::from_millis(200));
    }
}

#[cfg(not(unix))]
fn report(_line: &str) {}
