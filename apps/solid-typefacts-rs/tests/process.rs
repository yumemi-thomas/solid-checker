//! The real service loop in a real process, over the fixed-answer backend
//! (`solid-typefacts-fake`): crash hooks, timings trace, EOF handling.

mod common;

use common::{frame, frames, request};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use typefacts::v3::{Operation, Response};

const PROJECT: &str = "/p/tsconfig.json";

struct Outcome {
    stdout: Vec<u8>,
    stderr: String,
    code: Option<i32>,
}

fn run_with(binary: &Path, env: &[(&str, &str)], args: &[&str], stdin: &[u8]) -> Outcome {
    let mut command = Command::new(binary);
    command
        .args(args)
        .env_remove("SOLID_TYPEFACTS_TIMINGS")
        .env_remove("SOLID_TYPEFACTS_CRASH_BEFORE_UPDATE")
        .env_remove("SOLID_TYPEFACTS_CRASH_BEFORE_ANALYZE")
        .envs(env.iter().copied())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().unwrap();
    let mut input = child.stdin.take().unwrap();
    let bytes = stdin.to_vec();
    let writer = std::thread::spawn(move || {
        let _ = input.write_all(&bytes);
    });
    let output = child.wait_with_output().unwrap();
    writer.join().unwrap();
    Outcome {
        stdout: output.stdout,
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        code: output.status.code(),
    }
}

fn fake() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_solid-typefacts-fake"))
}

fn script() -> Vec<u8> {
    [
        frame(&request(1, Operation::Open, 1)),
        frame(&request(2, Operation::Update, 2)),
        frame(&request(3, Operation::Close, 2)),
    ]
    .concat()
}

fn decode_all(stdout: &[u8]) -> Vec<Response> {
    frames(stdout)
        .iter()
        .skip(1)
        .map(|payload| typefacts::decode(payload).unwrap())
        .collect()
}

#[test]
fn serves_a_session_and_exits_cleanly_on_eof() {
    let outcome = run_with(&fake(), &[], &["-project", PROJECT], &script());
    assert_eq!(outcome.code, Some(0), "{}", outcome.stderr);
    assert!(outcome.stderr.is_empty(), "{}", outcome.stderr);
    let responses = decode_all(&outcome.stdout);
    assert_eq!(responses.len(), 3);
    assert!(responses.iter().all(|response| response.ok));
    assert_eq!(responses[1].generation, 2);
}

#[test]
fn a_truncated_frame_exits_one_after_answering_what_arrived() {
    let mut input = frame(&request(1, Operation::Open, 1));
    input.extend([9, 0, 0, 0, 1]);
    let outcome = run_with(&fake(), &[], &["-project", PROJECT], &input);
    assert_eq!(outcome.code, Some(1));
    assert_eq!(outcome.stderr, "solid-typefacts: unexpected EOF\n");
    assert_eq!(decode_all(&outcome.stdout).len(), 1);
}

