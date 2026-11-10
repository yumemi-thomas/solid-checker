//! ADR 0936: every site of the universe ends, or is uncertifiable.
//!
//! The universe (`proof_universe.rs`) lists the reads, writes, actions, store
//! writes and `onFlush` calls from facts alone. Each ends in a row, a census
//! obligation, the compiler's discard or a silence its producer named; one that
//! ends in none gets an uncertifiable row of the rule's own code, blocker
//! `<family>-unaccounted`, and never a violation.
//!
//! The mutation: `SOLID_CHECKER_IR_ABLATE_SILENCE=<id>` drops one producer's
//! named silence, and the sites it ended must then each carry the row, with no
//! other answer moved.
use std::{collections::BTreeSet, env, path::Path, process::Command};

fn fixture(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../fixtures/reactive-ir")
        .join(name)
}

/// The findings of a one-shot check of the fixture `name`, with `ablate` the
/// silences to drop and `extra` the fixture's runtime arguments.
fn check(
    typefacts: &str,
    name: &str,
    ablate: Option<&str>,
    extra: &[&str],
) -> Vec<serde_json::Value> {
    let mut command = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"));
    command
        .env("SOLID_TYPEFACTS_BIN", typefacts)
        .env("SOLID_CHECKER_DAEMON", "0")
        .env_remove("SOLID_CHECKER_IR_ABLATE_SILENCE")
        .args(["--format", "json", "--project"])
        .arg(fixture(name).join("tsconfig.json"))
        .args(extra);
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

fn unaccounted(findings: &[serde_json::Value], blocker: &str) -> BTreeSet<String> {
    findings
        .iter()
        .filter(|finding| finding["analysisContext"] == blocker)
        .map(key)
        .collect()
}

#[test]
fn a_silence_a_producer_does_not_name_leaves_the_site_an_uncertifiable_row() {
    let typefacts = env::var("SOLID_TYPEFACTS_BIN").expect("arm the real Type Facts process");
    // `Absent`, `Passed` and `Defaulted` read `p.on` where every caller passes a
    // static value: the read ends in a named silence, no row.
    let name = "absent-guard-prop";
    let named = check(&typefacts, name, None, &[]);
    assert!(
        unaccounted(&named, "strict-read-unaccounted").is_empty(),
        "a read is unaccounted with every silence named"
    );
    let ablated = check(
        &typefacts,
        name,
        Some("local_access.discover_member.props-static"),
        &[],
    );
    let added = unaccounted(&ablated, "strict-read-unaccounted");
    assert!(
        !added.is_empty(),
        "dropping the props-static silence moved nothing: the sites it ends are not accounted"
    );
    for finding in ablated
        .iter()
        .filter(|finding| finding["analysisContext"] == "strict-read-unaccounted")
    {
        assert_eq!(finding["id"], "SC1001");
        assert_eq!(finding["kind"], "uncertifiable", "never a violation");
    }
    // Nothing else moved: the other answers are the named run's, byte for byte.
    let before = named.iter().map(key).collect::<BTreeSet<_>>();
    let after = ablated.iter().map(key).collect::<BTreeSet<_>>();
    assert!(before.is_subset(&after), "an answer was lost");
    assert_eq!(
        after.difference(&before).cloned().collect::<BTreeSet<_>>(),
        added,
        "an answer other than the unaccounted rows moved"
    );
}

#[test]
fn an_unnamed_silence_changes_no_other_family() {
    let typefacts = env::var("SOLID_TYPEFACTS_BIN").expect("arm the real Type Facts process");
    // Store writes the TypeScript-owned and exempt-standing silences end.
    let name = "store-delete";
    let named = check(&typefacts, name, None, &[]);
    let ablated = check(&typefacts, name, Some("all"), &[]);
    let before = named.iter().map(key).collect::<BTreeSet<_>>();
    let after = ablated.iter().map(key).collect::<BTreeSet<_>>();
    assert!(before.is_subset(&after), "an answer was lost");
    for added in after.difference(&before) {
        assert!(
            added.contains("-unaccounted"),
            "dropping every silence moved an answer that is no unaccounted row: {added}"
        );
    }
}

/// The rows dropping `silence` adds to the fixture `name`: each an
/// uncertifiable row of the family, and nothing else moves.
fn rows_the_silence_ends(
    typefacts: &str,
    name: &str,
    silence: &str,
    extra: &[&str],
) -> BTreeSet<String> {
    let named = check(typefacts, name, None, extra);
    let ablated = check(typefacts, name, Some(silence), extra);
    let before = named.iter().map(key).collect::<BTreeSet<_>>();
    let after = ablated.iter().map(key).collect::<BTreeSet<_>>();
    assert!(before.is_subset(&after), "an answer was lost");
    let added = after.difference(&before).cloned().collect::<BTreeSet<_>>();
    for row in &added {
        assert!(
            row.contains("SC1001")
                && row.contains("\"uncertifiable\"")
                && row.contains("-unaccounted"),
            "dropping `{silence}` moved an answer that is no unaccounted read: {row}"
        );
    }
    added
}

#[test]
fn a_read_no_strict_read_window_holds_ends_on_the_fact_that_says_so() {
    let typefacts = env::var("SOLID_TYPEFACTS_BIN").expect("arm the real Type Facts process");
    // A closed program (ADR 0203): the exports nothing calls never run, and the
    // read past an await runs on a later task.
    let closed = ["--program-boundary", "closed"];
    let name = "reads-entered-unseen-closed";
    assert!(
        unaccounted(
            &check(&typefacts, name, None, &closed),
            "strict-read-unaccounted"
        )
        .is_empty(),
        "a read is unaccounted with every silence named"
    );
    let nothing_enters = rows_the_silence_ends(
        &typefacts,
        name,
        "local_access.no_window.nothing-enters",
        &closed,
    );
    // `bump`, `peek`, the two reads of `readStore`, `arrow`, `Shaped`.
    assert_eq!(nothing_enters.len(), 6, "{nothing_enters:?}");
    let after_await = rows_the_silence_ends(
        &typefacts,
        name,
        "local_access.no_window.after-await",
        &closed,
    );
    assert_eq!(after_await.len(), 1, "{after_await:?}");
    // The module-scope read that rc.21 installs no strict-read context for.
    let module_scope = rows_the_silence_ends(
        &typefacts,
        "props-callers",
        "local_access.no_window.module-scope",
        &[],
    );
    assert_eq!(module_scope.len(), 1, "{module_scope:?}");
}
