//! The retained daemon's idle-time check (ADR 0394): after an
//! answer, a fresh build of the same inputs is compared with what was served,
//! and a mismatch drops the retained state. The faults and delays here come
//! from the debug build's test hooks.
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
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

struct Run {
    stdout: Vec<u8>,
    stderr: String,
    elapsed: Duration,
}

impl Run {
    /// The generation and cache hit the daemon reported.
    fn timing(&self) -> serde_json::Value {
        self.stderr
            .lines()
            .find_map(|line| {
                let value: serde_json::Value = serde_json::from_str(line).ok()?;
                (value["mode"] == "retained-daemon").then_some(value)
            })
            .unwrap_or_else(|| panic!("no retained answer: {}", self.stderr))
    }
}

/// A scratch project served by its own daemon, which is stopped on drop.
struct Project {
    root: PathBuf,
    tsconfig: PathBuf,
    sockets: PathBuf,
    typefacts: String,
    environment: Vec<(String, String)>,
}

impl Project {
    fn new(label: &str, environment: &[(&str, &str)]) -> Option<Self> {
        let typefacts = env::var("SOLID_TYPEFACTS_BIN").ok()?;
        let root = temporary_directory(label).canonicalize().unwrap();
        copy_tree(
            &PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../fixtures/reactive-ir/callback-forwarding"),
            &root,
        );
        let sockets = PathBuf::from(format!("/tmp/sci-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&sockets);
        fs::create_dir_all(&sockets).unwrap();
        Some(Self {
            tsconfig: root.join("tsconfig.json"),
            root,
            sockets,
            typefacts,
            environment: environment
                .iter()
                .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
                .collect(),
        })
    }

    fn log(&self) -> PathBuf {
        self.sockets.join("shadow.log")
    }

    fn log_text(&self) -> String {
        fs::read_to_string(self.log()).unwrap_or_default()
    }

    fn command(&self, daemon: bool) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"));
        command
            .env("SOLID_TYPEFACTS_BIN", &self.typefacts)
            .env("SOLID_CHECKER_DAEMON", if daemon { "1" } else { "0" })
            .env("SOLID_CHECKER_DAEMON_IDLE_SECS", "60")
            .env("SOLID_CHECKER_TIMINGS", "1")
            // The workspace config turns the inline shadow to `panic`.
            .env("SOLID_CHECKER_IR_SHADOW", "0")
            // Off by default (ADR 0394): these tests opt in.
            .env("SOLID_CHECKER_IDLE_SHADOW", "1")
            .env("SOLID_CHECKER_IDLE_SHADOW_DEBOUNCE_MS", "50")
            .env("SOLID_CHECKER_IDLE_SHADOW_TRACE", "1")
            .env("SOLID_CHECKER_IDLE_SHADOW_LOG", self.log())
            .env("TMPDIR", &self.sockets);
        for (key, value) in &self.environment {
            command.env(key, value);
        }
        command
            .args(["--format", "json", "--project"])
            .arg(&self.tsconfig);
        command
    }

    fn check(&self, daemon: bool) -> Run {
        let started = Instant::now();
        let output = self.command(daemon).output().unwrap();
        Run {
            stdout: output.stdout,
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            elapsed: started.elapsed(),
        }
    }

    fn wait_for_log(&self, needle: &str, count: usize) {
        let deadline = Instant::now() + Duration::from_secs(60);
        while Instant::now() < deadline {
            if self.log_text().matches(needle).count() >= count {
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        panic!("`{needle}` x{count} never logged:\n{}", self.log_text());
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

#[test]
fn an_injected_mismatch_is_detected_the_state_dropped_and_the_next_answer_fresh() {
    let Some(project) = Project::new("fault", &[("SOLID_CHECKER_IDLE_SHADOW_FAULT", "1")]) else {
        return;
    };
    let fresh = project.check(false);
    let served = project.check(true);
    assert_ne!(served.stdout, fresh.stdout, "the fault perturbs the answer");
    assert!(String::from_utf8_lossy(&served.stdout).contains("SC0000"));
    assert_eq!(served.timing()["generation"], 1);
    project.wait_for_log("differs from a fresh build", 1);
    let log = project.log_text();
    assert!(log.contains("table `findings` row"), "{log}");
    assert!(log.contains("SC0000"), "{log}");
    // The next answer is fresh: a new session, the one-shot bytes, and
    // flagged as a replacement.
    let next = project.check(true);
    assert_eq!(next.stdout, fresh.stdout);
    assert_eq!(next.timing()["cacheHit"], false);
    assert_eq!(
        next.timing()["generation"],
        1,
        "a state opened from scratch"
    );
    assert!(
        next.stderr.contains("this answer is the fresh one"),
        "{}",
        next.stderr
    );
    // That state verifies, and answers its repeat from cache.
    project.wait_for_log("verified", 1);
    let repeat = project.check(true);
    assert_eq!(repeat.stdout, fresh.stdout);
    assert_eq!(repeat.timing()["cacheHit"], true);
    assert!(!repeat.stderr.contains("fresh one"), "{}", repeat.stderr);
    assert_eq!(
        project
            .log_text()
            .matches("differs from a fresh build")
            .count(),
        1
    );
}

#[test]
fn a_panic_mode_mismatch_ends_the_daemon_and_removes_its_socket() {
    let Some(project) = Project::new(
        "panic",
        &[
            ("SOLID_CHECKER_IDLE_SHADOW_FAULT", "1"),
            ("SOLID_CHECKER_IDLE_SHADOW", "panic"),
        ],
    ) else {
        return;
    };
    let served = project.check(true);
    assert!(String::from_utf8_lossy(&served.stdout).contains("SC0000"));
    project.wait_for_log("differs from a fresh build", 1);
    let deadline = Instant::now() + Duration::from_secs(10);
    while !project.daemon_pids().is_empty() {
        assert!(Instant::now() < deadline, "the daemon is still running");
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn an_unperturbed_edit_sequence_verifies_every_answer() {
    let Some(project) = Project::new("verify", &[]) else {
        return;
    };
    let first = project.check(true);
    assert_eq!(first.timing()["generation"], 1);
    project.wait_for_log("verified", 1);
    let source = project.root.join("source.ts");
    let original = fs::read_to_string(&source).unwrap();
    for (index, text) in [
        format!("{original}\nexport const extra = readCount();\n"),
        original.clone(),
    ]
    .into_iter()
    .enumerate()
    {
        fs::write(&source, text).unwrap();
        let retained = project.check(true);
        assert_eq!(
            retained.timing()["generation"],
            index as u64 + 2,
            "an overlay edit"
        );
        let fresh = project.check(false);
        assert_eq!(retained.stdout, fresh.stdout);
        project.wait_for_log("verified", index + 2);
    }
    let log = project.log_text();
    assert!(!log.contains("differs"), "{log}");
    assert!(!log.contains("inconclusive"), "{log}");
}

#[test]
fn a_request_during_a_check_cancels_it_and_the_check_resumes_afterwards() {
    // The delay hook keeps the fresh build waiting (and cancellable) for 4 s.
    let Some(project) = Project::new("cancel", &[("SOLID_CHECKER_IDLE_SHADOW_DELAY_MS", "4000")])
    else {
        return;
    };
    let cold = project.check(true);
    assert_eq!(cold.timing()["cacheHit"], false);
    std::thread::sleep(Duration::from_millis(500)); // the check is waiting out its delay
    assert_eq!(project.log_text(), "");
    let repeat = project.check(true);
    assert_eq!(repeat.timing()["cacheHit"], true);
    assert!(
        repeat.elapsed < Duration::from_secs(3),
        "the request was not held back by the check: {:?}",
        repeat.elapsed
    );
    project.wait_for_log("cancelled", 1);
    // Resumed after the next debounce, and this time left alone.
    project.wait_for_log("verified", 1);
}

#[test]
fn no_check_runs_when_the_inline_shadow_is_on_or_the_check_is_off() {
    for (label, environment) in [
        ("inline", ("SOLID_CHECKER_IR_SHADOW", "1")),
        ("off", ("SOLID_CHECKER_IDLE_SHADOW", "0")),
        // Unset is off: the default answers without a background check.
        ("default", ("SOLID_CHECKER_IDLE_SHADOW", "")),
    ] {
        let Some(project) = Project::new(label, &[environment]) else {
            return;
        };
        let served = project.check(true);
        assert_eq!(served.timing()["cacheHit"], false);
        std::thread::sleep(Duration::from_secs(4));
        assert_eq!(project.log_text(), "", "{label}");
    }
}

/// Review R1 finding 10: a package's declarations edited while the daemon
/// is idle make the fresh build answer differently, which is the disk's
/// doing, not reuse's. The check calls that inconclusive, and in `panic`
/// mode the daemon lives on.
#[test]
fn a_package_edited_while_idle_is_inconclusive_not_a_mismatch() {
    let Some(project) = Project::new(
        "package-edit",
        &[
            ("SOLID_CHECKER_IDLE_SHADOW", "panic"),
            ("SOLID_CHECKER_IDLE_SHADOW_DELAY_MS", "1500"),
        ],
    ) else {
        return;
    };
    let package = project.root.join("node_modules/late-package");
    fs::create_dir_all(&package).unwrap();
    fs::write(
        package.join("package.json"),
        r#"{"name":"late-package","version":"1.0.0","types":"index.d.ts"}"#,
    )
    .unwrap();
    let declarations = package.join("index.d.ts");
    fs::write(
        &declarations,
        "export declare function invoke<T>(callback: () => T): T;\n\
         export declare function readCount(): number;\n",
    )
    .unwrap();
    let app = project.root.join("App.tsx");
    let original = fs::read_to_string(&app).unwrap();
    fs::write(
        &app,
        format!(
            "{original}\nimport {{ invoke as lateInvoke, readCount as lateRead }} from \
             \"late-package\";\nexport function Late() {{\n  const value = \
             lateInvoke(lateRead);\n  return <div>{{value}}</div>;\n}}\n"
        ),
    )
    .unwrap();
    let before = project.check(false);
    let served = project.check(true);
    assert_eq!(served.stdout, before.stdout);
    // While the check's fresh build waits: the package now hands back the
    // project's own helpers, and `Late` reads untracked.
    fs::write(
        &declarations,
        "export { invoke, readCount } from \"../../source\";\n",
    )
    .unwrap();
    assert_ne!(project.check(false).stdout, before.stdout);
    project.wait_for_log("inconclusive", 1);
    let log = project.log_text();
    assert!(!log.contains("differs from a fresh build"), "{log}");
    assert!(
        log.contains("the files the program resolved changed"),
        "{log}"
    );
    assert!(!project.daemon_pids().is_empty(), "the daemon ended: {log}");
}
