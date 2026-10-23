//! Review of ADR 0372: a spread of a package call reads the package's entry
//! only under the premise ADR 0297's implicit source selection needs, a Solid
//! Vite plugin resolving from the project. `package-spread-shape`'s
//! `solid-condition` variant imports `cond-pkg`, whose `solid` entry returns
//! `class` and whose `default` entry also `onClick`. Every fixture under
//! `fixtures/reactive-ir` sees the group's plugin, so the case without one runs
//! on a copy outside the group, retained and one-shot, as the plugin comes and
//! goes.
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

/// `id:kind` of every finding, sorted.
fn findings(findings: &serde_json::Value) -> Vec<String> {
    let mut rows = findings
        .as_array()
        .unwrap()
        .iter()
        .map(|finding| {
            format!(
                "{}:{}",
                finding["id"].as_str().unwrap_or("?"),
                finding["kind"].as_str().unwrap_or("?")
            )
        })
        .collect::<Vec<_>>();
    rows.sort();
    rows
}

#[test]
fn a_package_entry_is_read_only_where_a_solid_vite_plugin_resolves() {
    let Ok(typefacts) = env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let root = std::env::temp_dir()
        .join("sc-scratch.noindex")
        .join(format!(
            "solid-checker-package-returns-{}",
            std::process::id()
        ));
    let _ = fs::remove_dir_all(&root);
    copy_tree(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../fixtures/reactive-ir/package-spread-shape"),
        &root.join("project"),
    );
    let directory = fs::canonicalize(root.join("project")).unwrap();
    let tsconfig = directory.join("variants/solid-condition/tsconfig.json");
    let plugin = directory.join("node_modules/@solidjs/vite-plugin");
    let sockets = PathBuf::from(format!("/tmp/spr-{}", std::process::id()));
    let _ = fs::remove_dir_all(&sockets);
    fs::create_dir_all(&sockets).unwrap();
    // Without the plugin: the spread's obligation, and the handler before it,
    // which the spread may replace, stays conditional.
    let obligation = vec![
        "SC4001:uncertifiable".to_owned(),
        "SC4001:uncertifiable".to_owned(),
    ];
    let violation = vec!["SC4001:violation".to_owned()];
    for (name, installed, expected) in [
        ("no plugin", false, &obligation),
        ("plugin", true, &violation),
        ("no plugin again", false, &obligation),
    ] {
        if installed {
            fs::create_dir_all(&plugin).unwrap();
            fs::write(
                plugin.join("package.json"),
                r#"{"name":"@solidjs/vite-plugin","version":"3.0.0-next.44"}"#,
            )
            .unwrap();
        } else {
            let _ = fs::remove_dir_all(&plugin);
        }
        for retained in [true, false] {
            let (found, _) =
                cli_check_retaining(&typefacts, &tsconfig, &sockets, retained, "performance");
            assert_eq!(
                &findings(&found),
                expected,
                "{name}, retained {retained}: {found:#}"
            );
        }
    }
    let _ = fs::remove_dir_all(&root);
    let _ = fs::remove_dir_all(&sockets);
}
