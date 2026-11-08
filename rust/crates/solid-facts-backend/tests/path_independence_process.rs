//! The findings of a program do not depend on where it lies (review of the
//! C2 0904 coverage): the census's walks run in the order of symbol ids, which
//! hash the file's absolute path, and the row a span kept was the first walk's.
//! `loading-on-outside-hold`'s `setT(2)` was `value-forwarded` at some paths and
//! `value-to-unresolved-callee` at others.
use std::{env, fs, path::Path, process::Command};

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

/// The findings and status at `directory`, with the directory's own path
/// written as `<root>`.
fn findings_at(typefacts: &str, directory: &Path) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
        .env("SOLID_TYPEFACTS_BIN", typefacts)
        .env("SOLID_CHECKER_DAEMON", "0")
        .args(["--format", "json", "--project"])
        .arg(directory.join("tsconfig.json"))
        .output()
        .unwrap();
    assert!(
        output.status.success() || output.status.code() == Some(1),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let kept = serde_json::json!({ "status": report["status"], "findings": report["findings"] });
    let root = fs::canonicalize(directory).unwrap();
    serde_json::to_string_pretty(&kept)
        .unwrap()
        .replace(root.to_str().unwrap(), "<root>")
        .replace(directory.to_str().unwrap(), "<root>")
}

#[test]
fn a_fixture_has_the_same_findings_under_every_path() {
    let typefacts = env::var("SOLID_TYPEFACTS_BIN").expect("arm the real Type Facts process");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../fixtures/reactive-ir/loading-on-outside-hold");
    let parent = temporary_directory("path-independence");
    // Eight names: on the build before, about half the paths kept each label.
    let reports = ["a", "bb", "cccccc", "loh-copy", "x1", "projects-y", "zz-app", "q"]
        .iter()
        .map(|name| {
            let directory = parent.join(name);
            copy_tree(&fixture, &directory);
            (*name, findings_at(&typefacts, &directory))
        })
        .collect::<Vec<_>>();
    for (name, report) in &reports[1..] {
        assert_eq!(
            report, &reports[0].1,
            "`{name}` and `{}` differ",
            reports[0].0
        );
    }
    let _ = fs::remove_dir_all(parent);
}
