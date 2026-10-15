#[path = "support/diagnostics.rs"]
mod support;

// ADR 0296: retained dependency-source checks against one-shot ones.
#[path = "dependency_daemon_process.rs"]
mod dependency_daemon_process;

// ADR 0311: a dependency-source check is a fresh one-shot check.
#[path = "dependency_fresh_native_process.rs"]
mod dependency_fresh_native_process;

use support::{assert_rule_findings, diagnostic_fixture, findings_for_rule};

#[test]
fn authored_tuple_iteration_mutations_withdraw_returned_member_effects() {
    use solid_facts_backend::fixture_authorization::{
        authorize_fixture_contract, read_fixture_contract_request,
    };
    use std::{fs, path::Path, process::Command};
    let Ok(typefacts) = std::env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    fn copy(from: &Path, to: &Path) {
        fs::create_dir_all(to).unwrap();
        for entry in fs::read_dir(from).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_dir() {
                copy(&entry.path(), &to.join(entry.file_name()));
            } else {
                fs::copy(entry.path(), to.join(entry.file_name())).unwrap();
            }
        }
    }
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let scratch = std::path::PathBuf::from("/tmp/sc-scratch.noindex")
        .join(format!("tuple-contract-{}", std::process::id()));
    copy(
        &root.join("fixtures/reactive-ir/tuple-contract-iteration-veto"),
        &scratch,
    );
    let project = fs::canonicalize(&scratch).unwrap();
    let authorization =
        authorize_fixture_contract(&project, &read_fixture_contract_request(&project).unwrap())
            .unwrap();
    let trust = project.join("trust.json");
    fs::write(&trust, authorization.trust_configuration).unwrap();
    let mutation = fs::read_to_string(project.join("mutations.ts")).unwrap();
    for (source, kind, code) in [
        ("export {};", "violation", "SC1001"),
        (mutation.as_str(), "uncertifiable", "SC9012"),
        ("export {};", "violation", "SC1001"),
    ] {
        fs::write(project.join("mutations.ts"), source).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
            .env("SOLID_TYPEFACTS_BIN", &typefacts)
            .env("SOLID_CHECKER_DAEMON", "0")
            .args(["--format", "json", "--project"])
            .arg(project.join("tsconfig.json"))
            .arg("--receipt-trust-configuration")
            .arg(&trust)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let snapshot: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        let rows = snapshot["findings"].as_array().unwrap();
        assert_eq!(rows.len(), 1, "{rows:#?}");
        assert_eq!(rows[0]["kind"], kind, "{rows:#?}");
        assert_eq!(rows[0]["id"], code, "{rows:#?}");
    }
    fs::remove_dir_all(scratch).unwrap();
}

#[test]
fn returned_tuple_iteration_mutations_withdraw_only_the_proven_read() {
    use std::{fs, process::Command};
    let Ok(typefacts) = std::env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let fixture = root.join("fixtures/reactive-ir/tuple-iteration-veto");
    let scratch = std::path::PathBuf::from("/tmp/sc-scratch.noindex")
        .join(format!("tuple-iteration-{}", std::process::id()));
    for package in ["solid-js", "@solidjs/signals", "@solidjs/web"] {
        fs::create_dir_all(scratch.join("node_modules").join(package)).unwrap();
    }
    for name in [
        "App.tsx",
        "factory.ts",
        "gateways.ts",
        "tsconfig.json",
        "solid-js.d.ts",
        "node_modules/solid-js/package.json",
        "node_modules/@solidjs/signals/package.json",
        "node_modules/@solidjs/web/package.json",
    ] {
        fs::copy(fixture.join(name), scratch.join(name)).unwrap();
    }
    let cases: serde_json::Value =
        serde_json::from_slice(&fs::read(fixture.join("cases.json")).unwrap()).unwrap();
    for case in cases.as_array().unwrap() {
        fs::write(
            scratch.join("mutations.ts"),
            format!("{}\nexport {{}};\n", case["source"].as_str().unwrap()),
        )
        .unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
            .env("SOLID_TYPEFACTS_BIN", &typefacts)
            .env("SOLID_CHECKER_DAEMON", "0")
            .args(["--format", "json", "--project"])
            .arg(scratch.join("tsconfig.json"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}: {}",
            case["name"],
            String::from_utf8_lossy(&output.stderr)
        );
        let snapshot: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        let rows = snapshot["findings"].as_array().unwrap();
        let read = rows
            .iter()
            .filter(|row| {
                row["primaryLocation"]["path"]
                    .as_str()
                    .is_some_and(|path| path.ends_with("App.tsx"))
            })
            .collect::<Vec<_>>();
        assert_eq!(read.len(), 2, "{}: {rows:#?}", case["name"]);
        let (kind, code) = if case["veto"] == true {
            ("uncertifiable", "SC9012")
        } else {
            ("violation", "SC1001")
        };
        assert_eq!(read[0]["kind"], kind, "{}: {read:#?}", case["name"]);
        assert_eq!(read[0]["id"], code, "{}: {read:#?}", case["name"]);
        assert_eq!(read[1]["kind"], kind, "{}: {read:#?}", case["name"]);
        assert_eq!(read[1]["id"], code, "{}: {read:#?}", case["name"]);
        assert!(
            rows.iter()
                .all(|row| row["kind"] != "violation" || !case["veto"].as_bool().unwrap()),
            "{}: {rows:#?}",
            case["name"]
        );
    }
    fs::remove_dir_all(scratch).unwrap();
}

#[test]
fn returned_store_tuple_veto_preserves_uncertifiable_candidates() {
    use std::process::Command;
    let Ok(typefacts) = std::env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    for (fixture, expected, kind, code, flags) in [
        (
            "returned-store-proxy",
            4,
            "uncertifiable",
            "SC9012",
            Vec::new(),
        ),
        (
            "dependency-source-proxy-store",
            1,
            "violation",
            "SC1001",
            vec![
                "--runtime-target",
                "browser",
                "--runtime-build",
                "development",
                "--dependency-source",
                "dep-query",
            ],
        ),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
            .env("SOLID_TYPEFACTS_BIN", &typefacts)
            .env("SOLID_CHECKER_DAEMON", "0")
            .args(["--format", "json", "--project"])
            .arg(root.join(format!("fixtures/reactive-ir/{fixture}/tsconfig.json")))
            .args(flags)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let snapshot: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        let rows = snapshot["findings"].as_array().unwrap();
        let reads = rows
            .iter()
            .filter(|row| {
                row["primaryLocation"]["path"]
                    .as_str()
                    .is_some_and(|path| path.ends_with("App.tsx"))
            })
            .collect::<Vec<_>>();
        assert_eq!(reads.len(), expected, "{fixture}: {rows:#?}");
        assert!(
            reads
                .iter()
                .all(|row| row["kind"] == kind && row["id"] == code),
            "{fixture}: {rows:#?}"
        );
    }
}

#[test]
fn discovered_tsconfig_package_shadowing_cannot_create_guarded_violation() {
    use solid_facts_backend::fixture_authorization::{
        authorize_fixture_contract, read_fixture_contract_request,
    };
    use std::{fs, path::Path, process::Command};
    fn copy(from: &Path, to: &Path) {
        fs::create_dir_all(to).unwrap();
        for entry in fs::read_dir(from).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_dir() {
                copy(&entry.path(), &to.join(entry.file_name()));
            } else {
                fs::copy(entry.path(), to.join(entry.file_name())).unwrap();
            }
        }
    }
    let Ok(typefacts) = std::env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let repository = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let scratch =
        std::env::temp_dir().join(format!("tsconfig-shadow-process-{}", std::process::id()));
    copy(
        &repository.join("fixtures/reactive-ir/inferred-host-spa"),
        &scratch.join("app"),
    );
    fs::create_dir_all(scratch.join("shadow-base/@solidjs")).unwrap();
    fs::write(
        scratch.join("shadow-base/@solidjs/web.js"),
        "export const isServer = true;",
    )
    .unwrap();
    let project = fs::canonicalize(scratch.join("app")).unwrap();
    let application_manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(project.join("package.json")).unwrap()).unwrap();
    let mut analyzed_config: serde_json::Value =
        serde_json::from_slice(&fs::read(project.join("tsconfig.json")).unwrap()).unwrap();
    analyzed_config["compilerOptions"]["noEmit"] = true.into();
    analyzed_config["compilerOptions"]["allowImportingTsExtensions"] = true.into();
    fs::write(
        project.join("tsconfig.app.json"),
        serde_json::to_vec(&analyzed_config).unwrap(),
    )
    .unwrap();
    let package = project.join("node_modules/@solidjs/web");
    fs::create_dir_all(package.join("dist")).unwrap();
    fs::write(package.join("package.json"), r#"{"name":"@solidjs/web","version":"2.0.0-rc.13","type":"module","exports":{".":{"types":"./index.d.ts","browser":{"development":"./dist/web.dev.js","observe":"./dist/web.observe.js","default":"./dist/web.js"},"node":{"development":"./dist/server.dev.js","observe":"./dist/server.observe.js","default":"./dist/server.js"},"default":"./dist/web.js"}}}"#).unwrap();
    fs::write(
        package.join("index.d.ts"),
        "export declare const isServer: boolean;",
    )
    .unwrap();
    for (prefix, value) in [("web", false), ("server", true)] {
        for suffix in ["", ".dev", ".observe"] {
            fs::write(
                package.join(format!("dist/{prefix}{suffix}.js")),
                format!("export const isServer = {value};"),
            )
            .unwrap();
        }
    }
    let authorization =
        authorize_fixture_contract(&project, &read_fixture_contract_request(&project).unwrap())
            .unwrap();
    let trust = project.join("trust.json");
    fs::write(&trust, authorization.trust_configuration).unwrap();
    let source = "import {isServer} from '@solidjs/web'; import {startClosed} from 'reactive-package'; import {run} from './selected.ts'; new Date(); if (!isServer) { startClosed(); run(); }";
    fs::write(project.join("src/main.ts"), source).unwrap();
    let helper =
        "import {startClosed} from 'reactive-package'; export function run() { startClosed(); }";
    fs::write(project.join("src/selected.ts"), helper).unwrap();
    fs::write(
        project.join("src/shadow.js"),
        // Both browser-map replacements retain the requested export surface:
        // the local twin links successfully but its run never registers.
        "export const isServer = true; export function run() {}",
    )
    .unwrap();
    fs::write(
        project.join("inherited.json"),
        r#"{"compilerOptions":{"paths":{"@solidjs/web":["./src/shadow.js"]}}}"#,
    )
    .unwrap();
    let mut cases: serde_json::Value =
        serde_json::from_slice(
            &fs::read(repository.join(
                "fixtures/reactive-ir/inferred-host-reachability/resolver-selection-cases.json",
            ))
            .unwrap(),
        )
        .unwrap();
    let conventional: serde_json::Value = serde_json::from_slice(
        &fs::read(repository.join(
            "fixtures/reactive-ir/inferred-host-reachability/conventional-selection-cases.json",
        ))
        .unwrap(),
    )
    .unwrap();
    for mut case in conventional.as_array().unwrap().iter().cloned() {
        case["resolver"] = false.into();
        if case.get("runtime").is_none() {
            case["runtime"] = "absent".into();
        }
        case["tsconfig"] = serde_json::json!({});
        cases.as_array_mut().unwrap().push(case);
    }
    let start = u64::try_from(source.find("startClosed()").unwrap()).unwrap();
    for case in cases.as_array().unwrap() {
        if !cfg!(unix)
            && ["manifestSymlink", "workspaceSymlink", "configSymlink"]
                .iter()
                .any(|key| case[*key] == true)
        {
            continue;
        }
        for path in [
            project.join("package.json"),
            scratch.join("package.json"),
            project.join("vite.config.ts"),
        ] {
            if fs::symlink_metadata(&path).is_ok_and(|metadata| metadata.is_symlink()) {
                fs::remove_file(path).unwrap();
            }
        }
        if let Some(config) = case["outsideConfig"].as_str() {
            fs::write(scratch.join("other.ts"), config).unwrap();
        }
        let parent_config = scratch.join("vite.config.ts");
        if let Some(config) = case["parentConfig"].as_str() {
            fs::write(&parent_config, config).unwrap();
        } else if parent_config.exists() {
            fs::remove_file(&parent_config).unwrap();
        }
        let mut app_manifest = application_manifest.clone();
        if let Some(scripts) = case.get("scripts") {
            app_manifest["scripts"] = scripts.clone();
        }
        if let Some(browser) = case.get("browserMap") {
            app_manifest["browser"] = browser.clone();
        }
        fs::write(
            project.join("package.json"),
            serde_json::to_vec(&app_manifest).unwrap(),
        )
        .unwrap();
        let mut workspace = serde_json::json!({"private":true});
        if let Some(scripts) = case.get("workspaceScripts") {
            workspace["scripts"] = scripts.clone();
        }
        if let Some(browser) = case.get("workspaceBrowser") {
            workspace["browser"] = browser.clone();
        }
        fs::write(
            scratch.join("package.json"),
            serde_json::to_vec(&workspace).unwrap(),
        )
        .unwrap();
        if let Some(name) = case["configFile"].as_str() {
            fs::write(
                project.join(name),
                "export default {plugins:[],resolve:{tsconfigPaths:true}};",
            )
            .unwrap();
        }
        let expected = case["browser"].as_bool().unwrap();
        fs::write(
            project.join("vite.config.ts"),
            if let Some(source) = case["config"].as_str() {
                source
            } else if case["resolver"] == false {
                "export default {plugins:[]};"
            } else {
                "export default {plugins:[],resolve:{tsconfigPaths:true}};"
            },
        )
        .unwrap();
        fs::write(
            project.join("tsconfig.json"),
            serde_json::to_vec(&case["tsconfig"]).unwrap(),
        )
        .unwrap();
        #[cfg(unix)]
        for (key, path, target) in [
            (
                "manifestSymlink",
                project.join("package.json"),
                scratch.join("app-manifest.json"),
            ),
            (
                "workspaceSymlink",
                scratch.join("package.json"),
                scratch.join("workspace-manifest.json"),
            ),
            (
                "configSymlink",
                project.join("vite.config.ts"),
                scratch.join("linked-config.ts"),
            ),
        ] {
            if case[key] == true {
                fs::rename(&path, &target).unwrap();
                std::os::unix::fs::symlink(target, path).unwrap();
            }
        }
        let mut command = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"));
        command.env_remove("SOLID_CHECKER_RUNTIME_RESOLVER");
        if case["runtime"] != "absent" {
            command.args(["--runtime-resolution", "required"])
                .env("SOLID_CHECKER_RUNTIME_RESOLVER", repository.join("fixtures/reactive-ir/inferred-host-reachability/resolver-selection-worker.mjs"))
                .env("SOLID_CHECKER_TEST_RESOLVER_CHOICE", case["runtime"].as_str().unwrap());
        }
        let output = command
            .env("SOLID_TYPEFACTS_BIN", &typefacts)
            .env(
                "SOLID_CHECKER_DAEMON",
                if case.get("mutatedParentConfig").is_some() {
                    "1"
                } else {
                    "0"
                },
            )
            .args(["--format", "json", "--project"])
            .arg(project.join("tsconfig.app.json"))
            .arg("--receipt-trust-configuration")
            .arg(&trust)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        let at_site = |finding: &serde_json::Value| {
            finding["id"] == "SC4001"
                && finding["kind"] == "violation"
                && finding["primaryLocation"]["startByte"].as_u64() == Some(start)
                && finding["primaryLocation"]["path"]
                    .as_str()
                    .is_some_and(|path| path.ends_with("src/main.ts"))
        };
        assert_eq!(
            result["findings"].as_array().unwrap().iter().any(at_site),
            expected,
            "{case}: {result:#?}"
        );
        let helper_start = u64::try_from(helper.find("startClosed()").unwrap()).unwrap();
        assert_eq!(
            result["findings"]
                .as_array()
                .unwrap()
                .iter()
                .any(|finding| finding["id"] == "SC4001"
                    && finding["kind"] == "violation"
                    && finding["primaryLocation"]["startByte"].as_u64() == Some(helper_start)
                    && finding["primaryLocation"]["path"]
                        .as_str()
                        .is_some_and(|path| path.ends_with("src/selected.ts"))),
            expected,
            "helper: {case}: {result:#?}"
        );
        if let Some(note) = case["note"].as_str() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(stderr.contains(note), "{case}: {stderr}");
            if case.get("scripts").is_some() || case.get("workspaceScripts").is_some() {
                assert!(stderr.contains("package.json:"), "{stderr}");
            }
            if case.get("workspaceScripts").is_some() {
                assert!(
                    stderr.contains(&format!("{}:", scratch.join("package.json").display())),
                    "{stderr}"
                );
            }
            if let Some(name) = case["configFile"].as_str() {
                assert!(stderr.contains(name), "{stderr}");
            }
        }
        if !expected {
            assert_eq!(result["status"], "uncertifiable");
            if case["runtime"] == "absent"
                && case["configRefused"] != true
                && case["processRefused"] != true
                && case.get("note").is_none()
                && case["resolver"] != false
            {
                let stderr = String::from_utf8_lossy(&output.stderr);
                assert!(
                    stderr
                        .contains("active tsconfig resolver requires authenticated file selection"),
                    "{case}: {stderr}"
                );
            }
        }
        if let Some(config) = case["mutatedParentConfig"].as_str() {
            let app_manifest = fs::read(project.join("package.json")).unwrap();
            let parent_manifest = fs::read(scratch.join("package.json")).unwrap();
            let before = fs::read(&parent_config).unwrap();
            fs::write(&parent_config, config).unwrap();
            assert_ne!(before, fs::read(&parent_config).unwrap());
            assert_eq!(
                app_manifest,
                fs::read(project.join("package.json")).unwrap()
            );
            assert_eq!(
                parent_manifest,
                fs::read(scratch.join("package.json")).unwrap()
            );
            // Repeat against retained daemon state and a fresh one-shot check.
            // A parent-config edit with unchanged manifests can never admit
            // browser authority while the enclosing Vite launch is visible.
            for daemon in ["1", "0"] {
                let output = command
                    .env("SOLID_CHECKER_DAEMON", daemon)
                    .output()
                    .unwrap();
                assert!(output.status.success());
                let after: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
                assert_eq!(after["status"], "uncertifiable");
                assert_eq!(after["findings"], result["findings"], "{case}: {after:#?}");
                let stderr = String::from_utf8_lossy(&output.stderr);
                assert!(stderr.contains(case["note"].as_str().unwrap()), "{stderr}");
                assert!(
                    stderr.contains(&format!("{}:", scratch.join("package.json").display())),
                    "{stderr}"
                );
                assert!(!stderr.contains("daemon unavailable"), "{stderr}");
            }
        }
        if let Some(name) = case["configFile"].as_str() {
            fs::remove_file(project.join(name)).unwrap();
        }
    }
    fs::remove_dir_all(scratch).unwrap();
}

