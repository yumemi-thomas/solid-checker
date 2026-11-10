//! ADR 0933: the proof log changes no answer, and writes what a gate folds.
//!
//! `SOLID_CHECKER_IR_PROOF_LOG` is write-only: the findings of a check with the
//! log on are the findings of the check with it off, byte for byte. Its lines
//! are keyed by file and span, name each file once per process, and carry the
//! kinds `scripts/lib/proof-log.mjs` folds.
use std::{collections::BTreeSet, env, fs, path::Path, process::Command};

use crate::process_support::temporary_directory;

fn fixture(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../fixtures/reactive-ir")
        .join(name)
}

/// The report of a one-shot check of the fixture `name`, with the proof log at
/// `log` when given; the status and findings.
fn check(typefacts: &str, name: &str, log: Option<&Path>) -> serde_json::Value {
    let mut command = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"));
    command
        .env("SOLID_TYPEFACTS_BIN", typefacts)
        .env("SOLID_CHECKER_DAEMON", "0")
        .args(["--format", "json", "--project"])
        .arg(fixture(name).join("tsconfig.json"));
    if let Some(log) = log {
        command
            .env("SOLID_CHECKER_IR_PROOF_LOG", log)
            .env("SOLID_CHECKER_IR_PROOF_LOG_PROGRAM", name);
    }
    let output = command.output().unwrap();
    assert!(
        output.status.success() || output.status.code() == Some(1),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    serde_json::json!({ "status": report["status"], "findings": report["findings"] })
}

fn lines(log: &Path) -> Vec<serde_json::Value> {
    fs::read_to_string(log)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

#[test]
fn the_proof_log_changes_no_answer() {
    let typefacts = env::var("SOLID_TYPEFACTS_BIN").expect("arm the real Type Facts process");
    let directory = temporary_directory("proof-log-answers");
    // The strict read, the write rules, the owner rules, the pending rules and a
    // backend pass that demotes a violation.
    for name in [
        "latent-silent-arms",
        "store-delete",
        "rc21-server-arms",
        "absent-guard-prop",
        "pending-read-accounted",
    ] {
        let log = directory.join(format!("{name}.jsonl"));
        assert_eq!(
            check(&typefacts, name, None),
            check(&typefacts, name, Some(&log)),
            "{name}: the proof log moved an answer"
        );
        assert!(log.exists(), "{name}: no proof log was written");
    }
    fs::remove_dir_all(directory).ok();
}

#[test]
fn the_log_names_each_file_once_and_every_line_by_it() {
    let typefacts = env::var("SOLID_TYPEFACTS_BIN").expect("arm the real Type Facts process");
    let directory = temporary_directory("proof-log-lines");
    let log = directory.join("latent.jsonl");
    check(&typefacts, "latent-silent-arms", Some(&log));
    let lines = lines(&log);
    let mut named = BTreeSet::new();
    let mut kinds = BTreeSet::new();
    for line in &lines {
        let kind = line["k"].as_str().unwrap();
        kinds.insert(kind.to_owned());
        let process_and_path = (line["q"].as_u64().unwrap(), line["p"].as_u64().unwrap());
        if kind == "path" {
            assert!(
                named.insert(process_and_path),
                "a path is named twice: {line}"
            );
            assert_eq!(line["g"], "latent-silent-arms");
            assert!(line["f"].as_str().unwrap().starts_with('/'));
        } else {
            assert!(
                named.contains(&process_and_path),
                "a line names a path no earlier line gave: {line}"
            );
        }
    }
    // The classifier's decisions, the rows and findings of projection, the
    // backend's verdicts, the independent universe, and a named silence.
    for kind in [
        "decision", "row", "finding", "verdict", "site", "silent", "owner",
    ] {
        assert!(kinds.contains(kind), "no {kind} line: {kinds:?}");
    }
    fs::remove_dir_all(directory).ok();
}

#[test]
fn a_finding_a_pass_removes_leaves_a_typed_drop() {
    let typefacts = env::var("SOLID_TYPEFACTS_BIN").expect("arm the real Type Facts process");
    let directory = temporary_directory("proof-log-drops");
    let log = directory.join("server.jsonl");
    // A client-runtime rule's finding at a site only a server build runs is
    // removed by the execution paths (ADR 0659).
    check(&typefacts, "rc21-server-arms", Some(&log));
    let lines = lines(&log);
    let drops = lines
        .iter()
        .filter(|line| line["k"] == "drop" && line["pass"] == "ExecutionPaths")
        .collect::<Vec<_>>();
    assert!(!drops.is_empty(), "no drop names the execution paths");
    for drop in drops {
        assert!(
            !drop["premises"].as_array().unwrap().is_empty(),
            "a drop rests on no typed fact: {drop}"
        );
        let (start, end) = (drop["s"].clone(), drop["e"].clone());
        assert!(
            lines
                .iter()
                .any(|line| line["k"] == "row" && line["s"] == start && line["e"] == end),
            "a drop has no row at its site: {drop}"
        );
    }
    fs::remove_dir_all(directory).ok();
}

#[test]
fn a_named_end_rests_on_premises_that_name_code() {
    let typefacts = env::var("SOLID_TYPEFACTS_BIN").expect("arm the real Type Facts process");
    let directory = temporary_directory("proof-log-ends");
    let placing = [
        "CallEdgePlaces",
        "CarrierPlacesRead",
        "FreshStackScheduler",
        "RenderEntersComponent",
    ];
    // A member read in a function a call edge places elsewhere, and a props read
    // every caller passes statically.
    for (name, id, kinds) in [
        (
            "closed-two-file-caller",
            "local_access.discover_member.placed-elsewhere",
            &placing[..],
        ),
        (
            "absent-guard-prop",
            "local_access.discover_member.props-static",
            &["PropsCallersStatic"][..],
        ),
    ] {
        let log = directory.join(format!("{name}.jsonl"));
        check(&typefacts, name, Some(&log));
        let lines = lines(&log);
        let ends = lines
            .iter()
            .filter(|line| line["k"] == "silent" && line["rule"] == "end")
            .collect::<Vec<_>>();
        assert!(
            ends.iter().any(|line| line["proof"] == id),
            "{name}: no end named {id}"
        );
        for end in ends {
            let premises = end["premises"].as_array().unwrap();
            assert!(
                !premises.is_empty(),
                "{name}: an end rests on nothing: {end}"
            );
            for premise in premises {
                let at = premise["at"].as_array();
                assert!(
                    at.is_some_and(|at| at.len() == 2 && at.iter().all(|bound| bound.is_u64())),
                    "{name}: a premise names no code: {end}"
                );
            }
            if end["proof"] == id {
                for premise in premises {
                    let kind = premise["kind"].as_str().unwrap();
                    assert!(kinds.contains(&kind), "{name}: {id} rests on {kind}: {end}");
                }
            }
        }
    }
    fs::remove_dir_all(directory).ok();
}

#[test]
fn a_setter_names_its_guard_only_where_ownedwrite_is_proven_off() {
    let typefacts = env::var("SOLID_TYPEFACTS_BIN").expect("arm the real Type Facts process");
    let directory = temporary_directory("proof-log-guard");
    let log = directory.join("guard.jsonl");
    check(&typefacts, "owned-write-updater-read", Some(&log));
    let source = fs::read_to_string(fixture("owned-write-updater-read").join("App.tsx")).unwrap();
    // The updater call `setA(() => b())` written in the function `name`.
    let setter_call = |name: &str| {
        let function = source.find(&format!("export function {name}()")).unwrap();
        let start = function + source[function..].find("setA(() =>").unwrap();
        let end = start + source[start..].find(");").unwrap() + 1;
        (start as u64, end as u64)
    };
    let named = lines(&log)
        .iter()
        .filter(|line| {
            line["k"] == "silent"
                && line["rule"] == "end"
                && line["proof"] == "local_access.discover_call_as.setter-guard-throws-first"
        })
        .flat_map(|line| {
            let premises = line["premises"].as_array().unwrap().clone();
            assert!(
                premises
                    .iter()
                    .all(|premise| premise["kind"] == "SetterGuardThrowsFirst"),
                "the guard end rests on another fact: {line}"
            );
            premises
                .iter()
                .map(|premise| {
                    let at = premise["at"].as_array().unwrap();
                    (at[0].as_u64().unwrap(), at[1].as_u64().unwrap())
                })
                .collect::<Vec<_>>()
        })
        .collect::<BTreeSet<_>>();
    // A plain setter and `ownedWrite: false` throw first (the write is SC2001's);
    // `ownedWrite: true` runs the updater and an options value the syntax does
    // not show leaves it unproven: neither names an end.
    assert_eq!(
        named,
        BTreeSet::from([setter_call("Plain"), setter_call("OwnedOff")])
    );
    fs::remove_dir_all(directory).ok();
}
