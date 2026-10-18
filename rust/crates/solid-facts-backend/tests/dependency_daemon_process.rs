//! ADR 0311 (which supersedes ADR 0296's retention of dependency-source
//! programs): a check whose effective `--dependency-source` selection is
//! nonempty is a one-shot check even when the daemon is requested, and answers
//! what a disabled daemon answers, cold, repeated, after each kind of input
//! change and after the change is restored. A check with no selection keeps its
//! daemon, and the base daemon regressions here (idle, truncation, membership,
//! failed refresh) declare no host, so none of them silently becomes a
//! one-shot test.
//! `dependency_fresh_native_process.rs` holds the freshness and routing cases.
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
        .arg(project.join("tsconfig.json"));
    // No selection is the base check. It declares no host, so the implicit
    // default, which a declared target and build turn on, cannot apply.
    if !selection.is_empty() {
        command.args([
            "--runtime-target",
            "browser",
            "--runtime-build",
            "development",
        ]);
    }
    for specifier in selection {
        command.args(["--dependency-source", specifier]);
    }
    command
}

fn check(typefacts: &str, project: &Path, selection: &[&str], retained: bool) -> Run {
    // `retained` asks for the daemon; ADR 0311: only a check with no selection
    // gets it.
    finish(
        command(typefacts, project, selection, retained),
        retained && selection.is_empty(),
    )
}

/// Run `command`, which must succeed, through a daemon iff `retained` says it
/// is expected to be.
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
fn a_requested_daemon_never_serves_a_dependency_source_check() {
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
            assert!(!selection.is_empty(), "{name}");
            let scratch = Scratch::new(name, &format!("same-{name}-{index}"));
            let sockets = SocketDir::new(&format!("same-{index}"));
            let context = format!("{name} {selection:?}");
            let fresh = check(&typefacts, &scratch.project, selection, false);
            assert!(
                scratch.siblings().is_empty(),
                "{context}: one-shot left its sibling"
            );
            let requested = || {
                let mut requested = command(&typefacts, &scratch.project, selection, true);
                requested.env("TMPDIR", &sockets.0);
                requested
            };
            let cold = finish(requested(), false);
            assert_eq!(cold.cache_hit(), None, "{context}");
            let warm = finish(requested(), false);
            assert_eq!(warm.cache_hit(), None, "{context}");
            assert_same(&cold, &fresh, &context);
            assert_same(&warm, &fresh, &context);
            assert!(sockets.sockets().is_empty(), "{context}: a daemon started");
            // Nothing is written into the project: the generated
            // configuration is a scratch file opened at a logical path.
            let siblings = scratch.siblings();
            assert!(siblings.is_empty(), "{context}: {siblings:?}");
        }
    }
}

