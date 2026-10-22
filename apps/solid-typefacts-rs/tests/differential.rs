//! Byte-for-byte comparison against the Go producer (`bin/solid-typefacts`,
//! or `SOLID_TYPEFACTS_GO_BIN`). Without a Go binary these tests print a
//! notice and pass vacuously; `make build-typefacts` provides it.
//!
//! * Startup: handshake bytes, flag/startup error text, exit codes, including
//!   the project-open failures (the tsrs backend must reproduce Go's texts).
//! * Lifecycle: the compiler-free request/response sequence, driven through a
//!   live Go process on a tiny project and through the Rust `Session` on a
//!   fake backend; every response must be identical bytes.

mod common;

use ciborium::Value;
use common::{frame, frames, request};
use solid_typefacts::context::Context;
use solid_typefacts::fake::FakeBackend;
use solid_typefacts::session::Session;
use solid_typefacts::wire::{decode_request, encode_response};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use typefacts::v3::Operation;

fn go_binary() -> Option<PathBuf> {
    let path = std::env::var_os("SOLID_TYPEFACTS_GO_BIN").map_or_else(
        || Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bin/solid-typefacts"),
        PathBuf::from,
    );
    if path.is_file() {
        Some(path)
    } else {
        eprintln!(
            "NOTICE: no Go producer at {}; differential test skipped",
            path.display()
        );
        None
    }
}

fn rust_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_solid-typefacts"))
}

struct Outcome {
    stdout: Vec<u8>,
    stderr: String,
    code: Option<i32>,
}

