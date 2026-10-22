//! ADR 0324: the router-parameters premise reads the installed router's
//! release, which no source file holds. A retained daemon must answer what a
//! one-shot check answers after the release moves between audited and
//! unaudited and back, with the premise settling the unmodeled use only on
//! the audited one, and the notice appearing only on the other.
use std::{env, fs, path::Path, path::PathBuf};

use crate::invocation_flow_process::cli_check_retaining;

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

fn router_notice(findings: &serde_json::Value) -> bool {
    findings
        .as_array()
        .unwrap()
        .iter()
        .any(|finding| finding["analysisContext"] == "router-release")
}

#[test]
fn a_router_release_moved_between_audited_and_unaudited_is_resolved_again() {
    let Ok(typefacts) = env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    for retention in ["performance", "compact"] {
        let root = std::env::temp_dir()
            .join("sc-scratch.noindex")
            .join(format!(
                "solid-checker-router-params-{retention}-{}",
                std::process::id()
            ));
        let _ = fs::remove_dir_all(&root);
        copy_tree(
            &PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../fixtures/reactive-ir/router-params"),
            &root.join("project"),
        );
        let directory = fs::canonicalize(root.join("project")).unwrap();
        let tsconfig = directory.join("tsconfig.json");
        let sockets = PathBuf::from(format!("/tmp/srp-{retention}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&sockets);
        fs::create_dir_all(&sockets).unwrap();
        let source = fs::read_to_string(directory.join("App.tsx")).unwrap();
        let manifest = directory.join("node_modules/@solidjs/router/package.json");
        let audited = fs::read_to_string(&manifest).unwrap();
        let unaudited = audited.replace("2.0.0-next.41", "2.0.0-next.18");
        assert_ne!(audited, unaudited);
        // The accessor form's `props.clusterId` (the first `fetchThing`
        // argument) is the settled site.
        let needle = "fetchThing(props.clusterId)";
        let settled = |findings: &serde_json::Value| {
            let at = source.find(needle).unwrap() + "fetchThing(".len();
            findings.as_array().unwrap().iter().all(|finding| {
                !(finding["id"] == "SC9012"
                    && finding["analysisContext"] == "parameter-use-unmodeled"
                    && finding["primaryLocation"]["startByte"] == at as u64)
            })
        };
        for (name, text, premise_applies) in [
            ("audited", audited.as_str(), true),
            ("unaudited", unaudited.as_str(), false),
            ("audited again", audited.as_str(), true),
        ] {
            fs::write(&manifest, text).unwrap();
            let check = |retained| {
                cli_check_retaining(&typefacts, &tsconfig, &sockets, retained, retention)
            };
            let (retained, _) = check(true);
            let (fresh, _) = check(false);
            assert_eq!(
                retained, fresh,
                "{retention}: {name}: retained and one-shot differ"
            );
            assert_eq!(
                settled(&fresh),
                premise_applies,
                "{retention}: {name}: settled"
            );
            assert_eq!(
                router_notice(&fresh),
                !premise_applies,
                "{retention}: {name}: notice"
            );
            let (repeat, hit) = check(true);
            assert_eq!(repeat, fresh, "{retention}: {name}: repeat");
            assert_eq!(
                hit,
                Some(true),
                "{retention}: {name}: an unchanged repeat is cached"
            );
        }
        fs::remove_dir_all(&root).unwrap();
        let _ = fs::remove_dir_all(&sockets);
    }
}

/// Runs a retained and a one-shot check at each `(name, edit, premise_applies)`
/// step of a copy of `fixture`, asserting both agree, that an unchanged repeat
/// is a cache hit, and that the premise settles the `Screen` site of `App.tsx`
/// exactly when it should.
/// One step: its name, the edit it applies, and whether the premise applies.
type Step<'a> = (&'a str, &'a dyn Fn(&Path), bool);

fn premise_follows_edits(tag: &str, fixture: &str, steps: &[Step]) {
    let Ok(typefacts) = env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    for retention in ["performance", "compact"] {
        let root = std::env::temp_dir()
            .join("sc-scratch.noindex")
            .join(format!(
                "solid-checker-{tag}-{retention}-{}",
                std::process::id()
            ));
        let _ = fs::remove_dir_all(&root);
        copy_tree(
            &PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../fixtures/reactive-ir")
                .join(fixture),
            &root.join("project"),
        );
        let directory = fs::canonicalize(root.join("project")).unwrap();
        let tsconfig = directory.join("tsconfig.json");
        let sockets = PathBuf::from(format!("/tmp/srp-{tag}-{retention}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&sockets);
        fs::create_dir_all(&sockets).unwrap();
        let source = fs::read_to_string(directory.join("App.tsx")).unwrap();
        let at = source.find("fetchThing(props.clusterId)").unwrap() + "fetchThing(".len();
        let settled = |findings: &serde_json::Value| {
            findings.as_array().unwrap().iter().all(|finding| {
                !(finding["id"] == "SC9012"
                    && finding["analysisContext"] == "parameter-use-unmodeled"
                    && finding["primaryLocation"]["startByte"] == at as u64)
            })
        };
        for (name, edit, premise_applies) in steps {
            edit(&directory);
            let check = |retained| {
                cli_check_retaining(&typefacts, &tsconfig, &sockets, retained, retention)
            };
            let (retained, _) = check(true);
            let (fresh, _) = check(false);
            assert_eq!(
                retained, fresh,
                "{fixture} {retention}: {name}: retained and one-shot differ"
            );
            assert_eq!(
                settled(&fresh),
                *premise_applies,
                "{fixture} {retention}: {name}: settled"
            );
            let (repeat, hit) = check(true);
            assert_eq!(repeat, fresh, "{fixture} {retention}: {name}: repeat");
            assert_eq!(
                hit,
                Some(true),
                "{fixture} {retention}: {name}: an unchanged repeat is cached"
            );
        }
        fs::remove_dir_all(&root).unwrap();
        let _ = fs::remove_dir_all(&sockets);
    }
}

#[test]
fn a_history_edited_in_another_file_moves_the_premise() {
    let stock = "history: memoryHistory(\"/a\")";
    let overridden = "history: { get: () => \"/\", set: () => {}, utils: { go: () => {} } }";
    let edit = |to: &'static str, from: &'static str| {
        move |directory: &Path| {
            let path = directory.join("router.ts");
            let text = fs::read_to_string(&path).unwrap();
            assert!(text.contains(from), "{from}");
            fs::write(&path, text.replace(from, to)).unwrap();
        }
    };
    let nothing = |_: &Path| {};
    let to_overridden = edit(overridden, stock);
    let to_stock = edit(stock, overridden);
    premise_follows_edits(
        "history",
        "router-params-stock-history",
        &[
            ("stock", &nothing, true),
            ("overridden", &to_overridden, false),
            ("stock again", &to_stock, true),
        ],
    );
}

#[test]
fn a_recorded_patch_of_the_router_moves_the_premise() {
    let record = |present: bool| {
        move |directory: &Path| {
            let path = directory.join("package.json");
            let manifest = if present {
                "{\"name\":\"router-params-stock-history\",\"private\":true,\"patchedDependencies\":{\"@solidjs/router@2.0.0-next.26\":\"patches/router.patch\"}}\n"
            } else {
                "{\"name\":\"router-params-stock-history\",\"private\":true}\n"
            };
            fs::write(path, manifest).unwrap();
        }
    };
    let (patched, unpatched) = (record(true), record(false));
    premise_follows_edits(
        "patch",
        "router-params-stock-history",
        &[
            ("unpatched", &unpatched, true),
            ("patched", &patched, false),
            ("unpatched again", &unpatched, true),
        ],
    );
}

/// The installed router's bytes are no program source (`dist/index.js` is not
/// a tracked file), so the daemon watches their digest: one appended byte turns
/// the premise off under a retained daemon exactly as a one-shot check does,
/// and restoring it turns the premise back on.
#[test]
fn an_installed_router_byte_edit_moves_the_premise() {
    let index = |directory: &Path| directory.join("node_modules/@solidjs/router/dist/index.js");
    let appended = |directory: &Path| {
        let path = index(directory);
        let mut text = fs::read(&path).unwrap();
        text.push(b'\n');
        fs::write(path, text).unwrap();
    };
    let restored = |directory: &Path| {
        let path = index(directory);
        let mut text = fs::read(&path).unwrap();
        assert_eq!(text.pop(), Some(b'\n'));
        fs::write(path, text).unwrap();
    };
    let nothing = |_: &Path| {};
    premise_follows_edits(
        "bytes",
        "router-params-stock-history",
        &[
            ("published", &nothing, true),
            ("one byte appended", &appended, false),
            ("restored", &restored, true),
        ],
    );
}
