//! ADR 0950: the what-if checker, a build with the cargo feature `what-if` (`make test-what-if`; `make build-what-if` builds the binary).
//!
//! `--assume <hazard>` treats a named hazard fact as absent, in one seam, and every report says so: a top-level `assumed` always, and a typed `assumed` premise
//! on every finding once an answer flipped. The binary is one-shot (no daemon, no `--certify`, no request on stdin) and answers the internal JSON only.
//! `what_if_default_process.rs` is the same fixture on a default build, which has none of this.
#[path = "support/what_if.rs"]
mod what_if_support;

use std::{
    env, fs,
    io::Write,
    path::PathBuf,
    process::{Command, Output, Stdio},
};

use serde_json::{Value, json};
use what_if_support::{
    FIXTURE, WHAT_IF_MARKERS, contains, daemon_traces, fixture, socket_directory,
};

const BINARY: &str = env!("CARGO_BIN_EXE_solid-checker-rust");

fn typefacts() -> String {
    env::var("SOLID_TYPEFACTS_BIN").expect("arm the real Type Facts process")
}

fn tsconfig() -> PathBuf {
    fixture(FIXTURE).join("tsconfig.json")
}

/// The what-if binary on the fixture with `extra` arguments after the project, one-shot.
fn run(extra: &[&str]) -> Output {
    Command::new(BINARY)
        .env("SOLID_TYPEFACTS_BIN", typefacts())
        .env("SOLID_CHECKER_DAEMON", "0")
        .arg("--project")
        .arg(tsconfig())
        .args(extra)
        .output()
        .unwrap()
}

