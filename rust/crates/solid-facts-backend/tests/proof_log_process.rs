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
    check_with(typefacts, name, log, &[])
}

/// [`check`] with more environment: the dry runs of the proof log's measurement hooks.
fn check_with(
    typefacts: &str,
    name: &str,
    log: Option<&Path>,
    environment: &[(&str, &str)],
) -> serde_json::Value {
    let mut command = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"));
    command
        .env("SOLID_TYPEFACTS_BIN", typefacts)
        .env("SOLID_CHECKER_DAEMON", "0")
        .envs(environment.iter().copied())
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
        // Batch 11, slice 6: the owner-call universe's silences and declines.
        "owner-call-leaf-universe",
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

// Batch 11, slice 6 (coordinator-approved, owner delegated): a decline is no ending. `stored` is kept in an array, so it is entered by a reference that is no direct call and no
// silence applies: the log names why (a `declined` line listing the code), the site keeps SC3001's uncertifiable `leaf-operation-unaccounted`, and no silence lists the code at the
// span. Nothing reads a decline back.
#[test]
fn an_owner_call_decline_ends_nothing() {
    let typefacts = env::var("SOLID_TYPEFACTS_BIN").expect("arm the real Type Facts process");
    let directory = temporary_directory("proof-log-declined");
    let log = directory.join("owner-call.jsonl");
    let report = check(&typefacts, "owner-call-leaf-universe", Some(&log));
    let source =
        fs::read_to_string(fixture("owner-call-leaf-universe").join("App.tsx")).unwrap();
    let start = source.find("createMemo(() => 4)").unwrap() as u64;
    let end = start + "createMemo".len() as u64;
    let code = "PRIMITIVE_IN_FORBIDDEN_SCOPE";
    // The report: the code's own uncertifiable row stands at the callee.
    let rows = report["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|finding| {
            finding["id"] == "SC3001"
                && finding["primaryLocation"]["startByte"] == start
                && finding["primaryLocation"]["endByte"] == end
        })
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 1, "SC3001's row at the creation: {rows:?}");
    assert_eq!(rows[0]["kind"], "uncertifiable");
    assert_eq!(rows[0]["analysisContext"], "leaf-operation-unaccounted");
    // The log: a decline lists the code at the span, and no named end does.
    let lines = lines(&log);
    let lists = |line: &serde_json::Value| {
        line["s"] == start
            && line["e"] == end
            && line["codes"]
                .as_array()
                .is_some_and(|codes| codes.iter().any(|listed| listed == code))
    };
    assert!(
        lines.iter().any(|line| line["k"] == "declined" && lists(line)),
        "no decline lists {code} at the callee"
    );
    assert!(
        !lines.iter().any(|line| line["k"] == "silent" && lists(line)),
        "a silence lists {code} at a site only a decline stands at"
    );
    fs::remove_dir_all(directory).ok();
}

