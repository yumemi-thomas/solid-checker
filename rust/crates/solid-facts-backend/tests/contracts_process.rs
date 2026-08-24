#[path = "support/process.rs"]
mod support;

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

use support::{decode_findings, temporary_directory};

fn expanded_contract(path: &Path) -> serde_json::Value {
    serde_json::to_value(solid_facts_backend::read_package_contract(path).unwrap()).unwrap()
}

fn without_claim_evidence(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(object) => serde_json::Value::Object(
            object
                .iter()
                .filter(|(key, _)| key.as_str() != "evidence")
                .map(|(key, value)| (key.clone(), without_claim_evidence(value)))
                .collect(),
        ),
        serde_json::Value::Array(values) => {
            serde_json::Value::Array(values.iter().map(without_claim_evidence).collect())
        }
        value => value.clone(),
    }
}

#[test]
fn cli_consumes_discovered_package_contracts() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    for (fixture, rule, message, expected_count) in [
        ("package-consumer", "strict-read-untracked", "readCount", 2),
        (
            "package-return-consumer",
            "strict-read-untracked",
            "created count",
            1,
        ),
        (
            "package-callback-consumer",
            "strict-read-untracked",
            "runMixed",
            2,
        ),
        (
            "package-store-consumer",
            "strict-read-untracked",
            "state.value",
            1,
        ),
        (
            "package-store-destructure",
            "no-destructure",
            "destructuring",
            1,
        ),
        (
            "package-unknown-export",
            "package-contract-incomplete",
            "unknownPrimitive",
            1,
        ),
        (
            "bundled-solid-consumer",
            "strict-read-untracked",
            "doubled",
            1,
        ),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
            .env("SOLID_TYPEFACTS_BIN", &typefacts)
            .args(["--format", "json", "--project"])
            .arg(root.join(format!("fixtures/reactive-ir/{fixture}/tsconfig.json")))
            .output()
            .expect("run Rust diagnostic CLI");
        assert!(
            output.status.success(),
            "fixture {fixture}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let findings = decode_findings(&output.stdout);
        assert_eq!(
            findings.len(),
            expected_count,
            "fixture {fixture}: {findings:#?}"
        );
        assert!(
            findings.iter().any(|finding| {
                finding["rule"] == rule
                    && finding["message"]
                        .as_str()
                        .is_some_and(|message_text| message_text.contains(message))
            }),
            "fixture {fixture}: expected {rule} mentioning {message}, got {findings:#?}"
        );
    }
}

#[test]
fn cli_classifies_parameter_member_reads_at_each_call_site() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let output = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
        .env("SOLID_TYPEFACTS_BIN", &typefacts)
        .args(["--format", "json", "--project"])
        .arg(root.join("fixtures/reactive-ir/package-parameter-member-consumer/tsconfig.json"))
        .output()
        .expect("run Rust diagnostic CLI");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let findings = decode_findings(&output.stdout);
    assert_eq!(findings.len(), 3, "{findings:#?}");
    assert!(findings.iter().any(|finding| {
        finding["rule"] == "strict-read-untracked"
            && finding["message"]
                .as_str()
                .is_some_and(|message| message.contains("state"))
    }));
    assert!(findings.iter().any(|finding| {
        finding["id"] == "SC9012"
            && finding["analysisContext"]
                .as_str()
                .is_some_and(|message| message.contains("parameter-member"))
    }));
    // Spreading the store into the argument literal hands the callee snapshot
    // data, so the parameter-member claim proves nothing there and adds no
    // second obligation. The read that exists is the spread, reported once.
    assert_eq!(
        findings
            .iter()
            .filter(|finding| {
                finding["rule"] == "strict-read-untracked"
                    && finding["message"]
                        .as_str()
                        .is_some_and(|message| message.contains("state spread"))
            })
            .count(),
        1,
        "{findings:#?}"
    );
    assert_eq!(
        findings
            .iter()
            .filter(|finding| finding["id"] == "SC9012")
            .count(),
        1,
        "{findings:#?}"
    );
    assert!(
        findings
            .iter()
            .all(|finding| { finding["location"]["line"] != 10 })
    );
}

#[test]
fn cli_demands_unknown_callbacks_only_for_callable_arguments() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let output = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
        .env("SOLID_TYPEFACTS_BIN", &typefacts)
        .args(["--format", "json", "--project"])
        .arg(root.join("fixtures/reactive-ir/package-unknown-callback-consumer/tsconfig.json"))
        .output()
        .expect("run Rust diagnostic CLI");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let findings = decode_findings(&output.stdout);
    assert_eq!(findings.len(), 1, "{findings:#?}");
    assert_eq!(findings[0]["rule"], "package-contract-incomplete");
    assert_eq!(findings[0]["kind"], "uncertifiable");
    assert!(
        findings[0]["message"]
            .as_str()
            .is_some_and(|message| message.contains("callback")),
        "{findings:#?}"
    );
    assert_eq!(findings[0]["primaryLocation"]["line"], 13);
}

/// `{ "status": "unknown" }` is five independent claims, and the four
/// non-callback ones open the obligation where the claim enters the project
/// rather than where a call demands it. The finding must name the exact domain
/// left unknown: a summary that states four domains and withholds one is not
/// the same evidence as a summary that states nothing, and reporting it as
/// though it were would discard four reviewed claims.
///
/// The findings snapshot deliberately excludes message text, so the domain
/// string is pinned here.
#[test]
fn cli_reports_the_exact_unknown_claim_domain() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let output = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
        .env("SOLID_TYPEFACTS_BIN", &typefacts)
        .args(["--format", "json", "--project"])
        .arg(root.join("fixtures/reactive-ir/package-unknown-returns-consumer/tsconfig.json"))
        .output()
        .expect("run Rust diagnostic CLI");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let findings = decode_findings(&output.stdout);
    assert_eq!(findings.len(), 1, "{findings:#?}");
    assert_eq!(findings[0]["rule"], "package-contract-incomplete");
    assert_eq!(findings[0]["kind"], "uncertifiable");
    assert_eq!(
        findings[0]["analysisContext"],
        "unknown-contract-claims:returns"
    );
    assert!(
        findings[0]["message"]
            .as_str()
            .is_some_and(|message| message.contains("leaves returns unknown")
                && message.contains("openSource")),
        "{findings:#?}"
    );
}

/// Overlapping export-map branches are resolved by `precedence` -- the map's
/// own first-match-wins order -- and only when that removes the ambiguity
/// rather than guessing through it. The unit tests in
/// rust/crates/solid-reactive-ir/src/contracts.rs pin the selection function;
/// this pins that the selection reaches a consumer's proof at all, in both
/// directions, from one fixture whose two exports differ only in `precedence`.
#[test]
fn cli_resolves_overlapping_contract_variants_by_precedence() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let output = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
        .env("SOLID_TYPEFACTS_BIN", &typefacts)
        .args(["--format", "json", "--project"])
        .arg(root.join("fixtures/reactive-ir/package-variant-precedence-consumer/tsconfig.json"))
        // The same selection scripts/coverage.mjs reads from the fixture's
        // .solid-checker/runtime.json. Both branches of both exports match it;
        // with nothing selected there is no environment and every variant
        // fails closed, which would make the resolved half untestable.
        .args(["--runtime-target", "browser"])
        .args(["--runtime-build", "development"])
        .output()
        .expect("run Rust diagnostic CLI");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let findings = decode_findings(&output.stdout);
    assert_eq!(findings.len(), 2, "{findings:#?}");
    // The unique lowest precedence resolved: only the `development` branch
    // returns an accessor, and only that branch makes this read reactive.
    assert!(
        findings.iter().any(|finding| {
            finding["rule"] == "strict-read-untracked"
                && finding["message"]
                    .as_str()
                    .is_some_and(|message| message.contains("development counter"))
        }),
        "{findings:#?}"
    );
    // The tie did not: nothing says which branch the resolver reaches first,
    // so the import binding is uncertifiable and its identical accessor read
    // is never claimed to be reactive.
    assert!(
        findings.iter().any(|finding| {
            finding["rule"] == "package-contract-incomplete"
                && finding["kind"] == "uncertifiable"
                && finding["message"].as_str().is_some_and(|message| {
                    message.contains("conditional runtime targets")
                        && message.contains("openAmbiguous")
                })
        }),
        "{findings:#?}"
    );
    assert!(
        !findings.iter().any(|finding| {
            finding["message"]
                .as_str()
                .is_some_and(|message| message.contains("browser counter"))
        }),
        "{findings:#?}"
    );
}

/// A callback row's `arguments` descriptors are materialized in exactly one
/// shape — an inline function literal carrying an `accessor` descriptor.
/// Every other schema-valid shape has no binding the consumer can create, and
/// dropping the claim there analyzed the callback body as if the contract had
/// said nothing about its arguments. Those call sites fail closed instead.
#[test]
fn cli_demands_contract_callback_arguments_it_cannot_bind() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let output = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
        .env("SOLID_TYPEFACTS_BIN", &typefacts)
        .args(["--format", "json", "--project"])
        .arg(root.join("fixtures/reactive-ir/package-callback-arguments-consumer/tsconfig.json"))
        .output()
        .expect("run Rust diagnostic CLI");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let findings = decode_findings(&output.stdout);
    // The by-name callback, the `store-path` descriptor on a declared
    // parameter, and the two literals that reach the described argument without
    // declaring it -- through a rest parameter and through `arguments`. Only
    // the two inline-literal calls whose *restless arrow* provably cannot name
    // the argument keep their precise, silent behavior.
    assert_eq!(findings.len(), 4, "{findings:#?}");
    for finding in &findings {
        assert_eq!(finding["id"], "SC9005", "{findings:#?}");
        assert_eq!(finding["kind"], "uncertifiable", "{findings:#?}");
        assert!(
            finding["message"]
                .as_str()
                .is_some_and(|message| message.contains("nothing to bind to")),
            "{findings:#?}"
        );
    }
    assert_eq!(findings[0]["primaryLocation"]["line"], 25);
    assert_eq!(findings[1]["primaryLocation"]["line"], 32);
    assert_eq!(findings[2]["primaryLocation"]["line"], 40);
    assert_eq!(findings[3]["primaryLocation"]["line"], 47);
}

#[test]
fn cli_consumes_structured_returns_in_schema_one_contracts() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let output = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
        .env("SOLID_TYPEFACTS_BIN", &typefacts)
        .args([
            "--format",
            "json",
            "--runtime-target",
            "node",
            "--rendering",
            "string-ssr",
            "--runtime-conditions",
            "node,import",
            "--project",
        ])
        .arg(root.join("fixtures/reactive-ir/package-structured-return/tsconfig.json"))
        .output()
        .expect("run Rust diagnostic CLI");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let findings = decode_findings(&output.stdout);
    let messages = findings
        .iter()
        .filter(|finding| finding["rule"] == "strict-read-untracked")
        .filter_map(|finding| finding["message"].as_str())
        .collect::<Vec<_>>();
    for expected in ["state.value", "value", "active", "pending", "persisted"] {
        assert!(
            messages.iter().any(|message| message.contains(expected)),
            "missing {expected:?} in {findings:#?}"
        );
    }
    for expected_context in ["ObjectMemberConsumer", "DirectObjectMemberConsumer"] {
        assert!(
            findings.iter().any(|finding| {
                finding["rule"] == "strict-read-untracked"
                    && finding["analysisContext"] == expected_context
            }),
            "missing direct/member consumer {expected_context:?} in {findings:#?}"
        );
    }
}

#[test]
fn cli_validates_a_contract_without_opening_a_project() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let contract = root.join(
        "fixtures/reactive-ir/package-consumer/node_modules/reactive-package/solid-reactivity.json",
    );
    let output = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
        .env_remove("SOLID_TYPEFACTS_BIN")
        .args(["--validate-contract"])
        .arg(contract)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn bundled_contract_resolves_the_exact_web_subpath() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let output = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
        .env("SOLID_TYPEFACTS_BIN", &typefacts)
        .args([
            "--format",
            "json",
            "--runtime-target",
            "node",
            "--rendering",
            "string-ssr",
            "--runtime-conditions",
            "node,import",
            "--project",
        ])
        .arg(root.join("fixtures/reactive-ir/bundled-web-subpath-consumer/tsconfig.json"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        decode_findings(&output.stdout),
        Vec::<serde_json::Value>::new()
    );
}

#[test]
fn bundled_scheduled_contract_marks_debounce_callback_deferred() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let output = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
        .env("SOLID_TYPEFACTS_BIN", &typefacts)
        .args([
            "--format",
            "json",
            "--dialect",
            "solid-v1",
            "--runtime-target",
            "browser",
            "--runtime-conditions",
            "browser,import",
            "--project",
        ])
        .arg(root.join("fixtures/reactive-ir/bundled-scheduled-consumer/tsconfig.json"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        decode_findings(&output.stdout),
        Vec::<serde_json::Value>::new()
    );
}

