//! What the retained daemon sends and what it trusts between requests
//! (ADR 0400): an `--only-file` answer is the full report's findings
//! for that file and nothing else changes; nothing it keeps outlives an
//! edit, a revert, a package change or a configuration change.
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

use crate::process_support::temporary_directory;

fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// A scratch project served by its own daemon, which is stopped on drop.
struct Project {
    root: PathBuf,
    tsconfig: PathBuf,
    sockets: PathBuf,
    typefacts: String,
}

struct Run {
    stdout: Vec<u8>,
    stderr: String,
}

impl Run {
    fn json(&self) -> serde_json::Value {
        serde_json::from_slice(&self.stdout).unwrap()
    }

    fn cache_hit(&self) -> bool {
        self.stderr
            .lines()
            .find_map(|line| {
                let value: serde_json::Value = serde_json::from_str(line).ok()?;
                (value["mode"] == "retained-daemon").then(|| value["cacheHit"] == true)
            })
            .unwrap_or_else(|| panic!("no retained answer: {}", self.stderr))
    }
}

impl Project {
    fn new(label: &str) -> Option<Self> {
        Self::in_directory(label, "")
    }

    /// [`Self::new`] with the project in `directory` under the scratch root,
    /// so the root can hold what a monorepo hoists above it.
    fn in_directory(label: &str, directory: &str) -> Option<Self> {
        let typefacts = env::var("SOLID_TYPEFACTS_BIN").ok()?;
        let root = temporary_directory(label).canonicalize().unwrap();
        let project = root.join(directory);
        copy_tree(
            &PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../fixtures/reactive-ir/callback-forwarding"),
            &project,
        );
        // A second component file, so findings sit in more than one file.
        let app = fs::read_to_string(project.join("App.tsx")).unwrap();
        fs::write(
            project.join("Other.tsx"),
            app.replace("Good", "OtherGood").replace("Bad", "OtherBad"),
        )
        .unwrap();
        let sockets = PathBuf::from(format!("/tmp/sdr-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&sockets);
        fs::create_dir_all(&sockets).unwrap();
        Some(Self {
            tsconfig: project.join("tsconfig.json"),
            root,
            sockets,
            typefacts,
        })
    }

    fn check(&self, daemon: bool, only_file: Option<&Path>) -> Run {
        let mut command = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"));
        command
            .env("SOLID_TYPEFACTS_BIN", &self.typefacts)
            .env("SOLID_CHECKER_DAEMON", if daemon { "1" } else { "0" })
            .env("SOLID_CHECKER_DAEMON_IDLE_SECS", "60")
            .env("SOLID_CHECKER_TIMINGS", "1")
            .env("SOLID_CHECKER_IR_SHADOW", "0")
            .env("TMPDIR", &self.sockets)
            .args(["--format", "json", "--project"])
            .arg(&self.tsconfig);
        if let Some(file) = only_file {
            command.arg("--only-file").arg(file);
        }
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        Run {
            stdout: output.stdout,
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        }
    }

    fn daemon_pids(&self) -> Vec<String> {
        let output = Command::new("pgrep")
            .args(["-f", "--"])
            .arg(format!("--serve --project {}", self.tsconfig.display()))
            .output()
            .unwrap();
        String::from_utf8_lossy(&output.stdout)
            .split_whitespace()
            .map(str::to_owned)
            .collect()
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        for pid in self.daemon_pids() {
            let _ = Command::new("kill").arg(pid).status();
        }
        let _ = fs::remove_dir_all(&self.root);
        let _ = fs::remove_dir_all(&self.sockets);
    }
}

/// `full` with the findings that can matter to `file`, as the contract says.
fn narrowed(full: &serde_json::Value, file: &Path) -> serde_json::Value {
    let file = file.to_str().unwrap();
    let mut report = full.clone();
    report["findings"] = full["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|finding| {
            finding["subjectKind"] == "project"
                || finding["primaryLocation"]["path"] == file
                || finding["relatedLocations"]
                    .as_array()
                    .is_some_and(|related| related.iter().any(|place| place["path"] == file))
        })
        .cloned()
        .collect();
    report
}

#[test]
fn an_only_file_answer_is_the_full_reports_findings_for_that_file() {
    let Some(project) = Project::new("only-file") else {
        return;
    };
    let full = project.check(false, None);
    let report = full.json();
    let findings = report["findings"].as_array().unwrap();
    let paths = |report: &serde_json::Value| {
        report["findings"]
            .as_array()
            .unwrap()
            .iter()
            .map(|finding| {
                finding["primaryLocation"]["path"]
                    .as_str()
                    .unwrap()
                    .to_owned()
            })
            .collect::<std::collections::BTreeSet<_>>()
    };
    assert!(
        paths(&report).len() >= 2,
        "the fixture should have findings in two files: {findings:#?}"
    );
    // The daemon without the flag is today's answer, byte for byte.
    assert_eq!(project.check(true, None).stdout, full.stdout);
    for name in ["App.tsx", "Other.tsx", "source.ts", "absent.ts"] {
        let file = project.root.join(name);
        let expected = narrowed(&report, &file);
        let oneshot = project.check(false, Some(&file));
        assert_eq!(oneshot.json(), expected, "{name}");
        // Cold and cached, the retained answer is the one-shot one.
        for _ in 0..2 {
            let retained = project.check(true, Some(&file));
            assert_eq!(retained.stdout, oneshot.stdout, "{name}");
        }
        // And the full answer is still the full one afterwards.
        assert_eq!(project.check(true, None).stdout, full.stdout, "{name}");
    }
    let app = project.root.join("App.tsx");
    let one = project.check(true, Some(&app));
    assert!(one.stdout.len() < full.stdout.len());
    assert!(one.cache_hit());
    // A relative or unsettled name keeps everything for the caller to filter.
    let all = project.check(true, Some(Path::new("App.tsx")));
    assert_eq!(all.stdout, full.stdout);
}

#[test]
fn nothing_a_daemon_remembers_outlives_an_edit_a_revert_a_package_or_a_configuration_change() {
    let Some(project) = Project::new("memo") else {
        return;
    };
    let app = project.root.join("App.tsx");
    let source = project.root.join("source.ts");
    let original = fs::read_to_string(&app).unwrap();
    let source_original = fs::read_to_string(&source).unwrap();
    let rule = project.check(false, None).json()["findings"][0]["rule"]
        .as_str()
        .unwrap()
        .to_owned();
    // Every caller's retained answer must be its one-shot one at every step.
    let agree = |step: &str| -> serde_json::Value {
        let fresh = project.check(false, None);
        let fresh_app = project.check(false, Some(&app));
        for _ in 0..2 {
            assert_eq!(project.check(true, None).stdout, fresh.stdout, "{step}");
            assert_eq!(
                project.check(true, Some(&app)).stdout,
                fresh_app.stdout,
                "{step}"
            );
        }
        fresh.json()
    };
    let baseline = agree("baseline");
    // Remembered, then edited: the edit and its revert both show.
    fs::write(&app, format!("{original}\nexport const extra = 1;\n")).unwrap();
    agree("edit");
    fs::write(&app, &original).unwrap();
    assert_eq!(agree("revert"), baseline);
    fs::write(
        &source,
        format!("{source_original}\nexport const more = 2;\n"),
    )
    .unwrap();
    agree("edit of an imported file");
    fs::write(&source, &source_original).unwrap();
    assert_eq!(agree("revert of an imported file"), baseline);
    // A configuration change with no source edit: the answer is not served
    // from the cache, and changes with it.
    let options = project.root.join(".solid-checker");
    fs::create_dir_all(&options).unwrap();
    fs::write(
        options.join("rule-options.json"),
        format!(r#"{{"schemaVersion":1,"rules":{{"{rule}":{{"enabled":false}}}}}}"#),
    )
    .unwrap();
    let configured = agree("rule options added");
    assert_ne!(configured, baseline);
    // The same file edited in place, with no source edit either.
    fs::write(
        options.join("rule-options.json"),
        r#"{"schemaVersion":1,"rules":{}}"#,
    )
    .unwrap();
    assert_eq!(agree("rule options edited back"), baseline);
    fs::write(
        options.join("rule-options.json"),
        format!(r#"{{"schemaVersion":1,"rules":{{"{rule}":{{"enabled":false}}}}}}"#),
    )
    .unwrap();
    assert_eq!(agree("rule options edited again"), configured);
    fs::remove_file(options.join("rule-options.json")).unwrap();
    assert_eq!(agree("rule options removed"), baseline);
    // A package change: an installed manifest the project reads appears,
    // changes and disappears.
    let package = project.root.join("node_modules/solid-js");
    fs::create_dir_all(&package).unwrap();
    fs::write(
        package.join("package.json"),
        r#"{"name":"solid-js","version":"2.0.0-rc.13"}"#,
    )
    .unwrap();
    agree("package added");
    fs::write(
        package.join("package.json"),
        r#"{"name":"solid-js","version":"2.0.0-rc.14"}"#,
    )
    .unwrap();
    agree("package version changed");
    fs::remove_dir_all(project.root.join("node_modules")).unwrap();
    assert_eq!(agree("package removed"), baseline);
}

/// Review R1 finding 1: a client that goes away before reading its answer
/// leaves the daemon a failed write. What the request learned about the
/// edit's imports is kept all the same, so a package the edit imports,
/// installed afterwards where no tracked directory sees it (hoisted above the
/// project), is seen by the next request as a one-shot check sees it.
#[test]
fn an_answer_nobody_reads_still_leaves_what_the_edit_resolved_observed() {
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::{UnixListener, UnixStream};
    let Some(project) = Project::in_directory("unread", "app") else {
        return;
    };
    let directory = project.tsconfig.parent().unwrap().to_path_buf();
    let app = directory.join("App.tsx");
    let original = fs::read_to_string(&app).unwrap();
    // The hoisted `node_modules` is there from the start (the witness of the
    // opened program records it as a directory), so creating a package in
    // it moves no observation the open program made.
    fs::create_dir_all(project.root.join("node_modules/other-package")).unwrap();
    // An answer larger than the socket buffers, so the daemon's write is
    // still waiting on the client when the client goes.
    let many = (0..200)
        .map(|index| {
            format!(
                "export function Many{index}() {{\n  const value = invoke(readCount);\n  return \
                 <div>{{value}}</div>;\n}}\n"
            )
        })
        .collect::<String>();
    fs::write(
        directory.join("Many.tsx"),
        format!("import {{ invoke, readCount }} from \"./source\";\n{many}"),
    )
    .unwrap();
    project.check(true, None);
    fs::write(
        &app,
        format!(
            "{original}\nimport {{ invoke as lateInvoke, readCount as lateRead }} from \"late-package\";\n\
             export function Late() {{\n  const value = lateInvoke(lateRead);\n  return <div>{{value}}</div>;\n}}\n"
        ),
    )
    .unwrap();
    // Stand between the next client and the daemon: deliver the request,
    // read none of the answer, and hang up.
    let sockets = fs::read_dir(&project.sockets)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "sock")
        })
        .collect::<Vec<_>>();
    assert_eq!(sockets.len(), 1, "{sockets:?}");
    let socket = sockets[0].clone();
    let real = project.sockets.join("daemon.sock.real");
    fs::rename(&socket, &real).unwrap();
    let proxy = UnixListener::bind(&socket).unwrap();
    let upstream_path = real.clone();
    let relay = std::thread::spawn(move || {
        let (client, _) = proxy.accept().unwrap();
        let mut line = String::new();
        BufReader::new(&client).read_line(&mut line).unwrap();
        let mut upstream = UnixStream::connect(&upstream_path).unwrap();
        upstream.write_all(line.as_bytes()).unwrap();
        std::thread::sleep(std::time::Duration::from_secs(2));
    });
    // The client is answered by its one-shot fallback.
    let unread = project.check(true, None);
    relay.join().unwrap();
    assert!(
        unread.stderr.contains("daemon unavailable"),
        "{}",
        unread.stderr
    );
    fs::remove_file(&socket).unwrap();
    fs::rename(&real, &socket).unwrap();
    // Installed where only the edit's own resolution looked.
    let package = project.root.join("node_modules/late-package");
    fs::create_dir_all(&package).unwrap();
    fs::write(
        package.join("package.json"),
        r#"{"name":"late-package","version":"1.0.0","types":"index.d.ts"}"#,
    )
    .unwrap();
    // Resolved, the package hands back the project's own callback helpers,
    // and `Late` reads untracked as `Bad` does; unresolved, it reads nothing.
    fs::write(
        package.join("index.d.ts"),
        "export { invoke, readCount } from \"../../app/source\";\n",
    )
    .unwrap();
    let fresh = project.check(false, None);
    let count = |run: &Run| run.json()["findings"].as_array().unwrap().len();
    // ADR 0827: unresolved, each use of the package's exports is open.
    let unresolved = |run: &Run| {
        run.json()["findings"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|finding| finding["analysisContext"] == "import-unresolved")
            .count()
    };
    assert_eq!(unresolved(&unread), 2, "{}", unread.stderr);
    assert_eq!(unresolved(&fresh), 0, "{}", fresh.stderr);
    assert_eq!(count(&fresh), count(&unread) - unresolved(&unread) + 1);
    let retained = project.check(true, None);
    assert_eq!(count(&retained), count(&fresh), "{}", retained.stderr);
    assert!(retained.stdout == fresh.stdout, "{}", retained.stderr);
}

