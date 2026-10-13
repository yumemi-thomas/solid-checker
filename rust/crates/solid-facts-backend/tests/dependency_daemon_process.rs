//! ADR 0296: a retained dependency-source check (ADRs 0284, 0291) answers
//! exactly what a fresh one-shot check answers: the same canonical JSON and
//! the same notes, cold, warm, and after each kind of input change.
//!
//! Every case runs on a copy of a fixture in a temporary directory, never on
//! the checked-in tree: a live daemon owns a sibling tsconfig in its project's
//! directory.

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

const FIXTURES: &[&str] = &[
    "dependency-source",
    "dependency-source-all",
    "dependency-source-context-getter",
    "dependency-source-context-reprovided",
    "dependency-source-default-include",
    "dependency-source-owner",
    "dependency-source-program",
    "dependency-source-proxy-store",
];

fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &to.join(entry.file_name()));
        } else {
            fs::copy(entry.path(), to.join(entry.file_name())).unwrap();
        }
    }
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../fixtures/reactive-ir")
        .join(name)
}

/// A private copy of `name`, removed when dropped.
struct Scratch {
    root: PathBuf,
    project: PathBuf,
}

impl Scratch {
    fn new(name: &str, tag: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "solid-checker-dependency-daemon-{tag}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        copy_tree(&fixture(name), &root.join("project"));
        let project = fs::canonicalize(root.join("project")).unwrap();
        Self { root, project }
    }

    fn path(&self, relative: &str) -> PathBuf {
        self.project.join(relative)
    }

    fn siblings(&self) -> Vec<String> {
        fs::read_dir(&self.project)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|name| name.starts_with(".solid-checker-dependency-sources."))
            .collect()
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

struct Run {
    stdout: Vec<u8>,
    stderr: String,
}

impl Run {
    /// The daemon's own report of whether it answered from its cached
    /// snapshot; `None` for a one-shot check.
    fn cache_hit(&self) -> Option<bool> {
        self.stderr.lines().find_map(|line| {
            let value: serde_json::Value = serde_json::from_str(line).ok()?;
            (value["mode"] == "retained-daemon").then(|| value["cacheHit"] == true)
        })
    }

    /// The retained program's generation: 1 after the analyzed program was
    /// opened, one more for every edit it took instead.
    fn generation(&self) -> Option<u64> {
        self.stderr.lines().find_map(|line| {
            let value: serde_json::Value = serde_json::from_str(line).ok()?;
            (value["mode"] == "retained-daemon").then(|| value["generation"].as_u64())?
        })
    }

    fn notes(&self) -> Vec<&str> {
        self.stderr
            .lines()
            .filter(|line| line.starts_with("solid-checker: note:"))
            .collect()
    }