#[test]
fn bundled_contract_refuses_a_different_installed_version() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let directory = temporary_directory("bundled-version-mismatch");
    fs::write(
        directory.join("tsconfig.json"),
        r#"{
  "compilerOptions": {
    "strict": true,
    "target": "ES2022",
    "module": "ESNext",
    "moduleResolution": "bundler"
  },
  "include": ["App.ts"]
}
"#,
    )
    .unwrap();
    fs::write(
        directory.join("App.ts"),
        "import { createSignal } from \"solid-js\";\ncreateSignal(0);\n",
    )
    .unwrap();
    let package = directory.join("node_modules/solid-js");
    fs::create_dir_all(&package).unwrap();
    fs::write(
        package.join("package.json"),
        r#"{ "name": "solid-js", "version": "2.0.0-beta.25", "types": "index.d.ts" }"#,
    )
    .unwrap();
    fs::write(
        package.join("index.d.ts"),
        "export declare function createSignal<T>(value: T): [() => T, (value: T) => void];\n",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
        .env("SOLID_TYPEFACTS_BIN", &typefacts)
        .args(["--format", "json", "--check-contracts", "--project"])
        .arg(directory.join("tsconfig.json"))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["missing"], 1);
    assert_eq!(report["stale"], 1);
    // The dialect audited another solid-js release, so the installed one is
    // unaudited rather than uncontracted. The remedy names the audited version
    // instead of a generation command the consumer must not run for a bundled
    // package.
    assert_eq!(report["packages"][0]["status"], "stale");
    let detail = report["packages"][0]["detail"].as_str().unwrap();
    assert!(detail.contains("audited solid-js"), "{detail}");
    assert!(detail.contains("2.0.0-beta.25 is installed"), "{detail}");
    let remedy = report["packages"][0]["remedy"].as_str().unwrap();
    assert!(remedy.contains("upgrade solid-checker"), "{remedy}");
    assert!(!remedy.contains("contract generate"), "{remedy}");

    // Analysis reports the same fact. Before, an unaudited solid-js version
    // reported as "has no reactivity contract", which sent users looking for a
    // contract to write for solid-js itself.
    let analysis = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
        .env("SOLID_TYPEFACTS_BIN", &typefacts)
        .args(["--format", "json", "--certify", "--project"])
        .arg(directory.join("tsconfig.json"))
        .output()
        .unwrap();
    assert_eq!(analysis.status.code(), Some(1));
    let snapshot: serde_json::Value = serde_json::from_slice(&analysis.stdout).unwrap();
    assert_eq!(snapshot["status"], "uncertifiable");
    let message = snapshot["findings"][0]["message"].as_str().unwrap();
    assert!(
        message.contains("is audited by this checker at version"),
        "{message}"
    );
    assert!(message.contains("2.0.0-beta.25 is installed"), "{message}");
    let hint = snapshot["findings"][0]["hint"].as_str().unwrap();
    assert!(hint.contains("upgrade solid-checker"), "{hint}");
    assert!(!hint.contains("contract generate"), "{hint}");
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn cli_reports_missing_contracts_and_loads_project_owned_overrides() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let fixture = root.join("fixtures/reactive-ir/package-consumer");
    let directory = temporary_directory("local-contract");
    fs::copy(fixture.join("App.tsx"), directory.join("App.tsx")).unwrap();
    fs::copy(fixture.join("jsx.d.ts"), directory.join("jsx.d.ts")).unwrap();
    fs::copy(
        fixture.join("tsconfig.json"),
        directory.join("tsconfig.json"),
    )
    .unwrap();
    let package = directory.join("node_modules/reactive-package");
    fs::create_dir_all(&package).unwrap();
    fs::copy(
        fixture.join("node_modules/reactive-package/index.d.ts"),
        package.join("index.d.ts"),
    )
    .unwrap();
    fs::write(
        package.join("package.json"),
        r#"{
  "name": "reactive-package",
  "version": "1.0.0",
  "types": "index.d.ts",
  "peerDependencies": {
    "solid-js": "^2.0.0"
  }
}
"#,
    )
    .unwrap();

    let missing = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
        .env("SOLID_TYPEFACTS_BIN", &typefacts)
        .args(["--format", "json", "--check-contracts", "--project"])
        .arg(directory.join("tsconfig.json"))
        .output()
        .unwrap();
    assert_eq!(missing.status.code(), Some(1));
    let report: serde_json::Value = serde_json::from_slice(&missing.stdout).unwrap();
    assert_eq!(report["missing"], 1);
    assert_eq!(report["packages"][0]["name"], "reactive-package");
    assert_eq!(report["packages"][0]["status"], "missing");

    let uncertifiable = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
        .env("SOLID_TYPEFACTS_BIN", &typefacts)
        .args(["--format", "json", "--certify", "--project"])
        .arg(directory.join("tsconfig.json"))
        .output()
        .unwrap();
    assert_eq!(uncertifiable.status.code(), Some(1));
    let snapshot: serde_json::Value = serde_json::from_slice(&uncertifiable.stdout).unwrap();
    assert_eq!(snapshot["status"], "uncertifiable");
    assert_eq!(snapshot["findings"][0]["id"], "SC9005");
    assert_eq!(
        snapshot["findings"][0]["rule"],
        "package-contract-incomplete"
    );
    assert!(
        snapshot["findings"][0]["primaryLocation"]["path"]
            .as_str()
            .is_some_and(|path| path.ends_with("App.tsx"))
    );

    let local = directory.join(".solid-checker/contracts/reactive-package");
    fs::create_dir_all(&local).unwrap();
    fs::write(
        local.join("solid-reactivity.json"),
        r#"{
  "schemaVersion": 1,
  "package": {
    "name": "reactive-package",
    "version": "1.0.0"
  },
  "compilerFactsProtocol": 1,
  "summaries": {
    "function": {
      "kind": "function",
      "reactiveReads": [
        { "kind": "accessor", "label": "unreviewed generated value" }
      ]
    }
  },
  "entrypoints": {
    ".": {
      "exports": {
        "function": ["readCount", "readItems"]
      }
    }
  },
  "evidence": {
    "kind": "inferred",
    "generator": "solid-checker"
  }
}
"#,
    )
    .unwrap();
    let unverified = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
        .env("SOLID_TYPEFACTS_BIN", &typefacts)
        .args(["--format", "json", "--check-contracts", "--project"])
        .arg(directory.join("tsconfig.json"))
        .output()
        .unwrap();
    assert_eq!(unverified.status.code(), Some(1));
    let report: serde_json::Value = serde_json::from_slice(&unverified.stdout).unwrap();
    assert_eq!(report["missing"], 1);
    assert_eq!(report["packages"][0]["status"], "unverified");

    let analysis = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
        .env("SOLID_TYPEFACTS_BIN", &typefacts)
        .args(["--format", "json", "--project"])
        .arg(directory.join("tsconfig.json"))
        .output()
        .unwrap();
    assert!(analysis.status.success());
    let findings = decode_findings(&analysis.stdout);
    assert_eq!(findings.len(), 1, "{findings:#?}");
    assert_eq!(findings[0]["id"], "SC9005");
    assert!(
        findings[0]["message"]
            .as_str()
            .is_some_and(|message| message.contains("unverified"))
    );

    fs::write(
        local.join("solid-reactivity.json"),
        r#"{
  "schemaVersion": 1,
  "package": {
    "name": "reactive-package",
    "version": "1.0.0"
  },
  "compilerFactsProtocol": 1,
  "artifacts": {},
  "summaries": {
    "function-1": {
      "kind": "function",
      "reactiveReads": [
        {
          "kind": "accessor",
          "label": "project-owned reactive value"
        }
      ]
    }
  },
  "entrypoints": {
    ".": {
      "exports": {
        "function-1": ["readCount", "readItems"]
      }
    }
  },
  "evidence": {
    "kind": "reviewed",
    "generator": "application developer"
  }
}
"#,
    )
    .unwrap();

    let covered = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
        .env("SOLID_TYPEFACTS_BIN", &typefacts)
        .args(["--format", "json", "--check-contracts", "--project"])
        .arg(directory.join("tsconfig.json"))
        .output()
        .unwrap();
    assert!(
        covered.status.success(),
        "{}",
        String::from_utf8_lossy(&covered.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&covered.stdout).unwrap();
    assert_eq!(report["missing"], 0);
    assert_eq!(report["packages"][0]["status"], "local");

    let analysis = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
        .env("SOLID_TYPEFACTS_BIN", &typefacts)
        .args(["--format", "json", "--project"])
        .arg(directory.join("tsconfig.json"))
        .output()
        .unwrap();
    assert!(
        analysis.status.success(),
        "{}",
        String::from_utf8_lossy(&analysis.stderr)
    );
    let findings = decode_findings(&analysis.stdout);
    assert_eq!(findings.len(), 2, "{findings:#?}");
    assert!(
        findings
            .iter()
            .any(|finding| finding["rule"] == "strict-read-untracked"),
        "the project-owned contract must still prove the reactive read: {findings:#?}"
    );
    assert!(
        findings
            .iter()
            .any(|finding| finding["rule"] == "prefer-for"),
        "the same contract must prove the default list preference: {findings:#?}"
    );

    fs::write(
        local.join("solid-reactivity.json"),
        r#"{
  "schemaVersion": 1,
  "package": {
    "name": "reactive-package",
    "version": "1.0.0"
  },
  "compilerFactsProtocol": 1,
  "summaries": {
    "function-1": {
      "kind": "function",
      "reactiveReads": [
        { "kind": "accessor", "label": "project-owned reactive value" }
      ],
      "variants": [
        {
          "conditions": ["browser"],
          "summary": {
            "kind": "function",
            "reactiveReads": [
              { "kind": "accessor", "label": "project-owned reactive value" }
            ]
          }
        },
        {
          "conditions": ["node"],
          "summary": { "kind": "function" }
        }
      ]
    }
  },
  "entrypoints": {
    ".": {
      "exports": {
        "function-1": ["readCount", "readItems"]
      }
    }
  },
  "evidence": {
    "kind": "reviewed",
    "generator": "application developer"
  }
}
"#,
    )
    .unwrap();
    let conditional = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
        .env("SOLID_TYPEFACTS_BIN", &typefacts)
        .args(["--format", "json", "--certify", "--project"])
        .arg(directory.join("tsconfig.json"))
        .output()
        .unwrap();
    assert_eq!(conditional.status.code(), Some(1));
    let conditional_findings = decode_findings(&conditional.stdout);
    assert_eq!(conditional_findings.len(), 2);
    assert!(
        conditional_findings
            .iter()
            .all(|finding| finding["id"] == "SC9005")
    );
    assert!(conditional_findings.iter().all(|finding| {
        finding["message"]
            .as_str()
            .is_some_and(|message| message.contains("conditional runtime targets"))
    }));

    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn cli_emits_and_revalidates_package_contracts() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let directory = temporary_directory("emit-contract");
    let output = directory.join("solid-reactivity.json");
    let declaration = directory.join("index.d.ts");
    fs::write(
        &declaration,
        "export declare function createCount(): () => number;\n",
    )
    .unwrap();
    let producer = root.join("fixtures/reactive-ir/package-return-producer/tsconfig.json");
    let result = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
        .env("SOLID_TYPEFACTS_BIN", &typefacts)
        .args(["--project"])
        .arg(producer)
        .args(["--emit-contract"])
        .arg(&output)
        .args([
            "--package-name",
            "reactive-package",
            "--package-version",
            "1.0.0",
            "--declaration-artifact",
        ])
        .arg(&declaration)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let contract = expanded_contract(&output);
    for name in [
        "createCount",
        "createAliasedCount",
        "createArrowCount",
        "createMemoCount",
        "createWrappedCount",
        "createTransitivelyWrapped",
    ] {
        assert_eq!(
            contract["entrypoints"]["."]["exports"][name]["returns"]["kind"],
            "accessor"
        );
    }
    assert_eq!(
        contract["entrypoints"]["."]["exports"]["createState"]["returns"]["kind"],
        "store-path"
    );
    assert_eq!(
        contract["entrypoints"]["."]["exports"]["packageVersion"]["kind"],
        "value"
    );
    assert_eq!(
        without_claim_evidence(
            &contract["entrypoints"]["."]["exports"]["createWrappedCount"]["callbacks"]
        ),
        serde_json::json!([{ "parameter": 0, "execution": "tracked" }])
    );
    assert_eq!(
        without_claim_evidence(
            &contract["entrypoints"]["."]["exports"]["createTransitivelyWrapped"]["callbacks"]
        ),
        serde_json::json!([{ "parameter": 0, "execution": "tracked" }])
    );
    assert_eq!(
        without_claim_evidence(&contract["entrypoints"]["."]["exports"]["listen"]["callbacks"]),
        serde_json::json!([{ "parameter": 1, "execution": "deferred" }])
    );
    assert_eq!(
        without_claim_evidence(
            &contract["entrypoints"]["."]["exports"]["configureDeferredMethod"]["callbacks"]
        ),
        serde_json::json!([{ "parameter": 1, "execution": "deferred" }])
    );
    assert_eq!(
        without_claim_evidence(
            &contract["entrypoints"]["."]["exports"]["createDeferredProxy"]["callbacks"]
        ),
        serde_json::json!([{ "parameter": 0, "execution": "deferred" }])
    );
    assert_eq!(contract["artifacts"]["declaration"]["path"], "index.d.ts");

    let validate = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
        .env_remove("SOLID_TYPEFACTS_BIN")
        .args(["--validate-contract"])
        .arg(&output)
        .output()
        .unwrap();
    assert!(
        validate.status.success(),
        "{}",
        String::from_utf8_lossy(&validate.stderr)
    );
    fs::remove_dir_all(directory).unwrap();
}

/// `--emit-module-inventory` attests the program this run analyzed.
///
/// The point of the flag is that the answer comes from the process that resolved
/// the modules, so the assertions below are the three things only that process
/// can say: which files it opened, which specifier resolved to what, and which
/// specifier resolved to nothing at all. A generator-side walk can produce a
/// plausible version of the first and neither of the other two.
#[test]
fn cli_emits_the_analyzing_program_s_own_module_inventory() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let directory = temporary_directory("module-inventory");
    let package = directory.join("package");
    fs::create_dir_all(&package).unwrap();
    fs::write(
        package.join("package.json"),
        "{\"name\":\"inventory-package\",\"version\":\"1.0.0\",\"type\":\"module\"}\n",
    )
    .unwrap();
    // Three shapes in one entry: a specifier the resolver substitutes an
    // extension for, one it resolves to nothing, and a file the program reaches
    // only through the first.
    fs::write(
        package.join("index.js"),
        "import \"./styles.css\";\nexport { thing } from \"./impl.js\";\n",
    )
    .unwrap();
    fs::write(package.join("impl.ts"), "export const thing = 1;\n").unwrap();
    fs::write(package.join("styles.css"), ".thing { color: red; }\n").unwrap();
    let project = directory.join("tsconfig.json");
    fs::write(
        &project,
        format!(
            "{{\"compilerOptions\":{{\"allowJs\":true,\"checkJs\":true,\"module\":\"ESNext\",\
             \"moduleResolution\":\"Bundler\",\"skipLibCheck\":true,\"target\":\"ES2022\"}},\
             \"files\":[{:?}]}}\n",
            package.join("index.js").to_string_lossy()
        ),
    )
    .unwrap();
    let contract = directory.join("solid-reactivity.json");
    let inventory_path = directory.join("inventory.json");
    let result = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
        .env("SOLID_TYPEFACTS_BIN", &typefacts)
        .args(["--project"])
        .arg(&project)
        .args(["--emit-contract"])
        .arg(&contract)
        .args(["--emit-module-inventory"])
        .arg(&inventory_path)
        .args(["--contract-entry-file"])
        .arg(package.join("index.js"))
        .args(["--contract-package-root"])
        .arg(&package)
        .args([
            "--package-name",
            "inventory-package",
            "--package-version",
            "1.0.0",
        ])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let inventory: serde_json::Value =
        serde_json::from_slice(&fs::read(&inventory_path).unwrap()).unwrap();
    assert_eq!(inventory["schemaVersion"], 1);
    assert_eq!(inventory["complete"], true);
    assert_eq!(inventory["unknownImportPaths"].as_array().unwrap().len(), 0);

    // The spelling the caller passed, not the canonical one. Both name the same
    // directory and the consumer normalizes each side itself, so the assertions
    // below canonicalize rather than assuming which spelling the program used --
    // a `/var/folders` temporary directory is reachable by both.
    assert_eq!(
        inventory["packageRoot"].as_str().unwrap(),
        package.to_string_lossy()
    );
    let real_package = package.canonicalize().unwrap();
    let modules = inventory["modules"].as_array().unwrap();
    let local = modules
        .iter()
        .filter_map(|module| module["path"].as_str())
        .filter_map(|path| fs::canonicalize(path).ok())
        .filter(|path| path.starts_with(&real_package))
        .collect::<Vec<_>>();
    // `impl.ts` is there because the program opened it, not because the entry
    // named it: the entry names `./impl.js`, which does not exist.
    assert_eq!(
        local,
        vec![real_package.join("impl.ts"), real_package.join("index.js")]
    );
    // The inventory is not filtered to the package on the wire. It is a record
    // of what the analysis read, and the generator scopes it where the record is
    // built -- see `attestedClosure`.
    assert!(
        modules.len() > local.len(),
        "the inventory should name the library files the analysis opened too: {modules:?}"
    );

    let imports = inventory["imports"].as_array().unwrap();
    let of = |text: &str| {
        imports
            .iter()
            .find(|fact| fact["text"] == text)
            .unwrap_or_else(|| panic!("no import fact for {text}: {imports:?}"))
            .clone()
    };
    let asset = of("./styles.css");
    assert_eq!(asset["resolution"], "unresolved");
    assert_eq!(asset["resolvedPath"], serde_json::Value::Null);
    let implementation = of("./impl.js");
    assert_eq!(implementation["resolution"], "relative");
    assert_eq!(
        fs::canonicalize(implementation["resolvedPath"].as_str().unwrap()).unwrap(),
        real_package.join("impl.ts")
    );
    assert_eq!(implementation["extension"], ".ts");
    // Every fact joins to a consumer's own syntax facts by exact span, never by
    // matching specifier text.
    assert_eq!(
        fs::canonicalize(implementation["path"].as_str().unwrap()).unwrap(),
        real_package.join("index.js")
    );
    assert!(
        implementation["endByte"].as_u64().unwrap() > implementation["startByte"].as_u64().unwrap()
    );

    // An attestation of a run that emits no contract would attest nothing.
    let orphan = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
        .env("SOLID_TYPEFACTS_BIN", &typefacts)
        .args(["--project"])
        .arg(&project)
        .args(["--emit-module-inventory"])
        .arg(directory.join("orphan.json"))
        .output()
        .unwrap();
    assert!(!orphan.status.success());
    assert!(
        String::from_utf8_lossy(&orphan.stderr).contains("requires --emit-contract"),
        "{}",
        String::from_utf8_lossy(&orphan.stderr)
    );

    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn cli_emits_unknown_callback_claim_without_discarding_known_siblings() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let directory = temporary_directory("emit-unknown-callback");
    let output = directory.join("solid-reactivity.json");
    let result = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
        .env("SOLID_TYPEFACTS_BIN", &typefacts)
        .args(["--project"])
        .arg(root.join("fixtures/reactive-ir/package-unknown-callback-producer/tsconfig.json"))
        .args(["--emit-contract"])
        .arg(&output)
        .args([
            "--package-name",
            "callback-package",
            "--package-version",
            "1.0.0",
        ])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let contract = expanded_contract(&output);
    assert_eq!(
        contract["entrypoints"]["."]["exports"]["schedule"]["callbacks"],
        serde_json::json!({ "status": "unknown" })
    );
    assert_eq!(
        without_claim_evidence(
            &contract["entrypoints"]["."]["exports"]["invokeReflectively"]["callbacks"]
        ),
        serde_json::json!([{ "parameter": 0, "execution": "inline" }])
    );
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn package_generator_reviews_unknown_callback_claim_as_one_grouped_item() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let package = root.join("fixtures/package-contracts/unknown-callback-claim");
    let directory = temporary_directory("unknown-callback-claim-contract");
    let output = directory.join("solid-reactivity.json");
    let result = Command::new("node")
        .arg(root.join("packages/cli/bin/solid-checker.mjs"))
        .args(["contract", "generate", "--package-root"])
        .arg(&package)
        .arg("--output")
        .arg(&output)
        .env(
            "SOLID_CHECKER_NATIVE_BIN",
            env!("CARGO_BIN_EXE_solid-checker-rust"),
        )
        .env("SOLID_TYPEFACTS_BIN", &typefacts)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let contract = expanded_contract(&output);
    assert_eq!(
        contract["entrypoints"]["."]["exports"]["schedule"]["callbacks"],
        serde_json::json!({ "status": "unknown" })
    );
    assert_eq!(
        contract["entrypoints"]["."]["exports"]["plain"]["kind"],
        "function"
    );
    let review = fs::read_to_string(directory.join("solid-reactivity.review.md")).unwrap();
    assert!(review.contains("## unknown export claims"), "{review}");
    assert!(review.contains(".:schedule: callbacks"), "{review}");
    assert_eq!(
        review.matches(".:schedule: callbacks").count(),
        1,
        "{review}"
    );
    fs::remove_dir_all(directory).unwrap();
}