// Batch 11, slice 0: a finding and a verdict name the rule that reported them, so a fold can say which rule ended a site (`scripts/lib/proof-credit.mjs`).
#[test]
fn a_finding_and_a_verdict_name_their_rule() {
    let typefacts = env::var("SOLID_TYPEFACTS_BIN").expect("arm the real Type Facts process");
    let directory = temporary_directory("proof-log-rules");
    let log = directory.join("latent.jsonl");
    check(&typefacts, "latent-silent-arms", Some(&log));
    let found = lines(&log)
        .into_iter()
        .filter(|line| line["k"] == "finding" || line["k"] == "verdict")
        .collect::<Vec<_>>();
    assert!(!found.is_empty(), "no finding or verdict line");
    for line in &found {
        assert!(line["code"].as_str().unwrap().starts_with("SC"), "{line}");
        assert!(
            line["rule"].as_str().is_some_and(|rule| !rule.is_empty()),
            "a {} line names no rule: {line}",
            line["k"]
        );
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

// Batch 11, slice 0: the dry run that lists the candidate sites of the rules with no site universe (`proof_candidates.rs`) is asked for, writes nothing unasked, and moves no answer.
#[test]
fn the_candidates_are_listed_only_when_asked_and_move_no_answer() {
    let typefacts = env::var("SOLID_TYPEFACTS_BIN").expect("arm the real Type Facts process");
    let directory = temporary_directory("proof-log-candidates");
    let mut seen = BTreeSet::new();
    let mut loading_values = BTreeSet::new();
    let mut sync_states = BTreeSet::new();
    let mut census = Vec::new();
    let mut computations = BTreeSet::new();
    // `resolve` and `until`, the effect and computation primitives with `sync` options, `Loading` boundaries with `on`, and a namespace `flush`.
    for name in [
        "unplaced-site-obligations",
        "static-api",
        "rc21-loading-on-accessor",
        "flush-veto-namespace",
    ] {
        let off = directory.join(format!("{name}.off.jsonl"));
        let zero = directory.join(format!("{name}.zero.jsonl"));
        let on = directory.join(format!("{name}.on.jsonl"));
        let without = check_with(&typefacts, name, Some(&off), &[]);
        let declined = check_with(
            &typefacts,
            name,
            Some(&zero),
            &[("SOLID_CHECKER_IR_PROOF_CANDIDATES", "0")],
        );
        let asked = check_with(
            &typefacts,
            name,
            Some(&on),
            &[("SOLID_CHECKER_IR_PROOF_CANDIDATES", "1")],
        );
        assert_eq!(without, asked, "{name}: the dry run moved an answer");
        assert_eq!(without, declined, "{name}: a `0` moved an answer");
        for unasked in [&off, &zero] {
            assert!(
                lines(unasked).iter().all(|line| line["k"] != "candidate"),
                "{name}: a candidate was written unasked"
            );
        }
        let before = lines(&off);
        let after = lines(&on);
        // The log with the dry run is the log without it, plus the candidates: no other line moves.
        let others = |all: &[serde_json::Value]| {
            all.iter()
                .filter(|line| line["k"] != "candidate" && line["k"] != "path")
                .map(|line| {
                    (
                        line["k"].to_string(),
                        line["s"].to_string(),
                        line["e"].to_string(),
                    )
                })
                .collect::<BTreeSet<_>>()
        };
        assert_eq!(
            others(&before),
            others(&after),
            "{name}: the dry run moved a line"
        );
        for line in after.iter().filter(|line| line["k"] == "candidate") {
            let rule = line["rule"].as_str().unwrap();
            let table = line["t"].as_str().unwrap();
            assert!(
                line["s"].is_u64()
                    && line["e"].is_u64()
                    && line["alt"].is_array()
                    && line["facts"].is_object(),
                "{name}: a candidate names no place: {line}"
            );
            seen.insert(format!("{rule} {table}"));
            match (rule, table) {
                ("SC5007", "loading-on-value") => {
                    loading_values.insert(line["facts"]["value"].as_str().unwrap().to_owned());
                }
                ("SC7002", "sync-option") => {
                    sync_states.insert(line["facts"]["sync"].as_str().unwrap().to_owned());
                }
                ("flush", "census") => census.push((name, line["facts"].clone())),
                ("SC1002", "read-after-await") => {
                    computations.insert(line["facts"]["computation"].as_str().unwrap().to_owned());
                }
                _ => {}
            }
        }
    }
    for expected in [
        "SC1002 read-after-await",
        "SC2004 resolve",
        "SC2005 until",
        "SC5006 loading-on",
        "SC5007 loading-on-value",
        "SC7001 effect-function-slot",
        "SC7002 sync-option",
        "flush census",
    ] {
        assert!(
            seen.contains(expected),
            "no candidate `{expected}`: {seen:?}"
        );
    }
    // What a value is proven to be: an accessor handed to `on` is a function, a call's result is not, a prop is open.
    assert_eq!(
        loading_values,
        BTreeSet::from([
            "function".to_owned(),
            "not-function".to_owned(),
            "open".to_owned()
        ])
    );
    // A read after a dominating await is a candidate of SC1002, in a computation's function (`createMemo(async () => ...)`) or in another async function.
    assert!(
        computations.contains("yes"),
        "no read after an await in a computation: {computations:?}"
    );
    // `sync: true` is a site of SC7002 and so is a call whose options the syntax does not show.
    assert!(
        sync_states.contains("true") && sync_states.contains("unknown"),
        "{sync_states:?}"
    );
    assert!(
        census
            .iter()
            .any(|(name, facts)| *name == "flush-veto-namespace" && facts["called"] == "yes"),
        "no called flush site in the census: {census:?}"
    );
    fs::remove_dir_all(directory).ok();
}
