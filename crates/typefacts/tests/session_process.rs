use std::{fs, path::PathBuf};

use typefacts::{AnalysisDemand, Producer, Session, v3::FileChange};

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn producer() -> PathBuf {
    std::env::var_os("TYPEFACTS_TEST_BIN")
        .map(PathBuf::from)
        .unwrap_or_else(|| repository_root().join("bin/solid-typefacts"))
}

fn project() -> PathBuf {
    repository_root()
        .join("internal/typefacts/testdata/aliased-import/tsconfig.json")
        .canonicalize()
        .unwrap()
}

#[test]
fn public_session_owns_the_retained_process_lifecycle() {
    let producer = producer();
    assert!(
        producer.is_file(),
        "build the test producer at {} or set TYPEFACTS_TEST_BIN",
        producer.display()
    );
    let project = project();

    let mut session = Session::open(
        Producer::at(producer),
        project.to_string_lossy(),
        Vec::new(),
    )
    .unwrap();
    let sources = session.configured_sources().unwrap();
    assert!(
        sources
            .iter()
            .any(|source| source.path.ends_with("consumer.ts"))
    );

    let first = session.analyze(&AnalysisDemand::default()).unwrap();
    assert_eq!(first.generation, 1);
    assert_eq!(first.project_id, project.to_string_lossy());

    let changed_path = project.parent().unwrap().join("unrelated.ts");
    let changed_source = fs::read(&changed_path).unwrap();
    session
        .update([FileChange {
            path: changed_path.to_string_lossy().into_owned(),
            source: changed_source,
            deleted: false,
            version: 1,
        }])
        .unwrap();
    let second = session.analyze(&AnalysisDemand::default()).unwrap();
    assert_eq!(second.generation, 2);
    session.close().unwrap();
}

#[cfg(unix)]
#[test]
fn analyze_restarts_the_producer_and_replays_updates_after_a_crash() {
    use std::{os::unix::fs::PermissionsExt, process::Command};

    let directory =
        std::env::temp_dir().join(format!("typefacts-session-crash-{}", std::process::id()));
    fs::create_dir_all(&directory).unwrap();
    let pid_path = directory.join("producer.pid");
    let wrapper = directory.join("producer");
    fs::write(
        &wrapper,
        format!(
            "#!/bin/sh\nprintf '%s' \"$$\" > '{}'\nexec '{}' \"$@\"\n",
            pid_path.display(),
            producer().display()
        ),
    )
    .unwrap();
    fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o755)).unwrap();

    let project = project();
    let mut session = Session::open(
        Producer::at(&wrapper),
        project.to_string_lossy(),
        Vec::new(),
    )
    .unwrap();
    let changed_path = project.parent().unwrap().join("unrelated.ts");
    session
        .update([FileChange {
            path: changed_path.to_string_lossy().into_owned(),
            source: fs::read(&changed_path).unwrap(),
            deleted: false,
            version: 1,
        }])
        .unwrap();

    let pid = fs::read_to_string(&pid_path).unwrap();
    assert!(
        Command::new("kill")
            .args(["-9", &pid])
            .status()
            .unwrap()
            .success()
    );
    let facts = session.analyze(&AnalysisDemand::default()).unwrap();
    assert_eq!(facts.generation, 2);
    session.close().unwrap();

    fs::remove_file(wrapper).unwrap();
    fs::remove_file(pid_path).unwrap();
    fs::remove_dir(directory).unwrap();
}