#[test]
fn crash_hook_fires_once_per_marker_and_consumes_it() {
    let directory =
        std::env::temp_dir().join(format!("solid-typefacts-rs-crash-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let marker = directory.join("marker");
    let marker_text = marker.to_string_lossy().into_owned();

    // No marker file: the request runs normally.
    let outcome = run_with(
        &fake(),
        &[("SOLID_TYPEFACTS_CRASH_BEFORE_UPDATE", &marker_text)],
        &["-project", PROJECT],
        &script(),
    );
    assert_eq!(outcome.code, Some(0));

    // Marker present: the process dies before answering the update, having
    // answered the open, and the marker is consumed.
    std::fs::write(&marker, "").unwrap();
    let outcome = run_with(
        &fake(),
        &[("SOLID_TYPEFACTS_CRASH_BEFORE_UPDATE", &marker_text)],
        &["-project", PROJECT],
        &script(),
    );
    assert_eq!(outcome.code, Some(1));
    assert!(!marker.exists());
    assert!(outcome.stderr.is_empty(), "{}", outcome.stderr);
    assert!(decode_all(&outcome.stdout).len() <= 1);

    // The analyze marker leaves update alone.
    std::fs::write(&marker, "").unwrap();
    let outcome = run_with(
        &fake(),
        &[("SOLID_TYPEFACTS_CRASH_BEFORE_ANALYZE", &marker_text)],
        &["-project", PROJECT],
        &script(),
    );
    assert_eq!(outcome.code, Some(0));
    assert!(marker.exists());
    let _ = std::fs::remove_dir_all(&directory);
}

fn stages(stderr: &str) -> Vec<String> {
    stderr
        .lines()
        .map(|line| {
            let (head, tail) = line.split_once("\"elapsedNs\":").expect("stage line");
            assert!(tail.trim_end_matches('}').parse::<u64>().is_ok(), "{line}");
            head.to_owned()
        })
        .collect()
}

#[test]
fn timings_trace_has_the_go_stage_names_and_line_format() {
    let outcome = run_with(
        &fake(),
        &[("SOLID_TYPEFACTS_TIMINGS", "1")],
        &["-project", PROJECT],
        &script(),
    );
    assert_eq!(outcome.code, Some(0));
    let got = stages(&outcome.stderr);
    assert_eq!(
        got.first().map(String::as_str),
        Some("{\"typefactsStage\":\"handshake-written\",")
    );
    assert_eq!(
        got.get(1).map(String::as_str),
        Some("{\"typefactsStage\":\"open\",")
    );
    for name in ["update", "encode-response", "write-response"] {
        assert!(
            got.iter().any(|line| line.contains(&format!("\"{name}\""))),
            "{name} in {got:?}"
        );
    }

    // A set but empty variable disables tracing, as Go's Getenv check does.
    let quiet = run_with(
        &fake(),
        &[("SOLID_TYPEFACTS_TIMINGS", "")],
        &["-project", PROJECT],
        &script(),
    );
    assert!(quiet.stderr.is_empty());
}

/// The same trace against the Go producer on a real project: identical stage
/// names (Go's closure adds its own sub-stages, which are filtered out).
#[test]
fn trace_stage_sequence_matches_go() {
    let go = std::env::var_os("SOLID_TYPEFACTS_GO_BIN").map_or_else(
        || Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bin/solid-typefacts"),
        PathBuf::from,
    );
    if !go.is_file() {
        eprintln!(
            "NOTICE: no Go producer at {}; trace comparison skipped",
            go.display()
        );
        return;
    }
    let directory =
        std::env::temp_dir().join(format!("solid-typefacts-rs-trace-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join("tsconfig.json"), "{\"files\":[\"a.ts\"]}").unwrap();
    std::fs::write(directory.join("a.ts"), "export {};\n").unwrap();
    let project = directory
        .join("tsconfig.json")
        .to_string_lossy()
        .into_owned();
    let input = [
        frame(&{
            let mut r = request(1, Operation::Open, 1);
            r.project_id.clone_from(&project);
            r
        }),
        frame(&{
            let mut r = request(2, Operation::Update, 2);
            r.project_id.clone_from(&project);
            r
        }),
        frame(&{
            let mut r = request(3, Operation::Close, 2);
            r.project_id.clone_from(&project);
            r
        }),
    ]
    .concat();
    let env = [("SOLID_TYPEFACTS_TIMINGS", "1")];
    let go_out = run_with(&go, &env, &["-project", &project], &input);
    let rust_out = run_with(&fake(), &env, &["-project", &project], &input);
    assert_eq!(go_out.code, Some(0), "{}", go_out.stderr);
    assert_eq!(rust_out.code, Some(0), "{}", rust_out.stderr);
    let rust_stages = stages(&rust_out.stderr);
    let go_stages = stages(&go_out.stderr);
    // Go's closure also reports its own sub-stages for a real update; compare
    // only the stages the adapter and framing own.
    let owned = [
        "handshake-written",
        "open",
        "update",
        "encode-response",
        "write-response",
    ];
    let keep = |stages: &[String]| -> Vec<String> {
        stages
            .iter()
            .filter(|line| {
                owned
                    .iter()
                    .any(|name| line.contains(&format!("\"{name}\"")))
            })
            .cloned()
            .collect()
    };
    // Worker and writer threads interleave, so compare as multisets; the
    // first two stages (handshake, open) are ordered by construction.
    let (mut rust_kept, mut go_kept) = (keep(&rust_stages), keep(&go_stages));
    assert_eq!(rust_kept[..2], go_kept[..2]);
    rust_kept.sort();
    go_kept.sort();
    assert_eq!(rust_kept, go_kept);
    let _ = std::fs::remove_dir_all(&directory);
}