#[test]
fn inferred_hosts_admit_browser_claims_without_changing_no_target_scopes() {
    use solid_facts_backend::fixture_authorization::{
        authorize_fixture_contract, read_fixture_contract_request,
    };
    use std::{fs, path::Path, process::Command};

    fn copy_tree(from: &Path, to: &Path) {
        fs::create_dir_all(to).unwrap();
        for entry in fs::read_dir(from).unwrap() {
            let entry = entry.unwrap();
            let destination = to.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy_tree(&entry.path(), &destination);
            } else {
                fs::copy(entry.path(), destination).unwrap();
            }
        }
    }

    let Ok(typefacts) = std::env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let repository = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let scratch = repository.join(format!(
        "rust/target/inferred-host-process-{}",
        std::process::id()
    ));
    copy_tree(
        &repository.join("fixtures/reactive-ir/node_modules"),
        &scratch.join("node_modules"),
    );
    for (name, browser) in [
        ("client-start", true),
        ("spa", true),
        ("server-scopes", true),
        ("ssr", false),
        ("library", false),
        ("override", false),
        ("metadata", true),
        ("metadata-published", false),
        ("alias", false),
        ("alias-unresolved", false),
        ("dynamic", true),
        ("dynamic-unknown", false),
        ("workspace", true),
        ("workspace-published", false),
        ("reachability", false),
        ("alias-congruent", true),
        ("alias-divergent", false),
        ("certification-clean", false),
        ("callback-invocation", false),
    ] {
        let project = scratch.join(name);
        copy_tree(
            &repository.join(format!("fixtures/reactive-ir/inferred-host-{name}")),
            &project,
        );
        let project = fs::canonicalize(project).unwrap();
        if name.starts_with("workspace") {
            fs::write(scratch.join("package.json"), "{\"workspaces\":[\"*\"]}").unwrap();
        }
        let authorization =
            authorize_fixture_contract(&project, &read_fixture_contract_request(&project).unwrap())
                .unwrap();
        let trust = scratch.join(format!("{name}-trust.json"));
        fs::write(&trust, authorization.trust_configuration).unwrap();
        let analyze = |target: Option<&str>| {
            let mut command = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"));
            command
                .env("SOLID_TYPEFACTS_BIN", &typefacts)
                .args(["--format", "json", "--project"])
                .arg(project.join("tsconfig.json"))
                .arg("--receipt-trust-configuration")
                .arg(&trust);
            if let Some(target) = target {
                command.args(["--runtime-target", target]);
            }
            let output = command.output().unwrap();
            assert!(
                output.status.success(),
                "{name}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            let mut result = serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap();
            result["_stderr"] = String::from_utf8_lossy(&output.stderr).into_owned().into();
            result
        };
        let snapshot = analyze((name == "override").then_some("node"));
        let findings = snapshot["findings"].as_array().unwrap();
        if name == "certification-clean" {
            assert!(findings.is_empty(), "{findings:#?}");
            assert_eq!(snapshot["status"], "uncertifiable");
            let output = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
                .env("SOLID_TYPEFACTS_BIN", &typefacts)
                .args(["--format", "json", "--certify", "--project"])
                .arg(project.join("tsconfig.json"))
                .output()
                .unwrap();
            assert_eq!(output.status.code(), Some(1));
            assert_eq!(analyze(Some("browser"))["status"], "certified");
            continue;
        }
        let source = if name == "callback-invocation" {
            "src/main.tsx"
        } else if name == "reachability" {
            "src/loaded.ts"
        } else if ["alias-congruent", "alias-divergent"].contains(&name) {
            "src/helper.ts"
        } else if [
            "spa",
            "metadata",
            "metadata-published",
            "alias",
            "alias-unresolved",
            "dynamic",
            "dynamic-unknown",
            "workspace",
            "workspace-published",
        ]
        .contains(&name)
        {
            "src/main.ts"
        } else {
            "src/App.tsx"
        };
        let inferred = |finding: &serde_json::Value| {
            finding["evidence"].as_array().is_some_and(|steps| {
                steps.iter().any(|step| {
                    step["message"]
                        .as_str()
                        .is_some_and(|message| message.starts_with("inferred browser:"))
                })
            })
        };
        if name == "reachability" {
            for (path, marker, expected) in [
                ("src/runtime-helper.ts", "optional resolver target", false),
                (
                    "src/probe/folder/index.ts",
                    "congruent directory index browser target",
                    false,
                ),
                (
                    "src/probe/competing.ts",
                    "first Vite extension agrees with Type Facts",
                    true,
                ),
            ] {
                let source = fs::read_to_string(project.join(path)).unwrap();
                let marker = source.find(marker).unwrap();
                let start =
                    u64::try_from(source[..marker].rfind("startClosed()").unwrap()).unwrap();
                assert_eq!(
                    findings.iter().any(|finding| {
                        finding["rule"] == "missing-owner"
                            && finding["kind"] == "violation"
                            && inferred(finding)
                            && finding["primaryLocation"]["path"]
                                .as_str()
                                .is_some_and(|p| p.ends_with(path))
                            && finding["primaryLocation"]["startByte"].as_u64() == Some(start)
                    }),
                    expected,
                    "{path}: {findings:#?}"
                );
            }
        }
        if name == "callback-invocation" {
            let source = fs::read_to_string(project.join("src/main.tsx")).unwrap();
            for marker in [
                "ignored prop",
                "cancelled timer",
                "server-only callback",
                "Show never selected",
                "empty For",
                "empty Repeat",
                "cancelled animation",
                "removed listener",
                "client-dead timer",
                "RPC-only callback",
                "non-Call callback row",
                "shadowed timer",
                "immediately consumed handle",
                "inert-prefix cancellation",
                "client-guarded cancellation",
                "unregistered top-level cleanup",
                "event under dead Show",
                "argument never registers timer",
                "explicit capture object removed",
                "nested component under dead Show",
                "nested component under ignored prop",
                "JSX parameter initialization never completes",
                "unregistered helper cleanup",
                "unregistered fresh-stack cleanup",
                "permanently pending continuation",
                "deferred constant effect never applies",
                "inert bundle never errors",
                "exact throwing IIFE dead continuation",
            ] {
                let marker_start = source.find(marker).unwrap();
                let call_start =
                    u64::try_from(source[..marker_start].rfind("startClosed()").unwrap()).unwrap();
                assert!(
                    !findings.iter().any(|finding| finding["kind"] == "violation"
                        && inferred(finding)
                        && finding["primaryLocation"]["startByte"].as_u64() == Some(call_start)),
                    "{marker}: {findings:#?}"
                );
            }
            for marker in [
                "noncancelable microtask",
                "feasible timer",
                "feasible animation",
                "authored deferred callback",
                "different handle stays feasible",
                "direct intrinsic child selected",
                "browser prefix before await",
                "constant effect initial apply",
                "throwing bundle error feasible",
                "browser prefix before exact throw",
            ] {
                let marker = source.find(marker).unwrap();
                let start =
                    u64::try_from(source[..marker].rfind("startClosed()").unwrap()).unwrap();
                // No registration is reached until every static package import
                // has a loadability witness. These declaration-only stubs do
                // not supply that witness, even for reviewed feasible triggers.
                assert!(
                    !findings
                        .iter()
                        .any(|finding| finding["rule"] == "missing-owner"
                            && finding["kind"] == "violation"
                            && inferred(finding)
                            && finding["primaryLocation"]["startByte"].as_u64() == Some(start)),
                    "{findings:#?}"
                );
            }
        }
        let module_violation = findings.iter().any(|finding| {
            finding["rule"] == "missing-owner"
                && finding["kind"] == "violation"
                && inferred(finding)
                && finding["primaryLocation"]["path"]
                    .as_str()
                    .is_some_and(|path| path.ends_with(source))
        });
        assert_eq!(module_violation, browser, "{name}: {findings:#?}");
        // Refused inference preserves baseline, including its release refusal,
        // open claims, existing violations or empty findings. Snapshot coverage
        // pins that complete result; absence of a host is not itself a finding.
        // Reachability has an independent re-export root even though loaded.ts
        // remains baseline. Callback cleanup violations are baseline findings.
        assert_eq!(
            findings.iter().any(inferred),
            browser || name == "reachability",
            "{name}: {findings:#?}"
        );
        if name == "spa" {
            let mut cases: Vec<serde_json::Value> = serde_json::from_slice(
                &fs::read(
                    repository.join("fixtures/reactive-ir/inferred-host-spa/nullish-cases.json"),
                )
                .unwrap(),
            )
            .unwrap();
            let completion_cases: Vec<serde_json::Value> = serde_json::from_slice(
                &fs::read(
                    repository.join("fixtures/reactive-ir/inferred-host-spa/completion-cases.json"),
                )
                .unwrap(),
            )
            .unwrap();
            cases.extend(completion_cases.into_iter().map(|mut case| {
                case["completion"] = case["normal"].clone();
                case
            }));
            for case in cases {
                for form in ["{};", "const completion_result = {};", "consume({});"] {
                    let source = format!(
                        "import {{startClosed}} from 'reactive-package'; function run(){{ {}; function consume(value: unknown) {{}} {} startClosed(); }} run();",
                        case["declaration"].as_str().unwrap(),
                        form.replace("{}", case["expression"].as_str().unwrap())
                    );
                    fs::write(project.join("src/main.ts"), &source).unwrap();
                    let result = analyze(None);
                    let call = u64::try_from(source.find("startClosed()").unwrap()).unwrap();
                    let violation = result["findings"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|finding| {
                            finding["kind"] == "violation"
                                && inferred(finding)
                                && finding["primaryLocation"]["path"]
                                    .as_str()
                                    .is_some_and(|path| path.ends_with("src/main.ts"))
                                && finding["primaryLocation"]["startByte"].as_u64() == Some(call)
                        });
                    assert_eq!(
                        violation,
                        case["completion"] == true,
                        "{source}: {result:#?}"
                    );
                }
            }
            // Every preceding declarator includes binding completion, not just
            // initializer evaluation. A normal binding remains a live control.
            for (body, expected) in [
                (
                    "const value={get x(){throw 0}}; const {x}=value, result=startClosed();",
                    false,
                ),
                (
                    "const value=[1]; const [x]=value, result=startClosed();",
                    false,
                ),
                ("const x=1, result=startClosed();", true),
            ] {
                let source = format!("import {{startClosed}} from 'reactive-package'; {body}");
                fs::write(project.join("src/main.ts"), &source).unwrap();
                let result = analyze(None);
                let call = u64::try_from(source.find("startClosed()").unwrap()).unwrap();
                assert_eq!(
                    result["findings"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|finding| {
                            finding["kind"] == "violation"
                                && finding["rule"] == "missing-owner"
                                && inferred(finding)
                                && finding["primaryLocation"]["path"]
                                    .as_str()
                                    .is_some_and(|path| path.ends_with("src/main.ts"))
                                && finding["primaryLocation"]["startByte"].as_u64() == Some(call)
                        }),
                    expected,
                    "{source}: {result:#?}"
                );
            }
            // Successful builds admit compiled and raw linking alike. These
            // deliberately failing transform inputs cannot grant JS behavior;
            // this test asserts only the importing module's exact live site.
            for css in [
                "@reference './missing.css';",
                "@reference '../../outside.css';",
                "@\\72 eference './missing.css';",
                "@apply definitely-not-a-real-utility;",
                ".x{",
            ] {
                fs::write(project.join("src/bad.module.css"), css).unwrap();
                for raw in [false, true] {
                    let source = format!(
                        "import './bad.module.css{}'; import {{startClosed}} from 'reactive-package'; startClosed();",
                        if raw { "?raw" } else { "" }
                    );
                    fs::write(project.join("src/main.ts"), &source).unwrap();
                    let result = analyze(None);
                    let call = u64::try_from(source.find("startClosed()").unwrap()).unwrap();
                    assert!(
                        result["findings"].as_array().unwrap().iter().any(
                            |finding| finding["kind"] == "violation"
                                && inferred(finding)
                                && finding["primaryLocation"]["path"]
                                    .as_str()
                                    .is_some_and(|path| path.ends_with("src/main.ts"))
                                && finding["primaryLocation"]["startByte"].as_u64() == Some(call)
                        ),
                        "{source}: {result:#?}"
                    );
                }
            }
            let styles = project.join("node_modules/styles");
            fs::create_dir_all(&styles).unwrap();
            fs::write(styles.join("index.d.ts"), "export {};").unwrap();
            fs::write(styles.join("index.js"), "export {};").unwrap();
            fs::write(styles.join("style.css"), "@import './missing.css'; .x{").unwrap();
            fs::write(styles.join("style.scss"), "$invalid: ; .x{").unwrap();
            for (exports, text, expected) in [
                (
                    r#"{".":{"types":"./index.d.ts","default":"./style.css"}}"#,
                    "styles",
                    true,
                ),
                (
                    r#"{"./theme":{"types":"./index.d.ts","default":"./style.css"}}"#,
                    "styles/theme",
                    true,
                ),
                (
                    r#"{".":{"types":"./index.d.ts","production":"./style.css","default":"./index.js"}}"#,
                    "styles",
                    true,
                ),
                (
                    r#"{".":{"types":"./index.d.ts","solid":"./style.css","default":"./index.js"}}"#,
                    "styles",
                    true,
                ),
                (
                    r#"{".":{"types":"./index.d.ts","default":"./style.scss"}}"#,
                    "styles",
                    true,
                ),
                (
                    r#"{".":{"types":"./index.d.ts","default":"./index.js"}}"#,
                    "styles",
                    true,
                ),
                (
                    r#"{"./theme":{"types":"./index.d.ts","default":"./index.js"}}"#,
                    "styles/theme",
                    true,
                ),
                (
                    r#"{".":{"types":"./index.d.ts","default":"./style.css"}}"#,
                    "styles?raw",
                    true,
                ),
                (
                    r#"{"./theme":{"types":"./index.d.ts","default":"./style.css"}}"#,
                    "styles/theme?raw",
                    true,
                ),
            ] {
                fs::write(styles.join("package.json"), format!(r#"{{"name":"styles","version":"1.0.0","type":"module","exports":{exports}}}"#)).unwrap();
                let source = format!(
                    "import '{text}'; import {{startClosed}} from 'reactive-package'; startClosed();"
                );
                fs::write(project.join("src/main.ts"), &source).unwrap();
                let result = analyze(None);
                let call = u64::try_from(source.find("startClosed()").unwrap()).unwrap();
                assert_eq!(
                    result["findings"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|finding| finding["kind"] == "violation"
                            && finding["rule"] == "missing-owner"
                            && inferred(finding)
                            && finding["primaryLocation"]["path"]
                                .as_str()
                                .is_some_and(|path| path.ends_with("src/main.ts"))
                            && finding["primaryLocation"]["startByte"].as_u64() == Some(call)),
                    expected,
                    "{source}: {exports}: {result:#?}"
                );
            }
            let config_path = project.join("vite.config.ts");
            let original_config = fs::read_to_string(&config_path).unwrap();
            let tsconfig_path = project.join("tsconfig.json");
            let original_tsconfig = fs::read_to_string(&tsconfig_path).unwrap();
            let mut tsconfig: serde_json::Value = serde_json::from_str(&original_tsconfig).unwrap();
            tsconfig["compilerOptions"]["paths"] = serde_json::json!({"~/*":["./src/*"]});
            fs::write(&tsconfig_path, serde_json::to_vec(&tsconfig).unwrap()).unwrap();
            let resource_cases: Vec<serde_json::Value> = serde_json::from_slice(
                &fs::read(
                    repository
                        .join("fixtures/reactive-ir/inferred-host-spa/static-resource-cases.json"),
                )
                .unwrap(),
            )
            .unwrap();
            for name in ["tailwindcss", "css-provider"] {
                let root = project.join("node_modules").join(name);
                fs::create_dir_all(&root).unwrap();
                fs::write(root.join("package.json"), format!(r#"{{"name":"{name}","exports":{{".":{{"style":"./index.css","import":"./runtime.js"}}}}}}"#)).unwrap();
                fs::write(root.join("index.css"), ".x{}").unwrap();
            }
            for case in resource_cases {
                if let Some(dependencies) = case["dependencies"].as_object() {
                    for (file, contents) in dependencies {
                        fs::write(project.join("src").join(file), contents.as_str().unwrap())
                            .unwrap();
                    }
                }
                if let Some(file) = case["file"].as_str() {
                    fs::write(
                        project.join("src").join(file),
                        case["contents"].as_str().unwrap(),
                    )
                    .unwrap();
                }
                fs::write(
                    &config_path,
                    case["config"].as_str().unwrap_or(&original_config),
                )
                .unwrap();
                let source = format!(
                    "{} import {{startClosed}} from 'reactive-package'; startClosed();",
                    case["statement"].as_str().unwrap()
                );
                fs::write(project.join("src/main.ts"), &source).unwrap();
                let result = analyze(None);
                let call = u64::try_from(source.find("startClosed()").unwrap()).unwrap();
                assert_eq!(
                    result["findings"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|finding| finding["kind"] == "violation"
                            && finding["rule"] == "missing-owner"
                            && inferred(finding)
                            && finding["primaryLocation"]["path"]
                                .as_str()
                                .is_some_and(|path| path.ends_with("src/main.ts"))
                            && finding["primaryLocation"]["startByte"].as_u64() == Some(call)),
                    case["live"] == true,
                    "{}: {result:#?}",
                    case["name"]
                );
                // Explicit Node always wins over successful-build inference.
                if case["live"] == true {
                    assert!(
                        !analyze(Some("node"))["findings"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .any(inferred)
                    );
                }
            }
            fs::write(&config_path, original_config).unwrap();
            fs::write(&tsconfig_path, original_tsconfig).unwrap();
            fs::write(project.join("src/main.ts"), "import {startClosed} from 'reactive-package'; function run(){ class Stop {constructor(){throw 0}} new Stop(); startClosed(); } run();").unwrap();
            let constructor_refused = analyze(None);
            assert!(
                !constructor_refused["findings"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|finding| finding["kind"] == "violation" && inferred(finding)),
                "constructor continuation must stay baseline: {constructor_refused:#?}"
            );
            let factory = project.join("node_modules/startup-factory");
            fs::create_dir_all(&factory).unwrap();
            fs::write(factory.join("package.json"), r#"{"name":"startup-factory","version":"1.0.0","types":"index.d.ts","module":"index.js"}"#).unwrap();
            fs::write(factory.join("index.d.ts"), "export declare function createStore(value: unknown): unknown; export declare function createRouter(value: unknown): unknown;").unwrap();
            fs::write(factory.join("index.js"), "export function createStore(value){return value} export function createRouter(value){return value}").unwrap();
            let cases: Vec<serde_json::Value> = serde_json::from_slice(
                &fs::read(
                    repository.join("fixtures/reactive-ir/inferred-host-spa/startup-cases.json"),
                )
                .unwrap(),
            )
            .unwrap();
            for case in cases {
                fs::write(
                    project.join("src/startup.ts"),
                    case["dependency"].as_str().unwrap(),
                )
                .unwrap();
                let source = format!(
                    "{} import {{startClosed}} from 'reactive-package'; {}",
                    case["load"].as_str().unwrap_or("import './startup';"),
                    case["body"].as_str().unwrap()
                );
                fs::write(project.join("src/main.ts"), &source).unwrap();
                let result = analyze(None);
                let (site_source, site_path) = if case["site"] == "startup" {
                    (case["dependency"].as_str().unwrap(), "src/startup.ts")
                } else {
                    (source.as_str(), "src/main.ts")
                };
                let call = u64::try_from(site_source.find("startClosed()").unwrap()).unwrap();
                let at_site = |finding: &serde_json::Value| {
                    finding["kind"] == "violation"
                        && inferred(finding)
                        && finding["primaryLocation"]["path"]
                            .as_str()
                            .is_some_and(|path| path.ends_with(site_path))
                        && finding["primaryLocation"]["startByte"].as_u64() == Some(call)
                };
                assert_eq!(
                    result["findings"].as_array().unwrap().iter().any(at_site),
                    case["browser"] == true,
                    "{case}: {result:#?}"
                );
                if case["contradiction"] == true {
                    assert!(
                        result["_stderr"]
                            .as_str()
                            .unwrap()
                            .contains("certain top-level exit contradicts the app-starts premise"),
                        "{result:#?}"
                    );
                }
                assert!(
                    !analyze(Some("node"))["findings"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(inferred)
                );
            }
            fs::write(project.join("src/startup.ts"), "export {};").unwrap();
            // Use the independently authorized browser artifact, without Solid
            // declaration-only imports that would refuse module evaluation.
            for (twin, expected) in [("dead", false), ("live", true)] {
                let source = fs::read_to_string(repository.join(format!(
                    "fixtures/reactive-ir/inferred-host-reachability/src/completion-diagnostic-{twin}.ts"
                ))).unwrap();
                fs::write(project.join("src/main.ts"), &source).unwrap();
                let result = analyze(None);
                let findings = result["findings"].as_array().unwrap();
                let call = u64::try_from(source.find("startClosed()").unwrap()).unwrap();
                let violation = |finding: &serde_json::Value| {
                    finding["rule"] == "missing-owner"
                        && finding["kind"] == "violation"
                        && finding["primaryLocation"]["path"]
                            .as_str()
                            .is_some_and(|path| path.ends_with("src/main.ts"))
                        && finding["primaryLocation"]["startByte"].as_u64() == Some(call)
                };
                assert_eq!(
                    findings
                        .iter()
                        .any(|finding| violation(finding) && inferred(finding)),
                    expected,
                    "{twin}: {result:#?}"
                );
                if !expected {
                    assert!(!findings.iter().any(violation), "{result:#?}");
                    // A certainly dead call needs neither a violation nor
                    // an open package-behavior claim when no live import use
                    // remains. Do not manufacture uncertainty for dead code.
                }
                // Positive explicit target confirms the fixture's owner claim
                // is admitted independently of the inferred-host projection.
                if expected {
                    let explicit = analyze(Some("browser"));
                    assert!(
                        explicit["findings"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .any(violation),
                        "{explicit:#?}"
                    );
                }
            }
        }
        if name == "server-scopes" {
            for finding in findings.iter().filter(|finding| {
                finding["primaryLocation"]["path"]
                    .as_str()
                    .is_some_and(|path| {
                        path.ends_with("src/Document.tsx") || path.ends_with("src/server.ts")
                    })
            }) {
                assert!(!inferred(finding));
            }
            // No target input changes for the baseline: remove the config on
            // this private copy and compare every server-side finding byte for
            // byte, including evidence. ADR 0241 remains in force there.
            fs::rename(
                project.join("vite.config.ts"),
                project.join("config.disabled"),
            )
            .unwrap();
            let baseline = analyze(None);
            let server_findings = |snapshot: &serde_json::Value| {
                snapshot["findings"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|finding| {
                        finding["primaryLocation"]["path"]
                            .as_str()
                            .is_some_and(|path| {
                                path.ends_with("src/Document.tsx")
                                    || path.ends_with("src/server.ts")
                            })
                    })
                    .cloned()
                    .collect::<Vec<_>>()
            };
            let expected = server_findings(&baseline);
            assert!(!expected.is_empty(), "server baseline must be observed");
            assert_eq!(server_findings(&snapshot), expected);
        }
        let explicit = analyze(Some("browser"));
        let source_text = fs::read_to_string(project.join(source)).unwrap();
        assert!(
            !explicit["findings"]
                .as_array()
                .unwrap()
                .iter()
                .any(inferred)
        );
        assert!(
            explicit["findings"]
                .as_array()
                .unwrap()
                .iter()
                .any(|finding| finding["rule"] == "missing-owner"
                    && finding["kind"] == "violation"
                    && finding["primaryLocation"]["path"]
                        .as_str()
                        .is_some_and(|path| path.ends_with(source))
                    && finding["primaryLocation"]["startByte"]
                        .as_u64()
                        .zip(finding["primaryLocation"]["endByte"].as_u64())
                        .is_some_and(|(start, end)| source_text.get(start as usize..end as usize)
                            == Some("startClosed()")))
        );
        if name == "client-start" {
            // The generated client wrapper cannot invoke the default export
            // when importing App certainly throws during initialization.
            fs::write(project.join("src/App.tsx"),
                "import {startClosed} from 'reactive-package';\nthrow 0;\nexport default function App(){setTimeout(()=>startClosed(),0);return <div/>;}\n"
            ).unwrap();
            let failed = analyze(None);
            assert!(!failed["findings"].as_array().unwrap().iter().any(inferred));
        }
    }
    fs::remove_dir_all(scratch).unwrap();
}

/// The public Rust entry points after the host left the contract index: the
/// inference result's host must accompany its contracts, and the pair is the
/// same analysis the native entry point runs.
#[test]
fn inferred_host_is_a_separate_input_beside_the_browser_view_it_selected() {
    use solid_facts_backend::{
        AnalysisInputs, BackendError, DiagnosticSession, InferredAcceptedContracts,
        NativeIncrementalSession, RequestedRuleEnablement, TypeFactsSession,
        decode_policy2_trust_configuration,
        fixture_authorization::{authorize_fixture_contract, read_fixture_contract_request},
        inferred_project_accepted_contracts_with_note,
    };
    use solid_reactive_ir::{RuntimeEnvironment, contract_semantics::AcceptedContractIndex};
    use std::{fs, path::Path};

    fn copy_tree(from: &Path, to: &Path) {
        fs::create_dir_all(to).unwrap();
        for entry in fs::read_dir(from).unwrap() {
            let entry = entry.unwrap();
            let destination = to.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy_tree(&entry.path(), &destination);
            } else {
                fs::copy(entry.path(), destination).unwrap();
            }
        }
    }

    let Ok(producer) = std::env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let repository = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let scratch = repository.join(format!(
        "rust/target/inferred-host-input-{}",
        std::process::id()
    ));
    copy_tree(
        &repository.join("fixtures/reactive-ir/node_modules"),
        &scratch.join("node_modules"),
    );
    let directory = scratch.join("spa");
    copy_tree(
        &repository.join("fixtures/reactive-ir/inferred-host-spa"),
        &directory,
    );
    let directory = fs::canonicalize(directory).unwrap();
    let authorization = authorize_fixture_contract(
        &directory,
        &read_fixture_contract_request(&directory).unwrap(),
    )
    .unwrap();
    let trust = decode_policy2_trust_configuration(&authorization.trust_configuration).unwrap();
    let project = directory.join("tsconfig.json").to_str().unwrap().to_owned();
    let typescript = TypeFactsSession::open(&producer, &project, &[]).unwrap();
    let dialect = solid_facts_backend::dialect::default_dialect();
    let (mut session, sources) =
        NativeIncrementalSession::open_pipelined(dialect, project, typescript).unwrap();
    let facts = session.analyze().unwrap();
    let InferredAcceptedContracts {
        contracts,
        hosts,
        note,
    } = inferred_project_accepted_contracts_with_note(
        &directory,
        &[directory.join(solid_facts_backend::fixture_authorization::CATALOG)],
        &[],
        Some(&trust),
        false,
        &RuntimeEnvironment::default(),
        dialect.vocabulary,
        &facts,
        AcceptedContractIndex::default(),
    )
    .unwrap();
    // A live inference, not a refusal: the run note and a host.
    assert!(
        note.as_deref()
            .is_some_and(|note| note.starts_with("solid-checker: note: browser host inferred for")),
        "{note:?}"
    );
    let hosts = hosts.expect("the fixture infers a browser host");
    let analyze = |inputs: AnalysisInputs<'_>| {
        DiagnosticSession::new(dialect).analyze_inputs_measured_with_enablement(
            Path::new(&facts.project_id),
            &sources,
            &facts,
            inputs,
            RequestedRuleEnablement::default(),
        )
    };
    // The migrated caller: the contracts with the host that selected them.
    let (analysis, _) =
        analyze(AnalysisInputs::contracts_only(&contracts).with_hosts(&hosts)).unwrap();
    assert!(
        analysis.snapshot.findings.iter().any(|finding| {
            finding.rule == "missing-owner"
                && finding.kind == "violation"
                && finding
                    .evidence
                    .iter()
                    .any(|step| step.message.starts_with("inferred browser:"))
        }),
        "{:#?}",
        analysis.snapshot.findings
    );
    let refusal = |inputs: AnalysisInputs<'_>| match analyze(inputs) {
        Err(BackendError::Contract(message)) => message,
        Err(other) => panic!("unexpected error: {other}"),
        Ok(_) => panic!("an inconsistent pair was analyzed"),
    };
    // Dropping the host is a refusal, not a silent baseline run.
    assert!(refusal(AnalysisInputs::contracts_only(&contracts)).contains("has no host input"));
    // A host with no view built from it selects nothing.
    let (baseline, browser) = contracts.execution_views().unwrap();
    assert!(
        refusal(AnalysisInputs::contracts_only(&baseline).with_hosts(&hosts))
            .contains("no browser view")
    );
    // A host that moved after the view was selected from it.
    let mut later = hosts.clone();
    later.blocked_imports.insert(("/x.ts".into(), "x".into()));
    assert!(
        refusal(AnalysisInputs::contracts_only(&contracts).with_hosts(&later))
            .contains("does not match the host input")
    );
    // The two halves of the fork are valid on their own: the baseline needs no
    // host, and the browser half carries a view but projects no findings.
    analyze(AnalysisInputs::contracts_only(&baseline)).unwrap();
    analyze(AnalysisInputs::contracts_only(&browser)).unwrap();
    fs::remove_dir_all(scratch).unwrap();
}

/// A recorded source-edge refusal, supplied through the explicit analysis API
/// by this test and by no production caller: the dormant consumer end of
/// `PackageBoundary::record_source_edge_refusal`. It asserts the consumer's
/// own behavior, not that any producer decides to record anything, and not
/// that a contract emission or certification transaction refuses it.
#[test]
fn a_recorded_source_edge_refusal_is_one_neutral_finding_beside_independent_source_facts() {
    use solid_facts_backend::{
        AnalysisInputs, BackendError, DiagnosticSession, InferredAcceptedContracts,
        NativeIncrementalSession, RequestedRuleEnablement, TypeFactsSession,
        decode_policy2_trust_configuration,
        fixture_authorization::{authorize_fixture_contract, read_fixture_contract_request},
        inferred_project_accepted_contracts_with_note,
    };
    use solid_reactive_ir::{
        BuildError, RuntimeEnvironment,
        contract_semantics::AcceptedContractIndex,
        package_boundary::{
            ImportKey, PackageBoundary, SourceEdgeForm, SourceEdgeMismatch, SourceEdgeSite,
        },
    };
    use std::{fs, path::Path};

    fn copy_tree(from: &Path, to: &Path) {
        fs::create_dir_all(to).unwrap();
        for entry in fs::read_dir(from).unwrap() {
            let entry = entry.unwrap();
            let destination = to.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy_tree(&entry.path(), &destination);
            } else {
                fs::copy(entry.path(), destination).unwrap();
            }
        }
    }

    let Ok(producer) = std::env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let repository = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let scratch = repository.join(format!(
        "rust/target/source-edge-refusal-{}",
        std::process::id()
    ));
    copy_tree(
        &repository.join("fixtures/reactive-ir/node_modules"),
        &scratch.join("node_modules"),
    );
    let dialect = solid_facts_backend::dialect::default_dialect();
    // Opens a Type Facts session over `directory` and returns what an
    // analysis is given: the session (kept alive), the sources and the facts.
    macro_rules! open_project {
        ($directory:expr) => {{
            let project = $directory
                .join("tsconfig.json")
                .to_str()
                .unwrap()
                .to_owned();
            let typescript = TypeFactsSession::open(&producer, &project, &[]).unwrap();
            let (mut session, sources) =
                NativeIncrementalSession::open_pipelined(dialect, project, typescript).unwrap();
            let facts = session.analyze().unwrap();
            (session, sources, facts)
        }};
    }
    // The one record: `importer`, `specifier`, and the exact range of the
    // first occurrence of `statement` in that file, found by text.
    fn refusal(
        facts: &solid_facts::ProjectFacts,
        file: &str,
        specifier: &str,
        form: SourceEdgeForm,
        source: &str,
        statement: &str,
    ) -> PackageBoundary {
        let importer = facts
            .files
            .iter()
            .find(|candidate| candidate.path.as_str().ends_with(file))
            .expect("the importer is a project file")
            .path
            .to_string();
        let start = source.find(statement).expect("statement in source");
        let mut boundary = PackageBoundary::default();
        boundary.record_source_edge_refusal(
            ImportKey::new(importer, specifier),
            SourceEdgeSite {
                form,
                start_byte: start as u64,
                end_byte: (start + statement.len()) as u64,
            },
        );
        boundary
    }

    // A real accepted contract and a real inferred browser host for the
    // project in `directory`: what the host fork is built from.
    macro_rules! inferred_project {
        ($directory:expr) => {{
            let authorization = authorize_fixture_contract(
                &$directory,
                &read_fixture_contract_request(&$directory).unwrap(),
            )
            .unwrap();
            let trust =
                decode_policy2_trust_configuration(&authorization.trust_configuration).unwrap();
            let (session, sources, facts) = open_project!($directory);
            let InferredAcceptedContracts {
                contracts, hosts, ..
            } = inferred_project_accepted_contracts_with_note(
                &$directory,
                &[$directory.join(solid_facts_backend::fixture_authorization::CATALOG)],
                &[],
                Some(&trust),
                false,
                &RuntimeEnvironment::default(),
                dialect.vocabulary,
                &facts,
                AcceptedContractIndex::default(),
            )
            .unwrap();
            let hosts = hosts.expect("the fixture infers a browser host");
            (session, sources, facts, contracts, hosts)
        }};
    }

    // ---- 1. The inferred-host fixture: real accepted contract, real fork ----
    let directory = scratch.join("spa");
    copy_tree(
        &repository.join("fixtures/reactive-ir/inferred-host-spa"),
        &directory,
    );
    let directory = fs::canonicalize(directory).unwrap();
    let (_session, sources, facts, contracts, hosts) = inferred_project!(directory);
    let main = fs::read_to_string(directory.join("src/main.ts")).unwrap();
    let statement = r#"import { startClosed } from "reactive-package";"#;
    let recorded = refusal(
        &facts,
        "src/main.ts",
        "reactive-package",
        SourceEdgeForm::Import,
        &main,
        statement,
    );
    let analyze = |session: &mut DiagnosticSession, inputs: AnalysisInputs<'_>| {
        session.analyze_inputs_measured_with_enablement(
            Path::new(&facts.project_id),
            &sources,
            &facts,
            inputs,
            RequestedRuleEnablement::default(),
        )
    };
    let none = PackageBoundary::default();
    let with_host = |boundary| AnalysisInputs {
        boundary,
        contracts: &contracts,
        hosts: Some(&hosts),
    };

    // B1: no record. The real contract still yields the browser-view
    // violation, which is what the record will replace.
    let (base, _) = analyze(&mut DiagnosticSession::new(dialect), with_host(&none)).unwrap();
    assert!(
        base.snapshot.findings.iter().any(|finding| {
            finding.rule == "missing-owner"
                && finding.kind == "violation"
                && finding
                    .evidence
                    .iter()
                    .any(|step| step.message.starts_with("inferred browser:"))
        }),
        "{:#?}",
        base.snapshot.findings
    );

    // B2: the record, with the host. The eligible occurrence keeps the
    // browser half's marker and appends the host reason; the contract-derived
    // violation is gone, which is the intended loss of a positive claim.
    let (analysis, _) =
        analyze(&mut DiagnosticSession::new(dialect), with_host(&recorded)).unwrap();
    let findings = &analysis.snapshot.findings;
    assert_eq!(findings.len(), 1, "{findings:#?}");
    let finding = &findings[0];
    assert_eq!(finding.id, "SC9005");
    assert_eq!(finding.rule, "package-contract-incomplete");
    assert_eq!(finding.kind, "uncertifiable");
    assert_eq!(finding.severity, "error");
    assert_eq!(
        finding.message,
        "source analysis was withheld for reactive-package: the available facts do not establish the source boundary needed to determine its reactive behavior"
    );
    assert_eq!(
        finding.hint,
        "This finding does not certify reactive behavior across this edge. It does not establish a runtime target, execution, or the existence of an export."
    );
    assert_eq!(finding.analysis_context, "source-boundary-withheld:import");
    assert_eq!(finding.coverage_family, "");
    assert_eq!(finding.primary_location.start_byte, 0);
    assert_eq!(finding.primary_location.end_byte, statement.len() as u64);
    assert_eq!(
        (
            finding.primary_location.line,
            finding.primary_location.column
        ),
        (1, 1)
    );
    assert_eq!(analysis.snapshot.status, "uncertifiable");
    assert_eq!(
        finding
            .evidence
            .iter()
            .map(|step| step.message.as_str())
            .collect::<Vec<_>>(),
        [
            "the analysis input recorded that source-boundary analysis was withheld for this exact occurrence",
            // The statement is a browser proof site, so the merged finding
            // also carries the host's reason, as every eligible one does.
            format!(
                "inferred browser: Vite index.html module entry; roots: {}",
                directory.join("src/main.ts").display()
            )
            .as_str(),
        ]
    );

    // B3: both halves of the fork deny, so each alone answers the same one
    // neutral finding.
    let (baseline, browser) = contracts.execution_views().unwrap();
    let contracts_only = |contracts| AnalysisInputs {
        boundary: &recorded,
        contracts,
        hosts: None,
    };
    for half in [&baseline, &browser] {
        let (alone, _) =
            analyze(&mut DiagnosticSession::new(dialect), contracts_only(half)).unwrap();
        assert_eq!(
            alone.snapshot.findings.len(),
            1,
            "{:#?}",
            alone.snapshot.findings
        );
        assert_eq!(
            alone.snapshot.findings[0].analysis_context,
            "source-boundary-withheld:import"
        );
    }

    // B4: same-generation diagnostic reuse is a property of the baseline
    // index alone (the host fork never reports `reused`). Adding the record
    // misses the retained answer, repeating it hits, removing it misses and
    // answers what no record answered.
    let mut session = DiagnosticSession::new(dialect);
    let plain = |boundary| AnalysisInputs {
        boundary,
        contracts: &baseline,
        hosts: None,
    };
    let (first, timings) = analyze(&mut session, plain(&none)).unwrap();
    assert!(!timings.reused);
    let (again, timings) = analyze(&mut session, plain(&none)).unwrap();
    assert!(timings.reused);
    assert!(std::sync::Arc::ptr_eq(&first, &again));
    let (added, timings) = analyze(&mut session, plain(&recorded)).unwrap();
    assert!(!timings.reused);
    assert_eq!(added.snapshot.findings.len(), 1);
    let (repeated, timings) = analyze(&mut session, plain(&recorded)).unwrap();
    assert!(timings.reused);
    assert!(std::sync::Arc::ptr_eq(&added, &repeated));
    let (removed, timings) = analyze(&mut session, plain(&none)).unwrap();
    assert!(!timings.reused);
    assert_eq!(
        serde_json::to_value(&removed.snapshot.findings).unwrap(),
        serde_json::to_value(&first.snapshot.findings).unwrap()
    );
    // The host fork, repeated in one session, answers equal merged findings
    // each time, and equal to a fresh session's. It reports and promises no
    // reuse: the outer `timings.reused` is not asserted here.
    let mut host_session = DiagnosticSession::new(dialect);
    let host_first = analyze(&mut host_session, with_host(&recorded)).unwrap().0;
    let host_second = analyze(&mut host_session, with_host(&recorded)).unwrap().0;
    for repeated in [&host_first, &host_second] {
        assert_eq!(
            serde_json::to_value(&repeated.snapshot.findings).unwrap(),
            serde_json::to_value(&analysis.snapshot.findings).unwrap()
        );
    }

    // B5: a record the facts do not hold is an explicit error, no findings,
    // and no nearby statement stands in for it.
    let mut shifted = PackageBoundary::default();
    for (key, sites) in recorded.source_edge_refusals() {
        for site in sites {
            shifted.record_source_edge_refusal(
                key.clone(),
                SourceEdgeSite {
                    start_byte: site.start_byte + 1,
                    ..*site
                },
            );
        }
    }
    match analyze(&mut DiagnosticSession::new(dialect), with_host(&shifted)) {
        Err(BackendError::ReactiveIr(BuildError::SourceEdge(
            SourceEdgeMismatch::NoSuchOccurrence { .. },
        ))) => {}
        Err(other) => panic!("unexpected error: {other}"),
        Ok(_) => panic!("a record the facts do not hold was analyzed"),
    }

    // ---- 1b. Eligible, ineligible and repeated occurrences ----
    // Two statements of one key in the browser entry, and one in a file no
    // browser root reaches. Eligibility is the host's per-site answer, never
    // inferred from the import being there.
    let repeated = scratch.join("spa-repeated");
    copy_tree(&directory, &repeated);
    let repeated = fs::canonicalize(repeated).unwrap();
    let entry = "import { startClosed } from \"reactive-package\";\nimport { startClosed as again } from \"reactive-package\";\nstartClosed();\nagain();\n";
    let unreached = "import { startClosed } from \"reactive-package\";\nexport const run = () => startClosed();\n";
    fs::write(repeated.join("src/main.ts"), entry).unwrap();
    fs::write(repeated.join("src/unreached.ts"), unreached).unwrap();
    let (_repeated_session, repeated_sources, repeated_facts, repeated_contracts, repeated_hosts) =
        inferred_project!(repeated);
    let mut every = PackageBoundary::default();
    for (file, source, statement) in [
        (
            "src/main.ts",
            entry,
            "import { startClosed } from \"reactive-package\";",
        ),
        (
            "src/main.ts",
            entry,
            "import { startClosed as again } from \"reactive-package\";",
        ),
        (
            "src/unreached.ts",
            unreached,
            "import { startClosed } from \"reactive-package\";",
        ),
    ] {
        every.extend(refusal(
            &repeated_facts,
            file,
            "reactive-package",
            SourceEdgeForm::Import,
            source,
            statement,
        ));
    }
    let (every_analysis, _) = DiagnosticSession::new(dialect)
        .analyze_inputs_measured_with_enablement(
            Path::new(&repeated_facts.project_id),
            &repeated_sources,
            &repeated_facts,
            AnalysisInputs {
                boundary: &every,
                contracts: &repeated_contracts,
                hosts: Some(&repeated_hosts),
            },
            RequestedRuleEnablement::default(),
        )
        .unwrap();
    let rows = every_analysis
        .snapshot
        .findings
        .iter()
        .map(|finding| {
            (
                finding
                    .primary_location
                    .path
                    .rsplit('/')
                    .next()
                    .unwrap()
                    .to_owned(),
                finding.primary_location.start_byte,
                finding.analysis_context.clone(),
                finding
                    .evidence
                    .iter()
                    .filter(|step| step.message.starts_with("inferred browser:"))
                    .count(),
            )
        })
        .collect::<Vec<_>>();
    // Pinned after the first non-updating run: the unreached occurrence keeps
    // its ineligible baseline marker with no host step, and each of the two
    // repeated statements is its own finding that gained the host's reason.
    let neutral = "source-boundary-withheld:import".to_owned();
    assert_eq!(
        rows,
        vec![
            ("unreached.ts".to_owned(), 0, neutral.clone(), 0),
            ("main.ts".to_owned(), 0, neutral.clone(), 1),
            ("main.ts".to_owned(), 48, neutral, 1),
        ],
        "{:#?}",
        every_analysis.snapshot.findings
    );

    // ---- 2. Source facts survive the marker: an independent implementation ----
    // `callback-forwarding` imports `invoke` and `readCount` from a source
    // file in the program. The recorded refusal denies no contract (there is
    // none) and removes no source summary, so the forwarded callback still
    // derives its strict read beside the marker.
    let source_directory = scratch.join("callback-forwarding");
    copy_tree(
        &repository.join("fixtures/reactive-ir/callback-forwarding"),
        &source_directory,
    );
    let source_directory = fs::canonicalize(source_directory).unwrap();
    let (_source_session, source_sources, source_facts) = open_project!(source_directory);
    let app = fs::read_to_string(source_directory.join("App.tsx")).unwrap();
    let import = r#"import { invoke, readCount } from "./source";"#;
    let source_record = refusal(
        &source_facts,
        "App.tsx",
        "./source",
        SourceEdgeForm::Import,
        &app,
        import,
    );
    let empty = AcceptedContractIndex::default();
    let source_analysis = |boundary| {
        DiagnosticSession::new(dialect)
            .analyze_inputs_measured_with_enablement(
                Path::new(&source_facts.project_id),
                &source_sources,
                &source_facts,
                AnalysisInputs {
                    boundary,
                    contracts: &empty,
                    hosts: None,
                },
                RequestedRuleEnablement::default(),
            )
            .unwrap()
            .0
    };
    let plain = source_analysis(&none);
    let marked = source_analysis(&source_record);
    let strict = |analysis: &solid_facts_backend::DiagnosticAnalysis| {
        analysis
            .snapshot
            .findings
            .iter()
            .filter(|finding| finding.rule == "strict-read-untracked")
            .map(|finding| (finding.message.clone(), finding.primary_location.start_byte))
            .collect::<Vec<_>>()
    };
    assert_eq!(strict(&plain).len(), 1, "{:#?}", plain.snapshot.findings);
    assert!(strict(&plain)[0].0.contains("invoke"));
    assert_eq!(
        strict(&marked),
        strict(&plain),
        "the source-derived finding is unchanged beside the marker"
    );
    assert_eq!(
        marked
            .snapshot
            .findings
            .iter()
            .filter(|finding| finding.analysis_context == "source-boundary-withheld:import")
            .count(),
        1
    );
    assert!(
        plain
            .snapshot
            .findings
            .iter()
            .all(|finding| finding.analysis_context != "source-boundary-withheld:import")
    );

    // ---- 3. TypeScript's claim and this one are different propositions ----
    // An import of an export the declarations lack: `tsc` reports TS2305 for
    // it (recorded in the handoff report). The recorded refusal asserts only
    // that the analysis input withheld source analysis, never that the export
    // is missing or present.
    let missing_directory = scratch.join("spa-missing");
    copy_tree(&directory, &missing_directory);
    let missing_directory = fs::canonicalize(missing_directory).unwrap();
    let absent = "import { absent } from \"reactive-package\";\nabsent();\n";
    fs::write(missing_directory.join("src/main.ts"), absent).unwrap();
    let (_missing_session, missing_sources, missing_facts) = open_project!(missing_directory);
    let missing_record = refusal(
        &missing_facts,
        "src/main.ts",
        "reactive-package",
        SourceEdgeForm::Import,
        absent,
        "import { absent } from \"reactive-package\";",
    );
    let (missing, _) = DiagnosticSession::new(dialect)
        .analyze_inputs_measured_with_enablement(
            Path::new(&missing_facts.project_id),
            &missing_sources,
            &missing_facts,
            AnalysisInputs {
                boundary: &missing_record,
                contracts: &empty,
                hosts: None,
            },
            RequestedRuleEnablement::default(),
        )
        .unwrap();
    let neutral = missing
        .snapshot
        .findings
        .iter()
        .filter(|finding| finding.analysis_context == "source-boundary-withheld:import")
        .collect::<Vec<_>>();
    assert_eq!(neutral.len(), 1, "{:#?}", missing.snapshot.findings);
    for text in [&neutral[0].message, &neutral[0].hint] {
        for word in ["missing", "does not exist", "not found", "no export"] {
            assert!(!text.contains(word), "{word}: {text}");
        }
    }
    fs::remove_dir_all(scratch).unwrap();
}

#[test]
fn write_scope_diagnostics_have_semantic_locations() {
    let Some(findings) = diagnostic_fixture("write-scope") else {
        return;
    };
    // 12 writes / 2 actions: the untrack-wrapped writes in the component body
    // and in a memo count (the rc.0 guard keys on the owner, not tracking),
    // while writes and the action inside createTrackedEffect no longer do
    // (children-forbidden leaf scopes are legal write regions). The two store
    // setters directly in the component body count too: the fixture resolves
    // no solid-js, so it is analyzed as the audited release, whose store
    // setter guard rejects a root owner (ADR 0127; rc.13 keeps rc.9's
    // answer, ADR 0194).
    assert_eq!(
        (
            findings_for_rule(&findings, "reactive-write-in-owned-scope").len(),
            findings_for_rule(&findings, "action-called-in-owned-scope").len(),
        ),
        (14, 2)
    );
    assert!(
        findings
            .iter()
            .filter(|finding| {
                matches!(
                    finding["rule"].as_str(),
                    Some("reactive-write-in-owned-scope" | "action-called-in-owned-scope")
                )
            })
            .all(|finding| {
                finding["primaryLocation"]["path"]
                    .as_str()
                    .is_some_and(|path| path.ends_with(".tsx"))
                    && finding["message"].as_str().is_some_and(|message| {
                        message.contains("owned scope") || message.contains("action")
                    })
            })
    );
}

#[test]
fn diagnostic_domains_match_the_solid_two_matrix() {
    for (fixture, rules) in [
        (
            "leaf-owner",
            &[
                // Three owner-backed inline violations, two dynamic-extent
                // violations reached through exact helpers (registerTeardown
                // and the transitive indirectTeardown), plus the
                // exported-helper proof obligation. The two closed local
                // callback adapters are now included: their exact returned
                // function values reach the leaf scope and report SC3001 at
                // registerTeardown(). Silent, each for its own reason: the
                // out-of-band (event handler) onSettled, the helper call
                // written inside the event handler, and the helper that only
                // builds a nested function.
                ("leaf-owner-forbidden-call", 17),
                // Three inline (the function-seeded createSignal, createMemo,
                // createRoot) plus the dynamic-extent trackDouble() reached
                // through its helper.
                // Two inline plus flushNow() reached through its helper from
                // a block-bodied and an expression-bodied leaf callback.
                // The six returns that used to be reported here are the
                // domain `EffectFunction`'s `(() => void) | void` return type
                // rejects, so the value-legality rules are gone; the fixture
                // keeps them as sources because the ownership rules still
                // classify them.
                ("invalid-cleanup-return", 0),
                ("cleanup-return-unresolved", 0),
                // The local callback adapters have exact closed returns and
                // are no longer dispatch obligations.
                ("reactive-dispatch-unresolved", 0),
            ][..],
        ),
        (
            "static-api",
            &[
                // The valid deprecated one-argument overload, the same absent
                // apply slot behind an exact one-element tuple spread, plus
                // five cast-hidden non-callable runtime values (including a
                // bad EffectBundle.effect field). Raw invalid apply arguments
                // are TypeScript's diagnostics and stay silent.
                ("missing-effect-function", 7),
                // Signal-family only: the store constructors never route
                // options.sync into their node, so their three sync: true
                // async derives are negative cases now.
                ("sync-computation-received-async", 3),
                // The refresh/affects target rules were removed on
                // 2026-08-17: `Refreshable<T>` brands the target in the type
                // system, so every spelling this fixture writes -- wrapper,
                // literal, zero-arg, value-form store, store child record,
                // accessor member chain, and a key on an accessor -- is a
                // TS2345. Pinned at 0 so a reintroduction fails here.
                ("invalid-refresh-target", 0),
                ("invalid-affects-target", 0),
                ("affects-keys-on-accessor", 0),
                ("reactive-write-in-owned-scope", 1),
            ],
        ),
        (
            "directive-phases",
            &[
                // Building a directive value happens while the component
                // renders; only invoking the returned directive is in the
                // compiler's directive-application phase.
                ("reactive-write-in-owned-scope", 2),
                // Owner-attaching creations only: the direct createMemo, the
                // forwarded createEffect, and the function-form createSignal.
                // The three value-form createSignal(element) calls allocate
                // plain state that needs no owner and stay silent.
                ("primitive-in-directive-application", 3),
            ],
        ),
        ("owner-presence", &[("missing-owner", 14)]),
        (
            "async-boundary",
            &[
                // Four untracked reads: two plain async sources, the
                // declared-loadingValue source (still reported — the declared
                // window ends at the first real answer, so later re-asks
                // throw; conditional wording), and the opaque-options source
                // (downgraded to uncertifiable, asserted below).
                ("pending-async-unsuspendable-read", 7),
                // The declared sources (loadingValue memo, seedLoadingValue
                // projection and store) render bare without any SC5003: their
                // first flight never trips a Loading boundary. The
                // opaque-options render keeps the informational warning.
                ("async-outside-loading-boundary", 12),
            ],
        ),
        (
            "ssr-client-boundary",
            &[
                // Only the bare ssrSource: "client" read outside Loading in a
                // server-rendering project; the bounded, loadingValue, and
                // seedLoadingValue reads stay silent.
                ("async-outside-loading-boundary", 1),
            ],
        ),
    ] {
        let Some(findings) = diagnostic_fixture(fixture) else {
            return;
        };
        for (rule, expected) in rules {
            assert_rule_findings(&findings, rule, *expected);
        }
    }
}

#[test]
fn settled_leaf_rules_follow_call_site_ownership() {
    let Some(findings) = diagnostic_fixture("leaf-owner") else {
        return;
    };
    let cleanup = findings_for_rule(&findings, "leaf-owner-forbidden-call")
        .into_iter()
        .filter(|finding| {
            finding["message"]
                .as_str()
                .is_some_and(|message| message.starts_with("onCleanup"))
        })
        .collect::<Vec<_>>();
    // The out-of-band onSettled (event handler) must not carry any leaf-scope
    // finding: the runtime enqueues a plain callback there.
    assert!(
        findings
            .iter()
            .filter(|finding| finding["rule"] == "leaf-owner-forbidden-call")
            .all(|finding| {
                finding["message"]
                    .as_str()
                    .is_some_and(|message| !message.contains("OutOfBand"))
            }),
        "{findings:#?}"
    );
    // The exported helper's call sites are unknowable, so its onSettled leaf
    // finding is a proof obligation, not a proven violation; the owner-backed
    // component-body ones stay violations — three inline, three reached
    // through exactly-resolved dynamic-extent helpers, and two through the
    // closed local callback adapters.
    let kinds = cleanup
        .iter()
        .map(|finding| finding["kind"].as_str().unwrap_or_default())
        .collect::<Vec<_>>();
    assert_eq!(
        kinds.iter().filter(|kind| **kind == "violation").count(),
        8,
        "{cleanup:#?}"
    );
    assert_eq!(
        kinds
            .iter()
            .filter(|kind| **kind == "uncertifiable")
            .count(),
        1,
        "{cleanup:#?}"
    );
}

#[test]
fn solid2_precision_corrections_are_end_to_end() {
    let Some(findings) = diagnostic_fixture("solid2-precision") else {
        return;
    };
    let source = include_str!("../../../../fixtures/reactive-ir/solid2-precision/App.tsx");
    let start_of = |needle: &str| {
        source
            .find(needle)
            .unwrap_or_else(|| panic!("fixture landmark {needle:?}")) as u64
    };
    let starts = |rule: &str| {
        findings_for_rule(&findings, rule)
            .iter()
            .filter_map(|finding| finding["primaryLocation"]["startByte"].as_u64())
            .collect::<Vec<_>>()
    };
    let owner_starts = |message_prefix: &str| {
        findings_for_rule(&findings, "missing-owner")
            .iter()
            .filter(|finding| {
                finding["message"]
                    .as_str()
                    .is_some_and(|message| message.starts_with(message_prefix))
            })
            .filter_map(|finding| finding["primaryLocation"]["startByte"].as_u64())
            .collect::<Vec<_>>()
    };

    // Each count pins one side of a proof. SC1002: the accessor call and the
    // store member read inside the exact `Array#filter` callback, and nothing
    // for the Promise, shadowed, unresolved, or wrapper-built callbacks.
    // The cleanup-return *value* rules are gone (every illegal return is a
    // TypeScript error against the real `EffectFunction` signature), so the
    // contextual, explicit, parenthesized, `as`-cast, member, and returned-call
    // spellings this fixture still writes are pinned through the ownership
    // rules below instead of through a legality finding.
    // SC1001/SC2003: a plain store write is a write only,
    // while the compound and update forms also read their target and a
    // computed key stays a read. SC3001/SC4001/SC4001: the one owner-backed
    // settled cleanup written as a literal callback reports SC3001 without a
    // duplicate SC4001; the wrapper-built, identifier-referenced, and
    // out-of-band cleanups are SC4001 only. The returned-call block adds three
    // SC3004 (a produced `number` in both return spellings, plus the unowned
    // one), one SC9002 (`any`), and one SC4001 (a produced function is a real
    // cleanup); a produced function, `(() => void) | undefined`, and `void`
    // are legal and silent.
    for (rule, expected) in [
        ("reactive-read-after-await", 2),
        ("invalid-cleanup-return", 0),
        ("cleanup-return-unresolved", 0),
        ("strict-read-untracked", 5),
        // Still four, but not the same four: the 2026-08-17 narrowing traded the
        // three root-record writes for the nested, cast, and props writes
        // asserted by span below. A count alone cannot tell those apart.
        ("no-direct-mutation", 4),
        ("leaf-owner-forbidden-call", 1),
        // One proven returned cleanup plus four callbacks whose runtime
        // return may be a cleanup and therefore cannot be certified safe.
        ("missing-owner", 8),
    ] {
        assert_rule_findings(&findings, rule, expected);
    }

    // The leaf rules need the literal callback *and* the call's place in its
    // own synchronous extent. Past this landmark every leaf-owner call in the
    // fixture is wrapper-built, handed over as an identifier reference, or
    // has its onCleanup in a nested function the callback merely builds — no
    // leaf scope is proven at any of them, so SC3001 stops here while the
    // genuinely unowned SC4001 continues.
    let non_literal_leaf = start_of("// `wrap` may stash");
    assert!(
        starts("leaf-owner-forbidden-call")
            .iter()
            .all(|start| *start < non_literal_leaf),
        "a leaf callback that is not the literal argument proves no leaf scope"
    );
    assert_eq!(
        owner_starts("onCleanup")
            .iter()
            .filter(|start| **start > non_literal_leaf)
            .count(),
        3,
        "the wrapped, referenced, and out-of-band cleanups stay unowned"
    );

    assert!(
        !starts("strict-read-untracked").contains(&start_of("profile.name =")),
        "a plain assignment target is a write, not a read"
    );

    // 2.0's store proxy is shallowly `Readonly`, so a write to a *root*
    // property is TS2540 and no longer this checker's. The shallowness and the
    // cast escape hatch are what remain, and each is pinned by span -- the
    // count above is satisfied by four findings either way, so only these
    // assertions distinguish the narrowing from a regression that dropped the
    // wrong three.
    for root_write in ["profile.name = ", "profile.count += ", "profile.count++"] {
        assert!(
            !starts("no-direct-mutation").contains(&start_of(root_write)),
            "a root store property is TS2540, so {root_write:?} is TypeScript's to report"
        );
    }
    for kept in [
        // The readonly-ness stops at the top level.
        "profile.user.name = ",
        // A cast erases it, so TypeScript falls silent and the write is still
        // dropped. `member_root` resolves through the cast, which is why the
        // narrowing demands a bare-identifier root.
        "(profile as { user: { name: string } }).user = ",
        // A props member is not readonly at all.
        "props.n = ",
    ] {
        assert!(
            starts("no-direct-mutation").contains(&start_of(kept)),
            "{kept:?} is a write TypeScript accepts and the checker must still report"
        );
    }
    assert!(
        starts("strict-read-untracked").contains(&start_of("props.index")),
        "a computed key inside an assignment target is still a read"
    );

    // `filter(makePredicate(post => …))` hands the arrow to a wrapper, so the
    // accessor call inside it is not proven to run before the await resumes.
    let wrapped_callback = start_of("makePredicate((post)");
    assert!(
        starts("reactive-read-after-await")
            .iter()
            .all(|start| *start < wrapped_callback),
        "a wrapper-built filter callback is not a proven synchronous read"
    );

    // A returned call is still classified from its result, not from its
    // callable callee — the ownership half of that work is what survives the
    // removal of the legality rules. `return makeCount()` produces a number,
    // so no cleanup is handed over and SC4001 must not fire; the neighbouring
    // `onSettled(() => makeThunk())` produces a function and must.
    // Anchored on the module-level spellings: the returned expression is the
    // reported span, and both callees also appear inside the component above.
    let returned_call = start_of("makeCount();\n});");
    let unowned_thunk =
        start_of("onSettled(() => makeThunk())") + u64::try_from("onSettled(".len()).unwrap();
    assert!(
        !owner_starts("onSettled").contains(&returned_call),
        "a call producing a number hands the owner no cleanup to leak"
    );
    assert!(
        owner_starts("onSettled").contains(&unowned_thunk),
        "a call producing a function is an unowned returned cleanup"
    );

    // `return nothing` where `nothing: undefined` is a legal cleanup return
    // that hands the owner nothing, so it is not a returned cleanup that would
    // make these unowned callbacks SC4001.
    let cleanup_starts = owner_starts("onSettled");
    for typed_undefined in [
        start_of("return nothing;") + u64::try_from("return ".len()).unwrap(),
        start_of("=> nothing);") + u64::try_from("=> ".len()).unwrap(),
    ] {
        assert!(
            !cleanup_starts.contains(&typed_undefined),
            "a proven-`undefined` return is not an unowned cleanup"
        );
    }
}

#[test]
fn declared_first_paint_and_opaque_options_split_the_async_rules() {
    let Some(findings) = diagnostic_fixture("async-boundary") else {
        return;
    };
    // Fail-honest policy: SC5001 stays a proven violation when the options
    // argument is absent or readable, and downgrades to a proof obligation
    // when an unreadable options argument could declare a loadingValue.
    let untracked = findings_for_rule(&findings, "pending-async-unsuspendable-read")
        .into_iter()
        .filter(|finding| finding["severity"] == "error")
        .collect::<Vec<_>>();
    let kinds = untracked
        .iter()
        .map(|finding| finding["kind"].as_str().unwrap_or_default())
        .collect::<Vec<_>>();
    assert_eq!(
        kinds.iter().filter(|kind| **kind == "violation").count(),
        3,
        "{untracked:#?}"
    );
    assert_eq!(
        kinds
            .iter()
            .filter(|kind| **kind == "uncertifiable")
            .count(),
        1,
        "{untracked:#?}"
    );
    // The declared source keeps SC5001/SC5001 with conditional wording: the
    // first flight cannot throw, later re-asks can (probed against rc.0).
    assert!(
        untracked.iter().any(|finding| {
            finding["message"].as_str().is_some_and(|message| {
                message.contains("declares a loadingValue")
                    && message.contains("after the first real answer lands")
            })
        }),
        "{untracked:#?}"
    );
    // No declared source may carry the boundary warning: the declared first
    // paint is exactly what makes a Loading boundary unnecessary.
    assert!(
        findings_for_rule(&findings, "async-outside-loading-boundary")
            .iter()
            .all(|finding| {
                finding["message"].as_str().is_some_and(|message| {
                    !message.contains("declaredFeed")
                        && !message.contains("seededUser")
                        && !message.contains("seededStoreUser")
                })
            }),
        "{findings:#?}"
    );
}

#[test]
fn props_container_and_callback_creation_do_not_prove_nested_behavior() {
    let Some(findings) = diagnostic_fixture("callee-callback-timing") else {
        return;
    };
    let props = findings
        .iter()
        .filter(|finding| {
            finding["primaryLocation"]["path"]
                .as_str()
                .is_some_and(|path| path.ends_with("/Props.tsx"))
        })
        .cloned()
        .collect::<Vec<_>>();
    let reads = findings_for_rule(&props, "strict-read-untracked");
    assert_eq!(reads.len(), 1, "{props:#?}");
    // ADR 0199: `Handler` invokes the prop only inside a `<button>` click
    // handler, so both callback-prop literals run on dispatch and report
    // nothing. The consumers that leave such a read uncertifiable are
    // pinned by `forwarded-event-prop`.
    for line in [10, 15] {
        assert!(
            !reads
                .iter()
                .any(|read| read["primaryLocation"]["line"] == line),
            "{reads:#?}"
        );
    }
    let eager = reads
        .iter()
        .find(|read| read["primaryLocation"]["line"] == 21)
        .expect("eager component-body read");
    assert_eq!(eager["kind"], "violation", "{eager:#?}");
    let mutations = findings_for_rule(&props, "no-direct-mutation");
    assert_eq!(mutations.len(), 1, "{props:#?}");
    assert_eq!(mutations[0]["primaryLocation"]["line"], 36);
    assert_eq!(mutations[0]["kind"], "violation");
}

#[test]
fn pending_callback_reads_preserve_callee_invocation_uncertainty() {
    let Some(findings) = diagnostic_fixture("callee-callback-timing") else {
        return;
    };
    let pending = findings_for_rule(&findings, "pending-async-unsuspendable-read");
    assert_eq!(pending.len(), 7, "{pending:#?}");
    assert_eq!(
        pending
            .iter()
            .filter(|finding| finding["kind"] == "violation")
            .count(),
        4,
        "{pending:#?}"
    );
    let open = pending
        .iter()
        .filter(|finding| finding["kind"] == "uncertifiable")
        .collect::<Vec<_>>();
    assert_eq!(open.len(), 3, "{pending:#?}");
    assert!(
        open.iter().all(|finding| {
            finding["message"].as_str().is_some_and(|message| {
                (message.contains("pending handling are unproven")
                    || message.contains("strict-read window is unproven"))
                    && !message.contains("throws PENDING_ASYNC_UNTRACKED_READ")
            })
        }),
        "{open:#?}"
    );
}

#[test]
fn development_models_bind_exact_sources_and_primitive_symbols() {
    let Ok(typefacts) = std::env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let fixture = root.join("fixtures/reactive-ir/callee-callback-timing");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
        .env("SOLID_TYPEFACTS_BIN", typefacts)
        .args(["--format", "json", "--feedback-facts", "--project"])
        .arg(fixture.join("tsconfig.json"))
        .output()
        .expect("run development model analysis");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let snapshot: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let models = snapshot["feedbackFacts"]
        .as_array()
        .expect("requested source models");
    let source = models
        .iter()
        .find(|model| model["path"].as_str().unwrap().ends_with("/Async.tsx"))
        .unwrap();
    let text = std::fs::read_to_string(fixture.join("Async.tsx")).unwrap();
    assert_eq!(
        source["sourceSha256"],
        solid_facts::core::SourceHash::of(&text).to_string()
    );
    let origins = source["functions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|function| !function["derivedOrigin"].is_null())
        .collect::<Vec<_>>();
    assert_eq!(origins.len(), 8, "{origins:#?}");
    for function in origins {
        let origin = &function["derivedOrigin"];
        let call = &text
            [origin["start"].as_u64().unwrap() as usize..origin["end"].as_u64().unwrap() as usize];
        assert!(
            call.starts_with("createMemo(") || call.starts_with("Solid.createMemo("),
            "{call}"
        );
        assert_eq!(function["asynchronous"], true);
    }
    let shadowed = models
        .iter()
        .find(|model| model["path"].as_str().unwrap().ends_with("/Feedback.ts"))
        .unwrap();
    assert!(
        shadowed["functions"]
            .as_array()
            .unwrap()
            .iter()
            .all(|function| function["derivedOrigin"].is_null())
    );
    let text = std::fs::read_to_string(fixture.join("Feedback.ts")).unwrap();
    for (name, expected) in [("discarded", "open"), ("used", "return-expression")] {
        let child = shadowed["functions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|function| {
                let parent = &function["parent"];
                parent["start"]
                    .as_u64()
                    .zip(parent["end"].as_u64())
                    .is_some_and(|(start, end)| {
                        text[start as usize..end as usize].starts_with(&format!("function {name}("))
                    })
            })
            .unwrap();
        assert_eq!(child["allocationRelevance"], expected);
    }
}

#[test]
fn ssr_client_hole_distinguishes_proven_and_unresolved_server_rendering() {
    let Some(findings) = diagnostic_fixture("ssr-client-boundary") else {
        return;
    };
    let holes = findings_for_rule(&findings, "async-outside-loading-boundary");
    assert_eq!(holes.len(), 1, "{findings:#?}");
    // The visible server entry proves the application server-renders, but no
    // render chain is traced from a mount root to `BadWidget` (server
    // renderers are not modelled as mount roots), so the missing boundary
    // above it is not proven: uncertifiable, never a violation.
    assert_eq!(holes[0]["kind"], "uncertifiable", "{holes:#?}");
    // The server throw is unconditional, so the rule mirrors it as an error.
    assert_eq!(holes[0]["severity"], "error", "{holes:#?}");
    assert!(
        holes[0]["message"]
            .as_str()
            .is_some_and(|message| message.contains("ssrSource: \"client\"")),
        "{holes:#?}"
    );
    // With no visible server entry, the same source is not certified safe:
    // the entry may live in another tsconfig/package.
    let Some(csr_findings) = diagnostic_fixture("ssr-client-boundary-csr") else {
        return;
    };
    let unresolved = findings_for_rule(&csr_findings, "async-outside-loading-boundary");
    assert_eq!(unresolved.len(), 1, "{csr_findings:#?}");
    assert_eq!(unresolved[0]["kind"], "uncertifiable", "{unresolved:#?}");
}

/// The wave-6 server-surface and resolve rules, pinned at their probed
/// gates: SC7005's server-render + Loading-children dominance, SC7006's
/// module-directive export shapes, SC7007's enableRichArguments silence, and
/// SC2004's and SC2005's observer-keyed scope split.
#[test]
fn server_surface_and_resolve_rules_pin_their_probed_gates() {
    if let Some(findings) = diagnostic_fixture("http-response-flush") {
        let drops = findings_for_rule(&findings, "http-response-after-flush");
        // The two component-body calls below the Loading boundary plus the
        // lexical call in its children; the shell, fallback, and
        // event-handler calls stay silent.
        assert_eq!(drops.len(), 3, "{findings:#?}");
        assert!(
            drops.iter().all(|finding| {
                finding["severity"] == "warning" && finding["kind"] == "uncertifiable"
            }),
            "the post-flush race must remain explicitly uncertifiable: {drops:#?}"
        );
        // With no visible server entry, the rendering mode is unresolved;
        // absence of the import is not proof that the app is CSR-only.
        if let Some(csr) = diagnostic_fixture("http-response-flush-csr") {
            let unresolved = findings_for_rule(&csr, "http-response-after-flush");
            assert_eq!(unresolved.len(), 2, "{csr:#?}");
            assert!(
                unresolved
                    .iter()
                    .all(|finding| finding["kind"] == "uncertifiable"),
                "{unresolved:#?}"
            );
        }
    }
    if let Some(findings) = diagnostic_fixture("server-function-directive") {
        // Two wrapped exports, one named re-export, one star re-export, one
        // wrapped default export; the direct function exports stay silent.
        assert_rule_findings(&findings, "server-function-module-directive", 5);
        assert!(
            findings.iter().all(|finding| {
                !finding["primaryLocation"]["path"]
                    .as_str()
                    .is_some_and(|path| path.ends_with("plain.ts"))
            }),
            "a module without the directive puts nothing at risk: {findings:#?}"
        );
    }
    if let Some(findings) = diagnostic_fixture("server-function-rich-args") {
        // Fifteen proven rich/non-JSON primitive values -- thirteen at the
        // top level plus two witnessed inside a closed object literal, since
        // JSON.stringify reaches nested values -- and four explicit
        // obligations. Lone/trailing Uint8Array, compiler-proven JSON-safe
        // primitive domains, and object graphs closed against spreads,
        // computed and duplicate keys, and accessors are all certified.
        //
        // The four obligations are each irreducible for a different reason,
        // and every one is pinned: a broad `number` may be non-finite; a
        // getter's body runs on access, so no written value proves what the
        // property yields; a binding referenced twice could be mutated before
        // the call; and a spread could overwrite the witness.
        assert_rule_findings(&findings, "server-function-rich-argument", 19);
        assert_eq!(
            findings_for_rule(&findings, "server-function-rich-argument")
                .iter()
                .filter(|finding| finding["kind"] == "uncertifiable")
                .count(),
            4,
            "{findings:#?}"
        );
        if let Some(enabled) = diagnostic_fixture("server-function-rich-args-enabled") {
            assert!(
                enabled.is_empty(),
                "enableRichArguments installs the codec and removes the throw everywhere: {enabled:#?}"
            );
        }
    }
    if let Some(findings) = diagnostic_fixture("resolve-scope") {
        // Memo compute, effect compute, createTrackedEffect, tracked JSX;
        // untrack, component body, event handler, apply, and module scope
        // are observer-free and stay silent (probed, rc.0).
        assert_rule_findings(&findings, "resolve-in-tracked-scope", 4);
        assert!(
            findings
                .iter()
                .all(|finding| finding["rule"] == "resolve-in-tracked-scope"),
            "{findings:#?}"
        );
    }
    if let Some(findings) = diagnostic_fixture("rc9-until-scope") {
        // rc.9's `until` carries resolve's observer guard, so the same four
        // tracked scopes throw and the same observer-free ones -- plus the
        // action step rc.9 documents -- stay silent.
        assert_rule_findings(&findings, "until-in-tracked-scope", 4);
        // rc.9 is older than the audited rc.13 (ADR 0194): one SC9014 notice
        // beside them.
        assert_eq!(
            findings_for_rule(&findings, "unaudited-solid-release").len(),
            1,
            "{findings:#?}"
        );
        assert!(
            findings.iter().all(|finding| {
                (finding["rule"] == "until-in-tracked-scope" && finding["kind"] == "violation")
                    || finding["rule"] == "unaudited-solid-release"
            }),
            "{findings:#?}"
        );
    }
    if let Some(findings) = diagnostic_fixture("rc9-static-dynamic-async") {
        // rc.9's static dynamic() form throws in dev on a thenable source and
        // renders nothing in production. Four inline async sources, the
        // standard library's Promise.resolve and Promise construct signature,
        // and two identifiers resolved to same-file async functions; the
        // synchronous, default-form, unknown-form, shadowed, parameter, `let`
        // and block-bodied sources stay silent.
        assert_rule_findings(&findings, "static-dynamic-async-source", 8);
        assert!(
            findings.iter().all(|finding| {
                (finding["rule"] == "static-dynamic-async-source" && finding["kind"] == "violation")
                    || finding["rule"] == "unaudited-solid-release"
            }),
            "{findings:#?}"
        );
        // rc.3 has no static form: the same calls are the memo's async
        // compute, which settles the Promise.
        if let Some(rc3) = diagnostic_fixture("release-triple-static-dynamic-async-rc3") {
            // Only the older-release notice (ADR 0127).
            assert!(
                rc3.iter()
                    .all(|finding| finding["rule"] == "unaudited-solid-release"),
                "{rc3:#?}"
            );
            assert_eq!(rc3.len(), 1, "{rc3:#?}");
        }
    }
    // SC2006 is keyed on the resolved @solidjs/signals: the FLUSH_IN_ACTION
    // guard ships from rc.8, so the same positives report on rc.8 and rc.9
    // and nothing reports on rc.3, where flush drains inside a
    // step as anywhere else.
    let line = |finding: &serde_json::Value| finding["primaryLocation"]["line"].as_u64();
    for (fixture, lines, notice) in [
        ("rc9-flush-in-action", &[14_u64, 20, 27, 32, 39, 44][..], 1),
        ("release-triple-flush-rc8", &[9, 15][..], 1),
        ("release-triple-flush-rc3", &[][..], 1),
    ] {
        let Some(findings) = diagnostic_fixture(fixture) else {
            continue;
        };
        let flushes = findings_for_rule(&findings, "flush-in-action");
        assert_eq!(
            flushes
                .iter()
                .map(|finding| line(finding))
                .collect::<Vec<_>>(),
            lines.iter().copied().map(Some).collect::<Vec<_>>(),
            "{fixture}: {findings:#?}"
        );
        assert!(
            flushes
                .iter()
                .all(|finding| finding["id"] == "SC2006" && finding["kind"] == "violation"),
            "{fixture}: {findings:#?}"
        );
        // Beside them only the release notice, which every one of these
        // triples gets: all are older than the audited rc.13 (ADR 0194).
        assert_eq!(
            findings_for_rule(&findings, "unaudited-solid-release").len(),
            notice,
            "{fixture}: {findings:#?}"
        );
        assert_eq!(
            findings.len(),
            lines.len() + notice,
            "{fixture}: {findings:#?}"
        );
    }
    if let Some(findings) = diagnostic_fixture("uncalled-accessor-v2") {
        // The positions TypeScript permits: a string-concatenation operand, a
        // logical-not operand, the two unary numeric coercions (`-count` and
        // `~count`, both clean against the published typings), and a template
        // interpolation. The typed positions the 2026-08-17 narrowing dropped
        // -- a class object value, a native attribute, and a computed key --
        // are each a diagnostic of TypeScript's own, and the children
        // attribute and the called and passed-on accessors were already
        // silent. Binary arithmetic and bitwise operands stay silent because
        // TypeScript rejects a function there (TS2365/TS2362).
        assert_rule_findings(&findings, "uncalled-accessor", 5);
        for position in [
            "string concatenation",
            "logical-not operator",
            "numeric coercion",
            "template literal",
        ] {
            assert!(
                findings.iter().any(|finding| {
                    finding["message"]
                        .as_str()
                        .is_some_and(|message| message.contains(position))
                }),
                "{position} is a position TypeScript permits and must stay reported: {findings:#?}"
            );
        }
        for typed in [
            "class object value",
            "native JSX attribute",
            "computed property access",
        ] {
            assert!(
                findings.iter().all(|finding| {
                    !finding["message"]
                        .as_str()
                        .is_some_and(|message| message.contains(typed))
                }),
                "{typed} is TypeScript's; reporting it duplicates a diagnostic: {findings:#?}"
            );
        }
    }
}

/// rc.9's two new callback forms whose runtime the call shape selects.
///
/// `dynamic(source, { static: true })` is `untrack(source)` at the call, so it
/// owns and writes exactly as `untrack` does; an unproven options value claims
/// nothing. `omit(props, hidden)`'s predicate runs on reads of the returned
/// view, so nothing written inside it is placed in the component body, and a
/// predicate not proven inert is uncertifiable at the predicate argument.
#[test]
fn rc9_call_forms_follow_the_runtime_they_select() {
    let line = |finding: &serde_json::Value| finding["primaryLocation"]["line"].as_u64();
    if let Some(findings) = diagnostic_fixture("rc9-dynamic-static") {
        let owners = findings_for_rule(&findings, "missing-owner");
        // StaticEffect, NamespaceStaticEffect, and the `untrack` reference.
        assert_eq!(
            owners
                .iter()
                .map(|finding| line(finding))
                .collect::<Vec<_>>(),
            [Some(20), Some(27), Some(33)],
            "{findings:#?}"
        );
        let writes = findings_for_rule(&findings, "reactive-write-in-owned-scope");
        // DefaultWrite, FalseWrite, DeferStreamWrite, BodyStaticWrite; the
        // module-scope static write and both unknown-form writes are silent.
        assert_eq!(
            writes
                .iter()
                .map(|finding| line(finding))
                .collect::<Vec<_>>(),
            [Some(50), Some(57), Some(62), Some(71)],
            "{findings:#?}"
        );
        // Seven findings, and the SC9014 notice: rc.9 is older than the
        // audited rc.13 (ADR 0194).
        assert_eq!(findings.len(), 8, "{findings:#?}");
        assert_eq!(
            findings_for_rule(&findings, "unaudited-solid-release").len(),
            1,
            "{findings:#?}"
        );
    }
    if let Some(findings) = diagnostic_fixture("rc9-omit-predicate") {
        // The only violation is the control: the body read beside a
        // predicate. No predicate body is placed in a component.
        let reads = findings_for_rule(&findings, "strict-read-untracked");
        assert_eq!(
            reads
                .iter()
                .map(|finding| line(finding))
                .collect::<Vec<_>>(),
            [Some(48)],
            "{findings:#?}"
        );
        // Every predicate not proven inert, each at its argument and each
        // uncertifiable, with the reason it is not proven: a reactive
        // operation (inline read, inline write, named, const arrow, props,
        // forwarded through `hideBy`), a call out of the predicate, or no
        // inspectable body. The inert literal, the standard-library-only
        // literal, the key list and the forwarded parameter itself are silent.
        let predicates = findings_for_rule(&findings, "reactive-dispatch-unresolved");
        fn context(finding: &serde_json::Value) -> Option<&str> {
            finding["analysisContext"]
                .as_str()
                .map(|context| context.trim_start_matches("result-access-callback-"))
        }
        assert_eq!(
            predicates
                .iter()
                .map(|finding| (line(finding), context(finding)))
                .collect::<Vec<_>>(),
            [
                (Some(20), Some("reactive-operation")),
                (Some(27), Some("reactive-operation")),
                (Some(39), Some("reactive-operation")),
                (Some(67), Some("reactive-operation")),
                (Some(73), Some("reactive-operation")),
                (Some(84), Some("opaque-call")),
                (Some(91), Some("body-unresolved")),
                (Some(107), Some("reactive-operation")),
            ],
            "{findings:#?}"
        );
        assert!(
            predicates
                .iter()
                .all(|finding| finding["kind"] == "uncertifiable"),
            "{findings:#?}"
        );
        // The exported wrapper's own open callback (its callers may be outside
        // the project), the control and the eight predicates, and the SC9014
        // notice: rc.9 is older than the audited rc.13 (ADR 0194).
        assert_eq!(
            findings_for_rule(&findings, "package-contract-incomplete")
                .iter()
                .map(|finding| line(finding))
                .collect::<Vec<_>>(),
            [Some(100)],
            "{findings:#?}"
        );
        assert_eq!(findings.len(), 11, "{findings:#?}");
        assert_eq!(
            findings_for_rule(&findings, "unaudited-solid-release").len(),
            1,
            "{findings:#?}"
        );
    }
}

#[test]
fn broadened_rule_surfaces_pin_distinct_semantic_branches() {
    let Some(async_findings) = diagnostic_fixture("async-boundary") else {
        return;
    };
    assert!(
        async_findings.iter().all(|finding| {
            !finding["primaryLocation"]["path"]
                .as_str()
                .is_some_and(|path| path.ends_with("Good.tsx"))
        }),
        "nested Loading and inline pending observers must stay clean: {async_findings:#?}"
    );

    let Some(reactivity_findings) = diagnostic_fixture("shared-reactivity-v2") else {
        return;
    };
    for (rule, expected) in [
        // Two template interpolations. The third was a native JSX attribute,
        // which TypeScript rejects on its own (TS2322) and the 2026-08-17
        // narrowing dropped.
        ("uncalled-accessor", 2),
        // The reactive `props.onSave` read, plus the two hyphenated attributes
        // TypeScript deliberately declines to check: one proven invalid and
        // one callable/non-callable proof obligation. Ordinary `onClick`
        // non-callable values remain TS2322-owned.
        ("reactive-handler-frozen", 3),
    ] {
        assert_rule_findings(&reactivity_findings, rule, expected);
    }

    let Some(unresolved_findings) = diagnostic_fixture("static-api-unresolved") else {
        return;
    };
    // These six obligations asked whether the target carries the source
    // brand, and the brand is a type (`Refreshable<T>`), so TypeScript answers
    // the question outright: an unbranded target is TS2345, a branded one type
    // checks. The obligations were removed on 2026-08-17 and are pinned at 0;
    // the fixture keeps its unresolved targets so a reintroduction is caught
    // by the same cases that used to justify them.
    assert_rule_findings(&unresolved_findings, "refresh-target-unresolved", 0);
    assert_rule_findings(&unresolved_findings, "affects-target-unresolved", 0);
}

#[test]
fn static_violation_evidence_describes_the_actual_proof() {
    let Some(static_api) = diagnostic_fixture("static-api") else {
        return;
    };
    assert!(
        findings_for_rule(&static_api, "sync-computation-received-async")
            .into_iter()
            .all(|finding| {
                finding["evidence"][0]["message"]
                    .as_str()
                    .is_some_and(|message| message.contains("sync computation"))
            })
    );
}

#[test]
fn create_effect_owner_findings_require_runtime_allocation() {
    let Some(static_api) = diagnostic_fixture("static-api") else {
        return;
    };
    let lines = findings_for_rule(&static_api, "missing-owner")
        .into_iter()
        .map(|finding| finding["primaryLocation"]["line"].as_u64().unwrap())
        .collect::<Vec<_>>();
    // Solid 2 throws before allocating an effect node for an absent apply slot
    // (including the exact one-element tuple spread), undefined, or null apply
    // argument (including cast-hidden null). Other non-callable values
    // allocate first and still create an owner leak.
    assert_eq!(lines, [12, 13, 17, 19, 20, 21, 27, 40]);
}

#[test]
fn interprocedural_diagnostics_point_to_the_calling_component() {
    for (fixture, expected_count, message) in [
        ("interprocedural", 1, "readCount"),
        ("callback-forwarding", 1, "invoke"),
        ("polymorphic", 2, "readGeneric"),
        ("recursive", 1, "readA"),
        ("returned-closure", 1, "readCount"),
        ("store-flow", 1, "\"state.count\""),
        // Four: class, generic-function, the object passed to `invoke`, and
        // the conditional with equivalent reactive summaries. The direct
        // object-member call stays unproven after the receiver-proof change;
        // this count agrees with the checked-in snapshot. Divergent and
        // computed dispatch are SC9012.
        ("interprocedural-methods-v2", 4, "count"),
    ] {
        let Some(findings) = diagnostic_fixture(fixture) else {
            return;
        };
        let strict = findings_for_rule(&findings, "strict-read-untracked");
        assert_eq!(
            strict.len(),
            expected_count,
            "fixture {fixture}: {findings:#?}"
        );
        assert!(strict.iter().any(|finding| {
            finding["message"]
                .as_str()
                .is_some_and(|text| text.contains(message))
        }));
        assert!(strict.iter().all(|finding| {
            finding["primaryLocation"]["path"]
                .as_str()
                .is_some_and(|path| path.ends_with("App.tsx"))
        }));
    }
}

#[test]
fn unknown_callback_diagnostic_contains_actionable_open_claim_context() {
    let Some(findings) = diagnostic_fixture("package-unknown-callback-producer") else {
        return;
    };
    let callback_findings = findings_for_rule(&findings, "package-contract-incomplete");
    assert_eq!(callback_findings.len(), 1, "{findings:#?}");
    let finding = &callback_findings[0];
    let message = finding["message"].as_str().unwrap_or_default();
    let hint = finding["hint"].as_str().unwrap_or_default();
    assert!(message.contains("current project.:schedule"), "{message}");
    assert!(message.contains("parameter 0 (() => void)"), "{message}");
    assert!(hint.contains("solid-checker-open-contract-claim"), "{hint}");
    assert!(hint.contains("semanticModelVersion\":1"), "{hint}");
    assert!(!hint.contains("schemaVersion\":1"), "{hint}");
    assert!(hint.contains("choose exactly one audited mode"), "{hint}");
    assert!(hint.contains("solid-checker contract generate"), "{hint}");
}

#[test]
fn control_flow_and_effect_phases_classify_strict_reads() {
    // control-flow: the two frozen Show-callback reads, plus the two frozen
    // reads under <For keyed={byId}> — a named key function proven callable
    // through type facts keeps the custom-key accessor claims. The dynamic
    // boolean `keyed={flag()}` functions contribute nothing: the callback
    // shape is ambiguous, so neither parameter is claimed as an accessor.
    for (fixture, expected) in [("control-flow", 4), ("execution-phases", 1)] {
        let Some(findings) = diagnostic_fixture(fixture) else {
            return;
        };
        assert_eq!(
            findings_for_rule(&findings, "strict-read-untracked").len(),
            expected,
            "fixture {fixture}: {findings:#?}"
        );
    }
}

/// ADR 0202: the default output lists findings about the user's code in full
/// and folds the analysis-coverage gaps into one grouped section; a closed
/// program reports no contract-generation obligation for its own exports.
#[test]
fn default_output_groups_coverage_gaps_after_findings_to_review() {
    let Ok(typefacts) = std::env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let run = |fixture: &str, format: &str| {
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
            .env("SOLID_TYPEFACTS_BIN", &typefacts)
            .env("SOLID_CHECKER_DAEMON", "0")
            .args(["--format", format, "--project"])
            .arg(
                root.join("fixtures/reactive-ir")
                    .join(fixture)
                    .join("tsconfig.json"),
            )
            .output()
            .expect("run the checker");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    };

    let open = run("feedback-tiers-open", "default");
    let review = open.find("Needs review (1)").expect("review heading");
    let coverage = open
        .find("Analysis coverage: 2 sites in 2 groups")
        .expect("coverage heading");
    assert!(review < coverage, "{open}");
    // A coverage gap is grouped by family and subject, with its first site,
    // not a frame (ADR 0205).
    assert!(
        open.contains(
            "[SC9012] Helpers that call a method on a value their caller supplies: 1 site in 1 helper"
        ),
        "{open}"
    );
    assert!(open.contains("      ageOf (1)\n"), "{open}");
    assert!(open.contains("first at App.tsx:18:21"), "{open}");
    assert!(
        open.contains("[SC9005] Callbacks this project's exports hand to code with unknown timing: 1 site in 1 export"),
        "{open}"
    );
    assert!(!open[coverage..].contains(",-["), "{open}");
    // `--format full` keeps every finding in place.
    let full = run("feedback-tiers-open", "full");
    assert!(!full.contains("Analysis coverage"), "{full}");
    assert!(full.contains("[SC9005] callback parameter 0"), "{full}");

    // The same source in a closed program: no contract-generation obligation.
    let closed = run("feedback-tiers", "default");
    assert!(
        closed.contains("Analysis coverage: 1 site in 1 group"),
        "{closed}"
    );
    assert!(!closed.contains("SC9005"), "{closed}");
}
