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
            "runMixed",
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
fn bundled_contract_resolves_the_exact_web_subpath() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
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
    assert_eq!(report["packages"][0]["status"], "missing");
    fs::remove_dir_all(directory).unwrap();
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
        contract["entrypoints"]["."]["exports"]["createWrappedCount"]["callbacks"],
        serde_json::json!([{ "parameter": 0, "execution": "tracked" }])
    );
    assert_eq!(
        contract["entrypoints"]["."]["exports"]["createTransitivelyWrapped"]["callbacks"],
        serde_json::json!([{ "parameter": 0, "execution": "tracked" }])
    );
    assert_eq!(
        contract["entrypoints"]["."]["exports"]["listen"]["callbacks"],
        serde_json::json!([{ "parameter": 1, "execution": "deferred" }])
    );
    assert_eq!(
        contract["entrypoints"]["."]["exports"]["configureDeferredMethod"]["callbacks"],
        serde_json::json!([{ "parameter": 1, "execution": "deferred" }])
    );
    assert_eq!(
        contract["entrypoints"]["."]["exports"]["createDeferredProxy"]["callbacks"],
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
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
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
    assert!(stderr.contains("unresolved parameter behavior"), "{stderr}");
    assert!(stderr.contains("unknownScheduler"), "{stderr}");
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn cli_does_not_treat_noncallback_parameters_as_callback_obligations() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
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
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
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
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
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
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
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
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
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
fn package_generator_follows_runtime_esm_behind_declarations() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
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
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
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
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
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
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn package_generator_expands_external_export_all_from_dependency_contracts() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
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
        contract["entrypoints"]["."]["exports"]["forward"]["callbacks"],
        serde_json::json!([{ "parameter": 0, "execution": "inline" }])
    );
    fs::remove_dir_all(directory).unwrap();
}
