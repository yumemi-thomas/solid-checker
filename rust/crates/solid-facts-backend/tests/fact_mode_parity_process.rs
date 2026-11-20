//! ADR 1135: a run with feedback facts (the fuzz judge's mode) reads the program a user's run reads. With a declared host, both
//! take ADR 0297's default dependency-source selection, so their findings are equal; the feedback models are the project's own
//! files' only. The fixture's package names `solid-js` and patches `Array.prototype` under a run-time key: read as source, it
//! makes the standard `at` unproven (SC9012); unread, it does not, which is how the two modes differed before.
use std::{env, path::Path, process::Command};

fn report(typefacts: &str, project: &Path, feedback: bool) -> serde_json::Value {
    let mut command = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"));
    command
        .env("SOLID_TYPEFACTS_BIN", typefacts)
        .env("SOLID_CHECKER_DAEMON", "0")
        .args(["--format", "json", "--project"])
        .arg(project.join("tsconfig.json"))
        .args([
            "--runtime-target",
            "browser",
            "--runtime-build",
            "development",
            "--rendering",
            "csr",
        ]);
    if feedback {
        command.arg("--feedback-facts");
    }
    let output = command.output().unwrap();
    assert!(
        output.status.success() || output.status.code() == Some(1),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn feedback_facts_read_the_program_a_users_run_reads() {
    let typefacts = env::var("SOLID_TYPEFACTS_BIN").expect("arm the real Type Facts process");
    let project = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../fixtures/reactive-ir/fact-mode-parity");
    let user = report(&typefacts, &project, false);
    let judge = report(&typefacts, &project, true);
    let kept = |report: &serde_json::Value| serde_json::json!({ "status": report["status"], "findings": report["findings"], "hazards": report["hazards"] });
    assert_eq!(
        kept(&judge),
        kept(&user),
        "a run with feedback facts must give a user's findings"
    );
    // The package was read: its patch makes the standard call unproven in both.
    let findings = user["findings"].as_array().unwrap();
    assert!(
        findings.iter().any(|finding| finding["id"] == "SC9012"
            && finding["analysisContext"]
                .as_str()
                .is_some_and(|context| context.starts_with("standard-member-impostor"))),
        "{findings:#?}"
    );
    // The feedback models are the project's own files', never a package's.
    let models = judge["feedbackFacts"].as_array().unwrap();
    assert!(
        models.iter().any(|model| model["path"]
            .as_str()
            .is_some_and(|path| path.ends_with("App.tsx"))),
        "{models:#?}"
    );
    assert!(
        models.iter().all(|model| !model["path"]
            .as_str()
            .unwrap_or_default()
            .contains("/node_modules/")),
        "{models:#?}"
    );
}
