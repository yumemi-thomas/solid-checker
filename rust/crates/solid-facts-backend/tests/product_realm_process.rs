//! ADR 1145: the product realm. A site in a file the product's real entries load (an HTML page's module scripts, a package's `exports`, read from the build
//! configuration) is judged against the prototype writers those entries load, not against those only a test, a story, a script or a config loads.
//!
//! The mutation: `SOLID_CHECKER_IR_ABLATE_SILENCE=package_action_calls.package_action_calls.product-realm` drops the realm's typed silence (and the realm with it), so
//! every site is judged against every writer, as before: the rows come back, and nothing else moves. Each cleared site is named in the proof log.
use std::{collections::BTreeSet, env, fs, path::Path, process::Command};

use crate::process_support::temporary_directory;

const SILENCE: &str = "package_action_calls.package_action_calls.product-realm";

fn fixture(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../fixtures/reactive-ir")
        .join(name)
}

/// The checker arguments the fixture's `.solid-checker/runtime.json` declares.
fn runtime_arguments(name: &str) -> Vec<String> {
    let runtime: serde_json::Value = serde_json::from_slice(
        &fs::read(fixture(name).join(".solid-checker/runtime.json")).unwrap(),
    )
    .unwrap();
    let mut arguments = Vec::new();
    for (key, flag) in [
        ("target", "--runtime-target"),
        ("build", "--runtime-build"),
        ("rendering", "--rendering"),
    ] {
        if let Some(value) = runtime[key].as_str() {
            arguments.push(flag.to_owned());
            arguments.push(value.to_owned());
        }
    }
    arguments
}

