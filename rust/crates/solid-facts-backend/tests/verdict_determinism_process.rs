//! A verdict does not depend on the workers or the order that walked it (ADR 1128).
//!
//! The Reactive IR walks its starts (the value-use census: every reference of a proven source) on analysis workers that each carry a state
//! from start to start. ADR 0909's contract is that a start's answer is a function of the build and the start alone; which starts share a
//! state follows the scheduler, so a breach shows as a row that changes in one run in some number, and as the same project answering twice
//! alike never. `SOLID_CHECKER_IR_CLAIM_ORDER` reproduces it without a scheduler: it names the order the starts are claimed in.
//!
//! This repeats the unit of `fixtures/reactive-ir/census-claim-order` (a start whose walks past a withheld call queue late walks) past the
//! threshold at which the starts run on several workers, and holds every worker count and claim order to the answer of the first.

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

fn copy_directory(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_directory(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// One run of the checker on `project`; `None` where it failed (the process's own output says why).
fn run(
    project: &Path,
    producer: &str,
    workers: Option<&str>,
    order: &str,
) -> Result<String, String> {
    let mut command = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"));
    command
        .env("SOLID_TYPEFACTS_BIN", producer)
        .env("SOLID_CHECKER_DAEMON", "0")
        .env("SOLID_CHECKER_IR_CLAIM_ORDER", order)
        .args(["--format", "json", "--project"])
        .arg(project.join("tsconfig.json"));
    match workers {
        Some(workers) => command.env("SOLID_CHECKER_IR_WORKERS", workers),
        None => command.env_remove("SOLID_CHECKER_IR_WORKERS"),
    };
    let output = command.output().unwrap();
    if output.status.success() {
        Ok(String::from_utf8(output.stdout).unwrap())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).into_owned())
    }
}

/// The fixture's file with its unit (`Mapper0`) repeated `units` times: past 64 starts, so that the census runs on several workers.
fn repeated(fixture: &Path, units: usize) -> String {
    let source = fs::read_to_string(fixture.join("App.tsx")).unwrap();
    let first = source.find("export function Mapper0()").unwrap();
    let second = source.find("export function Mapper1()").unwrap();
    let patch = source.find("\ngp = Object.getPrototypeOf").unwrap();
    let unit = &source[first..second];
    let mut project = source[..first].to_owned();
    for index in 0..units {
        project.push_str(&unit.replace("Mapper0", &format!("Mapper{index}")));
    }
    project.push_str(&source[patch..]);
    project
}

/// The findings of a report that differ from another's, for a failure that says what moved (a finding is its whole JSON: the accessor a row
/// names is part of the answer).
fn moved(first: &str, other: &str) -> String {
    let rows = |report: &str| -> Vec<String> {
        let report: serde_json::Value = serde_json::from_str(report).unwrap();
        report["findings"]
            .as_array()
            .unwrap()
            .iter()
            .map(|finding| finding.to_string())
            .collect()
    };
    let (first, other) = (rows(first), rows(other));
    let shorten = |row: &String| row.chars().take(240).collect::<String>();
    let only_first = first.iter().filter(|row| !other.contains(row)).take(2);
    let only_other = other.iter().filter(|row| !first.contains(row)).take(2);
    format!(
        "{} rows against {}; only in the first: {:?}; only in this one: {:?}",
        first.len(),
        other.len(),
        only_first.map(shorten).collect::<Vec<_>>(),
        only_other.map(shorten).collect::<Vec<_>>()
    )
}

#[test]
fn a_verdict_does_not_depend_on_the_order_or_the_workers_that_walked_it() {
    let Ok(producer) = env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let fixture = root.join("fixtures/reactive-ir/census-claim-order");
    let project = env::temp_dir().join(format!(
        "solid-checker-verdict-determinism-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&project);
    copy_directory(&fixture, &project);
    fs::write(project.join("App.tsx"), repeated(&fixture, 48)).unwrap();

    // The first answer: one worker, the claims in input order. Every other worker count and order, and a repeat of the default (threads
    // claim as the scheduler lets them), answers the same.
    let first = run(&project, &producer, Some("1"), "forward").unwrap();
    assert!(
        first.matches("pending-read-unresolved").count() > 20,
        "the project is no longer one whose rows name an accessor: the check would compare nothing"
    );
    let mut runs: Vec<(Option<&str>, &str)> = Vec::new();
    for workers in [Some("1"), Some("3"), None] {
        for order in ["forward", "reverse", "rotate:7", "shuffle:1", "shuffle:2"] {
            runs.push((workers, order));
        }
    }
    runs.extend([(None, "forward"); 3]);
    let mut differing = Vec::new();
    for (workers, order) in runs {
        let answer = run(&project, &producer, workers, order).unwrap();
        if answer != first {
            differing.push(format!(
                "workers {workers:?}, claims {order}: {}",
                moved(&first, &answer)
            ));
        }
    }
    let _ = fs::remove_dir_all(&project);
    assert!(
        differing.is_empty(),
        "a verdict followed the workers or the claim order:\n{}",
        differing.join("\n")
    );
}

/// A misspelled claim order would have the check above compare a run with itself: it is refused instead.
#[test]
fn an_unknown_claim_order_is_refused() {
    let Ok(producer) = env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let fixture = root.join("fixtures/reactive-ir/census-claim-order");
    let project = env::temp_dir().join(format!(
        "solid-checker-verdict-determinism-order-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&project);
    copy_directory(&fixture, &project);
    let refused = run(&project, &producer, Some("1"), "sideways");
    let _ = fs::remove_dir_all(&project);
    let message = refused.expect_err("an order the checker does not know must not run");
    assert!(
        message.contains("SOLID_CHECKER_IR_CLAIM_ORDER"),
        "{message}"
    );
}
