//! ADR 0311: a native invocation whose effective `--dependency-source`
//! selection is nonempty never consults or starts the retained daemon. It
//! runs the one-shot path, which opens a new original program for preparation,
//! drops it, and opens a new analyzed program, so nothing an earlier
//! invocation read answers it.
//!
//! The freshness cases (`fresh_*`) change an input a retained program does not
//! watch, with the selected runtime files and the root configuration equal,
//! and compare what a requested daemon answers with what a disabled one does.
//! The observation that moves is named in each case; it is never a new
//! violation. They hold no claim about a coherent snapshot inside one run, a
//! caller-supplied producer, the retained session library or WASM.
//!
//! Every case runs on a copy of a fixture in a temporary directory and
//! points `TMPDIR` at its own short directory, so "no daemon" is read from that
//! directory and never from an arbitrary user's socket.

#![cfg(unix)]

use std::{
    fs,
    io::{BufRead, BufReader, Write},
    os::unix::{
        fs::{MetadataExt, PermissionsExt},
        net::{UnixListener, UnixStream},
    },
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

const HOST: [&str; 4] = [
    "--runtime-target",
    "browser",
    "--runtime-build",
    "development",
];

fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &to.join(entry.file_name()));
        } else {
            fs::copy(entry.path(), to.join(entry.file_name())).unwrap();
        }
    }
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../fixtures/reactive-ir")
        .join(name)
}

/// A private copy of a fixture, removed when dropped.
struct Scratch {
    root: PathBuf,
    project: PathBuf,
}

impl Scratch {
    fn new(name: &str, tag: &str) -> Self {
        let root = std::env::temp_dir()
            .join("sc-scratch.noindex")
            .join(format!(
                "solid-checker-fresh-native-{tag}-{}",
                std::process::id()
            ));
        let _ = fs::remove_dir_all(&root);
        copy_tree(&fixture(name), &root.join("project"));
        let project = fs::canonicalize(root.join("project")).unwrap();
        Self { root, project }
    }

    fn path(&self, relative: &str) -> PathBuf {
        self.project.join(relative)
    }

    /// The Solid Vite plugin that establishes the resolver the implicit source
    /// selection reads.
    fn with_solid_vite_plugin(self) -> Self {
        let directory = self.project.join("node_modules/@solidjs/vite-plugin");
        fs::create_dir_all(&directory).unwrap();
        fs::write(
            directory.join("package.json"),
            r#"{"name":"@solidjs/vite-plugin","version":"3.0.0-next.44"}"#,
        )
        .unwrap();
        self
    }

    fn siblings(&self) -> Vec<String> {
        fs::read_dir(&self.project)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|name| name.starts_with(".solid-checker-dependency-sources."))
            .collect()
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// A private `TMPDIR`, short enough for a socket path, so a case reads its own
/// daemon, or the absence of one, from it.
struct SocketDir(PathBuf);

impl SocketDir {
    fn new(tag: &str) -> Self {
        let directory = PathBuf::from(format!("/tmp/scf-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();
        Self(directory)
    }

    fn sockets(&self) -> Vec<PathBuf> {
        fs::read_dir(&self.0)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| {
                path.extension()
                    .is_some_and(|extension| extension == "sock")
            })
            .collect()
    }

    fn assert_no_daemon(&self, context: &str) {
        assert!(
            self.sockets().is_empty(),
            "{context}: a daemon socket was created: {:?}",
            self.sockets()
        );
    }
}

impl Drop for SocketDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct Run {
    success: bool,
    stdout: Vec<u8>,
    stderr: String,
}

impl Run {
    fn used_daemon(&self) -> bool {
        self.stderr.contains("\"mode\":\"retained-daemon\"")
    }

    /// The daemon's own report; `None` for a one-shot check.
    fn cache_hit(&self) -> Option<bool> {
        self.stderr.lines().find_map(|line| {
            let value: serde_json::Value = serde_json::from_str(line).ok()?;
            (value["mode"] == "retained-daemon").then(|| value["cacheHit"] == true)
        })
    }

    fn generation(&self) -> Option<u64> {
        self.stderr.lines().find_map(|line| {
            let value: serde_json::Value = serde_json::from_str(line).ok()?;
            (value["mode"] == "retained-daemon").then(|| value["generation"].as_u64())?
        })
    }

    fn notes(&self) -> Vec<&str> {
        self.stderr
            .lines()
            .filter(|line| line.starts_with("solid-checker: note:"))
            .collect()
    }

    fn text(&self) -> String {
        String::from_utf8_lossy(&self.stdout).into_owned()
    }