/// A local callee's summary is inherited by everything that forwards a
/// callback into it, so an empty summary is a *negative* claim about the
/// caller too. Where the callee retains the value instead of calling it,
/// the domain has to be the sentinel — and where the references only observe
/// the value, it must stay the honest omission.
/// See fixtures/package-contracts/retained-callback-parameter/README.md.
#[test]
fn package_generator_fails_closed_on_a_retained_callback_parameter() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let package = root.join("fixtures/package-contracts/retained-callback-parameter");
    let directory = temporary_directory("retained-callback-parameter-contract");
    let output = directory.join("solid-reactivity.json");
    let result = Command::new("node")
        .arg(root.join("packages/cli/bin/solid-checker.mjs"))
        .args(["contract", "generate", "--package-root"])
        .arg(&package)
        .arg("--output")
        .arg(&output)
        .env(
            "SOLID_CHECKER_NATIVE_BIN",
            env!("CARGO_BIN_EXE_solid-checker-rust"),
        )
        .env("SOLID_TYPEFACTS_BIN", &typefacts)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let contract = expanded_contract(&output);
    let exports = &contract["entrypoints"]["."]["exports"];
    for name in [
        "forwardsIntoRetainingHelper",
        "retainsInModuleBinding",
        "absorbsRest",
    ] {
        assert_eq!(
            exports[name]["callbacks"],
            serde_json::json!({ "status": "unknown" }),
            "{name}: {contract}"
        );
    }
    assert_eq!(
        without_claim_evidence(&exports["invokesCallback"]["callbacks"]),
        serde_json::json!([{ "parameter": 0, "execution": "inline" }]),
        "{contract}"
    );
    for name in ["observesCallback", "storesIntoCallerContainer"] {
        assert!(
            exports[name].get("callbacks").is_none(),
            "{name}: {contract}"
        );
    }
    fs::remove_dir_all(directory).unwrap();
}

/// `typeof C === "function"` for every class, but a class type has construct
/// signatures and no *call* signature, so Type Facts answers `nonCallable`
/// and the generator used to publish `kind: "value"` — a claim the runtime
/// kind probe contradicts. See fixtures/package-contracts/exported-class.
#[test]
fn package_generator_states_an_exported_class_as_a_function_kind() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let package = root.join("fixtures/package-contracts/exported-class");
    let directory = temporary_directory("exported-class-contract");
    let output = directory.join("solid-reactivity.json");
    let result = Command::new("node")
        .arg(root.join("packages/cli/bin/solid-checker.mjs"))
        .args(["contract", "generate", "--package-root"])
        .arg(&package)
        .arg("--output")
        .arg(&output)
        .env(
            "SOLID_CHECKER_NATIVE_BIN",
            env!("CARGO_BIN_EXE_solid-checker-rust"),
        )
        .env("SOLID_TYPEFACTS_BIN", &typefacts)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let contract = expanded_contract(&output);
    let exports = &contract["entrypoints"]["."]["exports"];
    // Declared here, reached through a barrel's `export { … }` of an imported
    // binding, and reached through `const Alias = SomeClass`.
    for name in [
        "DirectError",
        "BaseError",
        "ChildError",
        "Watcher",
        "AliasedWatcher",
    ] {
        assert_eq!(exports[name]["kind"], "function", "{name}: {contract}");
        assert_eq!(
            exports[name]["callbacks"],
            serde_json::json!({ "status": "unknown" }),
            "{name}: {contract}"
        );
    }
    assert_eq!(exports["plainFunction"]["kind"], "function", "{contract}");
    assert!(
        exports["plainFunction"].get("callbacks").is_none(),
        "{contract}"
    );
    assert_eq!(exports["settings"]["kind"], "value", "{contract}");
    fs::remove_dir_all(directory).unwrap();
}

/// The class shape a *published* package contains. A bundler lowers
/// `export class C {}` to `var C = class { … }`, so no class-name span covers
/// the exported binding and `nonCallable` is the truthful callability of a
/// class type — which made the generator publish `kind: "value"` for 45 of the
/// 53 failing kind claims in the corpus measurement. Also pins the honest
/// outcome when no closed type answers `kind` at all: the entrypoint is
/// refused, not published as the maximal certified negative that a bare
/// `value` summary is.
///
/// See fixtures/package-contracts/class-expression-kind/README.md.
#[test]
fn package_generator_states_a_class_expression_export_as_a_function_kind() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let package = root.join("fixtures/package-contracts/class-expression-kind");
    let directory = temporary_directory("class-expression-kind-contract");
    let output = directory.join("solid-reactivity.json");
    let result = Command::new("node")
        .arg(root.join("packages/cli/bin/solid-checker.mjs"))
        .args(["contract", "generate", "--package-root"])
        .arg(&package)
        .arg("--output")
        .arg(&output)
        .env(
            "SOLID_CHECKER_NATIVE_BIN",
            env!("CARGO_BIN_EXE_solid-checker-rust"),
        )
        .env("SOLID_TYPEFACTS_BIN", &typefacts)
        .output()
        .unwrap();
    // A refused entrypoint costs its own entrypoint and nothing else.
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let stdout = String::from_utf8_lossy(&result.stdout).to_string();
    assert!(
        stdout.contains("2 entrypoint(s) refused and omitted"),
        "{stdout}"
    );
    let contract = expanded_contract(&output);
    for refused in ["./unresolvable", "./destructured"] {
        assert!(contract["entrypoints"].get(refused).is_none(), "{contract}");
    }
    let plan = fs::read_to_string(directory.join("solid-reactivity.review.md")).unwrap();
    assert!(
        plan.contains("./unresolvable: solid-checker-rust: emit package contract:")
            && plan.contains("whose runtime kind no closed type answers (Unknown)"),
        "{plan}"
    );
    // The other refusal: `nonCallable` is a class type's answer too, and for a
    // destructuring pattern the class search never ran, so it proves nothing.
    assert!(
        plan.contains("./destructured: solid-checker-rust: emit package contract:")
            && plan.contains(
                "which destructures a member of another value, so no fact here rules out a class"
            ),
        "{plan}"
    );
    let exports = &contract["entrypoints"]["."]["exports"];
    // Bound in the entry file, reached through a `.js` barrel hop with no
    // `.d.ts` to answer for it, and reached through a bare-specifier
    // `export *` into an installed dependency's own artifact.
    for name in [
        "LocalCache",
        "InlineCache",
        "SiblingCache",
        "DependencyCache",
    ] {
        assert_eq!(exports[name]["kind"], "function", "{name}: {contract}");
        assert_eq!(
            exports[name]["callbacks"],
            serde_json::json!({ "status": "unknown" }),
            "{name}: {contract}"
        );
    }
    for name in ["siblingFunction", "dependencyFunction"] {
        assert_eq!(exports[name]["kind"], "function", "{name}: {contract}");
        assert!(
            exports[name].get("callbacks").is_none(),
            "{name}: {contract}"
        );
    }
    // The false-positive direction: real non-callable values whose binding is a
    // plain identifier, so the syntactic class search did run and did answer.
    for name in ["settings", "siblingTable"] {
        assert_eq!(exports[name]["kind"], "value", "{name}: {contract}");
    }
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn package_generator_describes_reactive_callback_arguments() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let package = root.join("fixtures/package-contracts/callback-reactive-arguments");
    let directory = temporary_directory("callback-reactive-arguments-contract");
    let output = directory.join("solid-reactivity.json");
    let result = Command::new("node")
        .arg(root.join("packages/cli/bin/solid-checker.mjs"))
        .args(["contract", "generate", "--package-root"])
        .arg(&package)
        .arg("--output")
        .arg(&output)
        .env(
            "SOLID_CHECKER_NATIVE_BIN",
            env!("CARGO_BIN_EXE_solid-checker-rust"),
        )
        .env("SOLID_TYPEFACTS_BIN", &typefacts)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let contract = expanded_contract(&output);
    let summary = &contract["entrypoints"]["."]["exports"]["mapValue"];
    assert!(summary.get("reactiveReads").is_none(), "{contract}");
    assert_eq!(
        without_claim_evidence(&summary["callbacks"]),
        serde_json::json!([{
            "parameter": 0,
            "execution": "inline",
            "arguments": [null, { "kind": "accessor", "label": "getItem" }]
        }]),
        "{contract}"
    );
    fs::remove_dir_all(directory).unwrap();
}

