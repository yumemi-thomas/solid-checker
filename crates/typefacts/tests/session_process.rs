use std::{fs, path::PathBuf, process::Command, sync::OnceLock};

use typefacts::{AnalysisDemand, Producer, Session, v3::FileChange};

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn producer() -> PathBuf {
    static PRODUCER: OnceLock<PathBuf> = OnceLock::new();
    PRODUCER
        .get_or_init(|| {
            if let Some(path) = std::env::var_os("TYPEFACTS_TEST_BIN") {
                return PathBuf::from(path);
            }
            let output = repository_root()
                .join("target/typefacts-test")
                .join(if cfg!(windows) {
                    "solid-typefacts.exe"
                } else {
                    "solid-typefacts"
                });
            fs::create_dir_all(output.parent().unwrap()).unwrap();
            let ldflags = format!("-X main.buildID={}", typefacts::v3::TYPE_FACTS_BUILD_ID);
            let status = Command::new("go")
                .current_dir(repository_root())
                .args(["build", "-ldflags", &ldflags, "-o"])
                .arg(&output)
                .arg("./cmd/solid-typefacts")
                .status()
                .expect("run go build for the session process test");
            assert!(status.success(), "build the Type Facts test producer");
            output
        })
        .clone()
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
    let timings = session.take_last_exchange_timings().unwrap();
    assert!(!timings.roundtrip.is_zero());
    assert!(timings.response_bytes > 0);
    assert!(timings.server_materialized);
    assert!(session.take_last_table_changes().is_some());

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
    assert!(session.take_last_exchange_timings().is_some());
    assert!(session.take_last_table_changes().is_some());
    session.close().unwrap();
}

#[cfg(unix)]
#[test]
fn analyze_restarts_the_producer_and_replays_updates_after_a_crash() {
    use std::os::unix::fs::PermissionsExt;

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
