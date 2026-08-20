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
    for (fixture, rule, message) in [
        ("package-consumer", "strict-read-untracked", "readCount"),
        (
            "package-return-consumer",
            "strict-read-untracked",
            "created count",
        ),
        (
            "package-callback-consumer",
            "strict-read-untracked",
            "runMixed",
        ),
        (
            "package-store-consumer",
            "strict-read-untracked",
            "state.value",
        ),
        (
            "package-store-destructure",
            "component-props-destructure",
            "destructuring",
        ),
        (
            "package-unknown-export",
            "package-contract-incomplete",
            "unknownPrimitive",
        ),
        ("bundled-solid-consumer", "strict-read-untracked", "doubled"),
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
        assert_eq!(findings.len(), 1, "fixture {fixture}: {findings:#?}");
        assert_eq!(findings[0]["rule"], rule, "fixture {fixture}");
        assert!(
            findings[0]["message"]
                .as_str()
                .is_some_and(|finding| finding.contains(message))
        );
    }
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
        .args(["--format", "json", "--project"])
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
        .args(["--format", "json", "--project"])
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
        .args(["--format", "json", "--dialect", "solid-v1", "--project"])
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
    "function": { "kind": "function" }
  },
  "entrypoints": {
    ".": {
      "exports": {
        "function": ["readCount"]
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
        "function-1": ["readCount"]
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
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0]["rule"], "strict-read-untracked");

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
        "function-1": ["readCount"]
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
    assert_eq!(conditional_findings.len(), 1);
    assert_eq!(conditional_findings[0]["id"], "SC9005");
    assert!(
        conditional_findings[0]["message"]
            .as_str()
            .is_some_and(|message| message.contains("conditional runtime targets"))
    );

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

#[test]
fn cli_refuses_to_emit_unknown_callback_execution() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let directory = temporary_directory("emit-unknown-callback");
    let result = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
        .env("SOLID_TYPEFACTS_BIN", &typefacts)
        .args(["--project"])
        .arg(root.join("fixtures/reactive-ir/package-unknown-callback-producer/tsconfig.json"))
        .args(["--emit-contract"])
        .arg(directory.join("solid-reactivity.json"))
        .args([
            "--package-name",
            "callback-package",
            "--package-version",
            "1.0.0",
        ])
        .output()
        .unwrap();
    assert!(!result.status.success());
    let stderr = String::from_utf8_lossy(&result.stderr);
    // The refusal comes from the obligation list, which knows the exported
    // surface, so it names both ends of the uncertifiable edge: the export
    // whose parameter escapes and the callee whose timing is unknown.
    assert!(stderr.contains("unresolved parameter behavior"), "{stderr}");
    assert!(stderr.contains("schedule"), "{stderr}");
    assert!(stderr.contains("unknownScheduler"), "{stderr}");
    assert!(stderr.contains("() => void"), "{stderr}");
    assert!(stderr.contains("schemaVersion"), "{stderr}");
    fs::remove_dir_all(directory).unwrap();
}

/// SC9-class obligations arrive as structured defects since the contract
/// resolver moved missing exports off `static_violations`; a contract must
/// not be written over them either. `package-unknown-export` reports SC9005,
/// so emission over it has to refuse.
#[test]
fn cli_refuses_to_emit_over_unresolved_obligations() {
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
    assert!(!result.status.success(), "emission must refuse over SC9005");
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(stderr.contains("unresolved obligation"), "{stderr}");
    assert!(!output.exists(), "no contract may be written over SC9005");
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
    let mut child =
        Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
            .env("SOLID_TYPEFACTS_BIN", &typefacts)
            .args(["--project"])
            .arg(root.join(
                "fixtures/reactive-ir/package-cyclic-unknown-callback-producer/tsconfig.json",
            ))
            .args(["--emit-contract"])
            .arg(directory.join("solid-reactivity.json"))
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
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("unresolved parameter behavior"), "{stderr}");
    assert!(stderr.contains("unknownScheduler"), "{stderr}");
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
    assert!(!broken.status.success());
    let stderr = String::from_utf8_lossy(&broken.stderr);
    assert!(stderr.contains("hiddenScheduler"), "{stderr}");
    assert!(stderr.contains("unknownScheduler"), "{stderr}");
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
fn package_generator_conservatively_merges_conditional_targets() {
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
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn package_generator_preserves_conditional_callback_execution_modes() {
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
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let contract = expanded_contract(&output);
    let callbacks = contract["entrypoints"]["."]["exports"]["schedule"]["callbacks"]
        .as_array()
        .unwrap();
    assert_eq!(callbacks.len(), 2);
    assert_eq!(callbacks[0]["parameter"], 0);
    assert_eq!(callbacks[0]["execution"], "deferred");
    assert_eq!(callbacks[1]["parameter"], 0);
    assert_eq!(callbacks[1]["execution"], "inline");
    let variants = contract["entrypoints"]["."]["exports"]["schedule"]["variants"]
        .as_array()
        .unwrap();
    assert_eq!(variants.len(), 2);
    let development = variants
        .iter()
        .find(|variant| variant["conditions"] == serde_json::json!(["development"]))
        .unwrap();
    assert_eq!(
        development["summary"]["callbacks"][0]["execution"],
        "inline"
    );
    let production = variants
        .iter()
        .find(|variant| variant["conditions"] == serde_json::json!(["default"]))
        .unwrap();
    assert_eq!(
        production["summary"]["callbacks"][0]["execution"],
        "deferred"
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
fn package_generator_expands_external_export_all_from_dependency_contracts() {
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
        .arg("--contract")
        .arg(package.join("dependency-contract.json"))
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
    assert_eq!(
        without_claim_evidence(&contract["entrypoints"]["."]["exports"]["forward"]["callbacks"]),
        serde_json::json!([{ "parameter": 0, "execution": "inline" }])
    );
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