/// SC9-class obligations invalidate only the claim domains of the export that
/// contains them. The generator must preserve clean siblings and represent
/// the affected export as an explicit partial draft.
#[test]
fn cli_attributes_unresolved_obligations_to_export_claims() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let directory = temporary_directory("emit-unresolved-obligation");
    let output = directory.join("solid-reactivity.json");
    let result = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
        .env("SOLID_TYPEFACTS_BIN", &typefacts)
        .args(["--project"])
        .arg(root.join("fixtures/reactive-ir/package-unknown-export/tsconfig.json"))
        .args(["--emit-contract"])
        .arg(&output)
        .args([
            "--package-name",
            "unknown-export-package",
            "--package-version",
            "1.0.0",
        ])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(result.status.success(), "{stderr}");
    let contract = expanded_contract(&output);
    let affected = &contract["entrypoints"]["."]["exports"]["App"];
    for claim in [
        "reactiveReads",
        "returns",
        "callbacks",
        "ownerRequirements",
        "asyncBehavior",
    ] {
        assert_eq!(
            affected[claim],
            serde_json::json!({ "status": "unknown" }),
            "{claim}: {contract}"
        );
    }
    let plain = &contract["entrypoints"]["."]["exports"]["plain"];
    assert_eq!(plain["kind"], "function", "{contract}");
    assert!(plain.get("reactiveReads").is_none(), "{contract}");
    assert!(plain.get("callbacks").is_none(), "{contract}");
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn cli_does_not_treat_noncallback_parameters_as_callback_obligations() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let directory = temporary_directory("emit-noncallback-parameter");
    let result = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
        .env("SOLID_TYPEFACTS_BIN", &typefacts)
        .args(["--project"])
        .arg(root.join("fixtures/reactive-ir/package-noncallback-parameter-producer/tsconfig.json"))
        .args(["--emit-contract"])
        .arg(directory.join("solid-reactivity.json"))
        .args([
            "--package-name",
            "noncallback-package",
            "--package-version",
            "1.0.0",
        ])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn cli_resolves_arguments_to_locally_returned_functions() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let directory = temporary_directory("emit-returned-value-consumer");
    let result = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
        .env("SOLID_TYPEFACTS_BIN", &typefacts)
        .args(["--project"])
        .arg(root.join("fixtures/reactive-ir/package-returned-value-consumer/tsconfig.json"))
        .args(["--emit-contract"])
        .arg(directory.join("solid-reactivity.json"))
        .args([
            "--package-name",
            "returned-value-package",
            "--package-version",
            "1.0.0",
        ])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn cyclic_unknown_callback_forwarding_terminates() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let directory = temporary_directory("emit-cyclic-unknown-callback");
    let contract_output = directory.join("solid-reactivity.json");
    let mut child =
        Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
            .env("SOLID_TYPEFACTS_BIN", &typefacts)
            .args(["--project"])
            .arg(root.join(
                "fixtures/reactive-ir/package-cyclic-unknown-callback-producer/tsconfig.json",
            ))
            .args(["--emit-contract"])
            .arg(&contract_output)
            .args([
                "--package-name",
                "callback-package",
                "--package-version",
                "1.0.0",
            ])
            // stdout is never read here, and an undrained pipe that fills
            // would block the child and read as a convergence failure.
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
    // The guard is for a non-terminating fixed point, which no deadline would
    // satisfy; it is not a performance budget. Emission for this fixture
    // measures in tens of milliseconds, so the margin is enormous either way,
    // and a tight bound only buys false failures: at 5s this tripped on
    // scheduler delay when the suite ran its process tests in parallel, each
    // spawning its own TypeFacts producer, rather than on the analysis.
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        if child.try_wait().unwrap().is_some() {
            break;
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            panic!("cyclic callback-obligation propagation did not converge");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let contract = expanded_contract(&contract_output);
    assert!(
        contract["entrypoints"]["."]["exports"]
            .as_object()
            .unwrap()
            .values()
            .any(|summary| summary["callbacks"] == serde_json::json!({ "status": "unknown" }))
    );
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn package_generator_resolves_legacy_esm_and_rejects_legacy_cjs() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let directory = temporary_directory("legacy-entrypoint-contract");
    for fixture in [
        "legacy-module-entrypoint",
        "legacy-main-esm-entrypoint",
        "legacy-index-esm-entrypoint",
    ] {
        let output = directory.join(format!("{fixture}.json"));
        let esm = Command::new("node")
            .arg(root.join("packages/cli/bin/solid-checker.mjs"))
            .args(["contract", "generate", "--package-root"])
            .arg(root.join("fixtures/package-contracts").join(fixture))
            .arg("--output")
            .arg(&output)
            .env(
                "SOLID_CHECKER_NATIVE_BIN",
                env!("CARGO_BIN_EXE_solid-checker-rust"),
            )
            .env("SOLID_TYPEFACTS_BIN", &typefacts)
            .output()
            .unwrap();
        assert!(
            esm.status.success(),
            "{fixture}: {}",
            String::from_utf8_lossy(&esm.stderr)
        );
        let contract = expanded_contract(&output);
        assert_eq!(
            without_claim_evidence(
                &contract["entrypoints"]["."]["exports"]["scheduleLegacy"]["callbacks"]
            ),
            serde_json::json!([{ "parameter": 0, "execution": "deferred" }]),
            "{fixture}"
        );
    }

    let cjs = Command::new("node")
        .arg(root.join("packages/cli/bin/solid-checker.mjs"))
        .args(["contract", "generate", "--package-root"])
        .arg(root.join("fixtures/package-contracts/legacy-cjs-entrypoint"))
        .arg("--output")
        .arg(directory.join("cjs.json"))
        .env(
            "SOLID_CHECKER_NATIVE_BIN",
            env!("CARGO_BIN_EXE_solid-checker-rust"),
        )
        .env("SOLID_TYPEFACTS_BIN", &typefacts)
        .output()
        .unwrap();
    assert!(!cjs.status.success());
    assert!(
        String::from_utf8_lossy(&cjs.stderr).contains("only a CJS runtime target"),
        "{}",
        String::from_utf8_lossy(&cjs.stderr)
    );
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn package_generator_discovers_exact_and_wildcard_subpaths() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let fixture = root.join("fixtures/package-contracts/multi-entrypoint");
    let directory = temporary_directory("multi-entrypoint-contract");
    let package = directory.join("package");
    fs::create_dir_all(package.join("features")).unwrap();
    for file in [
        "empty.ts",
        "package.json",
        "index.ts",
        "node.ts",
        "state.ts",
        "state-impl.ts",
    ] {
        fs::copy(fixture.join(file), package.join(file)).unwrap();
    }
    fs::copy(
        fixture.join("features/alpha.ts"),
        package.join("features/alpha.ts"),
    )
    .unwrap();
    let output = directory.join("solid-reactivity.json");
    let result = Command::new("node")
        .arg(root.join("packages/cli/bin/solid-checker.mjs"))
        .args(["contract", "generate", "--package-root"])
        .arg(&package)
        .arg("--output")
        .arg(&output)
        .args(["--conditions", "browser,import"])
        .env(
            "SOLID_CHECKER_NATIVE_BIN",
            env!("CARGO_BIN_EXE_solid-checker-rust"),
        )
        .env("SOLID_TYPEFACTS_BIN", &typefacts)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let contract = expanded_contract(&output);
    assert_eq!(contract["package"]["name"], "multi-entrypoint-package");
    assert_eq!(contract["package"]["version"], "1.2.3");
    assert_eq!(contract["evidence"]["kind"], "inferred");
    assert_eq!(
        contract["entrypoints"]["."]["exports"]["rootConstant"]["evidence"]["kind"],
        "inferred"
    );
    assert_eq!(
        contract["entrypoints"]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect::<Vec<_>>(),
        [".", "./features/alpha", "./state"]
    );
    assert_eq!(
        contract["entrypoints"]["."]["exports"]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect::<Vec<_>>(),
        ["default", "rootConstant", "rootValue"]
    );
    assert_eq!(
        contract["entrypoints"]["./state"]["exports"]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect::<Vec<_>>(),
        ["stateConstant", "stateValue"]
    );
    assert_eq!(
        contract["entrypoints"]["./features/alpha"]["exports"]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect::<Vec<_>>(),
        ["alphaValue"]
    );
    assert!(fs::read_dir(&package).unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".solid-checker-contract-")
    }));
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn package_generator_isolates_each_entrypoint_from_unrelated_runtime_files() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let package = root.join("fixtures/package-contracts/entrypoint-isolation");
    let directory = temporary_directory("entrypoint-isolation-contract");
    let output = directory.join("solid-reactivity.json");
    let result = Command::new("node")
        .arg(root.join("packages/cli/bin/solid-checker.mjs"))
        .args(["contract", "generate", "--package-root"])
        .arg(&package)
        .arg("--output")
        .arg(&output)
        .args(["--entrypoint", ".", "--entrypoint", "./feature"])
        .env(
            "SOLID_CHECKER_NATIVE_BIN",
            env!("CARGO_BIN_EXE_solid-checker-rust"),
        )
        .env("SOLID_TYPEFACTS_BIN", &typefacts)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let contract = expanded_contract(&output);
    assert_eq!(
        contract["entrypoints"]["."]["exports"]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect::<Vec<_>>(),
        ["readRoot"]
    );
    assert_eq!(
        contract["entrypoints"]["./feature"]["exports"]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect::<Vec<_>>(),
        ["featureValue"]
    );
    let broken = Command::new("node")
        .arg(root.join("packages/cli/bin/solid-checker.mjs"))
        .args(["contract", "generate", "--package-root"])
        .arg(&package)
        .arg("--output")
        .arg(directory.join("broken.json"))
        .args(["--entrypoint", "./broken"])
        .env(
            "SOLID_CHECKER_NATIVE_BIN",
            env!("CARGO_BIN_EXE_solid-checker-rust"),
        )
        .env("SOLID_TYPEFACTS_BIN", &typefacts)
        .output()
        .unwrap();
    assert!(
        broken.status.success(),
        "{}",
        String::from_utf8_lossy(&broken.stderr)
    );
    let broken_contract = expanded_contract(&directory.join("broken.json"));
    assert_eq!(
        broken_contract["entrypoints"]["./broken"]["exports"]["publicScheduler"]["callbacks"],
        serde_json::json!({ "status": "unknown" })
    );
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn package_generator_detects_the_dialect_from_the_package_root() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let package = root.join("fixtures/package-contracts/dialect-detection");
    let directory = temporary_directory("dialect-detection-contract");
    let output = directory.join("solid-reactivity.json");
    let result = Command::new("node")
        .arg(root.join("packages/cli/bin/solid-checker.mjs"))
        .args(["contract", "generate", "--package-root"])
        .arg(&package)
        .arg("--output")
        .arg(&output)
        .env(
            "SOLID_CHECKER_NATIVE_BIN",
            env!("CARGO_BIN_EXE_solid-checker-rust"),
        )
        .env("SOLID_TYPEFACTS_BIN", &typefacts)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let contract = expanded_contract(&output);
    assert_eq!(
        contract["entrypoints"]["."]["exports"]["observe"]["kind"],
        "function"
    );
    assert_eq!(
        without_claim_evidence(&contract["entrypoints"]["."]["exports"]["indirect"]["callbacks"]),
        serde_json::json!([{ "parameter": 0, "execution": "tracked" }])
    );
    assert_eq!(
        without_claim_evidence(
            &contract["entrypoints"]["."]["exports"]["indirectResource"]["callbacks"]
        ),
        serde_json::json!([{ "parameter": 0, "execution": "tracked" }])
    );
    assert_eq!(
        contract["entrypoints"]["."]["exports"]["returnedAccessor"]["returns"]["kind"],
        "accessor"
    );
    assert_eq!(
        contract["entrypoints"]["."]["exports"]["returnedResource"]["returns"]["kind"],
        "accessor"
    );
    assert_eq!(
        contract["entrypoints"]["."]["exports"]["assignedResource"]["returns"]["kind"],
        "accessor"
    );
    assert_eq!(
        without_claim_evidence(&contract["entrypoints"]["."]["exports"]["tupleResult"]["returns"]),
        serde_json::json!({
            "kind": "tuple",
            "elements": [
                { "kind": "store-path", "label": "result[0]" },
                { "kind": "accessor", "label": "result[1]" }
            ]
        })
    );
    assert_eq!(
        without_claim_evidence(&contract["entrypoints"]["."]["exports"]["objectResult"]["returns"]),
        serde_json::json!({
            "kind": "object",
            "properties": {
                "active": { "kind": "accessor", "label": "active" },
                "pending": { "kind": "accessor", "label": "memo result" }
            }
        })
    );
    for export in [
        "projectedObjectResult",
        "projectedAliasResult",
        "projectedTupleResult",
    ] {
        assert_eq!(
            contract["entrypoints"]["."]["exports"][export]["returns"]["kind"], "accessor",
            "missing projected return for {export}"
        );
    }
    assert_eq!(
        without_claim_evidence(
            &contract["entrypoints"]["."]["exports"]["identityResult"]["returns"]
        ),
        serde_json::json!({ "kind": "argument", "parameter": 0 })
    );
    assert_eq!(
        without_claim_evidence(
            &contract["entrypoints"]["."]["exports"]["contextLocation"]["returns"]
        ),
        serde_json::json!({
            "kind": "object",
            "properties": {
                "pathname": { "kind": "accessor", "label": "pathname" }
            }
        })
    );
    assert_eq!(
        contract["entrypoints"]["."]["exports"]["contextParams"]["returns"]["kind"],
        "store-path"
    );
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn shorthand_property_values_resolve_through_block_scope() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let package = root.join("fixtures/package-contracts/shorthand-block-scope");
    let directory = temporary_directory("shorthand-block-scope-contract");
    let output = directory.join("solid-reactivity.json");
    let result = Command::new("node")
        .arg(root.join("packages/cli/bin/solid-checker.mjs"))
        .args(["contract", "generate", "--package-root"])
        .arg(&package)
        .arg("--output")
        .arg(&output)
        .env(
            "SOLID_CHECKER_NATIVE_BIN",
            env!("CARGO_BIN_EXE_solid-checker-rust"),
        )
        .env("SOLID_TYPEFACTS_BIN", &typefacts)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let contract = expanded_contract(&output);
    let exports = &contract["entrypoints"]["."]["exports"];

    // A same-spelled accessor in a sibling block is invisible at the
    // shorthand. It must neither be chosen nor make the visible declaration
    // ambiguous, which the spelling within the enclosing function cannot
    // distinguish.
    for export in ["scopedShorthand", "writtenShorthand"] {
        assert_eq!(
            without_claim_evidence(&exports[export]["returns"]),
            serde_json::json!({
                "kind": "object",
                "properties": { "tracked": { "kind": "accessor", "label": "tracked" } }
            }),
            "missing proven shorthand return for {export}"
        );
    }

    // A shorthand naming a *named relative import* joins to the exporting
    // file's declaration — exact ESM resolution against the project's own
    // file set, then the same accessor-map match as the same-file arm.
    assert_eq!(
        without_claim_evidence(&exports["importedAccessorShorthand"]["returns"]),
        serde_json::json!({
            "kind": "object",
            "properties": {
                "importedTracked": { "kind": "accessor", "label": "importedTracked" }
            }
        }),
        "missing proven cross-file shorthand return"
    );

    for (export, property, label) in [
        (
            "defaultReexportShorthand",
            "defaultFromBarrel",
            "defaultFromBarrel",
        ),
        ("namedReexportShorthand", "chainedTracked", "chainedTracked"),
        ("exportAllShorthand", "starTracked", "starTracked"),
    ] {
        assert_eq!(
            without_claim_evidence(&exports[export]["returns"]),
            serde_json::json!({
                "kind": "object",
                "properties": {
                    property: { "kind": "accessor", "label": label }
                }
            }),
            "missing proven re-export shorthand return for {export}"
        );
    }

    // Each of these names a binding that is provably not a local accessor, or
    // one this file's scope tree does not declare. A same-spelled accessor is
    // in the enclosing function for the first two; none of them may borrow it.
    for export in [
        "unprovenShorthand",
        "shadowedShorthand",
        "importedShorthand",
        "namespaceShorthand",
    ] {
        assert_eq!(
            exports[export]["returns"],
            serde_json::Value::Null,
            "unproven shorthand return claimed for {export}"
        );
    }
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn package_generator_follows_runtime_esm_behind_declarations() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let package = root.join("fixtures/package-contracts/esm-barrel");
    let directory = temporary_directory("esm-barrel-contract");
    let output = directory.join("solid-reactivity.json");
    let result = Command::new("node")
        .arg(root.join("packages/cli/bin/solid-checker.mjs"))
        .args(["contract", "generate", "--package-root"])
        .arg(&package)
        .arg("--output")
        .arg(&output)
        .env(
            "SOLID_CHECKER_NATIVE_BIN",
            env!("CARGO_BIN_EXE_solid-checker-rust"),
        )
        .env("SOLID_TYPEFACTS_BIN", &typefacts)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let contract = expanded_contract(&output);
    assert_eq!(
        contract["entrypoints"]["."]["exports"]["createValue"]["kind"],
        "function"
    );
    assert_eq!(
        contract["entrypoints"]["."]["exports"]["createAlias"]["kind"],
        "function"
    );
    assert_eq!(
        contract["entrypoints"]["."]["exports"]["createLocal"]["kind"],
        "function"
    );
    assert_eq!(
        contract["entrypoints"]["."]["exports"]["createConditional"]["kind"],
        "function"
    );
    assert_eq!(
        contract["entrypoints"]["."]["exports"]["createFromMemberFactory"]["kind"],
        "function"
    );
    assert_eq!(
        contract["entrypoints"]["."]["exports"]["factoryComponent"]["kind"],
        "function"
    );
    assert_eq!(
        contract["entrypoints"]["."]["exports"]["bootstrapSource"]["kind"],
        "value"
    );
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn package_generator_collapses_semantically_identical_conditional_targets() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let package = root.join("fixtures/package-contracts/conditional-targets");
    let directory = temporary_directory("conditional-targets-contract");
    let output = directory.join("solid-reactivity.json");
    let result = Command::new("node")
        .arg(root.join("packages/cli/bin/solid-checker.mjs"))
        .args(["contract", "generate", "--package-root"])
        .arg(&package)
        .arg("--output")
        .arg(&output)
        .env(
            "SOLID_CHECKER_NATIVE_BIN",
            env!("CARGO_BIN_EXE_solid-checker-rust"),
        )
        .env("SOLID_TYPEFACTS_BIN", &typefacts)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let contract = expanded_contract(&output);
    let summary = &contract["entrypoints"]["."]["exports"]["maybeRead"];
    assert_eq!(summary["kind"], "function");
    assert_eq!(summary["callbacks"][0]["parameter"], 0);
    assert_eq!(summary["callbacks"][0]["execution"], "inline");
    assert!(summary["variants"].is_null());
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn package_generator_orders_overlapping_conditional_callback_semantics() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let package = root.join("fixtures/package-contracts/conditional-callback-conflict");
    let directory = temporary_directory("conditional-callback-conflict-contract");
    let output = directory.join("solid-reactivity.json");
    let result = Command::new("node")
        .arg(root.join("packages/cli/bin/solid-checker.mjs"))
        .args(["contract", "generate", "--package-root"])
        .arg(&package)
        .arg("--output")
        .arg(&output)
        .env(
            "SOLID_CHECKER_NATIVE_BIN",
            env!("CARGO_BIN_EXE_solid-checker-rust"),
        )
        .env("SOLID_TYPEFACTS_BIN", &typefacts)
        .output()
        .unwrap();
    // `package.json#exports` is ordered and resolved first-match-wins, and the
    // generator now records that order as each variant's `precedence`. This
    // overlap is therefore representable rather than ambiguous: `development`
    // is declared first, so a consumer that selects it resolves the `inline`
    // branch, and everything else falls through to the `deferred` default.
    // Refusing here would discard a fact the export map states outright.
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert_eq!(result.status.code(), Some(0), "{stderr}");
    let contract = fs::read_to_string(&output).unwrap();
    let document: serde_json::Value = serde_json::from_str(&contract).unwrap();
    let variants = document["summaries"]
        .as_object()
        .unwrap()
        .values()
        .find_map(|summary| summary.get("variants"))
        .expect("conditional export should carry variants")
        .as_array()
        .unwrap()
        .clone();
    let ordered = variants
        .iter()
        .map(|variant| {
            (
                variant["conditions"][0].as_str().unwrap().to_owned(),
                variant["precedence"].as_u64().unwrap(),
                variant["summary"]["callbacks"][0]["execution"]
                    .as_str()
                    .unwrap()
                    .to_owned(),
            )
        })
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        ordered,
        [
            ("default".to_owned(), 1, "deferred".to_owned()),
            ("development".to_owned(), 0, "inline".to_owned()),
        ]
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>(),
        "{contract}"
    );
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn package_generator_classifies_callbacks_invoked_by_returned_schedulers_as_deferred() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let package = root.join("fixtures/package-contracts/deferred-returned-callback");
    let directory = temporary_directory("deferred-returned-callback-contract");
    let output = directory.join("solid-reactivity.json");
    let result = Command::new("node")
        .arg(root.join("packages/cli/bin/solid-checker.mjs"))
        .args(["contract", "generate", "--package-root"])
        .arg(&package)
        .arg("--output")
        .arg(&output)
        .env(
            "SOLID_CHECKER_NATIVE_BIN",
            env!("CARGO_BIN_EXE_solid-checker-rust"),
        )
        .env("SOLID_TYPEFACTS_BIN", &typefacts)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let contract = expanded_contract(&output);
    assert_eq!(
        without_claim_evidence(&contract["entrypoints"]["."]["exports"]["debounce"]["callbacks"]),
        serde_json::json!([{ "parameter": 0, "execution": "deferred" }])
    );
    assert_eq!(
        without_claim_evidence(&contract["entrypoints"]["."]["exports"]["direct"]["callbacks"]),
        serde_json::json!([{ "parameter": 0, "execution": "inline" }])
    );
    assert_eq!(
        without_claim_evidence(&contract["entrypoints"]["."]["exports"]["decorated"]["callbacks"]),
        serde_json::json!([{ "parameter": 0, "execution": "deferred" }])
    );
    assert_eq!(
        without_claim_evidence(
            &contract["entrypoints"]["."]["exports"]["throughIdentity"]["callbacks"]
        ),
        serde_json::json!([{ "parameter": 0, "execution": "deferred" }])
    );
    assert_eq!(
        without_claim_evidence(
            &contract["entrypoints"]["."]["exports"]["nestedThroughIdentity"]["callbacks"]
        ),
        serde_json::json!([{ "parameter": 0, "execution": "deferred" }])
    );
    assert_eq!(
        without_claim_evidence(
            &contract["entrypoints"]["."]["exports"]["nestedThroughCallable"]["callbacks"]
        ),
        serde_json::json!([{ "parameter": 0, "execution": "deferred" }])
    );
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn package_generator_handles_observer_constructors() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let package = root.join("fixtures/package-contracts/observer-and-string");
    let directory = temporary_directory("observer-and-string-contract");
    let output = directory.join("solid-reactivity.json");
    let result = Command::new("node")
        .arg(root.join("packages/cli/bin/solid-checker.mjs"))
        .args(["contract", "generate", "--package-root"])
        .arg(&package)
        .arg("--output")
        .arg(&output)
        .env(
            "SOLID_CHECKER_NATIVE_BIN",
            env!("CARGO_BIN_EXE_solid-checker-rust"),
        )
        .env("SOLID_TYPEFACTS_BIN", &typefacts)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let contract = expanded_contract(&output);
    let exports = &contract["entrypoints"]["."]["exports"];
    for name in [
        "observeIntersection",
        "observeResize",
        "observeMutation",
        "observePerformance",
    ] {
        assert_eq!(
            without_claim_evidence(&exports[name]["callbacks"]),
            serde_json::json!([{ "parameter": 0, "execution": "deferred" }]),
            "{name}"
        );
    }
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn package_generator_handles_plain_js_string_calls() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let package = root.join("fixtures/package-contracts/plain-js-string");
    let directory = temporary_directory("plain-js-string-contract");
    let output = directory.join("solid-reactivity.json");
    let result = Command::new("node")
        .arg(root.join("packages/cli/bin/solid-checker.mjs"))
        .args(["contract", "generate", "--package-root"])
        .arg(&package)
        .arg("--output")
        .arg(&output)
        .env(
            "SOLID_CHECKER_NATIVE_BIN",
            env!("CARGO_BIN_EXE_solid-checker-rust"),
        )
        .env("SOLID_TYPEFACTS_BIN", &typefacts)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let contract = expanded_contract(&output);
    let exports = &contract["entrypoints"]["."]["exports"];
    for name in ["convertMap", "convertString"] {
        assert_eq!(exports[name]["kind"], "function", "{name}");
    }
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn package_generator_emits_parameter_member_reads_without_promoting_local_members() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let package = root.join("fixtures/package-contracts/parameter-member-read");
    let directory = temporary_directory("parameter-member-read-contract");
    let output = directory.join("solid-reactivity.json");
    let result = Command::new("node")
        .arg(root.join("packages/cli/bin/solid-checker.mjs"))
        .args(["contract", "generate", "--package-root"])
        .arg(&package)
        .arg("--output")
        .arg(&output)
        .env(
            "SOLID_CHECKER_NATIVE_BIN",
            env!("CARGO_BIN_EXE_solid-checker-rust"),
        )
        .env("SOLID_TYPEFACTS_BIN", &typefacts)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let contract = expanded_contract(&output);
    assert_eq!(
        without_claim_evidence(&contract["entrypoints"]["."]["exports"]["drop"]["reactiveReads"]),
        serde_json::json!([{ "kind": "parameter-member", "parameter": 0 }])
    );
    for name in ["readModuleLocal", "readBodyLocal"] {
        assert!(
            contract["entrypoints"]["."]["exports"][name]["reactiveReads"].is_null(),
            "{name}"
        );
    }
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn package_generator_handles_runtime_semantics_matrix() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let package = root.join("fixtures/package-contracts/runtime-semantics");
    let directory = temporary_directory("runtime-semantics-contract");
    let output = directory.join("solid-reactivity.json");
    let result = Command::new("node")
        .arg(root.join("packages/cli/bin/solid-checker.mjs"))
        .args(["contract", "generate", "--package-root"])
        .arg(&package)
        .arg("--output")
        .arg(&output)
        .env(
            "SOLID_CHECKER_NATIVE_BIN",
            env!("CARGO_BIN_EXE_solid-checker-rust"),
        )
        .env("SOLID_TYPEFACTS_BIN", &typefacts)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let contract = expanded_contract(&output);
    let exports = &contract["entrypoints"]["."]["exports"];
    for name in [
        "arrayFrom",
        "typedArrayFrom",
        "int8ArrayFrom",
        "uint8ClampedArrayFrom",
        "int16ArrayFrom",
        "uint16ArrayFrom",
        "int32ArrayFrom",
        "uint32ArrayFrom",
        "float32ArrayFrom",
        "float64ArrayFrom",
        "bigInt64ArrayFrom",
        "bigUint64ArrayFrom",
    ] {
        assert_eq!(
            without_claim_evidence(&exports[name]["callbacks"]),
            serde_json::json!([{ "parameter": 1, "execution": "inline" }]),
            "{name}"
        );
    }
    for name in ["replace", "replaceAll"] {
        assert_eq!(
            without_claim_evidence(&exports[name]["callbacks"]),
            serde_json::json!([{ "parameter": 0, "execution": "inline" }]),
            "{name}"
        );
    }
    for name in [
        "observeReporting",
        "observeIntersection",
        "postTask",
        "retainArray",
        "retainSet",
        "retainMap",
    ] {
        assert_eq!(
            without_claim_evidence(&exports[name]["callbacks"]),
            serde_json::json!([{ "parameter": 0, "execution": "deferred" }]),
            "{name}"
        );
    }
    for name in ["getPosition", "watchPosition"] {
        assert_eq!(
            without_claim_evidence(&exports[name]["callbacks"]),
            serde_json::json!([
                { "parameter": 0, "execution": "deferred" },
                { "parameter": 1, "execution": "deferred" }
            ]),
            "{name}"
        );
    }
    for name in [
        "convertNumber",
        "convertBoolean",
        "convertBigInt",
        "convertSymbol",
        "convertObject",
        "constructArray",
        "constructSet",
        "constructMap",
        "constructWeakSet",
        "constructWeakMap",
        "shadowedString",
        "shadowedQueueMicrotask",
    ] {
        assert_eq!(
            without_claim_evidence(&exports[name]["callbacks"]),
            serde_json::Value::Null,
            "{name}"
        );
    }
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn package_generator_marks_shadowed_observer_semantics_unknown() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let package = root.join("fixtures/package-contracts/runtime-semantics-shadowed");
    let directory = temporary_directory("runtime-semantics-shadowed-contract");
    let output = directory.join("solid-reactivity.json");
    let result = Command::new("node")
        .arg(root.join("packages/cli/bin/solid-checker.mjs"))
        .args(["contract", "generate", "--package-root"])
        .arg(&package)
        .arg("--output")
        .arg(&output)
        .env(
            "SOLID_CHECKER_NATIVE_BIN",
            env!("CARGO_BIN_EXE_solid-checker-rust"),
        )
        .env("SOLID_TYPEFACTS_BIN", &typefacts)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let contract = expanded_contract(&output);
    assert_eq!(
        contract["entrypoints"]["."]["exports"]["shadowedResizeObserver"]["callbacks"],
        serde_json::json!({ "status": "unknown" })
    );
    fs::remove_dir_all(directory).unwrap();
}