/// The report of a one-shot check of the fixture `name`, with `ablate` the silences to drop and `log` the proof log to write.
fn check(
    typefacts: &str,
    name: &str,
    ablate: Option<&str>,
    log: Option<&Path>,
) -> Vec<serde_json::Value> {
    let mut command = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"));
    command
        .env("SOLID_TYPEFACTS_BIN", typefacts)
        .env("SOLID_CHECKER_DAEMON", "0")
        .env_remove("SOLID_CHECKER_IR_ABLATE_SILENCE")
        .env_remove("SOLID_CHECKER_IR_PROOF_LOG")
        .args(["--format", "json", "--project"])
        .arg(fixture(name).join("tsconfig.json"))
        .args(runtime_arguments(name));
    if let Some(ablate) = ablate {
        command.env("SOLID_CHECKER_IR_ABLATE_SILENCE", ablate);
    }
    if let Some(log) = log {
        command.env("SOLID_CHECKER_IR_PROOF_LOG", log);
    }
    let output = command.output().unwrap();
    assert!(
        output.status.success() || output.status.code() == Some(1),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    report["findings"].as_array().unwrap().clone()
}

/// A finding as a comparable string: code, kind, place (the blocker's wording is the writer's, which a realm changes without moving the site).
fn place(finding: &serde_json::Value) -> String {
    format!(
        "{} {} {}:{}:{}",
        finding["id"],
        finding["kind"],
        finding["primaryLocation"]["path"]
            .as_str()
            .unwrap()
            .rsplit('/')
            .next()
            .unwrap(),
        finding["primaryLocation"]["line"],
        finding["primaryLocation"]["column"],
    )
}

fn places(findings: &[serde_json::Value]) -> BTreeSet<String> {
    findings.iter().map(place).collect()
}

/// The SC2002 rows of the file `file`, by place.
fn rows_in(findings: &[serde_json::Value], file: &str) -> BTreeSet<String> {
    findings
        .iter()
        .filter(|finding| finding["id"] == "SC2002")
        .map(place)
        .filter(|row| row.contains(&format!(" {file}:")))
        .collect()
}

/// The `silent` lines of the proof log at `log` that name the realm's silence: the file's path and the span.
fn realm_silences(log: &Path) -> Vec<(String, u64, u64)> {
    silences(log, SILENCE)
}

/// The `silent` lines of the proof log at `log` whose proof is `proof`: the file's path and the span.
fn silences(log: &Path, proof: &str) -> Vec<(String, u64, u64)> {
    let mut paths = std::collections::HashMap::new();
    let mut found = Vec::new();
    for line in fs::read_to_string(log).unwrap().lines() {
        let line: serde_json::Value = serde_json::from_str(line).unwrap();
        match line["k"].as_str() {
            Some("path") => {
                paths.insert(
                    (line["q"].as_u64().unwrap(), line["p"].as_u64().unwrap()),
                    line["f"].as_str().unwrap().to_owned(),
                );
            }
            Some("silent") if line["proof"] == proof => {
                let at = (line["q"].as_u64().unwrap(), line["p"].as_u64().unwrap());
                found.push((
                    paths[&at].clone(),
                    line["s"].as_u64().unwrap(),
                    line["e"].as_u64().unwrap(),
                ));
            }
            _ => {}
        }
    }
    found
}

/// The product's own file of each fixture below, and the test file that imports it.
const CLEARED: [(&str, &str, &str); 6] = [
    // A page's module script (`index.html` names `/main.tsx`), the test setup loads the writers.
    ("product-realm-test-writer", "App.tsx", "app.test.tsx"),
    // A package's `exports` are the entries; no page.
    (
        "product-realm-package-exports",
        "index.tsx",
        "index.test.tsx",
    ),
    // tsconfig `paths` that the config has Vite apply: both resolve the specifier to the shim, so the page does not load the writer.
    (
        "product-realm-tsconfig-paths-applied",
        "App.tsx",
        "app.test.tsx",
    ),
    // The `?url` of an image is data, no module.
    ("product-realm-asset-query", "App.tsx", "app.test.tsx"),
    // A pinned Solid plugin with its diagnostics bridge off (`diagnostics: false`, no declaration): the page never loads `@solidjs/diagnostics`.
    ("product-realm-diagnostics-off", "App.tsx", "app.test.tsx"),
    // The entries import a Solid package (`@solidjs/router`) whose manifest depends on nothing that writes: the test's writer stays out of the page.
    ("product-realm-solid-scope-inert", "App.tsx", "app.test.tsx"),
];

#[test]
fn a_site_only_a_test_loaded_writer_makes_is_cleared_by_a_typed_silence_and_dropping_it_gives_the_row_back()
 {
    let typefacts = env::var("SOLID_TYPEFACTS_BIN").expect("arm the real Type Facts process");
    let directory = temporary_directory("product-realm-named");
    for (name, product, test) in CLEARED {
        let log = directory.join(format!("{name}.jsonl"));
        let named = check(&typefacts, name, None, Some(&log));
        let ablated = check(&typefacts, name, Some(SILENCE), None);
        // The page loads no writer: the product has no SC2002 row, and the test that imports it keeps its own.
        assert!(
            rows_in(&named, product).is_empty(),
            "{name}: {:?}",
            rows_in(&named, product)
        );
        assert!(
            !rows_in(&named, test).is_empty(),
            "{name}: a test is no product"
        );
        // The mutation: the rows come back, and nothing else moves.
        let before = places(&named);
        let after = places(&ablated);
        assert!(before.is_subset(&after), "{name}: an answer was lost");
        let added = after.difference(&before).cloned().collect::<BTreeSet<_>>();
        assert!(
            !added.is_empty(),
            "{name}: dropping the silence moved nothing"
        );
        assert_eq!(added, rows_in(&ablated, product), "{name}: {added:?}");
        for row in &added {
            assert!(
                row.contains("\"uncertifiable\""),
                "{name}: never a violation: {row}"
            );
        }
        assert_eq!(
            places(&check(&typefacts, name, Some("all"), None)),
            after,
            "{name}: `all` is the same ablation"
        );
        // Every cleared site is named in the proof log, in its file, on a span that overlaps its row.
        let silences = realm_silences(&log);
        let rows = ablated
            .iter()
            .filter(|finding| added.contains(&place(finding)))
            .collect::<Vec<_>>();
        for finding in rows {
            let location = &finding["primaryLocation"];
            let (path, start, end) = (
                location["path"].as_str().unwrap(),
                location["startByte"].as_u64().unwrap(),
                location["endByte"].as_u64().unwrap(),
            );
            assert!(
                silences
                    .iter()
                    .any(|(file, s, e)| file == path && *s < end && start < *e),
                "{name}: no proof-log line names the cleared site at {path} {start}..{end}"
            );
        }
    }
}

/// Fixtures where the realm is unknown, or the page loads a writer: nothing is cleared, and the realm's silence is not named.
const KEPT: [(&str, &str); 20] = [
    // A second page loads the writer and shares a module.
    ("product-realm-second-page", "App.tsx"),
    // A plugin the grammar knows by name, adding a load: a name proves nothing.
    ("product-realm-plugin-name", "App.tsx"),
    // The config imports a package the grammar does not read.
    ("product-realm-config-writer", "App.tsx"),
    // A module loaded by a computed specifier.
    ("product-realm-computed-load", "App.tsx"),
    // The page loads the named writer itself (the any-name writer only the setup loads).
    ("product-realm-product-writer", "App.tsx"),
    // The package named `vite` is a writer, and Vite puts code in the page in either build: the dev client in a development one.
    ("product-realm-dev-client", "App.tsx"),
    // The same in a production build, where the page gets Vite's `modulepreload` polyfill.
    ("product-realm-dev-client-production", "App.tsx"),
    // The closure equals Vite's resolution, or the read ends (review round 1): a `.js` beside the `.ts` Vite ranks first,
    ("product-realm-js-sibling", "App.tsx"),
    // a package `imports` entry read under another condition set,
    ("product-realm-subpath-imports", "App.tsx"),
    // a tsconfig `customConditions`,
    ("product-realm-custom-conditions", "App.tsx"),
    // tsconfig `paths` Vite does not apply,
    ("product-realm-tsconfig-paths", "App.tsx"),
    // the `?url` of a script,
    ("product-realm-url-script", "App.tsx"),
    // another Vite config file, and a script that gives `vite` a `--config`,
    ("product-realm-other-vite-config", "App.tsx"),
    ("product-realm-vite-script-config", "App.tsx"),
    // and a Solid plugin that is not an installed closure ADR 0270 pins (unpinned, and not installed).
    ("product-realm-plugin-unpinned", "App.tsx"),
    ("product-realm-plugin-missing", "App.tsx"),
    // The reach index does not walk a Solid package (`@solidjs/*`, the dialect's), so the writer its manifest depends on is no writer of its root, and the page
    // loads it all the same (review round 2): through a package the entries import,
    ("product-realm-solid-scope-dependency", "App.tsx"),
    // and through the Solid plugin's dev-serve diagnostics bridge, which puts the app's own `@solidjs/diagnostics` in the page: forced on, the plugin's own
    // detection deciding, or switched off beside a declaration of the package.
    ("product-realm-diagnostics-on", "App.tsx"),
    ("product-realm-diagnostics-auto", "App.tsx"),
    ("product-realm-diagnostics-declared", "App.tsx"),
];

#[test]
fn an_unknown_realm_or_a_writer_the_page_loads_clears_no_site() {
    let typefacts = env::var("SOLID_TYPEFACTS_BIN").expect("arm the real Type Facts process");
    let directory = temporary_directory("product-realm-kept");
    for (name, product) in KEPT {
        let log = directory.join(format!("{name}.jsonl"));
        let named = check(&typefacts, name, None, Some(&log));
        let ablated = check(&typefacts, name, Some(SILENCE), None);
        assert!(
            !rows_in(&named, product).is_empty(),
            "{name}: the product keeps its rows"
        );
        assert_eq!(
            places(&named),
            places(&ablated),
            "{name}: the realm cleared a site"
        );
        assert!(
            realm_silences(&log).is_empty(),
            "{name}: a silence was named where nothing is cleared"
        );
    }
}

/// ADR 1144: Vite's reviewed dev client. `HOST_SILENCE` drops the premise's typed silence (the hosts kept in every scope, as before).
const HOST_SILENCE: &str = "package_action_calls.package_action_calls.reviewed-host";

/// The projects of `fixtures/reactive-ir/vite-dev-client` where the premise holds (the root's own prototype writes clear two sites of `App.tsx`), and where
/// it does not (every site keeps its row).
const HOST_CLEARED: [&str; 4] = [
    "vite-dev-client",
    "vite-dev-client/variants/type-import",
    "vite-dev-client/variants/solid-plugin",
    "vite-dev-client/variants/vite-7",
];
const HOST_KEPT: [&str; 5] = [
    "vite-dev-client/variants/runtime-import",
    "vite-dev-client/variants/plugin",
    "vite-dev-client/variants/define",
    "vite-dev-client/variants/production",
    "vite-dev-client/variants/mismatch",
];

#[test]
fn a_site_only_the_reviewed_vite_root_makes_ends_in_a_named_silence_and_every_twin_keeps_its_row() {
    let typefacts = env::var("SOLID_TYPEFACTS_BIN").expect("arm the real Type Facts process");
    let directory = temporary_directory("reviewed-host");
    for name in HOST_CLEARED.into_iter().chain(HOST_KEPT) {
        let log = directory.join(format!("{}.jsonl", name.replace('/', "-")));
        let named = check(&typefacts, name, None, Some(&log));
        let ablated = check(&typefacts, name, Some(HOST_SILENCE), None);
        let (named_rows, ablated_rows) = (rows_in(&named, "App.tsx"), rows_in(&ablated, "App.tsx"));
        let cleared = silences(&log, HOST_SILENCE);
        // `Logged`'s `console.error` meets the package as a writer of the global `console`, whose members Vite 8's client replaces: never cleared.
        assert!(named_rows.iter().any(|row| row.contains("App.tsx:21:")), "{name}: the console read keeps its row");
        assert!(named_rows.is_subset(&ablated_rows), "{name}: the premise only clears");
        if HOST_CLEARED.contains(&name) {
            assert!(named_rows.len() < ablated_rows.len(), "{name}: the premise clears a site");
            assert_eq!(
                cleared.len(),
                ablated_rows.len() - named_rows.len(),
                "{name}: every cleared site is named in the proof log"
            );
        } else {
            assert_eq!(places(&named), places(&ablated), "{name}: the premise cleared a site");
            assert!(cleared.is_empty(), "{name}: a silence was named where nothing is cleared");
        }
    }
}
