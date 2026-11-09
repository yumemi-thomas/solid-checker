//! ADR 0915: an input that exists and cannot be read is a missing fact,
//! never evidence of absence. A dangling `node_modules` link above the
//! project (a link into a deleted worktree) refused host inference, and the
//! ordinary analysis that follows a refusal certified the project that an
//! admitted host, or a real absence, leaves uncertifiable. The states git
//! cannot hold (a dangling link, a directory without permissions) are built
//! here from `inferred-host-alias-congruent`; the parse failures are the
//! committed fixtures `inferred-host-unreadable-manifest` and
//! `inferred-host-unreadable-package-manifest`. Each state is checked one-shot
//! and by a retained daemon, which must agree.
use std::{env, fs, path::Path, path::PathBuf, process::Command};

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

/// `(status, findings)` of one check of `tsconfig`, retained or one-shot.
fn check(
    typefacts: &str,
    tsconfig: &Path,
    sockets: &Path,
    retained: bool,
    args: &[&str],
) -> (String, Vec<serde_json::Value>) {
    let output = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
        .env("SOLID_TYPEFACTS_BIN", typefacts)
        .env("SOLID_CHECKER_DAEMON", if retained { "1" } else { "0" })
        .env("SOLID_CHECKER_DAEMON_IDLE_SECS", "10")
        .env("TMPDIR", sockets)
        .args(["--format", "json", "--project"])
        .arg(tsconfig)
        .args(args)
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let snapshot: serde_json::Value =
        serde_json::from_slice(&output.stdout).unwrap_or_else(|error| panic!("{error}: {stderr}"));
    (
        snapshot["status"].as_str().unwrap().to_owned(),
        snapshot["findings"].as_array().unwrap().clone(),
    )
}

/// The `analysisContext` of every uncertifiable row with code `code`.
fn blockers(findings: &[serde_json::Value], code: &str) -> Vec<String> {
    findings
        .iter()
        .filter(|finding| finding["id"] == code && finding["kind"] == "uncertifiable")
        .map(|finding| finding["analysisContext"].as_str().unwrap_or("").to_owned())
        .collect()
}

/// Whether the current user can still list `path` after its mode was
/// cleared: true for root, for whom a permission error cannot be built.
fn still_readable(path: &Path) -> bool {
    fs::read_dir(path).is_ok()
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
}