/// Review R1 item 12: a file created in a new directory under one that holds
/// no source itself (only subdirectories do) is part of the project the
/// configuration includes; the retained answer sees it as a one-shot one
/// does.
#[test]
fn a_file_in_a_new_directory_under_a_source_less_one_is_seen() {
    let Some(project) = Project::new("new-subdirectory") else {
        return;
    };
    let directory = project.tsconfig.parent().unwrap().to_path_buf();
    let config = fs::read_to_string(&project.tsconfig).unwrap();
    fs::write(
        &project.tsconfig,
        config.replace(
            r#""include": ["*.ts", "*.tsx"]"#,
            r#""include": ["**/*.ts", "**/*.tsx"]"#,
        ),
    )
    .unwrap();
    assert!(
        fs::read_to_string(&project.tsconfig)
            .unwrap()
            .contains("**/*.tsx")
    );
    let component = |name: &str| {
        format!(
            "import {{ invoke, readCount }} from \"../../source\";\nexport function {name}() {{\n  \
             const value = invoke(readCount);\n  return <div>{{value}}</div>;\n}}\n"
        )
    };
    fs::create_dir_all(directory.join("deep/a")).unwrap();
    fs::write(directory.join("deep/a/Inner.tsx"), component("Inner")).unwrap();
    let before = project.check(false, None);
    assert_eq!(project.check(true, None).stdout, before.stdout);
    fs::create_dir_all(directory.join("deep/b")).unwrap();
    fs::write(directory.join("deep/b/Added.tsx"), component("Added")).unwrap();
    let fresh = project.check(false, None);
    // ADR 0970: each exported component no tag renders also holds an SC4001 owner obligation, which is not this test's subject.
    let count = |run: &Run| {
        run.json()["findings"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|finding| finding["id"] != "SC4001")
            .count()
    };
    assert_eq!(count(&fresh), count(&before) + 1);
    let retained = project.check(true, None);
    assert_eq!(count(&retained), count(&fresh), "{}", retained.stderr);
    assert!(retained.stdout == fresh.stdout, "{}", retained.stderr);
}