/// A dependency contract's own `kind: "value"` survives the boundary, and the
/// same export refuses without it. The dependency has no type declarations, so
/// inside *this* package's project the re-exported specifier is `any` and no
/// closed type answers `kind` — which is a refusal when the answer would
/// otherwise be this project's guess, and not one when the dependency's own
/// contract already decided it against the dependency's own sources.
///
/// See fixtures/package-contracts/carried-value-kind/README.md.
#[test]
fn package_generator_keeps_a_dependency_contracts_value_kind_and_refuses_without_it() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let package = root.join("fixtures/package-contracts/carried-value-kind");
    let directory = temporary_directory("carried-value-kind-contract");
    let generate = |output: &Path, contract: Option<&Path>| {
        let mut command = Command::new("node");
        command
            .arg(root.join("packages/cli/bin/solid-checker.mjs"))
            .args(["contract", "generate", "--package-root"])
            .arg(&package)
            .arg("--output")
            .arg(output);
        if let Some(contract) = contract {
            command.arg("--contract").arg(contract);
        }
        command
            .env(
                "SOLID_CHECKER_NATIVE_BIN",
                env!("CARGO_BIN_EXE_solid-checker-rust"),
            )
            .env("SOLID_TYPEFACTS_BIN", &typefacts)
            .output()
            .unwrap()
    };

    let carried = directory.join("carried.json");
    let result = generate(&carried, Some(&package.join("dependency-contract.json")));
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let contract = expanded_contract(&carried);
    assert_eq!(
        contract["entrypoints"]["."]["exports"]["hostValue"]["kind"], "value",
        "{contract}"
    );
    // The laundering channel, closed: an `inferred` dependency contract found
    // by `dependencyContracts()` at `node_modules/<dep>/solid-reactivity.json`
    // -- no `--contract`, no review -- has its `kind` re-decided here like any
    // local claim. `laundered-dependency` has no typings, so the wrong `value`
    // it claims for `addClickInterceptor(fn)` is refused rather than
    // republished.
    assert!(
        contract["entrypoints"].get("./laundered").is_none(),
        "{contract}"
    );
    let plan = fs::read_to_string(directory.join("carried.review.md")).unwrap();
    assert!(
        plan.contains("./laundered: solid-checker-rust: emit package contract:")
            && plan.contains(
                "exports \"addClickInterceptor\", whose runtime kind no closed type answers (Unknown)"
            ),
        "{plan}"
    );
    // And re-deciding decides: the same unreviewed provenance over a
    // dependency that *does* ship declarations corrects the wrong negative
    // instead of refusing it.
    let typed = &contract["entrypoints"]["."]["exports"]["addTypedInterceptor"];
    assert_eq!(typed["kind"], "function", "{contract}");
    assert_eq!(
        typed["callbacks"],
        serde_json::json!({ "status": "unknown" }),
        "{contract}"
    );

    // Every entrypoint whose kind this project must decide for itself is
    // refused, so generation fails and names the reason rather than writing a
    // contract with entrypoints missing.
    let refused = directory.join("refused.json");
    let result = generate(&refused, None);
    assert!(!result.status.success(), "{contract}");
    let message = format!(
        "{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(
        message
            .contains("exports \"hostValue\", whose runtime kind no closed type answers (Unknown)"),
        "{message}"
    );
    assert!(!refused.exists(), "{message}");
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn package_generator_recursively_generates_external_export_all_contracts() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let package = root.join("fixtures/package-contracts/external-reexport");
    let directory = temporary_directory("external-reexport-contract");
    let output = directory.join("solid-reactivity.json");
    let result = Command::new("node")
        .arg(root.join("packages/cli/bin/solid-checker.mjs"))
        .args(["contract", "generate", "--package-root"])
        .arg(&package)
        .arg("--output")
        .arg(&output)
        .env(
            "SOLID_CHECKER_NATIVE_BIN",
            env!("CARGO_BIN_EXE_solid-checker-rust"),
        )
        .env("SOLID_TYPEFACTS_BIN", &typefacts)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let contract = expanded_contract(&output);
    assert_eq!(
        contract["entrypoints"]["."]["exports"]["dependencyValue"]["kind"],
        "function"
    );
    assert_eq!(
        contract["entrypoints"]["."]["exports"]["namedDependencyValue"]["kind"],
        "function"
    );
    // Recursive generation produces an inferred dependency draft. It is exact
    // enough to expand the barrel, but its unreviewed callback claim must not
    // silently become trusted evidence in the parent draft.
    assert_eq!(
        without_claim_evidence(&contract["entrypoints"]["."]["exports"]["forward"]["callbacks"]),
        serde_json::json!({ "status": "unknown" })
    );
    fs::remove_dir_all(directory).unwrap();
}

