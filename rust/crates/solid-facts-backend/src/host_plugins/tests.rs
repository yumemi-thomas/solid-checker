use super::*;

fn package(root: &Path, name: &str, manifest: &str, files: &[(&str, &str)]) {
    let dir = root.join("node_modules").join(name);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("package.json"), manifest).unwrap();
    for (path, text) in files {
        let path = dir.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
}

fn identity(directory: &Path, specifier: &str) -> Result<Closure, String> {
    closure(
        directory,
        specifier,
        &mut Vec::new(),
        &mut PackageAudits::new(),
        None,
    )
}

fn pin(closure: &Closure) -> Pin {
    Pin {
        name: closure.name.clone(),
        version: closure.version.clone(),
        digest: closure.digest.clone(),
    }
}

#[test]
fn a_pin_admits_only_the_exact_installed_closure_bytes_and_edges() {
    let scratch = std::env::temp_dir().join(format!("host-plugin-closure-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    package(
        &scratch,
        "@solidjs/vite-plugin",
        r#"{"name":"@solidjs/vite-plugin","version":"0.0.0-test","dependencies":{"helper":"1"},"peerDependencies":{"optional-peer":"1"}}"#,
        &[("dist/index.mjs", "export default () => ({name: 'solid'});")],
    );
    package(
        &scratch,
        "helper",
        r#"{"name":"helper","version":"1.0.0"}"#,
        &[("index.js", "export const conditions = ['solid'];")],
    );
    let reviewed = identity(&scratch, "@solidjs/vite-plugin").unwrap();
    let pins = [pin(&reviewed)];
    assert!(pinned(&pins, &reviewed));
    assert!(
        reviewed
            .rows
            .iter()
            .any(|row| row.ends_with(":optional-peer=absent"))
    );
    // The same bytes elsewhere are the same closure: location is no input.
    let moved = scratch.join("moved");
    fs::create_dir_all(&moved).unwrap();
    for name in ["@solidjs/vite-plugin", "helper"] {
        let target = moved.join("node_modules").join(name);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::rename(scratch.join("node_modules").join(name), &target).unwrap();
    }
    assert!(pinned(
        &pins,
        &identity(&moved, "@solidjs/vite-plugin").unwrap()
    ));

    // One changed dependency byte, a newly present optional peer, or a
    // removed dependency each name a different, unpinned closure.
    let helper = moved.join("node_modules/helper/index.js");
    fs::write(&helper, "export const conditions = ['node'];").unwrap();
    let modified = identity(&moved, "@solidjs/vite-plugin").unwrap();
    assert_eq!(modified.version, reviewed.version);
    assert!(!pinned(&pins, &modified));
    fs::write(&helper, "export const conditions = ['solid'];").unwrap();
    assert!(pinned(
        &pins,
        &identity(&moved, "@solidjs/vite-plugin").unwrap()
    ));
    package(
        &moved,
        "optional-peer",
        r#"{"name":"optional-peer","version":"1.0.0"}"#,
        &[],
    );
    assert!(!pinned(
        &pins,
        &identity(&moved, "@solidjs/vite-plugin").unwrap()
    ));
    fs::remove_dir_all(moved.join("node_modules/optional-peer")).unwrap();
    fs::remove_dir_all(moved.join("node_modules/helper")).unwrap();
    assert!(!pinned(
        &pins,
        &identity(&moved, "@solidjs/vite-plugin").unwrap()
    ));
    fs::remove_dir_all(scratch).unwrap();
}

/// Review helper and pin check against real installs, which are not
/// checked in. Prints each closure's identity and per-package rows;
/// `SOLID_CHECKER_EXPECT_PINNED=1` also requires every closure be pinned and
/// an exact copy of the plugin package be admitted and a
/// one-byte-modified copy refused.
///
/// `SOLID_CHECKER_PLUGIN_CLOSURES="<app dir>=<plugin>;..." cargo test -p
/// solid-facts-backend --lib host_plugins::tests::installed_closures -- --ignored --nocapture`
#[test]
#[ignore = "reads installs named by SOLID_CHECKER_PLUGIN_CLOSURES"]
fn installed_closures() {
    let expect = std::env::var("SOLID_CHECKER_EXPECT_PINNED").is_ok_and(|value| value == "1");
    let requests = std::env::var("SOLID_CHECKER_PLUGIN_CLOSURES")
        .expect("SOLID_CHECKER_PLUGIN_CLOSURES names <app dir>=<plugin> pairs");
    let pins: Vec<Pin> = serde_json::from_str(include_str!("../host-plugin-pins.json")).unwrap();
    for request in requests.split(';').filter(|request| !request.is_empty()) {
        let (directory, specifier) = request.split_once('=').expect("<app dir>=<plugin>");
        let closure = match identity(Path::new(directory), specifier) {
            Ok(closure) => closure,
            Err(refusal) if !expect => {
                println!("{directory} refused: {refusal}");
                continue;
            }
            Err(refusal) => panic!("{directory}: {refusal}"),
        };
        let admitted = pinned(&pins, &closure);
        println!(
            "{directory} {}@{} {} pinned={admitted}",
            closure.name, closure.version, closure.digest
        );
        for row in closure
            .rows
            .iter()
            .filter(|row| row.matches(':').count() == 1)
        {
            println!("  {row}");
        }
        if expect {
            assert!(admitted, "{directory}: closure is not pinned");
            let directory = Path::new(directory);
            assert!(
                pinned(&pins, &copied(directory, &closure, false)),
                "{}: an exact copy must be admitted",
                directory.display()
            );
            assert!(
                !pinned(&pins, &copied(directory, &closure, true)),
                "{}: a modified plugin package must be refused",
                directory.display()
            );
        }
    }
}

// Copy the plugin package, optionally with one appended manifest byte. Every
// package the original resolves (nested, then each ancestor's node_modules,
// nearest first) is linked at the same relative place, so only that byte can
// differ.
fn copied(directory: &Path, original: &Closure, modify: bool) -> Closure {
    let scratch = std::env::temp_dir().join(format!(
        "host-plugin-copy-{}-{}-{modify}",
        std::process::id(),
        original.digest
    ));
    let _ = fs::remove_dir_all(&scratch);
    let installed = installed(directory, &original.name, &mut Vec::new()).unwrap();
    let target = scratch.join("node_modules").join(&original.name);
    let mut files = Vec::new();
    tree(&installed, &mut files, &mut Vec::new()).unwrap();
    for file in &files {
        let to = target.join(file.strip_prefix(&installed).unwrap());
        fs::create_dir_all(to.parent().unwrap()).unwrap();
        fs::copy(file, &to).unwrap();
    }
    if modify {
        let manifest = target.join("package.json");
        let mut bytes = fs::read(&manifest).unwrap();
        bytes.push(b'\n');
        fs::write(&manifest, bytes).unwrap();
    }
    mirror(
        &installed.join("node_modules"),
        &target.join("node_modules"),
        "",
    );
    for ancestor in installed.ancestors().skip(1) {
        mirror(
            &ancestor.join("node_modules"),
            &scratch.join("node_modules"),
            &original.name,
        );
    }
    let copy = identity(&scratch, &original.name).unwrap();
    fs::remove_dir_all(scratch).unwrap();
    copy
}

fn mirror(from: &Path, to: &Path, skip: &str) {
    let Ok(entries) = fs::read_dir(from) else {
        return;
    };
    for entry in entries {
        let entry = entry.unwrap();
        let name = entry.file_name().to_str().unwrap().to_owned();
        if name.starts_with('.') {
            continue;
        }
        let packages = if name.starts_with('@') {
            fs::read_dir(entry.path())
                .unwrap()
                .map(|scoped| {
                    let scoped = scoped.unwrap();
                    (
                        format!("{name}/{}", scoped.file_name().to_str().unwrap()),
                        scoped.path(),
                    )
                })
                .collect()
        } else {
            vec![(name, entry.path())]
        };
        for (name, path) in packages {
            let link = to.join(&name);
            if name != skip
                && fs::symlink_metadata(&link).is_err()
                && path.join("package.json").is_file()
            {
                fs::create_dir_all(link.parent().unwrap()).unwrap();
                symlink(&fs::canonicalize(path).unwrap(), &link);
            }
        }
    }
}

#[cfg(unix)]
fn symlink(from: &Path, to: &Path) {
    std::os::unix::fs::symlink(from, to).unwrap();
}

#[cfg(not(unix))]
fn symlink(_: &Path, _: &Path) {
    panic!("copied-closure review needs symlinks");
}