fn run(binary: &Path, args: &[&str], cwd: &Path, stdin: &[u8]) -> Outcome {
    let mut child = Command::new(binary)
        .args(args)
        .current_dir(cwd)
        .env_remove("SOLID_TYPEFACTS_TIMINGS")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
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

fn scratch(name: &str) -> PathBuf {
    let directory =
        std::env::temp_dir().join(format!("solid-typefacts-rs-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).unwrap();
    directory
}

/// Startup failures that need no project: both producers must agree on the
/// handshake bytes (when one is written), stderr text and exit code.
#[test]
fn startup_failures_match_go() {
    let Some(go) = go_binary() else { return };
    let rust = rust_binary();
    let directory = scratch("startup");
    let cases: Vec<Vec<&str>> = vec![
        vec![],
        vec!["-bogus"],
        vec!["-project"],
        vec!["-project", "/nonexistent/tsconfig.json"],
        vec!["-project", "rel/tsconfig.json"],
        vec!["-project", "/a/b/../c//tsconfig.json"],
        vec!["-schema=2", "-project", "/x"],
        vec!["-schema=abc", "-project", "/x"],
        vec!["-schema=0x1", "-project", "/nonexistent/tsconfig.json"],
        vec!["-inline-sources=maybe", "-project", "/x"],
        vec!["-inline-sources", "-project", "/nonexistent/tsconfig.json"],
        vec!["-h"],
        vec!["--help"],
        vec!["---x"],
        vec!["-=x"],
        vec![
            "-project",
            "/nonexistent/tsconfig.json",
            "-transition-arena",
            "/nonexistent/arena",
        ],
        vec![
            "-project",
            "/nonexistent/tsconfig.json",
            "-cpuprofile",
            "/nonexistent/dir/cpu.prof",
        ],
        vec![
            "-project",
            "/nonexistent/tsconfig.json",
            "-project-config",
            "/nonexistent/config.json",
        ],
    ];
    for args in cases {
        let (go_out, rust_out) = (
            run(&go, &args, &directory, b""),
            run(&rust, &args, &directory, b""),
        );
        assert_eq!(go_out.stdout, rust_out.stdout, "stdout for {args:?}");
        assert_eq!(go_out.stderr, rust_out.stderr, "stderr for {args:?}");
        assert_eq!(go_out.code, rust_out.code, "exit code for {args:?}");
    }
    let _ = std::fs::remove_dir_all(&directory);
}

/// Open failures after the handshake: both producers must report the same
/// text and exit code for configurations that cannot be opened (the tsrs
/// backend reproduces Go's `parse tsconfig: ...` forms).
#[test]
fn open_failures_match_go() {
    let Some(go) = go_binary() else { return };
    let rust = rust_binary();
    let cases: &[(&str, &str)] = &[
        ("invalid-json", "{ not json"),
        ("empty-object", "{}"),
        ("no-inputs", r#"{"include":["nothing/*.ts"]}"#),
        ("empty-files", r#"{"files":[]}"#),
        ("missing-file", r#"{"files":["missing.ts"]}"#),
        (
            "missing-extends",
            r#"{"extends":"./missing.json","files":["a.ts"]}"#,
        ),
        (
            "bad-module",
            r#"{"compilerOptions":{"module":"bogus"},"files":["a.ts"]}"#,
        ),
        (
            "unknown-option",
            r#"{"compilerOptions":{"nope":true},"files":["a.ts"]}"#,
        ),
        (
            "missing-reference",
            r#"{"files":["a.ts"],"references":[{"path":"./missing"}]}"#,
        ),
        ("trailing-comma", r#"{"files":["a.ts",],}"#),
    ];
    for (name, config) in cases {
        let directory = scratch(&format!("open-{name}"));
        std::fs::write(directory.join("tsconfig.json"), config).unwrap();
        std::fs::write(directory.join("a.ts"), "export const a = 1;\n").unwrap();
        let project = directory
            .join("tsconfig.json")
            .to_string_lossy()
            .into_owned();
        let args = ["-project", project.as_str()];
        let (go_out, rust_out) = (
            run(&go, &args, &directory, b""),
            run(&rust, &args, &directory, b""),
        );
        assert_eq!(go_out.stdout, rust_out.stdout, "{name}: stdout");
        assert_eq!(go_out.stderr, rust_out.stderr, "{name}: stderr");
        assert_eq!(go_out.code, rust_out.code, "{name}: exit code");
        // The same bytes through `-project-config` at a path that does not
        // exist on disk.
        let logical = directory
            .join("virtual/tsconfig.json")
            .to_string_lossy()
            .into_owned();
        let bytes = directory.join("config.bytes");
        std::fs::write(&bytes, config).unwrap();
        let bytes = bytes.to_string_lossy().into_owned();
        let args = [
            "-project",
            logical.as_str(),
            "-project-config",
            bytes.as_str(),
        ];
        let (go_out, rust_out) = (
            run(&go, &args, &directory, b""),
            run(&rust, &args, &directory, b""),
        );
        assert_eq!(
            go_out.stderr, rust_out.stderr,
            "{name}: project-config stderr"
        );
        assert_eq!(
            go_out.code, rust_out.code,
            "{name}: project-config exit code"
        );
        let _ = std::fs::remove_dir_all(&directory);
    }
}

/// Opening a real project and reaching EOF: the handshake equals Go's byte for
/// byte, nothing else is written, and the process exits cleanly.
#[test]
fn handshake_is_identical_and_the_idle_process_exits_cleanly() {
    let Some(go) = go_binary() else { return };
    let directory = scratch("handshake");
    std::fs::write(
        directory.join("tsconfig.json"),
        "{\"compilerOptions\":{\"noEmit\":true},\"files\":[\"a.ts\"]}",
    )
    .unwrap();
    std::fs::write(directory.join("a.ts"), "export const a = 1;\n").unwrap();
    let project = directory
        .join("tsconfig.json")
        .to_string_lossy()
        .into_owned();

    let go_out = run(&go, &["-project", &project], &directory, b"");
    let rust_out = run(&rust_binary(), &["-project", &project], &directory, b"");
    assert_eq!(go_out.code, Some(0), "{}", go_out.stderr);
    assert_eq!(
        go_out.stdout.len(),
        4 + 108,
        "Go handshake payload is 108 bytes"
    );
    assert_eq!(rust_out.stdout, go_out.stdout, "handshake bytes");
    assert_eq!(rust_out.code, Some(0), "{}", rust_out.stderr);
    assert_eq!(rust_out.stderr, go_out.stderr);
    let _ = std::fs::remove_dir_all(&directory);
}

fn raw(entries: Vec<(&str, Value)>) -> Vec<u8> {
    let mut entries: Vec<(Value, Value)> = entries
        .into_iter()
        .map(|(key, value)| (Value::Text(key.to_owned()), value))
        .collect();
    entries.sort_by_key(|(key, _)| {
        let mut encoded = Vec::new();
        ciborium::into_writer(key, &mut encoded).unwrap();
        (encoded.len(), encoded)
    });
    let mut payload = Vec::new();
    ciborium::into_writer(&Value::Map(entries), &mut payload).unwrap();
    let mut bytes = u32::try_from(payload.len()).unwrap().to_le_bytes().to_vec();
    bytes.extend(payload);
    bytes
}

fn text(value: &str) -> Value {
    Value::Text(value.to_owned())
}

fn int(value: u64) -> Value {
    Value::Integer(value.into())
}

#[test]
fn compiler_free_lifecycle_responses_match_go_byte_for_byte() {
    let Some(go) = go_binary() else { return };
    let directory = scratch("lifecycle");
    std::fs::write(
        directory.join("tsconfig.json"),
        "{\"compilerOptions\":{\"noEmit\":true},\"files\":[\"a.ts\"]}",
    )
    .unwrap();
    std::fs::write(directory.join("a.ts"), "export const a = 1;\n").unwrap();
    let project = directory
        .join("tsconfig.json")
        .to_string_lossy()
        .into_owned();

    let with = |id: u64,
                operation: Operation,
                generation: u64,
                edit: &dyn Fn(&mut typefacts::v3::Request)| {
        let mut value = request(id, operation, generation);
        value.project_id = project.clone();
        edit(&mut value);
        frame(&value)
    };
    let plain = |id, operation, generation| with(id, operation, generation, &|_| {});
    let loose = |id: u64, operation: &str, generation: u64, schema: u64, project_id: &str| {
        raw(vec![
            ("schema", int(schema)),
            ("operation", text(operation)),
            ("projectId", text(project_id)),
            ("requestId", int(id)),
            ("generation", int(generation)),
        ])
    };
    let script: Vec<Vec<u8>> = vec![
        loose(1, "open", 1, 2, &project),
        loose(0, "open", 1, 1, &project),
        loose(3, "open", 0, 1, &project),
        loose(4, "boom", 1, 1, &project),
        raw(vec![
            ("schema", int(1)),
            ("projectId", text(&project)),
            ("requestId", int(5)),
            ("generation", int(1)),
        ]),
        raw(vec![("schema", int(1))]),
        loose(7, "open", 1, 1, "/elsewhere/tsconfig.json"),
        loose(8, "open", 1, 1, ""),
        plain(9, Operation::Open, 1),
        plain(10, Operation::Open, 2),
        plain(11, Operation::Update, 1),
        plain(12, Operation::Update, 3),
        with(13, Operation::Analyze, 1, &|r| {
            r.compact_demands = Some(typefacts::v3::CompactDemands {
                groups: Vec::new(),
                strings: Vec::new(),
            });
            r.demands = vec![typefacts::v3::EntityDemand::default()];
        }),
        with(14, Operation::Analyze, 1, &|r| {
            r.state_token = "9".to_owned()
        }),
        plain(15, Operation::Analyze, 2),
        plain(16, Operation::Symbols, 1),
        with(17, Operation::Symbols, 1, &|r| {
            r.state_token = "3".to_owned()
        }),
        plain(18, Operation::Symbols, 4),
        plain(19, Operation::Sources, 2),
        plain(20, Operation::Modules, 2),
        plain(21, Operation::Cancel, 1),
        plain(22, Operation::Cancel, 0),
        with(23, Operation::Cancel, 7, &|r| r.cancel_request_id = 999),
        plain(24, Operation::Close, 2),
        plain(25, Operation::Close, 1),
        plain(26, Operation::Close, 1),
        plain(27, Operation::Open, 1),
        plain(28, Operation::Cancel, 1),
        plain(29, Operation::Analyze, 1),
    ];
    let input: Vec<u8> = script.concat();
    let go_out = run(
        &go,
        &["-project", &project, "-inline-sources"],
        &directory,
        &input,
    );
    assert_eq!(go_out.code, Some(0), "{}", go_out.stderr);
    let go_frames = frames(&go_out.stdout);
    assert_eq!(
        go_frames.len(),
        script.len() + 1,
        "handshake plus one response per request"
    );

    let fake = FakeBackend::default();
    let mut session = Session::new(Box::new(fake), &project, true).unwrap();
    for (index, request_frame) in script.iter().enumerate() {
        let incoming = decode_request(&request_frame[4..]).unwrap();
        let mine = encode_response(&session.lifecycle(&Context::new(), &incoming)).unwrap();
        assert_eq!(
            mine,
            go_frames[index + 1],
            "request #{index} (id {}): rust {:?} vs go {:?}",
            incoming.request.request_id,
            typefacts::decode::<ciborium::Value>(&mine).unwrap(),
            typefacts::decode::<ciborium::Value>(&go_frames[index + 1]).unwrap(),
        );
    }
    let _ = std::fs::remove_dir_all(&directory);
}
