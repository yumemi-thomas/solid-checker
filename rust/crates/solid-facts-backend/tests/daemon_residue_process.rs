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
        let typefacts = env::var("SOLID_TYPEFACTS_BIN").ok()?;
        let root = temporary_directory(label).canonicalize().unwrap();
        copy_tree(
            &PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../fixtures/reactive-ir/callback-forwarding"),
            &root,
        );
        // A second component file, so findings sit in more than one file.
        let app = fs::read_to_string(root.join("App.tsx")).unwrap();
        fs::write(
            root.join("Other.tsx"),
            app.replace("Good", "OtherGood").replace("Bad", "OtherBad"),
        )
        .unwrap();
        let sockets = PathBuf::from(format!("/tmp/sdr-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&sockets);
        fs::create_dir_all(&sockets).unwrap();
        Some(Self {
            tsconfig: root.join("tsconfig.json"),
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
