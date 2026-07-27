#[path = "support/process.rs"]
mod support;

use std::{env, fs, path::PathBuf, process::Command};

use support::{decode_findings, temporary_directory};

#[test]
fn cli_consumes_discovered_package_contracts() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
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
            "runInline",
        ),
        (
            "package-store-consumer",
            "strict-read-untracked",
            "state.value",
        ),
        (
            "package-unknown-export",
            "package-contract-export-missing",
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
fn cli_validates_a_contract_without_opening_a_project() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
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
fn cli_reports_missing_contracts_and_loads_project_owned_overrides() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
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
    assert_eq!(snapshot["findings"][0]["rule"], "package-contract-missing");
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
  "artifacts": {},
  "exports": {
    "readCount": {
      "kind": "function",
      "reactiveReads": [
        {
          "kind": "accessor",
          "label": "project-owned reactive value"
        }
      ]
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

    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn cli_emits_and_revalidates_package_contracts() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
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
    let contract: serde_json::Value = serde_json::from_slice(&fs::read(&output).unwrap()).unwrap();
    for name in [
        "createCount",
        "createAliasedCount",
        "createArrowCount",
        "createMemoCount",
    ] {
        assert_eq!(contract["exports"][name]["returns"]["kind"], "accessor");
    }
    assert_eq!(
        contract["exports"]["createState"]["returns"]["kind"],
        "store-path"
    );
    assert_eq!(contract["exports"]["packageVersion"]["kind"], "value");
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
