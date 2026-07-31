use std::{env, fs, path::PathBuf, process::Command};

use solid_facts_backend::{SemanticDemandGroup, TypeFactsProvider, TypeFactsSession};
use typefacts::v3::FileChange;

#[test]
fn timing_lines_are_valid_json() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let output = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
        .args(["--format", "json", "--project"])
        .arg(root.join("fixtures/reactive-ir/tracer/tsconfig.json"))
        .args(["--typefacts", &typefacts])
        .env("SOLID_CHECKER_TIMINGS", "1")
        .output()
        .expect("run Rust CLI with timings");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8(output.stderr).expect("timings are UTF-8");
    let timings = stderr
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("valid timing JSON"))
        .collect::<Vec<_>>();
    assert!(
        timings
            .iter()
            .any(|timing| timing["reactiveIrStage"].is_string()),
        "expected reactive IR stage timings: {timings:#?}"
    );
}

#[cfg(unix)]
#[test]
fn cli_rejects_a_mismatched_typefacts_build() {
    use std::os::unix::fs::PermissionsExt;

    let directory = env::temp_dir().join(format!("solid-checker-handshake-{}", std::process::id()));
    fs::create_dir_all(&directory).unwrap();
    let service = directory.join("mismatched-typefacts");
    let handshake = typefacts::v3::Handshake {
        protocol: typefacts::v3::TYPE_FACTS_HANDSHAKE_PROTOCOL,
        schema_hash: typefacts::v3::TYPE_FACTS_SCHEMA_SHA256.into(),
        build_id: "definitely-not-this-engine".into(),
    };
    let payload = typefacts::encode(&handshake).unwrap();
    let mut frame = u32::try_from(payload.len()).unwrap().to_le_bytes().to_vec();
    frame.extend(payload);
    let escaped = frame
        .iter()
        .map(|byte| format!("\\{byte:03o}"))
        .collect::<String>();
    fs::write(
        &service,
        format!("#!/bin/sh\nprintf '{escaped}'\ncat >/dev/null\n"),
    )
    .unwrap();
    fs::set_permissions(&service, fs::Permissions::from_mode(0o755)).unwrap();

    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let output = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
        .args(["--typefacts"])
        .arg(&service)
        .args(["--project"])
        .arg(root.join("fixtures/reactive-ir/tracer/tsconfig.json"))
        .output()
        .expect("run Rust CLI with mismatched service");
    assert_eq!(output.status.code(), Some(3));
    assert!(String::from_utf8_lossy(&output.stderr).contains("compatibility handshake failed"));
    fs::remove_dir_all(directory).unwrap();
}

/// The raw v3 wire — framing, packed tables, deltas, compact demands — is the
/// `typefacts` crate's own test surface. What belongs here is the checker's use
/// of it: open a real producer, read the configured sources, apply an overlay,
/// and analyse the retained project.
#[test]
fn retained_session_serves_sources_and_facts_for_the_project() {
    let executable = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let fixture = root.join("fixtures/reactive-ir/tracer");
    let project = fixture.join("tsconfig.json").canonicalize().unwrap();
    let project_id = project.to_string_lossy().into_owned();
    let mut session = TypeFactsSession::open(&executable, &project_id, &[]).unwrap();

    let sources = session.configured_sources().expect("configured sources");
    assert!(
        sources
            .iter()
            .any(|source| source.path.ends_with("App.tsx") && !source.source.is_empty()),
        "the session hydrates configured sources: {sources:#?}"
    );

    let table = session
        .semantic_grouped(&[] as &[SemanticDemandGroup<'_>])
        .expect("analyze the opened generation");
    assert_eq!(table.project_id(), project_id);
    assert!(table.sources().next().is_some());

    let app = fixture.join("App.tsx").canonicalize().unwrap();
    session
        .update(vec![FileChange {
            path: app.to_string_lossy().into_owned(),
            version: 1,
            source: fs::read(&app).unwrap(),
            deleted: false,
        }])
        .expect("apply an overlay");
    assert_eq!(session.generation(), 2);

    let updated = session
        .semantic_grouped(&[] as &[SemanticDemandGroup<'_>])
        .expect("analyze the new generation");
    assert_eq!(updated.generation(), 2);
}