    fn json(&self) -> serde_json::Value {
        serde_json::from_slice(&self.stdout).unwrap()
    }
}

fn check(typefacts: &str, project: &Path, selection: &[&str], retained: bool) -> Run {
    let mut command = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"));
    command
        .env("SOLID_TYPEFACTS_BIN", typefacts)
        .env("SOLID_CHECKER_DAEMON", if retained { "1" } else { "0" })
        .env("SOLID_CHECKER_DAEMON_IDLE_SECS", "20")
        .env("SOLID_CHECKER_TIMINGS", "1")
        .args(["--format", "json", "--project"])
        .arg(project.join("tsconfig.json"))
        .args([
            "--runtime-target",
            "browser",
            "--runtime-build",
            "development",
        ]);
    for specifier in selection {
        command.args(["--dependency-source", specifier]);
    }
    let output = command.output().unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(output.status.success(), "{stderr}");
    assert!(!stderr.contains("daemon unavailable"), "{stderr}");
    assert_eq!(
        retained,
        stderr.contains("\"mode\":\"retained-daemon\""),
        "{stderr}"
    );
    Run {
        stdout: output.stdout,
        stderr,
    }
}

fn assert_same(retained: &Run, fresh: &Run, context: &str) {
    assert_eq!(
        String::from_utf8_lossy(&retained.stdout),
        String::from_utf8_lossy(&fresh.stdout),
        "{context}: retained and one-shot JSON differ"
    );
    assert_eq!(retained.notes(), fresh.notes(), "{context}: notes differ");
}

fn selection_of(name: &str) -> Vec<String> {
    let runtime: serde_json::Value = serde_json::from_slice(
        &fs::read(fixture(name).join(".solid-checker/runtime.json")).unwrap(),
    )
    .unwrap();
    runtime["dependencySources"]
        .as_array()
        .unwrap()
        .iter()
        .map(|specifier| specifier.as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn a_retained_dependency_source_check_is_the_one_shot_check() {
    let Ok(typefacts) = std::env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    for name in FIXTURES {
        let listed = selection_of(name);
        let selections = [
            listed.iter().map(String::as_str).collect::<Vec<_>>(),
            vec!["*"],
        ];
        for (index, selection) in selections.iter().enumerate() {
            let scratch = Scratch::new(name, &format!("same-{name}-{index}"));
            let context = format!("{name} {selection:?}");
            let fresh = check(&typefacts, &scratch.project, selection, false);
            assert!(
                scratch.siblings().is_empty(),
                "{context}: one-shot left its sibling"
            );
            let cold = check(&typefacts, &scratch.project, selection, true);
            assert_eq!(cold.cache_hit(), Some(false), "{context}");
            let warm = check(&typefacts, &scratch.project, selection, true);
            assert_eq!(warm.cache_hit(), Some(true), "{context}");
            assert_same(&cold, &fresh, &context);
            assert_same(&warm, &fresh, &context);
            // The daemon's sibling is named by its bytes, not by a process.
            let siblings = scratch.siblings();
            assert!(siblings.len() <= 1, "{context}: {siblings:?}");
            for sibling in siblings {
                let key = sibling
                    .trim_start_matches(".solid-checker-dependency-sources.")
                    .trim_end_matches(".tsconfig.json");
                assert_eq!(key.len(), 64, "{context}: {sibling}");
            }
        }
    }
}

/// How a retained daemon took a change.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Taken {
    /// As an edit of the retained analyzed program.
    Edit,
    /// By opening the analyzed program again.
    Reopened,
}

/// Cold daemon, `mutate`, then two retained checks and a forced fresh one:
/// both retained answers are the fresh answer, the second from the cache,
/// the daemon took the change as `taken` says, and the mutation moved
/// something, so the agreement is not vacuous.
fn mutation_case(
    typefacts: &str,
    name: &str,
    tag: &str,
    selection: &[&str],
    taken: Taken,
    prepare: impl FnOnce(&Scratch),
    mutate: impl FnOnce(&Scratch),
) {
    let scratch = Scratch::new(name, tag);
    prepare(&scratch);
    let before = check(typefacts, &scratch.project, selection, true);
    let warm = check(typefacts, &scratch.project, selection, true);
    assert_eq!(warm.cache_hit(), Some(true), "{tag}");
    mutate(&scratch);
    let retained = check(typefacts, &scratch.project, selection, true);
    assert_eq!(retained.cache_hit(), Some(false), "{tag}");
    assert_eq!(
        retained.generation(),
        Some(match taken {
            Taken::Edit => 2,
            Taken::Reopened => 1,
        }),
        "{tag}: {}",
        retained.stderr
    );
    let again = check(typefacts, &scratch.project, selection, true);
    assert_eq!(again.cache_hit(), Some(true), "{tag}");
    let fresh = check(typefacts, &scratch.project, selection, false);
    assert_same(&retained, &fresh, tag);
    assert_same(&again, &fresh, tag);
    assert_ne!(
        before.json()["findings"],
        fresh.json()["findings"],
        "{tag}: the mutation moved nothing"
    );
}

fn edit(path: &Path, from: &str, to: &str) {
    let text = fs::read_to_string(path).unwrap();
    assert!(text.contains(from), "{}: {from:?}", path.display());
    fs::write(path, text.replacen(from, to, 1)).unwrap();
}

#[test]
fn an_edited_package_runtime_file_is_the_one_shot_answer() {
    let Ok(typefacts) = std::env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    mutation_case(
        &typefacts,
        "dependency-source",
        "package-edit",
        &["dep-hooks"],
        Taken::Edit,
        |_| {},
        |scratch| {
            edit(
                &scratch.path("node_modules/dep-hooks/src/read.js"),
                "return source();",
                "return () => source();",
            );
        },
    );
}

#[test]
fn a_nearer_mjs_beside_a_js_closure_file_is_the_one_shot_answer() {
    let Ok(typefacts) = std::env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let mjs = "node_modules/dep-ext/src/read.mjs";
    mutation_case(
        &typefacts,
        "dependency-source-all",
        "mjs",
        &["*"],
        Taken::Reopened,
        |scratch| fs::remove_file(scratch.path(mjs)).unwrap(),
        |scratch| {
            fs::copy(
                fixture("dependency-source-all").join(mjs),
                scratch.path(mjs),
            )
            .unwrap();
        },
    );
}

#[test]
fn a_changed_exports_map_is_the_one_shot_answer() {
    let Ok(typefacts) = std::env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    mutation_case(
        &typefacts,
        "dependency-source",
        "exports",
        &["dep-hooks"],
        Taken::Reopened,
        |_| {},
        |scratch| {
            edit(
                &scratch.path("node_modules/dep-hooks/package.json"),
                "\"solid\": \"./src/index.jsx\",",
                "",
            );
        },
    );
}

#[test]
fn an_edited_project_file_is_the_one_shot_answer() {
    let Ok(typefacts) = std::env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    for selection in [&["dep-hooks"][..], &["*"][..]] {
        mutation_case(
            &typefacts,
            "dependency-source",
            &format!("project-edit-{}", selection[0].replace('*', "all")),
            selection,
            Taken::Edit,
            |_| {},
            |scratch| {
                edit(
                    &scratch.path("App.tsx"),
                    "const label = useLabel(name);",
                    "const label = useLabelMemo(name);",
                );
            },
        );
    }
}

#[test]
fn an_edited_host_fold_runtime_target_is_the_one_shot_answer() {
    let Ok(typefacts) = std::env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    // Not a program file: only the recomputed fold sees it.
    mutation_case(
        &typefacts,
        "dependency-source-proxy-store",
        "host-fold",
        &["dep-query"],
        Taken::Reopened,
        |_| {},
        |scratch| {
            edit(
                &scratch.path("node_modules/@solidjs/web/dist/web.dev.js"),
                "const isServer = false;",
                "const isServer = true;",
            );
        },
    );
}

#[test]
fn an_edited_folded_package_file_is_the_one_shot_answer() {
    let Ok(typefacts) = std::env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    // An edit carries the program's own text, so a file whose old or new
    // text folds is never taken as one.
    mutation_case(
        &typefacts,
        "dependency-source-proxy-store",
        "folded-edit",
        &["dep-query"],
        Taken::Reopened,
        |_| {},
        |scratch| {
            edit(
                &scratch.path("node_modules/dep-query/src/index.js"),
                "      if (isServer) {",
                "      if (!isServer) {",
            );
        },
    );
}