    /// `rule@file` of every finding, sorted: the named observation of the
    /// freshness cases.
    fn findings(&self) -> Vec<String> {
        let value: serde_json::Value = serde_json::from_slice(&self.stdout).unwrap();
        let mut findings = value["findings"]
            .as_array()
            .unwrap()
            .iter()
            .map(|finding| {
                let path = finding["primaryLocation"]["path"].as_str().unwrap_or("?");
                format!(
                    "{}:{}@{}",
                    finding["id"].as_str().unwrap_or("?"),
                    finding["kind"].as_str().unwrap_or("?"),
                    path.rsplit('/').next().unwrap_or(path)
                )
            })
            .collect::<Vec<_>>();
        findings.sort();
        findings
    }
}

/// One check with exactly `args` after the project. `daemon` sets
/// `SOLID_CHECKER_DAEMON` to `1` or `0`; `sockets` points `TMPDIR` at a
/// private directory; `env` adds variables.
fn invoke(
    typefacts: &str,
    project: &Path,
    args: &[&str],
    daemon: bool,
    sockets: Option<&SocketDir>,
    env: &[(&str, &Path)],
) -> Run {
    let mut command = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"));
    command
        .env("SOLID_TYPEFACTS_BIN", typefacts)
        .env("SOLID_CHECKER_DAEMON", if daemon { "1" } else { "0" })
        .env("SOLID_CHECKER_DAEMON_IDLE_SECS", "20")
        .env("SOLID_CHECKER_TIMINGS", "1")
        .args(["--format", "json", "--project"])
        .arg(project.join("tsconfig.json"))
        .args(args);
    if let Some(sockets) = sockets {
        command.env("TMPDIR", &sockets.0);
    }
    for (key, value) in env {
        command.env(key, value);
    }
    let output = command.output().unwrap();
    Run {
        success: output.status.success(),
        stdout: output.stdout,
        stderr: String::from_utf8(output.stderr).unwrap(),
    }
}

fn with_host(rest: &[&'static str]) -> Vec<&'static str> {
    HOST.iter().chain(rest).copied().collect()
}

fn assert_same(requested: &Run, disabled: &Run, context: &str) {
    assert_eq!(
        requested.text(),
        disabled.text(),
        "{context}: requested and disabled JSON differ (daemon {}, cacheHit {:?}, generation {:?})",
        requested.used_daemon(),
        requested.cache_hit(),
        requested.generation()
    );
    assert_eq!(
        requested.notes(),
        disabled.notes(),
        "{context}: notes differ"
    );
}

/// The claim itself: a requested daemon never answered, announced nothing,
/// and created no socket.
fn assert_one_shot(run: &Run, sockets: &SocketDir, context: &str) {
    assert!(run.success, "{context}: {}", run.stderr);
    assert!(
        !run.stderr.contains("daemon unavailable"),
        "{context}: {}",
        run.stderr
    );
    assert!(!run.used_daemon(), "{context}: {}", run.stderr);
    assert_eq!(run.cache_hit(), None, "{context}");
    sockets.assert_no_daemon(context);
}

macro_rules! armed {
    () => {
        match std::env::var("SOLID_TYPEFACTS_BIN") {
            Ok(path) => path,
            Err(_) => return,
        }
    };
}

// ---------------------------------------------------------------------------
// Routing: every effective nonempty selection is one-shot.
// ---------------------------------------------------------------------------

/// Named, duplicated, mixed with `*`, `*`, a selection that selects nothing,
/// and the implicit default: each requested twice through the daemon switch,
/// each equal to the disabled check, none touching a socket. The JSON request
/// path with a nonempty wired selection is the same.
#[test]
fn every_effective_dependency_selection_bypasses_the_daemon() {
    let typefacts = armed!();
    let scratch = Scratch::new("dependency-source", "bypass").with_solid_vite_plugin();
    let cases: Vec<(&str, Vec<&'static str>)> = vec![
        ("named", with_host(&["--dependency-source", "dep-hooks"])),
        (
            "duplicate",
            with_host(&[
                "--dependency-source",
                "dep-hooks",
                "--dependency-source",
                "dep-hooks",
            ]),
        ),
        (
            "mixed",
            with_host(&[
                "--dependency-source",
                "dep-hooks",
                "--dependency-source",
                "*",
            ]),
        ),
        ("star", with_host(&["--dependency-source", "*"])),
        (
            "selects-nothing",
            with_host(&["--dependency-source", "package-that-is-not-installed"]),
        ),
        // The implicit default (target and build declared, plugin installed,
        // ordinary mode) sets `*` before the daemon branch.
        ("implicit-default", HOST.to_vec()),
    ];
    for (name, args) in &cases {
        let sockets = SocketDir::new(name);
        let disabled = invoke(&typefacts, &scratch.project, args, false, None, &[]);
        assert!(disabled.success, "{name}: {}", disabled.stderr);
        for attempt in ["first", "second"] {
            let requested = invoke(
                &typefacts,
                &scratch.project,
                args,
                true,
                Some(&sockets),
                &[],
            );
            let context = format!("{name} {attempt}");
            assert_one_shot(&requested, &sockets, &context);
            assert_same(&requested, &disabled, &context);
        }
    }
    assert!(scratch.siblings().is_empty(), "{:?}", scratch.siblings());

    // A wired request: nonempty `dependencySources` in the JSON request.
    let request = serde_json::json!({
        "projectId": scratch.path("tsconfig.json"),
        "generation": 1,
        "sources": [],
        "typefactsExecutable": typefacts,
        "dependencySources": ["dep-hooks"],
        "format": "json",
        "runtime": {"target": "browser", "build": "development"},
    })
    .to_string();
    let wired = |daemon: bool, sockets: &SocketDir| {
        let mut child = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
            .env("SOLID_CHECKER_DAEMON", if daemon { "1" } else { "0" })
            .env("SOLID_CHECKER_DAEMON_IDLE_SECS", "20")
            .env("SOLID_CHECKER_TIMINGS", "1")
            .env("TMPDIR", &sockets.0)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(request.as_bytes())
            .unwrap();
        let output = child.wait_with_output().unwrap();
        Run {
            success: output.status.success(),
            stdout: output.stdout,
            stderr: String::from_utf8(output.stderr).unwrap(),
        }
    };
    let sockets = SocketDir::new("wired");
    let disabled = wired(false, &sockets);
    let requested = wired(true, &sockets);
    assert_one_shot(&requested, &sockets, "wired");
    assert_same(&requested, &disabled, "wired");
}

/// What stays retained: a check with no effective selection, built from the
/// explicit opt-out or from a verified non-defaulting request (a target
/// alone, a build alone, no host), takes the real cold then warm daemon.
#[test]
fn a_check_with_no_effective_selection_keeps_its_daemon() {
    let typefacts = armed!();
    let scratch = Scratch::new("dependency-source", "base").with_solid_vite_plugin();
    let cases: Vec<(&str, Vec<&'static str>)> = vec![
        ("opt-out", with_host(&["--no-dependency-source"])),
        ("target-only", vec!["--runtime-target", "browser"]),
        ("build-only", vec!["--runtime-build", "development"]),
        ("no-host", vec![]),
    ];
    for (name, args) in &cases {
        let sockets = SocketDir::new(name);
        let disabled = invoke(&typefacts, &scratch.project, args, false, None, &[]);
        assert!(disabled.success, "{name}: {}", disabled.stderr);
        // The case really is base: no dependency-source note, and the
        // opted-out answer is this answer.
        assert!(
            !disabled.stderr.contains("dependency source"),
            "{name}: {}",
            disabled.stderr
        );
        let cold = invoke(
            &typefacts,
            &scratch.project,
            args,
            true,
            Some(&sockets),
            &[],
        );
        let warm = invoke(
            &typefacts,
            &scratch.project,
            args,
            true,
            Some(&sockets),
            &[],
        );
        assert!(
            cold.success && warm.success,
            "{name}: {}\n{}",
            cold.stderr,
            warm.stderr
        );
        assert_eq!(cold.cache_hit(), Some(false), "{name}: {}", cold.stderr);
        assert_eq!(warm.cache_hit(), Some(true), "{name}: {}", warm.stderr);
        assert_eq!(sockets.sockets().len(), 1, "{name}");
        assert_same(&cold, &disabled, name);
        assert_same(&warm, &disabled, name);
    }
}

/// A server cannot be started for a selection. The refusal comes before the
/// live-socket early success, the bind, any state and any producer, so the
/// producer named here, which records that it ran, never does.
#[test]
fn a_direct_serve_with_a_selection_refuses_before_a_producer_or_a_socket() {
    let _ = armed!();
    let scratch = Scratch::new("dependency-source", "serve");
    let marker = scratch.root.join("producer-ran");
    let producer = scratch.root.join("poison-typefacts");
    fs::write(
        &producer,
        format!("#!/bin/sh\ntouch '{}'\nexit 1\n", marker.display()),
    )
    .unwrap();
    fs::set_permissions(&producer, fs::Permissions::from_mode(0o755)).unwrap();
    for selection in [
        &["--dependency-source", "dep-hooks"][..],
        &["--dependency-source", "*"][..],
        &[
            "--dependency-source",
            "dep-hooks",
            "--dependency-source",
            "dep-hooks",
        ][..],
    ] {
        let sockets = SocketDir::new("serve");
        let mut child = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
            .env("SOLID_TYPEFACTS_BIN", &producer)
            .env("SOLID_CHECKER_DAEMON_IDLE_SECS", "20")
            .env("TMPDIR", &sockets.0)
            .args(["--serve", "--format", "json", "--project"])
            .arg(scratch.path("tsconfig.json"))
            .args(HOST)
            .args(selection)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let started = Instant::now();
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break Some(status);
            }
            if started.elapsed() > Duration::from_secs(15) {
                let _ = child.kill();
                break None;
            }
            std::thread::sleep(Duration::from_millis(50));
        };
        let output = child.wait_with_output().unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        let context = format!("{selection:?}: {stderr}");
        assert!(status.is_some(), "serve ran instead of refusing: {context}");
        assert!(!status.unwrap().success(), "{context}");
        assert!(stderr.contains("one-shot check"), "{context}");
        assert!(!marker.exists(), "a producer ran: {context}");
        assert!(
            fs::read_dir(&sockets.0).unwrap().next().is_none(),
            "a socket or program was created: {context}"
        );
    }
}

/// A base server refuses a wire request that carries a selection, through the
/// existing mismatch check, and never reaches a retained worker.
#[test]
fn a_base_daemon_refuses_a_wire_request_carrying_a_selection() {
    let typefacts = armed!();
    let scratch = Scratch::new("dependency-source", "wire");
    let sockets = SocketDir::new("wire");
    let args = with_host(&["--no-dependency-source"]);
    let cold = invoke(
        &typefacts,
        &scratch.project,
        &args,
        true,
        Some(&sockets),
        &[],
    );
    assert_eq!(cold.cache_hit(), Some(false), "{}", cold.stderr);
    let socket = sockets.sockets().into_iter().next().unwrap();
    let daemon = sockets.0.join("daemon.sock.real");
    fs::rename(&socket, &daemon).unwrap();
    let proxy = UnixListener::bind(&socket).unwrap();
    let relay = std::thread::spawn(move || {
        let (client, _) = proxy.accept().unwrap();
        let mut line = String::new();
        BufReader::new(client.try_clone().unwrap())
            .read_line(&mut line)
            .unwrap();
        let mut request: serde_json::Value = serde_json::from_str(&line).unwrap();
        request["dependencySources"] = serde_json::json!(["dep-hooks"]);
        let mut upstream = UnixStream::connect(&daemon).unwrap();
        upstream
            .write_all(format!("{request}\n").as_bytes())
            .unwrap();
        let mut header = String::new();
        BufReader::new(upstream).read_line(&mut header).unwrap();
        let mut client = client;
        client.write_all(header.as_bytes()).unwrap();
        header
    });
    let refused = invoke(
        &typefacts,
        &scratch.project,
        &args,
        true,
        Some(&sockets),
        &[],
    );
    let header = relay.join().unwrap();
    assert!(header.contains("\"ok\":false"), "{header}");
    assert!(
        header.contains("daemon serves a different dependency selection"),
        "{header}"
    );
    // The client then answers one-shot and says why.
    assert!(refused.success, "{}", refused.stderr);
    assert!(
        refused
            .stderr
            .contains("daemon serves a different dependency selection"),
        "{}",
        refused.stderr
    );
}

// ---------------------------------------------------------------------------
// Freshness: an input a retained program does not watch.
// ---------------------------------------------------------------------------

const DECLARATION: &str = "node_modules/dep-hooks/dist/index.d.ts";
const HELPER: &str = "import { createSignal } from \"solid-js\";\nimport { useLabel } from \"dep-hooks\";\n\nexport function HelperLabel() {\n  const [name] = createSignal(\"a\");\n  const label = useLabel(name);\n  return <p>{label}</p>;\n}\n";

fn dependency_args() -> Vec<&'static str> {
    with_host(&["--dependency-source", "dep-hooks"])
}

fn append(path: &Path, text: &str) {
    let mut file = fs::OpenOptions::new().append(true).open(path).unwrap();
    file.write_all(text.as_bytes()).unwrap();
}

fn mtime(path: &Path) -> std::time::SystemTime {
    fs::metadata(path).unwrap().modified().unwrap()
}

/// `prepare`, then: cold requested check, a benign project edit (the retained
/// generation a daemon would hold is now 2), `mutate`, a requested check and
/// a disabled one, `restore`, a requested and a disabled one. The named
/// observation of `findings` must move between the disabled checks and must be
/// the disabled one after the mutation, which is exactly what a retained
/// program that does not watch the mutated input could not answer.
fn fresh_case(
    typefacts: &str,
    tag: &str,
    prepare: impl FnOnce(&Scratch),
    mutate: impl FnOnce(&Scratch),
    restore: impl FnOnce(&Scratch),
) -> (Vec<String>, Vec<String>) {
    let scratch = Scratch::new("dependency-source", tag);
    let sockets = SocketDir::new(tag);
    prepare(&scratch);
    let args = dependency_args();
    let requested = || {
        invoke(
            typefacts,
            &scratch.project,
            &args,
            true,
            Some(&sockets),
            &[],
        )
    };
    let disabled = || invoke(typefacts, &scratch.project, &args, false, None, &[]);

    let before = requested();
    let fresh_before = disabled();
    assert!(before.success && fresh_before.success, "{}", before.stderr);
    assert_same(&before, &fresh_before, &format!("{tag} before"));
    append(&scratch.path("App.tsx"), "// pad\n");
    let padded = requested();
    assert_same(&padded, &fresh_before, &format!("{tag} benign edit"));

    mutate(&scratch);
    let after = requested();
    let fresh_after = disabled();
    assert!(after.success && fresh_after.success, "{}", after.stderr);
    // The mutation matters: the disabled observation moved.
    assert_ne!(
        fresh_before.findings(),
        fresh_after.findings(),
        "{tag}: the mutation moved nothing"
    );
    // The decisive comparison. A retained daemon answered the stale
    // observation here, with its cache and generation in the message.
    assert_eq!(
        after.findings(),
        fresh_after.findings(),
        "{tag}: the requested answer is stale (daemon {}, cacheHit {:?}, generation {:?})",
        after.used_daemon(),
        after.cache_hit(),
        after.generation()
    );
    assert_same(&after, &fresh_after, &format!("{tag} after"));
    assert_one_shot(&after, &sockets, &format!("{tag} after"));

    restore(&scratch);
    let restored = requested();
    let fresh_restored = disabled();
    assert_same(&restored, &fresh_restored, &format!("{tag} restored"));
    assert_eq!(
        restored.findings(),
        fresh_before.findings(),
        "{tag}: the restoration did not return the earlier observation"
    );
    assert_one_shot(&restored, &sockets, &format!("{tag} restored"));
    (fresh_before.findings(), fresh_after.findings())
}

/// RF1a. The declaration is edited in place (same inode) into a re-export of
/// the package's own `dist/index.js`. The selected runtime files, the manifest
/// and the root configuration are byte-equal, and no tracked directory stamp
/// moves. The binding of the declaration to the selected runtime source is
/// what the fresh program no longer has, so the two reads of `useLabel` become
/// uncertifiable instead of violations; no violation is added.
#[test]
fn fresh_an_in_place_declaration_edit_is_read_fresh() {
    let typefacts = armed!();
    let original = std::cell::RefCell::new(Vec::new());
    let (before, after) = fresh_case(
        &typefacts,
        "decl-edit",
        |scratch| {
            *original.borrow_mut() = fs::read(scratch.path(DECLARATION)).unwrap();
        },
        |scratch| {
            let inode = fs::metadata(scratch.path(DECLARATION)).unwrap().ino();
            let manifest = fs::read(scratch.path("node_modules/dep-hooks/package.json")).unwrap();
            let runtime = fs::read(scratch.path("node_modules/dep-hooks/src/index.jsx")).unwrap();
            let config = fs::read(scratch.path("tsconfig.json")).unwrap();
            let tracked = (
                mtime(&scratch.path(".")),
                mtime(&scratch.path("node_modules/dep-hooks/src")),
                mtime(&scratch.path("node_modules/dep-hooks/dist")),
            );
            fs::write(scratch.path(DECLARATION), "export * from \"./index.js\";\n").unwrap();
            assert_eq!(
                fs::metadata(scratch.path(DECLARATION)).unwrap().ino(),
                inode
            );
            assert_eq!(
                fs::read(scratch.path("node_modules/dep-hooks/package.json")).unwrap(),
                manifest
            );
            assert_eq!(
                fs::read(scratch.path("node_modules/dep-hooks/src/index.jsx")).unwrap(),
                runtime
            );
            assert_eq!(fs::read(scratch.path("tsconfig.json")).unwrap(), config);
            // In-place content edits move no directory stamp.
            assert_eq!(mtime(&scratch.path(".")), tracked.0);
            assert_eq!(
                mtime(&scratch.path("node_modules/dep-hooks/src")),
                tracked.1
            );
            assert_eq!(
                mtime(&scratch.path("node_modules/dep-hooks/dist")),
                tracked.2
            );
        },
        |scratch| fs::write(scratch.path(DECLARATION), &*original.borrow()).unwrap(),
    );
    assert!(
        before
            .iter()
            .filter(|finding| finding.starts_with("SC1001"))
            .count()
            > after
                .iter()
                .filter(|finding| finding.starts_with("SC1001"))
                .count(),
        "{before:?} {after:?}"
    );
}

/// RF1b. The declaration is deleted. Unlink changes its parent directory's
/// stamp, so this is a separate case: the parent (`dist`) is an untracked
/// directory, and the project directory and the selected sources' directory
/// are tracked and stay equal. TypeScript recovers (it falls back to the
/// package's `default` build), so nothing here asserts an error.
#[test]
fn fresh_a_deleted_declaration_is_read_fresh() {
    let typefacts = armed!();
    let original = std::cell::RefCell::new(Vec::new());
    let (before, after) = fresh_case(
        &typefacts,
        "decl-delete",
        |scratch| {
            *original.borrow_mut() = fs::read(scratch.path(DECLARATION)).unwrap();
        },
        |scratch| {
            let manifest = fs::read(scratch.path("node_modules/dep-hooks/package.json")).unwrap();
            let runtime = fs::read(scratch.path("node_modules/dep-hooks/src/index.jsx")).unwrap();
            let project = mtime(&scratch.path("."));
            let sources = mtime(&scratch.path("node_modules/dep-hooks/src"));
            let dist = mtime(&scratch.path("node_modules/dep-hooks/dist"));
            fs::remove_file(scratch.path(DECLARATION)).unwrap();
            assert_eq!(mtime(&scratch.path(".")), project);
            assert_eq!(mtime(&scratch.path("node_modules/dep-hooks/src")), sources);
            assert_ne!(
                mtime(&scratch.path("node_modules/dep-hooks/dist")),
                dist,
                "an unlinked file moves its parent's stamp, which is untracked"
            );
            assert_eq!(
                fs::read(scratch.path("node_modules/dep-hooks/package.json")).unwrap(),
                manifest
            );
            assert_eq!(
                fs::read(scratch.path("node_modules/dep-hooks/src/index.jsx")).unwrap(),
                runtime
            );
        },
        |scratch| fs::write(scratch.path(DECLARATION), &*original.borrow()).unwrap(),
    );
    assert_ne!(before, after);
}

/// RF2. `lib/H.tsx` already exists, outside every configured root and imported
/// by nothing. Only an in-place edit of the package's declaration imports it,
/// which puts it in a fresh original program's inventory, hence in the
/// analyzed program's files; restoring the declaration removes it. The
/// observation is that `H.tsx` is analyzed at all, with the project's
/// configuration, the selected runtime and the root files unchanged.
#[test]
fn fresh_a_helper_acquired_through_a_declaration_edit_enters_and_leaves_the_inventory() {
    let typefacts = armed!();
    let original = std::cell::RefCell::new(Vec::new());
    let (before, after) = fresh_case(
        &typefacts,
        "helper",
        |scratch| {
            fs::create_dir_all(scratch.path("lib")).unwrap();
            fs::write(scratch.path("lib/H.tsx"), HELPER).unwrap();
            *original.borrow_mut() = fs::read(scratch.path(DECLARATION)).unwrap();
        },
        |scratch| {
            let inode = fs::metadata(scratch.path(DECLARATION)).unwrap().ino();
            let config = fs::read(scratch.path("tsconfig.json")).unwrap();
            append(
                &scratch.path(DECLARATION),
                "export type { HelperLabel } from \"../../../lib/H\";\n",
            );
            assert_eq!(
                fs::metadata(scratch.path(DECLARATION)).unwrap().ino(),
                inode
            );
            assert_eq!(fs::read(scratch.path("tsconfig.json")).unwrap(), config);
        },
        |scratch| fs::write(scratch.path(DECLARATION), &*original.borrow()).unwrap(),
    );
    assert!(
        !before.iter().any(|finding| finding.ends_with("@H.tsx")),
        "{before:?}"
    );
    assert!(
        after.iter().any(|finding| finding.ends_with("@H.tsx")),
        "{after:?}"
    );
}

/// RF3. The project's tsconfig extends a base configuration. An in-place edit
/// of the base (not the root config, which a retained program fingerprints)
/// adds `lib/*.tsx` to `include`. That the extended configuration is really
/// consulted is the observation: `H.tsx` becomes an analyzed file, with the
/// root configuration's and the runtime's bytes equal.
#[test]
fn fresh_an_extended_configuration_edit_is_read_fresh() {
    let typefacts = armed!();
    let base_before = "{\n  \"compilerOptions\": {\n    \"jsx\": \"preserve\", \"module\": \"ESNext\", \"moduleResolution\": \"Bundler\",\n    \"strict\": true, \"target\": \"ES2022\"\n  },\n  \"include\": [\"*.ts\", \"*.tsx\"]\n}\n";
    let base_after = base_before.replace("\"*.tsx\"]", "\"*.tsx\", \"lib/*.tsx\"]");
    assert_ne!(base_before, base_after);
    let (before, after) = fresh_case(
        &typefacts,
        "extended",
        |scratch| {
            fs::create_dir_all(scratch.path("lib")).unwrap();
            fs::write(scratch.path("lib/H.tsx"), HELPER).unwrap();
            fs::write(scratch.path("tsconfig.base.json"), base_before).unwrap();
            fs::write(
                scratch.path("tsconfig.json"),
                "{ \"extends\": \"./tsconfig.base.json\" }\n",
            )
            .unwrap();
        },
        |scratch| {
            let root = fs::read(scratch.path("tsconfig.json")).unwrap();
            let runtime = fs::read(scratch.path("node_modules/dep-hooks/src/index.jsx")).unwrap();
            fs::write(scratch.path("tsconfig.base.json"), &base_after).unwrap();
            assert_eq!(fs::read(scratch.path("tsconfig.json")).unwrap(), root);
            assert_eq!(
                fs::read(scratch.path("node_modules/dep-hooks/src/index.jsx")).unwrap(),
                runtime
            );
        },
        |scratch| fs::write(scratch.path("tsconfig.base.json"), base_before).unwrap(),
    );
    assert!(
        !before.iter().any(|finding| finding.ends_with("@H.tsx")),
        "{before:?}"
    );
    assert!(
        after.iter().any(|finding| finding.ends_with("@H.tsx")),
        "{after:?}"
    );
}

// ---------------------------------------------------------------------------
// Failure and fallback semantics stay as they are.
// ---------------------------------------------------------------------------

/// A propagated failure after a successful dependency-source run is an error
/// with no stale or partial answer on stdout, and the run after the repair is
/// the fresh answer. The failure is a selected package's manifest the selector
/// reads, made unparsable in place; it does not assume that TypeScript fails.
#[test]
fn a_propagated_failure_after_success_is_an_error_and_the_repair_recovers() {
    let typefacts = armed!();
    let scratch = Scratch::new("dependency-source", "failure");
    let sockets = SocketDir::new("failure");
    let args = dependency_args();
    let requested = || {
        invoke(
            &typefacts,
            &scratch.project,
            &args,
            true,
            Some(&sockets),
            &[],
        )
    };
    let before = requested();
    assert_one_shot(&before, &sockets, "before");
    let manifest = scratch.path("node_modules/dep-hooks/package.json");
    let original = fs::read(&manifest).unwrap();
    fs::write(&manifest, b"{ not json").unwrap();
    let failed = requested();
    assert!(!failed.success, "{}", failed.stderr);
    assert!(failed.stdout.is_empty(), "{}", failed.text());
    assert!(
        !failed.stderr.contains("daemon unavailable"),
        "{}",
        failed.stderr
    );
    assert!(!failed.used_daemon(), "{}", failed.stderr);
    sockets.assert_no_daemon("failed");
    fs::write(&manifest, original).unwrap();
    let recovered = requested();
    let fresh = invoke(&typefacts, &scratch.project, &args, false, None, &[]);
    assert_one_shot(&recovered, &sockets, "recovered");
    assert_same(&recovered, &fresh, "recovered");
    assert_same(&recovered, &before, "recovered is the first answer again");
}

/// A Type Facts program-open failure after a successful run. The root
/// tsconfig is cut in place so that the producer's own `buildProgram`
/// configuration parse returns an error when the one-shot path opens the
/// original program. The requested-daemon check returns that error, with
/// nothing on stdout, no retained-daemon or cache event, no fallback notice and
/// no socket; the exact bytes restored, the requested answer is the first
/// answer and the independently disabled fresh answer. A failing disabled
/// check shows the failure is the program's and not the daemon switch's. No
/// production hook: the failure is a configuration the producer rejects.
#[test]
fn a_producer_config_failure_after_success_is_an_error_and_the_repair_recovers() {
    let typefacts = armed!();
    let scratch = Scratch::new("dependency-source", "config-failure");
    let sockets = SocketDir::new("config-failure");
    let args = dependency_args();
    let requested = || {
        invoke(
            &typefacts,
            &scratch.project,
            &args,
            true,
            Some(&sockets),
            &[],
        )
    };
    let disabled = || invoke(&typefacts, &scratch.project, &args, false, None, &[]);
    let before = requested();
    assert_one_shot(&before, &sockets, "before");
    assert_same(&before, &disabled(), "before, disabled");

    let config = scratch.path("tsconfig.json");
    let original = fs::read(&config).unwrap();
    let text = String::from_utf8(original.clone()).unwrap();
    // The closing brace of the whole document removed: an unterminated
    // object, which the producer's configuration parser rejects.
    let cut = text.trim_end().strip_suffix('}').unwrap().to_owned();
    assert_ne!(cut.as_bytes(), original.as_slice());
    fs::write(&config, &cut).unwrap();

    let failed = requested();
    let context = format!("requested: {}", failed.stderr);
    assert!(!failed.success, "{context}");
    assert!(failed.stdout.is_empty(), "{}", failed.text());
    // The error is the producer's configuration error, surfaced by the
    // native client; it is not a daemon, a cache event or a fallback.
    assert!(
        failed.stderr.contains("open TS-Go project: parse tsconfig"),
        "{context}"
    );
    assert!(
        failed.stderr.contains("TypeFacts session error"),
        "{context}"
    );
    assert!(!failed.stderr.contains("daemon unavailable"), "{context}");
    assert!(!failed.used_daemon(), "{context}");
    assert_eq!(failed.cache_hit(), None, "{context}");
    sockets.assert_no_daemon("config failure");
    let failed_disabled = disabled();
    assert!(!failed_disabled.success, "{}", failed_disabled.stderr);
    assert!(failed_disabled.stdout.is_empty());

    fs::write(&config, &original).unwrap();
    assert_eq!(fs::read(&config).unwrap(), original);
    let recovered = requested();
    let fresh = disabled();
    assert_one_shot(&recovered, &sockets, "recovered");
    assert_same(&recovered, &fresh, "recovered, disabled");
    assert_same(&recovered, &before, "recovered is the first answer again");
}

/// The implicit default is refused for an unwritable generated-program
/// directory with a note and an ordinary contract-mode answer; an explicit
/// selection errors. Both are unchanged by the routing.
#[test]
fn an_unwritable_program_directory_keeps_its_fallback_and_its_refusal() {
    let typefacts = armed!();
    let scratch = Scratch::new("dependency-source-all", "unwritable").with_solid_vite_plugin();
    let blocker = scratch.root.join("blocker");
    fs::write(&blocker, b"a file, not a directory").unwrap();
    let program_dir = blocker.join("programs");
    let env = [("SOLID_CHECKER_PROGRAM_DIR", program_dir.as_path())];
    let sockets = SocketDir::new("unwritable");
    let opted_out = invoke(
        &typefacts,
        &scratch.project,
        &with_host(&["--no-dependency-source"]),
        false,
        None,
        &[],
    );
    assert!(opted_out.success, "{}", opted_out.stderr);
    for daemon in [false, true] {
        let implicit = invoke(
            &typefacts,
            &scratch.project,
            &HOST,
            daemon,
            Some(&sockets),
            &env,
        );
        assert!(implicit.success, "daemon {daemon}: {}", implicit.stderr);
        assert!(
            implicit
                .stderr
                .contains("packages are not analyzed from source by default"),
            "{}",
            implicit.stderr
        );
        assert!(!implicit.used_daemon(), "{}", implicit.stderr);
        assert_eq!(implicit.text(), opted_out.text(), "daemon {daemon}");
        let explicit = invoke(
            &typefacts,
            &scratch.project,
            &with_host(&["--dependency-source", "*"]),
            daemon,
            Some(&sockets),
            &env,
        );
        assert!(!explicit.success, "daemon {daemon}: {}", explicit.stderr);
        assert!(explicit.stdout.is_empty(), "{}", explicit.text());
        assert!(
            !explicit.stderr.contains("daemon unavailable"),
            "{}",
            explicit.stderr
        );
    }
    sockets.assert_no_daemon("unwritable");
}