// ADR 0543: the fuzzer's warm checker. One retained daemon per executor slot
// answers `--feedback-facts` checks of program after program written into the
// same directory. Every answer is compared, byte for byte with its exit
// status, with the one-shot answer at the same path.

const FEEDBACK_ARGS: [&str; 9] = [
    "--format",
    "json",
    "--feedback-facts",
    "--runtime-target",
    "browser",
    "--runtime-build",
    "development",
    "--rendering",
    "csr",
];

/// A fuzzer slot: a fixed directory whose files the programs rewrite.
struct Slot {
    root: PathBuf,
    tsconfig: PathBuf,
    sockets: PathBuf,
    typefacts: String,
}

struct Answer {
    status: Option<i32>,
    stdout: Vec<u8>,
    stderr: String,
}

impl Answer {
    /// The retained session's generation, `None` for a one-shot answer.
    fn generation(&self) -> Option<u64> {
        self.stderr.lines().find_map(|line| {
            let value: serde_json::Value = serde_json::from_str(line).ok()?;
            (value["mode"] == "retained-daemon").then(|| value["generation"].as_u64().unwrap())
        })
    }
}

/// The programs a slot cycles through: the fixture's `App.tsx` and variants
/// that edit a body, add a component and import, and add a module file
/// (which changes the project's file set).
fn slot_programs() -> Vec<Vec<(&'static str, String)>> {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../fixtures/reactive-ir/callback-forwarding");
    let read = |name: &str| fs::read_to_string(fixture.join(name)).unwrap();
    let app = read("App.tsx");
    assert!(app.contains("const value = invoke(readCount);"));
    let base = vec![
        ("tsconfig.json", read("tsconfig.json")),
        ("solid-js.d.ts", read("solid-js.d.ts")),
        ("source.ts", read("source.ts")),
    ];
    let with = |app: String, extra: Option<(&'static str, String)>| {
        let mut files = base.clone();
        files.push(("App.tsx", app));
        files.extend(extra);
        files
    };
    vec![
        with(app.clone(), None),
        with(
            app.replace(
                "const value = invoke(readCount);",
                "const value = invoke(readCount) + invoke(readCount);",
            ),
            None,
        ),
        with(
            format!(
                "{}\nexport function Third() {{\n  const third = invoke(readCount);\n  return <p>{{third}}</p>;\n}}\n",
                app.replace(
                    "import { invoke, readCount } from \"./source\";",
                    "import { invoke } from \"./source\";\nimport { readCount } from \"./source\";",
                )
            ),
            None,
        ),
        with(
            format!("import {{ helper }} from \"./helper\";\n{app}\nexport const used = helper;\n"),
            Some((
                "helper.ts",
                "import { invoke, readCount } from \"./source\";\nexport const helper = () => invoke(readCount);\n"
                    .into(),
            )),
        ),
    ]
}