#[cfg(unix)]
#[test]
fn an_unreadable_discovery_input_never_certifies() {
    let Ok(typefacts) = env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let root = env::temp_dir().join(format!("sc-hiu-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let sockets = root.join("s");
    fs::create_dir_all(&sockets).unwrap();
    let root = fs::canonicalize(&root).unwrap();
    let sockets = fs::canonicalize(&sockets).unwrap();

    // Each case: a name, the fixture copied to `<case>/app`, the state built
    // around it, the runtime arguments, the status, the SC9005 and SC9014
    // blockers expected. `None` for a status means "anything but certified".
    type Build = fn(&Path, &Path) -> bool;
    /// A name, the fixture, the state built, the arguments, the status, and
    /// the SC9005 and SC9014 blockers expected.
    type Case<'a> = (
        &'a str,
        &'a str,
        Build,
        &'a [&'a str],
        Option<&'a str>,
        Vec<String>,
        Vec<String>,
    );
    let healthy: Build = |_, _| true;
    let ancestor_directory: Build = |case, _| {
        fs::create_dir(case.join("node_modules")).unwrap();
        true
    };
    let ancestor_dangling: Build = |case, _| {
        std::os::unix::fs::symlink(
            case.join("deleted-worktree/node_modules"),
            case.join("node_modules"),
        )
        .unwrap();
        true
    };
    let ancestor_unreadable: Build = |case, _| {
        let directory = case.join("node_modules");
        fs::create_dir(&directory).unwrap();
        set_mode(&directory, 0o000);
        !still_readable(&directory)
    };
    let package_dangling: Build = |case, app| {
        let package = app.join("node_modules/reactive-package");
        fs::remove_dir_all(&package).unwrap();
        std::os::unix::fs::symlink(case.join("deleted-store/reactive-package"), &package).unwrap();
        true
    };
    let package_unreadable: Build = |_, app| {
        let package = app.join("node_modules/reactive-package");
        set_mode(&package, 0o000);
        !still_readable(&package)
    };
    let solid_dangling: Build = |case, app| {
        let package = app.join("node_modules/solid-js");
        fs::remove_dir_all(&package).unwrap();
        std::os::unix::fs::symlink(case.join("deleted-store/solid-js"), &package).unwrap();
        true
    };
    let solid_unreadable: Build = |_, app| {
        let package = app.join("node_modules/solid-js");
        set_mode(&package, 0o000);
        !still_readable(&package)
    };
    let explicit: &[&str] = &[
        "--runtime-target",
        "browser",
        "--runtime-build",
        "development",
        "--rendering",
        "csr",
        // The fixture's own `.solid-checker/runtime.json` selection.
        "--dependency-source",
        "dep-hooks",
    ];
    let host = "inferred-host-alias-congruent";
    let declared = "dependency-source-default-include";
    let unreadable = vec!["host-input-unreadable".to_owned()];
    let unclassified = vec!["solid-version-unclassified".to_owned()];
    let cases: Vec<Case> = vec![
        // The healthy twins: inference admits the host, which never certifies,
        // and no input is unobservable.
        (
            "healthy",
            host,
            healthy,
            &[],
            Some("uncertifiable"),
            vec![],
            vec![],
        ),
        (
            "ancestor node_modules directory",
            host,
            ancestor_directory,
            &[],
            Some("uncertifiable"),
            vec![],
            vec![],
        ),
        // The reported shape, and a directory the checker may not list.
        (
            "ancestor node_modules dangling",
            host,
            ancestor_dangling,
            &[],
            Some("uncertifiable"),
            unreadable.clone(),
            vec![],
        ),
        (
            "ancestor node_modules unreadable",
            host,
            ancestor_unreadable,
            &[],
            Some("uncertifiable"),
            unreadable.clone(),
            vec![],
        ),
        // A package directory the producer cannot read leaves its import
        // unresolved (SC9012): already closed, pinned here.
        (
            "package dangling",
            host,
            package_dangling,
            &[],
            None,
            vec![],
            vec![],
        ),
        (
            "package unreadable",
            host,
            package_unreadable,
            &[],
            None,
            vec![],
            vec![],
        ),
        // Dialect detection: the solid-js manifest cannot be read, so which
        // release runs is unknown (ADR 0700's notice), not a silent default.
        (
            "solid-js dangling",
            declared,
            solid_dangling,
            explicit,
            None,
            vec![],
            unclassified.clone(),
        ),
        (
            "solid-js unreadable",
            declared,
            solid_unreadable,
            explicit,
            None,
            vec![],
            unclassified.clone(),
        ),
        (
            "declared healthy",
            declared,
            healthy,
            explicit,
            Some("violation"),
            vec![],
            vec![],
        ),
    ];
    for (name, source, build, args, status, sc9005, sc9014) in cases {
        let case = root.join(name.replace(' ', "-"));
        let app = case.join("app");
        copy_tree(&fixture(source), &app);
        if !build(&case, &app) {
            // Root can read a directory with no permissions: that state cannot
            // be built here, and the case is the healthy one it then is.
            eprintln!("{name}: the permission error cannot be built as this user; skipped");
            continue;
        }
        let tsconfig = app.join("tsconfig.json");
        let (fresh_status, fresh) = check(&typefacts, &tsconfig, &sockets, false, args);
        let (retained_status, retained) = check(&typefacts, &tsconfig, &sockets, true, args);
        assert_eq!(
            (&retained_status, &retained),
            (&fresh_status, &fresh),
            "{name}: retained and one-shot differ"
        );
        match status {
            Some(status) => assert_eq!(fresh_status, status, "{name}: {fresh:#?}"),
            None => assert_ne!(fresh_status, "certified", "{name}: {fresh:#?}"),
        }
        assert_eq!(
            blockers(&fresh, "SC9005"),
            sc9005,
            "{name}: SC9005 {fresh:#?}"
        );
        let notices = blockers(&fresh, "SC9014");
        assert!(
            sc9014.iter().all(|blocker| notices.contains(blocker)),
            "{name}: SC9014 {fresh:#?}"
        );
        if sc9014.is_empty() {
            assert!(
                !notices
                    .iter()
                    .any(|blocker| blocker == "solid-version-unclassified"),
                "{name}: SC9014 {fresh:#?}"
            );
        }
    }
    for entry in walk(&root) {
        if fs::symlink_metadata(&entry).is_ok_and(|metadata| metadata.is_dir()) {
            set_mode(&entry, 0o755);
        }
    }
    fs::remove_dir_all(&root).unwrap();
}

/// Every directory below `root`, parents first, restoring access on the way
/// so the walk can descend.
#[cfg(unix)]
fn walk(root: &Path) -> Vec<PathBuf> {
    let mut found = vec![root.to_owned()];
    let mut index = 0;
    while index < found.len() {
        let directory = found[index].clone();
        index += 1;
        if !fs::symlink_metadata(&directory).is_ok_and(|metadata| metadata.is_dir()) {
            continue;
        }
        set_mode(&directory, 0o755);
        if let Ok(entries) = fs::read_dir(&directory) {
            found.extend(entries.flatten().map(|entry| entry.path()));
        }
    }
    found
}