/// The report of a check that must succeed, with `assume` assumed.
fn check(assume: &[&str]) -> Value {
    let assumed = assume.join(",");
    let mut arguments = vec!["--format", "json"];
    if !assume.is_empty() {
        arguments.extend(["--assume", &assumed]);
    }
    let output = run(&arguments);
    assert!(
        output.status.success() || output.status.code() == Some(1),
        "{assume:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("{assume:?}: not JSON ({error})"))
}

fn findings(report: &Value) -> &Vec<Value> {
    report["findings"].as_array().expect("findings")
}

/// The findings without the taint, to compare what the assumption left with what it did not touch.
fn untainted(report: &Value) -> Vec<Value> {
    findings(report)
        .iter()
        .map(|finding| {
            let mut finding = finding.clone();
            finding.as_object_mut().unwrap().remove("premises");
            finding
        })
        .collect()
}

#[test]
fn nothing_assumed_changes_no_row_and_says_so() {
    let report = check(&[]);
    assert_eq!(report["assumed"], json!({ "hazards": [], "flipped": [] }));
    assert_eq!(report["status"], "uncertifiable");
    assert_eq!(findings(&report).len(), 3, "{report:#}");
    for finding in findings(&report) {
        assert!(
            finding.get("premises").is_none(),
            "an untouched finding carries a premise: {finding:#}"
        );
    }
    // The vetoes name the hazards present, in the report's `blockedBy`.
    let fired: Vec<&Value> = findings(&report)
        .iter()
        .filter_map(|finding| finding["blockedBy"]["fired"].as_array())
        .flatten()
        .collect();
    assert!(
        fired.contains(&&json!("StandardRootHanded"))
            && fired.contains(&&json!("OpenMemberWrite(*)")),
        "{fired:?}"
    );
}

#[test]
fn an_assumed_hazard_the_program_has_changes_a_row_and_taints_the_report() {
    let baseline = check(&[]);
    // The veto is a formula over the hazards: with every open write absent (or the handed root absent) no member may be an impostor, and the rows go.
    for hazard in ["OpenMemberWrite(*)", "StandardRootHanded"] {
        let report = check(&[hazard]);
        assert_eq!(report["status"], "certified", "{hazard}: {report:#}");
        assert!(findings(&report).is_empty(), "{hazard}: {report:#}");
        assert_eq!(
            report["assumed"],
            json!({ "hazards": [hazard], "flipped": [hazard] }),
            "{hazard}: a report with a row gone must say what was assumed"
        );
    }
    assert_eq!(findings(&baseline).len(), 3);
}

#[test]
fn an_assumed_hazard_that_leaves_a_row_marks_the_row_with_the_premise() {
    let baseline = check(&[]);
    // `StandardRootReceived` is one of the four the veto names, not one of its alternatives: the rows stay, and are the baseline's but for the premise and `fired`.
    let report = check(&["StandardRootReceived"]);
    assert_eq!(
        report["assumed"],
        json!({ "hazards": ["StandardRootReceived"], "flipped": ["StandardRootReceived"] })
    );
    assert_eq!(findings(&report).len(), findings(&baseline).len());
    for finding in findings(&report) {
        assert_eq!(
            finding["premises"],
            json!([{ "kind": "assumed", "hazards": ["StandardRootReceived"] }]),
            "{finding:#}"
        );
    }
    for (assumed, default) in untainted(&report).iter().zip(findings(&baseline)) {
        for key in ["rule", "kind", "primaryLocation", "status"] {
            assert_eq!(assumed.get(key), default.get(key), "{key}");
        }
        let fired = assumed["blockedBy"]["fired"]
            .as_array()
            .into_iter()
            .flatten();
        assert!(
            fired.clone().all(|id| id != "StandardRootReceived"),
            "the assumed hazard is still fired: {assumed:#}"
        );
    }
}

#[test]
fn an_assumed_hazard_the_program_lacks_flips_nothing_and_taints_nothing() {
    let baseline = check(&[]);
    let report = check(&["RootPatch"]);
    assert_eq!(
        report["assumed"],
        json!({ "hazards": ["RootPatch"], "flipped": [] })
    );
    assert_eq!(
        findings(&report),
        findings(&baseline),
        "assuming a hazard the program does not have moved a finding or marked one"
    );
    assert_eq!(report["status"], baseline["status"]);
}

/// The kind of each finding's `blockedBy`, where it has one.
fn blocked_kinds(report: &Value) -> Vec<String> {
    findings(report)
        .iter()
        .filter_map(|finding| finding["blockedBy"]["kind"].as_str())
        .map(str::to_owned)
        .collect()
}

/// The report of the fixture `name` (`fixtures/reactive-ir/<name>`, the browser development build) with `assume` assumed.
fn check_fixture(name: &str, assume: &[&str]) -> Value {
    let assumed = assume.join(",");
    let tsconfig = fixture(name).join("tsconfig.json");
    let mut command = Command::new(BINARY);
    command
        .env("SOLID_TYPEFACTS_BIN", typefacts())
        .env("SOLID_CHECKER_DAEMON", "0")
        .args(["--format", "json", "--runtime-target", "browser"])
        .args(["--runtime-build", "development", "--project"])
        .arg(tsconfig);
    if !assume.is_empty() {
        command.args(["--assume", &assumed]);
    }
    let output = command.output().unwrap();
    assert!(
        output.status.success() || output.status.code() == Some(1),
        "{name} {assume:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("{name} {assume:?}: not JSON ({error})"))
}

// ADR 0965: a typed cause is a hazard too. `Premise(<blockedBy.kind>)` takes every uncertifiable finding of that kind out of the report, says so, and leaves the rest of
// the report to the baseline's but for the taint.
#[test]
fn an_assumed_premise_takes_the_rows_of_its_blocker_out_and_taints_the_rest() {
    let fixture_name = "stored-function-after-await";
    let baseline = check_fixture(fixture_name, &[]);
    let kind = "unresolved-invocation";
    assert!(
        blocked_kinds(&baseline).iter().any(|found| found == kind),
        "the fixture holds rows of `{kind}`: {:?}",
        blocked_kinds(&baseline)
    );
    let id = format!("Premise({kind})");
    let report = check_fixture(fixture_name, &[id.as_str()]);
    assert_eq!(
        report["assumed"],
        json!({ "hazards": [id.as_str()], "flipped": [id.as_str()] })
    );
    assert!(
        blocked_kinds(&report).iter().all(|found| found != kind),
        "a row of the assumed cause is left: {:?}",
        blocked_kinds(&report)
    );
    // Only those rows left: every other finding of the baseline stands (by rule, kind, place and blocker), each marked with the premise.
    let key = |finding: &Value| {
        (
            finding["rule"].as_str().unwrap_or("").to_owned(),
            finding["kind"].as_str().unwrap_or("").to_owned(),
            finding["primaryLocation"].to_string(),
            finding["blockedBy"]["kind"]
                .as_str()
                .unwrap_or("")
                .to_owned(),
        )
    };
    let kept = findings(&baseline)
        .iter()
        .filter(|finding| finding["blockedBy"]["kind"] != kind)
        .map(key)
        .collect::<Vec<_>>();
    assert_eq!(
        untainted(&report).iter().map(key).collect::<Vec<_>>(),
        kept,
        "the assumption moved a finding of another cause"
    );
    assert!(
        findings(&report).len() < findings(&baseline).len(),
        "the assumption lifted nothing"
    );
    for finding in findings(&report) {
        assert_eq!(
            finding["premises"],
            json!([{ "kind": "assumed", "hazards": [id.as_str()] }]),
            "{finding:#}"
        );
    }
    // A violation is no uncertifiable row: the cause does not touch it.
    assert!(
        findings(&report)
            .iter()
            .any(|finding| finding["kind"] == "violation"),
        "{report:#}"
    );
}

#[test]
fn every_cause_of_a_program_assumed_away_certifies_it() {
    // The fixture's three rows are two `standard-member-impostor` and one `function-run-unplaced`: the baseline's blockers are exactly those two kinds, and assuming both
    // away leaves nothing, so the status is the assumption's.
    let baseline = check(&[]);
    let mut kinds = blocked_kinds(&baseline);
    kinds.sort();
    kinds.dedup();
    assert_eq!(kinds, ["function-run-unplaced", "standard-member-impostor"]);
    let one = check(&["Premise(standard-member-impostor)"]);
    assert_eq!(one["status"], "uncertifiable", "{one:#}");
    assert_eq!(blocked_kinds(&one), ["function-run-unplaced"]);
    let both = check(&[
        "Premise(standard-member-impostor)",
        "Premise(function-run-unplaced)",
    ]);
    assert_eq!(both["status"], "certified", "{both:#}");
    assert!(findings(&both).is_empty(), "{both:#}");
    assert_eq!(
        both["assumed"],
        json!({
            "hazards": ["Premise(standard-member-impostor)", "Premise(function-run-unplaced)"],
            "flipped": ["Premise(function-run-unplaced)", "Premise(standard-member-impostor)"]
        })
    );
}

#[test]
fn an_assumed_premise_no_row_carries_moves_no_finding() {
    // A premise is present wherever the seam is asked (no census holds it), so the run is tainted: the findings are the baseline's but for the premise.
    let baseline = check(&[]);
    let report = check(&["Premise(search-depth-limit)"]);
    assert_eq!(
        report["assumed"],
        json!({
            "hazards": ["Premise(search-depth-limit)"],
            "flipped": ["Premise(search-depth-limit)"]
        })
    );
    assert_eq!(&untainted(&report), findings(&baseline));
    assert_eq!(report["status"], baseline["status"]);
}

// ADR 0965, slice 2: the two stops of the slot climb that owe an obligation (ADR 0959) are SC3001's uncertifiable `leaf-owner-unproven` rows in every report, the blocker every
// uncertain leaf operation shares. The what-if build tells them apart: a stop's premise lifts the rows owed at that stop and no other, and `Premise(leaf-owner-unproven)` the rest.
const OWED: &str = "leaf-owner-unproven";
const THE_REST: &str = "Premise(leaf-owner-unproven)";
const IN_PLACE: &str = "Premise(leaf-owner-climb-unmodelled-in-place)";
const BOUND: &str = "Premise(leaf-owner-climb-bound)";

/// What identifies a finding in a comparison: rule, kind, place and blocker.
fn finding_key(finding: &Value) -> (String, String, String, String) {
    (
        finding["rule"].as_str().unwrap_or("").to_owned(),
        finding["kind"].as_str().unwrap_or("").to_owned(),
        finding["primaryLocation"].to_string(),
        finding["blockedBy"]["kind"]
            .as_str()
            .unwrap_or("")
            .to_owned(),
    )
}

/// The places of the findings whose blocker is `kind`.
fn places_of(report: &Value, kind: &str) -> Vec<String> {
    findings(report)
        .iter()
        .filter(|finding| finding["blockedBy"]["kind"] == kind)
        .map(|finding| finding["primaryLocation"].to_string())
        .collect()
}

/// The places of the rows of `OWED` in the fixture `name` that stand in its baseline and not with `premise` assumed. The run is the premise's alone: every finding that stays is the
/// baseline's, each marked with it, and every other finding of the baseline is there.
fn lifted(name: &str, premise: &str) -> Vec<String> {
    let baseline = check_fixture(name, &[]);
    let report = check_fixture(name, &[premise]);
    assert_eq!(
        report["assumed"],
        json!({ "hazards": [premise], "flipped": [premise] }),
        "{name} {premise}"
    );
    let before = places_of(&baseline, OWED);
    let after = places_of(&report, OWED);
    let gone: Vec<String> = before
        .iter()
        .filter(|place| !after.contains(place))
        .cloned()
        .collect();
    let kept: Vec<_> = findings(&baseline)
        .iter()
        .filter(|finding| {
            !(finding["kind"] == "uncertifiable"
                && finding["blockedBy"]["kind"] == OWED
                && gone.contains(&finding["primaryLocation"].to_string()))
        })
        .map(finding_key)
        .collect();
    assert_eq!(
        untainted(&report)
            .iter()
            .map(finding_key)
            .collect::<Vec<_>>(),
        kept,
        "{name} {premise}: the assumption moved a finding that is no row of `{OWED}`"
    );
    for finding in findings(&report) {
        assert_eq!(
            finding["premises"],
            json!([{ "kind": "assumed", "hazards": [premise] }]),
            "{name} {premise}: {finding:#}"
        );
    }
    gone
}

#[test]
fn a_callback_the_climb_cannot_read_is_a_cause_of_its_own() {
    // `leaf-unmodelled-slot`: a call under a callback with a parameter, an async one, `latest`'s and a namespace spelling, each of a primitive that runs it in place (the
    // climb's `UnmodelledInPlace`), and a plain `untrack` the walk proves (a violation, no row).
    let name = "leaf-unmodelled-slot";
    let rows = places_of(&check_fixture(name, &[]), OWED);
    assert_eq!(rows.len(), 5, "{rows:?}");
    assert_eq!(
        lifted(name, IN_PLACE),
        rows,
        "the stop lifts every row it owes"
    );
    assert!(
        lifted(name, THE_REST).is_empty(),
        "the rest lifts a row of a stop"
    );
    assert!(
        lifted(name, BOUND).is_empty(),
        "the bound lifts a row of another stop"
    );
}

#[test]
fn the_bound_of_the_climb_is_a_cause_of_its_own() {
    // `leaf-slot-climb-bound`: a call under seventeen in-place layers (a row), and under sixteen, which the walk crosses (a violation).
    let name = "leaf-slot-climb-bound";
    let rows = places_of(&check_fixture(name, &[]), OWED);
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert_eq!(lifted(name, BOUND), rows);
    assert!(lifted(name, IN_PLACE).is_empty());
    assert!(lifted(name, THE_REST).is_empty());
    assert!(
        findings(&check_fixture(name, &[BOUND]))
            .iter()
            .any(|finding| finding["kind"] == "violation"),
        "the violation at sixteen layers is no row of the cause"
    );
}

#[test]
fn a_leaf_row_owed_for_another_reason_is_the_rests_and_no_stops() {
    // `flush-in-tracked-named-async`: the rows of a named async callback a slot takes the throw of (ADR 0959, amended): no climb stop owes them.
    let name = "flush-in-tracked-named-async";
    let rows = places_of(&check_fixture(name, &[]), OWED);
    assert_eq!(rows.len(), 4, "{rows:?}");
    assert_eq!(lifted(name, THE_REST), rows);
    assert!(lifted(name, IN_PLACE).is_empty());
    assert!(lifted(name, BOUND).is_empty());
}

#[test]
fn every_reason_assumed_lifts_every_row_and_assuming_none_changes_none() {
    // The stops and the rest together are the cause as it was before the stops were told apart: all five rows of `leaf-unmodelled-slot`; the control moves nothing.
    let name = "leaf-unmodelled-slot";
    let all = check_fixture(name, &[THE_REST, IN_PLACE, BOUND]);
    assert!(
        places_of(&all, OWED).is_empty(),
        "a row of the cause is left: {all:#}"
    );
    let baseline = check_fixture(name, &[]);
    assert_eq!(baseline["assumed"], json!({ "hazards": [], "flipped": [] }));
    assert_eq!(places_of(&baseline, OWED).len(), 5);
    for finding in findings(&baseline) {
        assert!(finding.get("premises").is_none(), "{finding:#}");
    }
}

#[test]
fn a_premise_id_that_is_no_blocker_kind_is_an_error() {
    for id in [
        "Premise()",
        "Premise(App)",
        "Premise(function-run-unplaced:detail)",
    ] {
        let output = run(&["--format", "json", "--assume", id]);
        assert_eq!(output.status.code(), Some(2), "{id}");
        assert!(output.stdout.is_empty(), "{id}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("names no hazard fact"),
            "{id}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn several_assumptions_are_one_assumption_and_an_id_twice_is_one() {
    for arguments in [
        vec![
            "--format",
            "json",
            "--assume",
            "RootPatch,StandardRootHanded",
        ],
        vec![
            "--format",
            "json",
            "--assume",
            "RootPatch",
            "--assume",
            "StandardRootHanded",
            "--assume",
            "RootPatch",
        ],
    ] {
        let output = run(&arguments);
        let report: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(
            report["assumed"]["hazards"],
            json!(["RootPatch", "StandardRootHanded"]),
            "{arguments:?}"
        );
        assert_eq!(
            report["assumed"]["flipped"],
            json!(["StandardRootHanded"]),
            "{arguments:?}"
        );
    }
}

#[test]
fn the_taint_reaches_the_narrowed_report_too() {
    let app = fixture(FIXTURE).join("App.tsx");
    let output = run(&[
        "--format",
        "json",
        "--assume",
        "StandardRootReceived",
        "--only-file",
        app.to_str().unwrap(),
    ]);
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        report["assumed"]["flipped"],
        json!(["StandardRootReceived"])
    );
    assert!(!findings(&report).is_empty());
    for finding in findings(&report) {
        assert_eq!(
            finding["premises"],
            json!([{ "kind": "assumed", "hazards": ["StandardRootReceived"] }])
        );
    }
}

#[test]
fn the_what_if_binary_refuses_what_would_certify_or_serve_a_prediction() {
    for (arguments, said) in [
        (vec!["--certify"], "no --certify, --serve or --stop-daemon"),
        (vec!["--serve"], "no --certify, --serve or --stop-daemon"),
        (
            vec!["--stop-daemon"],
            "no --certify, --serve or --stop-daemon",
        ),
        (vec!["--format", "text"], "answers the internal JSON only"),
        (vec!["--format=eslint"], "answers the internal JSON only"),
        (vec!["--format", "sarif"], "answers the internal JSON only"),
        (vec!["--assume", "NoSuchHazard"], "names no hazard fact"),
        (
            vec!["--assume", "RootPatch,StandardPatch(nodot)"],
            "names no hazard fact",
        ),
        (vec!["--assume"], "--assume needs hazard ids"),
    ] {
        let output = run(&arguments);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(2), "{arguments:?}: {stderr}");
        assert!(
            output.stdout.is_empty(),
            "{arguments:?}: a refused run wrote a report"
        );
        assert!(stderr.contains(said), "{arguments:?}: {stderr}");
    }
}

#[test]
fn the_what_if_binary_writes_no_analysis_log() {
    // The proof and role logs and the other records a gate folds carry no taint, so a run asked to write one refuses to start (and writes nothing).
    let directory = socket_directory("logs");
    for name in [
        "SOLID_CHECKER_IR_PROOF_LOG",
        "SOLID_CHECKER_IR_ROLE_LOG",
        "SOLID_CHECKER_IR_BOOKKEEPING_LOG",
        "SOLID_CHECKER_IR_MEMO_LOG",
        "SOLID_CHECKER_IR_DELTA_LOG",
    ] {
        let log = directory.join(format!("{name}.log"));
        let output = Command::new(BINARY)
            .env("SOLID_TYPEFACTS_BIN", typefacts())
            .env("SOLID_CHECKER_DAEMON", "0")
            .env(name, &log)
            .args(["--format", "json", "--assume", "RootPatch", "--project"])
            .arg(tsconfig())
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(2), "{name}: {stderr}");
        assert!(
            output.stdout.is_empty(),
            "{name}: a refused run wrote a report"
        );
        assert!(
            stderr.contains("writes no analysis log") && stderr.contains(name),
            "{name}: {stderr}"
        );
        assert!(!log.exists(), "{name}: the refused run wrote its log");
    }
    // An empty value asks for nothing, as it does of a default build.
    let output = Command::new(BINARY)
        .env("SOLID_TYPEFACTS_BIN", typefacts())
        .env("SOLID_CHECKER_DAEMON", "0")
        .env("SOLID_CHECKER_IR_PROOF_LOG", "")
        .args(["--format", "json", "--project"])
        .arg(tsconfig())
        .output()
        .unwrap();
    assert!(
        output.status.success() || output.status.code() == Some(1),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    fs::remove_dir_all(&directory).ok();
}

#[test]
fn the_what_if_binary_reads_no_request_from_stdin() {
    // A request on stdin is how a host (the editor, the daemon's client) asks the default build to check, certify or serve; this binary has no such door.
    let request = json!({
        "projectId": tsconfig(), "generation": 1, "sources": [], "typefactsExecutable": typefacts(),
        "certify": true, "serve": true, "format": "text",
    });
    let mut child = Command::new(BINARY)
        .env("SOLID_TYPEFACTS_BIN", typefacts())
        .env("SOLID_CHECKER_DAEMON", "0")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(request.to_string().as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        !output.status.success(),
        "a request on stdin was answered: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        output.stdout.is_empty(),
        "a request on stdin was answered with a report"
    );
}

#[test]
fn the_what_if_binary_makes_no_daemon() {
    // Asked for the daemon outright it is still one-shot: no "daemon unavailable" fallback, no retained answer, no socket, no process.
    let temp = socket_directory("no-daemon");
    let output = Command::new(BINARY)
        .env("SOLID_TYPEFACTS_BIN", typefacts())
        .env("SOLID_CHECKER_DAEMON", "1")
        .env("SOLID_CHECKER_DAEMON_IDLE_SECS", "60")
        .env("SOLID_CHECKER_TIMINGS", "1")
        // As the default build's control (`what_if_default_process`) asks for it: with the shadow comparison on, no daemon is used and the test would see none either way.
        .env("SOLID_CHECKER_IR_SHADOW", "0")
        .env("TMPDIR", &temp)
        .args(["--format", "json", "--assume", "RootPatch", "--project"])
        .arg(tsconfig())
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let traces = daemon_traces(&temp, &tsconfig());
    fs::remove_dir_all(&temp).ok();
    assert!(
        output.status.success() || output.status.code() == Some(1),
        "{stderr}"
    );
    assert!(traces.is_empty(), "a what-if run left a daemon: {traces:?}");
    assert!(
        !stderr.contains("daemon unavailable") && !stderr.contains("retained-daemon"),
        "{stderr}"
    );
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["assumed"]["hazards"], json!(["RootPatch"]));
}

#[test]
fn the_what_if_binary_holds_the_strings_the_default_build_must_not() {
    // The control of `what_if_default_process::the_default_binary_holds_none_of_the_what_if_code`: the search finds them where they are.
    let bytes = fs::read(BINARY).unwrap();
    for marker in WHAT_IF_MARKERS {
        assert!(
            contains(&bytes, marker),
            "the what-if binary lacks `{marker}`: the default build's search proves nothing"
        );
    }
}