/// The recursion above is driven by one line this binary writes to stderr, and
/// the test above cannot tell a working marker from a working prose fallback.
///
/// This pins the interface itself, end to end and in both directions: this
/// binary's *real* stderr at the missing-dependency boundary is fed to the
/// generator's *real* parser (`unresolvedDependencyModule` in
/// packages/cli/scripts/generate-package-contract.mjs). Reword the marker on
/// either side and this fails here, naming the seam, instead of quietly
/// degrading demand-driven dependency generation into a refused entrypoint --
/// which exits 0 and is therefore invisible.
///
/// It also holds the refusal classification: the marker must travel *with* the
/// `emit package contract:` prose, because `runChecked` treats a native
/// failure without that prefix as a bug to rethrow rather than a boundary to
/// resolve, and the retry loop would never see the marker at all.
#[test]
fn package_generator_dependency_boundary_marker_drives_recursion() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let package = root.join("fixtures/package-contracts/external-reexport");
    let entry = package.join("index.ts");
    let directory = temporary_directory("external-reexport-boundary");
    let project = directory.join("tsconfig.json");
    fs::write(
        &project,
        serde_json::to_string(&serde_json::json!({
            "compilerOptions": {
                "allowJs": true,
                "checkJs": true,
                "module": "ESNext",
                "moduleResolution": "Bundler",
                "skipLibCheck": true,
                "target": "ES2022"
            },
            "files": [entry.to_string_lossy()]
        }))
        .unwrap(),
    )
    .unwrap();
    // Deliberately no `--contract`: the dependency's contract is exactly what
    // is missing, which is the boundary the generator resolves by recursing.
    let emitted = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
        .arg("--project")
        .arg(&project)
        .arg("--emit-contract")
        .arg(directory.join("solid-reactivity.json"))
        .args(["--package-name", "external-reexport-package"])
        .args(["--package-version", "1.0.0"])
        .arg("--contract-entry-file")
        .arg(&entry)
        .arg("--contract-package-root")
        .arg(&package)
        .env("SOLID_TYPEFACTS_BIN", &typefacts)
        .output()
        .unwrap();
    assert!(
        !emitted.status.success(),
        "emission must refuse without the dependency contract"
    );
    let stderr = String::from_utf8(emitted.stderr).unwrap();
    assert!(
        stderr.contains("solid-checker:unresolved-dependency-module=dependency-package"),
        "missing dependency marker in: {stderr}"
    );
    assert!(
        stderr.contains("emit package contract:"),
        "the marker must accompany a refusal the generator classifies as one: {stderr}"
    );

    // The other half of the seam: the generator's own parser, on these bytes.
    let parsed = Command::new("node")
        .arg("--input-type=module")
        .arg("-e")
        .arg(format!(
            "import {{ unresolvedDependencyModule }} from {:?};\
             process.stdout.write(String(unresolvedDependencyModule(process.env.NATIVE_STDERR)));",
            root.join("packages/cli/scripts/generate-package-contract.mjs")
                .to_string_lossy()
        ))
        .env("NATIVE_STDERR", &stderr)
        .output()
        .unwrap();
    assert!(
        parsed.status.success(),
        "{}",
        String::from_utf8_lossy(&parsed.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&parsed.stdout),
        "dependency-package"
    );
    fs::remove_dir_all(directory).unwrap();
}

/// The attribution seam, on real bytes from both sides.
///
/// The native emitter names every unknown-claim decision on one stderr line;
/// the generator parses those lines and records them on the matching
/// `unknown-sentinel` items of the review plan. Neither half is observable in
/// the contract document -- schema v1's `unknownClaim` is
/// `additionalProperties: false`, so the reason cannot live there -- which
/// means a silently broken pairing costs the review plan its only account of
/// *why* a claim is unknown, and nothing else fails.
///
/// So this feeds the binary's actual stderr to the script's actual parser, and
/// then runs the whole generation to check the notes land on the right items.
#[test]
fn unknown_claim_attribution_markers_reach_the_review_plan() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let package = root.join("fixtures/package-contracts/unresolved-dispatch-attribution");
    let directory = temporary_directory("unknown-claim-attribution");
    let output = directory.join("solid-reactivity.json");
    let result = Command::new("node")
        .arg(root.join("packages/cli/bin/solid-checker.mjs"))
        .args(["contract", "generate", "--package-root"])
        .arg(&package)
        .arg("--output")
        .arg(&output)
        .env(
            "SOLID_CHECKER_NATIVE_BIN",
            env!("CARGO_BIN_EXE_solid-checker-rust"),
        )
        .env("SOLID_TYPEFACTS_BIN", &typefacts)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    // The markers are addressed to the generator, not to a person: nothing a
    // human reads may carry them.
    let visible = format!(
        "{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(
        !visible.contains("solid-checker:unknown-claim-attribution="),
        "the marker leaked into human-visible output: {visible}"
    );

    let plan: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(directory.join("solid-reactivity.review.json")).unwrap(),
    )
    .unwrap();
    let mechanism = |export: &str, field: &str| -> Option<String> {
        plan["items"].as_array()?.iter().find_map(|item| {
            (item["kind"] == "unknown-sentinel"
                && item["target"]["export"] == export
                && item["target"]["field"] == field)
                .then(|| {
                    item["because"]["attributions"][0]["mechanism"]
                        .as_str()
                        .unwrap_or_default()
                        .to_owned()
                })
        })
    };
    // Which rung answered is the whole content of the note: `Direct` holds the
    // obligation in its own body, `Arrow` and `Helper` lexically contain it.
    assert_eq!(
        mechanism("Direct", "reactiveReads").as_deref(),
        Some("joined"),
        "{plan:#}"
    );
    assert_eq!(
        mechanism("Arrow", "returns").as_deref(),
        Some("enclosing-chain"),
        "{plan:#}"
    );
    assert_eq!(
        mechanism("Helper", "reactiveReads").as_deref(),
        Some("enclosing-chain"),
        "{plan:#}"
    );
    // The negative control has no unknown claim at all, so no note may name it.
    assert!(
        plan["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|item| item["kind"] != "unknown-sentinel" || item["target"]["export"] != "inert"),
        "an export that cannot reach the obligation was marked: {plan:#}"
    );

    // The other half of the seam: this binary's real stderr, this script's real
    // parser. A reworded or restructured line stops the review plan explaining
    // anything, and every other check in this file still passes.
    let entry = package.join("index.js");
    let project = directory.join("tsconfig.json");
    fs::write(
        &project,
        serde_json::to_string(&serde_json::json!({
            "compilerOptions": {
                "allowJs": true,
                "checkJs": true,
                "module": "ESNext",
                "moduleResolution": "Bundler",
                "skipLibCheck": true,
                "target": "ES2022"
            },
            "files": [
                entry.to_string_lossy(),
                package.join("channel.js").to_string_lossy(),
            ]
        }))
        .unwrap(),
    )
    .unwrap();
    let emitted = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
        .arg("--project")
        .arg(&project)
        .arg("--emit-contract")
        .arg(directory.join("direct.json"))
        .args(["--package-name", "unresolved-dispatch-attribution"])
        .args(["--package-version", "1.0.0"])
        .arg("--contract-entry-file")
        .arg(&entry)
        .arg("--contract-package-root")
        .arg(&package)
        .env("SOLID_TYPEFACTS_BIN", &typefacts)
        .output()
        .unwrap();
    assert!(
        emitted.status.success(),
        "{}",
        String::from_utf8_lossy(&emitted.stderr)
    );
    let stderr = String::from_utf8(emitted.stderr).unwrap();
    assert!(
        stderr.contains("solid-checker:unknown-claim-attribution="),
        "no attribution marker in: {stderr}"
    );
    let parsed = Command::new("node")
        .arg("--input-type=module")
        .arg("-e")
        .arg(format!(
            "import {{ unknownClaimAttributions }} from {:?};\
             const notes = unknownClaimAttributions(process.env.NATIVE_STDERR);\
             process.stdout.write(JSON.stringify([...new Set(notes.flatMap(note => note.exports))].sort()));",
            root.join("packages/cli/scripts/generate-package-contract.mjs")
                .to_string_lossy()
        ))
        .env("NATIVE_STDERR", &stderr)
        .output()
        .unwrap();
    assert!(
        parsed.status.success(),
        "{}",
        String::from_utf8_lossy(&parsed.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&parsed.stdout),
        r#"["Arrow","Direct","Helper"]"#
    );
    fs::remove_dir_all(directory).unwrap();
}

/// An obligation the ladder resolves to *no* export marks nothing, so there is
/// no `unknown-sentinel` item to hang the reason on -- and the resulting
/// contract is byte-identical to one where the analyzer never saw the
/// obligation. The two are not the same claim, and the difference is the one a
/// reviewer most needs: the second is silence, the first is a decision that no
/// export of this entrypoint can reach a proof obligation that exists.
#[test]
fn zero_export_attribution_narrowing_reaches_the_review_plan() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let package = root.join("fixtures/package-contracts/unreached-private-obligation");
    let directory = temporary_directory("zero-export-attribution");
    let output = directory.join("solid-reactivity.json");
    let result = Command::new("node")
        .arg(root.join("packages/cli/bin/solid-checker.mjs"))
        .args(["contract", "generate", "--package-root"])
        .arg(&package)
        .arg("--output")
        .arg(&output)
        .env(
            "SOLID_CHECKER_NATIVE_BIN",
            env!("CARGO_BIN_EXE_solid-checker-rust"),
        )
        .env("SOLID_TYPEFACTS_BIN", &typefacts)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let plan: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(directory.join("solid-reactivity.review.json")).unwrap(),
    )
    .unwrap();
    let items = plan["items"].as_array().unwrap();
    // Nothing was marked: the narrowing is correct, and the contract is the
    // same either way. That is precisely why the note has to exist.
    assert!(
        items.iter().all(|item| item["kind"] != "unknown-sentinel"),
        "{plan:#}"
    );
    let note = items
        .iter()
        .filter(|item| item["kind"] == "artifact-binding")
        .find_map(|item| {
            item["text"]
                .as_str()
                .filter(|text| text.contains("attributed to no export"))
        })
        .unwrap_or_else(|| panic!("no zero-export narrowing note on the plan: {plan:#}"));
    // The note has to name the obligation, where it is, and which rung decided
    // -- a bare "something was narrowed" is not checkable against the source.
    for expected in ["ReactiveDispatchUnresolved", "channel.js", "`reachability`"] {
        assert!(note.contains(expected), "{note}");
    }
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn package_generator_uses_bundled_solid_contract_for_renderer_reexports() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let package = root.join("fixtures/package-contracts/solid-reexport");
    let directory = temporary_directory("solid-reexport-contract");
    let output = directory.join("solid-reactivity.json");
    let result = Command::new("node")
        .arg(root.join("packages/cli/bin/solid-checker.mjs"))
        .args(["contract", "generate", "--package-root"])
        .arg(&package)
        .arg("--output")
        .arg(&output)
        .env(
            "SOLID_CHECKER_NATIVE_BIN",
            env!("CARGO_BIN_EXE_solid-checker-rust"),
        )
        .env("SOLID_TYPEFACTS_BIN", &typefacts)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let contract = expanded_contract(&output);
    for name in ["createMemo", "createSignal"] {
        assert_eq!(
            contract["entrypoints"]["."]["exports"][name]["kind"],
            "function"
        );
    }
    fs::remove_dir_all(directory).unwrap();
}

/// A project-owned contract that was reviewed against an earlier release of the
/// package is stale after an upgrade: it is evidence about an artifact this
/// project no longer installs.
///
/// The report must classify it rather than fail, and the analysis must fail
/// closed with a message that names the command that fixes it. Between them
/// these are how a user notices drift at all.
#[test]
fn cli_reports_a_project_owned_contract_that_the_installed_version_outran() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let fixture = root.join("fixtures/reactive-ir/package-consumer");
    let directory = temporary_directory("stale-contract");
    for file in ["App.tsx", "jsx.d.ts", "tsconfig.json"] {
        fs::copy(fixture.join(file), directory.join(file)).unwrap();
    }
    let package = directory.join("node_modules/reactive-package");
    fs::create_dir_all(&package).unwrap();
    fs::copy(
        fixture.join("node_modules/reactive-package/index.d.ts"),
        package.join("index.d.ts"),
    )
    .unwrap();
    let manifest = |version: &str| {
        format!(
            r#"{{
  "name": "reactive-package",
  "version": "{version}",
  "types": "index.d.ts",
  "peerDependencies": {{ "solid-js": "^2.0.0" }}
}}
"#
        )
    };
    fs::write(package.join("package.json"), manifest("1.0.0")).unwrap();
    let local = directory.join(".solid-checker/contracts/reactive-package");
    fs::create_dir_all(&local).unwrap();
    fs::write(
        local.join("solid-reactivity.json"),
        r#"{
  "schemaVersion": 1,
  "package": {
    "name": "reactive-package",
    "version": "1.0.0"
  },
  "compilerFactsProtocol": 1,
  "artifacts": {},
  "summaries": {
    "function-1": {
      "kind": "function",
      "reactiveReads": [
        { "kind": "accessor", "label": "project-owned reactive value" }
      ]
    }
  },
  "entrypoints": {
    ".": { "exports": { "function-1": ["readCount"] } }
  },
  "evidence": {
    "kind": "reviewed",
    "generator": "application developer"
  }
}
"#,
    )
    .unwrap();

    let check = |arguments: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
            .env("SOLID_TYPEFACTS_BIN", &typefacts)
            .args(arguments)
            .arg(directory.join("tsconfig.json"))
            .output()
            .unwrap()
    };

    let fresh = check(&["--format", "json", "--check-contracts", "--project"]);
    assert!(
        fresh.status.success(),
        "{}",
        String::from_utf8_lossy(&fresh.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&fresh.stdout).unwrap();
    assert_eq!(report["packages"][0]["status"], "local");
    assert_eq!(report["stale"], 0);
    // A certifying status carries neither a complaint nor an action.
    assert!(report["packages"][0]["remedy"].is_null());
    assert!(report["packages"][0]["detail"].is_null());

    // The dependency is upgraded; the reviewed contract still describes 1.0.0.
    fs::write(package.join("package.json"), manifest("1.1.0")).unwrap();

    let stale = check(&["--format", "json", "--check-contracts", "--project"]);
    assert_eq!(stale.status.code(), Some(1));
    let report: serde_json::Value = serde_json::from_slice(&stale.stdout).unwrap();
    assert_eq!(report["packages"][0]["status"], "stale");
    assert_eq!(report["stale"], 1);
    assert_eq!(report["missing"], 1);
    // The drift itself is reported, not just the label: both versions are
    // named, so the user does not have to open two files to see what moved.
    assert_eq!(
        report["packages"][0]["detail"],
        "the contract describes reactive-package 1.0.0, but 1.1.0 is installed"
    );
    let remedy = report["packages"][0]["remedy"].as_str().unwrap();
    assert!(
        remedy.contains("solid-checker contract generate"),
        "{remedy}"
    );
    assert!(
        remedy.contains(".solid-checker/contracts/reactive-package/solid-reactivity.json"),
        "{remedy}"
    );

    // Text output is the default one a user gets from `contract check`, with no
    // --format at all, and it carries the same remedy.
    let text = check(&["--check-contracts", "--project"]);
    assert_eq!(text.status.code(), Some(1));
    let rendered = String::from_utf8_lossy(&text.stdout);
    assert!(rendered.contains("reactive-package: stale"), "{rendered}");
    assert!(
        rendered.contains("the contract describes reactive-package 1.0.0, but 1.1.0 is installed"),
        "{rendered}"
    );
    assert!(
        rendered.contains("solid-checker contract generate"),
        "{rendered}"
    );
    assert!(
        rendered.contains("1 of 1 package contracts need action (1 stale)"),
        "{rendered}"
    );

    // Analysis fails closed on the contract without failing the run: the stale
    // contract is refused, and the package reports as uncertifiable at the
    // import instead of taking every other finding in the project down with it.
    let analysis = check(&["--format", "json", "--certify", "--project"]);
    assert_eq!(analysis.status.code(), Some(1));
    let snapshot: serde_json::Value = serde_json::from_slice(&analysis.stdout).unwrap();
    assert_eq!(snapshot["status"], "uncertifiable");
    let finding = &snapshot["findings"][0];
    assert_eq!(finding["id"], "SC9005");
    assert_eq!(finding["rule"], "package-contract-incomplete");
    // The message states what is true — a contract exists, for another
    // version — rather than claiming there is none.
    let message = finding["message"].as_str().unwrap();
    assert!(
        message.contains("has a reactivity contract for version 1.0.0"),
        "{message}"
    );
    assert!(message.contains("version 1.1.0 is installed"), "{message}");
    let hint = finding["hint"].as_str().unwrap();
    assert!(hint.contains("solid-checker contract generate"), "{hint}");
    assert!(
        finding["primaryLocation"]["path"]
            .as_str()
            .is_some_and(|path| path.ends_with("App.tsx")),
        "the finding anchors at the import, not at the project root"
    );
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn package_generator_preserves_environment_dependent_export_kind() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let package = root.join("fixtures/package-contracts/conditional-kind-divergence");
    let directory = temporary_directory("conditional-kind-divergence-contract");
    let output = directory.join("solid-reactivity.json");
    let result = Command::new("node")
        .arg(root.join("packages/cli/bin/solid-checker.mjs"))
        .args(["contract", "generate", "--package-root"])
        .arg(&package)
        .arg("--output")
        .arg(&output)
        .env(
            "SOLID_CHECKER_NATIVE_BIN",
            env!("CARGO_BIN_EXE_solid-checker-rust"),
        )
        .env("SOLID_TYPEFACTS_BIN", &typefacts)
        .output()
        .unwrap();
    // The base is deliberately conservative for an environment-unaware
    // consumer, while the complete variant summaries preserve the exact kind
    // selected by the ordered export map.
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert_eq!(result.status.code(), Some(0), "{stderr}");
    let contract = expanded_contract(&output);
    let summary = &contract["entrypoints"]["."]["exports"]["conditionalShape"];
    assert_eq!(summary["kind"], "value", "{contract}");
    let variants = summary["variants"].as_array().unwrap();
    assert!(
        variants.iter().any(|variant| {
            variant["conditions"] == serde_json::json!(["development"])
                && variant["summary"]["kind"] == "function"
                && variant["precedence"] == 0
        }),
        "{contract}"
    );
    assert!(
        variants.iter().any(|variant| {
            variant["conditions"] == serde_json::json!(["default"])
                && variant["summary"]["kind"] == "value"
                && variant["precedence"] == 1
        }),
        "{contract}"
    );
    fs::remove_dir_all(directory).unwrap();
}

