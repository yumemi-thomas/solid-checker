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
        let root = std::env::temp_dir()
            .join("sc-scratch.noindex")
            .join(format!(
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

    /// Installs a manifest-only `package` at `node_modules/<package>`.
    fn install(&self, package: &str) {
        let directory = self.project.join("node_modules").join(package);
        fs::create_dir_all(&directory).unwrap();
        let name = package.rsplit("node_modules/").next().unwrap();
        fs::write(
            directory.join("package.json"),
            format!(r#"{{"name":"{name}","version":"3.0.0-next.44"}}"#),
        )
        .unwrap();
    }

    /// ADR 0297 review round 1: the Solid Vite plugin that establishes the
    /// resolver the default selection reads.
    fn with_solid_vite_plugin(self) -> Self {
        self.install("@solidjs/vite-plugin");
        self
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

fn command(typefacts: &str, project: &Path, selection: &[&str], retained: bool) -> Command {
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
    command
}

fn check(typefacts: &str, project: &Path, selection: &[&str], retained: bool) -> Run {
    finish(command(typefacts, project, selection, retained), retained)
}

/// Run `command`, which must succeed, through a daemon iff `retained`.
fn finish(mut command: Command, retained: bool) -> Run {
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

/// ADR 0297: one check with exactly `args` after the project, retained or
/// one-shot, whatever its exit status.
fn invoke(typefacts: &str, project: &Path, args: &[&str], retained: bool) -> (bool, Run) {
    let output = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
        .env("SOLID_TYPEFACTS_BIN", typefacts)
        .env("SOLID_CHECKER_DAEMON", if retained { "1" } else { "0" })
        .env("SOLID_CHECKER_DAEMON_IDLE_SECS", "20")
        .env("SOLID_CHECKER_TIMINGS", "1")
        .args(["--format", "json", "--project"])
        .arg(project.join("tsconfig.json"))
        .args(args)
        .output()
        .unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(!stderr.contains("daemon unavailable"), "{stderr}");
    (
        output.status.success(),
        Run {
            stdout: output.stdout,
            stderr,
        },
    )
}

const HOST: [&str; 4] = [
    "--runtime-target",
    "browser",
    "--runtime-build",
    "development",
];

fn with_host(rest: &[&'static str]) -> Vec<&'static str> {
    HOST.iter().chain(rest).copied().collect()
}

fn source_notes(run: &Run) -> Vec<&str> {
    run.notes()
        .into_iter()
        .filter(|note| note.contains("dependency source"))
        .collect()
}

#[test]
fn a_declared_host_analyzes_every_solid_package_from_source_by_default() {
    let Ok(typefacts) = std::env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let scratch = Scratch::new("dependency-source-all", "default-source").with_solid_vite_plugin();
    let ok = |args: &[&str]| {
        let (success, run) = invoke(&typefacts, &scratch.project, args, false);
        assert!(success, "{args:?}: {}", run.stderr);
        run
    };
    let explicit = ok(&with_host(&["--dependency-source", "*"]));
    let defaulted = ok(&HOST);
    assert_same(&defaulted, &explicit, "the default is `*`");
    // `dep-array`'s `exports` array is refused with a note naming it: the
    // selection ran.
    assert!(
        source_notes(&defaulted)
            .iter()
            .any(|note| note.contains("dep-array")),
        "{}",
        defaulted.stderr
    );

    // The opt-out is contract mode, and contract mode answers differently
    // here, so the agreement above is not vacuous.
    let opted_out = ok(&with_host(&["--no-dependency-source"]));
    assert!(source_notes(&opted_out).is_empty(), "{}", opted_out.stderr);
    assert_ne!(
        String::from_utf8_lossy(&opted_out.stdout),
        String::from_utf8_lossy(&defaulted.stdout),
        "contract mode answers differently here"
    );

    // Without both a target and a build nothing is selected: the answer is
    // the opted-out one, without a note.
    for partial in [
        &["--runtime-target", "browser"][..],
        &["--runtime-build", "development"][..],
        &[][..],
    ] {
        let run = ok(partial);
        assert!(source_notes(&run).is_empty(), "{partial:?}: {}", run.stderr);
        let mut opted = partial.to_vec();
        opted.push("--no-dependency-source");
        assert_same(&run, &ok(&opted), &format!("{partial:?} is contract mode"));
    }

    // Contract inspection keeps contracts under a declared host. It exits 1
    // for the packages here that have none.
    let inspect = |args: &[&str]| invoke(&typefacts, &scratch.project, args, false);
    let (inspected_ok, inspected) = inspect(&with_host(&["--check-contracts"]));
    assert!(source_notes(&inspected).is_empty(), "{}", inspected.stderr);
    let (opted_ok, opted) = inspect(&with_host(&["--check-contracts", "--no-dependency-source"]));
    assert_eq!(inspected_ok, opted_ok);
    assert!(!inspected.stdout.is_empty(), "{}", inspected.stderr);
    assert_same(&inspected, &opted, "--check-contracts");

    // The opt-out and a selection contradict each other.
    let (success, refused) = invoke(
        &typefacts,
        &scratch.project,
        &with_host(&["--no-dependency-source", "--dependency-source", "*"]),
        false,
    );
    assert!(!success);
    assert!(
        refused
            .stderr
            .contains("--no-dependency-source cannot be combined with --dependency-source"),
        "{}",
        refused.stderr
    );
}

#[test]
fn a_retained_default_check_is_the_one_shot_check() {
    let Ok(typefacts) = std::env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let scratch =
        Scratch::new("dependency-source-all", "default-retained").with_solid_vite_plugin();
    let fresh = invoke(&typefacts, &scratch.project, &HOST, false).1;
    let (cold_ok, cold) = invoke(&typefacts, &scratch.project, &HOST, true);
    let (warm_ok, warm) = invoke(&typefacts, &scratch.project, &HOST, true);
    assert!(cold_ok && warm_ok, "{}\n{}", cold.stderr, warm.stderr);
    assert_eq!(cold.cache_hit(), Some(false), "{}", cold.stderr);
    assert_eq!(warm.cache_hit(), Some(true), "{}", warm.stderr);
    assert_same(&cold, &fresh, "default cold");
    assert_same(&warm, &fresh, "default warm");
    // The retained default is the explicit `*` selection's daemon.
    let explicit = invoke(
        &typefacts,
        &scratch.project,
        &with_host(&["--dependency-source", "*"]),
        true,
    )
    .1;
    assert_eq!(explicit.cache_hit(), Some(true), "{}", explicit.stderr);
    assert_same(&explicit, &fresh, "explicit `*` against the default");
    // An opted-out retained check is a contract-mode daemon's answer.
    let opted_out = with_host(&["--no-dependency-source"]);
    let retained = invoke(&typefacts, &scratch.project, &opted_out, true).1;
    let one_shot = invoke(&typefacts, &scratch.project, &opted_out, false).1;
    assert!(retained.cache_hit().is_some(), "{}", retained.stderr);
    assert_same(&retained, &one_shot, "opted out");
    assert!(source_notes(&retained).is_empty(), "{}", retained.stderr);
}

// Review round 1 of ADR 0296.

/// Finding 1: a refresh that fails part-way has already read the edits it
/// failed on. The request after it must still answer them.
#[test]
fn an_aborted_refresh_does_not_consume_its_edits() {
    let Ok(typefacts) = std::env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    for selection in [&["dep-hooks"][..], &[][..]] {
        let tag = format!("aborted-{}", selection.len());
        let scratch = Scratch::new("dependency-source", &tag);
        // A second project file, after `App.tsx` in the order a refresh
        // reads them.
        fs::write(scratch.path("Zed.tsx"), "export {};\n").unwrap();
        let before = check(&typefacts, &scratch.project, selection, true);
        let warm = check(&typefacts, &scratch.project, selection, true);
        assert_eq!(warm.cache_hit(), Some(true), "{tag}");
        edit(
            &scratch.path("App.tsx"),
            "const label = useLabel(name);",
            "const label = useLabelMemo(name);",
        );
        // Broken in place, so no directory stamp moves: the dependency's
        // manifest, which preparation reads after both programs took the
        // edit, or a project file the refresh reads after `App.tsx` and
        // refuses as not UTF-8.
        let broken = if selection.is_empty() {
            scratch.path("Zed.tsx")
        } else {
            scratch.path("node_modules/dep-hooks/package.json")
        };
        let original = fs::read(&broken).unwrap();
        if selection.is_empty() {
            fs::write(&broken, b"export {};\n// \xff\xfe\n").unwrap();
        } else {
            fs::write(&broken, b"{ not json").unwrap();
        }
        let failed = command(&typefacts, &scratch.project, selection, true)
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&failed.stderr);
        assert!(stderr.contains("daemon unavailable"), "{tag}: {stderr}");
        fs::write(&broken, &original).unwrap();
        let recovered = check(&typefacts, &scratch.project, selection, true);
        let fresh = check(&typefacts, &scratch.project, selection, false);
        assert_same(&recovered, &fresh, &tag);
        assert_ne!(
            before.json()["findings"],
            fresh.json()["findings"],
            "{tag}: the edit moved nothing"
        );
    }
}

const HELPER_TSCONFIG: &str = r#"{
  "compilerOptions": {
    "jsx": "preserve", "module": "ESNext", "moduleResolution": "Bundler",
    "strict": true, "target": "ES2022"
  },
  "files": ["App.tsx", "solid-js.d.ts"],
  "include": []
}
"#;

/// `Helper.tsx` holds the fixture's components and enters the program only
/// through an import from `App.tsx`, which says `app`.
fn helper_program(scratch: &Scratch, app: &str) {
    fs::rename(scratch.path("App.tsx"), scratch.path("Helper.tsx")).unwrap();
    fs::write(scratch.path("App.tsx"), app).unwrap();
    fs::write(scratch.path("tsconfig.json"), HELPER_TSCONFIG).unwrap();
}

/// Finding 2: an edit that changes which files the project's own program
/// holds, without touching the file set on disk.
#[test]
fn an_import_edit_that_changes_program_membership_is_the_one_shot_answer() {
    let Ok(typefacts) = std::env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    const IMPORTS: &str = "export * from \"./Helper\";\n";
    const ALONE: &str = "export {};\n";
    for (direction, from, to) in [("removed", IMPORTS, ALONE), ("added", ALONE, IMPORTS)] {
        mutation_case(
            &typefacts,
            "dependency-source",
            &format!("membership-{direction}"),
            &["dep-hooks"],
            Taken::Reopened,
            |scratch| helper_program(scratch, from),
            |scratch| fs::write(scratch.path("App.tsx"), to).unwrap(),
        );
        // The same edit without a selection.
        let tag = format!("membership-base-{direction}");
        let scratch = Scratch::new("dependency-source", &tag);
        helper_program(&scratch, from);
        let before = check(&typefacts, &scratch.project, &[], true);
        fs::write(scratch.path("App.tsx"), to).unwrap();
        let retained = check(&typefacts, &scratch.project, &[], true);
        let fresh = check(&typefacts, &scratch.project, &[], false);
        assert_same(&retained, &fresh, &tag);
        assert_ne!(
            before.json()["findings"],
            fresh.json()["findings"],
            "{tag}: the mutation moved nothing"
        );
    }
}

/// Finding 3: a relative path is the caller's, not the daemon's.
#[test]
fn a_relative_catalog_path_is_read_from_the_callers_directory() {
    let Ok(typefacts) = std::env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    for selection in [&["dep-hooks"][..], &[][..]] {
        let tag = format!("relative-{}", selection.len());
        let scratch = Scratch::new("dependency-source", &tag);
        let with_catalog = scratch.root.join("a");
        let without = scratch.root.join("b");
        fs::create_dir_all(&with_catalog).unwrap();
        fs::create_dir_all(&without).unwrap();
        fs::write(
            with_catalog.join("catalog.json"),
            r#"{"format":"solid-checker-accepted-contract-catalog","catalogVersion":2,"contracts":[]}"#,
        )
        .unwrap();
        let from = |directory: &Path, retained: bool| {
            let mut command = command(&typefacts, &scratch.project, selection, retained);
            command
                .current_dir(directory)
                .args(["--accepted-contracts", "catalog.json"]);
            command
        };
        let warm = finish(from(&with_catalog, true), true);
        assert_same(&warm, &finish(from(&with_catalog, false), false), &tag);
        let fresh = from(&without, false).output().unwrap();
        assert!(
            !fresh.status.success(),
            "{tag}: one-shot read a missing catalog"
        );
        let retained = from(&without, true).output().unwrap();
        assert!(
            !retained.status.success(),
            "{tag}: the daemon read the first caller's catalog: {}",
            String::from_utf8_lossy(&retained.stderr)
        );
    }
}

/// A private `TMPDIR`, short enough for a socket path, so a test can find
/// its own daemon's socket.
struct SocketDir(PathBuf);

impl SocketDir {
    fn new(tag: &str) -> Self {
        let directory = PathBuf::from(format!("/tmp/scd-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();
        Self(directory)
    }

    fn socket(&self) -> PathBuf {
        let sockets = fs::read_dir(&self.0)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| {
                path.extension()
                    .is_some_and(|extension| extension == "sock")
            })
            .collect::<Vec<_>>();
        assert_eq!(sockets.len(), 1, "{sockets:?}");
        sockets.into_iter().next().unwrap()
    }
}

impl Drop for SocketDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Finding 4: a request the daemon is still serving is not idle time.
#[test]
fn the_idle_watchdog_spares_an_active_request() {
    let Ok(typefacts) = std::env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let scratch = Scratch::new("dependency-source", "idle");
    let sockets = SocketDir::new("idle");
    let retained = || {
        let mut command = command(&typefacts, &scratch.project, &["dep-hooks"], true);
        command
            .env("TMPDIR", &sockets.0)
            .env("SOLID_CHECKER_DAEMON_IDLE_SECS", "1");
        command
    };
    let cold = finish(retained(), true);
    assert_eq!(cold.cache_hit(), Some(false));
    // A client that has connected and not yet sent its request is being
    // served: the daemon is reading it.
    let socket = sockets.socket();
    let slow = std::os::unix::net::UnixStream::connect(&socket).unwrap();
    std::thread::sleep(std::time::Duration::from_secs(3));
    assert!(socket.exists(), "the watchdog ended a request in progress");
    drop(slow);
    let warm = finish(retained(), true);
    assert_eq!(warm.cache_hit(), Some(true), "{}", warm.stderr);
}

/// Finding 4: a body shorter than its header says is a failed answer, never
/// a successful one, and nothing of it reaches stdout.
#[test]
fn a_truncated_daemon_response_is_not_an_answer() {
    use std::io::{BufRead, BufReader, Read, Write};
    use std::os::unix::net::{UnixListener, UnixStream};
    let Ok(typefacts) = std::env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let scratch = Scratch::new("dependency-source", "truncated");
    let sockets = SocketDir::new("truncated");
    let retained = || {
        let mut command = command(&typefacts, &scratch.project, &["dep-hooks"], true);
        command.env("TMPDIR", &sockets.0);
        command
    };
    finish(retained(), true);
    // Stand between the client and its live daemon, and drop the end of the
    // body the daemon sends.
    let socket = sockets.socket();
    let daemon = sockets.0.join("daemon.sock.real");
    fs::rename(&socket, &daemon).unwrap();
    let proxy = UnixListener::bind(&socket).unwrap();
    let relay = std::thread::spawn(move || {
        let (client, _) = proxy.accept().unwrap();
        let mut client_reader = BufReader::new(client.try_clone().unwrap());
        let mut line = String::new();
        client_reader.read_line(&mut line).unwrap();
        let mut upstream = UnixStream::connect(&daemon).unwrap();
        upstream.write_all(line.as_bytes()).unwrap();
        let mut upstream = BufReader::new(upstream);
        let mut header = String::new();
        upstream.read_line(&mut header).unwrap();
        let mut body = Vec::new();
        upstream.read_to_end(&mut body).unwrap();
        assert!(body.len() > 16);
        let mut client = client;
        client.write_all(header.as_bytes()).unwrap();
        client.write_all(&body[..body.len() - 16]).unwrap();
    });
    let output = retained().output().unwrap();
    relay.join().unwrap();
    let fresh = check(&typefacts, &scratch.project, &["dep-hooks"], false);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&fresh.stdout),
        "{stderr}"
    );
    assert!(stderr.contains("daemon unavailable"), "{stderr}");
}

/// Finding 5: a JSON request the one-shot path refuses is refused retained
/// too.
#[test]
fn a_json_request_with_generation_zero_is_refused_by_both_paths() {
    use std::io::Write;
    let Ok(typefacts) = std::env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    for selection in [&["dep-hooks"][..], &[][..]] {
        let tag = format!("generation-{}", selection.len());
        let scratch = Scratch::new("dependency-source", &tag);
        // A warm generation-1 daemon for the same project and selection.
        check(&typefacts, &scratch.project, selection, true);
        let request = serde_json::json!({
            "projectId": scratch.path("tsconfig.json"),
            "generation": 0,
            "sources": [],
            "typefactsExecutable": typefacts,
            "dependencySources": selection,
            "format": "json",
            "runtime": {"target": "browser", "build": "development"},
        })
        .to_string();
        for retained in [false, true] {
            let mut child = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
                .env("SOLID_CHECKER_DAEMON", if retained { "1" } else { "0" })
                .env("SOLID_CHECKER_DAEMON_IDLE_SECS", "20")
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .unwrap();
            child
                .stdin
                .take()
                .unwrap()
                .write_all(request.as_bytes())
                .unwrap();
            let output = child.wait_with_output().unwrap();
            assert!(
                !output.status.success(),
                "{tag} retained={retained}: generation 0 was answered: {}",
                String::from_utf8_lossy(&output.stdout)
            );
        }
    }
}

fn stdout(run: &Run) -> std::borrow::Cow<'_, str> {
    String::from_utf8_lossy(&run.stdout)
}

const NO_PLUGIN: &str = "no Solid Vite plugin";

/// Review round 1 of ADR 0297: a target and a build do not establish the
/// Vite + Solid plugin resolver source selection reads, so without the
/// plugin the default is contract mode, with a note. An explicit selection
/// states the premise and is unchanged.
#[test]
fn the_default_needs_a_solid_vite_plugin() {
    let Ok(typefacts) = std::env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let scratch = Scratch::new("dependency-source-all", "default-plugin");
    let ok = |args: &[&str]| {
        let (success, run) = invoke(&typefacts, &scratch.project, args, false);
        assert!(success, "{args:?}: {}", run.stderr);
        run
    };
    let explicit = ok(&with_host(&["--dependency-source", "*"]));
    assert!(
        source_notes(&explicit)
            .iter()
            .any(|note| note.contains("dep-array")),
        "an explicit selection needs no plugin: {}",
        explicit.stderr
    );
    let opted_out = ok(&with_host(&["--no-dependency-source"]));
    assert_ne!(stdout(&opted_out), stdout(&explicit), "not vacuous");
    assert!(
        !opted_out.stderr.contains(NO_PLUGIN),
        "{}",
        opted_out.stderr
    );

    let contract_mode = |run: &Run, context: &str| {
        assert_eq!(stdout(run), stdout(&opted_out), "{context}");
        assert!(source_notes(run).is_empty(), "{context}: {}", run.stderr);
        assert!(run.stderr.contains(NO_PLUGIN), "{context}: {}", run.stderr);
    };
    contract_mode(&ok(&HOST), "no plugin installed");
    // A manifest naming no version is no installation, as for `solid-js`.
    let versionless = scratch.path("node_modules/vite-plugin-solid");
    fs::create_dir_all(&versionless).unwrap();
    fs::write(
        versionless.join("package.json"),
        r#"{"name":"vite-plugin-solid"}"#,
    )
    .unwrap();
    contract_mode(&ok(&HOST), "a versionless plugin manifest");
    // SolidStart alone is not the plugin; the one it ships is.
    scratch.install("@solidjs/start");
    contract_mode(&ok(&HOST), "SolidStart without its plugin");
    scratch.install("@solidjs/start/node_modules/vite-plugin-solid");
    assert_same(&ok(&HOST), &explicit, "the plugin SolidStart ships");
    fs::remove_dir_all(scratch.path("node_modules/@solidjs/start")).unwrap();
    contract_mode(&ok(&HOST), "SolidStart removed");
    for plugin in ["vite-plugin-solid", "@solidjs/vite-plugin"] {
        let _ = fs::remove_dir_all(scratch.path("node_modules").join("vite-plugin-solid"));
        let _ = fs::remove_dir_all(scratch.path("node_modules/@solidjs/vite-plugin"));
        scratch.install(plugin);
        assert_same(&ok(&HOST), &explicit, plugin);
    }
}

/// Restores a directory's mode when dropped, so a failed assertion still
/// lets `Scratch` remove the tree.
#[cfg(unix)]
struct ReadOnly(PathBuf);

#[cfg(unix)]
impl ReadOnly {
    fn new(directory: &Path) -> Self {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(directory, fs::Permissions::from_mode(0o555)).unwrap();
        Self(directory.to_path_buf())
    }
}

#[cfg(unix)]
impl Drop for ReadOnly {
    fn drop(&mut self) {
        use std::os::unix::fs::PermissionsExt as _;
        let _ = fs::set_permissions(&self.0, fs::Permissions::from_mode(0o755));
    }
}

/// Review round 1 of ADR 0297: contract mode writes nothing into the
/// project, so the default does not make a read-only project fail. It is
/// contract mode there, one-shot and retained, with a note; an explicit
/// selection keeps its write requirement and its error.
#[cfg(unix)]
#[test]
fn a_read_only_project_keeps_contracts_by_default() {
    let Ok(typefacts) = std::env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let scratch =
        Scratch::new("dependency-source-all", "default-read-only").with_solid_vite_plugin();
    let opted_out = invoke(
        &typefacts,
        &scratch.project,
        &with_host(&["--no-dependency-source"]),
        false,
    )
    .1;
    let _read_only = ReadOnly::new(&scratch.project);
    for retained in [false, true] {
        let (success, run) = invoke(&typefacts, &scratch.project, &HOST, retained);
        assert!(success, "retained {retained}: {}", run.stderr);
        assert_eq!(stdout(&run), stdout(&opted_out), "retained {retained}");
        assert!(
            run.stderr.contains("is not writable"),
            "retained {retained}: {}",
            run.stderr
        );
    }
    let (success, refused) = invoke(
        &typefacts,
        &scratch.project,
        &with_host(&["--dependency-source", "*"]),
        false,
    );
    assert!(!success, "{}", refused.stderr);
    assert!(
        refused.stderr.contains("--dependency-source: cannot write"),
        "{}",
        refused.stderr
    );
    assert!(scratch.siblings().is_empty());
}

/// The closure digest of a one-manifest fixture resolution, as
/// `artifact_resolution::closure_digest` computes it.
fn manifest_closure_digest(manifest: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hash = Sha256::new();
    hash.update(b"solid-checker:artifact-closure:v1");
    hash.update(1u64.to_be_bytes());
    for text in ["Manifest", "./package.json", manifest] {
        hash.update(u64::try_from(text.len()).unwrap().to_be_bytes());
        hash.update(text.as_bytes());
    }
    hash.update([0]);
    hash.update(0u64.to_be_bytes());
    hash.update(0u64.to_be_bytes());
    format!("{:x}", hash.finalize())
}

/// `package-plain-return-consumer` with its `reactive-package` made a Solid
/// package with runtime source, its fixture resolution rebound to the new
/// manifest, `App.tsx` replaced by `app`, and its accepted contract
/// authorized: `isReady` closes `returns`, `isReadyOpen` leaves it open.
/// Answers the scratch and the trust configuration, outside the project.
fn accepted_solid_package(tag: &str, app: &str) -> (Scratch, PathBuf) {
    accepted_solid_package_with(tag, app, |_| {})
}

fn accepted_solid_package_with(
    tag: &str,
    app: &str,
    setup: impl FnOnce(&Scratch),
) -> (Scratch, PathBuf) {
    use sha2::{Digest, Sha256};
    use solid_facts_backend::fixture_authorization::{
        authorize_fixture_contract, read_fixture_contract_request,
    };
    let scratch = Scratch::new("package-plain-return-consumer", tag).with_solid_vite_plugin();
    let package = scratch.path("node_modules/reactive-package");
    let digest = |bytes: &[u8]| format!("{:x}", Sha256::digest(bytes));
    let old = digest(&fs::read(package.join("package.json")).unwrap());
    let manifest = r#"{"name":"reactive-package","version":"1.0.0","types":"index.d.ts","module":"index.js","peerDependencies":{"solid-js":"*"}}"#;
    fs::write(package.join("package.json"), manifest).unwrap();
    fs::write(
        package.join("index.js"),
        "export function isReady() { return true; }\nexport function isReadyOpen() { return true; }\n",
    )
    .unwrap();
    let new = digest(manifest.as_bytes());
    let rebind = |path: PathBuf| {
        let text = fs::read_to_string(&path)
            .unwrap()
            .replace(&old, &new)
            .replace(
                &manifest_closure_digest(&format!("sha256:{old}")),
                &manifest_closure_digest(&format!("sha256:{new}")),
            );
        fs::write(path, text).unwrap();
    };
    rebind(scratch.path(".solid-checker/authorize-contract.json"));
    rebind(package.join("solid-reactivity.json"));
    fs::write(scratch.path("App.tsx"), app).unwrap();
    setup(&scratch);
    let authorization = authorize_fixture_contract(
        &scratch.project,
        &read_fixture_contract_request(&scratch.project).unwrap(),
    )
    .unwrap();
    let trust = scratch.root.join("trust.json");
    fs::write(&trust, authorization.trust_configuration).unwrap();
    (scratch, trust)
}

const NAMED_APP: &str = r#"import { isReady, isReadyOpen } from "reactive-package";

export function Status() {
  return <div>{isReady() ? "ready" : "waiting"}</div>;
}

export function StatusOpen() {
  return <div>{isReadyOpen() ? "ready" : "waiting"}</div>;
}
"#;

const NAMESPACE_APP: &str = r#"import * as reactive from "reactive-package";

export function Status() {
  return <div>{reactive.isReady() ? "ready" : "waiting"}</div>;
}

export function StatusOpen() {
  return <div>{reactive.isReadyOpen() ? "ready" : "waiting"}</div>;
}
"#;

/// Review round 3: a relative barrel cannot authorize a rejected recursive
/// edge to another installed copy, even when both manifests name the same
/// package and version. Both body orders are type-valid.
fn check_recursive_reexport_copy(nested_reads: bool) {
    let Ok(typefacts) = std::env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let app = "import { createSignal } from 'solid-js';\nimport { isReadyOpen as read } from './nested/barrel';\nexport function App() { const [ready] = createSignal(true); const value = read(ready); return <div>{value ? 'yes' : 'no'}</div>; }";
    let (scratch, _) =
        accepted_solid_package_with(&format!("recursive-copy-{nested_reads}"), app, |scratch| {
            fs::copy(
                fixture("dependency-source-all").join("solid-js.d.ts"),
                scratch.path("solid-js.d.ts"),
            )
            .unwrap();
            let outer = scratch.path("node_modules/reactive-package");
            edit(
                &outer.join("index.d.ts"),
                "isReadyOpen()",
                "isReadyOpen(source?: () => boolean)",
            );
            let nested = scratch.path("nested/node_modules/reactive-package");
            copy_tree(&outer, &nested);
            let read = "export function isReadyOpen(source) { return source(); }";
            let plain = "export function isReadyOpen(source) { return true; }";
            fs::write(
                outer.join("index.js"),
                if nested_reads { plain } else { read },
            )
            .unwrap();
            fs::write(
                nested.join("index.js"),
                if nested_reads { read } else { plain },
            )
            .unwrap();
            fs::write(
                scratch.path("nested/barrel.ts"),
                "export { isReadyOpen } from 'reactive-package';",
            )
            .unwrap();
            let config_path = scratch.path("tsconfig.json");
            let mut config: serde_json::Value =
                serde_json::from_slice(&fs::read(&config_path).unwrap()).unwrap();
            config["include"] = serde_json::json!([]);
            config["files"] = serde_json::json!(["App.tsx", "jsx.d.ts", "solid-js.d.ts"]);
            fs::write(config_path, serde_json::to_vec_pretty(&config).unwrap()).unwrap();
        });
    let run = |selection: &'static str| {
        let (success, run) = invoke(
            &typefacts,
            &scratch.project,
            &with_host(&["--dependency-source", selection]),
            false,
        );
        assert!(success, "{}", run.stderr);
        run
    };
    let explicit = run("reactive-package");
    assert!(
        explicit.json()["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|finding| finding["id"] == "SC9011" && finding["kind"] == "uncertifiable"),
        "nested reads {nested_reads}: {}",
        stdout(&explicit)
    );
    assert!(
        !explicit.json()["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|finding| finding["kind"] == "violation"),
        "{}",
        stdout(&explicit)
    );
    let wildcard = run("*");
    assert_eq!(
        wildcard.json()["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|finding| finding["id"] == "SC1001" && finding["kind"] == "violation"),
        nested_reads,
        "{}",
        stdout(&wildcard)
    );
    if !nested_reads {
        assert!(
            wildcard.json()["findings"]
                .as_array()
                .unwrap()
                .iter()
                .all(|finding| finding["id"] == "SC9014"),
            "{}",
            stdout(&wildcard)
        );
    }
}

#[test]
fn round3_duplicates_minimal() {
    check_recursive_reexport_copy(false);
}

#[test]
fn round3_duplicates_reverse() {
    check_recursive_reexport_copy(true);
}

/// Build a second fixture-accepted package bound to the exact importing
/// runtime file, then merge its catalog with the first package's catalog.
fn authorize_external_test_package(scratch: &Scratch, name: &str) {
    use sha2::{Digest, Sha256};
    use solid_facts_backend::fixture_authorization::{
        authorize_fixture_contract, read_fixture_contract_request,
    };
    let base = fixture("package-plain-return-consumer");
    let old = format!(
        "{:x}",
        Sha256::digest(fs::read(base.join("node_modules/reactive-package/package.json")).unwrap())
    );
    let package = scratch.path(&format!("node_modules/{name}"));
    let manifest = fs::read(package.join("package.json")).unwrap();
    let new = format!("{:x}", Sha256::digest(&manifest));
    let rebind = |text: String| {
        text.replace(&old, &new)
            .replace(
                &manifest_closure_digest(&format!("sha256:{old}")),
                &manifest_closure_digest(&format!("sha256:{new}")),
            )
            .replace("reactive-package", name)
    };
    let mut document: serde_json::Value = serde_json::from_str(&rebind(
        fs::read_to_string(base.join("node_modules/reactive-package/solid-reactivity.json"))
            .unwrap(),
    ))
    .unwrap();
    let mut request: serde_json::Value = serde_json::from_str(&rebind(
        fs::read_to_string(base.join(".solid-checker/authorize-contract.json")).unwrap(),
    ))
    .unwrap();
    let version: serde_json::Value = serde_json::from_slice(&manifest).unwrap();
    document["package"]["version"] = version["version"].clone();
    request["import"]["packageVersion"] = version["version"].clone();
    request["import"]["importer"] = "node_modules/reactive-package/index.js".into();
    if name == "clsx" {
        document["summaries"]
            .as_object_mut()
            .unwrap()
            .remove("summary-plain-return");
        document["entrypoints"]["."]["exports"] =
            serde_json::json!({"clsx": "summary-returns-open"});
        let mut export = request["import"]["exports"]["isReadyOpen"].clone();
        export["declarations"]["exportName"] = "clsx".into();
        export["runtime"]["exportName"] = "clsx".into();
        request["import"]["exports"] = serde_json::json!({"clsx": export});
    }
    fs::write(
        package.join("solid-reactivity.json"),
        serde_json::to_vec_pretty(&document).unwrap(),
    )
    .unwrap();
    let catalog_path = scratch.path(".solid-checker/accepted-contracts.json");
    let mut catalog: serde_json::Value =
        serde_json::from_slice(&fs::read(&catalog_path).unwrap()).unwrap();
    fs::write(
        scratch.path(".solid-checker/authorize-contract.json"),
        serde_json::to_vec_pretty(&request).unwrap(),
    )
    .unwrap();
    authorize_fixture_contract(
        &scratch.project,
        &read_fixture_contract_request(&scratch.project).unwrap(),
    )
    .unwrap();
    let external: serde_json::Value =
        serde_json::from_slice(&fs::read(&catalog_path).unwrap()).unwrap();
    catalog["contracts"]
        .as_array_mut()
        .unwrap()
        .extend(external["contracts"].as_array().unwrap().iter().cloned());
    fs::write(catalog_path, serde_json::to_vec_pretty(&catalog).unwrap()).unwrap();
}

fn check_accepted_external_value_provenance(external: &str, tag: &str, body: &str) {
    let Ok(typefacts) = std::env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let (scratch, trust) = accepted_solid_package_with(
        &format!("open-provenance-{external}-{tag}"),
        NAMED_APP,
        |scratch| {
            let package = scratch.path(&format!("node_modules/{external}"));
            if external == "clsx" {
                copy_tree(
                    &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/clsx-2.1.1"),
                    &package,
                );
            } else {
                fs::create_dir_all(&package).unwrap();
                fs::write(package.join("package.json"), r#"{"name":"external-package","version":"1.0.0","types":"index.d.ts","module":"index.js"}"#).unwrap();
                fs::write(
                    package.join("index.d.ts"),
                    "export declare function isReadyOpen(): boolean;",
                )
                .unwrap();
                fs::write(
                    package.join("index.js"),
                    "export function isReadyOpen() { return true; }",
                )
                .unwrap();
            }
            let (imported, call) = if external == "clsx" {
                ("clsx", "x('ok')")
            } else {
                ("isReadyOpen", "x()")
            };
            fs::write(scratch.path("node_modules/reactive-package/index.js"), format!("import {{ {imported} as x }} from '{external}'; {}\nexport function isReady() {{ return true; }}", body.replace("x()", call))).unwrap();
            if external == "clsx" && tag != "discarded" {
                edit(
                    &scratch.path("node_modules/reactive-package/index.d.ts"),
                    "isReadyOpen(): boolean",
                    "isReadyOpen(): string",
                );
            }
        },
    );
    authorize_external_test_package(&scratch, external);
    let trust = trust.to_string_lossy();
    let run = |extra: &[&str]| {
        let mut args = with_host(&[]);
        args.extend(["--receipt-trust-configuration", trust.as_ref()]);
        args.extend(extra);
        let (success, run) = invoke(&typefacts, &scratch.project, &args, false);
        assert!(success, "{external}/{tag}: {}", run.stderr);
        run
    };
    let contracts = run(&["--no-dependency-source"]);
    assert!(
        contracts.json()["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|finding| finding["id"] == "SC9005" && finding["kind"] == "uncertifiable"),
        "{}",
        stdout(&contracts)
    );
    for selection in ["reactive-package", "*"] {
        let selected = run(&["--dependency-source", selection]);
        assert!(selected.stderr.contains(&format!("accepted unselected package {external}")) || selected.json()["packageSummaries"].as_array().unwrap().iter().any(|summary| summary["name"] == external && summary["evidence"] == "accepted"), "{external}/{tag}: {}", stdout(&selected));
        if tag == "discarded" {
            assert!(
                !selected.stderr.contains("accepted unselected package"),
                "{}",
                selected.stderr
            );
        } else {
            assert_eq!(
                selected.json()["findings"],
                contracts.json()["findings"],
                "{external}/{tag}/{selection}"
            );
            assert!(
                selected
                    .stderr
                    .contains(&format!("accepted unselected package {external}")),
                "{}",
                selected.stderr
            );
        }
    }
}

macro_rules! open_provenance_test {
    ($name:ident, $external:literal, $tag:literal, $body:literal) => {
        #[test]
        fn $name() {
            check_accepted_external_value_provenance($external, $tag, $body);
        }
    };
}

open_provenance_test!(
    round3_external_class_field,
    "external-package",
    "class-field",
    "class C { value = x(); } const c = new C(); export function isReadyOpen() { return c.value; }"
);
open_provenance_test!(
    round3_external_class_export,
    "external-package",
    "class-export",
    "export class C { value = x(); } export function isReadyOpen() { return new C().value; }"
);
open_provenance_test!(
    round3_external_assign_object,
    "external-package",
    "assign-object",
    "const obj = { value: false }; Object.assign(obj, { value: x() }); export { obj }; export function isReadyOpen() { return obj.value; }"
);
open_provenance_test!(
    round3_clsx_class_field,
    "clsx",
    "class-field",
    "class C { value = x(); } const c = new C(); export function isReadyOpen() { return c.value; }"
);
open_provenance_test!(
    round3_clsx_class_export,
    "clsx",
    "class-export",
    "export class C { value = x(); } export function isReadyOpen() { return new C().value; }"
);
open_provenance_test!(
    round3_clsx_assign_object,
    "clsx",
    "assign-object",
    "const obj = { value: false }; Object.assign(obj, { value: x() }); export { obj }; export function isReadyOpen() { return obj.value; }"
);
open_provenance_test!(
    round3_external_mutation_closure,
    "external-package",
    "mutation-closure",
    "const obj = { value: false }; function set(value) { obj.value = value; } export const update = () => set(x()); export function isReadyOpen() { return obj.value; }"
);
open_provenance_test!(
    round3_external_getter,
    "external-package",
    "getter",
    "const obj = { get value() { return x(); } }; export function isReadyOpen() { return obj.value; }"
);
open_provenance_test!(
    round3_external_discarded_control,
    "external-package",
    "discarded",
    "x(); export function isReadyOpen() { return true; }"
);
open_provenance_test!(
    round3_clsx_discarded_control,
    "clsx",
    "discarded",
    "x(); export function isReadyOpen() { return true; }"
);

/// Exact re-export edges select runtime source, in project and package
/// barrels alike. A reactive runtime read proves a violation where contract
/// mode could only leave the re-export's return claim uncertifiable.
#[test]
fn reexport_barrels_redirect_exact_bindings_and_retain_unselected_contracts() {
    let Ok(typefacts) = std::env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    for (tag, barrel, in_package) in [
        (
            "named",
            "export { isReady, isReadyOpen } from 'reactive-package';",
            false,
        ),
        ("star", "export * from 'reactive-package';", false),
        ("star-mixed", "export * from 'reactive-package';", false),
        (
            "star-unselected-provider",
            "export * from 'reactive-package'; export * from 'external-star-provider';",
            false,
        ),
        (
            "star-same-binding",
            "export * from 'reactive-package'; export * from './other';",
            false,
        ),
        (
            "star-ambiguous",
            "export * from 'reactive-package'; export * from './other';",
            false,
        ),
        (
            "star-opaque-provider",
            "export * from 'reactive-package'; export * from './opaque.js';",
            false,
        ),
        (
            "star-explicit",
            "export * from 'reactive-package'; export { isReadyOpen } from './other';",
            false,
        ),
        (
            "default-as",
            "export { isReady, default as isReadyOpen } from 'reactive-package';",
            false,
        ),
        (
            "files-only-named",
            "export { isReady, isReadyOpen } from 'reactive-package';",
            false,
        ),
        (
            "files-only-star",
            "export * from 'reactive-package';",
            false,
        ),
        (
            "files-only-default-as",
            "export { isReady, default as isReadyOpen } from 'reactive-package';",
            false,
        ),
        (
            "closure-named",
            "export { isReady, isReadyOpen } from 'reactive-package';",
            true,
        ),
        ("closure-star", "export * from 'reactive-package';", true),
        (
            "closure-default-as",
            "export { isReady, default as isReadyOpen } from 'reactive-package';",
            true,
        ),
    ] {
        let specifier = if in_package {
            "barrel-package"
        } else {
            "./barrel"
        };
        let mut app = NAMED_APP.replace("reactive-package", specifier);
        app = format!("import {{ createSignal }} from 'solid-js';\n{}", app.replace("  return <div>{isReadyOpen() ?", "  const [ready] = createSignal(true);\n  const value = isReadyOpen(ready);\n  return <div>{value ?"));
        let source_read_line = app
            .lines()
            .position(|line| line.contains("const value = isReadyOpen(ready)"))
            .unwrap()
            + 1;
        let plain_line = app
            .lines()
            .position(|line| line.contains("return <div>{isReady()"))
            .unwrap()
            + 1;
        if tag == "star-mixed" {
            app.push_str("\nimport { isReadyOpen as directOpen } from 'reactive-package';\nexport function DirectStatusOpen() { return <div>{directOpen() ? 'ready' : 'waiting'}</div>; }\n");
        }
        let (scratch, trust) = accepted_solid_package_with(
            &format!("reexport-{tag}"),
            &app,
            |scratch| {
                fs::copy(
                    fixture("dependency-source-all").join("solid-js.d.ts"),
                    scratch.path("solid-js.d.ts"),
                )
                .unwrap();
                fs::write(scratch.path("node_modules/reactive-package/index.js"), "export function isReady() { return true; }\nexport function isReadyOpen(source) { return source(); }\n").unwrap();
                edit(
                    &scratch.path("node_modules/reactive-package/index.d.ts"),
                    "isReadyOpen()",
                    "isReadyOpen(source?: () => boolean)",
                );
                if matches!(
                    tag,
                    "star-same-binding" | "star-ambiguous" | "star-explicit"
                ) {
                    fs::write(scratch.path("other.ts"), if tag == "star-same-binding" { "export * from 'reactive-package';" } else { "export function isReadyOpen(_source?: () => boolean): boolean { return true; }" }).unwrap();
                }
                if tag == "star-opaque-provider" {
                    fs::write(
                        scratch.path("opaque.js"),
                        "exports.isReadyOpen = () => true;",
                    )
                    .unwrap();
                    fs::write(
                        scratch.path("opaque.d.ts"),
                        "export declare const another: number;",
                    )
                    .unwrap();
                    let config_path = scratch.path("tsconfig.json");
                    let mut config: serde_json::Value =
                        serde_json::from_slice(&fs::read(&config_path).unwrap()).unwrap();
                    config["compilerOptions"]["allowJs"] = true.into();
                    config["include"]
                        .as_array_mut()
                        .unwrap()
                        .push("*.js".into());
                    fs::write(config_path, serde_json::to_vec_pretty(&config).unwrap()).unwrap();
                }
                if tag == "star-unselected-provider" {
                    let external = scratch.path("node_modules/external-star-provider");
                    fs::create_dir_all(&external).unwrap();
                    fs::write(external.join("package.json"), r#"{"name":"external-star-provider","version":"1.0.0","types":"index.d.ts","module":"index.js"}"#).unwrap();
                    // TypeScript is clean, but this unselected runtime
                    // provider contributes a competing binding.
                    fs::write(
                        external.join("index.d.ts"),
                        "export declare const another: number;",
                    )
                    .unwrap();
                    fs::write(
                        external.join("index.js"),
                        "export const isReadyOpen = () => true;",
                    )
                    .unwrap();
                }
                let importer = if in_package {
                    let package = scratch.path("node_modules/barrel-package");
                    fs::create_dir_all(&package).unwrap();
                    fs::write(package.join("package.json"), r#"{"name":"barrel-package","version":"1.0.0","types":"index.d.ts","module":"index.js","peerDependencies":{"solid-js":"*"}}"#).unwrap();
                    fs::write(package.join("index.d.ts"), barrel).unwrap();
                    fs::write(package.join("index.js"), barrel).unwrap();
                    "node_modules/barrel-package/index.js"
                } else {
                    fs::write(scratch.path("barrel.ts"), barrel).unwrap();
                    "barrel.ts"
                };
                if tag.starts_with("files-only") {
                    let config_path = scratch.path("tsconfig.json");
                    let mut config: serde_json::Value =
                        serde_json::from_slice(&fs::read(&config_path).unwrap()).unwrap();
                    config["include"] = serde_json::json!([]);
                    config["files"] = serde_json::json!(["App.tsx", "jsx.d.ts", "solid-js.d.ts"]);
                    fs::write(config_path, serde_json::to_vec_pretty(&config).unwrap()).unwrap();
                }
                let request_path = scratch.path(".solid-checker/authorize-contract.json");
                let mut request: serde_json::Value =
                    serde_json::from_slice(&fs::read(&request_path).unwrap()).unwrap();
                request["import"]["importer"] = if tag == "star-mixed" {
                    "App.tsx"
                } else {
                    importer
                }
                .into();
                if tag.contains("default-as") {
                    let package = scratch.path("node_modules/reactive-package");
                    let declarations = fs::read_to_string(package.join("index.d.ts")).unwrap();
                    fs::write(
                        package.join("index.d.ts"),
                        format!("{declarations}\nexport default isReadyOpen;\n"),
                    )
                    .unwrap();
                    let runtime = fs::read_to_string(package.join("index.js")).unwrap();
                    fs::write(
                        package.join("index.js"),
                        format!("{runtime}\nexport default isReadyOpen;\n"),
                    )
                    .unwrap();
                    let document_path = package.join("solid-reactivity.json");
                    let mut document: serde_json::Value =
                        serde_json::from_slice(&fs::read(&document_path).unwrap()).unwrap();
                    document["entrypoints"]["."]["exports"]["default"] =
                        "summary-returns-open".into();
                    fs::write(document_path, serde_json::to_vec_pretty(&document).unwrap())
                        .unwrap();
                    let mut export = request["import"]["exports"]["isReadyOpen"].clone();
                    export["declarations"]["exportName"] = "default".into();
                    export["runtime"]["exportName"] = "default".into();
                    request["import"]["exports"]["default"] = export;
                }
                fs::write(request_path, serde_json::to_vec_pretty(&request).unwrap()).unwrap();
            },
        );
        let trust = trust.to_string_lossy();
        // The ambiguous-star negative deliberately has TS2308. The checker
        // must refuse its runtime join, not choose TypeScript's recovery arm.
        if let Ok(tsc) = std::env::var("SOLID_CHECKER_TEST_TSC")
            && tag != "star-ambiguous"
        {
            let output = Command::new("node")
                .arg(tsc)
                .args(["--noEmit", "--project"])
                .arg(scratch.path("tsconfig.json"))
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{tag}: tsc: {}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        let run = |extra: &[&str]| {
            let mut args = with_host(&[]);
            args.extend(["--receipt-trust-configuration", trust.as_ref()]);
            args.extend(extra);
            let (success, run) = invoke(&typefacts, &scratch.project, &args, false);
            assert!(success, "{tag}: {}", run.stderr);
            run
        };
        let contracts = run(&["--no-dependency-source"]);
        let explicit = if in_package {
            vec![
                "--dependency-source",
                "barrel-package",
                "--dependency-source",
                "reactive-package",
            ]
        } else {
            vec!["--dependency-source", "reactive-package"]
        };
        for args in [vec![], vec!["--dependency-source", "*"], explicit] {
            let selected = run(&args);
            assert!(
                !selected
                    .notes()
                    .iter()
                    .any(|note| note.contains("dependency source reactive-package")
                        && note.contains("its contract stays")),
                "{tag}: {}",
                selected.stderr
            );
            let proves_read = !matches!(
                tag,
                "star-ambiguous"
                    | "star-explicit"
                    | "star-unselected-provider"
                    | "star-opaque-provider"
            );
            assert_eq!(
                selected.json()["findings"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|finding| finding["id"] == "SC1001"
                        && finding["kind"] == "violation"
                        && finding["primaryLocation"]["line"] == source_read_line),
                proves_read,
                "{tag}: {args:?}: {}",
                stdout(&selected)
            );
            if matches!(tag, "star-unselected-provider" | "star-opaque-provider") {
                assert!(
                    selected.json()["findings"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|finding| finding["id"] == "SC9011"
                            && finding["kind"] == "uncertifiable"),
                    "{}",
                    stdout(&selected)
                );
            }
            assert!(
                !selected.json()["findings"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|finding| finding["kind"] == "violation"
                        && finding["primaryLocation"]["line"] == plain_line),
                "{tag}: plain return: {}",
                stdout(&selected)
            );
            for obligation in contracts.json()["findings"].as_array().unwrap() {
                assert!(
                    selected.json()["findings"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|finding| {
                            ["path", "line", "column"].iter().all(|key| {
                                finding["primaryLocation"][key]
                                    == obligation["primaryLocation"][key]
                            }) && finding["id"] == obligation["id"]
                                && finding["kind"] == obligation["kind"]
                                || finding["id"] == "SC1001"
                                    && finding["kind"] == "violation"
                                    && finding["primaryLocation"]["line"] == source_read_line
                                    && proves_read
                        }),
                    "{tag}: {args:?}: lost obligation {obligation}; selected {}",
                    stdout(&selected)
                );
            }
        }
        if in_package {
            let retained = run(&["--dependency-source", "barrel-package"]);
            assert_eq!(
                stdout(&retained),
                stdout(&contracts),
                "{tag}: accepted unselected target"
            );
            assert!(
                retained
                    .notes()
                    .iter()
                    .any(|note| note.contains("accepted unselected package reactive-package")),
                "{tag}: {}",
                retained.stderr
            );
        }
        let findings = contracts.json()["findings"].as_array().unwrap().clone();
        // Contract mode itself does not bind a bare star's open claims:
        // resolve_contract_imports_inner walks explicit specifiers only.
        // Source selection must prove the read even where contract mode
        // projects no star obligation; that separate gap remains in ADR 0297.
        assert!(
            (tag.contains("star") && tag != "star-mixed")
                || findings
                    .iter()
                    .any(|finding| finding["id"] == "SC9005" && finding["kind"] == "uncertifiable"),
            "{tag}: {findings:#?}"
        );
        assert!(
            !findings
                .iter()
                .any(|finding| finding["kind"] == "violation"),
            "{tag}: {findings:#?}"
        );
    }
}

#[test]
fn only_literal_bare_module_loads_retain_the_named_imports_contract() {
    let Ok(typefacts) = std::env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    for (tag, load) in [
        (
            "literal",
            "export async function load() { return import('reactive-package'); }",
        ),
        (
            "nonliteral",
            "export async function load(name: string) { return import(name); }",
        ),
        (
            "relative",
            "export async function load() { return import('./barrel'); }",
        ),
    ] {
        let app = format!("{NAMED_APP}\n{load}\n");
        let (scratch, trust) =
            accepted_solid_package_with(&format!("module-load-{tag}"), &app, |scratch| {
                fs::write(scratch.path("barrel.ts"), "export const value = true;").unwrap();
            });
        let trust = trust.to_string_lossy();
        let run = |extra: &[&str]| {
            let mut args = with_host(&[]);
            args.extend(["--receipt-trust-configuration", trust.as_ref()]);
            args.extend(extra);
            let (success, run) = invoke(&typefacts, &scratch.project, &args, false);
            assert!(success, "{tag}: {}", run.stderr);
            run
        };
        let selected = run(&[]);
        let contracts = run(&["--no-dependency-source"]);
        if tag != "literal" {
            assert!(
                selected.json()["findings"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|finding| finding["id"] == "SC9014"),
                "{tag}: {}",
                stdout(&selected)
            );
            assert!(
                !selected
                    .notes()
                    .iter()
                    .any(|note| note.contains("dependency source reactive-package")
                        && note.contains("its contract stays")),
                "{tag}: {}",
                selected.stderr
            );
            continue;
        }
        assert_eq!(stdout(&selected), stdout(&contracts), "{tag}");
        assert!(
            stdout(&selected).contains("SC9005"),
            "{tag}: {}",
            stdout(&selected)
        );
        assert!(
            selected
                .notes()
                .iter()
                .any(|note| note.contains("dependency source reactive-package")
                    && note.contains("its contract stays")),
            "{tag}: {}",
            selected.stderr
        );
    }
}

/// An exact source re-export may end at an unselected declaration. Absence
/// of that external package's accepted contract is source analysis's
/// obligation, not a reason to retain every export of the selected package.
#[test]
fn reexport_to_an_unaccepted_external_package_keeps_source_selection() {
    let Ok(typefacts) = std::env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let app = "import { createSignal } from 'solid-js';\nimport { isReadyOpen } from 'reactive-package';\nexport function Status() { const [ready] = createSignal(true); const value = isReadyOpen(ready); return <div>{value}</div>; }";
    let (scratch, trust) = accepted_solid_package_with("unaccepted-reexport", app, |scratch| {
        fs::copy(
            fixture("dependency-source-all").join("solid-js.d.ts"),
            scratch.path("solid-js.d.ts"),
        )
        .unwrap();
        edit(
            &scratch.path("node_modules/reactive-package/index.d.ts"),
            "isReadyOpen()",
            "isReadyOpen(source?: () => boolean)",
        );
        fs::write(
            scratch.path("node_modules/reactive-package/index.js"),
            "export { isReady, isReadyOpen } from 'external-package';",
        )
        .unwrap();
        let external = scratch.path("node_modules/external-package");
        fs::create_dir_all(&external).unwrap();
        fs::write(external.join("package.json"), r#"{"name":"external-package","version":"1.0.0","types":"index.d.ts","module":"index.js"}"#).unwrap();
        fs::write(external.join("index.d.ts"), "export declare function isReady(): boolean; export declare function isReadyOpen(source?: () => boolean): boolean;").unwrap();
        fs::write(external.join("index.js"), "export function isReady() { return true; } export function isReadyOpen(source) { return source(); }").unwrap();
    });
    let trust = trust.to_string_lossy();
    let mut args = with_host(&[]);
    args.extend(["--receipt-trust-configuration", trust.as_ref()]);
    let (success, selected) = invoke(&typefacts, &scratch.project, &args, false);
    assert!(success, "{}", selected.stderr);
    assert!(
        !selected
            .notes()
            .iter()
            .any(|note| note.contains("dependency source reactive-package")
                && note.contains("its contract stays")),
        "{}",
        selected.stderr
    );
    assert!(
        selected.json()["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|finding| finding["id"] == "SC9011" && finding["kind"] == "uncertifiable"),
        "{}",
        stdout(&selected)
    );
    assert!(
        !selected.json()["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|finding| finding["kind"] == "violation"),
        "{}",
        stdout(&selected)
    );
}

#[test]
fn config_only_namespace_edges_do_not_retain_runtime_packages() {
    let Ok(typefacts) = std::env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let (scratch, trust) =
        accepted_solid_package_with("config-only-namespace", NAMED_APP, |scratch| {
            fs::write(
                scratch.path("vite.config.ts"),
                "import * as reactive from 'reactive-package'; export default reactive;",
            )
            .unwrap();
        });
    let trust = trust.to_string_lossy();
    let mut args = with_host(&[]);
    args.extend(["--receipt-trust-configuration", trust.as_ref()]);
    let (success, selected) = invoke(&typefacts, &scratch.project, &args, false);
    assert!(success, "{}", selected.stderr);
    assert!(
        selected.json()["findings"]
            .as_array()
            .unwrap()
            .iter()
            .all(|finding| finding["id"] == "SC9014"),
        "{}",
        stdout(&selected)
    );
    assert!(
        !selected
            .notes()
            .iter()
            .any(|note| note.contains("dependency source reactive-package")
                && note.contains("its contract stays")),
        "{}",
        selected.stderr
    );
}

/// Review round 1 of ADR 0297: a package imported through a namespace
/// binding is not selected, so the namespace keeps its accepted contract.
/// Selecting it removed the contract, which nothing redirects a namespace
/// member to, and `isReadyOpen`'s open `returns` was then reported nowhere.
#[test]
fn a_namespace_import_keeps_its_accepted_contract() {
    let Ok(typefacts) = std::env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let answers = |tag: &str, app: &str| {
        let (scratch, trust) = accepted_solid_package(tag, app);
        let trust = trust.to_string_lossy().into_owned();
        let run = |rest: &[&str]| {
            let mut args = with_host(&[]);
            args.extend(["--receipt-trust-configuration", trust.as_str()]);
            args.extend(rest);
            let (success, run) = invoke(&typefacts, &scratch.project, &args, false);
            assert!(success, "{tag} {rest:?}: {}", run.stderr);
            run
        };
        (run(&[]), run(&["--no-dependency-source"]))
    };
    // The control: named imports are redirected, so the package is selected
    // and its contract unbound.
    let (named, named_contracts) = answers("namespace-control", NAMED_APP);
    assert!(
        stdout(&named_contracts).contains("SC9005"),
        "{}",
        named_contracts.stderr
    );
    assert_ne!(
        stdout(&named),
        stdout(&named_contracts),
        "the package is selectable"
    );

    let (namespace, namespace_contracts) = answers("namespace", NAMESPACE_APP);
    assert!(
        stdout(&namespace_contracts).contains("isReadyOpen"),
        "{}",
        stdout(&namespace_contracts)
    );
    assert_eq!(stdout(&namespace), stdout(&namespace_contracts));
    assert!(
        namespace.notes().iter().any(|note| note
            .contains("dependency source reactive-package: imported through a namespace binding")),
        "{}",
        namespace.stderr
    );
}

/// The same for a namespace import inside a selected package's own source:
/// the package it names keeps its contract, and nothing proves the call.
#[test]
fn a_namespace_import_in_package_source_keeps_that_package_on_contracts() {
    let Ok(typefacts) = std::env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let scratch =
        Scratch::new("dependency-source-all", "default-closure-namespace").with_solid_vite_plugin();
    edit(
        &scratch.path("node_modules/dep-hooks/src/index.jsx"),
        "import { readInner } from \"dep-inner\";",
        "import * as inner from \"dep-inner\";",
    );
    edit(
        &scratch.path("node_modules/dep-hooks/src/index.jsx"),
        "return readInner(source);",
        "return inner.readInner(source);",
    );
    let (success, run) = invoke(&typefacts, &scratch.project, &HOST, false);
    assert!(success, "{}", run.stderr);
    assert!(
        run.notes().iter().any(|note| note
            .contains("dependency source dep-inner: imported through a namespace binding")),
        "{}",
        run.stderr
    );
    let app = fs::read_to_string(scratch.path("App.tsx")).unwrap();
    let line = app
        .lines()
        .position(|line| line.contains("useInner(name)"))
        .unwrap()
        + 1;
    let findings = run.json()["findings"].as_array().unwrap().clone();
    assert!(
        !findings
            .iter()
            .any(|finding| finding["kind"] == "violation"
                && finding["primaryLocation"]["line"] == line),
        "{findings:#?}"
    );
    // `useLabel` in the same package is still analyzed from source.
    assert!(
        findings
            .iter()
            .any(|finding| finding["kind"] == "violation"),
        "{findings:#?}"
    );
}
