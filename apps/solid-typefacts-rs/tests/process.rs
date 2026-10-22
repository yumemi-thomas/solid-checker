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

/// A real process streams an `analyze` transition into the arena the client
/// created, and the committed range decodes as the delta it should be.
#[test]
fn analyze_streams_its_transition_into_the_arena_file() {
    use std::io::{Read, Seek, SeekFrom};
    use typefacts::producer::{TransitionMode, inspect_table_transition};
    use typefacts::v3::{EntityDemand, FileChange};

    let arena = std::env::temp_dir().join(format!(
        "solid-typefacts-rs-process-arena-{}",
        std::process::id()
    ));
    std::fs::write(&arena, b"").unwrap();
    let demand = EntityDemand {
        location: typefacts::Location {
            path: "/p/a.ts".into(),
            start_byte: 2,
            end_byte: 5,
        },
        symbol: true,
        ..EntityDemand::default()
    };
    let mut cold = request(2, Operation::Analyze, 1);
    cold.reset_state = true;
    cold.demands = vec![demand];
    let mut update = request(3, Operation::Update, 2);
    update.changes = vec![FileChange {
        path: "/p/a.ts".to_owned(),
        source: b"changed".to_vec(),
        deleted: false,
        version: 2,
    }];
    let mut warm = request(4, Operation::Analyze, 2);
    warm.state_token = "1".to_owned();
    let input = [
        frame(&request(1, Operation::Open, 1)),
        frame(&cold),
        frame(&update),
        frame(&warm),
        frame(&request(5, Operation::Close, 2)),
    ]
    .concat();
    let arena_flag = format!("-transition-arena={}", arena.display());
    let outcome = run_with(&fake(), &[], &["-project", PROJECT, &arena_flag], &input);
    assert_eq!(outcome.code, Some(0), "{}", outcome.stderr);
    let responses = decode_all(&outcome.stdout);
    assert_eq!(responses.len(), 5);
    let (cold, warm) = (&responses[1], &responses[3]);
    for response in [cold, warm] {
        assert!(response.ok, "{:?}", response.error);
        assert!(response.table_transition.is_empty());
        assert_eq!(response.source_arena, arena.to_str().unwrap());
    }
    // Only the latest transition survives in the file; its descriptor is the
    // warm response's.
    let [request_id, offset, length] = warm.source_lengths[..] else {
        panic!("{:?}", warm.source_lengths);
    };
    assert_eq!(request_id, 4);
    let mut file = std::fs::File::open(&arena).unwrap();
    let mut header = [0_u8; 32];
    file.read_exact(&mut header).unwrap();
    assert_eq!(&header[..8], b"STFARENA");
    assert_eq!(u64::from_le_bytes(header[8..16].try_into().unwrap()), 4);
    let mut bytes = vec![0; usize::try_from(length).unwrap()];
    file.seek(SeekFrom::Start(offset)).unwrap();
    file.read_exact(&mut bytes).unwrap();
    let summary = inspect_table_transition(&bytes).unwrap();
    assert_eq!(summary.mode, TransitionMode::Delta);
    assert_eq!(summary.base_state_token, "1");
    assert_eq!(summary.path_operations, 1);
    // A sparse file: the rows start at 64 MiB and nothing is allocated below.
    assert!(file.metadata().unwrap().len() >= 64 << 20);
    let _ = std::fs::remove_file(arena);
}
