use solid_facts_backend::Snapshot;

use super::go_compatible_report;

/// What the report was before it was indexed: serde's pretty string with the
/// Go escapes applied as replacements.
fn reference(snapshot: &Snapshot) -> Vec<u8> {
    serde_json::to_string_pretty(snapshot)
        .unwrap()
        .replace('&', "\\u0026")
        .replace('<', "\\u003c")
        .replace('>', "\\u003e")
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029")
        .into_bytes()
}

fn finding(id: &str, path: &str, related: &[&str], subject: &str) -> serde_json::Value {
    let location = |path: &str| serde_json::json!({"path": path, "startByte": 1, "endByte": 2, "line": 1, "column": 2});
    serde_json::json!({
        "id": id, "rule": "r", "kind": "violation", "severity": "error",
        "message": format!("a < b && c > d \u{2028} \u{2029} caf\u{e9} \u{1f600} \"{id}\"\nline"),
        "subjectKind": subject,
        "primaryLocation": location(path),
        "relatedLocations": related.iter().map(|path| location(path)).collect::<Vec<_>>(),
        "evidence": [{"message": "x & y", "location": location(path)}],
    })
}

fn snapshot(findings: Vec<serde_json::Value>, premises: &[&str]) -> Snapshot {
    serde_json::from_value(serde_json::json!({
        "premises": premises,
        "status": "violation",
        "findings": findings,
        "metrics": {
            "filesAnalyzed": 3, "functionsAnalyzed": 4, "proofObligations": 5,
            "cachedSummaries": 0, "unresolvedObligations": 1
        },
    }))
    .unwrap()
}

fn with_findings(snapshot: &Snapshot, keep: &[usize]) -> Snapshot {
    let mut narrowed = snapshot.clone();
    narrowed.findings = keep.iter().map(|&i| snapshot.findings[i].clone()).collect();
    narrowed
}

#[test]
fn the_indexed_report_is_the_report_it_replaced() {
    for premises in [&[][..], &["one & two", "<three>"][..]] {
        for count in [0, 1, 3] {
            let findings = (0..count)
                .map(|i| {
                    finding(
                        &format!("SC{i}"),
                        &format!("/p/f{i}.ts"),
                        &["/p/other.ts"],
                        "",
                    )
                })
                .collect();
            let snapshot = snapshot(findings, premises);
            assert_eq!(
                String::from_utf8(go_compatible_report(&snapshot).unwrap().output).unwrap(),
                String::from_utf8(reference(&snapshot)).unwrap()
            );
        }
    }
}

#[test]
fn one_files_answer_is_the_report_of_the_findings_that_concern_it() {
    let snapshot = snapshot(
        vec![
            finding("SC0", "/p/a.ts", &[], ""),
            finding("SC1", "/p/b.ts", &[], ""),
            finding("SC2", "/p/b.ts", &["/p/a.ts"], "package"),
            finding("SC3", "/p/c.ts", &[], "project"),
            finding("SC4", "/p/./sub/../a.ts", &[], ""),
            finding("SC5", "relative/a.ts", &[], ""),
        ],
        &["premise"],
    );
    let report = go_compatible_report(&snapshot).unwrap();
    let only = |file: &str| report.index.only_file(&report.output, file).unwrap();
    // a.ts: itself, the related one, the project one, the same file spelled
    // with dots, and the relative path nothing can settle.
    assert_eq!(
        only("/p/a.ts"),
        reference(&with_findings(&snapshot, &[0, 2, 3, 4, 5]))
    );
    assert_eq!(
        only("/p/b.ts"),
        reference(&with_findings(&snapshot, &[1, 2, 3, 5]))
    );
    assert_eq!(
        only("/p/none.ts"),
        reference(&with_findings(&snapshot, &[3, 5]))
    );
    // A file the report does not know and no path to doubt: only project ones.
    let tidy = super::go_compatible_report(&with_findings(&snapshot, &[0, 1, 3])).unwrap();
    assert_eq!(
        tidy.index.only_file(&tidy.output, "/p/none.ts").unwrap(),
        reference(&with_findings(&snapshot, &[3]))
    );
    let none = super::go_compatible_report(&with_findings(&snapshot, &[0, 1])).unwrap();
    assert_eq!(
        none.index.only_file(&none.output, "/p/none.ts").unwrap(),
        reference(&with_findings(&snapshot, &[]))
    );
    // A file the caller cannot spell absolutely keeps everything.
    assert_eq!(only("a.ts"), report.output);
    // Narrowing a report that has findings elsewhere makes it shorter.
    assert!(
        report
            .index
            .only_file(&report.output, "/p/a.ts")
            .unwrap()
            .len()
            < report.output.len()
    );
}

#[test]
fn a_report_without_findings_is_its_own_answer() {
    let snapshot = snapshot(Vec::new(), &[]);
    let report = go_compatible_report(&snapshot).unwrap();
    assert_eq!(
        report.index.only_file(&report.output, "/p/a.ts").unwrap(),
        report.output
    );
}