/// A daemon-requested dependency-source check, a repeat, `mutate`, one more
/// and a forced disabled one, then the tree restored: every requested answer
/// is the disabled answer, none of them touches a daemon, the mutation moved
/// the findings (so the agreement is not vacuous), and the restoration returns
/// the first answer.
fn mutation_case(
    typefacts: &str,
    name: &str,
    tag: &str,
    selection: &[&str],
    prepare: impl FnOnce(&Scratch),
    mutate: impl FnOnce(&Scratch),
) {
    let scratch = Scratch::new(name, tag);
    let sockets = SocketDir::new(tag);
    prepare(&scratch);
    let requested = || {
        let mut requested = command(typefacts, &scratch.project, selection, true);
        requested.env("TMPDIR", &sockets.0);
        finish(requested, false)
    };
    let before = requested();
    let repeat = requested();
    assert_same(&repeat, &before, tag);
    let backup = scratch.root.join("backup");
    copy_tree(&scratch.project, &backup);
    mutate(&scratch);
    let after = requested();
    let fresh = check(typefacts, &scratch.project, selection, false);
    assert_same(&after, &fresh, tag);
    assert_ne!(
        before.json()["findings"],
        fresh.json()["findings"],
        "{tag}: the mutation moved nothing"
    );
    fs::remove_dir_all(&scratch.project).unwrap();
    copy_tree(&backup, &scratch.project);
    let restored = requested();
    assert_same(&restored, &before, tag);
    assert_same(
        &restored,
        &check(typefacts, &scratch.project, selection, false),
        tag,
    );
    assert!(sockets.sockets().is_empty(), "{tag}: a daemon started");
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

/// ADR 0322: a namespace import that starts escaping as a value (and, on
/// restore, stops) moves its redirection and its `NamespaceValue` obligation;
/// a requested daemon never answers either way.
#[test]
fn an_escaping_namespace_import_is_the_one_shot_answer() {
    let Ok(typefacts) = std::env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    mutation_case(
        &typefacts,
        "dependency-source-namespace-import",
        "namespace-escape",
        &["dep-ns/widget"],
        |_| {},
        |scratch| {
            edit(
                &scratch.path("App.tsx"),
                "const now = Widget.readNow(() => count());",
                "const all = Widget;\n  const now = all.readNow(() => count());",
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

    // No selection answers differently here, so the agreement above is not
    // vacuous.
    let unselected = ok(&[]);
    assert!(
        source_notes(&unselected).is_empty(),
        "{}",
        unselected.stderr
    );
    assert_ne!(
        String::from_utf8_lossy(&unselected.stdout),
        String::from_utf8_lossy(&defaulted.stdout),
        "no selection answers differently here"
    );

    // Without both a target and a build nothing is selected, without a note.
    for partial in [
        &["--runtime-target", "browser"][..],
        &["--runtime-build", "development"][..],
    ] {
        let run = ok(partial);
        assert!(source_notes(&run).is_empty(), "{partial:?}: {}", run.stderr);
    }

    // The contract opt-out is retired, and refused rather than ignored.
    let (success, refused) = invoke(
        &typefacts,
        &scratch.project,
        &with_host(&["--no-dependency-source"]),
        false,
    );
    assert!(!success);
    assert!(
        refused
            .stderr
            .contains("--no-dependency-source was removed"),
        "{}",
        refused.stderr
    );
}

#[test]
fn a_requested_daemon_for_the_default_is_one_shot_and_a_hostless_check_keeps_its_daemon() {
    let Ok(typefacts) = std::env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let scratch =
        Scratch::new("dependency-source-all", "default-retained").with_solid_vite_plugin();
    let fresh = invoke(&typefacts, &scratch.project, &HOST, false).1;
    let (cold_ok, cold) = invoke(&typefacts, &scratch.project, &HOST, true);
    let (warm_ok, warm) = invoke(&typefacts, &scratch.project, &HOST, true);
    assert!(cold_ok && warm_ok, "{}\n{}", cold.stderr, warm.stderr);
    // ADR 0311: the default is the effective selection `*`, so a requested
    // daemon is bypassed, cold and repeated.
    assert_eq!(cold.cache_hit(), None, "{}", cold.stderr);
    assert_eq!(warm.cache_hit(), None, "{}", warm.stderr);
    assert_same(&cold, &fresh, "default cold");
    assert_same(&warm, &fresh, "default warm");
    // The explicit `*` is the same one-shot check.
    let explicit = invoke(
        &typefacts,
        &scratch.project,
        &with_host(&["--dependency-source", "*"]),
        true,
    )
    .1;
    assert_eq!(explicit.cache_hit(), None, "{}", explicit.stderr);
    assert_same(&explicit, &fresh, "explicit `*` against the default");
    // A check declaring no host has no selection and keeps its daemon: a real
    // cold and a real warm hit.
    let hostless: [&str; 0] = [];
    let retained = invoke(&typefacts, &scratch.project, &hostless, true).1;
    let warm_retained = invoke(&typefacts, &scratch.project, &hostless, true).1;
    let one_shot = invoke(&typefacts, &scratch.project, &hostless, false).1;
    assert!(retained.cache_hit().is_some(), "{}", retained.stderr);
    assert_eq!(
        warm_retained.cache_hit(),
        Some(true),
        "{}",
        warm_retained.stderr
    );
    assert_same(&retained, &one_shot, "hostless");
    assert_same(&warm_retained, &one_shot, "hostless, warm");
    assert!(source_notes(&retained).is_empty(), "{}", retained.stderr);
}

// Review round 1 of ADR 0296.

/// Finding 1: a refresh that fails part-way has already read the edits it
/// failed on. The request after it must still answer them. This is a base
/// daemon case; the dependency-source failure case is one-shot and lives in
/// `dependency_fresh_native_process.rs`.
#[test]
fn an_aborted_refresh_does_not_consume_its_edits() {
    let Ok(typefacts) = std::env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    {
        let selection: &[&str] = &[];
        let tag = "aborted-base".to_owned();
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
        let broken = scratch.path("Zed.tsx");
        let original = fs::read(&broken).unwrap();
        fs::write(&broken, b"export {};\n// \xff\xfe\n").unwrap();
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

    /// Every daemon socket in this private directory, possibly none.
    fn sockets(&self) -> Vec<PathBuf> {
        fs::read_dir(&self.0)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| {
                path.extension()
                    .is_some_and(|extension| extension == "sock")
            })
            .collect()
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
        let mut command = command(&typefacts, &scratch.project, &[], true);
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
        let mut command = command(&typefacts, &scratch.project, &[], true);
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
    let fresh = check(&typefacts, &scratch.project, &[], false);
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
    // With no plugin installed the declared host selects nothing.
    let unselected = ok(&HOST);
    assert_ne!(stdout(&unselected), stdout(&explicit), "not vacuous");
    assert!(
        source_notes(&unselected).is_empty(),
        "{}",
        unselected.stderr
    );
    assert!(
        unselected.stderr.contains(NO_PLUGIN),
        "{}",
        unselected.stderr
    );

    let unselected_mode = |run: &Run, context: &str| {
        assert_eq!(stdout(run), stdout(&unselected), "{context}");
        assert!(source_notes(run).is_empty(), "{context}: {}", run.stderr);
        assert!(run.stderr.contains(NO_PLUGIN), "{context}: {}", run.stderr);
    };
    // A manifest naming no version is no installation, as for `solid-js`.
    let versionless = scratch.path("node_modules/vite-plugin-solid");
    fs::create_dir_all(&versionless).unwrap();
    fs::write(
        versionless.join("package.json"),
        r#"{"name":"vite-plugin-solid"}"#,
    )
    .unwrap();
    unselected_mode(&ok(&HOST), "a versionless plugin manifest");
    // SolidStart alone is not the plugin; the one it ships is.
    scratch.install("@solidjs/start");
    unselected_mode(&ok(&HOST), "SolidStart without its plugin");
    scratch.install("@solidjs/start/node_modules/vite-plugin-solid");
    assert_same(&ok(&HOST), &explicit, "the plugin SolidStart ships");
    fs::remove_dir_all(scratch.path("node_modules/@solidjs/start")).unwrap();
    unselected_mode(&ok(&HOST), "SolidStart removed");
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

/// A source-mode program is written to a scratch directory and opened at a
/// logical path beside the project's tsconfig, so a read-only project gets
/// the same source analysis as a writable one, one-shot and retained, by
/// default and when selected explicitly, and nothing is written into it.
#[cfg(unix)]
#[test]
fn a_read_only_project_is_analyzed_from_source() {
    let Ok(typefacts) = std::env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let scratch =
        Scratch::new("dependency-source-all", "default-read-only").with_solid_vite_plugin();
    let writable = invoke(
        &typefacts,
        &scratch.project,
        &with_host(&["--dependency-source", "*"]),
        false,
    )
    .1;
    let _read_only = ReadOnly::new(&scratch.project);
    for retained in [false, true] {
        for arguments in [HOST.to_vec(), with_host(&["--dependency-source", "*"])] {
            let (success, run) = invoke(&typefacts, &scratch.project, &arguments, retained);
            assert!(success, "retained {retained} {arguments:?}: {}", run.stderr);
            // Asking for the daemon never reaches one for a dependency-source
            // check (ADR 0311).
            assert_eq!(run.cache_hit(), None, "retained {retained}: {}", run.stderr);
            assert_eq!(
                stdout(&run),
                stdout(&writable),
                "retained {retained} {arguments:?}"
            );
            assert!(
                !run.stderr.contains("not writable"),
                "retained {retained}: {}",
                run.stderr
            );
        }
    }
    assert!(scratch.siblings().is_empty());
}

/// A namespace import inside a selected package's own source. Since ADR 0313
/// it does not take the package it names off source; since ADR 0322 a
/// namespace used only through static members binds each member like the
/// named import, so the call is proved exactly as the unedited
/// `import { readInner }` proves it.
#[test]
fn a_static_namespace_import_in_package_source_proves_its_call() {
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
        !run.notes().iter().any(|note| note
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
        findings
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
    // The namespace escaping as a value redirects nothing: the call is
    // proved by nothing.
    edit(
        &scratch.path("node_modules/dep-hooks/src/index.jsx"),
        "return inner.readInner(source);",
        "const all = inner;\n  return all.readInner(source);",
    );
    let (success, run) = invoke(&typefacts, &scratch.project, &HOST, false);
    assert!(success, "{}", run.stderr);
    let findings = run.json()["findings"].as_array().unwrap().clone();
    assert!(
        !findings
            .iter()
            .any(|finding| finding["kind"] == "violation"
                && finding["primaryLocation"]["line"] == line),
        "{findings:#?}"
    );
}
