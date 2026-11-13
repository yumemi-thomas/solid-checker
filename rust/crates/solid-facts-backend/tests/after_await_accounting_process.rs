//! Batch 11, slice 9B (ADR 0973): every read and call of a project function written after an async function's own suspension ends, or is uncertifiable.
//!
//! `after_await_accounting.rs` lists the sites (the reads of the universe and the calls written after their async function's own suspension) and each must end in a
//! row of SC1002, an SC9012, the compiler's discard or a typed reason; one that ends in none gets an uncertifiable row of SC1002, blocker
//! `read-after-await-unaccounted`, and never a violation.
//!
//! The mutation: `SOLID_CHECKER_IR_ABLATE_SILENCE=<id>` drops one reason, and the sites it ended must then each carry the row, with no other answer moved.
use std::{collections::BTreeSet, env, fs, path::Path, process::Command};

const BLOCKER: &str = "read-after-await-unaccounted";
const NEVER_RUNS: &str = "after_await_accounting.account.never-runs";

fn fixture(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../fixtures/reactive-ir")
        .join(name)
}

/// The findings of a one-shot check of the fixture `name`, with `ablate` the reasons to drop.
fn check(typefacts: &str, name: &str, ablate: Option<&str>) -> Vec<serde_json::Value> {
    let mut command = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"));
    command
        .env("SOLID_TYPEFACTS_BIN", typefacts)
        .env("SOLID_CHECKER_DAEMON", "0")
        .env_remove("SOLID_CHECKER_IR_ABLATE_SILENCE")
        .args(["--format", "json", "--project"])
        .arg(fixture(name).join("tsconfig.json"));
    if let Some(ablate) = ablate {
        command.env("SOLID_CHECKER_IR_ABLATE_SILENCE", ablate);
    }
    let output = command.output().unwrap();
    assert!(
        output.status.success() || output.status.code() == Some(1),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    report["findings"].as_array().unwrap().clone()
}

/// A finding as a comparable string: code, kind, place, blocker.
fn key(finding: &serde_json::Value) -> String {
    format!(
        "{} {} {}:{}:{} {}",
        finding["id"],
        finding["kind"],
        finding["primaryLocation"]["path"],
        finding["primaryLocation"]["line"],
        finding["primaryLocation"]["column"],
        finding["analysisContext"]
    )
}

/// The start bytes of the rows the pass files: SC1002, blocker `read-after-await-unaccounted`. Each is uncertifiable, never a violation.
fn unaccounted(findings: &[serde_json::Value]) -> BTreeSet<u64> {
    findings
        .iter()
        .filter(|finding| finding["analysisContext"] == BLOCKER)
        .map(|finding| {
            assert_eq!(finding["id"], "SC1002", "the row is SC1002's");
            assert_eq!(finding["kind"], "uncertifiable", "never a violation");
            finding["primaryLocation"]["startByte"].as_u64().unwrap()
        })
        .collect()
}

/// Where `skip` ends inside the first occurrence of `anchor` in `source`.
fn at(source: &str, anchor: &str, skip: &str) -> u64 {
    let found = source
        .find(anchor)
        .unwrap_or_else(|| panic!("no `{anchor}`"));
    (found + skip.len()) as u64
}

fn source(name: &str) -> String {
    fs::read_to_string(fixture(name).join("App.tsx")).unwrap()
}

/// Every answer of the named run is still there in the ablated run, and what is new is exactly `added`.
fn only_these_moved(
    named: &[serde_json::Value],
    ablated: &[serde_json::Value],
    added: &BTreeSet<u64>,
) {
    let before = named.iter().map(key).collect::<BTreeSet<_>>();
    let after = ablated.iter().map(key).collect::<BTreeSet<_>>();
    assert!(before.is_subset(&after), "an answer was lost");
    let new = ablated
        .iter()
        .filter(|finding| !before.contains(&key(finding)))
        .map(|finding| {
            assert_eq!(
                finding["analysisContext"], BLOCKER,
                "an answer other than the unaccounted rows moved"
            );
            finding["primaryLocation"]["startByte"].as_u64().unwrap()
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(&new, added, "the rows the dropped reason ended");
}

#[test]
fn a_site_no_reason_ends_gets_sc1002s_uncertifiable_row() {
    let typefacts = env::var("SOLID_TYPEFACTS_BIN").expect("arm the real Type Facts process");
    let name = "after-await-universe";
    let source = source(name);
    let rows = unaccounted(&check(&typefacts, name, None));
    // A read after an await in an async function no computation runs, a call of a project function whose closure is not silent, a call of a function value.
    for (anchor, skip, what) in [
        (
            "count();\n      }}\n    >",
            "",
            "a read in an event handler",
        ),
        (
            "return count();\n}\n\n// A call",
            "return ",
            "a read in a helper",
        ),
        (
            "stringify(1);\n  readsCount",
            "",
            "a callee that calls the standard library",
        ),
        ("readsCount();\n  ping", "", "a callee that reads a source"),
        ("ping(3);\n}", "", "a callee that calls itself"),
        ("callback();\n}", "", "a function value"),
        ("reassigned();\n}", "", "a reassigned `let`"),
        ("count();\n    void item", "", "a read after a `for await`"),
    ] {
        let start = at(&source, anchor, skip);
        assert!(rows.contains(&start), "{what}: no SC1002 row at {start}");
    }
}

#[test]
fn the_reason_ends_exactly_its_sites() {
    let typefacts = env::var("SOLID_TYPEFACTS_BIN").expect("arm the real Type Facts process");
    let name = "after-await-universe-none";
    let source = source(name);
    let named = check(&typefacts, name, None);
    assert!(
        unaccounted(&named).is_empty(),
        "a site is unaccounted with every reason named"
    );
    let never = BTreeSet::from([
        at(&source, "stringify(1);", ""),
        at(&source, "stringify(2);", ""),
    ]);
    // The reason by id, not `all`: `all` also drops every other pass's silences (the owner stage's, the flush accounting's), which file rows of their own.
    let ablated = check(&typefacts, name, Some(NEVER_RUNS));
    only_these_moved(&named, &ablated, &never);
}

#[test]
fn a_program_without_a_recognized_computation_keeps_every_sites_row() {
    let typefacts = env::var("SOLID_TYPEFACTS_BIN").expect("arm the real Type Facts process");
    let name = "after-await-universe-census";
    let source = source(name);
    let named = check(&typefacts, name, None);
    let rows = unaccounted(&named);
    // ADR 0973: there is no census reason. The program holds no tracked async computation the producers find, and every read written after an await keeps its row: the
    // functions the program runs itself, the exported one, and `work`, a function declaration handed by identifier to a wrapper that gives it to `createMemo` (Chrome
    // runs it as a computation: the oracle case `after-await-universe-census-wrapper`).
    for (anchor, skip, what) in [
        (
            "async function load() {\n  await pause();\n  return count()",
            "async function load() {\n  await pause();\n  return ",
            "a read in a function the program calls at module level",
        ),
        (
            "await pause();\n  count();\n  return load()",
            "await pause();\n  ",
            "a read in a function that calls another",
        ),
        (
            "return load();",
            "return ",
            "a call of a project function whose closure reads a source",
        ),
        (
            "export async function exported() {\n  await pause();\n  return count()",
            "export async function exported() {\n  await pause();\n  return ",
            "a read in an exported function",
        ),
        (
            "async function work() {\n  await pause();\n  return count()",
            "async function work() {\n  await pause();\n  return ",
            "a read in a function declaration handed to a wrapper that gives it to `createMemo`",
        ),
    ] {
        let start = at(&source, anchor, skip);
        assert!(rows.contains(&start), "{what}: no SC1002 row at {start}");
    }
    // No typed reason holds at any of these sites: dropping it moves nothing.
    let ablated = check(&typefacts, name, Some(NEVER_RUNS));
    only_these_moved(&named, &ablated, &BTreeSet::new());
}