/// Copies a fixture package into a scratch directory.
///
/// In-package generation *writes into the package root* (the contract, its
/// review plan, and a temporary candidate beside them), so the checked-in
/// fixture is never the thing generated into.
fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).unwrap();
        }
    }
}

/// The byte binding, end to end, against the real engine on both sides.
///
/// `artifacts.implementation` is the only thing in schema v1 that ties a
/// contract to the code it describes, and it is only emitted when the single
/// runtime artifact sits inside the contract's own directory -- the in-package
/// output form. Every other real-binary generation in this repository writes
/// `--output` to a scratch directory, which is *outside* the package by
/// construction and therefore takes the unbound branch: the emission path and
/// the consumer's hash check had only stub coverage, on either side of a seam
/// neither stub crosses.
///
/// This pins all three halves at once: the generator computes the hash of the
/// bytes it analyzed, `--validate-contract` recomputes it and agrees, and a
/// single changed byte in the implementation makes the same command refuse.
/// Without the last one the check could be vacuous -- a validator that never
/// reads the artifact passes every contract, correct hash or not.
#[test]
fn in_package_generation_binds_the_contract_to_the_implementation_bytes() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let directory = temporary_directory("in-package-artifact-binding");
    let package = directory.join("plain-js-string");
    copy_tree(
        &root.join("fixtures/package-contracts/plain-js-string"),
        &package,
    );
    // The default-output decision compares `process.cwd()` -- which Node
    // reports symlink-resolved -- against the resolved `--package-root`. On a
    // platform whose temporary directory is itself a symlink (macOS's
    // /var -> /private/var) an unresolved path makes the two differ and
    // silently takes the project-owned output form instead.
    let package = fs::canonicalize(&package).unwrap();

    // The default output is in-package only when the process runs *in* the
    // package root (`defaultOutput` in
    // packages/cli/scripts/generate-package-contract.mjs), so this passes no
    // `--output` and sets the working directory instead.
    let generated = Command::new("node")
        .arg(root.join("packages/cli/bin/solid-checker.mjs"))
        .args(["contract", "generate", "--package-root"])
        .arg(&package)
        .current_dir(&package)
        .env(
            "SOLID_CHECKER_NATIVE_BIN",
            env!("CARGO_BIN_EXE_solid-checker-rust"),
        )
        .env("SOLID_TYPEFACTS_BIN", &typefacts)
        .output()
        .unwrap();
    assert!(
        generated.status.success(),
        "{}",
        String::from_utf8_lossy(&generated.stderr)
    );

    let output = package.join("solid-reactivity.json");
    let contract: serde_json::Value = serde_json::from_slice(&fs::read(&output).unwrap()).unwrap();
    let implementation = &contract["artifacts"]["implementation"];
    assert_eq!(
        implementation["path"], "index.js",
        "the artifact path must resolve inside the contract's own directory: {contract}"
    );
    let expected = format!(
        "sha256:{:x}",
        <sha2::Sha256 as sha2::Digest>::digest(fs::read(package.join("index.js")).unwrap())
    );
    assert_eq!(
        implementation["hash"], expected,
        "the emitted hash must be the sha256 of the analyzed bytes: {contract}"
    );

    let validated = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
        .arg("--validate-contract")
        .arg(&output)
        .env("SOLID_TYPEFACTS_BIN", &typefacts)
        .output()
        .unwrap();
    assert!(
        validated.status.success(),
        "the real validator must accept the contract it was just handed: {}",
        String::from_utf8_lossy(&validated.stderr)
    );

    // One byte of the implementation, changed the way a republished release
    // changes it. The contract is now evidence about bytes this package no
    // longer contains, and loading it must say so rather than certify.
    let mut tampered = fs::read(package.join("index.js")).unwrap();
    tampered.push(b'\n');
    fs::write(package.join("index.js"), tampered).unwrap();
    let refused = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
        .arg("--validate-contract")
        .arg(&output)
        .env("SOLID_TYPEFACTS_BIN", &typefacts)
        .output()
        .unwrap();
    assert!(
        !refused.status.success(),
        "a changed implementation byte must fail validation"
    );
    let message = String::from_utf8_lossy(&refused.stderr);
    assert!(
        message.contains("implementation hash"),
        "the refusal must name the artifact whose bytes moved: {message}"
    );
    fs::remove_dir_all(directory).unwrap();
}

/// `package.integrity` is the only field in a contract that pins the *bytes*
/// of the release it describes, and until now nothing compared it to anything:
/// a contract bound to a version string alone still applies after a republish,
/// an `npm overrides` entry, or a local patch swaps the tarball underneath it.
///
/// This pins the enforced slice end to end, in all three states that matter.
/// The absent-lockfile state is not a footnote: it is most of the ecosystem
/// (pnpm, Yarn, a fresh checkout with no lock at all), and enforcement that
/// silently refused those contracts would be worse than none.
#[test]
fn cli_refuses_a_contract_whose_lockfile_integrity_moved_under_the_same_version() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let fixture = root.join("fixtures/reactive-ir/package-consumer");
    let directory = temporary_directory("contract-integrity");
    for file in ["App.tsx", "jsx.d.ts", "tsconfig.json"] {
        fs::copy(fixture.join(file), directory.join(file)).unwrap();
    }
    let package = directory.join("node_modules/reactive-package");
    fs::create_dir_all(&package).unwrap();
    fs::copy(
        fixture.join("node_modules/reactive-package/index.d.ts"),
        package.join("index.d.ts"),
    )
    .unwrap();
    fs::write(
        package.join("package.json"),
        r#"{
  "name": "reactive-package",
  "version": "1.0.0",
  "types": "index.d.ts",
  "peerDependencies": { "solid-js": "^2.0.0" }
}
"#,
    )
    .unwrap();
    // The version never moves in this test. Every state below differs only in
    // what the lockfile says about the bytes behind that one version.
    const AUDITED: &str = "sha512-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA==";
    const INSTALLED: &str = "sha512-BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB==";
    let local = directory.join(".solid-checker/contracts/reactive-package");
    fs::create_dir_all(&local).unwrap();
    fs::write(
        local.join("solid-reactivity.json"),
        format!(
            r#"{{
  "schemaVersion": 1,
  "package": {{
    "name": "reactive-package",
    "version": "1.0.0",
    "integrity": "{AUDITED}"
  }},
  "compilerFactsProtocol": 1,
  "artifacts": {{}},
  "summaries": {{
    "function-1": {{
      "kind": "function",
      "reactiveReads": [
        {{ "kind": "accessor", "label": "project-owned reactive value" }}
      ]
    }}
  }},
  "entrypoints": {{
    ".": {{ "exports": {{ "function-1": ["readCount"] }} }}
  }},
  "evidence": {{
    "kind": "reviewed",
    "generator": "application developer"
  }}
}}
"#
        ),
    )
    .unwrap();

    let check = |arguments: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
            .env("SOLID_TYPEFACTS_BIN", &typefacts)
            .args(arguments)
            .arg(directory.join("tsconfig.json"))
            .output()
            .unwrap()
    };
    let status = || {
        let result = check(&["--format", "json", "--check-contracts", "--project"]);
        let report: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
        report["packages"][0].clone()
    };
    let write_lockfile = |integrity: &str| {
        fs::write(
            directory.join("package-lock.json"),
            format!(
                r#"{{
  "name": "app",
  "lockfileVersion": 3,
  "packages": {{
    "": {{ "name": "app" }},
    "node_modules/reactive-package": {{
      "version": "1.0.0",
      "resolved": "https://registry.npmjs.org/reactive-package/-/reactive-package-1.0.0.tgz",
      "integrity": "{integrity}"
    }}
  }}
}}
"#
            ),
        )
        .unwrap();
    };

    // 1. No lockfile at all -- pnpm, Yarn, or no lock. There is no installed
    //    integrity to recover, so behavior is exactly what it was: the
    //    contract applies on version identity.
    let package_status = status();
    assert_eq!(package_status["status"], "local", "{package_status}");
    assert!(package_status["detail"].is_null(), "{package_status}");

    // 2. A lockfile that records the audited bytes. Nothing changes, and this
    //    is the case that proves the check is not simply refusing every
    //    contract that carries an integrity.
    write_lockfile(AUDITED);
    let package_status = status();
    assert_eq!(package_status["status"], "local", "{package_status}");
    assert!(package_status["detail"].is_null(), "{package_status}");

    // 3. Same version, different bytes.
    write_lockfile(INSTALLED);
    let refused = check(&["--format", "json", "--check-contracts", "--project"]);
    assert_eq!(refused.status.code(), Some(1));
    let report: serde_json::Value = serde_json::from_slice(&refused.stdout).unwrap();
    assert_eq!(report["packages"][0]["status"], "stale");
    assert_eq!(report["stale"], 1);
    let detail = report["packages"][0]["detail"].as_str().unwrap();
    // Both integrities, because the versions agree: a message naming only the
    // versions here would read as a contradiction.
    assert!(detail.contains(AUDITED), "{detail}");
    assert!(detail.contains(INSTALLED), "{detail}");
    let remedy = report["packages"][0]["remedy"].as_str().unwrap();
    assert!(
        remedy.contains("solid-checker contract generate"),
        "{remedy}"
    );

    // The analysis refuses the contract the documented way: fail closed on the
    // contract, not on the run.
    let analysis = check(&["--format", "json", "--certify", "--project"]);
    assert_eq!(analysis.status.code(), Some(1));
    let snapshot: serde_json::Value = serde_json::from_slice(&analysis.stdout).unwrap();
    assert_eq!(snapshot["status"], "uncertifiable");
    let finding = &snapshot["findings"][0];
    assert_eq!(finding["id"], "SC9005");
    assert_eq!(finding["rule"], "package-contract-incomplete");
    let message = finding["message"].as_str().unwrap();
    assert!(message.contains(AUDITED), "{message}");
    assert!(message.contains(INSTALLED), "{message}");
    assert!(
        finding["primaryLocation"]["path"]
            .as_str()
            .is_some_and(|path| path.ends_with("App.tsx")),
        "the finding anchors at the import, not at the project root"
    );

    // 4. A lockfile whose entry names no tarball -- a workspace link. There is
    //    no installed integrity to compare, so the ambiguity resolves to no
    //    enforcement rather than to a verdict in either direction.
    fs::write(
        directory.join("package-lock.json"),
        r#"{
  "name": "app",
  "lockfileVersion": 3,
  "packages": {
    "": { "name": "app" },
    "node_modules/reactive-package": { "resolved": "packages/reactive-package", "link": true }
  }
}
"#,
    )
    .unwrap();
    let package_status = status();
    assert_eq!(package_status["status"], "local", "{package_status}");
    fs::remove_dir_all(directory).unwrap();
}

/// One minimal contract for `package`: a single export whose callback claim
/// carries an argument descriptor a by-name callback cannot bind, so a bound
/// contract raises exactly one `SC9005` and a refused one raises none.
fn identity_probe_contract(package: &str, entrypoint: &str) -> String {
    format!(
        "{{\"schemaVersion\":1,\"package\":{{\"name\":\"{package}\",\"version\":\"1.0.0\"}},\
         \"compilerFactsProtocol\":1,\
         \"summaries\":{{\"map-value\":{{\"kind\":\"function\",\"callbacks\":[{{\"parameter\":0,\
         \"execution\":\"inline\",\"arguments\":[null,{{\"kind\":\"accessor\",\"label\":\"item\"}}]}}]}}}},\
         \"entrypoints\":{{\"{entrypoint}\":{{\"exports\":{{\"map-value\":[\"mapValue\"]}}}}}},\
         \"evidence\":{{\"kind\":\"reviewed\"}}}}\n"
    )
}

const IDENTITY_PROBE_DECLARATION: &str = "export declare function mapValue(\n  \
     map: (index: number, item: () => number) => unknown\n): void;\n";

const IDENTITY_PROBE_CONSUMER: &str = "import { mapValue } from \"linked-package\";\nfunction named(index: number, item: () => number) {\n  \
     return item();\n}\nexport function use() {\n  mapValue(named);\n}\n";

