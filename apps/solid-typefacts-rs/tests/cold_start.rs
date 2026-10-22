//! Cold-start timing of `open` on a small fixture, Rust (tsrs) against Go.
//! Ignored by default (it measures; it asserts nothing about speed):
//!
//! ```text
//! SOLID_TYPEFACTS_RUST_BIN=target/release/solid-typefacts \
//!   cargo test --release --test cold_start -- --ignored --nocapture
//! ```
//!
//! "open" is the wall time from spawning the process to reading the `open`
//! response; "total" adds close and process exit.

mod common;

use common::{frame, request};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use typefacts::v3::{Operation, Response};

fn read_frame(reader: &mut impl Read) -> Vec<u8> {
    let mut length = [0_u8; 4];
    reader.read_exact(&mut length).unwrap();
    let mut payload = vec![0_u8; u32::from_le_bytes(length) as usize];
    reader.read_exact(&mut payload).unwrap();
    payload
}

fn once(binary: &Path, project: &str) -> (Duration, Duration) {
    let started = Instant::now();
    let mut child = Command::new(binary)
        .args(["-project", project, "-inline-sources"])
        .env_remove("SOLID_TYPEFACTS_TIMINGS")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    let mut output = child.stdout.take().unwrap();
    let _handshake = read_frame(&mut output);
    let mut open = request(1, Operation::Open, 1);
    open.project_id = project.to_owned();
    input.write_all(&frame(&open)).unwrap();
    let response: Response = typefacts::decode(&read_frame(&mut output)).unwrap();
    assert!(response.ok, "{:?}", response.error);
    let opened = started.elapsed();
    let mut close = request(2, Operation::Close, 1);
    close.project_id = project.to_owned();
    input.write_all(&frame(&close)).unwrap();
    let _ = read_frame(&mut output);
    drop(input);
    child.wait().unwrap();
    (opened, started.elapsed())
}

fn median(mut values: Vec<Duration>) -> Duration {
    values.sort();
    values[values.len() / 2]
}

#[test]
#[ignore = "measures; run explicitly"]
fn open_cold_start_against_go() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let go = std::env::var_os("SOLID_TYPEFACTS_GO_BIN")
        .map_or_else(|| root.join("bin/solid-typefacts"), PathBuf::from);
    let rust = std::env::var_os("SOLID_TYPEFACTS_RUST_BIN").map_or_else(
        || PathBuf::from(env!("CARGO_BIN_EXE_solid-typefacts")),
        PathBuf::from,
    );
    let project = root
        .join("fixtures/reactive-ir/inferred-host-alias/tsconfig.json")
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    for (label, binary) in [("go", &go), ("rust (tsrs)", &rust)] {
        let _ = once(binary, &project); // warm the page cache
        let runs: Vec<_> = (0..25).map(|_| once(binary, &project)).collect();
        let open = median(runs.iter().map(|run| run.0).collect());
        let total = median(runs.iter().map(|run| run.1).collect());
        let best = runs.iter().map(|run| run.0).min().unwrap();
        eprintln!("{label:12} open median {open:?} (best {best:?}), total median {total:?}");
    }
}