impl Slot {
    fn new(label: &str) -> Option<Self> {
        let typefacts = env::var("SOLID_TYPEFACTS_BIN").ok()?;
        let root = temporary_directory(label).canonicalize().unwrap();
        let sockets = PathBuf::from(format!("/tmp/sdw-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&sockets);
        fs::create_dir_all(&sockets).unwrap();
        Some(Self {
            tsconfig: root.join("tsconfig.json"),
            root,
            sockets,
            typefacts,
        })
    }

    /// Makes the slot hold exactly `files`, as the executor's `sync_files`
    /// does: other entries removed, a changed file rewritten in place, an
    /// equal one untouched. Then the type check's transient file comes and
    /// goes, which moves the directory's stamp and nothing else.
    fn write(&self, files: &[(&str, String)]) {
        for entry in fs::read_dir(&self.root).unwrap() {
            let entry = entry.unwrap();
            let name = entry.file_name().to_string_lossy().into_owned();
            if !files.iter().any(|(file, _)| *file == name) {
                fs::remove_file(entry.path()).unwrap();
            }
        }
        for (name, text) in files {
            let path = self.root.join(name);
            if fs::read_to_string(&path).ok().as_deref() != Some(text) {
                fs::write(&path, text).unwrap();
            }
        }
        let transient = self.root.join("tsconfig.tsbuildinfo");
        fs::write(&transient, "{}").unwrap();
        fs::remove_file(&transient).unwrap();
    }

    fn command(&self, daemon: bool, feedback: bool) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"));
        command
            .env("SOLID_TYPEFACTS_BIN", &self.typefacts)
            .env("SOLID_CHECKER_DAEMON", if daemon { "1" } else { "0" })
            .env("SOLID_CHECKER_DAEMON_IDLE_SECS", "60")
            .env("SOLID_CHECKER_TIMINGS", "1")
            .env("SOLID_CHECKER_IR_SHADOW", "0")
            .env("TMPDIR", &self.sockets)
            .current_dir(&self.root)
            .arg("--project")
            .arg(&self.tsconfig)
            .args(if feedback {
                &FEEDBACK_ARGS[..]
            } else {
                &FEEDBACK_ARGS[..2]
            });
        command
    }

    fn check(&self, daemon: bool, feedback: bool) -> Answer {
        let output = self.command(daemon, feedback).output().unwrap();
        Answer {
            status: output.status.code(),
            stdout: output.stdout,
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        }
    }

    /// The retained answer, checked against the one-shot answer at the same
    /// path: stdout byte for byte and the exit status.
    fn agree(&self, step: &str) -> Answer {
        let warm = self.check(true, true);
        let once = self.check(false, true);
        assert!(
            matches!(once.status, Some(0 | 1)),
            "{step}: {}",
            once.stderr
        );
        assert!(
            warm.generation().is_some(),
            "{step}: not a retained answer: {}",
            warm.stderr
        );
        assert_eq!(warm.status, once.status, "{step}: {}", warm.stderr);
        assert_eq!(
            String::from_utf8_lossy(&warm.stdout),
            String::from_utf8_lossy(&once.stdout),
            "{step}"
        );
        let report: serde_json::Value = serde_json::from_slice(&once.stdout).unwrap();
        assert!(
            report["feedbackFacts"]
                .as_array()
                .is_some_and(|files| !files.is_empty()),
            "{step}: the program has feedback models"
        );
        warm
    }

    fn stop(&self) {
        let output = self
            .command(true, true)
            .arg("--stop-daemon")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn daemon_pids(&self) -> Vec<String> {
        let output = Command::new("pgrep")
            .args(["-f", "--"])
            .arg(format!("--serve --project {}", self.tsconfig.display()))
            .output()
            .unwrap();
        String::from_utf8_lossy(&output.stdout)
            .split_whitespace()
            .map(str::to_owned)
            .collect()
    }

    fn sockets(&self) -> usize {
        fs::read_dir(&self.sockets)
            .unwrap()
            .filter(|entry| {
                entry
                    .as_ref()
                    .unwrap()
                    .path()
                    .extension()
                    .is_some_and(|extension| extension == "sock")
            })
            .count()
    }
}

impl Drop for Slot {
    fn drop(&mut self) {
        for pid in self.daemon_pids() {
            let _ = Command::new("kill").arg(pid).status();
        }
        let _ = fs::remove_dir_all(&self.root);
        let _ = fs::remove_dir_all(&self.sockets);
    }
}

#[test]
fn a_slot_daemon_answers_every_program_of_a_sequence_as_a_one_shot_check_does() {
    let Some(slot) = Slot::new("warm-sequence") else {
        return;
    };
    let programs = slot_programs();
    // A→B→A, A repeated, a new import, a new file and its removal, then a
    // fixed pseudo-random order.
    let mut sequence = vec![0, 1, 0, 0, 2, 0, 3, 1, 3, 0];
    let mut state = 0x9e37_79b9_u32;
    for _ in 0..10 {
        state = state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        sequence.push((state >> 16) as usize % programs.len());
    }
    let mut previous: Option<(usize, u64)> = None;
    let mut highest = 0;
    for (step, &index) in sequence.iter().enumerate() {
        slot.write(&programs[index]);
        let generation = slot
            .agree(&format!("step {step}: program {index}"))
            .generation()
            .unwrap();
        // The type check's file, created and removed, rebuilds nothing: an
        // edit within one file set is the retained session's next generation.
        if let Some((last, last_generation)) = previous {
            let same_files = (last == 3) == (index == 3);
            if same_files {
                assert!(
                    generation >= last_generation,
                    "step {step}: rebuilt from {last_generation} to {generation}"
                );
            }
        }
        previous = Some((index, generation));
        highest = highest.max(generation);
    }
    assert!(highest >= 3, "the session was retained across edits");

    // An edit of equal length whose modification time is put back: the
    // file's change time still moves, and the answer follows the bytes.
    slot.write(&programs[0]);
    slot.agree("before the equal-length edit");
    let app = slot.root.join("App.tsx");
    let original = fs::read_to_string(&app).unwrap();
    let reference = slot.root.join("reference");
    fs::copy(&app, &reference).unwrap();
    Command::new("touch")
        .arg("-r")
        .arg(&app)
        .arg(&reference)
        .status()
        .unwrap();
    let edited = original.replacen("Good", "Gooe", 1);
    assert_eq!(edited.len(), original.len());
    fs::write(&app, &edited).unwrap();
    Command::new("touch")
        .arg("-r")
        .arg(&reference)
        .arg(&app)
        .status()
        .unwrap();
    fs::remove_file(&reference).unwrap();
    slot.agree("an equal-length edit with its modification time restored");
}

#[test]
fn a_fresh_slot_a_reused_slot_and_another_slot_answer_one_program_alike() {
    let (Some(reused), Some(fresh)) = (Slot::new("warm-reused"), Slot::new("warm-fresh")) else {
        return;
    };
    let programs = slot_programs();
    for index in [1, 3, 2] {
        reused.write(&programs[index]);
        reused.agree(&format!("reused slot, program {index}"));
    }
    reused.write(&programs[0]);
    let after = reused.agree("reused slot, program 0");
    fresh.write(&programs[0]);
    let first = fresh.agree("fresh slot, program 0");
    assert_eq!(first.generation(), Some(1), "a new daemon's first answer");
    // The same text in two slots: equal once each slot's own root is named
    // alike (the same-path comparisons above are the authority).
    let spelled = |answer: &Answer, slot: &Slot| {
        String::from_utf8_lossy(&answer.stdout).replace(slot.root.to_str().unwrap(), "<slot>")
    };
    assert_eq!(spelled(&after, &reused), spelled(&first, &fresh));
}

/// A program whose check takes long enough to be abandoned part-way: the
/// fixture with a module of many components.
fn heavy_program() -> Vec<(&'static str, String)> {
    let mut files = slot_programs().remove(0);
    let many = (0..400)
        .map(|index| {
            format!(
                "export function Many{index}() {{\n  const value = invoke(readCount);\n  return \
                 <div>{{value}}</div>;\n}}\n"
            )
        })
        .collect::<String>();
    files.push((
        "Many.tsx",
        format!("import {{ invoke, readCount }} from \"./source\";\n{many}"),
    ));
    files
}

impl Slot {
    /// Starts a warm check and kills its client while the daemon still works
    /// on it (asserted: the client had not finished).
    fn abandon(&self) {
        let mut client = self
            .command(true, true)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(30));
        assert!(
            client.try_wait().unwrap().is_none(),
            "the check finished before it could be abandoned"
        );
        client.kill().unwrap();
        client.wait().unwrap();
    }
}

#[test]
fn an_abandoned_check_a_stop_and_a_restart_never_leak_into_the_next_program() {
    let Some(slot) = Slot::new("warm-cancel") else {
        return;
    };
    let programs = slot_programs();
    slot.write(&programs[0]);
    slot.agree("program 0");
    assert_eq!(slot.daemon_pids().len(), 1);

    // A client killed while its daemon works, and the next program written
    // into the slot at once, with no stop: the next answer is that program's.
    slot.write(&heavy_program());
    slot.abandon();
    slot.write(&programs[2]);
    slot.agree("after an abandoned check");

    // The executor's way: a killed check is followed by a stop, which waits
    // for what the daemon accepted and ends it.
    slot.write(&heavy_program());
    slot.abandon();
    slot.stop();
    assert!(slot.daemon_pids().is_empty(), "the stop ended the daemon");
    assert_eq!(slot.sockets(), 0, "and removed its socket");
    slot.write(&programs[1]);
    let restarted = slot.agree("a new daemon after a stop");
    assert_eq!(restarted.generation(), Some(1));

    // Restart after an update: the retained answer to an edit is the answer
    // of a daemon that starts on the edited files.
    slot.write(&programs[2]);
    let retained = slot.agree("an edit on the retained session");
    assert!(retained.generation().unwrap() > 1);
    slot.stop();
    let reopened = slot.agree("the same files on a new daemon");
    assert_eq!(reopened.generation(), Some(1));
    assert_eq!(reopened.stdout, retained.stdout);
    // Stopping no daemon is not an error.
    slot.stop();
    slot.stop();

    // The ordinary mode is its own daemon beside the feedback one, and
    // each answers as its one-shot check.
    slot.agree("feedback daemon");
    let ordinary = slot.check(true, false);
    assert!(ordinary.generation().is_some(), "{}", ordinary.stderr);
    assert_eq!(ordinary.stdout, slot.check(false, false).stdout);
    assert_eq!(slot.daemon_pids().len(), 2);
    let report: serde_json::Value = serde_json::from_slice(&ordinary.stdout).unwrap();
    assert!(report.get("feedbackFacts").is_none());
}

#[test]
fn a_stop_ends_a_daemon_that_cannot_drain_by_its_pid() {
    let Some(slot) = Slot::new("warm-stuck") else {
        return;
    };
    let programs = slot_programs();
    slot.write(&programs[0]);
    // The daemon this client starts takes its environment: every answer
    // waits 30 s (a debug-build hook), longer than the stop's bound.
    let mut stuck = slot
        .command(true, true)
        .env("SOLID_CHECKER_DAEMON_TEST_DELAY_MS", "30000")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while slot.daemon_pids().is_empty() || slot.sockets() == 0 {
        assert!(std::time::Instant::now() < deadline, "no daemon started");
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    std::thread::sleep(std::time::Duration::from_millis(300));
    let started = std::time::Instant::now();
    let output = slot
        .command(true, true)
        .env("SOLID_CHECKER_DAEMON_STOP_SECS", "1")
        .arg("--stop-daemon")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("it was ended"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(started.elapsed() < std::time::Duration::from_secs(10));
    stuck.wait().unwrap();
    assert!(slot.daemon_pids().is_empty(), "the stuck daemon is gone");
    assert_eq!(slot.sockets(), 0);
    let fresh = slot.agree("a new daemon after the stuck one was ended");
    assert_eq!(fresh.generation(), Some(1));
}

/// A client started with a descriptor it did not create (a multithreaded
/// parent's pipe that was not yet close-on-exec): the daemon it starts must
/// not keep it, or the parent's reader of that pipe waits for the daemon to
/// end. The fuzzer's executor stalled on exactly this.
#[test]
fn a_daemon_keeps_no_descriptor_its_client_inherited() {
    let Some(slot) = Slot::new("warm-descriptors") else {
        return;
    };
    slot.write(&slot_programs()[0]);
    let check = slot.command(true, true);
    let mut line = format!("exec {:?}", check.get_program());
    for argument in check.get_args() {
        line.push_str(&format!(" {argument:?}"));
    }
    // The shell gives the client this test's stdout pipe as descriptor 3,
    // inheritable, and its own stdout and stderr elsewhere.
    line.push_str(" 3>&1 >/dev/null 2>/dev/null");
    let mut shell = Command::new("sh");
    shell.arg("-c").arg(&line).current_dir(&slot.root);
    for (key, value) in check.get_envs() {
        if let Some(value) = value {
            shell.env(key, value);
        }
    }
    let mut child = shell.stdout(std::process::Stdio::piped()).spawn().unwrap();
    let mut pipe = child.stdout.take().unwrap();
    let (done, ended) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut rest = Vec::new();
        let _ = std::io::Read::read_to_end(&mut pipe, &mut rest);
        let _ = done.send(());
    });
    assert!(child.wait().unwrap().success());
    assert_eq!(slot.daemon_pids().len(), 1, "the client started a daemon");
    assert!(
        ended
            .recv_timeout(std::time::Duration::from_secs(10))
            .is_ok(),
        "the daemon still holds the pipe its client inherited"
    );
    slot.agree("after the descriptor check");
}

/// A role log's decision lines with each one's file (`f`) and program (`g`)
/// in place of the process and path ids that name them, sorted: what the fold
/// reads, whichever process wrote it. Every line is whole JSON and names a
/// path line written before it.
fn role_log_decisions(log: &Path) -> Vec<String> {
    let mut paths = std::collections::HashMap::new();
    let mut decisions = Vec::new();
    for line in fs::read_to_string(log).unwrap().lines() {
        let mut value: serde_json::Value =
            serde_json::from_str(line).unwrap_or_else(|_| panic!("a partial line: {line}"));
        let key = format!("{}:{}", value["q"], value["p"]);
        if value["k"] == "path" {
            assert!(
                value["g"].is_string(),
                "a path line names its program: {line}"
            );
            paths.insert(key, (value["f"].clone(), value["g"].clone()));
            continue;
        }
        if let Some((file, program)) = paths.get(&key) {
            let object = value.as_object_mut().unwrap();
            object.remove("q");
            object.remove("p");
            object.insert("f".into(), file.clone());
            object.insert("g".into(), program.clone());
        } else {
            assert!(value["p"].is_null(), "a line names an unknown path: {line}");
        }
        decisions.push(value.to_string());
    }
    decisions.sort();
    decisions
}

#[test]
fn a_slot_daemon_logs_each_programs_role_decisions_as_a_one_shot_check_does() {
    let Some(slot) = Slot::new("warm-role-log") else {
        return;
    };
    let programs = slot_programs();
    let warm_log = slot.sockets.join("warm.roles.jsonl");
    let once_log = slot.sockets.join("once.roles.jsonl");
    for (step, index) in [0, 1, 2, 3, 1, 0, 0].into_iter().enumerate() {
        slot.write(&programs[index]);
        let program = format!("program-{step}");
        let run = |daemon: bool, log: &Path| {
            let output = slot
                .command(daemon, true)
                .env("SOLID_CHECKER_IR_ROLE_LOG", log)
                .env("SOLID_CHECKER_IR_ROLE_LOG_PROGRAM", &program)
                .output()
                .unwrap();
            assert!(
                matches!(output.status.code(), Some(0 | 1)),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            output.stdout
        };
        assert_eq!(run(true, &warm_log), run(false, &once_log), "step {step}");
        let (warm, once) = (role_log_decisions(&warm_log), role_log_decisions(&once_log));
        assert!(
            !once.is_empty(),
            "step {step}: the one-shot check logged decisions"
        );
        assert_eq!(warm, once, "step {step}: the warm log differs");
    }
    // The same program twice in a row is a cached answer without the log;
    // with it, its decisions are logged again, under its own program.
    let decisions = role_log_decisions(&warm_log);
    for step in [5, 6] {
        assert!(
            decisions
                .iter()
                .any(|line| line.contains(&format!("\"g\":\"program-{step}\""))),
            "program-{step} has its own decisions"
        );
    }
    // A check without the log logs nothing, though its daemon logged before.
    let before = fs::read_to_string(&warm_log).unwrap();
    slot.write(&programs[2]);
    let output = slot.command(true, true).output().unwrap();
    assert!(matches!(output.status.code(), Some(0 | 1)));
    assert_eq!(fs::read_to_string(&warm_log).unwrap(), before);
}