/// A workspace- or pnpm-linked install still binds its contract.
///
/// Contract discovery walks `node_modules/<name>` and finds the link; the
/// compiler reports the resolution's realpath, which is the *target*. Comparing
/// those two spellings directly would refuse every linked install, so the
/// classified directory is compared in both spellings. This cannot be a
/// committed fixture -- it would be this repository's first committed symlink,
/// and a Windows checkout materializes one as a plain file -- so it is built
/// against the real producer here.
#[cfg(unix)]
#[test]
fn a_linked_install_binds_its_contract_through_the_realpath() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let directory = temporary_directory("linked-install");
    let real = directory.join("packages/linked-package");
    fs::create_dir_all(&real).unwrap();
    fs::write(
        real.join("package.json"),
        "{\"name\":\"linked-package\",\"version\":\"1.0.0\",\"types\":\"index.d.ts\"}\n",
    )
    .unwrap();
    fs::write(real.join("index.d.ts"), IDENTITY_PROBE_DECLARATION).unwrap();
    fs::write(
        real.join("solid-reactivity.json"),
        identity_probe_contract("linked-package", "."),
    )
    .unwrap();
    fs::create_dir_all(directory.join("node_modules")).unwrap();
    std::os::unix::fs::symlink(
        Path::new("../packages/linked-package"),
        directory.join("node_modules/linked-package"),
    )
    .unwrap();
    fs::write(directory.join("App.ts"), IDENTITY_PROBE_CONSUMER).unwrap();
    let project = directory.join("tsconfig.json");
    fs::write(
        &project,
        "{\"compilerOptions\":{\"module\":\"ESNext\",\"moduleResolution\":\"Bundler\",\
         \"strict\":true,\"target\":\"ES2022\"},\"include\":[\"App.ts\"]}\n",
    )
    .unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
        .env("SOLID_TYPEFACTS_BIN", &typefacts)
        .args(["--project"])
        .arg(&project)
        .args(["--format", "json"])
        .output()
        .unwrap();
    let findings = decode_findings(&result.stdout);
    assert_eq!(
        findings
            .iter()
            .filter(|finding| finding["id"] == "SC9005")
            .count(),
        1,
        "{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}

/// The identity attestation is scoped to the files that carry a bare specifier,
/// and its answer covers every one of them.
///
/// The module-graph operation answers the whole program's file inventory
/// unconditionally, so a program in which no specifier could name a package
/// must not pay for one at all. And a requested file the program does not hold
/// refuses every specifier in it, which would be a plumbing defect rather than
/// a project property -- so it is counted rather than left silent.
#[test]
fn the_identity_attestation_is_scoped_to_files_a_contract_could_bind_in() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let directory = temporary_directory("identity-scope");
    let package = directory.join("node_modules/contracted-package");
    fs::create_dir_all(&package).unwrap();
    fs::write(
        package.join("package.json"),
        "{\"name\":\"contracted-package\",\"version\":\"1.0.0\",\"types\":\"index.d.ts\"}\n",
    )
    .unwrap();
    fs::write(package.join("index.d.ts"), IDENTITY_PROBE_DECLARATION).unwrap();
    fs::write(
        package.join("solid-reactivity.json"),
        identity_probe_contract("contracted-package", "."),
    )
    .unwrap();
    let plain = directory.join("node_modules/plain-package");
    fs::create_dir_all(&plain).unwrap();
    fs::write(
        plain.join("package.json"),
        "{\"name\":\"plain-package\",\"version\":\"1.0.0\",\"types\":\"index.d.ts\"}\n",
    )
    .unwrap();
    fs::write(plain.join("index.d.ts"), IDENTITY_PROBE_DECLARATION).unwrap();
    fs::write(
        directory.join("Contracted.ts"),
        "import { mapValue } from \"contracted-package\";\nexport const use = () => mapValue(() => 1);\n",
    )
    .unwrap();
    fs::write(
        directory.join("Plain.ts"),
        "import { mapValue } from \"plain-package\";\nexport const use = () => mapValue(() => 1);\n",
    )
    .unwrap();
    fs::write(directory.join("Local.ts"), "export const local = 1;\n").unwrap();
    let timings = |files: &[&str]| {
        let project = directory.join(format!("tsconfig-{}.json", files.join("-")));
        fs::write(
            &project,
            format!(
                "{{\"compilerOptions\":{{\"module\":\"ESNext\",\"moduleResolution\":\"Bundler\",\
                 \"strict\":true,\"target\":\"ES2022\"}},\"files\":{}}}\n",
                serde_json::to_string(files).unwrap()
            ),
        )
        .unwrap();
        let result = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
            .env("SOLID_TYPEFACTS_BIN", &typefacts)
            .env("SOLID_CHECKER_TIMINGS", "1")
            .args(["--project"])
            .arg(&project)
            .args(["--format", "json"])
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&result.stderr).into_owned();
        let line = stderr
            .lines()
            .rev()
            .find(|line| line.contains("importIdentityFilesRequested"))
            .unwrap_or_else(|| panic!("no timings line in {stderr}"))
            .to_owned();
        serde_json::from_str::<serde_json::Value>(&line).expect("timings json")
    };

    // No specifier in the program could name a package, so nothing is asked
    // for at all -- not even the inventory the operation always answers.
    let none = timings(&["Local.ts"]);
    assert_eq!(none["importIdentityFilesRequested"], 0);
    assert_eq!(none["importIdentityModules"], 0);

    // Two files carry a bare specifier and one carries none. The two are asked
    // about, every requested file is answered, and each answer covers its one
    // specifier. `Plain.ts` is in scope even though no contract exists for the
    // package it names: the scope is keyed on the program, never on today's
    // contract discovery, because a contract that appears later must not find
    // its files silently unanswered.
    let scoped = timings(&["Contracted.ts", "Plain.ts", "Local.ts"]);
    assert_eq!(scoped["importIdentityFilesRequested"], 2);
    assert_eq!(scoped["importIdentityFilesAttested"], 2);
    assert_eq!(scoped["importIdentityFilesUnknown"], 0);
    assert_eq!(scoped["importIdentitySpecifiers"], 2);
    // The inventory half is the whole program and is not scoped: it is the
    // operation's reason to exist.
    assert!(scoped["importIdentityModules"].as_u64().unwrap() >= 3);
}

/// A contract every import refuses is reported as `unbound`, not as coverage.
///
/// `contract check` exists to answer "is my contract coverage complete?", and
/// loading a contract is only half of that: this project's `paths` entry owns
/// the one specifier carrying the contract's name, so the contract describes
/// nothing here. The report used to say `published` and `missing: 0` about
/// exactly this project while the analysis refused the contract at every
/// import -- the command's answer and the analysis's behavior disagreeing in
/// silence. The refusal stays silent in the *findings* by design, and the same
/// run's timings count it.
#[test]
fn a_contract_no_import_binds_is_reported_as_unbound() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let directory = temporary_directory("unbound-contract");
    let package = directory.join("node_modules/reactive-package");
    fs::create_dir_all(&package).unwrap();
    fs::write(
        package.join("package.json"),
        "{\"name\":\"reactive-package\",\"version\":\"1.0.0\",\"types\":\"index.d.ts\",\
         \"peerDependencies\":{\"solid-js\":\"^2.0.0\"}}\n",
    )
    .unwrap();
    fs::write(package.join("index.d.ts"), IDENTITY_PROBE_DECLARATION).unwrap();
    fs::write(
        package.join("solid-reactivity.json"),
        identity_probe_contract("reactive-package", "."),
    )
    .unwrap();
    fs::create_dir_all(directory.join("src")).unwrap();
    fs::write(
        directory.join("src/local-impl.ts"),
        "export function mapValue(\n  map: (index: number, item: () => number) => unknown\n\
         ): void {\n  setTimeout(() => map(0, () => 1), 0);\n}\n",
    )
    .unwrap();
    fs::write(
        directory.join("App.ts"),
        "import { mapValue } from \"reactive-package\";\n\
         function named(index: number, item: () => number) {\n  return item();\n}\n\
         export function use() {\n  mapValue(named);\n}\n",
    )
    .unwrap();
    let project = directory.join("tsconfig.json");
    fs::write(
        &project,
        "{\"compilerOptions\":{\"baseUrl\":\".\",\"module\":\"ESNext\",\
         \"moduleResolution\":\"Bundler\",\"strict\":true,\"target\":\"ES2022\",\
         \"paths\":{\"reactive-package\":[\"./src/local-impl\"]}},\
         \"include\":[\"*.ts\",\"src/*.ts\"]}\n",
    )
    .unwrap();

    let report = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
        .env("SOLID_TYPEFACTS_BIN", &typefacts)
        .args(["--format", "json", "--check-contracts", "--project"])
        .arg(&project)
        .output()
        .unwrap();
    let decoded: serde_json::Value = serde_json::from_slice(&report.stdout).unwrap();
    assert_eq!(decoded["packages"][0]["status"], "unbound");
    assert_eq!(decoded["missing"], 1);
    // Not drift: the contract describes the version that is installed.
    assert_eq!(decoded["stale"], 0);
    assert_eq!(report.status.code(), Some(1));
    let remedy = decoded["packages"][0]["remedy"].as_str().unwrap();
    assert!(remedy.contains("tsconfig path mapping"), "{remedy}");
    assert!(!remedy.contains("contract generate"), "{remedy}");

    // The analysis raises nothing for the refusal, and counts it.
    let analysis = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
        .env("SOLID_TYPEFACTS_BIN", &typefacts)
        .env("SOLID_CHECKER_TIMINGS", "1")
        .args(["--format", "json", "--project"])
        .arg(&project)
        .output()
        .unwrap();
    assert_eq!(
        decode_findings(&analysis.stdout),
        Vec::<serde_json::Value>::new()
    );
    let stderr = String::from_utf8_lossy(&analysis.stderr).into_owned();
    let timings: serde_json::Value = serde_json::from_str(
        stderr
            .lines()
            .rev()
            .find(|line| line.contains("contractBindingsRefused"))
            .unwrap_or_else(|| panic!("no timings line in {stderr}")),
    )
    .unwrap();
    assert_eq!(timings["contractBindingsRefused"], 1);
    assert_eq!(timings["contractBindingsBound"], 0);
    fs::remove_dir_all(directory).unwrap();
}

/// The same project with the `paths` entry removed: the contract binds, the
/// report says `published`, and the counts swap. Without this control an
/// `unbound` verdict could come from binding being broken outright.
#[test]
fn the_same_contract_without_the_path_mapping_binds_and_reports_published() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let directory = temporary_directory("bound-contract");
    let package = directory.join("node_modules/reactive-package");
    fs::create_dir_all(&package).unwrap();
    fs::write(
        package.join("package.json"),
        "{\"name\":\"reactive-package\",\"version\":\"1.0.0\",\"types\":\"index.d.ts\",\
         \"peerDependencies\":{\"solid-js\":\"^2.0.0\"}}\n",
    )
    .unwrap();
    fs::write(package.join("index.d.ts"), IDENTITY_PROBE_DECLARATION).unwrap();
    fs::write(
        package.join("solid-reactivity.json"),
        identity_probe_contract("reactive-package", "."),
    )
    .unwrap();
    fs::write(
        directory.join("App.ts"),
        "import { mapValue } from \"reactive-package\";\n\
         function named(index: number, item: () => number) {\n  return item();\n}\n\
         export function use() {\n  mapValue(named);\n}\n",
    )
    .unwrap();
    let project = directory.join("tsconfig.json");
    fs::write(
        &project,
        "{\"compilerOptions\":{\"module\":\"ESNext\",\"moduleResolution\":\"Bundler\",\
         \"strict\":true,\"target\":\"ES2022\"},\"include\":[\"App.ts\"]}\n",
    )
    .unwrap();

    let report = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
        .env("SOLID_TYPEFACTS_BIN", &typefacts)
        .args(["--format", "json", "--check-contracts", "--project"])
        .arg(&project)
        .output()
        .unwrap();
    let decoded: serde_json::Value = serde_json::from_slice(&report.stdout).unwrap();
    assert_eq!(decoded["packages"][0]["status"], "published");
    assert_eq!(decoded["missing"], 0);
    assert_eq!(report.status.code(), Some(0));

    let analysis = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
        .env("SOLID_TYPEFACTS_BIN", &typefacts)
        .env("SOLID_CHECKER_TIMINGS", "1")
        .args(["--format", "json", "--project"])
        .arg(&project)
        .output()
        .unwrap();
    let findings = decode_findings(&analysis.stdout);
    assert_eq!(
        findings
            .iter()
            .filter(|finding| finding["id"] == "SC9005")
            .count(),
        1
    );
    let stderr = String::from_utf8_lossy(&analysis.stderr).into_owned();
    let timings: serde_json::Value = serde_json::from_str(
        stderr
            .lines()
            .rev()
            .find(|line| line.contains("contractBindingsRefused"))
            .unwrap_or_else(|| panic!("no timings line in {stderr}")),
    )
    .unwrap();
    assert_eq!(timings["contractBindingsBound"], 1);
    assert_eq!(timings["contractBindingsRefused"], 0);
    fs::remove_dir_all(directory).unwrap();
}

/// A project whose only mention of a contracted package is `import type` keeps
/// the tier that supplied the contract.
///
/// Zero bindings is not by itself a complaint: a type-only declaration carries
/// no bindable specifier — contract resolution skips it exactly as analysis
/// does — so there is nothing for the contract to describe and nothing wrong
/// with the contract. `unbound` requires a *refusal*, which is the state where
/// a bindable specifier exists and resolves somewhere the contract's package is
/// not.
#[test]
fn a_type_only_import_of_a_contracted_package_is_not_unbound() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let directory = temporary_directory("type-only-contract");
    let package = directory.join("node_modules/reactive-package");
    fs::create_dir_all(&package).unwrap();
    fs::write(
        package.join("package.json"),
        "{\"name\":\"reactive-package\",\"version\":\"1.0.0\",\"types\":\"index.d.ts\",\
         \"peerDependencies\":{\"solid-js\":\"^2.0.0\"}}\n",
    )
    .unwrap();
    fs::write(
        package.join("index.d.ts"),
        "export declare function mapValue(\n  map: (index: number, item: () => number) => unknown\n\
         ): void;\nexport type MapCallback = (index: number, item: () => number) => unknown;\n",
    )
    .unwrap();
    fs::write(
        package.join("solid-reactivity.json"),
        identity_probe_contract("reactive-package", "."),
    )
    .unwrap();
    fs::write(
        directory.join("App.ts"),
        "import type { MapCallback } from \"reactive-package\";\n\
         export const named: MapCallback = (index, item) => item();\n",
    )
    .unwrap();
    let project = directory.join("tsconfig.json");
    fs::write(
        &project,
        "{\"compilerOptions\":{\"module\":\"ESNext\",\"moduleResolution\":\"Bundler\",\
         \"strict\":true,\"target\":\"ES2022\"},\"include\":[\"App.ts\"]}\n",
    )
    .unwrap();
    let report = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
        .env("SOLID_TYPEFACTS_BIN", &typefacts)
        .args(["--format", "json", "--check-contracts", "--project"])
        .arg(&project)
        .output()
        .unwrap();
    let decoded: serde_json::Value = serde_json::from_slice(&report.stdout).unwrap();
    assert_eq!(decoded["packages"][0]["status"], "published");
    assert_eq!(decoded["missing"], 0);
    assert_eq!(report.status.code(), Some(0));
    fs::remove_dir_all(directory).unwrap();
}
